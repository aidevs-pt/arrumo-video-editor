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
        let mut bytes = Vec::new();
        archive
            .take(MAX_ARCHIVE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(invalid_package("Plugin package exceeds 16 MiB").into());
        }

        let manifest = read_package_manifest(&bytes)?;
        let mut sha256 = String::with_capacity(64);
        for byte in Sha256::digest(&bytes) {
            write!(sha256, "{byte:02x}")?;
        }
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
