use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{MotionArtifact, WorkspaceStore};

#[derive(Deserialize, Serialize)]
pub(crate) struct CapturedMotionRenderer {
    plugin_id: String,
    package_sha256: String,
}

pub struct MotionWorkerPaths<'a> {
    pub worker: &'a Path,
    pub output: &'a Path,
    pub toolchain: Option<&'a Path>,
}

pub fn configured_motion_worker(bundled_worker: Option<PathBuf>) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let path = std::env::var_os("ARRUMO_MOTION_SDK_WORKER")
        .map(PathBuf::from)
        .or(bundled_worker)?;
    (path.is_absolute() && path.is_file()).then_some(path)
}

impl WorkspaceStore {
    pub(crate) fn capture_motion_renderer(
        &self,
        id: &str,
    ) -> rusqlite::Result<Option<CapturedMotionRenderer>> {
        self.0
            .query_row(
                "SELECT package_sha256 FROM installed_plugin_manifests
             WHERE plugin_id = ?1 AND package_bytes IS NOT NULL AND package_sha256 IS NOT NULL",
                [id],
                |row| {
                    Ok(CapturedMotionRenderer {
                        plugin_id: id.to_owned(),
                        package_sha256: row.get(0)?,
                    })
                },
            )
            .optional()
    }

    pub fn process_next_motion_stage(
        &self,
        database: &Path,
        worker: &Path,
    ) -> Result<Option<i64>, Box<dyn Error>> {
        let candidate: Option<(i64, i64)> = self.0.query_row(
            "SELECT run.workspace_id, run.id FROM generation_runs run
             JOIN installed_plugin_manifests plugin
               ON plugin.plugin_id = json_extract(run.snapshot_json, '$.motion_renderer.plugin_id')
              AND plugin.package_sha256 = json_extract(run.snapshot_json, '$.motion_renderer.package_sha256')
             WHERE plugin.package_bytes IS NOT NULL
               AND (run.state = 'queued' OR (run.state = 'running' AND run.lease_until <= unixepoch()))
               AND json_extract(run.plan_json, '$.steps[' || run.stage_index || '].stage') = 'assets'
             ORDER BY run.id LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        match candidate {
            Some((workspace_id, run_id)) => {
                self.process_queued_motion_stage(database, workspace_id, run_id, worker)
            }
            None => Ok(None),
        }
    }

    pub fn process_queued_motion_stage(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        worker: &Path,
    ) -> Result<Option<i64>, Box<dyn Error>> {
        let snapshot: Option<String> = self
            .0
            .query_row(
                "SELECT snapshot_json FROM generation_runs
             WHERE workspace_id = ?1 AND id = ?2
               AND (state = 'queued' OR (state = 'running' AND lease_until <= unixepoch()))
               AND json_extract(plan_json, '$.steps[' || stage_index || '].stage') = 'assets'",
                rusqlite::params![workspace_id, run_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(snapshot) = snapshot else {
            return Ok(None);
        };
        let snapshot: serde_json::Value = serde_json::from_str(&snapshot)?;
        let Some(renderer) = snapshot
            .get("motion_renderer")
            .filter(|value| !value.is_null())
        else {
            return Ok(None);
        };
        let renderer: CapturedMotionRenderer = serde_json::from_value(renderer.clone())?;
        self.render_run_motion_with_worker(
            database,
            workspace_id,
            run_id,
            &renderer.plugin_id,
            &renderer.package_sha256,
            worker,
        )
        .map(Some)
    }

    pub fn render_run_motion_with_worker(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        id: &str,
        digest: &str,
        worker: &Path,
    ) -> Result<i64, Box<dyn Error>> {
        let toolchain = std::env::var_os("ARRUMO_MOTION_TOOLCHAIN").map(PathBuf::from);
        let job = tempfile::tempdir()?;
        self.render_run_motion(
            database,
            workspace_id,
            run_id,
            id,
            digest,
            &MotionWorkerPaths {
                worker,
                output: &job.path().join("output"),
                toolchain: toolchain.as_deref(),
            },
        )
    }

    pub fn render_installed_motion(
        &self,
        id: &str,
        digest: &str,
        paths: &MotionWorkerPaths<'_>,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let bytes = self.reviewed_motion_renderer_package(id, digest)?;
        run_motion_worker(&bytes, digest, paths, || Ok(()))
    }

    pub fn render_run_motion(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        id: &str,
        digest: &str,
        paths: &MotionWorkerPaths<'_>,
    ) -> Result<i64, Box<dyn Error>> {
        let run = self
            .generation_run(workspace_id, run_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let order = &run.snapshot["provider_orders"]["motion-render"];
        if matches!(order["source"].as_str(), Some("app" | "workspace" | "run"))
            && !order["effective"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|provider| provider.as_str() == Some(id)))
        {
            return Err(io::Error::other(
                "The run's captured provider order excludes this renderer",
            )
            .into());
        }
        if let Some(renderer) = run
            .snapshot
            .get("motion_renderer")
            .filter(|value| !value.is_null())
        {
            let renderer: CapturedMotionRenderer = serde_json::from_value(renderer.clone())?;
            if renderer.plugin_id == id && !renderer.package_sha256.eq_ignore_ascii_case(digest) {
                return Err(
                    io::Error::other("The renderer digest differs from the run snapshot").into(),
                );
            }
        }
        let bytes = self.reviewed_motion_renderer_package(id, digest)?;
        let attempt = self
            .claim_queued_motion_stage(workspace_id, run_id)?
            .ok_or_else(|| io::Error::other("The selected run has no queued Assets stage"))?;
        let rendered = (|| -> Result<i64, Box<dyn Error>> {
            let mut last_renewal = std::time::Instant::now();
            let result = run_motion_worker(&bytes, digest, paths, || {
                if last_renewal.elapsed() >= std::time::Duration::from_secs(5) {
                    self.renew_generation_lease(workspace_id, run_id, attempt)?;
                    last_renewal = std::time::Instant::now();
                }
                Ok(())
            })?;
            let artifacts: Vec<MotionArtifact> = serde_json::from_slice(&result)?;
            self.reviewed_motion_renderer_package(id, digest)?;
            self.renew_generation_lease(workspace_id, run_id, attempt)?;
            Ok(self
                .submit_motion_artifacts(database, workspace_id, run_id, attempt, &artifacts)
                .map_err(io::Error::other)?)
        })();
        match rendered {
            Ok(result_id) => Ok(result_id),
            Err(error) => {
                match self.fail_generation(
                    workspace_id,
                    run_id,
                    attempt,
                    "Motion rendering or publication failed.",
                ) {
                    Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => {}
                    Err(failure) => return Err(failure.into()),
                }
                Err(error)
            }
        }
    }
}

fn run_motion_worker(
    bytes: &[u8],
    digest: &str,
    paths: &MotionWorkerPaths<'_>,
    mut check_lease: impl FnMut() -> rusqlite::Result<()>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err(io::Error::other("The local motion renderer worker requires Linux").into());
    }
    let worker = paths.worker.canonicalize()?;
    if !worker.is_file() {
        return Err(io::Error::other("The SDK worker must be a regular script file").into());
    }
    let job = tempfile::tempdir()?;
    let package = job.path().join("renderer.zip");
    std::fs::write(&package, bytes)?;
    let mut runner = std::process::Command::new("python3");
    #[cfg(target_os = "linux")]
    stop_worker_on_host_exit(&mut runner);
    runner
        .args(["-E", "-B"])
        .arg(worker)
        .arg(package)
        .arg("--sha256")
        .arg(digest.to_ascii_lowercase())
        .arg("--output")
        .arg(paths.output)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit());
    if let Some(path) = paths.toolchain {
        runner.arg("--toolchain").arg(path);
    }
    let reply = job.path().join("artifacts.json");
    let mut child = runner.stdout(File::create(&reply)?).spawn()?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if let Err(error) = check_lease() {
                    let _ = child.kill();
                    let _ = child.wait();
                    if error == rusqlite::Error::QueryReturnedNoRows {
                        return Err(
                            io::Error::other("The renderer no longer holds the run lease").into(),
                        );
                    }
                    return Err(error.into());
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    };
    if !status.success() {
        return Err(io::Error::other("The SDK renderer worker failed").into());
    }
    const MAX_WORKER_RESPONSE: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(reply)?
        .take(MAX_WORKER_RESPONSE + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_WORKER_RESPONSE {
        return Err(io::Error::other("The SDK renderer response exceeded its size limit").into());
    }
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn stop_worker_on_host_exit(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;

    let host_pid = std::process::id() as libc::pid_t;
    // The child runs this between fork and exec. Use only syscalls and
    // construct errors without allocating in the forked process.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != host_pid {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}
