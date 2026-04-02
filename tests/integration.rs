use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

use predicates::prelude::*;
use tempfile::TempDir;

static COMPILE_FIXTURES: Once = Once::new();

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn compile_fixtures() {
    COMPILE_FIXTURES.call_once(|| {
        let dir = fixtures_dir();
        let fixtures = [
            "hello",
            "fs_read",
            "fs_write",
            "env_test",
            "args_test",
            "fuel_burn",
            "escape_attempt",
        ];
        for name in fixtures {
            let src = dir.join(format!("{name}.rs"));
            let dst = dir.join(format!("{name}.wasm"));
            if dst.exists() {
                // Already compiled (e.g. from a previous test run)
                continue;
            }
            let status = Command::new("rustc")
                .args(["--target", "wasm32-wasip1", "--edition", "2021", "-o"])
                .arg(&dst)
                .arg(&src)
                .status()
                .unwrap_or_else(|e| panic!("rustc not found: {e}"));
            assert!(status.success(), "Failed to compile {}", src.display());
        }
    });
}

fn cask_cmd() -> assert_cmd::Command {
    compile_fixtures();
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    // Use isolated state directory for tests
    let test_home = std::env::temp_dir().join("cask-integration-test");
    std::fs::create_dir_all(&test_home).unwrap();
    cmd.env("CASK_HOME", &test_home);
    cmd
}

fn fixture(name: &str) -> String {
    fixtures_dir()
        .join(format!("{name}.wasm"))
        .to_string_lossy()
        .to_string()
}

// ── Basic execution ──────────────────────────────────────────────

#[test]
fn test_hello_world() {
    cask_cmd()
        .args(["run", &fixture("hello")])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello from WASM sandbox!"));
}

#[test]
fn test_info() {
    cask_cmd()
        .args(["info"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wasmtime"))
        .stdout(predicate::str::contains("deny-by-default"));
}

// ── Sandbox isolation ────────────────────────────────────────────

#[test]
fn test_escape_blocked() {
    cask_cmd()
        .args(["run", &fixture("escape_attempt")])
        .assert()
        .success()
        .stdout(predicate::str::contains("BLOCKED").count(5))
        .stdout(predicate::str::contains("ESCAPED").not());
}

#[test]
fn test_no_env_by_default() {
    // env_test expects SANDBOX_TEST to be set; without it, it exits 1
    cask_cmd()
        .args(["run", &fixture("env_test")])
        .assert()
        .failure()
        .stderr(predicate::str::contains("SANDBOX_TEST not set"));
}

#[test]
fn test_env_granted() {
    cask_cmd()
        .env("SANDBOX_TEST", "hello")
        .args(["run", &fixture("env_test"), "-e", "SANDBOX_TEST"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SANDBOX_TEST = hello"));
}

#[test]
fn test_env_set_value() {
    cask_cmd()
        .args(["run", &fixture("env_test"), "-e", "SANDBOX_TEST=from_flag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SANDBOX_TEST = from_flag"));
}

// ── Arguments ────────────────────────────────────────────────────

#[test]
fn test_args_passed() {
    cask_cmd()
        .args(["run", &fixture("args_test"), "--", "hello", "world"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First real arg: hello"));
}

#[test]
fn test_args_missing_fails() {
    cask_cmd()
        .args(["run", &fixture("args_test")])
        .assert()
        .failure();
}

// ── Filesystem capabilities ──────────────────────────────────────

#[test]
fn test_fs_read_denied_without_grant() {
    cask_cmd()
        .args(["run", &fixture("fs_read")])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot read /sandbox"));
}

#[test]
fn test_fs_read_granted() {
    let fixtures = fixtures_dir();
    cask_cmd()
        .args([
            "run",
            &fixture("fs_read"),
            "-v",
            &format!("{}:/sandbox", fixtures.display()),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Directory listing of /sandbox:"))
        .stdout(predicate::str::contains("hello.wasm"));
}

#[test]
fn test_fs_write() {
    let tmp = TempDir::new().unwrap();
    cask_cmd()
        .args([
            "run",
            &fixture("fs_write"),
            "-v",
            &format!("{}:/workspace", tmp.path().display()),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully wrote to /workspace/test_output.txt",
        ));

    // Verify the file was actually written on the host
    let content = std::fs::read_to_string(tmp.path().join("test_output.txt")).unwrap();
    assert_eq!(content, "Written from WASM sandbox!\n");
}

// ── Resource limits ──────────────────────────────────────────────

#[test]
fn test_fuel_limit_enforced() {
    cask_cmd()
        .args(["run", &fixture("fuel_burn"), "--fuel", "100000"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("CPU fuel limit exceeded"));
}

#[test]
fn test_fuel_sufficient() {
    cask_cmd()
        .args(["run", &fixture("hello"), "--fuel", "1000000000"])
        .assert()
        .success();
}

// ── Image management ─────────────────────────────────────────────

#[test]
fn test_import_and_run_by_name() {
    let test_home = TempDir::new().unwrap();
    compile_fixtures();

    // Import
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["import", "test-hello", &fixture("hello")])
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported: test-hello"));

    // List
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["images"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-hello"));

    // Run by name
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["run", "test-hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello from WASM sandbox!"));

    // Remove
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["rmi", "test-hello"])
        .assert()
        .success();
}

// ── Container management ─────────────────────────────────────────

#[test]
fn test_ps_and_prune() {
    let test_home = TempDir::new().unwrap();
    compile_fixtures();

    // Run a container
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["run", &fixture("hello")])
        .assert()
        .success();

    // List (shows with -a)
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["ps", "-a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Exited (0)"));

    // Prune
    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["prune"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed 1 stopped container(s)"));
}

// ── Inspect ──────────────────────────────────────────────────────

#[test]
fn test_inspect_module() {
    cask_cmd()
        .args(["inspect", &fixture("hello")])
        .assert()
        .success()
        .stdout(predicate::str::contains("_start"))
        .stdout(predicate::str::contains("wasi_snapshot_preview1"));
}

// ── Named containers ─────────────────────────────────────────────

#[test]
fn test_named_container() {
    let test_home = TempDir::new().unwrap();
    compile_fixtures();

    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["run", &fixture("hello"), "--name", "my-sandbox"])
        .assert()
        .success();

    let mut cmd = assert_cmd::Command::cargo_bin("cask").unwrap();
    cmd.env("CASK_HOME", test_home.path())
        .args(["ps", "-a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("my-sandbox"));
}

// ── Error cases ──────────────────────────────────────────────────

#[test]
fn test_missing_image() {
    cask_cmd()
        .args(["run", "nonexistent.wasm"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Image not found"));
}

#[test]
fn test_invalid_cap() {
    cask_cmd()
        .args(["run", &fixture("hello"), "--cap", "invalid:stuff"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid capability"));
}
