//! Canvas components: modules the agent writes once and canvas pages import afterwards. Each one
//! is a library item whose animation demonstrates it, so it shares the library's previews,
//! versions and profile sharing. Runs snapshot the components they could use, so a video renders
//! the same however its components change later.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::WorkspaceStore;

/// The largest module source, in bytes.
pub const MAX_COMPONENT_SOURCE: usize = 48 * 1024;
const MAX_OPTIONS: usize = 16;
/// Components offered to one agent step, newest first.
const MAX_OFFERED: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasComponent {
    /// Pages import the module as `workspace/<slug>`.
    pub slug: String,
    /// An ES module whose default export takes an element and options and answers `render(t)`.
    pub source: String,
    #[serde(default)]
    pub options: Vec<ComponentOption>,
    /// Canvas renderer libraries the module imports or reads.
    #[serde(default)]
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentOption {
    pub name: String,
    pub description: String,
}

impl CanvasComponent {
    pub fn import_path(&self) -> String {
        format!("workspace/{}", self.slug)
    }

    pub(crate) fn valid(&self) -> bool {
        valid_slug(&self.slug)
            && !self.source.trim().is_empty()
            && self.source.len() <= MAX_COMPONENT_SOURCE
            && self.options.len() <= MAX_OPTIONS
            && self.options.iter().all(|option| {
                !option.name.is_empty()
                    && option.name.len() <= 40
                    && option.description.len() <= 300
            })
            && self.libraries.len() <= 8
    }
}

/// Lowercase words joined by hyphens, as an import path segment.
pub(crate) fn valid_slug(slug: &str) -> bool {
    (2..=40).contains(&slug.len())
        && slug.starts_with(|c: char| c.is_ascii_lowercase())
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A component as the agent sees it in a brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OfferedComponent {
    pub title: String,
    pub usage: String,
    pub component: CanvasComponent,
}

/// How components are described to the agent next to the canvas guide.
pub(crate) const COMPONENTS_GUIDE: &str = "The brief's components are this workspace's own canvas \
components, made for its brand. Import one by its import path, such as import confetti from \
\"workspace/confetti\": the default export takes an element and options and answers render(t), \
like the motion presets, and draws in the video's theme. Use one when it fits the moment, with \
the options it lists; otherwise use a preset or write your own code.";

/// What the agent reads when it makes or changes a component.
pub(crate) const COMPONENT_DESIGN_GUIDE: &str = "You are making a canvas component: a reusable piece \
of motion this workspace will import into many animations, such as a confetti burst, a light \
sweep, a signature reveal or a background effect. Write it in component.source as an ES module \
whose default export is function (element, options = {}) returning render(t): it builds its \
elements or canvas inside element (usually document.body), sized from window.arrumo.width and \
height, and draws the moment t seconds after it starts from t alone. Take colours from \
window.arrumo.theme (colors, background, text, accent) unless options give others, set text in \
brand-heading or brand-body, and keep sizes proportional. Give it a few clear options with \
sensible defaults, such as start, speed, colors or text, and describe each in component.options. \
It may import canvas renderer libraries listed in component.libraries, such as motion or shaders. \
component.slug is its import path segment, short lowercase words joined by hyphens. The \
animation itself demonstrates the component: one canvas layer imports it as \
\"workspace/<slug>\" and plays it in a way that shows what it does, with any titles in text \
layers. The usage says when to use it and which options matter. When a component to change is \
included, keep its slug and keep existing options working, since animations already use them.";

impl WorkspaceStore {
    /// Components the agent may use for a profile's videos (or the workspace's, without one),
    /// newest first.
    pub(crate) fn offered_components(
        &self,
        workspace_id: i64,
        profile_id: Option<i64>,
    ) -> rusqlite::Result<Vec<OfferedComponent>> {
        self.read_components(
            "SELECT title, usage_markdown, component_json FROM motion_library AS item
             WHERE workspace_id = ?1 AND component_json IS NOT NULL
               AND (profile_id IS NULL OR profile_id = ?2
                    OR EXISTS (SELECT 1 FROM motion_profile_shares AS share
                               WHERE share.workspace_id = ?1 AND share.item_id = item.id
                                 AND share.profile_id = ?2))
             ORDER BY id DESC LIMIT ?3",
            params![workspace_id, profile_id, MAX_OFFERED as i64],
        )
    }

    /// Every component of the workspace, for drawing pages that import them.
    pub(crate) fn workspace_components(
        &self,
        workspace_id: i64,
    ) -> rusqlite::Result<Vec<CanvasComponent>> {
        Ok(self
            .read_components(
                "SELECT title, usage_markdown, component_json FROM motion_library
                 WHERE workspace_id = ?1 AND component_json IS NOT NULL ORDER BY id DESC LIMIT ?2",
                params![workspace_id, 200],
            )?
            .into_iter()
            .map(|offered| offered.component)
            .collect())
    }

    fn read_components(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> rusqlite::Result<Vec<OfferedComponent>> {
        let rows = self
            .0
            .prepare(sql)?
            .query_map(params, |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(title, usage, json)| {
                let component: CanvasComponent = serde_json::from_str(&json).ok()?;
                component.valid().then_some(OfferedComponent {
                    title,
                    usage,
                    component,
                })
            })
            .collect())
    }

    /// A slug no other component of the workspace uses, starting from `wanted`.
    pub(crate) fn free_component_slug(
        &self,
        workspace_id: i64,
        wanted: &str,
        except_item: Option<i64>,
    ) -> rusqlite::Result<String> {
        let taken: Vec<String> = self
            .0
            .prepare(
                "SELECT json_extract(component_json, '$.slug') FROM motion_library
                 WHERE workspace_id = ?1 AND component_json IS NOT NULL AND id IS NOT ?2",
            )?
            .query_map(params![workspace_id, except_item], |row| {
                row.get::<_, Option<String>>(0)
            })?
            .filter_map(|slug| slug.ok().flatten())
            .collect();
        let base = if valid_slug(wanted) {
            wanted.to_owned()
        } else {
            "component".to_owned()
        };
        let mut slug = base.clone();
        let mut number = 2;
        while taken.contains(&slug) {
            slug = format!("{}-{number}", &base[..base.len().min(36)]);
            number += 1;
        }
        Ok(slug)
    }
}

/// The components in a brief: what each is for, how to import it and its options.
pub(crate) fn component_brief(components: &[OfferedComponent]) -> Value {
    json!(
        components
            .iter()
            .map(|offered| json!({
                "import": offered.component.import_path(),
                "name": offered.title,
                "usage": offered.usage,
                "options": offered.component.options,
            }))
            .collect::<Vec<_>>()
    )
}

/// The animation guide for a video run, with the components its snapshot recorded.
pub(crate) fn run_spec_guide(snapshot: &Value) -> String {
    let guide = crate::motion_spec::spec_guide();
    let components = snapshot_components(snapshot);
    if components.is_empty() || crate::canvas_render::active().is_none() {
        return guide;
    }
    format!(
        "{guide} {COMPONENTS_GUIDE}\nCOMPONENTS (JSON):\n{}\n",
        component_brief(&components)
    )
}

/// The components a run's snapshot recorded.
pub(crate) fn snapshot_components(snapshot: &Value) -> Vec<OfferedComponent> {
    serde_json::from_value::<Vec<OfferedComponent>>(snapshot["canvas_components"].clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|offered| offered.component.valid())
        .collect()
}

/// The answer property in which the agent writes a component.
pub(crate) fn component_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["slug", "source", "options", "libraries"],
        "properties": {
            "slug": {"type": "string", "pattern": "^[a-z][a-z0-9-]{1,39}$"},
            "source": {"type": "string", "maxLength": MAX_COMPONENT_SOURCE},
            "options": {
                "type": "array",
                "maxItems": MAX_OPTIONS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["name", "description"],
                    "properties": {
                        "name": {"type": "string", "maxLength": 40},
                        "description": {"type": "string", "maxLength": 300}
                    }
                }
            },
            "libraries": {"type": "array", "maxItems": 8, "items": {"type": "string", "maxLength": 40}}
        }
    })
}

/// Points the answer's canvas pages at `to` where they import `from`.
pub(crate) fn rename_imports(answer: &mut Value, from: &str, to: &str) {
    if from == to {
        return;
    }
    let Some(layers) = answer["layers"].as_array_mut() else {
        return;
    };
    for layer in layers {
        if let Some(html) = layer["canvas"]["html"].as_str() {
            let renamed = html
                .replace(
                    &format!("\"workspace/{from}\""),
                    &format!("\"workspace/{to}\""),
                )
                .replace(&format!("'workspace/{from}'"), &format!("'workspace/{to}'"));
            layer["canvas"]["html"] = json!(renamed);
        }
    }
}

/// Whether a canvas page in the answer imports `slug`.
pub(crate) fn imports_component(answer: &Value, slug: &str) -> bool {
    answer["layers"].as_array().is_some_and(|layers| {
        layers.iter().any(|layer| {
            layer["canvas"]["html"].as_str().is_some_and(|html| {
                html.contains(&format!("\"workspace/{slug}\""))
                    || html.contains(&format!("'workspace/{slug}'"))
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(slug: &str) -> CanvasComponent {
        CanvasComponent {
            slug: slug.into(),
            source: "export default () => () => {};".into(),
            options: vec![ComponentOption {
                name: "start".into(),
                description: "When it begins, in seconds.".into(),
            }],
            libraries: vec![],
        }
    }

    #[test]
    fn slugs_are_import_path_segments() {
        for good in ["confetti", "light-sweep", "reveal-2"] {
            assert!(valid_slug(good), "{good}");
        }
        for bad in [
            "", "a", "Confetti", "2fast", "a--b", "trail-", "../x", "a/b", "a b",
        ] {
            assert!(!valid_slug(bad), "{bad}");
        }
        assert!(component("confetti").valid());
        let mut large = component("confetti");
        large.source = "x".repeat(MAX_COMPONENT_SOURCE + 1);
        assert!(!large.valid());
    }

    #[test]
    fn imports_follow_a_renamed_slug() {
        let mut answer = json!({"layers": [
            {"canvas": {"html": "<script type=\"module\">import c from \"workspace/confetti\";</script>"}},
            {"text": "plain"}
        ]});
        assert!(imports_component(&answer, "confetti"));
        rename_imports(&mut answer, "confetti", "confetti-2");
        assert!(imports_component(&answer, "confetti-2"));
        assert!(!imports_component(&answer, "confetti"));
    }

    #[test]
    fn components_are_offered_by_profile_and_slugs_stay_unique() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let profile = store
            .0
            .query_row(
                "INSERT INTO profiles (workspace_id, name) VALUES (?1, 'Shorts') RETURNING id",
                [workspace],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        let preview = store
            .0
            .query_row(
                "INSERT INTO workspace_assets (workspace_id, name, kind, byte_size, notes)
                 VALUES (?1, 'a.mp4', 'video', 1, '') RETURNING id",
                [workspace],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        for (title, owner, slug) in [
            ("Confetti", None, "confetti"),
            ("Shorts sweep", Some(profile), "sweep"),
        ] {
            store
                .0
                .execute(
                    "INSERT INTO motion_library
                     (workspace_id, profile_id, kind, title, usage_markdown, preview_asset_id, component_json)
                     VALUES (?1, ?2, 'animation', ?3, 'Use it.', ?4, ?5)",
                    params![
                        workspace,
                        owner,
                        title,
                        preview,
                        serde_json::to_string(&component(slug)).unwrap()
                    ],
                )
                .unwrap();
        }
        let titles = |profile_id| -> Vec<String> {
            store
                .offered_components(workspace, profile_id)
                .unwrap()
                .into_iter()
                .map(|offered| offered.title)
                .collect()
        };
        assert_eq!(titles(None), ["Confetti"]);
        assert_eq!(titles(Some(profile)), ["Shorts sweep", "Confetti"]);
        assert_eq!(store.workspace_components(workspace).unwrap().len(), 2);
        assert_eq!(
            store
                .free_component_slug(workspace, "confetti", None)
                .unwrap(),
            "confetti-2"
        );
        assert_eq!(
            store
                .free_component_slug(workspace, "Not a slug", None)
                .unwrap(),
            "component"
        );
        let brief = component_brief(&store.offered_components(workspace, None).unwrap());
        assert_eq!(brief[0]["import"], "workspace/confetti");
        assert_eq!(brief[0]["options"][0]["name"], "start");
    }
}
