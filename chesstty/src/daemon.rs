//! UNIX double-fork daemon builder for the ChessTTY server.
//!
//! Provides a builder-pattern API for daemonizing a process using the classic
//! double-fork technique. No external daemonization crate is used — all
//! `fork`/`setsid`/`dup2` calls go through `libc` directly.

use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// Newtype wrapper around [`std::io::Error`] used as a `thiserror` source.
///
/// `thiserror` requires error sources to implement `std::error::Error`, but
/// `std::io::Error` cannot be used directly as a named field inside
/// `#[error(...)]` variants when the variant also needs to be constructed via
/// `IoError::from`. This wrapper satisfies the trait bounds while keeping
/// variant construction ergonomic.
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

/// Identifies a Unix group for privilege-dropping purposes.
///
/// A group can be specified either by name (resolved via `getgrnam_r`) or by
/// numeric GID. Both variants are accepted wherever a `Group` is expected via
/// the `From<&str>`, `From<String>`, and `From<u32>` implementations.
#[derive(Debug, Clone)]
pub enum Group {
    /// Group identified by name; resolved to a GID via `getgrnam_r`.
    Name(String),
    /// Group identified directly by its numeric GID.
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
///
/// Covers every failure that can occur during the double-fork daemonization
/// sequence, privilege dropping, I/O redirection, and PID file management.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    /// The first `fork(2)` call failed.
    #[error("first fork failed: {0}")]
    FirstForkFailed(IoError),

    /// The second `fork(2)` call (used to prevent acquiring a controlling
    /// terminal) failed.
    #[error("second fork failed: {0}")]
    SecondForkFailed(IoError),

    /// The `setsid(2)` call to create a new session failed.
    #[error("setsid failed: {0}")]
    SetsidFailed(IoError),

    /// The PID file could not be created or written.
    #[error("failed to write PID file: {0}")]
    PidFileWrite(IoError),

    /// Changing ownership of the PID file (`chown`) failed.
    #[error("failed to chown PID file: {0}")]
    PidFileChown(IoError),

    /// `setuid(2)` or `setgid(2)` failed when dropping privileges.
    #[error("failed to drop privileges: {0}")]
    PrivilegeDrop(String),

    /// The user-supplied privileged action closure returned an error.
    #[error("privileged action failed: {0}")]
    PrivilegedAction(#[from] anyhow::Error),

    /// The specified username could not be resolved to a UID via `getpwnam_r`.
    #[error("failed to get user '{0}'")]
    UserNotFound(String),

    /// The specified group name could not be resolved to a GID via `getgrnam_r`.
    #[error("failed to get group '{0}'")]
    GroupNotFound(String),

    /// `/dev/null` could not be opened for stdin/stdout/stderr redirection.
    #[error("failed to open /dev/null: {0}")]
    DevNullOpen(IoError),

    /// `dup2(2)` failed when redirecting stdin.
    #[error("failed to redirect stdin: {0}")]
    StdinRedirect(IoError),

    /// `dup2(2)` failed when redirecting stdout.
    #[error("failed to redirect stdout: {0}")]
    StdoutRedirect(IoError),

    /// `dup2(2)` failed when redirecting stderr.
    #[error("failed to redirect stderr: {0}")]
    StderrRedirect(IoError),

    /// `chdir(2)` to the configured working directory failed.
    #[error("failed to change directory: {0}")]
    ChdirFailed(IoError),
}

/// Builder for creating a UNIX daemon process.
///
/// Implements the double-fork daemonization pattern. Consuming `self` in
/// [`Daemon::start`] enforces that the builder is a one-shot operation.
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
    #[allow(dead_code)] // planned API: privilege dropping not yet wired up in main.rs
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
    #[allow(dead_code)] // planned API: privilege dropping not yet wired up in main.rs
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the group to drop privileges to.
    ///
    /// Can be a group name string or GID.
    #[must_use]
    #[allow(dead_code)] // planned API: privilege dropping not yet wired up in main.rs
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
    #[allow(dead_code)] // planned API: privilege dropping not yet wired up in main.rs
    pub fn privileged_action<F>(mut self, f: F) -> Self
    where
        F: FnOnce() -> anyhow::Result<()> + 'static,
    {
        self.privileged_action = Some(Box::new(f));
        self
    }

    /// Execute the daemonization sequence.
    ///
    /// Consumes `self` — this is a one-shot operation. On success, returns
    /// `Ok(())` in the daemon (grandchild) process. The original caller
    /// process exits via `std::process::exit(0)`.
    ///
    /// Sequence:
    /// 1. First fork — parent exits
    /// 2. `setsid()` — detach from controlling terminal
    /// 3. Execute privileged_action (if set)
    /// 4. Drop privileges (user/group, if set)
    /// 5. Second fork — session leader exits
    /// 6. `umask` + `chdir`
    /// 7. Redirect stdin/stdout/stderr
    /// 8. Write PID file (grandchild PID)
    ///
    /// # Errors
    ///
    /// Returns an error if any step of the daemonization fails.
    pub fn start(self) -> Result<(), DaemonError> {
        let Daemon {
            pid_file,
            chown_pid_file,
            working_directory,
            user,
            group,
            umask,
            stdout,
            stderr,
            privileged_action,
        } = self;

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
        if let Some(action) = privileged_action {
            action().map_err(DaemonError::PrivilegedAction)?;
        }

        // Drop privileges if configured
        drop_privileges(&user, &group)?;

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
        std::env::set_current_dir(&working_directory)
            .map_err(|err| DaemonError::ChdirFailed(err.into()))?;

        // Set umask
        // SAFETY: umask is a simple bitmask operation, safe to call.
        unsafe { libc::umask(umask as libc::mode_t) };

        // Redirect stdin/stdout/stderr
        redirect_io(&stdout, &stderr)?;

        // Write PID file
        if let Some(ref pid_path) = pid_file {
            write_pid_file(pid_path, chown_pid_file, &user, &group)?;
        }

        Ok(())
    }
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Free functions — extracted from Daemon impl for use with consumed self
// =============================================================================

/// Drop process privileges to the configured user and group.
///
/// Group is dropped first (while still root) so that supplementary group
/// membership is updated before the UID change locks us out of root operations.
fn drop_privileges(user: &Option<String>, group: &Option<Group>) -> Result<(), DaemonError> {
    if let Some(ref group) = group {
        let gid = resolve_group(group)?;
        // SAFETY: setgid is safe when dropping privileges.
        if unsafe { libc::setgid(gid) } != 0 {
            return Err(DaemonError::PrivilegeDrop(format!(
                "failed to setgid to {}: {}",
                gid,
                std::io::Error::last_os_error()
            )));
        }
    }

    if let Some(ref user) = user {
        let uid = resolve_user(user)?;
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

/// Resolve a username or numeric UID string to a `uid_t`.
fn resolve_user(user: &str) -> Result<libc::uid_t, DaemonError> {
    if let Ok(uid) = user.parse::<libc::uid_t>() {
        return Ok(uid);
    }

    let c_str =
        std::ffi::CString::new(user).map_err(|_| DaemonError::UserNotFound(user.to_string()))?;

    // SAFETY: getpwnam_r is the thread-safe reentrant version.
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

    if ret != 0 || result.is_null() {
        return Err(DaemonError::UserNotFound(user.to_string()));
    }

    // SAFETY: result points to passwd which we initialized.
    Ok(unsafe { (*result).pw_uid })
}

/// Resolve a [`Group`] value to a numeric `gid_t`.
fn resolve_group(group: &Group) -> Result<libc::gid_t, DaemonError> {
    match group {
        Group::Id(gid) => Ok(*gid),
        Group::Name(name) => {
            if let Ok(gid) = name.parse::<libc::gid_t>() {
                return Ok(gid);
            }

            let c_str = std::ffi::CString::new(name.as_str())
                .map_err(|_| DaemonError::GroupNotFound(name.clone()))?;

            // SAFETY: getgrnam_r is the thread-safe reentrant version.
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

            if ret != 0 || result.is_null() {
                return Err(DaemonError::GroupNotFound(name.clone()));
            }

            // SAFETY: result points to grp which we initialized.
            Ok(unsafe { (*result).gr_gid })
        }
    }
}

/// Redirect stdin, stdout, and stderr for the daemon process.
fn redirect_io(stdout: &Option<File>, stderr: &Option<File>) -> Result<(), DaemonError> {
    let devnull = File::open("/dev/null").map_err(|err| DaemonError::DevNullOpen(err.into()))?;

    // Redirect stdin to /dev/null
    // SAFETY: dup2 is safe for redirecting file descriptors.
    if unsafe { libc::dup2(devnull.as_raw_fd(), libc::STDIN_FILENO) } == -1 {
        return Err(DaemonError::StdinRedirect(
            std::io::Error::last_os_error().into(),
        ));
    }

    // Redirect stdout
    let stdout_fd = stdout
        .as_ref()
        .map(|f| f.as_raw_fd())
        .unwrap_or(devnull.as_raw_fd());
    // SAFETY: dup2 is safe for redirecting file descriptors.
    if unsafe { libc::dup2(stdout_fd, libc::STDOUT_FILENO) } == -1 {
        return Err(DaemonError::StdoutRedirect(
            std::io::Error::last_os_error().into(),
        ));
    }

    // Redirect stderr
    let stderr_fd = stderr
        .as_ref()
        .map(|f| f.as_raw_fd())
        .unwrap_or(devnull.as_raw_fd());
    // SAFETY: dup2 is safe for redirecting file descriptors.
    if unsafe { libc::dup2(stderr_fd, libc::STDERR_FILENO) } == -1 {
        return Err(DaemonError::StderrRedirect(
            std::io::Error::last_os_error().into(),
        ));
    }

    Ok(())
}

/// Write the daemon's PID to the configured PID file.
fn write_pid_file(
    pid_path: &Path,
    chown: bool,
    user: &Option<String>,
    group: &Option<Group>,
) -> Result<(), DaemonError> {
    let pid = std::process::id();

    // Create parent directory if it doesn't exist
    if let Some(parent) = pid_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| DaemonError::PidFileWrite(err.into()))?;
        }
    }

    // Write PID file
    let mut file = File::create(pid_path).map_err(|err| DaemonError::PidFileWrite(err.into()))?;
    writeln!(file, "{}", pid).map_err(|err| DaemonError::PidFileWrite(err.into()))?;
    file.sync_all()
        .map_err(|err| DaemonError::PidFileWrite(err.into()))?;

    // Chown if requested
    if chown {
        let uid = user.as_ref().map(|u| resolve_user(u)).transpose()?;
        let gid = group.as_ref().map(resolve_group).transpose()?;

        if let (Some(uid), Some(gid)) = (uid, gid) {
            let c_path =
                std::ffi::CString::new(pid_path.as_os_str().as_encoded_bytes()).map_err(|_| {
                    DaemonError::PidFileChown(
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "PID path contains null byte",
                        )
                        .into(),
                    )
                })?;

            // SAFETY: chown is safe here as we're just changing ownership.
            if unsafe { libc::chown(c_path.as_ptr(), uid, gid) } != 0 {
                return Err(DaemonError::PidFileChown(
                    std::io::Error::last_os_error().into(),
                ));
            }
        }
    }

    Ok(())
}
