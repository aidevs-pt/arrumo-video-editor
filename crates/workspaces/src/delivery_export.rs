use std::{
    error::Error,
    fs::{self, File},
    io::{self, Write},
    path::Path,
};

use tempfile::NamedTempFile;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{WorkspaceStore, asset_path, asset_root, delivery_directory};

impl WorkspaceStore {
    pub fn export_generation_delivery(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        destination: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let snapshot = self.0.unchecked_transaction()?;
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

        let outputs = delivery
            .verified_outputs
            .into_iter()
            .map(|output| {
                let edited = self.read_edited_subtitles(run_id, output.format_id, "", None)?;
                let translations = self
                    .subtitle_languages(run_id, output.format_id)?
                    .into_iter()
                    .map(|language| {
                        let revision = self
                            .read_edited_subtitles(run_id, output.format_id, &language, None)?
                            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                        Ok((language, revision))
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok((output, edited, translations))
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let snapshot_json: String = self.0.query_row(
            "SELECT snapshot_json FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
            rusqlite::params![workspace_id, run_id],
            |row| row.get(0),
        )?;
        let burned = burned_formats(&snapshot_json)?;
        let supplemental = self.completed_supplemental_text(workspace_id, run_id)?;
        let media = self.completed_supplemental_media(workspace_id, run_id)?;
        snapshot.commit()?;

        let source_dir = delivery_directory(database, workspace_id, run_id);
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut pending = NamedTempFile::new_in(parent)?;
        let mut archive = ZipWriter::new(pending.as_file_mut());
        for (output, edited, translations) in outputs {
            for (recorded, extension) in [(output.video, "mp4"), (output.subtitles, "srt")] {
                let filename = format!("{}.{extension}", output.format_id);
                if recorded != filename {
                    return Err(io::Error::other("Unexpected filename in verified delivery").into());
                }
                if let Some(latest) = edited.as_ref().filter(|_| extension == "srt") {
                    archive.start_file(filename, SimpleFileOptions::default())?;
                    archive.write_all(latest.subtitles.as_bytes())?;
                    continue;
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
            if let Some(&dimensions) = burned.get(&output.format_id) {
                let subtitles = match &edited {
                    Some(latest) => latest.subtitles.clone(),
                    None => {
                        fs::read_to_string(source_dir.join(format!("{}.srt", output.format_id)))?
                    }
                };
                let work = tempfile::tempdir()?;
                let captioned = work.path().join("captioned.mp4");
                crate::delivery_captions::burn_captions(
                    &source_dir
                        .join(format!("{}.mp4", output.format_id))
                        .canonicalize()?,
                    &subtitles,
                    dimensions,
                    &captioned,
                )?;
                let length = captioned.metadata()?.len();
                archive.start_file(
                    format!("{}-captioned.mp4", output.format_id),
                    SimpleFileOptions::default().large_file(length >= u32::MAX.into()),
                )?;
                io::copy(&mut File::open(&captioned)?, &mut archive)?;
            }
            for (language, revision) in translations {
                archive.start_file(
                    format!("{}.{language}.srt", output.format_id),
                    SimpleFileOptions::default(),
                )?;
                archive.write_all(revision.subtitles.as_bytes())?;
            }
        }
        for (index, entry) in supplemental.iter().enumerate() {
            archive.start_file(entry.filename(index), SimpleFileOptions::default())?;
            archive.write_all(entry.text.as_bytes())?;
        }
        if !media.is_empty() {
            let root = asset_root(&database.canonicalize()?);
            for (index, entry) in media.iter().enumerate() {
                let source = asset_path(&root, workspace_id, entry.asset_id);
                let metadata = source.symlink_metadata()?;
                if !metadata.is_file()
                    || source.canonicalize()? != source
                    || metadata.len() != u64::try_from(entry.byte_size)?
                {
                    return Err(io::Error::other("Supplemental media is unavailable").into());
                }
                let mut file = File::open(source)?;
                archive.start_file(
                    entry.filename(index),
                    SimpleFileOptions::default().large_file(metadata.len() >= u32::MAX.into()),
                )?;
                if io::copy(&mut file, &mut archive)? != metadata.len() {
                    return Err(io::Error::other("Supplemental media changed during export").into());
                }
            }
        }
        archive.finish()?.sync_all()?;
        pending.persist_noclobber(destination)?;
        Ok(())
    }
}

/// The frozen formats whose delivery adds a captioned copy, with their sizes.
fn burned_formats(
    snapshot: &str,
) -> Result<std::collections::BTreeMap<i64, (u32, u32)>, Box<dyn Error>> {
    let snapshot: serde_json::Value = serde_json::from_str(snapshot)?;
    Ok(snapshot["formats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["delivery"]["supplemental"]["burned_captions"] == true)
        .filter_map(|entry| {
            let format = &entry["format"];
            Some((
                format["id"].as_i64()?,
                (
                    u32::try_from(format["width"].as_u64()?).ok()?,
                    u32::try_from(format["height"].as_u64()?).ok()?,
                ),
            ))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_formats_that_ask_get_a_captioned_copy() {
        let snapshot = serde_json::json!({"formats": [
            {"format": {"id": 3, "width": 1920, "height": 1080}, "delivery": {"supplemental": {}}},
            {"format": {"id": 4, "width": 1080, "height": 1920},
             "delivery": {"supplemental": {"burned_captions": true}}}
        ]});
        let burned = super::burned_formats(&snapshot.to_string()).unwrap();
        assert_eq!(
            burned.into_iter().collect::<Vec<_>>(),
            vec![(4, (1080, 1920))]
        );
    }
}
