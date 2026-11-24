//! The plugin tour: one shared introduction to the plugins the creator hasn't met yet, from the
//! `introduction` each plugin declares. The studio shows it, so a new install sees its bundled
//! plugins once its first workspace exists, and later updates introduce the plugins they add.

use std::error::Error;

use rusqlite::params;
use serde::Serialize;

use crate::{WorkspaceStore, plugin_catalog::LocalizedText};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TourStop {
    pub plugin_id: String,
    pub name: String,
    pub headline: LocalizedText,
    pub body: LocalizedText,
}

impl WorkspaceStore {
    fn tour_stops(&self) -> Result<Vec<TourStop>, Box<dyn Error>> {
        Ok(self
            .plugin_catalog()?
            .into_iter()
            .filter_map(|plugin| {
                let introduction = plugin.manifest.introduction?;
                Some(TourStop {
                    plugin_id: plugin.manifest.id,
                    name: plugin.manifest.name,
                    headline: introduction.headline,
                    body: introduction.body,
                })
            })
            .collect())
    }

    /// The plugins to introduce now, in catalogue order.
    pub fn pending_plugin_tour(&self) -> Result<Vec<TourStop>, Box<dyn Error>> {
        let mut seen = self
            .0
            .prepare("SELECT 1 FROM plugin_introductions_seen WHERE plugin_id = ?1")?;
        let mut pending = Vec::new();
        for stop in self.tour_stops()? {
            if !seen.exists([&stop.plugin_id])? {
                pending.push(stop);
            }
        }
        Ok(pending)
    }

    /// The introductions of the named plugins, to replay the tour.
    pub fn plugin_tour(&self, ids: &[String]) -> Result<Vec<TourStop>, Box<dyn Error>> {
        Ok(self
            .tour_stops()?
            .into_iter()
            .filter(|stop| ids.is_empty() || ids.contains(&stop.plugin_id))
            .collect())
    }

    /// Records that the creator saw (or skipped) these plugins' introductions.
    pub fn mark_plugins_introduced(&self, ids: &[String]) -> rusqlite::Result<()> {
        for id in ids.iter().filter(|id| (1..=128).contains(&id.len())) {
            self.0.execute(
                "INSERT OR IGNORE INTO plugin_introductions_seen (plugin_id) VALUES (?1)",
                params![id],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(core_only, ignore = "needs the bundled default plugins")]
    fn each_plugin_is_introduced_once() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        // A new install introduces its bundled plugins once.
        let all = store.pending_plugin_tour().unwrap();
        assert!(all.iter().any(|stop| stop.plugin_id == "codex"));
        let ids: Vec<String> = all.iter().map(|stop| stop.plugin_id.clone()).collect();
        store.mark_plugins_introduced(&ids).unwrap();
        assert!(store.pending_plugin_tour().unwrap().is_empty());
        // A plugin that was never seen (as after an update adding it) is introduced once.
        store
            .0
            .execute(
                "DELETE FROM plugin_introductions_seen WHERE plugin_id = 'codex'",
                [],
            )
            .unwrap();
        let pending = store.pending_plugin_tour().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].plugin_id, "codex");
        assert!(pending[0].headline.contains_key("en"));
        store.mark_plugins_introduced(&["codex".into()]).unwrap();
        assert!(store.pending_plugin_tour().unwrap().is_empty());
        assert_eq!(store.plugin_tour(&["codex".into()]).unwrap().len(), 1);
    }
}
