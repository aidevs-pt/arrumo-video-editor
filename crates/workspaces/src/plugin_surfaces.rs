use std::io::{self, Cursor, Read};

use serde::Serialize;

use crate::{PluginManifest, PluginSurfacePlacement, WorkspaceStore};

const MAX_DOCUMENT_BYTES: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginSurface {
    pub plugin_id: String,
    pub plugin_name: String,
    pub package_sha256: String,
    pub id: String,
    pub title: String,
}

impl WorkspaceStore {
    pub fn plugin_surfaces(
        &self,
        placement: PluginSurfacePlacement,
    ) -> Result<Vec<InstalledPluginSurface>, Box<dyn std::error::Error>> {
        let mut surfaces = Vec::new();
        for plugin in self.plugin_catalog()? {
            let (Some(package), Some(code)) = (plugin.package, plugin.manifest.code) else {
                continue;
            };
            surfaces.extend(
                code.frontend
                    .into_iter()
                    .filter(|surface| surface.placement == placement)
                    .map(|surface| InstalledPluginSurface {
                        plugin_id: plugin.manifest.id.clone(),
                        plugin_name: plugin.manifest.name.clone(),
                        package_sha256: package.sha256.clone(),
                        id: surface.id,
                        title: surface.title,
                    }),
            );
        }
        Ok(surfaces)
    }

    pub fn plugin_surface_document(
        &self,
        plugin_id: &str,
        surface_id: &str,
        package_sha256: &str,
    ) -> io::Result<String> {
        let (manifest, bytes): (Vec<u8>, Vec<u8>) = self
            .0
            .query_row(
                "SELECT manifest, package_bytes FROM installed_plugin_manifests
             WHERE plugin_id = ?1 AND package_sha256 = ?2",
                rusqlite::params![plugin_id, package_sha256],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| io::Error::other("Plugin package unavailable or changed"))?;
        let manifest = PluginManifest::parse(&manifest)
            .map_err(|_| io::Error::other("Plugin manifest invalid"))?;
        let surface = manifest
            .code
            .into_iter()
            .flat_map(|code| code.frontend)
            .find(|surface| surface.id == surface_id)
            .ok_or_else(|| io::Error::other("Plugin surface unavailable"))?;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| io::Error::other("Invalid plugin ZIP"))?;
        let file = archive
            .by_name(&surface.document)
            .map_err(|_| io::Error::other("Plugin document unavailable"))?;
        if !file.is_file() || file.size() > MAX_DOCUMENT_BYTES {
            return Err(io::Error::other("Invalid plugin document size"));
        }
        let mut document = String::new();
        file.take(MAX_DOCUMENT_BYTES + 1)
            .read_to_string(&mut document)
            .map_err(|_| io::Error::other("Plugin document must be UTF-8"))?;
        if document.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(io::Error::other("Plugin document exceeds 512 KiB"));
        }
        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn reads_only_declared_documents_from_the_selected_package_revision() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        archive.start_file("manifest.json", opts).unwrap();
        archive.write_all(br#"{
          "schemaVersion":3,"id":"fixture-panel","name":"Fixture","description":"Fixture",
          "capabilities":{"panel":{"name":"Panel","description":"Fixture"}},
          "code":{"frontend":[{"id":"fixture","title":"Fixture","document":"index.html","placement":"settings"}]}
        }"#).unwrap();
        archive.start_file("index.html", opts).unwrap();
        archive.write_all(b"<p>Fixture</p>").unwrap();
        let package = archive.finish().unwrap().into_inner();
        store.install_plugin_zip(package.as_slice()).unwrap();
        assert!(
            store
                .plugin_surfaces(PluginSurfacePlacement::Workspace)
                .unwrap()
                .is_empty()
        );
        let panels = store
            .plugin_surfaces(PluginSurfacePlacement::Settings)
            .unwrap();
        assert_eq!(panels.len(), 1);
        let panel = &panels[0];
        assert_eq!(
            store
                .plugin_surface_document(&panel.plugin_id, &panel.id, &panel.package_sha256)
                .unwrap(),
            "<p>Fixture</p>"
        );
        assert!(
            store
                .plugin_surface_document(&panel.plugin_id, "manifest.json", &panel.package_sha256)
                .is_err()
        );
        assert!(
            store
                .plugin_surface_document(&panel.plugin_id, &panel.id, "stale")
                .is_err()
        );
        store
            .remove_installed_plugin_manifest(&panel.plugin_id)
            .unwrap();
        assert!(
            store
                .plugin_surface_document(&panel.plugin_id, &panel.id, &panel.package_sha256)
                .is_err()
        );
    }
}
