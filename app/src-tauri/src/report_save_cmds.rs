//! 첨부 저장 (report-copy-save, D-174). The webview sends a task id and an attachment index only; this side finds the
//! report folder, reads the original again with the open rules, asks where to save in the native save panel and
//! writes there through a temporary file in the same folder. No path or content comes from the webview, and errors
//! go back as fixed codes (`busy`, `refused`, `failed`, `missing`), never an io message or a path.

use std::sync::atomic::{AtomicBool, Ordering};

use dduck_core::attachment::{self, AttachmentKind};
use dduck_core::task_meta;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{AppState, Res, reports_root};

/// One save panel at a time: a second press while it is open answers `busy` at once.
static OPEN: AtomicBool = AtomicBool::new(false);

struct PanelSlot;

impl PanelSlot {
    fn take() -> Option<Self> {
        OPEN.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).ok().map(|_| PanelSlot)
    }
}

impl Drop for PanelSlot {
    fn drop(&mut self) {
        OPEN.store(false, Ordering::Release);
    }
}

const BUSY: &str = "busy";
/// The original no longer passes the read rules (link, size, UTF-8, missing).
const REFUSED: &str = "refused";
const FAILED: &str = "failed";
/// The original file is gone (the report still lists it).
const MISSING: &str = "missing";

/// A failed read of the original as a fixed code: gone → `missing`, any other rule → `refused`.
fn read_code(status: dduck_core::evidence::Status) -> &'static str {
    if status == dduck_core::evidence::Status::Missing { MISSING } else { REFUSED }
}

/// `Ok(true)` saved, `Ok(false)` the panel was cancelled (nothing written).
#[tauri::command(async)]
pub fn save_report_attachment(app: AppHandle, state: State<'_, AppState>, id: String, index: usize) -> Res<bool> {
    if !task_meta::valid_bg_id(&id) {
        return Err(FAILED.into());
    }
    let Some(_slot) = PanelSlot::take() else { return Err(BUSY.into()) };
    let root = reports_root(&app).ok_or(FAILED)?;
    let target = dduck_core::task_report::target(&*state.conn()?, &id).map_err(|_| FAILED.to_string())?;
    // The DB lock is released above, before the file is read or the panel is shown.
    let (att, bytes) = dduck_core::task_report::attachment_original_of(&root, &target, index).map_err(|_| FAILED.to_string())?;
    let bytes = bytes.map_err(|s| read_code(s).to_string())?;
    let (filter, ext) = match att.kind {
        AttachmentKind::Html => ("HTML", "html"),
        AttachmentKind::Markdown => ("Markdown", "md"),
    };
    let mut panel = app.dialog().file().set_file_name(&att.file).add_filter(filter, &[ext]);
    if let Some(w) = app.get_webview_window("main") {
        panel = panel.set_parent(&w);
    }
    let Some(chosen) = panel.blocking_save_file() else { return Ok(false) };
    let dest = chosen.into_path().map_err(|_| FAILED.to_string())?;
    attachment::save_copy(&dest, &bytes, att.kind == AttachmentKind::Html).map_err(|_| FAILED.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_panel_slot_at_a_time() {
        let first = PanelSlot::take();
        assert!(first.is_some());
        assert!(PanelSlot::take().is_none(), "a second press is busy");
        drop(first);
        assert!(PanelSlot::take().is_some(), "free again once the panel closes");
    }

    #[test]
    fn a_missing_original_has_its_own_code() {
        use dduck_core::evidence::Status;
        assert_eq!(read_code(Status::Missing), "missing");
        for s in [Status::Refused, Status::TooLarge, Status::BadType] {
            assert_eq!(read_code(s), "refused", "{s:?}");
        }
    }
}
