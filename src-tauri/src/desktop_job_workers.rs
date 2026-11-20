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

pub struct DesktopJobWorkers(Arc<AtomicBool>);

impl DesktopJobWorkers {
    pub fn start(app: AppHandle, database: PathBuf) -> io::Result<Self> {
        let workers = Self(Arc::new(AtomicBool::new(false)));
        if let Some(worker) = crate::media_worker::desktop_motion_worker(&app) {
            let motion_database = database.clone();
            workers.spawn(
                "arrumo-motion-jobs",
                database.clone(),
                Duration::from_secs(2),
                move |store| {
                    store
                        .process_next_motion_stage(&motion_database, &worker)
                        .map(|result| result.is_some())
                        .map_err(|_| ())
                },
            )?;
        }
        let script_database = database.clone();
        workers.spawn(
            "arrumo-script-writer",
            database.clone(),
            Duration::from_secs(2),
            move |store| {
                store
                    .process_next_script_stage(&script_database)
                    .map(|result| result.is_some())
                    .map_err(|_| ())
            },
        )?;
        let subtitles_database = database.clone();
        workers.spawn(
            "arrumo-delivery-subtitles",
            database.clone(),
            Duration::from_secs(2),
            move |store| {
                store
                    .process_next_automatic_subtitles(&subtitles_database)
                    .map_err(|_| ())
            },
        )?;
        let delivery_database = database.clone();
        workers.spawn(
            "arrumo-delivery-verification",
            database.clone(),
            Duration::from_secs(2),
            move |store| {
                store
                    .process_next_ready_delivery(&delivery_database)
                    .map_err(|_| ())
            },
        )?;
        workers.spawn(
            "arrumo-plugin-jobs",
            database,
            Duration::from_millis(500),
            move |store| {
                let processed = store
                    .process_next_plugin_action_job()
                    .map_err(|_| ())?
                    .is_some();
                if processed {
                    let _ = app.emit("arrumo-plugin-jobs-changed", ());
                }
                Ok(processed)
            },
        )?;
        Ok(workers)
    }

    fn spawn(
        &self,
        name: &str,
        database: PathBuf,
        idle_delay: Duration,
        process: impl Fn(&WorkspaceStore) -> Result<bool, ()> + Send + 'static,
    ) -> io::Result<()> {
        let stopped = Arc::clone(&self.0);
        thread::Builder::new().name(name.into()).spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                if let Ok(store) = WorkspaceStore::open(&database) {
                    while !stopped.load(Ordering::Relaxed) {
                        match process(&store) {
                            Ok(true) => {}
                            Ok(false) => thread::sleep(idle_delay),
                            Err(()) => break,
                        }
                    }
                }
                if !stopped.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(2));
                }
            }
        })?;
        Ok(())
    }

    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl Drop for DesktopJobWorkers {
    fn drop(&mut self) {
        self.stop();
    }
}
