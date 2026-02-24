//! Socket wait utilities for ChessTTY shim.
//!
//! Provides waiting for Unix domain sockets to become available.

use std::path::Path;
use std::time::{Duration, Instant};

/// Error type for socket wait operations.
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("timeout waiting for socket {0} after {1:?}")]
    Timeout(String, Duration),

    #[error("failed to connect to socket {0}: {1}")]
    Connect(String, #[source] std::io::Error),
}

/// Wait for a Unix domain socket to become available.
///
/// This function polls for socket availability in a loop:
/// 1. Check if the socket file exists
/// 2. Try to connect to the socket
/// 3. If either step fails, sleep for the poll interval and retry
///
/// # Arguments
///
/// * `socket_path` - Path to the Unix domain socket
/// * `timeout` - Maximum time to wait (default: 5 seconds)
/// * `poll_interval` - Time between connection attempts (default: 100ms)
///
/// # Errors
///
/// Returns `WaitError::Connect` if the socket file exists but connections
/// consistently fail until the timeout expires.
///
/// Returns `WaitError::Timeout` if the socket doesn't become available within
/// the timeout period and no connection was attempted.
///
/// # Example
///
/// ```ignore
/// use chesstty::wait::wait_for_socket;
///
/// wait_for_socket(
///     "/tmp/chesstty.sock",
///     Duration::from_secs(5),
///     Duration::from_millis(100),
/// ).await?;
/// ```
pub async fn wait_for_socket(
    socket_path: impl AsRef<Path>,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), WaitError> {
    let socket_path = socket_path.as_ref();
    let socket_path_str = socket_path.display().to_string();
    let deadline = Instant::now() + timeout;
    let mut last_connect_err: Option<std::io::Error> = None;

    loop {
        if Instant::now() > deadline {
            // If socket existed but connections failed, return Connect
            if socket_path.exists() {
                if let Some(err) = last_connect_err {
                    return Err(WaitError::Connect(socket_path_str, err));
                }
            }
            return Err(WaitError::Timeout(socket_path_str, timeout));
        }

        if !socket_path.exists() {
            last_connect_err = None;
            tokio::time::sleep(poll_interval).await;
            continue;
        }

        match tokio::net::UnixStream::connect(socket_path).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                tracing::debug!(
                    "Socket {} not ready yet, connection attempt failed: {}",
                    socket_path_str,
                    e
                );
                last_connect_err = Some(e);
                tokio::time::sleep(poll_interval).await;
            }
        }
    }
}

/// Wait for a socket with default settings (5s timeout, 100ms poll interval).
///
/// Convenience function that uses sensible defaults.
///
/// # Errors
///
/// Returns `WaitError::Timeout` if the socket doesn't become available within
/// 5 seconds.
#[allow(dead_code)]
pub async fn wait_for_socket_default(socket_path: impl AsRef<Path>) -> Result<(), WaitError> {
    wait_for_socket(
        socket_path,
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_wait_for_socket_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("nonexistent.sock");

        let result = wait_for_socket(
            &socket_path,
            Duration::from_millis(100),
            Duration::from_millis(10),
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WaitError::Timeout(_, _)));
    }

    #[tokio::test]
    async fn test_wait_for_socket_default_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("nonexistent.sock");

        // Using default timeout of 5s would take too long in tests
        // So we just verify the function works with a very short timeout
        let result = wait_for_socket(&socket_path, Duration::ZERO, Duration::ZERO).await;

        assert!(result.is_err());
    }
}
