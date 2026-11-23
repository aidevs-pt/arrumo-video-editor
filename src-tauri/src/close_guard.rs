//! Asks before the window closes while videos are generating, because closing pauses them until
//! Arrumo opens again. A second close request while the question is open closes anyway, so an
//! unresponsive page can never keep the window open.

use std::sync::atomic::{AtomicBool, Ordering};

use arrumo_workspaces::WorkspaceStore;
use tauri::{Emitter, Manager, Window, WindowEvent};

#[derive(Default)]
pub struct CloseGuard {
    asking: AtomicBool,
    confirmed: AtomicBool,
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let guard = window.state::<CloseGuard>();
    if guard.confirmed.load(Ordering::SeqCst) || guard.asking.swap(true, Ordering::SeqCst) {
        return;
    }
    let generating = window
        .app_handle()
        .path()
        .app_local_data_dir()
        .ok()
        .and_then(|directory| WorkspaceStore::open(directory.join("workspaces.sqlite")).ok())
        .and_then(|store| store.active_generation_count().ok())
        .unwrap_or(0);
    if generating == 0
        || window
            .emit_to(window.label(), "close-requested", generating)
            .is_err()
    {
        guard.asking.store(false, Ordering::SeqCst);
        return;
    }
    api.prevent_close();
}

#[tauri::command]
pub fn answer_close_request(close: bool, window: Window, guard: tauri::State<'_, CloseGuard>) {
    if close {
        guard.confirmed.store(true, Ordering::SeqCst);
        let _ = window.close();
    } else {
        guard.asking.store(false, Ordering::SeqCst);
    }
}
