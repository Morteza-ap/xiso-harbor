//! One live extraction session per app-data directory, and bounded, durable records.
use crate::engine::Partial;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{fs::OpenOptionsExt, io::AsRawFd},
    path::Path,
};

pub fn lock(data: &Path) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(data.join("instance.lock"))?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(file)
}

pub fn save(path: &Path, partial: &Partial) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(partial)?)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        if let Some(parent) = path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn load(path: &Path) -> std::io::Result<Option<Partial>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other(
            "Recovery record is not a regular file.",
        ));
    }
    let mut data = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut data)?;
    if data.len() > 16 * 1024 {
        return Err(std::io::Error::other("Recovery record exceeds 16 KiB."));
    }
    Ok(Some(serde_json::from_slice(&data)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_one_instance_can_own_recovery_state() {
        let dir = tempfile::tempdir().unwrap();
        let first = lock(dir.path()).unwrap();
        assert!(lock(dir.path()).is_err());
        drop(first);
        assert!(lock(dir.path()).is_ok());
    }
    #[test]
    fn atomic_record_roundtrip_and_oversized_record_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pending.json");
        let p = Partial {
            path: dir.path().join("Xiso Harbor fixture.partial"),
            dev: 1,
            ino: 2,
        };
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap().unwrap().path, p.path);
        fs::write(&path, vec![b' '; 17000]).unwrap();
        assert!(load(&path).is_err());
    }
    #[test]
    fn refuses_symlink_records_and_lock_files() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        fs::write(&outside, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("instance.lock")).unwrap();
        assert!(lock(dir.path()).is_err());
        let path = dir.path().join("pending.json");
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"unchanged");
    }
}
