//! Full backups: one ZIP with a database snapshot (settings, workspaces, installed plugins), the
//! managed media and the verified deliveries, plus a manifest of SHA-256 digests checked before
//! anything is restored. Restoring replaces everything, so it is staged beside the database and
//! applied at the next start, before anything opens the database; the replaced data is kept.

use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::WorkspaceStore;

const FORMAT: &str = "arrumo-backup";
const VERSION: u32 = 1;
const MANIFEST: &str = "arrumo-backup.json";
const DATABASE: &str = "database/workspaces.sqlite";
/// Data folders beside the database, by their name in the archive.
const FOLDERS: [(&str, &str); 2] = [("media", ".media"), ("deliveries", ".deliveries")];
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 500_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupManifest {
    pub format: String,
    pub version: u32,
    pub app_version: String,
    pub created_at: u64,
    pub workspaces: Vec<String>,
    /// Installed plugin IDs; absent in backups made before it was recorded.
    #[serde(default)]
    pub plugins: Vec<String>,
    pub files: Vec<BackupFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub app_version: String,
    pub created_at: u64,
    pub workspaces: Vec<String>,
    pub plugins: usize,
    pub files: usize,
    pub bytes: u64,
}

impl From<&BackupManifest> for BackupSummary {
    fn from(manifest: &BackupManifest) -> Self {
        Self {
            app_version: manifest.app_version.clone(),
            created_at: manifest.created_at,
            workspaces: manifest.workspaces.clone(),
            plugins: manifest.plugins.len(),
            files: manifest.files.len(),
            bytes: manifest.files.iter().map(|file| file.size).sum(),
        }
    }
}

fn sibling(database: &Path, suffix: &str) -> PathBuf {
    let mut path = database.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

fn hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Copies `source` into the archive entry, returning its size and digest.
fn add_file(archive: &mut ZipWriter<File>, name: &str, source: &Path) -> io::Result<BackupFile> {
    let mut file = File::open(source)?;
    let size = file.metadata()?.len();
    // Media is already compressed; storing it keeps exports fast.
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(size >= u32::MAX.into());
    archive.start_file(name, options)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    let mut written = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        archive.write_all(&buffer[..count])?;
        written += count as u64;
    }
    if written != size {
        return Err(io::Error::other("A file changed while it was exported"));
    }
    Ok(BackupFile {
        path: name.to_owned(),
        size,
        sha256: hex(digest.finalize()),
    })
}

/// Regular files under `root`, with their archive names under `prefix`. Links are skipped.
fn files_under(root: &Path, prefix: &str) -> io::Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), prefix.to_owned())];
    while let Some((folder, name)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let Some(part) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push((entry.path(), format!("{name}/{part}")));
            } else if kind.is_file() {
                found.push((format!("{name}/{part}"), entry.path()));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// An archive path that stays inside the folder it is extracted to.
fn safe_entry(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    let allowed = name == DATABASE
        || FOLDERS
            .iter()
            .any(|(folder, _)| name.starts_with(&format!("{folder}/")));
    (allowed
        && !name.contains('\\')
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_))))
    .then(|| path.to_path_buf())
}

impl WorkspaceStore {
    /// Writes a full backup to `destination`, which must not exist yet.
    pub fn export_backup(
        &self,
        database: &Path,
        destination: &Path,
    ) -> Result<BackupSummary, Box<dyn Error>> {
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
        let scratch = tempfile::tempdir_in(parent)?;
        let snapshot = scratch.path().join("workspaces.sqlite");
        self.snapshot_database(&snapshot)?;
        // Chosen agent programs are paths on this computer; another one has its own.
        rusqlite::Connection::open(&snapshot)?.execute("DELETE FROM agent_executables", [])?;
        let workspaces = self
            .list()?
            .into_iter()
            .map(|workspace| workspace.name)
            .collect();
        let plugins = self
            .0
            .prepare("SELECT plugin_id FROM installed_plugin_manifests ORDER BY plugin_id")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        let pending = NamedTempFile::new_in(parent)?;
        let mut archive = ZipWriter::new(pending.reopen()?);
        let mut files = vec![add_file(&mut archive, DATABASE, &snapshot)?];
        for (name, suffix) in FOLDERS {
            for (entry, path) in files_under(&sibling(database, suffix), name)? {
                if files.len() >= MAX_FILES {
                    return Err(io::Error::other("Too many files to back up").into());
                }
                files.push(add_file(&mut archive, &entry, &path)?);
            }
        }
        let manifest = BackupManifest {
            format: FORMAT.to_owned(),
            version: VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            workspaces,
            plugins,
            files,
        };
        archive.start_file(MANIFEST, SimpleFileOptions::default())?;
        archive.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
        archive.finish()?.sync_all()?;
        pending.persist_noclobber(destination)?;
        Ok(BackupSummary::from(&manifest))
    }
}

/// Reads the manifest and checks every listed file's size and digest, and that the archive holds
/// nothing else. Returns the manifest when the backup is intact.
pub fn verify_backup(archive: &Path) -> Result<BackupManifest, Box<dyn Error>> {
    check_backup(archive, None)
}

fn check_backup(
    archive: &Path,
    extract_to: Option<&Path>,
) -> Result<BackupManifest, Box<dyn Error>> {
    let invalid = |reason: &str| io::Error::new(io::ErrorKind::InvalidData, reason.to_owned());
    let mut zip = ZipArchive::new(File::open(archive)?)
        .map_err(|_| invalid("This is not an Arrumo backup"))?;
    let manifest: BackupManifest = {
        let entry = zip
            .by_name(MANIFEST)
            .map_err(|_| invalid("This is not an Arrumo backup"))?;
        let mut bytes = Vec::new();
        entry.take(MAX_MANIFEST_BYTES).read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)
            .map_err(|_| invalid("The backup's manifest is unreadable"))?
    };
    if manifest.format != FORMAT || manifest.version != VERSION {
        return Err(invalid("This backup was made by an unsupported version of Arrumo").into());
    }
    if manifest.files.len() > MAX_FILES
        || !manifest.files.iter().any(|file| file.path == DATABASE)
        || zip.len() != manifest.files.len() + 1
    {
        return Err(invalid("The backup's contents do not match its manifest").into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for listed in &manifest.files {
        let relative = safe_entry(&listed.path)
            .filter(|_| seen.insert(listed.path.clone()))
            .ok_or_else(|| invalid("The backup lists a file outside its folders"))?;
        let mut entry = zip
            .by_name(&listed.path)
            .map_err(|_| invalid("A file listed in the backup is missing"))?;
        let mut output = match extract_to {
            Some(root) => {
                let path = root.join(&relative);
                fs::create_dir_all(path.parent().unwrap_or(root))?;
                Some(File::create_new(path)?)
            }
            None => None,
        };
        let mut digest = Sha256::new();
        let mut buffer = vec![0; 256 * 1024];
        let mut size = 0u64;
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > listed.size {
                break;
            }
            digest.update(&buffer[..count]);
            if let Some(file) = &mut output {
                file.write_all(&buffer[..count])?;
            }
        }
        if size != listed.size || hex(digest.finalize()) != listed.sha256 {
            return Err(invalid("The backup was changed or damaged after it was made").into());
        }
        if let Some(file) = output {
            file.sync_all()?;
        }
    }
    Ok(manifest)
}

fn pending_folder(database: &Path) -> PathBuf {
    sibling(database, ".pending-import")
}

/// Verifies the backup and stages it beside the database, to replace everything at the next
/// start. The staged database is opened once so a backup from a newer Arrumo is refused now.
pub fn stage_backup_import(
    database: &Path,
    archive: &Path,
) -> Result<BackupSummary, Box<dyn Error>> {
    let pending = pending_folder(database);
    if pending.exists() {
        fs::remove_dir_all(&pending)?;
    }
    let parent = database.parent().unwrap_or_else(|| Path::new("."));
    let staging = tempfile::tempdir_in(parent)?;
    let manifest = check_backup(archive, Some(staging.path()))?;
    WorkspaceStore::open(staging.path().join(DATABASE)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "This backup needs a newer version of Arrumo",
        )
    })?;
    fs::rename(staging.keep(), &pending)?;
    Ok(BackupSummary::from(&manifest))
}

pub fn pending_backup_import(database: &Path) -> bool {
    pending_folder(database).join(DATABASE).is_file()
}

pub fn cancel_backup_import(database: &Path) -> io::Result<()> {
    match fs::remove_dir_all(pending_folder(database)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Applies a staged import before the database is opened. The replaced database and folders are
/// moved to a dated folder beside them; if any step fails, they are moved back.
pub fn apply_pending_import(database: &Path) -> io::Result<Option<PathBuf>> {
    let pending = pending_folder(database);
    if !pending.join(DATABASE).is_file() {
        return Ok(None);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .unwrap_or_default();
    let kept = sibling(database, &format!(".before-import-{stamp}"));
    fs::create_dir(&kept)?;
    // What exists now, where it goes while kept, and what replaces it.
    let mut moves = vec![(
        database.to_path_buf(),
        kept.join("workspaces.sqlite"),
        pending.join(DATABASE),
    )];
    for suffix in ["-wal", "-shm"] {
        moves.push((
            sibling(database, suffix),
            kept.join(format!("workspaces.sqlite{suffix}")),
            PathBuf::new(),
        ));
    }
    for (name, suffix) in FOLDERS {
        moves.push((
            sibling(database, suffix),
            kept.join(name),
            pending.join(name),
        ));
    }
    let mut kept_moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut placed: Vec<(PathBuf, PathBuf)> = Vec::new();
    let result = (|| -> io::Result<()> {
        for (current, keep, _) in &moves {
            if current.symlink_metadata().is_ok() {
                fs::rename(current, keep)?;
                kept_moves.push((keep.clone(), current.clone()));
            }
        }
        for (current, _, replacement) in &moves {
            if !replacement.as_os_str().is_empty() && replacement.exists() {
                fs::rename(replacement, current)?;
                placed.push((current.clone(), replacement.clone()));
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        // Put the staged files back, then the original data, so a later start can try again.
        for (current, replacement) in placed.into_iter().rev() {
            let _ = fs::rename(current, replacement);
        }
        for (keep, current) in kept_moves.into_iter().rev() {
            let _ = fs::rename(keep, current);
        }
        let _ = fs::remove_dir(&kept);
        return Err(error);
    }
    let _ = fs::remove_dir_all(&pending);
    Ok(Some(kept))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio(folder: &Path, name: &str) -> PathBuf {
        let database = folder.join("workspaces.sqlite");
        let store = WorkspaceStore::open(&database).unwrap();
        store.create(name).unwrap();
        let media = sibling(&database, ".media").join("1");
        fs::create_dir_all(&media).unwrap();
        fs::write(media.join("7"), format!("{name} picture")).unwrap();
        database
    }

    #[test]
    fn a_backup_restores_everything_and_keeps_what_it_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&target).unwrap();
        let database = studio(&source, "Studio");
        let archive = temp.path().join("backup.zip");
        let summary = WorkspaceStore::open(&database)
            .unwrap()
            .export_backup(&database, &archive)
            .unwrap();
        assert_eq!(
            (summary.files, summary.workspaces.clone()),
            (2, vec!["Studio".to_owned()])
        );
        assert!(
            WorkspaceStore::open(&database)
                .unwrap()
                .export_backup(&database, &archive)
                .is_err()
        );
        verify_backup(&archive).unwrap();

        let current = studio(&target, "Other");
        stage_backup_import(&current, &archive).unwrap();
        assert!(pending_backup_import(&current));
        // Nothing changes until the next start applies it.
        assert_eq!(
            WorkspaceStore::open(&current).unwrap().list().unwrap()[0].name,
            "Other"
        );
        let kept = apply_pending_import(&current).unwrap().unwrap();
        assert_eq!(
            WorkspaceStore::open(&current).unwrap().list().unwrap()[0].name,
            "Studio"
        );
        assert_eq!(
            fs::read_to_string(sibling(&current, ".media").join("1/7")).unwrap(),
            "Studio picture"
        );
        assert_eq!(
            fs::read_to_string(kept.join("media/1/7")).unwrap(),
            "Other picture"
        );
        assert!(!pending_backup_import(&current));
        assert_eq!(apply_pending_import(&current).unwrap(), None);
    }

    #[test]
    fn changed_or_foreign_archives_are_refused() {
        let temp = tempfile::tempdir().unwrap();
        let database = studio(temp.path(), "Studio");
        let archive = temp.path().join("backup.zip");
        WorkspaceStore::open(&database)
            .unwrap()
            .export_backup(&database, &archive)
            .unwrap();

        // Rewrite the archive with one media file changed but the manifest left as it was.
        let mut original = ZipArchive::new(File::open(&archive).unwrap()).unwrap();
        let tampered = temp.path().join("tampered.zip");
        let mut writer = ZipWriter::new(File::create(&tampered).unwrap());
        for index in 0..original.len() {
            let mut entry = original.by_index(index).unwrap();
            let name = entry.name().unwrap().into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if name == "media/1/7" {
                bytes = b"Studio picturE".to_vec();
            }
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
        assert!(
            verify_backup(&tampered)
                .unwrap_err()
                .to_string()
                .contains("changed or damaged")
        );
        assert!(stage_backup_import(&database, &tampered).is_err());
        assert!(!pending_backup_import(&database));

        let foreign = temp.path().join("foreign.zip");
        let mut writer = ZipWriter::new(File::create(&foreign).unwrap());
        writer
            .start_file("notes.txt", SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        assert!(verify_backup(&foreign).is_err());
        assert_eq!(safe_entry("media/../../etc/passwd"), None);
        assert_eq!(safe_entry("other/file"), None);
    }
}
