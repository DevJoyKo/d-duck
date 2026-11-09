//! "삭제" on the extensions tab (extension-edit-delete, D-147): the user's own skills, slash commands and workflows go
//! to the macOS Trash. The webview sends a catalog id only; core `ext_trash` decides what may be moved and re-checks
//! it after the native confirmation. Nothing here logs or stores the path or the name.

use dduck_core::ext_trash::{self, TrashRoots};
use dduck_core::skills_catalog::SkillSource;
use dduck_core::text::display_safe;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, Res, catalog_roots, err, home_dir, settings};
use dduck_core::i18n::{Code as C, Params as P, ReasonCode, tr, trp};

fn kind_label(kind: &str) -> String {
    tr(match kind {
        "command" => C::KindCommand,
        "workflow" => C::KindWorkflow,
        _ => C::KindSkill,
    })
}

/// `true` when moved, `false` when cancelled. `unsaved` only adds a line to the dialog (the editor had changes).
#[tauri::command]
pub async fn trash_extension_item(app: AppHandle, id: String, unsaved: bool) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let item = crate::add_items::find_item(&state, &id)?;
        let roots = catalog_roots(&state)?;
        let troots = TrashRoots::from_configured(roots.config_dir.clone(), &dduck_core::skills_catalog::with_parents(&roots.config_dir, &roots.project_dirs), &home_dir());
        let plan = ext_trash::plan_trash(&item, &troots).map_err(|e| e.to_string())?;
        let users = if item.kind == "skill" {
            let agents = crate::fresh_catalog(&state)?;
            let stored = settings::team_config(&*state.conn()?).map_err(err)?;
            let types: Vec<_> = dduck_core::team_config::load(stored.as_deref()).types.into_iter().map(|t| t.def).collect();
            ext_trash::loadout_users(&item, plan.skill_token.as_deref(), &agents, &types).line()
        } else {
            None
        };
        let label = kind_label(plan.kind);
        let place = match (&item.source, &plan.project) {
            (SkillSource::Project, Some(p)) => trp(C::PlaceProject, &P::new().text("path", &p.display().to_string(), 200)),
            _ => tr(C::PlaceGlobalShort),
        };
        let mut msg = format!("{}{}\n", plan.disk_name, if plan.kind == "skill" { "/" } else { "" });
        let object = match plan.kind {
            "command" => C::DlgExtTrashCommand,
            "workflow" => C::DlgExtTrashWorkflow,
            _ => C::DlgExtTrashSkill,
        };
        msg.push_str(&trp(object, &P::new().text("name", &plan.name, 100)));
        if plan.kind == "skill" {
            msg.push_str(&format!("\n{}", trp(C::DlgExtTrashFolder, &P::new().num("count", plan.files as i64))));
        }
        msg.push_str(&format!("\n{place} · {}", display_safe(&plan.path.display().to_string(), 300)));
        if let Some(line) = users {
            msg.push('\n');
            msg.push_str(&line);
        }
        if unsaved {
            msg.push_str(&format!("\n{}", tr(C::DlgExtUnsaved)));
        }
        msg.push_str(&format!("\n\n{}", tr(C::DlgTrashRestore)));
        // Held until the move is done, like trash_agent: no second dialog can race this one.
        let _guard = state.confirm_lock.try_lock().map_err(|_| ReasonCode::ConfirmOpen.now())?;
        let ok = app
            .dialog()
            .message(msg)
            .title(trp(C::DlgExtTrashTitle, &P::new().text("kind", &label, 20)))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(tr(C::BtnTrash), tr(C::BtnCancel)))
            .blocking_show();
        if !ok {
            return Ok(false);
        }
        ext_trash::recheck(&plan).map_err(|e| e.to_string())?;
        crate::move_to_trash(&plan.path)?;
        Ok(true)
    })
    .await
    .map_err(err)?
}
