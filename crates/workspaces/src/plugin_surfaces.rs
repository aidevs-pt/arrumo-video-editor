use std::io::{self, Cursor, Read};

use serde::Serialize;

use crate::{
    MAX_PLUGIN_IO_BYTES, PluginManifest, PluginSurfacePlacement, WorkspaceStore,
    plugin_schema::{PluginPermission, PluginSurface},
};

const MAX_DOCUMENT_BYTES: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPluginSurface {
    pub plugin_id: String,
    pub plugin_name: String,
    pub package_sha256: String,
    pub id: String,
    pub title: String,
    pub requests_workspace_context: bool,
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
                        requests_workspace_context: surface
                            .permissions
                            .contains(&PluginPermission::WorkspaceContextRead),
                        title: surface.title,
                    }),
            );
        }
        Ok(surfaces)
    }

    pub fn plugin_surface_context(
        &self,
        plugin_id: &str,
        surface_id: &str,
        package_sha256: &str,
        workspace_id: i64,
    ) -> io::Result<Option<String>> {
        let tx = self
            .0
            .unchecked_transaction()
            .map_err(|_| io::Error::other("Could not read plugin surface permissions"))?;
        let package = self.resolve_plugin_package(plugin_id, Some(package_sha256))?;
        let surface = find_surface(package.manifest, surface_id)?;
        if !surface
            .permissions
            .contains(&PluginPermission::WorkspaceContextRead)
            || !self.plugin_context_granted(plugin_id, workspace_id)?
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Plugin surface context permission required",
            ));
        }
        let context = self
            .workspace_context(workspace_id)
            .map_err(|_| io::Error::other("Could not read granted workspace context"))?;
        if context
            .as_ref()
            .is_some_and(|text| text.len() > MAX_PLUGIN_IO_BYTES)
        {
            return Err(io::Error::other(
                "Workspace context exceeds the transfer limit",
            ));
        }
        tx.commit()
            .map_err(|_| io::Error::other("Could not read plugin surface permissions"))?;
        Ok(context)
    }

    pub fn plugin_surface_document(
        &self,
        plugin_id: &str,
        surface_id: &str,
        package_sha256: &str,
    ) -> io::Result<String> {
        let package = self.resolve_plugin_package(plugin_id, Some(package_sha256))?;
        let surface = find_surface(package.manifest, surface_id)?;
        let mut archive = zip::ZipArchive::new(Cursor::new(package.bytes))
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

fn find_surface(manifest: PluginManifest, surface_id: &str) -> io::Result<PluginSurface> {
    manifest
        .code
        .into_iter()
        .flat_map(|code| code.frontend)
        .find(|surface| surface.id == surface_id)
        .ok_or_else(|| io::Error::other("Plugin surface unavailable"))
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
    #[test]
    fn context_requires_a_grant_for_the_declared_panel_and_package() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Shared").unwrap().id;
        let other = store.create("Private").unwrap().id;
        store
            .set_workspace_context(workspace, "Shared text")
            .unwrap();
        store.set_workspace_context(other, "Private text").unwrap();
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        archive.start_file("manifest.json", opts).unwrap();
        archive.write_all(br#"{
          "schemaVersion":5,"id":"fixture-reader","name":"Fixture","description":"Fixture",
          "capabilities":{"panel":{"name":"Panel","description":"Fixture"}},
          "code":{"frontend":[
            {"id":"reader","title":"Reader","document":"index.html","placement":"workspace","permissions":["workspace-context-read"]},
            {"id":"settings","title":"Settings","document":"index.html","placement":"settings"}
          ]}
        }"#).unwrap();
        archive.start_file("index.html", opts).unwrap();
        archive.write_all(b"<p>Fixture</p>").unwrap();
        let package = archive.finish().unwrap().into_inner();
        let installed = store.install_plugin_zip(package.as_slice()).unwrap();
        let digest = installed.package.unwrap().sha256;
        let read = |panel, digest, scope| {
            store.plugin_surface_context("fixture-reader", panel, digest, scope)
        };
        assert!(read("reader", &digest, workspace).is_err());
        store
            .set_plugin_context_grant("fixture-reader", workspace, true)
            .unwrap();
        assert_eq!(
            read("reader", &digest, workspace).unwrap().as_deref(),
            Some("Shared text")
        );
        assert!(read("settings", &digest, workspace).is_err());
        assert!(read("reader", "stale", workspace).is_err());
        assert!(read("reader", &digest, other).is_err());
        store
            .set_plugin_context_grant("fixture-reader", workspace, false)
            .unwrap();
        assert!(read("reader", &digest, workspace).is_err());
    }
}
