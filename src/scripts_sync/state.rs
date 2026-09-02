//! Version state file for `scripts-sync` (default `./dst_version.txt`).
//!
//! The state file records the game version whose `scripts` tree was last
//! synced, mirroring the legacy Python tool's `dst_version.txt`. A missing
//! file simply means "never synced".

use crate::error::Result;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Default state file name, resolved against the current working directory.
pub const DEFAULT_STATE_FILE: &str = "dst_version.txt";

/// Default state path: `dst_version.txt` in the current working directory.
pub fn default_state_path() -> PathBuf {
    PathBuf::from(DEFAULT_STATE_FILE)
}

/// Reads the last synced version from the state file.
/// A missing file means "never synced" (`None`).
pub fn read_last_version(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.trim().to_string())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Persists the synced version to the state file.
pub fn write_version(path: &Path, version: &str) -> Result<()> {
    Ok(fs::write(path, format!("{version}\n"))?)
}

/// Whether a sync is needed: no record yet, or a version mismatch.
pub fn needs_update(last: Option<&str>, new: &str) -> bool {
    last != Some(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_last_version_missing_file() {
        let path = std::env::temp_dir().join("dst_huiji_scripts_sync_missing_version.txt");
        let _ = fs::remove_file(&path);
        assert_eq!(read_last_version(&path).unwrap(), None);
    }

    #[test]
    fn test_write_then_read_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "dst_huiji_scripts_sync_state_{}.txt",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        write_version(&path, "747465").unwrap();
        assert_eq!(read_last_version(&path).unwrap().as_deref(), Some("747465"));
        // Trailing whitespace is trimmed on read.
        fs::write(&path, "  747466 \n").unwrap();
        assert_eq!(read_last_version(&path).unwrap().as_deref(), Some("747466"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_needs_update_truth_table() {
        assert!(needs_update(None, "747465"));
        assert!(needs_update(Some("747464"), "747465"));
        assert!(!needs_update(Some("747465"), "747465"));
        // Empty record counts as a (bogus) version, still triggers sync.
        assert!(needs_update(Some(""), "747465"));
    }
}
