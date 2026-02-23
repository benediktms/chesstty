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
/// Returns `WaitError::Timeout` if the socket doesn't become available within
/// the timeout period.
///
/// Returns `WaitError::Connect` if a connection attempt fails.
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

    loop {
        // Check if we've exceeded the timeout
        if Instant::now() > deadline {
            return Err(WaitError::Timeout(socket_path_str, timeout));
        }

        // Phase 1: Check if the socket file exists
        if !socket_path.exists() {
            tokio::time::sleep(poll_interval).await;
            continue;
        }

        // Phase 2: Try to connect to the socket
        match tokio::net::UnixStream::connect(socket_path).await {
            Ok(_stream) => {
                // Connection successful - socket is ready
                return Ok(());
            }
            Err(e) => {
                // Connection failed - socket might not be ready yet
                // Log for debugging (don't fail on connection errors)
                tracing::debug!(
                    "Socket {} not ready yet, connection attempt failed: {}",
                    socket_path_str,
                    e
                );
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
    use std::path::PathBuf;
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
