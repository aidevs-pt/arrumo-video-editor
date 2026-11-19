use std::{
    error::Error,
    fs::{self, File},
    io,
    path::Path,
};

use tempfile::NamedTempFile;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{WorkspaceStore, delivery_directory};

impl WorkspaceStore {
    pub fn export_generation_delivery(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        destination: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let delivery = self.verified_delivery(workspace_id, run_id)?;
        if delivery.verified_outputs.is_empty() {
            return Err(io::Error::other("No verified delivery files to export").into());
        }
        if destination.symlink_metadata().is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Export destination already exists",
            )
            .into());
        }

        let source_dir = delivery_directory(database, workspace_id, run_id);
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut pending = NamedTempFile::new_in(parent)?;
        let mut archive = ZipWriter::new(pending.as_file_mut());
        for output in delivery.verified_outputs {
            for (recorded, extension) in [(output.video, "mp4"), (output.subtitles, "srt")] {
                let filename = format!("{}.{extension}", output.format_id);
                if recorded != filename {
                    return Err(io::Error::other("Unexpected filename in verified delivery").into());
                }
                let source = source_dir.join(&filename);
                let metadata = fs::symlink_metadata(&source)?;
                if !metadata.is_file() || metadata.len() == 0 {
                    return Err(
                        io::Error::other(format!("Invalid delivery file: {filename}")).into(),
                    );
                }
                let mut file = File::open(source)?;
                // ZIP64 supports source videos that exceed the ZIP32 size limit.
                archive.start_file(
                    filename,
                    SimpleFileOptions::default().large_file(metadata.len() >= u32::MAX.into()),
                )?;
                let copied = io::copy(&mut file, &mut archive)?;
                if copied != metadata.len() {
                    return Err(io::Error::other("A delivery file changed during export").into());
                }
            }
        }
        archive.finish()?.sync_all()?;
        pending.persist_noclobber(destination)?;
        Ok(())
    }
}
