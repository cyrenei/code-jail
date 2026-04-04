use std::path::Path;
use std::process::Command;

/// Check if bubblewrap is available
pub fn bwrap_available() -> bool {
    Command::new("bwrap")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Result of probing the host for sandbox capabilities.
#[derive(Debug, Clone)]
pub struct SandboxCheck {
    /// Whether bubblewrap (bwrap) is installed and runnable.
    pub bwrap_installed: bool,
    /// Whether user namespaces are available (required by bwrap).
    pub user_namespaces: bool,
    /// Human-readable messages describing the sandbox environment.
    pub messages: Vec<String>,
}

impl SandboxCheck {
    /// True if OS-level containment can be enforced.
    pub fn can_sandbox(&self) -> bool {
        self.bwrap_installed && self.user_namespaces
    }
}

/// Probe the host for sandbox capabilities.
///
/// Checks:
/// 1. Is bwrap installed? (`bwrap --version`)
/// 2. Can bwrap actually create a sandbox? (runs a trivial bwrap command)
/// 3. Falls back to user namespace checks if bwrap probe is inconclusive
pub fn check_sandbox_capabilities() -> SandboxCheck {
    let bwrap_installed = bwrap_available();
    let mut messages = Vec::new();

    if bwrap_installed {
        messages.push("bwrap: installed".into());
    } else {
        messages.push("bwrap: NOT FOUND ��� install bubblewrap for OS containment".into());
        return SandboxCheck {
            bwrap_installed: false,
            user_namespaces: false,
            messages,
        };
    }

    // Actually try to run bwrap with minimal isolation to see if it works.
    // This catches environments where bwrap is installed but namespaces are
    // restricted (e.g., running inside a container, restricted seccomp, etc.).
    let bwrap_functional = check_bwrap_functional(&mut messages);

    // Also check user namespaces for diagnostics when bwrap is not functional
    if !bwrap_functional {
        let _userns = check_user_namespaces(&mut messages);
    }

    SandboxCheck {
        bwrap_installed,
        // Only report can_sandbox() if bwrap actually works
        user_namespaces: bwrap_functional,
        messages,
    }
}

/// Try to run a trivial bwrap sandbox to verify it actually works.
fn check_bwrap_functional(messages: &mut Vec<String>) -> bool {
    // Minimal bwrap invocation: unshare user+pid, bind /usr, run /bin/true.
    // Note: we bind /lib64 directly (no --symlink) because on Debian/Ubuntu
    // the ELF interpreter already lives inside /lib64 as a symlink.
    // Adding --symlink on top of --ro-bind /lib64 causes bwrap to fail with
    // "Can't make symlink: existing destination".
    let mut args = vec![
        "--unshare-user",
        "--unshare-pid",
        "--ro-bind", "/usr", "/usr",
        "--ro-bind", "/lib", "/lib",
        "--dev", "/dev",
        "--proc", "/proc",
        "--die-with-parent",
    ];
    if Path::new("/lib64").exists() {
        args.extend_from_slice(&["--ro-bind", "/lib64", "/lib64"]);
    }
    // Use /usr/bin/true (not /bin/true) because on modern Debian/Ubuntu
    // /bin is a symlink to /usr/bin, which doesn't exist inside the sandbox.
    args.extend_from_slice(&["--", "/usr/bin/true"]);
    let result = Command::new("bwrap")
        .args(&args)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            messages.push("bwrap: functional (probe succeeded)".into());
            true
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            messages.push(format!(
                "bwrap: installed but NOT FUNCTIONAL (exit {}, {})",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            ));
            false
        }
        Err(e) => {
            messages.push(format!("bwrap: probe failed ({e})"));
            false
        }
    }
}

fn check_user_namespaces(messages: &mut Vec<String>) -> bool {
    // Method 1: check procfs knob (Debian/Ubuntu)
    if let Ok(content) = std::fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone") {
        let enabled = content.trim() == "1";
        if enabled {
            messages.push("user namespaces: enabled (procfs)".into());
        } else {
            messages.push("user namespaces: DISABLED in /proc/sys/kernel/unprivileged_userns_clone".into());
            return false;
        }
    }

    // Check AppArmor restriction (Ubuntu 24.04+).
    // Even when unprivileged_userns_clone=1, AppArmor can block user namespace
    // creation unless the binary has an AppArmor profile with userns_create.
    if let Ok(content) = std::fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns") {
        let restricted = content.trim() == "1";
        if restricted {
            messages.push(
                "user namespaces: BLOCKED by AppArmor (apparmor_restrict_unprivileged_userns=1)".into()
            );
            messages.push(
                "  fix: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0".into()
            );
            // Don't return false yet — let the unshare probe below do the definitive test,
            // since the binary might have an AppArmor profile that allows it.
        }
    }

    // Method 2: attempt unshare -U true (definitive functional test)
    if let Ok(output) = Command::new("unshare").args(["-U", "true"]).output() {
        let available = output.status.success();
        if available {
            messages.push("user namespaces: available (unshare probe)".into());
        } else {
            messages.push("user namespaces: UNAVAILABLE (unshare -U failed)".into());
        }
        return available;
    }

    // Method 3: check max_user_namespaces > 0
    if let Ok(content) = std::fs::read_to_string("/proc/sys/user/max_user_namespaces") {
        let max: u64 = content.trim().parse().unwrap_or(0);
        let available = max > 0;
        if available {
            messages.push(format!("user namespaces: available (max={})", max));
        } else {
            messages.push("user namespaces: DISABLED (max_user_namespaces=0)".into());
        }
        return available;
    }

    // Cannot determine — assume available (most modern kernels)
    messages.push("user namespaces: assumed available (could not probe)".into());
    true
}

/// Resolved sandbox capabilities for a native binary execution.
/// Passed to `build_native_bridge_bwrap_command` to construct the bwrap invocation.
#[derive(Debug, Clone, Default)]
pub struct NativeSandboxCaps {
    /// Paths to bind read-only inside the sandbox.
    pub fs_read: Vec<String>,
    /// Paths to bind read-write inside the sandbox.
    pub fs_write: Vec<String>,
    /// Whether to allow network access.
    pub allow_network: bool,
}

/// Build a bubblewrap command that wraps execution of the inner command.
///
/// The outer sandbox uses Linux namespaces to isolate the wasmtime process
/// itself, adding defense in depth on top of WASM's capability isolation.
pub fn build_bwrap_command(
    inner_cmd: &str,
    inner_args: &[&str],
    readonly_binds: &[&str],
    writable_binds: &[(&str, &str)],
    allow_network: bool,
) -> Command {
    let mut cmd = Command::new("bwrap");

    // Unshare everything
    cmd.arg("--unshare-user")
        .arg("--unshare-pid")
        .arg("--unshare-ipc")
        .arg("--unshare-uts")
        .arg("--unshare-cgroup");

    if !allow_network {
        cmd.arg("--unshare-net");
    }

    // Minimal filesystem
    cmd.arg("--tmpfs").arg("/tmp");
    cmd.arg("--dev").arg("/dev");
    cmd.arg("--proc").arg("/proc");

    // System directories needed by wasmtime (read-only)
    for sys_dir in &[
        "/usr",
        "/lib",
        "/etc/alternatives",
        "/etc/ld.so.cache",
    ] {
        if std::path::Path::new(sys_dir).exists() {
            cmd.arg("--ro-bind").arg(sys_dir).arg(sys_dir);
        }
    }
    // Handle merged-usr symlinks (Debian/Ubuntu: /bin→/usr/bin, etc.)
    for (link, target) in &[
        ("/bin", "/usr/bin"),
        ("/sbin", "/usr/sbin"),
        ("/lib64", "/usr/lib64"),
    ] {
        let link_path = std::path::Path::new(link);
        if link_path.is_symlink() {
            cmd.arg("--symlink").arg(target).arg(link);
        } else if link_path.is_dir() {
            cmd.arg("--ro-bind").arg(link).arg(link);
        }
    }

    // User-specified read-only binds
    for path in readonly_binds {
        cmd.arg("--ro-bind").arg(path).arg(path);
    }

    // Writable binds
    for (host, guest) in writable_binds {
        cmd.arg("--bind").arg(host).arg(guest);
    }

    // Die with parent process
    cmd.arg("--die-with-parent");

    // The inner command
    cmd.arg("--").arg(inner_cmd);
    for arg in inner_args {
        cmd.arg(arg);
    }

    cmd
}

/// Build a bubblewrap command for native bridge execution.
///
/// Wraps a native binary in OS-level namespace isolation based on the
/// resolved capabilities from the JailFile. This is the Tier 2 enforcement
/// layer for `codejail run --native-exec`.
///
/// Isolation properties:
/// - User, PID, IPC, UTS, cgroup namespaces always unshared
/// - Network namespace unshared unless `caps.allow_network` is true
/// - Filesystem is deny-by-default: only explicitly listed paths are visible
/// - Binary and its interpreter/libraries are auto-bound read-only
/// - Process dies with parent (no orphan escape)
pub fn build_native_bridge_bwrap_command(
    binary_path: &Path,
    args: &[String],
    caps: &NativeSandboxCaps,
    env_vars: &[(String, String)],
    inherit_env: bool,
) -> Command {
    build_native_bridge_bwrap_command_with_seccomp(
        binary_path, args, caps, env_vars, inherit_env, None,
    )
}

/// Build a bubblewrap command for native bridge execution with optional seccomp.
///
/// Same as `build_native_bridge_bwrap_command` but accepts an optional seccomp
/// file descriptor. When provided, bwrap applies the BPF filter via `--seccomp FD`
/// after setting up namespaces but before exec'ing the child binary.
///
/// This adds syscall filtering on top of namespace isolation:
/// - Namespaces control WHAT the process can see (filesystem, network, PIDs)
/// - Seccomp controls WHAT OPERATIONS the process can perform (syscalls)
pub fn build_native_bridge_bwrap_command_with_seccomp(
    binary_path: &Path,
    args: &[String],
    caps: &NativeSandboxCaps,
    env_vars: &[(String, String)],
    inherit_env: bool,
    seccomp_fd: Option<std::os::unix::io::RawFd>,
) -> Command {
    let mut cmd = Command::new("bwrap");

    // Unshare everything
    cmd.arg("--unshare-user")
        .arg("--unshare-pid")
        .arg("--unshare-ipc")
        .arg("--unshare-uts")
        .arg("--unshare-cgroup");

    if !caps.allow_network {
        cmd.arg("--unshare-net");
    }

    // Apply seccomp filter if provided.
    // The --seccomp flag tells bwrap to read a BPF program from the given fd
    // and apply it to the child process after namespace setup.
    if let Some(fd) = seccomp_fd {
        cmd.arg("--seccomp").arg(fd.to_string());
    }

    // Minimal filesystem
    cmd.arg("--tmpfs").arg("/tmp");
    cmd.arg("--dev").arg("/dev");
    cmd.arg("--proc").arg("/proc");

    // System directories needed for dynamic linking (read-only).
    // On merged-usr systems (Debian/Ubuntu), /bin, /sbin, /lib64 are symlinks
    // into /usr. We bind the real directories and recreate the symlinks so that
    // paths like /bin/sh and /lib64/ld-linux-x86-64.so.2 resolve correctly.
    for sys_dir in &[
        "/usr",
        "/lib",
        "/etc/alternatives",
        "/etc/ld.so.cache",
        "/etc/ld.so.conf",
        "/etc/ld.so.conf.d",
    ] {
        if Path::new(sys_dir).exists() {
            cmd.arg("--ro-bind").arg(sys_dir).arg(sys_dir);
        }
    }
    // Handle merged-usr symlinks: recreate them inside the sandbox
    for (link, target) in &[
        ("/bin", "/usr/bin"),
        ("/sbin", "/usr/sbin"),
        ("/lib64", "/usr/lib64"),
    ] {
        let link_path = Path::new(link);
        if link_path.is_symlink() {
            // It's a symlink on the host → recreate as symlink in sandbox
            cmd.arg("--symlink").arg(target).arg(link);
        } else if link_path.is_dir() {
            // Real directory → bind-mount it
            cmd.arg("--ro-bind").arg(link).arg(link);
        }
    }

    // The binary itself must be readable
    let binary_str = binary_path.to_string_lossy();
    cmd.arg("--ro-bind").arg(binary_str.as_ref()).arg(binary_str.as_ref());

    // JailFile fs_read paths as read-only binds
    for path in &caps.fs_read {
        if Path::new(path).exists() {
            cmd.arg("--ro-bind").arg(path).arg(path);
        }
    }

    // JailFile fs_write paths as read-write binds
    for path in &caps.fs_write {
        if Path::new(path).exists() {
            cmd.arg("--bind").arg(path).arg(path);
        }
    }

    // Die with parent process
    cmd.arg("--die-with-parent");

    // Environment handling
    if !inherit_env {
        cmd.arg("--clearenv");
    }
    for (k, v) in env_vars {
        cmd.arg("--setenv").arg(k).arg(v);
    }

    // The native binary and its arguments
    cmd.arg("--").arg(binary_str.as_ref());
    for arg in args {
        cmd.arg(arg);
    }

    // Inherit stdio for terminal passthrough
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    cmd
}
