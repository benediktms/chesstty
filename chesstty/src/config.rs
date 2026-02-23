//! Configuration for ChessTTY runtime
//!
//! Handles socket path, PID file path, and timeout configuration for IPC between
//! the shim CLI and the server.

use std::path::PathBuf;

/// Default socket path for server communication.
#[allow(dead_code)]
const DEFAULT_SOCKET_PATH: &str = "/tmp/chesstty.sock";

/// Default PID file path for server process tracking.
#[allow(dead_code)]
const DEFAULT_PID_PATH: &str = "/tmp/chesstty.pid";

/// Default timeout for waiting on socket availability (in seconds).
#[allow(dead_code)]
const DEFAULT_SOCKET_TIMEOUT_SECS: u64 = 5;

/// Default poll interval for socket availability checks (in milliseconds).
#[allow(dead_code)]
const DEFAULT_SOCKET_POLL_INTERVAL_MS: u64 = 100;

/// Get the socket path for server communication.
///
/// Priority:
/// 1. CHESSTTY_SOCKET_PATH env variable if set
/// 2. /tmp/chesstty.sock as fallback
#[allow(dead_code)]
pub fn get_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(DEFAULT_SOCKET_PATH)
}

/// Get the PID file path for server process tracking.
///
/// Priority:
/// 1. CHESSTTY_PID_PATH env variable if set
/// 2. /tmp/chesstty.pid as fallback
#[allow(dead_code)]
pub fn get_pid_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_PID_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(DEFAULT_PID_PATH)
}

/// Get the socket wait timeout in seconds.
///
/// Priority:
/// 1. CHESSTTY_SOCKET_TIMEOUT_SECS env variable if set
/// 2. 5 seconds as fallback
#[allow(dead_code)]
pub fn get_socket_timeout_secs() -> u64 {
    if let Ok(timeout) = std::env::var("CHESSTTY_SOCKET_TIMEOUT_SECS") {
        return timeout.parse().unwrap_or(DEFAULT_SOCKET_TIMEOUT_SECS);
    }

    DEFAULT_SOCKET_TIMEOUT_SECS
}

/// Get the socket poll interval in milliseconds.
///
/// This is a fixed value and not configurable via environment variable.
#[allow(dead_code)]
pub fn get_socket_poll_interval_ms() -> u64 {
    DEFAULT_SOCKET_POLL_INTERVAL_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_socket_path_fallback() {
        let path = get_socket_path();
        assert_eq!(path, PathBuf::from(DEFAULT_SOCKET_PATH));
    }

    #[test]
    fn test_get_pid_path_fallback() {
        let path = get_pid_path();
        assert_eq!(path, PathBuf::from(DEFAULT_PID_PATH));
    }

    #[test]
    fn test_get_socket_timeout_secs_default() {
        let timeout = get_socket_timeout_secs();
        assert_eq!(timeout, DEFAULT_SOCKET_TIMEOUT_SECS);
    }

    #[test]
    fn test_get_socket_poll_interval_ms() {
        let interval = get_socket_poll_interval_ms();
        assert_eq!(interval, DEFAULT_SOCKET_POLL_INTERVAL_MS);
    }
}
