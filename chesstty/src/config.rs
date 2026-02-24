//! Configuration for ChessTTY runtime.
//!
//! Centralises all runtime tunables for the shim: socket path, PID file path,
//! server log path, and timeout/poll-interval values for the socket-readiness
//! wait. Every value has a compile-time default and can be overridden at runtime
//! via a dedicated environment variable.

use std::path::PathBuf;

/// Default socket path for server communication.
const DEFAULT_SOCKET_PATH: &str = "/tmp/chesstty.sock";

/// Default PID file path for server process tracking.
const DEFAULT_PID_PATH: &str = "/tmp/chesstty.pid";

/// Default timeout for waiting on socket availability (in seconds).
const DEFAULT_SOCKET_TIMEOUT_SECS: u64 = 5;

/// Default poll interval for socket availability checks (in milliseconds).
const DEFAULT_SOCKET_POLL_INTERVAL_MS: u64 = 100;

/// Default server log path. `/dev/null` discards all server output.
const DEFAULT_SERVER_LOG_PATH: &str = "/dev/null";

/// Get the socket path for server communication.
///
/// Priority:
/// 1. `CHESSTTY_SOCKET_PATH` env variable if set
/// 2. `/tmp/chesstty.sock` as fallback
pub fn get_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(DEFAULT_SOCKET_PATH)
}

/// Get the PID file path for server process tracking.
///
/// Priority:
/// 1. `CHESSTTY_PID_PATH` env variable if set
/// 2. `/tmp/chesstty.pid` as fallback
pub fn get_pid_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_PID_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(DEFAULT_PID_PATH)
}

/// Get the socket wait timeout in seconds.
///
/// Priority:
/// 1. `CHESSTTY_SOCKET_TIMEOUT_SECS` env variable if set (falls back to default
///    if the value cannot be parsed as a `u64`)
/// 2. `5` seconds as fallback
pub fn get_socket_timeout_secs() -> u64 {
    if let Ok(timeout) = std::env::var("CHESSTTY_SOCKET_TIMEOUT_SECS") {
        return timeout.parse().unwrap_or(DEFAULT_SOCKET_TIMEOUT_SECS);
    }

    DEFAULT_SOCKET_TIMEOUT_SECS
}

/// Get the socket poll interval in milliseconds.
///
/// Returns the fixed default of 100 ms. This value is not currently overridable
/// via an environment variable.
pub fn get_socket_poll_interval_ms() -> u64 {
    DEFAULT_SOCKET_POLL_INTERVAL_MS
}

/// Get the file path where server stdout and stderr should be written.
///
/// Priority:
/// 1. `CHESSTTY_SERVER_LOG_PATH` env variable if set
/// 2. `/dev/null` as fallback (server output is discarded by default)
///
/// Set this variable to a writable file path to capture server logs for
/// debugging, for example `CHESSTTY_SERVER_LOG_PATH=/tmp/chesstty-server.log`.
pub fn get_server_log_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHESSTTY_SERVER_LOG_PATH") {
        return PathBuf::from(path);
    }

    PathBuf::from(DEFAULT_SERVER_LOG_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_socket_path() {
        let path = get_socket_path();
        match std::env::var("CHESSTTY_SOCKET_PATH") {
            Ok(val) => assert_eq!(path, PathBuf::from(val)),
            Err(_) => assert_eq!(path, PathBuf::from(DEFAULT_SOCKET_PATH)),
        }
    }

    #[test]
    fn test_get_pid_path() {
        let path = get_pid_path();
        match std::env::var("CHESSTTY_PID_PATH") {
            Ok(val) => assert_eq!(path, PathBuf::from(val)),
            Err(_) => assert_eq!(path, PathBuf::from(DEFAULT_PID_PATH)),
        }
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

    #[test]
    fn test_get_server_log_path_default() {
        let path = get_server_log_path();
        assert_eq!(path, PathBuf::from(DEFAULT_SERVER_LOG_PATH));
    }
}
