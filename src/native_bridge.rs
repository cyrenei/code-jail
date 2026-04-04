//! Native bridge runtime for executing host binaries through WASM supervision.
//!
//! The WASM supervisor pattern: the .wasm module is the control plane that
//! decides execution happens. The native binary runs on the data plane.
//! The bridge connects them through the `codejail_host.exec` host function.
//!
//! Architecture:
//! ```text
//! User terminal
//!   └─ codejail run --native-exec <binary> bridge.wasm
//!       └─ NativeBridgeRuntime
//!           ├─ loads bridge.wasm (WASM supervisor)
//!           ├─ provides codejail_host.exec host function
//!           └─ bridge.wasm calls exec → forks native binary
//!               └─ native binary inherits terminal (PTY passthrough)
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use wasmtime::*;
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

use crate::sandbox;
use crate::seccomp;

/// Configuration for native binary execution.
#[derive(Debug, Clone)]
pub struct NativeExecConfig {
    /// Path to the native binary to execute.
    pub binary_path: PathBuf,
    /// Arguments to pass to the native binary.
    pub args: Vec<String>,
    /// Environment variables for the native binary.
    pub env_vars: Vec<(String, String)>,
    /// Working directory (None = inherit from parent).
    pub cwd: Option<PathBuf>,
    /// If true, inherit all env vars from parent instead of env_clear + set.
    pub inherit_env: bool,
    /// If true, wrap execution in bwrap for OS-level containment.
    /// Default: true (bwrap ON if available).
    pub use_bwrap: bool,
    /// Resolved sandbox capabilities for bwrap (fs_read, fs_write, net).
    pub sandbox_caps: sandbox::NativeSandboxCaps,
}

/// Combined state for the WASM store: WASI context + native exec config.
struct NativeBridgeState {
    wasi: WasiP1Ctx,
    native_config: NativeExecConfig,
}

/// Runtime for executing WASM bridge modules with native exec support.
pub struct NativeBridgeRuntime {
    engine: Engine,
}

impl NativeBridgeRuntime {
    pub fn new() -> anyhow::Result<Self> {
        let engine = Engine::new(&Config::new())?;
        Ok(Self { engine })
    }

    /// Run a WASM bridge module that calls exec to launch a host binary.
    ///
    /// The bridge module is expected to import `codejail_host.exec` and
    /// call it from `_start`. The host function launches the configured
    /// native binary with terminal inheritance.
    pub fn run(
        &self,
        bridge_wasm: &Path,
        native_config: NativeExecConfig,
        _wasm_args: &[String],
    ) -> anyhow::Result<i32> {
        // Minimal WASI context for the bridge module itself
        let mut builder = WasiCtxBuilder::new();
        builder.allow_blocking_current_thread(true);
        builder.inherit_stdio();
        builder.args(&["bridge.wasm"]);
        let wasi_ctx = builder.build_p1();

        let state = NativeBridgeState {
            wasi: wasi_ctx,
            native_config,
        };

        let mut store = Store::new(&self.engine, state);

        let module = Module::from_file(&self.engine, bridge_wasm)
            .map_err(|e| anyhow::anyhow!("failed to load bridge module '{}': {e}", bridge_wasm.display()))?;

        let mut linker = Linker::<NativeBridgeState>::new(&self.engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |state| &mut state.wasi)?;

        // Register codejail_host.exec — the bridge between WASM and native execution
        linker.func_wrap("codejail_host", "exec", exec_host_function)?;

        linker.module(&mut store, "", &module)?;

        let start = Instant::now();

        let func = linker
            .get_default(&mut store, "")
            .map_err(|e| anyhow::anyhow!("no _start function in bridge module: {e}"))?;

        let typed = func.typed::<(), ()>(&store)?;

        let exit_code = match typed.call(&mut store, ()) {
            Ok(()) => 0,
            Err(e) => {
                if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
                    exit.0
                } else {
                    return Err(e.into());
                }
            }
        };

        eprintln!(
            "[codejail] bridge wall time: {:.2}s",
            start.elapsed().as_secs_f64()
        );

        Ok(exit_code)
    }
}

/// Host function: `codejail_host.exec() -> i32`
///
/// Launches the configured native binary, inheriting the terminal.
/// Returns the process exit code.
fn exec_host_function(caller: Caller<'_, NativeBridgeState>) -> i32 {
    let config = &caller.data().native_config;

    match exec_native(config) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("[codejail] exec_native failed: {e}");
            126 // "command invoked cannot execute"
        }
    }
}

/// Execute a native binary with the given configuration.
///
/// When `use_bwrap` is true and bwrap is available, the binary is wrapped
/// in OS-level namespace isolation (Tier 2). Otherwise falls back to
/// direct execution (no OS containment).
///
/// The child process inherits the parent's terminal directly, preserving:
/// - PTY (isatty() returns true in the child)
/// - Terminal control codes (colors, cursor, etc.)
/// - Signal delivery (Ctrl+C -> SIGINT, etc.)
/// - Terminal size (SIGWINCH propagation via the kernel)
fn exec_native(config: &NativeExecConfig) -> anyhow::Result<i32> {
    if config.use_bwrap {
        return exec_native_bwrap(config);
    }

    exec_native_direct(config)
}

/// Execute a native binary directly (no OS containment, but with seccomp).
///
/// Even without namespace isolation, seccomp-BPF restricts the syscall surface.
/// The filter is applied in `pre_exec` (after fork, before exec) so it constrains
/// the new program. PR_SET_NO_NEW_PRIVS ensures the filter persists across exec.
fn exec_native_direct(config: &NativeExecConfig) -> anyhow::Result<i32> {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new(&config.binary_path);

    cmd.args(&config.args);

    if !config.inherit_env {
        cmd.env_clear();
    }
    for (k, v) in &config.env_vars {
        cmd.env(k, v);
    }

    if let Some(ref cwd) = config.cwd {
        cmd.current_dir(cwd);
    }

    // Inherit stdio — this is what gives us terminal passthrough.
    // The native process gets the same fd 0/1/2 as codejail, which
    // are the real terminal. No PTY allocation needed.
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    // Apply seccomp-BPF filter in pre_exec (after fork, before exec).
    // The filter persists across exec because PR_SET_NO_NEW_PRIVS is set.
    // This constrains what syscalls the native binary can make even without
    // namespace isolation — defense in depth.
    let seccomp_profile = seccomp::SeccompProfile::from_capabilities(&config.sandbox_caps);
    eprintln!("[codejail] {}", seccomp_profile.summary());

    // SAFETY: pre_exec runs between fork and exec in the child process.
    // We only call async-signal-safe operations (prctl) and our BPF
    // setup uses pre-allocated data. No heap allocation or locks.
    unsafe {
        cmd.pre_exec(move || {
            seccomp_profile.apply().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })
        });
    }

    let status = cmd.status().map_err(|e| {
        anyhow::anyhow!(
            "failed to execute '{}': {e}",
            config.binary_path.display()
        )
    })?;

    Ok(status.code().unwrap_or(1))
}

/// Execute a native binary through bwrap OS containment + seccomp.
///
/// The seccomp BPF filter is written to a memfd and passed to bwrap via
/// `--seccomp FD`. This is cleaner than applying seccomp in pre_exec
/// inside bwrap, because bwrap manages the process lifecycle.
///
/// Defense layers applied:
/// 1. Linux namespaces (user, pid, ipc, uts, cgroup, optionally net)
/// 2. Filesystem deny-by-default (only explicitly granted paths visible)
/// 3. Seccomp-BPF syscall filtering (only needed syscalls allowed)
fn exec_native_bwrap(config: &NativeExecConfig) -> anyhow::Result<i32> {
    // Build seccomp filter and write to a memfd for bwrap's --seccomp flag.
    let seccomp_profile = seccomp::SeccompProfile::from_capabilities(&config.sandbox_caps);
    eprintln!("[codejail] {}", seccomp_profile.summary());

    let bpf_bytes = seccomp_profile.to_bpf_bytes();
    let seccomp_fd = seccomp::write_bpf_to_memfd(&bpf_bytes)?;

    // Build the bwrap command with the seccomp fd.
    // bwrap's --seccomp flag reads the BPF program from the fd and applies it
    // after namespace setup but before exec'ing the child binary.
    let mut cmd = sandbox::build_native_bridge_bwrap_command_with_seccomp(
        &config.binary_path,
        &config.args,
        &config.sandbox_caps,
        &config.env_vars,
        config.inherit_env,
        Some(seccomp_fd),
    );

    if let Some(ref cwd) = config.cwd {
        cmd.current_dir(cwd);
    }

    let status = cmd.status().map_err(|e| {
        // Clean up the memfd on error
        unsafe { libc::close(seccomp_fd); }
        anyhow::anyhow!(
            "bwrap exec failed for '{}': {e}",
            config.binary_path.display()
        )
    })?;

    // The memfd is consumed by bwrap; close our copy
    unsafe { libc::close(seccomp_fd); }

    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a minimal NativeExecConfig for testing direct execution.
    fn test_config(binary: &str) -> NativeExecConfig {
        NativeExecConfig {
            binary_path: PathBuf::from(binary),
            args: vec![],
            env_vars: vec![],
            cwd: None,
            inherit_env: false,
            // Tests run without bwrap by default (direct execution)
            use_bwrap: false,
            sandbox_caps: sandbox::NativeSandboxCaps::default(),
        }
    }

    #[test]
    fn test_exec_native_true() {
        let config = test_config("/bin/true");
        assert_eq!(exec_native(&config).unwrap(), 0);
    }

    #[test]
    fn test_exec_native_false() {
        let config = test_config("/bin/false");
        assert_ne!(exec_native(&config).unwrap(), 0);
    }

    #[test]
    fn test_exec_native_with_args() {
        let mut config = test_config("/bin/echo");
        config.args = vec!["hello".into(), "world".into()];
        assert_eq!(exec_native(&config).unwrap(), 0);
    }

    #[test]
    fn test_exec_native_missing_binary() {
        let config = test_config("/nonexistent/binary/that/does/not/exist");
        assert!(exec_native(&config).is_err());
    }

    #[test]
    fn test_exec_native_with_env() {
        let mut config = test_config("/usr/bin/env");
        config.env_vars = vec![("TEST_VAR".into(), "test_value".into())];
        assert_eq!(exec_native(&config).unwrap(), 0);
    }

    #[test]
    fn test_exec_native_with_cwd() {
        let mut config = test_config("/bin/pwd");
        config.cwd = Some(PathBuf::from("/tmp"));
        assert_eq!(exec_native(&config).unwrap(), 0);
    }

    #[test]
    fn test_exec_native_inherit_env() {
        let mut config = test_config("/bin/true");
        config.inherit_env = true;
        assert_eq!(exec_native(&config).unwrap(), 0);
    }

    #[test]
    fn test_native_bridge_runtime_creation() {
        assert!(NativeBridgeRuntime::new().is_ok());
    }
}
