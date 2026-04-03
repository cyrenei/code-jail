//! Integration tests for `codejail make` — the native binary packaging command.
//!
//! These tests verify the end-to-end workflow:
//! 1. `codejail make <binary> -o <name>` generates artifacts
//! 2. The launcher script correctly invokes codejail
//! 3. The WASM bridge executes the native binary
//! 4. Exit codes propagate correctly

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use predicates::prelude::*;
use tempfile::TempDir;

fn codejail_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("codejail")
}

fn codejail_cmd() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("codejail").unwrap();
    let test_home = std::env::temp_dir().join("codejail-make-integration-test");
    fs::create_dir_all(&test_home).unwrap();
    cmd.env("CODEJAIL_HOME", &test_home);
    cmd
}

// ── codejail make — artifact generation ─────────────────────────

#[test]
fn test_make_generates_artifacts() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-true");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/true", "-o"])
        .arg(&output)
        .assert()
        .success()
        .stderr(predicate::str::contains("[codejail make] done"));

    // Verify all artifacts exist
    assert!(output.exists(), "launcher should exist");
    let staging = dir.path().join("jailed-true.d");
    assert!(staging.exists(), "staging dir should exist");
    assert!(staging.join("bridge.wasm").exists(), "bridge.wasm should exist");
    assert!(
        staging.join("JailFile.toml").exists(),
        "JailFile.toml should exist"
    );

    // Verify launcher is executable
    let meta = fs::metadata(&output).unwrap();
    assert!(
        meta.permissions().mode() & 0o111 != 0,
        "launcher should be executable"
    );
}

use std::os::unix::fs::PermissionsExt;

#[test]
fn test_make_analyze_only() {
    let dir = TempDir::new().unwrap();

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o", "test", "--analyze-only"])
        .assert()
        .success()
        .stderr(predicate::str::contains("analysis complete"));

    // No artifacts should be generated
    assert!(!dir.path().join("test").exists());
    assert!(!dir.path().join("test.d").exists());
}

#[test]
fn test_make_missing_binary() {
    let dir = TempDir::new().unwrap();

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/nonexistent/binary", "-o", "test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot resolve"));
}

#[test]
fn test_make_jailfile_is_valid_toml() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-echo");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o"])
        .arg(&output)
        .assert()
        .success();

    let jailfile = fs::read_to_string(dir.path().join("jailed-echo.d/JailFile.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(&jailfile).expect("JailFile should be valid TOML");

    assert!(parsed.get("sandbox").is_some());
    assert!(parsed.get("capabilities").is_some());
    assert!(parsed.get("limits").is_some());
}

#[test]
fn test_make_bridge_wasm_is_valid() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-echo");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o"])
        .arg(&output)
        .assert()
        .success();

    let wasm = fs::read(dir.path().join("jailed-echo.d/bridge.wasm")).unwrap();
    assert_eq!(&wasm[..4], b"\x00asm", "should have WASM magic");
    assert!(wasm.len() > 8, "should be non-trivial");
}

#[test]
fn test_make_permissive_flag() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-echo");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o"])
        .arg(&output)
        .args(["--permissive"])
        .assert()
        .success();

    let jailfile = fs::read_to_string(dir.path().join("jailed-echo.d/JailFile.toml")).unwrap();
    assert!(
        jailfile.contains("inherit_env = true"),
        "permissive should inherit env"
    );
    assert!(
        jailfile.contains("net_allow = [\"*\"]"),
        "permissive should allow all network"
    );
}

// ── codejail make + run — end-to-end ────────────────────────────

#[test]
fn test_make_and_run_echo() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-echo");

    // Step 1: make
    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o"])
        .arg(&output)
        .args(["--permissive"])
        .assert()
        .success();

    // Step 2: run the launcher
    let result = Command::new(&output)
        .arg("hello from jailed echo")
        .output()
        .expect("launcher should execute");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        stdout.contains("hello from jailed echo"),
        "native binary should produce output. stdout={stdout}, stderr={stderr}"
    );
}

#[test]
fn test_make_and_run_true() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-true");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/true", "-o"])
        .arg(&output)
        .args(["--permissive"])
        .assert()
        .success();

    let status = Command::new(&output)
        .status()
        .expect("launcher should execute");

    assert!(status.success(), "/bin/true should exit 0");
}

#[test]
fn test_make_and_run_false_propagates_exit_code() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-false");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/false", "-o"])
        .arg(&output)
        .args(["--permissive"])
        .assert()
        .success();

    let status = Command::new(&output)
        .status()
        .expect("launcher should execute");

    assert!(!status.success(), "/bin/false should exit non-zero");
}

#[test]
fn test_make_and_run_with_args() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("jailed-echo");

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o"])
        .arg(&output)
        .args(["--permissive"])
        .assert()
        .success();

    let result = Command::new(&output)
        .args(["arg1", "arg2", "arg3"])
        .output()
        .expect("launcher should execute");

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("arg1 arg2 arg3"),
        "args should pass through: {stdout}"
    );
}

// ── codejail run --native-exec — direct bridge invocation ────────

#[test]
fn test_run_native_exec_directly() {
    let dir = TempDir::new().unwrap();

    // First generate a bridge module
    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o", "test"])
        .assert()
        .success();

    let bridge = dir.path().join("test.d/bridge.wasm");
    let jailfile = dir.path().join("test.d/JailFile.toml");

    // Run directly with --native-exec
    codejail_cmd()
        .args([
            "run",
            "--native-exec",
            "/bin/echo",
            "--jailfile",
        ])
        .arg(&jailfile)
        .arg("--fuel")
        .arg("0")
        .arg("--timeout")
        .arg("0")
        .arg(&bridge)
        .arg("--")
        .arg("direct bridge test")
        .assert()
        .success()
        .stdout(predicate::str::contains("direct bridge test"));
}

// ── Analysis quality ─────────────────────────────────────────────

#[test]
fn test_make_detects_elf_binary() {
    let dir = TempDir::new().unwrap();

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/ls", "-o", "test", "--analyze-only"])
        .assert()
        .success()
        .stderr(predicate::str::contains("ELF"));
}

#[test]
fn test_make_detects_script() {
    // Find a script binary on the system
    let dir = TempDir::new().unwrap();

    // Create a simple test script
    let script_path = dir.path().join("test_script.sh");
    fs::write(&script_path, "#!/bin/sh\necho hello\n").unwrap();
    fs::set_permissions(
        &script_path,
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    codejail_cmd()
        .current_dir(dir.path())
        .args(["make"])
        .arg(&script_path)
        .args(["-o", "test", "--analyze-only"])
        .assert()
        .success()
        .stderr(predicate::str::contains("script"));
}
