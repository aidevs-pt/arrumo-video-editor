use std::{
    error::Error,
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
            },
        )?;
        let assets_database = database.clone();
        workers.spawn(
            "arrumo-asset-planner",
            database.clone(),
            Duration::from_secs(2),
            move |store| {
                store
                    .process_next_asset_plan(&assets_database)
                    .map(|result| result.is_some())
            },
        )?;
        let preview_database = database.clone();
        workers.spawn(
            "arrumo-first-cut",
            database.clone(),
            Duration::from_secs(2),
            move |store| {
                store
                    .process_next_preview_edit(&preview_database)
                    .map(|result| result.is_some())
            },
        )?;
        let publishing_database = database.clone();
        workers.spawn(
            "arrumo-publishing-text",
            database.clone(),
            Duration::from_secs(2),
            move |store| {
                store
                    .process_next_supplemental_writing(&publishing_database)
                    .map(|result| result.is_some())
            },
        )?;
        let render_database = database.clone();
        workers.spawn(
            "arrumo-final-render",
            database.clone(),
            Duration::from_secs(2),
            move |store| store.process_next_final_render(&render_database),
        )?;
        let subtitles_database = database.clone();
        workers.spawn(
            "arrumo-delivery-subtitles",
            database.clone(),
            Duration::from_secs(2),
            move |store| store.process_next_automatic_subtitles(&subtitles_database),
        )?;
        let delivery_database = database.clone();
        workers.spawn(
            "arrumo-delivery-verification",
            database.clone(),
            Duration::from_secs(2),
            move |store| store.process_next_ready_delivery(&delivery_database),
        )?;
        workers.spawn(
            "arrumo-agent-sessions",
            database.clone(),
            Duration::from_secs(300),
            |store| {
                store
                    .sweep_agent_sessions()
                    .map(|_| false)
                    .map_err(Into::into)
            },
        )?;
        workers.spawn(
            "arrumo-publish-dates",
            database.clone(),
            Duration::from_secs(600),
            |store| {
                store
                    .queue_publish_date_hooks()
                    .map(|_| false)
                    .map_err(Into::into)
            },
        )?;
        workers.spawn(
            "arrumo-plugin-jobs",
            database,
            Duration::from_millis(500),
            move |store| {
                let processed = store.process_next_plugin_action_job()?.is_some();
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
        process: impl Fn(&WorkspaceStore) -> Result<bool, Box<dyn Error>> + Send + 'static,
    ) -> io::Result<()> {
        let stopped = Arc::clone(&self.0);
        let worker = name.to_owned();
        thread::Builder::new().name(name.into()).spawn(move || {
            // Workers retry every few seconds, so each failure is reported once, until it changes.
            let mut last_failure = String::new();
            let mut failed = |error: &(dyn Error + 'static)| {
                let chain = arrumo_workspaces::error_chain(error);
                if chain != last_failure {
                    arrumo_workspaces::report_error("background", error, &[("worker", &worker)]);
                    last_failure = chain;
                }
            };
            while !stopped.load(Ordering::Relaxed) {
                match WorkspaceStore::open(&database) {
                    Ok(store) => {
                        while !stopped.load(Ordering::Relaxed) {
                            match process(&store) {
                                Ok(true) => {}
                                Ok(false) => thread::sleep(idle_delay),
                                Err(error) => {
                                    failed(&*error);
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => failed(&error),
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
