use crate::IoStep;
use std::{
    fs::{self, File, TryLockError},
    io,
    path::{Path, PathBuf},
};

pub(crate) struct MotionScratch {
    // Elsewhere the lease is a file inside the directory, and Windows can't remove a directory
    // while a file in it is open, so the lease is released first.
    #[cfg(not(target_os = "linux"))]
    _lease: File,
    // On Linux the lease is a handle on the directory itself, held until the directory is gone.
    directory: tempfile::TempDir,
    #[cfg(target_os = "linux")]
    _lease: File,
}

impl MotionScratch {
    pub(crate) fn create(database: &Path) -> io::Result<Self> {
        let root = scratch_root(database)?;
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::DirBuilderExt;
            match fs::DirBuilder::new().mode(0o700).create(&root) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        #[cfg(not(target_os = "linux"))]
        fs::create_dir_all(&root).io_step("Creating a folder")?;
        let root_lock = open_directory(&root)?;
        root_lock.lock()?;
        let directory = tempfile::Builder::new()
            .prefix("render-")
            .rand_bytes(16)
            .tempdir_in(&root)?;
        let lease = open_directory(directory.path())?;
        lease.lock()?;
        Ok(Self {
            directory,
            _lease: lease,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.directory.path()
    }
}

fn scratch_root(database: &Path) -> io::Result<PathBuf> {
    let filename = database.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let mut scratch_name = filename.to_os_string();
    scratch_name.push(".motion-scratch");
    Ok(database.with_file_name(scratch_name))
}

/// A handle whose lock marks the directory as in use. Linux locks the directory itself; other
/// systems can't open a directory as a file, so they lock a `.lease` file inside it.
fn open_directory(path: &Path) -> io::Result<File> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;

        File::options()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        if !path.is_dir() {
            return Err(io::ErrorKind::NotFound.into());
        }
        File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.join(".lease"))
    }
}

pub fn recover_motion_scratch(database: &Path) -> io::Result<()> {
    let root = scratch_root(database)?;
    let root_lock = match open_directory(&root) {
        Ok(lock) => lock,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    root_lock.lock()?;
    for entry in fs::read_dir(&root).io_step("Listing a folder")? {
        let entry = entry?;
        let name = entry.file_name();
        let owned_name = name
            .to_str()
            .and_then(|name| name.strip_prefix("render-"))
            .is_some_and(|suffix| {
                suffix.len() == 16 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
            });
        if !owned_name || !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let lease = match open_directory(&path) {
            Ok(lease) => lease,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        match lease.try_lock() {
            // Nobody holds it, and new scratch waits for the root lock, so it can go. The lease
            // closes first because Windows can't remove a directory with an open file in it.
            Ok(()) => {
                drop(lease);
                match fs::remove_dir_all(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_is_removed_when_done_and_recovered_when_abandoned() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("studio.sqlite");
        let done = MotionScratch::create(&database).unwrap();
        let done_path = done.path().to_path_buf();
        fs::write(done_path.join("animation-0.mp4"), b"video").unwrap();
        drop(done);
        assert!(!done_path.exists());

        let active = MotionScratch::create(&database).unwrap();
        let orphan = MotionScratch::create(&database).unwrap().directory.keep();
        let unrelated = scratch_root(&database).unwrap().join("render-keep");
        fs::create_dir(&unrelated).unwrap();
        for _ in 0..2 {
            recover_motion_scratch(&database).unwrap();
            assert!(!orphan.exists());
            assert!(active.path().is_dir());
            assert!(unrelated.is_dir());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn removes_interrupted_scratch_and_preserves_active_and_unowned_entries() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("studio.sqlite");
        let active = MotionScratch::create(&database).unwrap();
        let orphan = MotionScratch::create(&database).unwrap().directory.keep();
        let unrelated = scratch_root(&database).unwrap().join("render-keep");
        fs::create_dir(&unrelated).unwrap();
        let link = scratch_root(&database)
            .unwrap()
            .join("render-0123456789abcdef");
        std::os::unix::fs::symlink(&unrelated, &link).unwrap();
        for _ in 0..2 {
            recover_motion_scratch(&database).unwrap();
            assert!(!orphan.exists());
            assert!(active.path().is_dir());
            assert!(unrelated.is_dir());
            assert!(link.is_symlink());
        }
    }
}
