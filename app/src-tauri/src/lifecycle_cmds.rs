//! 리뷰 대기·정리 칸 (task-lifecycle, D-149 (7)·D-152): the board's DB-backed columns, "정리 끝" and its undo, the
//! local-only wrap-up checklist and "worktree 정리" behind a native confirm. Errors are fixed lines, never ids or paths.

use dduck_core::i18n::Fail;
use std::path::PathBuf;

use dduck_core::task_lifecycle::{self as tl, Refusal};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, Res, chrono_now_ms, err, reports_root, settings};
use dduck_core::i18n::{Code as C, ReasonCode, tr};

fn known(state: &AppState) -> Res<bool> {
    Ok(dduck_core::review_inbox::poll_known() && state.agents_error.read().map_err(err)?.is_none())
}

/// 리뷰 대기 (same total as the review badge) and the 정리 records the CLI no longer lists. `wrapup_cards`: how many
/// 정리 cards to return ("더 보기"; 40 by default, at most 500).
#[tauri::command(async)]
pub fn board_wrapup(state: State<'_, AppState>, wrapup_cards: Option<usize>) -> Res<tl::Board> {
    let sessions = state.agents.read().map_err(err)?.clone();
    let conn = state.conn()?;
    let since = settings::review_since(&conn, chrono_now_ms()).map_err(err)?;
    tl::board(&conn, &sessions, since, wrapup_cards.unwrap_or(tl::MAX_CARDS)).map_err(|_| Fail::BoardUnreadable.now())
}

/// "정리 끝". `id` is a session key (live card) or a background id (record card).
#[tauri::command(async)]
pub fn wrap_task(state: State<'_, AppState>, id: String) -> Res<()> {
    let sessions = state.agents.read().map_err(err)?.clone();
    let known = known(&state)?;
    let conn = state.conn()?;
    let since = settings::review_since(&conn, chrono_now_ms()).map_err(err)?;
    tl::wrap(&conn, &id, &sessions, known, since, chrono_now_ms()).map_err(|e| match e {
        dduck_core::Error::Config(tl::RESUME_PENDING) => Fail::WrapResumePending.now(),
        _ => Fail::NotWrappable.now(),
    })
}

/// "되돌리기" after "정리 끝".
#[tauri::command(async)]
pub fn unwrap_task(state: State<'_, AppState>, id: String) -> Res<bool> {
    let sessions = state.agents.read().map_err(err)?.clone();
    tl::unwrap(&*state.conn()?, &id, &sessions).map_err(|_| Fail::UndoFailed.now())
}

/// The listed session's folder and whether it still runs (by the last listing; unknown counts as running).
fn live(state: &AppState, bg_id: &str) -> Res<(Option<PathBuf>, bool)> {
    let sessions = state.agents.read().map_err(err)?;
    let s = sessions.iter().find(|s| s.id.as_deref() == Some(bg_id));
    let running = !known(state)? || s.is_some_and(|s| dduck_core::review_inbox::running_of(s.status).is_some());
    Ok((s.map(|s| PathBuf::from(&s.cwd)).filter(|p| p.is_absolute()), running))
}

/// The checklist, read when a 정리 card is opened (local git only).
#[tauri::command]
pub async fn wrapup_check(app: AppHandle, id: String) -> Res<tl::Check> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if !dduck_core::task_meta::valid_bg_id(&id) {
            return Err(Fail::BadTaskId.now());
        }
        let (cwd, running) = live(&state, &id)?;
        let root = reports_root(&app);
        // The DB lock covers the record read only; git runs after it is released.
        let rec = tl::record(&*state.conn()?, &id).map_err(err)?.ok_or_else(|| Fail::WrapupUnchecked.now())?;
        Ok(tl::check(&rec, cwd.as_deref(), running, root.as_deref()))
    })
    .await
    .map_err(err)?
}

fn refusal_text(r: Refusal) -> String {
    r.fail().now()
}

/// "worktree 정리": refused while running, with changes or untracked files, or outside `.claude/worktrees/`; then a
/// native confirm with the ignored-file count; then `git worktree remove` without `--force`. The branch stays.
/// `false` = cancelled.
#[tauri::command]
pub async fn remove_worktree(app: AppHandle, id: String) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if !dduck_core::task_meta::valid_bg_id(&id) {
            return Err(Fail::BadTaskId.now());
        }
        let (cwd, running) = live(&state, &id)?;
        let rec = tl::record(&*state.conn()?, &id).map_err(err)?.ok_or_else(|| refusal_text(Refusal::NoWorktree))?;
        let plan = tl::remove_plan(&rec, cwd.as_deref(), running).map_err(refusal_text)?;
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = app
            .dialog()
            .message(plan.confirm_text(dduck_core::i18n::current()))
            .title(tr(C::DlgWorktreeTitle))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnDelete), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        // A usage-limit resume may have been scheduled while the dialog was open (D-191).
        tl::still_removable(&*state.conn()?, &id).map_err(refusal_text)?;
        // Re-checked after the dialog with a fresh listing: the task may have restarted while it was open.
        let fresh = state.client()?.list_agents().map_err(|_| refusal_text(Refusal::Running))?;
        if fresh.iter().any(|s| s.id.as_deref() == Some(id.as_str()) && dduck_core::review_inbox::running_of(s.status).is_some()) {
            return Err(refusal_text(Refusal::Running));
        }
        tl::remove(&plan).map_err(|r| if r == Refusal::Changes { refusal_text(r) } else { Fail::WorktreeRemoveFailed.now() })?;
        Ok(true)
    })
    .await
    .map_err(err)?
}
