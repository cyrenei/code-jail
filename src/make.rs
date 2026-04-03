//! `codejail make` — package a native binary into a WASM-supervised sandbox.
//!
//! Generates:
//! 1. A WASM bridge module (.wasm) — the supervisor that mediates execution
//! 2. A JailFile.toml — auto-inferred capability manifest
//! 3. A launcher script — self-contained executable that invokes codejail
//!
//! The WASM Supervisor Pattern: the .wasm module is the control plane.
//! It doesn't contain the binary's code (ISA translation is impossible).
//! Instead, it imports `codejail_host.exec` and calls it. The runtime
//! enforces the JailFile's capability policy before launching the native binary.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::analyzer::{self, BinaryAnalysis, BinaryType};

/// Configuration for the make command.
pub struct MakeConfig {
    pub binary_path: PathBuf,
    pub output_name: String,
    pub analyze_only: bool,
    pub permissive: bool,
}

/// Output artifacts from a successful make.
#[derive(Debug)]
pub struct MakeOutput {
    pub staging_dir: PathBuf,
    pub bridge_wasm_path: PathBuf,
    pub jailfile_path: PathBuf,
    pub launcher_path: PathBuf,
}

/// Execute the make command.
pub fn cmd_make(config: MakeConfig) -> anyhow::Result<Option<MakeOutput>> {
    eprintln!(
        "[codejail make] analyzing {}...",
        config.binary_path.display()
    );

    let analysis = analyzer::analyze(&config.binary_path)?;
    print_analysis(&analysis);

    if config.analyze_only {
        eprintln!();
        eprintln!("[codejail make] analysis complete (--analyze-only, no files generated)");
        return Ok(None);
    }

    // Create staging directory alongside the launcher
    let staging_dir = PathBuf::from(format!("{}.d", config.output_name));
    fs::create_dir_all(&staging_dir)?;

    // Generate WASM bridge module
    let bridge_path = staging_dir.join("bridge.wasm");
    generate_bridge_wasm(&bridge_path)?;
    eprintln!(
        "[codejail make] bridge:   {}",
        bridge_path.display()
    );

    // Generate JailFile
    let jailfile_path = staging_dir.join("JailFile.toml");
    generate_jailfile(&jailfile_path, &analysis, &config)?;
    eprintln!(
        "[codejail make] jailfile: {}",
        jailfile_path.display()
    );

    // Generate launcher script
    let launcher_path = PathBuf::from(&config.output_name);
    generate_launcher(&launcher_path, &staging_dir, &analysis)?;
    eprintln!(
        "[codejail make] launcher: {}",
        launcher_path.display()
    );

    eprintln!();
    eprintln!("[codejail make] done. run with:");
    eprintln!("  ./{}", config.output_name);
    eprintln!();
    eprintln!("[codejail make] edit permissions:");
    eprintln!("  {}", jailfile_path.display());

    Ok(Some(MakeOutput {
        staging_dir,
        bridge_wasm_path: bridge_path,
        jailfile_path,
        launcher_path,
    }))
}

fn print_analysis(a: &BinaryAnalysis) {
    eprintln!("[codejail make] binary:  {}", a.binary_path.display());
    match &a.binary_type {
        BinaryType::Elf => eprintln!("[codejail make] type:    ELF"),
        BinaryType::Script { interpreter } => {
            eprintln!(
                "[codejail make] type:    script ({})",
                interpreter.display()
            )
        }
        BinaryType::Symlink { target } => {
            eprintln!("[codejail make] type:    symlink -> {}", target.display())
        }
        BinaryType::Unknown => eprintln!("[codejail make] type:    unknown"),
    }
    if let Some(ref interp) = a.interpreter {
        eprintln!("[codejail make] interp:  {}", interp.display());
    }
    if !a.linked_libraries.is_empty() {
        eprintln!(
            "[codejail make] libs:    {} linked",
            a.linked_libraries.len()
        );
    }
    for note in &a.notes {
        eprintln!("[codejail make]   {note}");
    }
    eprintln!(
        "[codejail make] inferred: {} fs_read, {} fs_write, net={}, {} env",
        a.inferred_fs_read.len(),
        a.inferred_fs_write.len(),
        a.needs_network,
        a.inferred_env.len(),
    );
}

// ---------------------------------------------------------------------------
// WASM bridge generation
// ---------------------------------------------------------------------------

/// The WAT source for the bridge module.
///
/// This is the WASM supervisor: it imports the host function and calls it.
/// The module is intentionally minimal — all intelligence is in the runtime.
const BRIDGE_WAT: &str = r#"(module
  ;; codejail native bridge — WASM supervisor module
  ;;
  ;; This module mediates execution of a native binary through the
  ;; codejail runtime. It is the control plane: it decides that execution
  ;; happens by calling exec. The runtime is the enforcement plane:
  ;; it applies JailFile capabilities before launching the binary.

  ;; Host function: launch the configured native binary.
  ;; Returns the process exit code.
  (import "codejail_host" "exec" (func $exec (result i32)))

  ;; WASI proc_exit for clean shutdown with the native process's exit code.
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))

  ;; Memory export — required by WASI even for modules that don't use it.
  (memory (export "memory") 1)

  ;; WASI _start entry point.
  (func (export "_start")
    ;; Launch the native binary via host bridge
    call $exec
    ;; Propagate its exit code through WASI
    call $proc_exit
    ;; Unreachable — proc_exit terminates the module
    unreachable
  )
)"#;

/// Generate the WASM bridge module from WAT.
pub fn generate_bridge_wasm(output: &Path) -> anyhow::Result<()> {
    let wasm_bytes =
        wat::parse_str(BRIDGE_WAT).map_err(|e| anyhow::anyhow!("WAT compilation failed: {e}"))?;
    fs::write(output, wasm_bytes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// JailFile generation
// ---------------------------------------------------------------------------

/// Generate a JailFile.toml from binary analysis results.
pub fn generate_jailfile(
    output: &Path,
    analysis: &BinaryAnalysis,
    config: &MakeConfig,
) -> anyhow::Result<()> {
    let mut out = String::with_capacity(1024);

    // Header
    out.push_str("# Auto-generated by `codejail make`\n");
    out.push_str(&format!("# Source: {}\n", analysis.binary_path.display()));
    out.push_str(&format!(
        "# Type:   {}\n",
        match &analysis.binary_type {
            BinaryType::Elf => "ELF".to_string(),
            BinaryType::Script { interpreter } =>
                format!("script ({})", interpreter.display()),
            BinaryType::Symlink { target } => format!("symlink -> {}", target.display()),
            BinaryType::Unknown => "unknown".to_string(),
        }
    ));
    out.push_str("#\n");
    out.push_str("# Review and tighten these permissions before production use.\n");
    out.push_str("# Auto-inferred capabilities are PERMISSIVE by design (T-003).\n\n");

    // [sandbox]
    out.push_str("[sandbox]\n");
    out.push_str(&format!("name = \"{}\"\n", config.output_name));
    out.push_str("entrypoint = \"bridge.wasm\"\n\n");

    // [capabilities]
    out.push_str("[capabilities]\n");

    out.push_str("fs_read = [\n");
    for p in &analysis.inferred_fs_read {
        out.push_str(&format!("    \"{}\",\n", p.display()));
    }
    out.push_str("]\n");

    out.push_str("fs_write = [\n");
    for p in &analysis.inferred_fs_write {
        out.push_str(&format!("    \"{}\",\n", p.display()));
    }
    out.push_str("]\n");

    if analysis.needs_network || config.permissive {
        out.push_str("net_allow = [\"*\"]\n");
    } else {
        out.push_str("net_allow = []\n");
    }

    out.push_str("env = [\n");
    for v in &analysis.inferred_env {
        out.push_str(&format!("    \"{v}\",\n"));
    }
    out.push_str("]\n");

    if config.permissive {
        out.push_str("inherit_env = true\n");
    } else {
        out.push_str("inherit_env = false\n");
    }
    out.push_str("stdin = true\n");
    out.push_str("stdout = true\n");
    out.push_str("stderr = true\n\n");

    // [limits]
    out.push_str("[limits]\n");
    out.push_str("memory_mb = 512\n");
    // fuel = 0 means no CPU fuel limit (native exec, not WASM metering)
    out.push_str("fuel = 0\n");
    // wall_time_secs = 0 means no timeout (interactive applications)
    out.push_str("wall_time_secs = 0\n");

    fs::write(output, &out)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Launcher generation
// ---------------------------------------------------------------------------

/// Find the codejail binary path for embedding in the launcher.
fn find_codejail_binary() -> PathBuf {
    // Check PATH
    if let Some(path) = analyzer::which("codejail") {
        return path;
    }
    // Fall back to current executable
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("codejail"))
}

/// Generate a launcher shell script.
pub fn generate_launcher(
    output: &Path,
    staging_dir: &Path,
    analysis: &BinaryAnalysis,
) -> anyhow::Result<()> {
    let staging_abs = fs::canonicalize(staging_dir)?;
    let codejail_bin = find_codejail_binary();

    let type_desc = match &analysis.binary_type {
        BinaryType::Elf => "ELF".to_string(),
        BinaryType::Script { interpreter } => format!("script ({})", interpreter.display()),
        BinaryType::Symlink { target } => format!("symlink -> {}", target.display()),
        BinaryType::Unknown => "unknown".to_string(),
    };

    let script = format!(
        r#"#!/bin/sh
# codejail launcher — generated by `codejail make`
#
# Binary:  {binary}
# Type:    {type_desc}
# Staging: {staging}
#
# To edit permissions, modify: {staging}/JailFile.toml

set -e

STAGING="{staging}"
CODEJAIL="{codejail}"

# Verify staging directory exists
if [ ! -d "$STAGING" ]; then
    echo "error: staging directory not found: $STAGING" >&2
    echo "hint: was the .d directory moved or deleted?" >&2
    exit 1
fi

# Verify bridge module exists
if [ ! -f "$STAGING/bridge.wasm" ]; then
    echo "error: bridge module not found: $STAGING/bridge.wasm" >&2
    exit 1
fi

# Launch through codejail native bridge
exec "$CODEJAIL" run \
    --native-exec "{binary}" \
    --jailfile "$STAGING/JailFile.toml" \
    --fuel 0 \
    --timeout 0 \
    "$STAGING/bridge.wasm" \
    -- "$@"
"#,
        binary = analysis.binary_path.display(),
        type_desc = type_desc,
        staging = staging_abs.display(),
        codejail = codejail_bin.display(),
    );

    fs::write(output, &script)?;

    // Make executable
    let mut perms = fs::metadata(output)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(output, perms)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bridge_wat_compiles() {
        let wasm = wat::parse_str(BRIDGE_WAT);
        assert!(wasm.is_ok(), "bridge WAT should compile: {:?}", wasm.err());
    }

    #[test]
    fn test_bridge_wasm_valid_header() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bridge.wasm");
        generate_bridge_wasm(&path).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 8, "wasm should be non-trivial");
        assert_eq!(&bytes[..4], b"\x00asm", "WASM magic number");
        assert_eq!(&bytes[4..8], &[1, 0, 0, 0], "WASM version 1");
    }

    #[test]
    fn test_bridge_wasm_loadable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bridge.wasm");
        generate_bridge_wasm(&path).unwrap();

        // Verify wasmtime can load it
        let engine = wasmtime::Engine::default();
        let result = wasmtime::Module::from_file(&engine, &path);
        assert!(result.is_ok(), "wasmtime should load bridge: {:?}", result.err());

        let module = result.unwrap();
        let imports: Vec<_> = module.imports().map(|i| {
            format!("{}.{}", i.module(), i.name())
        }).collect();
        assert!(imports.contains(&"codejail_host.exec".to_string()),
            "bridge should import codejail_host.exec, got: {:?}", imports);
    }

    #[test]
    fn test_generate_jailfile_structure() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("JailFile.toml");

        let analysis = BinaryAnalysis {
            binary_path: PathBuf::from("/bin/echo"),
            binary_type: BinaryType::Elf,
            interpreter: None,
            linked_libraries: vec![],
            inferred_fs_read: vec![PathBuf::from("/bin/echo"), PathBuf::from("/usr/lib")],
            inferred_fs_write: vec![PathBuf::from("/tmp")],
            needs_network: false,
            inferred_env: vec!["PATH".into(), "HOME".into()],
            notes: vec![],
        };

        let config = MakeConfig {
            binary_path: PathBuf::from("/bin/echo"),
            output_name: "jailed-echo".into(),
            analyze_only: false,
            permissive: false,
        };

        generate_jailfile(&path, &analysis, &config).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        // Verify it's valid TOML
        let parsed: toml::Value = toml::from_str(&content).expect("should be valid TOML");
        assert!(parsed.get("sandbox").is_some());
        assert!(parsed.get("capabilities").is_some());
        assert!(parsed.get("limits").is_some());

        // Verify our JailFile struct can parse it
        let jf: crate::capability::JailFile = toml::from_str(&content)
            .expect("should parse as JailFile");
        assert_eq!(jf.sandbox.name, Some("jailed-echo".to_string()));
        assert_eq!(jf.sandbox.entrypoint, "bridge.wasm");
        assert!(jf.capabilities.fs_read.contains(&"/bin/echo".to_string()));
        assert!(jf.capabilities.stdin);
    }

    #[test]
    fn test_generate_jailfile_permissive_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("JailFile.toml");

        let analysis = BinaryAnalysis {
            binary_path: PathBuf::from("/bin/echo"),
            binary_type: BinaryType::Elf,
            interpreter: None,
            linked_libraries: vec![],
            inferred_fs_read: vec![],
            inferred_fs_write: vec![],
            needs_network: false,
            inferred_env: vec![],
            notes: vec![],
        };

        let config = MakeConfig {
            binary_path: PathBuf::from("/bin/echo"),
            output_name: "test".into(),
            analyze_only: false,
            permissive: true,
        };

        generate_jailfile(&path, &analysis, &config).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("inherit_env = true"));
        assert!(content.contains("net_allow = [\"*\"]"));
    }

    #[test]
    fn test_generate_launcher_executable() {
        let dir = TempDir::new().unwrap();
        let staging = dir.path().join("test.d");
        fs::create_dir_all(&staging).unwrap();

        let launcher = dir.path().join("test-launcher");
        let analysis = BinaryAnalysis {
            binary_path: PathBuf::from("/bin/echo"),
            binary_type: BinaryType::Elf,
            interpreter: None,
            linked_libraries: vec![],
            inferred_fs_read: vec![],
            inferred_fs_write: vec![],
            needs_network: false,
            inferred_env: vec![],
            notes: vec![],
        };

        generate_launcher(&launcher, &staging, &analysis).unwrap();

        let meta = fs::metadata(&launcher).unwrap();
        assert!(
            meta.permissions().mode() & 0o111 != 0,
            "launcher should be executable"
        );

        let content = fs::read_to_string(&launcher).unwrap();
        assert!(content.starts_with("#!/bin/sh"));
        assert!(content.contains("codejail"));
        assert!(content.contains("--native-exec"));
        assert!(content.contains("--jailfile"));
        assert!(content.contains("bridge.wasm"));
        assert!(content.contains("/bin/echo"));
    }

    #[test]
    fn test_cmd_make_analyze_only() {
        let config = MakeConfig {
            binary_path: PathBuf::from("/bin/true"),
            output_name: "test-output".into(),
            analyze_only: true,
            permissive: false,
        };
        let result = cmd_make(config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none(), "analyze-only should produce no output");
    }

    #[test]
    fn test_cmd_make_end_to_end() {
        let dir = TempDir::new().unwrap();
        let output_name = dir.path().join("jailed-echo").to_string_lossy().to_string();

        let config = MakeConfig {
            binary_path: PathBuf::from("/bin/echo"),
            output_name: output_name.clone(),
            analyze_only: false,
            permissive: false,
        };

        let result = cmd_make(config).unwrap();
        assert!(result.is_some());

        let output = result.unwrap();
        assert!(output.bridge_wasm_path.exists(), "bridge.wasm should exist");
        assert!(output.jailfile_path.exists(), "JailFile.toml should exist");
        assert!(output.launcher_path.exists(), "launcher should exist");
    }
}
