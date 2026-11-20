use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    path::Path,
};

use crate::{MotionArtifact, WorkspaceStore};

pub struct MotionWorkerPaths<'a> {
    pub worker: &'a Path,
    pub output: &'a Path,
    pub toolchain: Option<&'a Path>,
}

impl WorkspaceStore {
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
