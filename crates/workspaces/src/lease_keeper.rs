//! Keeps a run's lease while one long call, such as generating a clip, blocks the worker.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::WorkspaceStore;

/// Renews the lease from its own connection every 20 seconds until stopped.
pub(crate) struct LeaseKeeper {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<bool>>,
}

impl LeaseKeeper {
    pub(crate) fn start(database: &Path, workspace_id: i64, run_id: i64, attempt: i64) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let database: PathBuf = database.to_owned();
        let stopped = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            let Ok(store) = WorkspaceStore::open(&database) else {
                return false;
            };
            let mut renewed = Instant::now();
            while !stopped.load(Ordering::Relaxed) {
                if renewed.elapsed() >= Duration::from_secs(20) {
                    if store
                        .renew_generation_lease(workspace_id, run_id, attempt)
                        .is_err()
                    {
                        return false;
                    }
                    renewed = Instant::now();
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            true
        });
        Self {
            stop,
            worker: Some(worker),
        }
    }

    /// Stops renewing. False when the run lost its lease meanwhile, such as when it was stopped.
    pub(crate) fn finish(mut self) -> bool {
        self.stop.store(true, Ordering::Relaxed);
        self.worker
            .take()
            .is_some_and(|worker| worker.join().unwrap_or(false))
    }
}

impl Drop for LeaseKeeper {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
