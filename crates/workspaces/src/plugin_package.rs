use std::{
    collections::HashSet,
    error::Error,
    fmt::Write as _,
    io::{self, Cursor, Read},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::{CatalogPlugin, MAX_PLUGIN_MANIFEST_BYTES, WorkspaceStore};

const MAX_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 512;

pub const THIRD_PARTY_PLUGIN_NOTICE: &str = "Third-party plugins can be unsafe, including plugins listed in a registry. Review the publisher and source before trusting the plugin.";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPackageInfo {
    pub sha256: String,
    pub byte_length: i64,
}

impl WorkspaceStore {
    pub fn install_plugin_zip(&self, archive: impl Read) -> Result<CatalogPlugin, Box<dyn Error>> {
        self.register_plugin_zip(archive, None)
    }

    pub fn install_verified_plugin_zip(
        &self,
        archive: impl Read,
        expected_sha256: &str,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        if expected_sha256.len() != 64
            || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(
                invalid_package("Expected SHA-256 must contain 64 hexadecimal digits").into(),
            );
        }
        self.register_plugin_zip(archive, Some(expected_sha256))
    }

    fn register_plugin_zip(
        &self,
        archive: impl Read,
        expected_sha256: Option<&str>,
    ) -> Result<CatalogPlugin, Box<dyn Error>> {
        let mut bytes = Vec::new();
        archive
            .take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(invalid_package("Plugin package exceeds 16 MiB").into());
        }

        let mut sha256 = String::with_capacity(64);
        for byte in Sha256::digest(&bytes) {
            write!(sha256, "{byte:02x}")?;
        }
        if expected_sha256.is_some_and(|expected| !sha256.eq_ignore_ascii_case(expected)) {
            return Err(invalid_package(
                "Plugin package SHA-256 does not match the expected digest",
            )
            .into());
        }
        let manifest = read_package_manifest(&bytes)?;
        let info = PluginPackageInfo {
            sha256,
            byte_length: bytes.len() as i64,
        };
        self.register_plugin(&manifest, Some((&info, &bytes)))
    }
}

fn read_package_manifest(bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(invalid_package("Plugin package has more than 512 entries").into());
    }

    archive.validate_header_consistency()?;

    let mut entries = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index)?;
        let path = std::str::from_utf8(entry.name_raw())?;
        let path = path.trim_end_matches('/');
        if path.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.contains(['\\', ':'])
                || part.chars().any(char::is_control)
        }) || entry.is_symlink()
            || entry.encrypted()
            || !entries.insert(path.to_lowercase())
        {
            return Err(
                invalid_package("Plugin package contains an unsafe or duplicate entry").into(),
            );
        }
    }

    let entry = archive.by_name("manifest.json")?;
    if !entry.is_file() || entry.size() > MAX_PLUGIN_MANIFEST_BYTES as u64 {
        return Err(invalid_package(
            "Plugin package requires a root manifest.json of at most 64 KiB",
        )
        .into());
    }
    let mut manifest = Vec::new();
    entry
        .take(MAX_PLUGIN_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut manifest)?;
    Ok(manifest)
}

fn invalid_package(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    #[test]
    fn verifies_expected_digest_before_registering_package() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(
                br#"{
            "schemaVersion": 1,
            "id": "local-plugin",
            "name": "Local Plugin",
            "description": "Fixture",
            "capabilities": {"custom": {"name": "Custom", "description": "Fixture"}}
        }"#,
            )
            .unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let mut checksum = String::new();
        for byte in Sha256::digest(&bytes) {
            write!(checksum, "{byte:02X}").unwrap();
        }
        let initial_count = store.plugin_catalog().unwrap().len();
        for digest in ["invalid", &"0".repeat(64)] {
            assert!(
                store
                    .install_verified_plugin_zip(bytes.as_slice(), digest)
                    .is_err()
            );
        }
        assert_eq!(store.plugin_catalog().unwrap().len(), initial_count);
        let plugin = store
            .install_verified_plugin_zip(bytes.as_slice(), &checksum)
            .unwrap();
        assert_eq!(plugin.manifest.id, "local-plugin");
        assert_eq!(
            plugin.package.unwrap().sha256,
            checksum.to_ascii_lowercase()
        );
    }

    #[test]
    fn rejects_conflicting_and_unsafe_archive_paths() {
        for path in ["../manifest.json", "/manifest.json", "MANIFEST.JSON"] {
            let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
            for name in ["manifest.json", path] {
                archive
                    .start_file(name, SimpleFileOptions::default())
                    .unwrap();
                archive.write_all(b"{}").unwrap();
            }
            let bytes = archive.finish().unwrap().into_inner();
            assert!(read_package_manifest(&bytes).is_err(), "accepted {path}");
        }
    }
}
