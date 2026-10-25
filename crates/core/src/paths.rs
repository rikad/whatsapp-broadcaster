use std::io;
use std::path::PathBuf;

const APP_DIR: &str = "WhatsApp Broadcaster";

/// `%APPDATA%\WhatsApp Broadcaster` on Windows (`~/Library/Application Support/…`
/// on macOS during development). Override with `WB_DATA_DIR`.
pub fn data_dir() -> io::Result<PathBuf> {
    let dir = match std::env::var_os("WB_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => directories::BaseDirs::new()
            .ok_or_else(|| io::Error::other("could not locate the user data folder"))?
            .data_dir()
            .join(APP_DIR),
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn logs_dir() -> io::Result<PathBuf> {
    let dir = data_dir()?.join("logs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Holds `instance.lock` for the life of the process. `Ok(None)` means another
/// copy of the app already has it.
pub fn lock_instance(dir: &std::path::Path) -> io::Result<Option<std::fs::File>> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("instance.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn second_lock_is_refused() {
        let dir = std::env::temp_dir().join(format!("wb-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = super::lock_instance(&dir).unwrap();
        assert!(first.is_some());
        assert!(super::lock_instance(&dir).unwrap().is_none());
        drop(first);
        assert!(super::lock_instance(&dir).unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
