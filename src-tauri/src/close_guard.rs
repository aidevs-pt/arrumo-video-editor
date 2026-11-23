//! Asks before the window closes while videos are generating, because closing pauses them until
//! Arrumo opens again. A second close request while the question is open closes anyway, so an
//! unresponsive page can never keep the window open.

use std::sync::atomic::{AtomicBool, Ordering};

use arrumo_workspaces::WorkspaceStore;
use tauri::{AppHandle, Emitter, ExitRequestApi, Manager, Window, WindowEvent};

#[derive(Default)]
pub struct CloseGuard {
    asking: AtomicBool,
    confirmed: AtomicBool,
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && hold(window.app_handle())
    {
        api.prevent_close();
    }
}

/// Quitting from the app menu or Cmd+Q asks the same question as closing the window.
pub fn on_exit_requested(app: &AppHandle, api: &ExitRequestApi) {
    if hold(app) {
        api.prevent_exit();
    }
}

/// Whether to keep Arrumo open and ask first. Returns false when the creator already chose to
/// close, when the question is already open, or when nothing is generating.
fn hold(app: &AppHandle) -> bool {
    let guard = app.state::<CloseGuard>();
    if guard.confirmed.load(Ordering::SeqCst) || guard.asking.swap(true, Ordering::SeqCst) {
        return false;
    }
    let generating = app
        .path()
        .app_local_data_dir()
        .ok()
        .and_then(|directory| WorkspaceStore::open(directory.join("workspaces.sqlite")).ok())
        .and_then(|store| store.active_generation_count().ok())
        .unwrap_or(0);
    if generating == 0 || app.emit_to("main", "close-requested", generating).is_err() {
        guard.asking.store(false, Ordering::SeqCst);
        return false;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    true
}

#[tauri::command]
pub fn answer_close_request(close: bool, window: Window, guard: tauri::State<'_, CloseGuard>) {
    if close {
        guard.confirmed.store(true, Ordering::SeqCst);
        window.app_handle().exit(0);
    } else {
        guard.asking.store(false, Ordering::SeqCst);
    }
}
