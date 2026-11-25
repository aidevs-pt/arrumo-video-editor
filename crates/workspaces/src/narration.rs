//! Voice-over for storyboarded videos: the run's text-to-speech provider reads each scene's
//! narration once, at the preview, and scenes grow to fit it. The final render reuses the audio.

use std::{error::Error, fs, io, path::Path};

use serde_json::{Value, json};

use crate::{
    WorkspaceStore, delivery_directory, motion_spec::MotionSpec,
    source_transcripts::media_duration, speech_synthesis::speech_extension,
};

/// Silence kept after a scene's narration before the next scene starts.
const BREATH_SECONDS: f64 = 0.6;

/// Whether `name` is a narration file this module writes into a run's delivery directory.
pub(crate) fn narration_file_name(name: &str) -> bool {
    written_voice_name(name, "narration-")
}

/// Whether `name` is a voice-over line written for an edit of recorded footage.
pub(crate) fn voice_over_file_name(name: &str) -> bool {
    written_voice_name(name, "voice-")
}

fn written_voice_name(name: &str, prefix: &str) -> bool {
    let Some(rest) = name.strip_prefix(prefix) else {
        return false;
    };
    let Some((stem, extension)) = rest.rsplit_once('.') else {
        return false;
    };
    let Some((attempt, scene)) = stem.split_once('-') else {
        return false;
    };
    matches!(extension, "mp3" | "wav")
        && [attempt, scene].iter().all(|part| {
            !part.is_empty() && part.len() <= 6 && part.bytes().all(|b| b.is_ascii_digit())
        })
}

/// The text-to-speech providers the run captured, in order.
fn run_speech_providers(snapshot: &Value) -> Vec<String> {
    snapshot["provider_orders"]["text-to-speech"]["effective"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

impl WorkspaceStore {
    /// Voices the storyboard in `edit` when the run has a configured speech provider. A failure
    /// leaves the storyboard silent and says why in its summary.
    pub(crate) fn narrate_storyboard(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        snapshot: &Value,
        edit: &mut Value,
    ) -> Result<(), Box<dyn Error>> {
        let lines: Vec<String> = edit["narration"]
            .as_array()
            .map(|lines| {
                lines
                    .iter()
                    .map(|line| line.as_str().unwrap_or("").to_owned())
                    .collect()
            })
            .unwrap_or_default();
        if lines.iter().all(|line| line.trim().is_empty()) {
            return Ok(());
        }
        let providers = self.without_inactive(run_speech_providers(snapshot))?;
        let Some(plugin) = providers.first().map(String::as_str) else {
            return Ok(());
        };
        let configured = self
            .speech_provider_status(workspace_id, plugin)
            .is_ok_and(|status| status.credential_configured());
        if !configured {
            return Ok(());
        }
        match self.synthesize_narration(
            database,
            workspace_id,
            run_id,
            attempt,
            plugin,
            &lines,
            edit,
        ) {
            Ok(files) => edit["narration_audio"] = json!(files),
            Err(error) => {
                let summary = edit["summary"].as_str().unwrap_or("").to_owned();
                edit["summary"] = json!(format!(
                    "{summary} The narration could not be voiced ({error}), so this preview is silent."
                ));
            }
        }
        Ok(())
    }

    /// The speech provider and voice that read voice-over now, if one is on, has its key and has
    /// a voice chosen.
    pub(crate) fn current_voice(
        &self,
        workspace_id: i64,
    ) -> Result<Option<(String, String)>, Box<dyn Error>> {
        let Some(plugin) = self
            .current_providers(workspace_id, "text-to-speech")?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let configured = self
            .speech_provider_status(workspace_id, &plugin)
            .is_ok_and(|status| status.credential_configured());
        if !configured {
            return Ok(None);
        }
        Ok(self
            .narration_voice(workspace_id, &plugin)?
            .map(|voice| (plugin, voice)))
    }

    /// Voices each line in the edit's `voice_cues` and records its file. A failure drops the
    /// voice-over and says why in the summary.
    pub(crate) fn voice_cut_lines(
        &self,
        database: &Path,
        (workspace_id, run_id, attempt): (i64, i64, i64),
        (plugin, voice): (&str, &str),
        edit: &mut Value,
    ) -> Result<(), Box<dyn Error>> {
        let Some(cues) = edit["voice_cues"].as_array().cloned() else {
            return Ok(());
        };
        let directory = delivery_directory(database, workspace_id, run_id);
        fs::create_dir_all(&directory)?;
        let mut written = Vec::new();
        let result = (|| -> Result<Vec<Value>, Box<dyn Error>> {
            let extension = speech_extension(plugin)?;
            let mut voiced = Vec::with_capacity(cues.len());
            for (index, cue) in cues.iter().enumerate() {
                let text = cue["text"].as_str().unwrap_or("").trim();
                if text.is_empty() {
                    continue;
                }
                self.renew_generation_lease(workspace_id, run_id, attempt)?;
                let name = format!("voice-{attempt}-{index}.{extension}");
                let path = directory.join(&name);
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
                self.synthesize_speech_to_file(workspace_id, plugin, voice, text, &path)?;
                written.push(path);
                let mut cue = cue.clone();
                cue["file"] = json!(name);
                voiced.push(cue);
            }
            Ok(voiced)
        })();
        match result {
            Ok(voiced) => edit["voice_cues"] = json!(voiced),
            Err(error) => {
                for file in written {
                    let _ = fs::remove_file(file);
                }
                crate::report_failure_with(
                    "voice_over",
                    &"The voice-over could not be voiced",
                    &[
                        ("agent_plugin", plugin),
                        ("cause", &crate::error_chain(&*error)),
                    ],
                );
                if let Some(edit) = edit.as_object_mut() {
                    edit.remove("voice_cues");
                }
                let summary = edit["summary"].as_str().unwrap_or("").to_owned();
                edit["summary"] = json!(format!(
                    "{summary} The voice-over could not be voiced ({error}), so this version has none."
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn synthesize_narration(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: i64,
        attempt: i64,
        plugin: &str,
        lines: &[String],
        edit: &mut Value,
    ) -> Result<Vec<Option<String>>, Box<dyn Error>> {
        let voice = self
            .narration_voice(workspace_id, plugin)?
            .ok_or_else(|| io::Error::other("choose a voice in the speech plugin's settings"))?;
        let extension = speech_extension(plugin)?;
        let mut scenes: Vec<MotionSpec> = serde_json::from_value(edit["scenes"].clone())?;
        let directory = delivery_directory(database, workspace_id, run_id);
        fs::create_dir_all(&directory)?;
        let mut written = Vec::new();
        let result = (|| -> Result<Vec<Option<String>>, Box<dyn Error>> {
            let mut files = Vec::with_capacity(scenes.len());
            for (index, scene) in scenes.iter_mut().enumerate() {
                let text = lines.get(index).map(|line| line.trim()).unwrap_or("");
                if text.is_empty() {
                    files.push(None);
                    continue;
                }
                self.renew_generation_lease(workspace_id, run_id, attempt)?;
                let name = format!("narration-{attempt}-{index}.{extension}");
                let path = directory.join(&name);
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
                self.synthesize_speech_to_file(workspace_id, plugin, &voice, text, &path)?;
                written.push(path.clone());
                scene.extend_to(media_duration(&path)? + BREATH_SECONDS);
                files.push(Some(name));
            }
            Ok(files)
        })();
        match result {
            Ok(files) => {
                edit["scenes"] = serde_json::to_value(&scenes)?;
                Ok(files)
            }
            Err(error) => {
                for file in written {
                    let _ = fs::remove_file(file);
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_narration_names_are_accepted() {
        assert!(narration_file_name("narration-3-0.mp3"));
        assert!(narration_file_name("narration-12-40.wav"));
        assert!(voice_over_file_name("voice-2-7.mp3"));
        assert!(!voice_over_file_name("narration-2-7.mp3"));
        for name in [
            "narration-3.mp3",
            "narration-3-0.mp4",
            "../narration-3-0.mp3",
            "narration-a-0.mp3",
            "3.mp4",
        ] {
            assert!(!narration_file_name(name), "{name}");
        }
    }

    #[test]
    fn the_first_captured_speech_provider_narrates() {
        let snapshot =
            json!({"provider_orders": {"text-to-speech": {"effective": ["elevenlabs", "x"]}}});
        assert_eq!(run_speech_providers(&snapshot), ["elevenlabs", "x"]);
        assert!(run_speech_providers(&json!({})).is_empty());
    }
}
