//! Process management utilities for ChessTTY shim.
//!
//! Provides PID file operations for detecting and managing the server process.

use std::fs;
use std::os::fd::AsRawFd;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("failed to read PID file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("invalid PID file content: expected integer, got '{0}'")]
    InvalidContent(String),

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
    use std::path::PathBuf;
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

// =============================================================================
// Daemon builder - UNIX double-fork daemonization
// =============================================================================

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug)]
pub struct IoError(std::io::Error);

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<std::io::Error> for IoError {
    fn from(err: std::io::Error) -> Self {
        Self(err)
    }
}

/// Represents a group for privilege dropping.
#[derive(Debug, Clone)]
pub enum Group {
    Name(String),
    Id(u32),
}

impl From<&str> for Group {
    fn from(s: &str) -> Self {
        Group::Name(s.to_string())
    }
}

impl From<String> for Group {
    fn from(s: String) -> Self {
        Group::Name(s)
    }
}

impl From<u32> for Group {
    fn from(gid: u32) -> Self {
        Group::Id(gid)
    }
}

/// Error type for daemon operations.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("first fork failed: {0}")]
    FirstForkFailed(IoError),

    #[error("second fork failed: {0}")]
    SecondForkFailed(IoError),

    #[error("setsid failed: {0}")]
    SetsidFailed(IoError),

    #[error("failed to write PID file: {0}")]
    PidFileWrite(IoError),

    #[error("failed to chown PID file: {0}")]
    PidFileChown(IoError),

    #[error("failed to drop privileges: {0}")]
    PrivilegeDrop(String),

    #[error("privileged action failed: {0}")]
    PrivilegedAction(#[from] anyhow::Error),

    #[error("failed to get user '{0}': {0}")]
    UserNotFound(String),

    #[error("failed to get group '{0}': {0}")]
    GroupNotFound(String),

    #[error("failed to open /dev/null: {0}")]
    DevNullOpen(IoError),

    #[error("failed to redirect stdin: {0}")]
    StdinRedirect(IoError),

    #[error("failed to redirect stdout: {0}")]
    StdoutRedirect(IoError),

    #[error("failed to redirect stderr: {0}")]
    StderrRedirect(IoError),

    #[error("failed to change directory: {0}")]
    ChdirFailed(IoError),

    #[error("failed to set umask: {0}")]
    UmaskFailed(IoError),
}

/// Builder for creating a UNIX daemon process.
///
/// Implements the double-fork daemonization pattern.
///
/// # Example
///
/// ```ignore
/// let stdout = File::create("/tmp/chesstty-server.out")?;
/// let stderr = File::create("/tmp/chesstty-server.err")?;
///
/// Daemon::new()
///     .pid_file("/tmp/chesstty.pid")
///     .working_directory("/tmp")
///     .umask(0o027)
///     .stdout(stdout)
///     .stderr(stderr)
///     .start()?;
///
/// // We are now the daemon
/// ```
pub struct Daemon {
    pid_file: Option<PathBuf>,
    chown_pid_file: bool,
    working_directory: PathBuf,
    user: Option<String>,
    group: Option<Group>,
    umask: u32,
    stdout: Option<File>,
    stderr: Option<File>,
    privileged_action: Option<Box<dyn FnOnce() -> anyhow::Result<()> + 'static>>,
}

impl std::fmt::Debug for Daemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Daemon { ... }")
    }
}

impl Daemon {
    /// Create a new Daemon builder with default settings.
    ///
    /// Defaults:
    /// - working_directory: "/"
    /// - umask: 0o027
    /// - stdout/stderr: not configured (will redirect to /dev/null)
    #[must_use]
    pub fn new() -> Self {
        Self {
            pid_file: None,
            chown_pid_file: false,
            working_directory: PathBuf::from("/"),
            user: None,
            group: None,
            umask: 0o027,
            stdout: None,
            stderr: None,
            privileged_action: None,
        }
    }

    /// Set the PID file path.
    ///
    /// The PID of the daemon (grandchild process) will be written to this file.
    #[must_use]
    pub fn pid_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.pid_file = Some(path.into());
        self
    }

    /// Whether to chown the PID file to the configured user/group.
    ///
    /// Requires `user` or `group` to be set.
    #[must_use]
    pub fn chown_pid_file(mut self, chown: bool) -> Self {
        self.chown_pid_file = chown;
        self
    }

    /// Set the working directory for the daemon.
    ///
    /// Defaults to "/".
    #[must_use]
    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = path.into();
        self
    }

    /// Set the user to drop privileges to.
    ///
    /// Can be a username string or UID (via `Into<Group>`).
    #[must_use]
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the group to drop privileges to.
    ///
    /// Can be a group name string or GID.
    #[must_use]
    pub fn group(mut self, group: impl Into<Group>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set the umask for file creation.
    ///
    /// Defaults to 0o027 (read/execute for group, none for others).
    #[must_use]
    pub fn umask(mut self, mask: u32) -> Self {
        self.umask = mask;
        self
    }

    /// Set the stdout file for the daemon.
    ///
    /// If not set, stdout will be redirected to /dev/null.
    #[must_use]
    pub fn stdout(mut self, f: File) -> Self {
        self.stdout = Some(f);
        self
    }

    /// Set the stderr file for the daemon.
    ///
    /// If not set, stderr will be redirected to /dev/null.
    #[must_use]
    pub fn stderr(mut self, f: File) -> Self {
        self.stderr = Some(f);
        self
    }

    /// Set a privileged action to run between setsid and privilege drop.
    ///
    /// This runs in the intermediate process after setsid but before
    /// dropping privileges. Useful for operations that need elevated
    /// permissions (e.g., binding to privileged ports).
    #[must_use]
    pub fn privileged_action<F>(mut self, f: F) -> Self
    where
        F: FnOnce() -> anyhow::Result<()> + 'static,
    {
        self.privileged_action = Some(Box::new(f));
        self
    }

    /// Execute the daemonization sequence.
    ///
    /// On success, returns `Ok(())` in the daemon (grandchild) process.
    /// The original caller process exits via `std::process::exit(0)`.
    ///
    /// # Errors
    ///
    /// Returns an error if any step of the daemonization fails.
    pub fn start(&mut self) -> Result<(), DaemonError> {
        // First fork
        // SAFETY: This is the standard UNIX double-fork pattern.
        // We document that the parent exits and child continues.
        match unsafe { libc::fork() } {
            -1 => {
                return Err(DaemonError::FirstForkFailed(
                    std::io::Error::last_os_error().into(),
                ));
            }
            0 => {
                // Child continues
            }
            _ => {
                // Parent exits
                std::process::exit(0);
            }
        }

        // Create new session - become session leader
        // SAFETY: setsid() is safe to call in a child process.
        if unsafe { libc::setsid() } == -1 {
            return Err(DaemonError::SetsidFailed(
                std::io::Error::last_os_error().into(),
            ));
        }

        // Run privileged action if configured
        if let Some(action) = std::mem::take(&mut self.privileged_action) {
            action().map_err(DaemonError::PrivilegedAction)?;
        }

        // Drop privileges if configured
        self.drop_privileges()?;

        // Second fork to prevent acquiring a controlling terminal
        // SAFETY: This is the second fork in the double-fork pattern.
        match unsafe { libc::fork() } {
            -1 => {
                return Err(DaemonError::SecondForkFailed(
                    std::io::Error::last_os_error().into(),
                ));
            }
            0 => {
                // Grandchild continues as daemon
            }
            _ => {
                // Session leader exits
                std::process::exit(0);
            }
        }

        // Change working directory
        std::env::set_current_dir(&self.working_directory)
            .map_err(|err| DaemonError::ChdirFailed(err.into()))?;

        // Set umask
        // SAFETY: umask is a simple bitmask operation, safe to call.
        let old_umask = unsafe { libc::umask(self.umask as libc::mode_t) };
        // Restore old umask by setting it back (we just wanted to apply it)
        // Actually, we want to KEEP the new umask, so don't restore
        let _ = old_umask; // Acknowledge the old value

        // Redirect stdin/stdout/stderr
        self.redirect_io()?;

        // Write PID file
        if let Some(ref pid_path) = self.pid_file {
            self.write_pid_file(pid_path)?;
        }

        Ok(())
    }

    fn drop_privileges(&self) -> Result<(), DaemonError> {
        // Drop group first (if user also set, we need primary group)
        if let Some(ref group) = self.group {
            let gid = self.resolve_group(group)?;
            // SAFETY: setgid is safe when dropping privileges.
            if unsafe { libc::setgid(gid) } != 0 {
                return Err(DaemonError::PrivilegeDrop(format!(
                    "failed to setgid to {}: {}",
                    gid,
                    std::io::Error::last_os_error()
                )));
            }
        }

        // Then drop user
        if let Some(ref user) = self.user {
            let uid = self.resolve_user(user)?;
            // SAFETY: setuid is safe when dropping privileges.
            if unsafe { libc::setuid(uid) } != 0 {
                return Err(DaemonError::PrivilegeDrop(format!(
                    "failed to setuid to {}: {}",
                    uid,
                    std::io::Error::last_os_error()
                )));
            }
        }

        Ok(())
    }

    fn resolve_user(&self, user: &str) -> Result<libc::uid_t, DaemonError> {
        // Try to parse as UID first
        if let Ok(uid) = user.parse::<libc::uid_t>() {
            return Ok(uid);
        }

        // Look up by name using getpwnam
        // SAFETY: getpwnam is thread-safe when using the reentrant version.
        let c_str = std::ffi::CString::new(user)
            .map_err(|_| DaemonError::UserNotFound(user.to_string()))?;

        // SAFETY: getpwnam_r is the thread-safe version.
        let mut passwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut buf: Vec<u8> = vec![0; 4096];
        let mut result: *mut libc::passwd = std::ptr::null_mut();

        let ret = unsafe {
            libc::getpwnam_r(
                c_str.as_ptr(),
                &mut passwd,
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                &mut result,
            )
        };

        if ret != 0 {
            return Err(DaemonError::UserNotFound(user.to_string()));
        }

        if result.is_null() {
            return Err(DaemonError::UserNotFound(user.to_string()));
        }

        // SAFETY: result points to passwd which we initialized.
        Ok(unsafe { (*result).pw_uid })
    }

    fn resolve_group(&self, group: &Group) -> Result<libc::gid_t, DaemonError> {
        match group {
            Group::Id(gid) => Ok(*gid),
            Group::Name(name) => {
                // Try to parse as GID first
                if let Ok(gid) = name.parse::<libc::gid_t>() {
                    return Ok(gid);
                }

                // Look up by name using getgrnam
                // SAFETY: getgrnam is thread-safe when using the reentrant version.
                let c_str = std::ffi::CString::new(name.as_str())
                    .map_err(|_| DaemonError::GroupNotFound(name.clone()))?;

                let mut grp: libc::group = unsafe { std::mem::zeroed() };
                let mut buf: Vec<u8> = vec![0; 4096];
                let mut result: *mut libc::group = std::ptr::null_mut();

                let ret = unsafe {
                    libc::getgrnam_r(
                        c_str.as_ptr(),
                        &mut grp,
                        buf.as_mut_ptr() as *mut libc::c_char,
                        buf.len(),
                        &mut result,
                    )
                };

                if ret != 0 {
                    return Err(DaemonError::GroupNotFound(name.clone()));
                }

                if result.is_null() {
                    return Err(DaemonError::GroupNotFound(name.clone()));
                }

                // SAFETY: result points to grp which we initialized.
                Ok(unsafe { (*result).gr_gid })
            }
        }
    }

    fn redirect_io(&self) -> Result<(), DaemonError> {
        // Open /dev/null
        let devnull =
            File::open("/dev/null").map_err(|err| DaemonError::DevNullOpen(err.into()))?;

        // Redirect stdin to /dev/null
        // SAFETY: dup2 is safe for redirecting file descriptors.
        if unsafe { libc::dup2(devnull.as_raw_fd(), libc::STDIN_FILENO) } == -1 {
            return Err(DaemonError::StdinRedirect(
                std::io::Error::last_os_error().into(),
            ));
        }

        // Redirect stdout
        if let Some(ref stdout) = self.stdout {
            // SAFETY: dup2 is safe for redirecting file descriptors.
            if unsafe { libc::dup2(stdout.as_raw_fd(), libc::STDOUT_FILENO) } == -1 {
                return Err(DaemonError::StdoutRedirect(
                    std::io::Error::last_os_error().into(),
                ));
            }
        } else {
            // Redirect to /dev/null
            // SAFETY: dup2 is safe for redirecting file descriptors.
            if unsafe { libc::dup2(devnull.as_raw_fd(), libc::STDOUT_FILENO) } == -1 {
                return Err(DaemonError::StdoutRedirect(
                    std::io::Error::last_os_error().into(),
                ));
            }
        }

        // Redirect stderr
        if let Some(ref stderr) = self.stderr {
            // SAFETY: dup2 is safe for redirecting file descriptors.
            if unsafe { libc::dup2(stderr.as_raw_fd(), libc::STDERR_FILENO) } == -1 {
                return Err(DaemonError::StderrRedirect(
                    std::io::Error::last_os_error().into(),
                ));
            }
        } else {
            // Redirect to /dev/null
            // SAFETY: dup2 is safe for redirecting file descriptors.
            if unsafe { libc::dup2(devnull.as_raw_fd(), libc::STDERR_FILENO) } == -1 {
                return Err(DaemonError::StderrRedirect(
                    std::io::Error::last_os_error().into(),
                ));
            }
        }

        Ok(())
    }

    fn write_pid_file(&self, pid_path: &PathBuf) -> Result<(), DaemonError> {
        let pid = std::process::id();

        // Create parent directory if it doesn't exist
        if let Some(parent) = pid_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| DaemonError::PidFileWrite(err.into()))?;
            }
        }

        // Write PID file
        let mut file =
            File::create(pid_path).map_err(|err| DaemonError::PidFileWrite(err.into()))?;
        writeln!(file, "{}", pid).map_err(|err| DaemonError::PidFileWrite(err.into()))?;
        file.sync_all()
            .map_err(|err| DaemonError::PidFileWrite(err.into()))?;

        // Chown if requested
        if self.chown_pid_file {
            let mut uid: Option<libc::uid_t> = None;
            let mut gid: Option<libc::gid_t> = None;

            if let Some(ref user) = self.user {
                uid = Some(self.resolve_user(user)?);
            }
            if let Some(ref group) = self.group {
                gid = Some(self.resolve_group(group)?);
            }

            if let (Some(uid), Some(gid)) = (uid, gid) {
                // SAFETY: chown is safe here as we're just changing ownership.
                if unsafe {
                    libc::chown(
                        pid_path.as_os_str().as_encoded_bytes().as_ptr() as *const libc::c_char,
                        uid,
                        gid,
                    )
                } != 0
                {
                    return Err(DaemonError::PidFileChown(
                        std::io::Error::last_os_error().into(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}
