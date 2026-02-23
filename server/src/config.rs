//! Configuration for ChessTTY server
//!
//! Handles data directory, database path, and socket configuration:
//! - Legacy JSON data directory (for migration): `get_legacy_data_dir()`
//! - SQLite database path: `get_db_path()`
//! - Unix Domain Socket path: `get_socket_path()`

use std::path::PathBuf;

const DEFAULT_CONFIG_DIR: &str = ".config/chesstty/data";
const DEV_DATA_DIR: &str = "./data";

/// Default socket path for server communication.
const DEFAULT_SOCKET_PATH: &str = "/tmp/chesstty.sock";

/// Get the data directory for JSON file migration only.
///
/// Priority:
/// 1. CHESSTTY_DATA_DIR env variable if set
/// 2. $HOME/.config/chesstty/data if HOME is set
/// 3. ./data as fallback
pub fn get_legacy_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CHESSTTY_DATA_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(DEFAULT_CONFIG_DIR);
    }

    PathBuf::from(DEV_DATA_DIR)
}

/// Get the SQLite database file path.
///
/// Priority:
/// 1. CHESSTTY_DB_PATH env variable if set
/// 2. Platform data directory via `directories` crate:
///    - macOS: ~/Library/Application Support/chesstty/chesstty.db
///    - Linux: ~/.local/share/chesstty/chesstty.db
/// 3. ./data/chesstty.db as fallback
pub fn get_db_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_DB_PATH") {
        return PathBuf::from(path);
    }

    if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "chesstty") {
        return proj_dirs.data_dir().join("chesstty.db");
    }

    PathBuf::from("./data/chesstty.db")
}

/// Get the Unix Domain Socket path for server communication.
///
/// Priority:
/// 1. CHESSTTY_SOCKET_PATH env variable if set
/// 2. /tmp/chesstty.sock as fallback
pub fn get_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(DEFAULT_SOCKET_PATH)
}

/// Get the directory containing default positions (version controlled).
///
/// This is always relative to the server binary location, not configurable.
#[allow(dead_code)]
pub fn get_defaults_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defaults")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_legacy_data_dir_fallback() {
        // Note: This test assumes CHESSTTY_DATA_DIR is not set in the test environment
        // If it is set, it will return that value (which is correct behavior)
        let dir = get_legacy_data_dir();
        // Should be a valid path (either env var, ~/.config/chesstty/data, or ./data)
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_get_db_path_fallback() {
        let path = get_db_path();
        assert!(!path.as_os_str().is_empty());
        assert!(path.to_string_lossy().ends_with("chesstty.db"));
    }

    #[test]
    fn test_get_defaults_dir() {
        let dir = get_defaults_dir();
        assert!(dir.ends_with("server/defaults"));
    }

    // Note: test_get_data_dir_with_env removed to avoid test pollution
    // Environment variable behavior is tested via integration tests or manual verification
}
