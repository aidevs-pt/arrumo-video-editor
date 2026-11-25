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
            if let Some((dimensions, style)) = burned.get(&output.format_id) {
                let work = tempfile::tempdir()?;
                let captioned = work.path().join("captioned.mp4");
                burn_output(
                    &source_dir,
                    output.format_id,
                    edited.as_ref().map(|latest| latest.subtitles.as_str()),
                    *dimensions,
                    style,
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
                    || !crate::assets::is_canonical(&source)?
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

impl WorkspaceStore {
    /// Writes one format's MP4 with the latest subtitles drawn into the picture to a new file
    /// at `destination`. Only formats whose frozen delivery asks for a captioned copy qualify.
    pub fn export_captioned_output(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        format_id: i64,
        destination: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let snapshot = self.0.unchecked_transaction()?;
        let delivery = self.verified_delivery(workspace_id, run_id)?;
        if !delivery
            .verified_outputs
            .iter()
            .any(|output| output.format_id == format_id)
        {
            return Err(io::Error::other("That format has no verified delivery").into());
        }
        let snapshot_json: String = self.0.query_row(
            "SELECT snapshot_json FROM generation_runs WHERE workspace_id = ?1 AND id = ?2",
            rusqlite::params![workspace_id, run_id],
            |row| row.get(0),
        )?;
        let (dimensions, style) = burned_formats(&snapshot_json)?
            .remove(&format_id)
            .ok_or_else(|| {
                io::Error::other("That format does not ask for captions in the picture")
            })?;
        let edited = self.read_edited_subtitles(run_id, format_id, "", None)?;
        snapshot.commit()?;
        if destination.symlink_metadata().is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Export destination already exists",
            )
            .into());
        }
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let work = tempfile::tempdir_in(parent)?;
        let captioned = work.path().join("captioned.mp4");
        burn_output(
            &delivery_directory(database, workspace_id, run_id),
            format_id,
            edited.as_ref().map(|latest| latest.subtitles.as_str()),
            dimensions,
            &style,
            &captioned,
        )?;
        match fs::hard_link(&captioned, destination) {
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Err(error.into()),
            Err(_) => {
                fs::copy(&captioned, destination)?;
            }
            Ok(()) => {}
        }
        Ok(())
    }
}

/// Draws the edited subtitles, or the delivered SRT, into a format's delivered MP4.
fn burn_output(
    source_dir: &Path,
    format_id: i64,
    edited: Option<&str>,
    dimensions: (u32, u32),
    style: &crate::CaptionStyle,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    let subtitles = match edited {
        Some(latest) => latest.to_owned(),
        None => fs::read_to_string(source_dir.join(format!("{format_id}.srt")))?,
    };
    crate::delivery_captions::burn_captions(
        &source_dir.join(format!("{format_id}.mp4")).canonicalize()?,
        &subtitles,
        dimensions,
        style,
        output,
    )
}

/// A format's frame size and caption style.
type Burned = ((u32, u32), crate::CaptionStyle);

/// The frozen formats whose delivery adds a captioned copy, with their sizes and caption style.
fn burned_formats(
    snapshot: &str,
) -> Result<std::collections::BTreeMap<i64, Burned>, Box<dyn Error>> {
    let snapshot: serde_json::Value = serde_json::from_str(snapshot)?;
    Ok(snapshot["formats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["delivery"]["supplemental"]["burned_captions"] == true)
        .filter_map(|entry| {
            let format = &entry["format"];
            let style =
                serde_json::from_value(entry["delivery"]["supplemental"]["caption_style"].clone())
                    .unwrap_or_default();
            Some((
                format["id"].as_i64()?,
                (
                    (
                        u32::try_from(format["width"].as_u64()?).ok()?,
                        u32::try_from(format["height"].as_u64()?).ok()?,
                    ),
                    style,
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
            vec![(4, ((1080, 1920), crate::CaptionStyle::default()))]
        );
    }

    #[test]
    fn the_frozen_caption_style_reaches_the_burn() {
        let snapshot = serde_json::json!({"formats": [
            {"format": {"id": 5, "width": 1920, "height": 1080},
             "delivery": {"supplemental": {"burned_captions": true,
                "caption_style": {"font": "serif", "bold": false, "size": 70, "color": "#FFCC00",
                    "outline_color": "#000000", "outline": 5, "background": true, "position": "top", "margin": 12}}}}
        ]});
        let (_, style) = super::burned_formats(&snapshot.to_string())
            .unwrap()
            .remove(&5)
            .unwrap();
        assert_eq!(style.position, crate::CaptionPosition::Top);
        assert_eq!(
            (style.size, style.margin, style.background),
            (70, Some(12), true)
        );
    }
}
