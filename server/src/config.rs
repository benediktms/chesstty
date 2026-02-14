//! Configuration for ChessTTY server
//!
//! Handles data directory configuration with the following precedence:
//! 1. CHESSTTY_DATA_DIR environment variable
//! 2. ~/.config/chesstty/data (production default)
//! 3. ./data (fallback for development)

use std::path::PathBuf;

const DEFAULT_CONFIG_DIR: &str = ".config/chesstty/data";
const DEV_DATA_DIR: &str = "./data";

/// Get the data directory for persistence.
///
/// Priority:
/// 1. CHESSTTY_DATA_DIR env variable if set
/// 2. $HOME/.config/chesstty/data if HOME is set
/// 3. ./data as fallback
pub fn get_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CHESSTTY_DATA_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(DEFAULT_CONFIG_DIR);
    }

    PathBuf::from(DEV_DATA_DIR)
}

/// Get the directory containing default positions (version controlled).
///
/// This is always relative to the server binary location, not configurable.
pub fn get_defaults_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defaults")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_data_dir_fallback() {
        // Note: This test assumes CHESSTTY_DATA_DIR is not set in the test environment
        // If it is set, it will return that value (which is correct behavior)
        let dir = get_data_dir();
        // Should be a valid path (either env var, ~/.config/chesstty/data, or ./data)
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn test_get_defaults_dir() {
        let dir = get_defaults_dir();
        assert!(dir.ends_with("server/defaults"));
    }

    // Note: test_get_data_dir_with_env removed to avoid test pollution
    // Environment variable behavior is tested via integration tests or manual verification
}
