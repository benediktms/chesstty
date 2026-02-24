//! PID file operations for detecting and managing the server process.

use std::fs;
use std::path::Path;

/// Error type for PID file and process-existence operations.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// The PID file could not be read from disk.
    #[error("failed to read PID file: {0}")]
    ReadError(#[from] std::io::Error),

    /// The PID file existed but did not contain a valid integer PID.
    #[error("invalid PID file content: expected integer, got '{0}'")]
    InvalidContent(String),

    /// No process with the recorded PID exists on this system.
    #[error("process {0} does not exist")]
    ProcessNotFound(i32),
}

/// Read the PID from a PID file.
///
/// # Errors
/// Returns an error if the file cannot be read or contains invalid content.
pub fn read_pid(pid_path: &Path) -> Result<i32, ProcessError> {
    let content = fs::read_to_string(pid_path)?;
    let pid = content
        .trim()
        .parse::<i32>()
        .map_err(|_| ProcessError::InvalidContent(content))?;
    Ok(pid)
}

/// Check if a process with the given PID is currently running.
///
/// Uses `kill(pid, 0)` to check process existence without actually sending a signal.
/// This will detect if the process exists AND we have permission to signal it.
///
/// # Safety
/// The kill system call with signal 0 only checks existence, not actual signal delivery.
pub fn is_server_running(pid_path: &Path) -> bool {
    // Try to read the PID file first
    let pid = match read_pid(pid_path) {
        Ok(pid) => pid,
        Err(_) => return false,
    };

    // SAFETY: kill(pid, 0) only checks if the process exists and we have permission.
    // It does not send any signal or modify process state.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Remove a stale PID file if the process is no longer running.
///
/// A stale PID file contains a PID that no longer corresponds to a running process.
/// This can happen if the server was killed ungracefully or crashed.
///
/// # Errors
/// Returns an error if the file cannot be removed.
pub fn remove_stale_pid(pid_path: &Path) -> anyhow::Result<()> {
    if !pid_path.exists() {
        return Ok(());
    }

    if !is_server_running(pid_path) {
        fs::remove_file(pid_path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_pid_valid() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("pid");
        fs::write(&pid_path, "12345\n").unwrap();

        let pid = read_pid(&pid_path).unwrap();
        assert_eq!(pid, 12345);
    }

    #[test]
    fn test_read_pid_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("pid");
        fs::write(&pid_path, "not_a_pid\n").unwrap();

        let result = read_pid(&pid_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_pid_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("nonexistent");

        let result = read_pid(&pid_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_server_running_current() {
        // Use current process PID
        let pid = std::process::id() as i32;
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("pid");
        fs::write(&pid_path, format!("{}\n", pid)).unwrap();

        assert!(is_server_running(&pid_path));
    }

    #[test]
    fn test_is_server_running_invalid() {
        // Use a PID that definitely doesn't exist
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("pid");
        fs::write(&pid_path, "999999\n").unwrap();

        // PID 999999 is extremely unlikely to be running
        assert!(!is_server_running(&pid_path));
    }

    #[test]
    fn test_remove_stale_pid() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("pid");

        // Write a stale PID (999999 definitely not running)
        fs::write(&pid_path, "999999\n").unwrap();
        assert!(pid_path.exists());

        remove_stale_pid(&pid_path).unwrap();
        // Stale PID file should be removed
        assert!(!pid_path.exists());
    }

    #[test]
    fn test_remove_stale_pid_running() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("pid");

        // Write current process PID
        let pid = std::process::id() as i32;
        fs::write(&pid_path, format!("{}\n", pid)).unwrap();
        assert!(pid_path.exists());

        remove_stale_pid(&pid_path).unwrap();
        // Running process PID file should NOT be removed
        assert!(pid_path.exists());
    }

    #[test]
    fn test_remove_stale_pid_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join("nonexistent");

        // Should not error on nonexistent file
        remove_stale_pid(&pid_path).unwrap();
    }
}
