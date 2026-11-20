use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use arrumo_workspaces::WorkspaceStore;
use tauri::{AppHandle, Emitter};

const IDLE_DELAY: Duration = Duration::from_millis(500);
const ERROR_DELAY: Duration = Duration::from_secs(2);

pub struct PluginJobWorker(Arc<AtomicBool>);

impl PluginJobWorker {
    pub fn start(app: AppHandle, database: PathBuf) -> io::Result<Self> {
        let stopped = Arc::new(AtomicBool::new(false));
        let thread_stopped = Arc::clone(&stopped);
        thread::Builder::new()
            .name("arrumo-plugin-jobs".into())
            .spawn(move || {
                while !thread_stopped.load(Ordering::Relaxed) {
                    match WorkspaceStore::open(&database) {
                        Ok(store) => run_jobs(&app, &store, &thread_stopped),
                        Err(_) => thread::sleep(ERROR_DELAY),
                    }
                }
            })?;
        Ok(Self(stopped))
    }

    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn run_jobs(app: &AppHandle, store: &WorkspaceStore, stopped: &AtomicBool) {
    while !stopped.load(Ordering::Relaxed) {
        match store.process_next_plugin_action_job() {
            Ok(Some(_)) => {
                let _ = app.emit("arrumo-plugin-jobs-changed", ());
            }
            Ok(None) => thread::sleep(IDLE_DELAY),
            Err(_) => {
                thread::sleep(ERROR_DELAY);
                return;
            }
        }
    }
}
