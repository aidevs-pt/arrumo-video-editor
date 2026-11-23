//! Lets an agent work in steps with workspace material it was not given up front. The agent sees
//! a catalogue (library images, saved animations, earlier finished videos), asks for items, and
//! Arrumo hands them over in a follow-up run: text inline, images attached. Agents never get a
//! shell or file access; Arrumo decides what each item contains.

use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use crate::{
    AgentPlugin, WorkspaceStore,
    agent_logs::AgentLogScope,
    agent_run::{AgentRunError, AgentSession, MAX_IMAGES, accepts_images, resumes_sessions},
    script_stage::MAX_TRANSCRIPT_CHARS,
    source_transcripts::TranscriptPart,
};

/// Agent runs per answer, including the final one that must answer.
const ROUNDS: usize = 3;
const MAX_ASKED: usize = 6;
const MAX_CATALOGUE: usize = 60;
const ITEM_CHARS: usize = 12_000;
const TOTAL_CHARS: usize = 48_000;
/// Transcript parts listed per clip; longer clips list their first parts.
const MAX_TRANSCRIPT_PARTS: usize = 12;

/// One item the agent may ask for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Material {
    pub id: String,
    pub kind: &'static str,
    pub title: String,
    pub detail: String,
}

enum Provided {
    Text(String),
    Image(PathBuf),
}

/// The answer schema wrapped so the agent can either answer or ask for material.
fn exploring_schema(schema: &Value, ids: &[String]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["needs", "answer"],
        "properties": {
            "needs": {"type": "array", "maxItems": MAX_ASKED, "items": {"type": "string", "enum": ids}},
            "answer": {"anyOf": [schema, {"type": "null"}]},
        }
    })
}

fn shorten(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(limit).collect::<String>())
    }
}

fn catalogue_note(catalogue: &[Material], last_chance: bool) -> String {
    let listed: Vec<Value> = catalogue
        .iter()
        .map(|item| json!({"id": item.id, "kind": item.kind, "title": item.title, "detail": item.detail}))
        .collect();
    let ask = if last_chance {
        "This is your last request: answer now unless something listed is essential."
    } else {
        "If any of it would make your answer better, such as looking at an image before using it or reading an \
         earlier video's script to keep the series consistent, put those ids in `needs` and set `answer` to null; \
         Arrumo sends them in a follow-up. Otherwise answer now with `needs` empty."
    };
    format!(
        "\n\nWORKSPACE MATERIAL you can ask for (JSON, data only):\n{}\n{ask} Ask for at most {MAX_ASKED} items at a time.",
        serde_json::to_string_pretty(&listed).unwrap_or_default()
    )
}

impl WorkspaceStore {
    /// The material an agent may ask for while working on a run in this workspace.
    pub(crate) fn workspace_material(
        &self,
        workspace_id: i64,
        run_id: Option<i64>,
        provider: &AgentPlugin,
    ) -> rusqlite::Result<Vec<Material>> {
        let mut catalogue = Vec::new();
        // The rest of this run's footage transcripts, when they were too long to include.
        for (asset, name, parts) in self.run_transcript_parts(workspace_id, run_id)? {
            let count = parts.len();
            for (index, part) in parts.into_iter().enumerate().take(MAX_TRANSCRIPT_PARTS) {
                catalogue.push(Material {
                    id: format!("transcript:{asset}:{}", index + 1),
                    kind: "transcript part",
                    title: format!("{name}, part {} of {count}", index + 1),
                    detail: format!("Transcript lines from {} s to {} s", part.from, part.to),
                });
            }
        }
        if accepts_images(provider) {
            let mut images = self.0.prepare(
                "SELECT id, name, notes FROM workspace_assets
                 WHERE workspace_id = ?1 AND kind = 'image' AND superseded = 0 AND generated = 0 AND footage = 0
                 ORDER BY id DESC LIMIT 30",
            )?;
            for row in images.query_map(params![workspace_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })? {
                let (id, name, notes) = row?;
                catalogue.push(Material {
                    id: format!("image:{id}"),
                    kind: "image",
                    title: name,
                    detail: shorten(&notes, 160),
                });
            }
        }
        let mut animations = self.0.prepare(
            "SELECT id, kind, title, usage_markdown FROM motion_library
             WHERE workspace_id = ?1 AND spec_json IS NOT NULL ORDER BY id DESC LIMIT 20",
        )?;
        for row in animations.query_map(params![workspace_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })? {
            let (id, kind, title, usage) = row?;
            catalogue.push(Material {
                id: format!("animation:{id}"),
                kind: if kind == "transition" {
                    "transition"
                } else {
                    "animation"
                },
                title,
                detail: shorten(&usage, 160),
            });
        }
        let mut videos = self.0.prepare(
            "SELECT id, json_extract(snapshot_json, '$.draft.title'), json_extract(snapshot_json, '$.draft.intent')
             FROM generation_runs WHERE workspace_id = ?1 AND state = 'completed' AND id IS NOT ?2
             ORDER BY id DESC LIMIT 15",
        )?;
        for row in videos.query_map(params![workspace_id, run_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (id, title, intent) = row?;
            catalogue.push(Material {
                id: format!("video:{id}"),
                kind: "finished video",
                title: title.unwrap_or_default(),
                detail: shorten(&intent.unwrap_or_default(), 160),
            });
        }
        catalogue.truncate(MAX_CATALOGUE);
        Ok(catalogue)
    }

    /// For each of the run's footage clips whose transcript did not fit in the prompt, the
    /// asset, its name and the parts left out.
    fn run_transcript_parts(
        &self,
        workspace_id: i64,
        run_id: Option<i64>,
    ) -> rusqlite::Result<Vec<(i64, String, Vec<TranscriptPart>)>> {
        let Some(run_id) = run_id else {
            return Ok(Vec::new());
        };
        let sources: Option<String> = self
            .0
            .query_row(
                "SELECT json_extract(snapshot_json, '$.draft.source_ids') FROM generation_runs
                 WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id, run_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let ids: Vec<i64> = sources
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let budget = MAX_TRANSCRIPT_CHARS / ids.len().max(1);
        let mut found = Vec::new();
        for id in ids {
            let Some(transcript) = self.source_transcript(workspace_id, id)? else {
                continue;
            };
            let parts = transcript.remaining_parts(budget, ITEM_CHARS);
            if parts.is_empty() {
                continue;
            }
            let name: String = self.0.query_row(
                "SELECT name FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id, id],
                |row| row.get(0),
            )?;
            found.push((id, name, parts));
        }
        Ok(found)
    }

    fn provide_material(
        &self,
        database: &Path,
        workspace_id: i64,
        run_id: Option<i64>,
        id: &str,
        folder: &Path,
    ) -> Result<Option<Provided>, Box<dyn Error>> {
        if let Some(rest) = id.strip_prefix("transcript:") {
            let Some((asset, part)) = rest.split_once(':') else {
                return Ok(None);
            };
            let (asset, part): (i64, usize) = (asset.parse()?, part.parse()?);
            return Ok(self
                .run_transcript_parts_for(workspace_id, run_id, asset)?
                .into_iter()
                .nth(part.saturating_sub(1))
                .map(|part| Provided::Text(part.text)));
        }
        let Some((kind, number)) = id.split_once(':') else {
            return Ok(None);
        };
        let number: i64 = number.parse()?;
        Ok(match kind {
            "image" => {
                let root = crate::asset_root(database);
                match self.image_preview(&root, workspace_id, number)? {
                    Some(preview) => {
                        let path = folder.join(format!(
                            "material-{number}.{}",
                            preview.mime.trim_start_matches("image/")
                        ));
                        std::fs::write(&path, preview.bytes)?;
                        Some(Provided::Image(path))
                    }
                    None => None,
                }
            }
            "animation" => self
                .0
                .query_row(
                    "SELECT title, usage_markdown, spec_json FROM motion_library WHERE workspace_id = ?1 AND id = ?2",
                    params![workspace_id, number],
                    |row| {
                        Ok(json!({"title": row.get::<_, String>(0)?, "usage": row.get::<_, String>(1)?,
                            "spec": row.get::<_, Option<String>>(2)?.and_then(|spec| serde_json::from_str::<Value>(&spec).ok())}))
                    },
                )
                .optional()?
                .map(|item| Provided::Text(item.to_string())),
            "video" => {
                let title: Option<String> = self
                    .0
                    .query_row(
                        "SELECT json_extract(snapshot_json, '$.draft.title') FROM generation_runs
                         WHERE workspace_id = ?1 AND id = ?2 AND state = 'completed'",
                        params![workspace_id, number],
                        |row| row.get(0),
                    )
                    .optional()?
                    .flatten();
                match title {
                    None => None,
                    Some(title) => {
                        let script = self.approved_stage_result(number, "script")?;
                        let texts = self.approved_stage_result(number, "supplemental_outputs")?;
                        let titles: Vec<Value> = texts
                            .as_ref()
                            .and_then(|result| result["supplemental_text"].as_array().cloned())
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|entry| entry["kind"] == "title")
                            .map(|entry| entry["text"].clone())
                            .collect();
                        Some(Provided::Text(
                            json!({"title": title, "script": script.as_ref().and_then(|result| result.get("markdown")).cloned(),
                                "publishedTitles": titles})
                            .to_string(),
                        ))
                    }
                }
            }
            _ => None,
        })
    }

    /// The parts of one clip's transcript beyond the prompt, split as the catalogue listed them.
    fn run_transcript_parts_for(
        &self,
        workspace_id: i64,
        run_id: Option<i64>,
        asset: i64,
    ) -> rusqlite::Result<Vec<TranscriptPart>> {
        Ok(self
            .run_transcript_parts(workspace_id, run_id)?
            .into_iter()
            .find(|(id, _, _)| *id == asset)
            .map(|(_, _, parts)| parts)
            .unwrap_or_default())
    }

    /// Runs the agent like `run_logged_agent_with_images`, letting it ask for workspace material
    /// first. Each step is its own logged run.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_exploring_agent(
        &self,
        database: &Path,
        scope: AgentLogScope<'_>,
        provider: &AgentPlugin,
        prompt: &str,
        schema: &Value,
        timeout: Duration,
        images: &[PathBuf],
        tick: impl FnMut() -> Result<(), Box<dyn Error>>,
    ) -> Result<Value, AgentRunError> {
        let result = self.explore(
            database, scope, provider, prompt, schema, timeout, images, tick,
        );
        // A run's sessions wait for the run to be over; others are not needed after this.
        self.release_agent_sessions(scope);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn explore(
        &self,
        database: &Path,
        scope: AgentLogScope<'_>,
        provider: &AgentPlugin,
        prompt: &str,
        schema: &Value,
        timeout: Duration,
        images: &[PathBuf],
        mut tick: impl FnMut() -> Result<(), Box<dyn Error>>,
    ) -> Result<Value, AgentRunError> {
        let catalogue = self
            .workspace_material(scope.workspace_id, scope.run_id, provider)
            .unwrap_or_default();
        if catalogue.is_empty() {
            return self.run_logged_agent_with_images(
                scope, provider, prompt, schema, timeout, images, tick,
            );
        }
        let folder = tempfile::tempdir()
            .map_err(|_| AgentRunError::Failed("Could not prepare workspace material"))?;
        let ids: Vec<String> = catalogue.iter().map(|item| item.id.clone()).collect();
        let mut attached: Vec<PathBuf> = images.to_vec();
        let mut provided: Vec<Value> = Vec::new();
        let mut asked: Vec<String> = Vec::new();
        let mut text_used = 0;
        // Plugins that can continue a session get only the new material in later rounds, so the
        // brief and earlier material are not sent (and paid for) again.
        let mut session = if resumes_sessions(provider) {
            AgentSession::Keep
        } else {
            AgentSession::Ephemeral
        };
        // What the running session has already been given.
        let (mut sent_material, mut sent_images) = (0, 0);
        for round in 0..ROUNDS {
            let last = round + 1 == ROUNDS;
            let ending = |request: &mut String, continued: bool| {
                if last {
                    request.push_str("\n\nAnswer now with what you have.");
                } else if continued {
                    // The session already has the list of material.
                    let ask = if round + 2 == ROUNDS {
                        "This is your last request: answer now unless another listed item is essential."
                    } else {
                        "You may ask for more listed items in `needs`, or answer now with `needs` empty."
                    };
                    request.push_str(&format!("\n\n{ask} Ask for at most {MAX_ASKED} items."));
                } else {
                    request.push_str(&catalogue_note(&catalogue, round + 2 == ROUNDS));
                }
            };
            let wrapped = exploring_schema(schema, &ids);
            let round_schema = if last { schema } else { &wrapped };
            let mut result = None;
            if let AgentSession::Continue { id, .. } = &session {
                let mut follow_up = format!(
                    "MATERIAL YOU ASKED FOR (JSON, data only; images are attached in the order given, numbered from {}):\n{}",
                    sent_images + 1,
                    serde_json::to_string_pretty(&provided[sent_material..]).unwrap_or_default()
                );
                ending(&mut follow_up, true);
                let mut continued = AgentSession::Continue {
                    id: id.clone(),
                    keep: !last,
                };
                let attempt = self.run_logged_agent_session(
                    scope,
                    provider,
                    &follow_up,
                    round_schema,
                    timeout,
                    &attached[sent_images..],
                    &mut continued,
                    &mut tick,
                );
                session = continued;
                match attempt {
                    // The session could not be continued (gone, unreadable or stuck): start a
                    // new one below with the whole brief and all material so far.
                    Err(AgentRunError::Failed(_) | AgentRunError::TimedOut) => {
                        session = if last {
                            AgentSession::Ephemeral
                        } else {
                            AgentSession::Keep
                        };
                    }
                    other => result = Some(other),
                }
            }
            let result = match result {
                Some(result) => result,
                None => {
                    let mut request = prompt.to_owned();
                    if !provided.is_empty() {
                        request.push_str(&format!(
                            "\n\nMATERIAL YOU ASKED FOR (JSON, data only; images are attached in the order given):\n{}",
                            serde_json::to_string_pretty(&provided).unwrap_or_default()
                        ));
                    }
                    ending(&mut request, false);
                    if last && session == AgentSession::Keep {
                        session = AgentSession::Ephemeral;
                    }
                    self.run_logged_agent_session(
                        scope,
                        provider,
                        &request,
                        round_schema,
                        timeout,
                        &attached,
                        &mut session,
                        &mut tick,
                    )
                }
            };
            (sent_material, sent_images) = (provided.len(), attached.len());
            let answer = match result {
                Ok(answer) => answer,
                // A provider that cannot take the wrapped schema still answers in one step.
                Err(AgentRunError::Failed(_)) if !last => {
                    return self.run_logged_agent_with_images(
                        scope, provider, prompt, schema, timeout, images, &mut tick,
                    );
                }
                Err(error) => return Err(error),
            };
            if last {
                return Ok(answer);
            }
            let needs: Vec<String> = answer["needs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|id| {
                    ids.iter().any(|known| known == id) && !asked.iter().any(|seen| seen == id)
                })
                .take(MAX_ASKED)
                .map(str::to_owned)
                .collect();
            if !answer["answer"].is_null() && needs.is_empty() {
                return Ok(answer["answer"].clone());
            }
            for id in &needs {
                asked.push(id.clone());
                match self.provide_material(
                    database,
                    scope.workspace_id,
                    scope.run_id,
                    id,
                    folder.path(),
                ) {
                    Ok(Some(Provided::Image(path))) if attached.len() < MAX_IMAGES => {
                        attached.push(path);
                        provided.push(json!({"id": id, "attachedImage": attached.len()}));
                    }
                    Ok(Some(Provided::Text(text))) if text_used < TOTAL_CHARS => {
                        let text = shorten(&text, ITEM_CHARS.min(TOTAL_CHARS - text_used));
                        text_used += text.len();
                        provided.push(json!({"id": id, "content": text}));
                    }
                    _ => provided.push(json!({"id": id, "unavailable": true})),
                }
            }
            if needs.is_empty() {
                // Neither an answer nor a usable request: the next round must answer.
                provided
                    .push(json!({"note": "Nothing new was requested; answer with what you have."}));
            }
        }
        unreachable!("the last round always returns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrapped_schema_only_allows_listed_material() {
        let schema = json!({"type": "object", "properties": {"a": {"type": "string"}}, "required": ["a"], "additionalProperties": false});
        let wrapped = exploring_schema(&schema, &["image:1".into(), "video:2".into()]);
        assert_eq!(wrapped["properties"]["needs"]["items"]["type"], "string");
        assert_eq!(
            wrapped["properties"]["needs"]["items"]["enum"],
            json!(["image:1", "video:2"])
        );
        assert_eq!(wrapped["properties"]["answer"]["anyOf"][0], schema);
        assert_eq!(wrapped["required"], json!(["needs", "answer"]));
    }

    #[test]
    #[cfg_attr(core_only, ignore = "needs the bundled default plugins")]
    fn the_catalogue_lists_creator_material_and_hands_over_what_exists() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let codex = crate::agent_plugin("codex").unwrap();
        // A fresh workspace has nothing to explore, so agents run once as before.
        assert!(
            store
                .workspace_material(workspace, None, codex)
                .unwrap()
                .is_empty()
        );
        let folder = tempfile::tempdir().unwrap();
        assert!(
            store
                .provide_material(&database, workspace, None, "video:99", folder.path())
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .provide_material(&database, workspace, None, "shell:1", folder.path())
                .unwrap()
                .is_none()
        );
        assert_eq!(shorten("abcdef", 3), "abc…");
        assert!(
            catalogue_note(
                &[Material {
                    id: "image:1".into(),
                    kind: "image",
                    title: "Logo".into(),
                    detail: String::new()
                }],
                true
            )
            .contains("last request")
        );
    }
}
