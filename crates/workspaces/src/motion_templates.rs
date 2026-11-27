//! Library animations as templates: a design plus the inputs it asks for each time it is used,
//! such as a step's title or a figure. A video fills them per use, so one library item serves
//! every step of a tutorial, and the library keeps a single asset.

use std::{error::Error, path::Path};

use rusqlite::{Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::motion_spec::{LayerType, MAX_TEXT, MotionSpec, clean_text};

pub(crate) const MAX_INPUTS: usize = 6;
const MAX_ADVICE: usize = 4;
const ADVICE_LENGTH: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Text,
    Number,
}

/// One thing a template asks for, with guidance for whoever fills it in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInput {
    pub key: String,
    pub label: String,
    pub kind: InputKind,
    /// Shown in the library preview and used when a video leaves the input empty.
    pub default: String,
    pub max_length: usize,
    pub tips: String,
    pub dos: Vec<String>,
    pub donts: Vec<String>,
}

fn placeholder(key: &str) -> String {
    format!("{{{{{key}}}}}")
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && key.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Keeps the inputs an agent declared that are well formed and that some text layer uses.
pub(crate) fn clean_inputs(spec: &mut MotionSpec) {
    let used = |key: &str| {
        spec.layers.iter().any(|layer| {
            layer.kind == LayerType::Text
                && layer
                    .text
                    .as_deref()
                    .is_some_and(|text| text.contains(&placeholder(key)))
        })
    };
    let mut kept: Vec<TemplateInput> = Vec::new();
    for mut input in std::mem::take(&mut spec.inputs) {
        if kept.len() == MAX_INPUTS
            || !valid_key(&input.key)
            || kept.iter().any(|other| other.key == input.key)
            || !used(&input.key)
        {
            continue;
        }
        input.max_length = input.max_length.clamp(1, MAX_TEXT);
        input.label = clean_text(&input.label, 60);
        if input.label.is_empty() {
            input.label = input.key.replace('_', " ");
        }
        input.default = clean_text(&input.default, input.max_length);
        if input.kind == InputKind::Number && input.default.parse::<f64>().is_err() {
            input.default = "0".into();
        }
        input.tips = clean_text(&input.tips, 300);
        for advice in [&mut input.dos, &mut input.donts] {
            advice.truncate(MAX_ADVICE);
            *advice = advice
                .iter()
                .map(|line| clean_text(line, ADVICE_LENGTH))
                .filter(|line| !line.is_empty())
                .collect();
        }
        kept.push(input);
    }
    spec.inputs = kept;
}

impl MotionSpec {
    /// The design with `values` filled into its text. Inputs left out take their defaults;
    /// keys the template doesn't ask for, or values that don't fit, are refused.
    pub(crate) fn instantiate(&self, values: &Map<String, Value>) -> Result<MotionSpec, String> {
        if let Some(key) = values
            .keys()
            .find(|key| !self.inputs.iter().any(|input| &input.key == *key))
        {
            return Err(format!("“{}” doesn't ask for {key}", self.title));
        }
        let mut filled = Vec::with_capacity(self.inputs.len());
        for input in &self.inputs {
            let value = match values.get(&input.key) {
                None | Some(Value::Null) => input.default.clone(),
                Some(Value::String(text)) => text.clone(),
                Some(Value::Number(number)) => number.to_string(),
                Some(_) => return Err(format!("{} takes text", input.label)),
            };
            let value = clean_text(&value, MAX_TEXT);
            if value.chars().count() > input.max_length {
                return Err(format!(
                    "{} takes up to {} characters",
                    input.label, input.max_length
                ));
            }
            if input.kind == InputKind::Number && value.parse::<f64>().is_err() {
                return Err(format!("{} takes a number", input.label));
            }
            filled.push((placeholder(&input.key), value));
        }
        let mut spec = self.clone();
        spec.inputs.clear();
        for layer in &mut spec.layers {
            if let Some(text) = &mut layer.text {
                for (mark, value) in &filled {
                    *text = text.replace(mark, value);
                }
                *text = clean_text(&strip_placeholders(text), MAX_TEXT);
            }
        }
        Ok(spec)
    }

    pub(crate) fn with_defaults(&self) -> MotionSpec {
        self.instantiate(&Map::new()).unwrap_or_else(|_| {
            let mut spec = self.clone();
            spec.inputs.clear();
            spec
        })
    }
}

impl MotionSpec {
    /// Fills a template the way an agent's placement asks, forgiving what an agent gets wrong:
    /// unknown keys are ignored, long values cut to fit and a number that isn't one left at its
    /// default. Answers the design and the values it used.
    pub(crate) fn fill_leniently(
        &self,
        pairs: &[(String, String)],
    ) -> (MotionSpec, Map<String, Value>) {
        let mut values = Map::new();
        for input in &self.inputs {
            let Some((_, value)) = pairs.iter().find(|(key, _)| key == &input.key) else {
                continue;
            };
            let value: String = clean_text(value, MAX_TEXT)
                .chars()
                .take(input.max_length)
                .collect();
            if value.is_empty()
                || (input.kind == InputKind::Number && value.parse::<f64>().is_err())
            {
                continue;
            }
            values.insert(input.key.clone(), Value::String(value));
        }
        let mut spec = self
            .instantiate(&values)
            .unwrap_or_else(|_| self.with_defaults());
        let shown: Vec<&str> = values.values().filter_map(Value::as_str).collect();
        if !shown.is_empty() {
            spec.title = clean_text(&format!("{} · {}", self.title, shown.join(", ")), 80);
        }
        (spec, values)
    }
}

impl crate::WorkspaceStore {
    /// The rendered preview of one use of a template in a run's asset plan.
    pub fn run_plan_use(
        &self,
        workspace_id: i64,
        run_id: i64,
        result_id: i64,
        index: usize,
    ) -> rusqlite::Result<crate::MotionLibrarySettings> {
        let entry: String = self.0.query_row(
            "SELECT json_extract(result.result_json, '$.asset_plan[' || ?4 || ']')
             FROM production_results AS result JOIN generation_runs AS run ON run.id = result.run_id
             WHERE run.workspace_id = ?1 AND run.id = ?2 AND result.id = ?3",
            rusqlite::params![workspace_id, run_id, result_id, index as i64],
            |row| row.get(0),
        )?;
        let entry: Value =
            serde_json::from_str(&entry).map_err(|_| rusqlite::Error::InvalidQuery)?;
        Ok(crate::MotionLibrarySettings {
            profile_id: None,
            kind: crate::MotionKind::Animation,
            title: entry["name"].as_str().unwrap_or_default().to_owned(),
            usage_markdown: String::new(),
            preview_asset_id: entry["preview_asset_id"]
                .as_i64()
                .ok_or(rusqlite::Error::InvalidQuery)?,
            thumbnail_asset_id: entry["thumbnail_asset_id"].as_i64(),
            sound_asset_id: None,
        })
    }

    /// The inputs a library animation asks for, with their guidance; empty when it is fixed.
    pub fn motion_template_inputs(
        &self,
        workspace_id: i64,
        item_id: i64,
    ) -> rusqlite::Result<Vec<TemplateInput>> {
        Ok(self
            .motion_spec_of(workspace_id, item_id)?
            .and_then(|spec| serde_json::from_value::<MotionSpec>(spec).ok())
            .map(|spec| spec.inputs)
            .unwrap_or_default())
    }

    /// Renders each filled use of a template in an asset plan, so the creator reviews every
    /// use with its real values; the library keeps the one template. Best effort: a use
    /// without a preview still plays in the video.
    pub(crate) fn render_plan_previews(
        &self,
        database: &Path,
        workspace_id: i64,
        size: (u32, u32),
        result: &mut Value,
        check: &mut dyn FnMut() -> Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(plan) = result["asset_plan"].as_array_mut() else {
            return Ok(());
        };
        let uses: Vec<(usize, MotionSpec)> = plan
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry["values"]
                    .as_object()
                    .is_some_and(|values| !values.is_empty())
            })
            .filter_map(|(index, entry)| {
                Some((index, serde_json::from_value(entry["spec"].clone()).ok()?))
            })
            .collect();
        if uses.is_empty() {
            return Ok(());
        }
        let scratch = crate::motion_scratch::MotionScratch::create(database)?;
        let specs: Vec<MotionSpec> = uses.iter().map(|(_, spec)| spec.clone()).collect();
        let artifacts = self.render_motion_artifacts(
            database,
            workspace_id,
            &specs,
            size,
            scratch.path(),
            check,
        )?;
        let root = crate::asset_root(&database.canonicalize()?);
        let tx = Transaction::new_unchecked(&self.0, TransactionBehavior::Immediate)?;
        for ((index, _), artifact) in uses.iter().zip(&artifacts) {
            let import = |source: &Path| -> Result<i64, Box<dyn Error>> {
                let asset =
                    crate::assets::copy_asset_in(&tx, &root, workspace_id, source, false, None)
                        .map_err(|error| -> Box<dyn Error> { error.to_string().into() })?;
                crate::assets::mark_generated(&tx, workspace_id, &[asset.id])?;
                Ok(asset.id)
            };
            plan[*index]["preview_asset_id"] = json!(import(&artifact.preview)?);
            if let Some(still) = &artifact.thumbnail {
                plan[*index]["thumbnail_asset_id"] = json!(import(still)?);
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// Removes `{{…}}` marks no input fills, so they are never drawn.
fn strip_placeholders(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        match rest[start..].find("}}") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = &rest[start..];
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The JSON schema of a template's inputs, added to the animation schema where the library
/// designs one.
pub(crate) fn inputs_schema() -> Value {
    let advice = json!({"type": "array", "maxItems": MAX_ADVICE, "items": {"type": "string", "maxLength": ADVICE_LENGTH}});
    json!({
        "type": "array",
        "maxItems": MAX_INPUTS,
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["key", "label", "kind", "default", "maxLength", "tips", "dos", "donts"],
            "properties": {
                "key": {"type": "string", "pattern": "^[a-z][a-z0-9_]{0,31}$"},
                "label": {"type": "string", "maxLength": 60},
                "kind": {"enum": ["text", "number"]},
                "default": {"type": "string", "maxLength": MAX_TEXT},
                "maxLength": {"type": "integer", "minimum": 1, "maximum": MAX_TEXT},
                "tips": {"type": "string", "maxLength": 300},
                "dos": advice,
                "donts": advice
            }
        }
    })
}

/// An animation schema that also declares template inputs.
pub(crate) fn template_schema(mut schema: Value) -> Value {
    schema["properties"]["inputs"] = inputs_schema();
    if let Some(required) = schema["required"].as_array_mut() {
        required.push(json!("inputs"));
    }
    schema
}

/// How templates are described to the agent that designs one for the library.
pub(crate) const TEMPLATE_GUIDE: &str = "Library animations are templates: design the look once and \
let each video fill in what changes. Declare an input for every piece of text a video would change \
(a name, a step title, a figure) and write it in a text layer as {{key}}; text that never changes, \
such as a fixed label, stays literal. Give each input a short label, kind text or number, a realistic \
default that shows the design at its best, maxLength (the longest value the design still holds), \
tips on what makes a good value, and up to four dos and donts. Keep keys lowercase, such as title \
or step_number. Design for the longest value too: when a box is the card behind a text layer, set \
the box's fitText to that text layer's index (its position in layers, from 0), and centre both on \
the same y; the card then grows taller for longer text. Leave fitText null on every other layer. \
Use no inputs for an animation with nothing to fill in, such as a transition.";

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn template() -> MotionSpec {
        let mut spec: MotionSpec = serde_json::from_value(json!({
            "title": "Step card", "kind": "animation", "usage": "", "durationSeconds": 3.0,
            "background": "#101010",
            "layers": [
                {"type": "box", "text": null, "assetId": null, "x": 0.5, "y": 0.5, "width": 0.6, "height": 0.2,
                 "size": 0.05, "color": "#2537B8", "bold": false, "opacity": 1.0, "start": 0.0, "end": 3.0,
                 "enter": "fade", "exit": "fade", "fitText": 1},
                {"type": "text", "text": "Step {{number}}: {{title}} {{unknown}}", "assetId": null, "x": 0.5, "y": 0.5,
                 "width": 0.5, "height": 0.1, "size": 0.05, "color": "#FFFFFF", "bold": true, "opacity": 1.0,
                 "start": 0.0, "end": 3.0, "enter": "fade", "exit": "fade"}
            ],
            "inputs": [
                {"key": "number", "label": "Step", "kind": "number", "default": "1", "maxLength": 3,
                 "tips": "", "dos": [], "donts": []},
                {"key": "title", "label": "Title", "kind": "text", "default": "Open the app", "maxLength": 40,
                 "tips": "A verb first", "dos": ["Start with a verb", ""], "donts": ["End with a full stop"]},
                {"key": "Bad Key", "label": "", "kind": "text", "default": "", "maxLength": 9,
                 "tips": "", "dos": [], "donts": []},
                {"key": "unused", "label": "", "kind": "text", "default": "", "maxLength": 9,
                 "tips": "", "dos": [], "donts": []}
            ]
        }))
        .unwrap();
        clean_inputs(&mut spec);
        spec
    }

    #[test]
    fn templates_keep_well_formed_inputs_their_text_uses() {
        let spec = template();
        let keys: Vec<&str> = spec.inputs.iter().map(|input| input.key.as_str()).collect();
        assert_eq!(keys, ["number", "title"]);
        assert_eq!(
            spec.inputs[1].dos,
            ["Start with a verb"],
            "empty advice goes"
        );
    }

    #[test]
    fn each_use_fills_its_own_values_and_never_draws_placeholders() {
        let spec = template();
        let defaults = spec.with_defaults();
        assert_eq!(
            defaults.layers[1].text.as_deref(),
            Some("Step 1: Open the app")
        );
        assert!(defaults.inputs.is_empty());

        let values = json!({"number": 3, "title": "Export your video"});
        let third = spec.instantiate(values.as_object().unwrap()).unwrap();
        assert_eq!(
            third.layers[1].text.as_deref(),
            Some("Step 3: Export your video")
        );

        for bad in [
            json!({"title": "x".repeat(41)}),
            json!({"number": "three"}),
            json!({"colour": "red"}),
            json!({"title": ["a"]}),
        ] {
            assert!(spec.instantiate(bad.as_object().unwrap()).is_err(), "{bad}");
        }
    }
}

#[cfg(test)]
mod render_tests {
    use serde_json::json;

    use crate::quiet_process::Launch;

    use crate::WorkspaceStore;

    #[test]
    fn each_use_in_a_plan_gets_its_own_rendered_preview() {
        if crate::quiet_command(crate::media_tools::ffmpeg())
            .arg("-version")
            .launch_output()
            .is_err()
        {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let spec = |title: &str| {
            json!({"title": title, "kind": "animation", "usage": "", "durationSeconds": 1.0, "background": "#101010",
                "layers": [{"type": "text", "text": title, "assetId": null, "x": 0.5, "y": 0.5, "width": 0.6,
                    "height": 0.1, "size": 0.06, "color": "#FFFFFF", "bold": true, "opacity": 1.0, "start": 0.0,
                    "end": 1.0, "enter": "fade", "exit": "fade"}]})
        };
        let mut result = json!({"asset_plan": [
            {"reference": "motion-1", "name": "Step", "values": {"title": "One"}, "spec": spec("One")},
            {"reference": "motion-1", "name": "Step", "values": {"title": "Two"}, "spec": spec("Two")},
            {"reference": "asset-3", "name": "Logo"}
        ]});
        store
            .render_plan_previews(
                &database,
                workspace,
                (320, 180),
                &mut result,
                &mut || Ok(()),
            )
            .unwrap();
        let plan = result["asset_plan"].as_array().unwrap();
        let first = plan[0]["preview_asset_id"].as_i64().unwrap();
        let second = plan[1]["preview_asset_id"].as_i64().unwrap();
        assert_ne!(first, second);
        assert!(plan[2].get("preview_asset_id").is_none());
        let root = crate::asset_root(&database.canonicalize().unwrap());
        assert!(crate::asset_path(&root, workspace, first).is_file());
        let hidden: bool = store
            .0
            .query_row(
                "SELECT generated FROM workspace_assets WHERE id = ?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        assert!(hidden, "previews stay out of the asset library");
    }
}
