use std::process::Command;

/// Check if bubblewrap is available
pub fn bwrap_available() -> bool {
    Command::new("bwrap")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Build a bubblewrap command that wraps execution of the inner command.
///
/// The outer sandbox uses Linux namespaces to isolate the wasmtime process
/// itself — defense in depth on top of WASM's capability isolation.
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
    for sys_dir in &["/usr", "/lib", "/lib64", "/etc/alternatives", "/etc/ld.so.cache"] {
        if std::path::Path::new(sys_dir).exists() {
            cmd.arg("--ro-bind").arg(sys_dir).arg(sys_dir);
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
