//! Landlock LSM enforcement for native bridge mode.
//!
//! Applies kernel-level filesystem restrictions to native processes before exec.
//! Uses raw syscalls (no external crate) — security code should be explicit.
//!
//! Landlock ABI v1 (Linux 5.13+): filesystem access control
//! Landlock ABI v2 (Linux 5.19+): adds REFER
//! Landlock ABI v3 (Linux 6.2+):  adds TRUNCATE
//!
//! We target ABI v1 as the minimum. Higher ABIs are detected and used when
//! available to avoid "silently permitted" gaps on newer kernels.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;

use nix::libc;

// ---------------------------------------------------------------------------
// Syscall numbers (identical on x86_64 and aarch64 since Linux 5.13)
// ---------------------------------------------------------------------------

const SYS_LANDLOCK_CREATE_RULESET: libc::c_long = 444;
const SYS_LANDLOCK_ADD_RULE: libc::c_long = 445;
const SYS_LANDLOCK_RESTRICT_SELF: libc::c_long = 446;

// ---------------------------------------------------------------------------
// Landlock constants
// ---------------------------------------------------------------------------

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1 << 0;
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

// ABI v1 filesystem access rights (Linux 5.13)
const ACCESS_FS_EXECUTE: u64 = 1 << 0;
const ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const ACCESS_FS_READ_FILE: u64 = 1 << 2;
const ACCESS_FS_READ_DIR: u64 = 1 << 3;
const ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
const ACCESS_FS_MAKE_SYM: u64 = 1 << 12;

// ABI v2 (Linux 5.19)
const ACCESS_FS_REFER: u64 = 1 << 13;

// ABI v3 (Linux 6.2)
const ACCESS_FS_TRUNCATE: u64 = 1 << 14;

/// All access rights for a given ABI version.
fn all_access_for_abi(abi: i32) -> u64 {
    let mut access = ACCESS_FS_EXECUTE
        | ACCESS_FS_WRITE_FILE
        | ACCESS_FS_READ_FILE
        | ACCESS_FS_READ_DIR
        | ACCESS_FS_REMOVE_DIR
        | ACCESS_FS_REMOVE_FILE
        | ACCESS_FS_MAKE_CHAR
        | ACCESS_FS_MAKE_DIR
        | ACCESS_FS_MAKE_REG
        | ACCESS_FS_MAKE_SOCK
        | ACCESS_FS_MAKE_FIFO
        | ACCESS_FS_MAKE_BLOCK
        | ACCESS_FS_MAKE_SYM;
    if abi >= 2 {
        access |= ACCESS_FS_REFER;
    }
    if abi >= 3 {
        access |= ACCESS_FS_TRUNCATE;
    }
    access
}

/// Access rights valid for regular files (non-directories).
const FILE_ONLY_ACCESS: u64 = ACCESS_FS_EXECUTE | ACCESS_FS_WRITE_FILE | ACCESS_FS_READ_FILE;

/// Read-only access for directories: execute + read files + read dirs.
const READ_ACCESS_DIR: u64 = ACCESS_FS_EXECUTE | ACCESS_FS_READ_FILE | ACCESS_FS_READ_DIR;

/// Read-only access for regular files.
const READ_ACCESS_FILE: u64 = ACCESS_FS_EXECUTE | ACCESS_FS_READ_FILE;

/// Read-only access for a path, accounting for file vs directory and ABI version.
fn read_access_for(abi: i32, is_dir: bool) -> u64 {
    if is_dir {
        let mut access = READ_ACCESS_DIR;
        if abi >= 2 {
            access |= ACCESS_FS_REFER;
        }
        access
    } else {
        READ_ACCESS_FILE
    }
}

/// Full access for a path, accounting for file vs directory and ABI version.
fn write_access_for(abi: i32, is_dir: bool) -> u64 {
    if is_dir {
        all_access_for_abi(abi)
    } else {
        let mut access = FILE_ONLY_ACCESS;
        if abi >= 3 {
            access |= ACCESS_FS_TRUNCATE;
        }
        access
    }
}

// ---------------------------------------------------------------------------
// Landlock kernel structs (repr C for syscall interface)
// ---------------------------------------------------------------------------

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    // v4 adds handled_access_net here, but we don't set the size to include it
    // unless we detect v4+. For v1-v3, the kernel ignores extra zeroed bytes,
    // but we only pass the v1 struct size.
}

#[repr(C)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// A prepared Landlock ruleset, ready to be enforced via `restrict_child`.
///
/// Create this in the parent process. The raw fd is used in `pre_exec` to
/// apply restrictions to the child process only.
pub struct PreparedRuleset {
    ruleset_fd: OwnedFd,
    /// Keep path fds alive — the kernel references them by fd number in rules,
    /// and they must remain open until the ruleset is applied.
    _path_fds: Vec<OwnedFd>,
}

impl PreparedRuleset {
    /// Raw fd for use in `pre_exec`. The fd is valid as long as `self` lives.
    pub fn raw_fd(&self) -> RawFd {
        self.ruleset_fd.as_raw_fd()
    }
}

/// Detect the best available Landlock ABI version.
///
/// Returns `None` if Landlock is not supported (old kernel or disabled).
pub fn detect_abi() -> Option<i32> {
    let version = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if version < 0 {
        None
    } else {
        Some(version as i32)
    }
}

/// Prepare a Landlock ruleset from a list of (path, writable) pairs.
///
/// Opens each path as a directory fd and creates rules granting the
/// appropriate access. Call this in the parent process before spawning.
///
/// Fails if:
/// - Landlock is not supported by the kernel
/// - A mount path does not exist or cannot be opened
pub fn prepare(mounts: &[(impl AsRef<Path>, bool)]) -> io::Result<PreparedRuleset> {
    let abi = detect_abi().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "landlock not supported by this kernel (requires Linux 5.13+ with CONFIG_SECURITY_LANDLOCK=y)",
        )
    })?;

    let handled = all_access_for_abi(abi);

    // Create ruleset
    let attr = RulesetAttr {
        handled_access_fs: handled,
    };
    let fd = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &attr as *const RulesetAttr,
            std::mem::size_of::<RulesetAttr>(),
            0u32,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let ruleset_fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };

    let mut path_fds = Vec::with_capacity(mounts.len());

    for (path, writable) in mounts {
        let path = path.as_ref();

        // Open path with O_PATH — doesn't require read permission on content,
        // just the ability to resolve the path. Works for directories and files.
        let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))?;

        let path_fd = unsafe { libc::open(c_path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if path_fd < 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("landlock: cannot open '{}': {}", path.display(), io::Error::last_os_error()),
            ));
        }
        let path_fd = unsafe { OwnedFd::from_raw_fd(path_fd) };

        // Landlock requires different access flags for files vs directories.
        // Non-directory paths reject directory-specific flags with EINVAL.
        let is_dir = path.is_dir();
        let access = if *writable {
            write_access_for(abi, is_dir)
        } else {
            read_access_for(abi, is_dir)
        };

        let beneath = PathBeneathAttr {
            allowed_access: access,
            parent_fd: path_fd.as_raw_fd(),
        };

        let ret = unsafe {
            libc::syscall(
                SYS_LANDLOCK_ADD_RULE,
                ruleset_fd.as_raw_fd(),
                LANDLOCK_RULE_PATH_BENEATH,
                &beneath as *const PathBeneathAttr,
                0u32,
            )
        };
        if ret < 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "landlock: add_rule failed for '{}': {}",
                    path.display(),
                    io::Error::last_os_error()
                ),
            ));
        }

        path_fds.push(path_fd);
    }

    Ok(PreparedRuleset {
        ruleset_fd,
        _path_fds: path_fds,
    })
}

/// Apply a prepared Landlock ruleset to the current process.
///
/// # Safety
///
/// This is intended to be called from `Command::pre_exec` (between fork and exec).
/// It only calls async-signal-safe functions: `prctl` and `syscall`.
///
/// After this call, the process (and all future children) are restricted to
/// the paths in the ruleset. This is irreversible.
pub unsafe fn restrict(ruleset_fd: RawFd) -> io::Result<()> {
    // Required: set no-new-privs so Landlock can restrict an unprivileged process
    let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }

    let ret = unsafe { libc::syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset_fd, 0u32) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_abi() {
        // This test will pass on kernels with Landlock (5.13+) and skip otherwise
        match detect_abi() {
            Some(v) => {
                assert!(v >= 1, "Landlock ABI version should be >= 1, got {v}");
                eprintln!("Landlock ABI version: {v}");
            }
            None => {
                eprintln!("Landlock not available on this kernel — skipping");
            }
        }
    }

    #[test]
    fn test_prepare_with_tmp() {
        if detect_abi().is_none() {
            eprintln!("Landlock not available — skipping");
            return;
        }

        let mounts: Vec<(&Path, bool)> = vec![
            (Path::new("/tmp"), true),
            (Path::new("/usr"), false),
        ];
        let ruleset = prepare(&mounts);
        assert!(ruleset.is_ok(), "prepare failed: {:?}", ruleset.err());
        assert!(ruleset.unwrap().raw_fd() >= 0);
    }

    #[test]
    fn test_prepare_nonexistent_path() {
        if detect_abi().is_none() {
            eprintln!("Landlock not available — skipping");
            return;
        }

        let mounts: Vec<(&Path, bool)> = vec![
            (Path::new("/nonexistent/path/that/does/not/exist"), false),
        ];
        assert!(prepare(&mounts).is_err());
    }
}
