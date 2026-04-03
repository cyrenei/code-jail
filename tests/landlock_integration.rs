//! Integration tests proving Landlock filesystem enforcement in native bridge mode.
//!
//! These tests demonstrate that the Landlock LSM restricts native processes
//! to only the paths declared in their JailFile. This is the fix for the
//! original bug where `codejail make` produced sandboxes that logged mount
//! restrictions but never enforced them.

use std::fs;
use std::path::PathBuf;

use predicates::prelude::*;
use tempfile::TempDir;

fn codejail_cmd() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("codejail").unwrap();
    let test_home = std::env::temp_dir().join("codejail-landlock-test");
    fs::create_dir_all(&test_home).unwrap();
    cmd.env("CODEJAIL_HOME", &test_home);
    cmd
}

/// Run `codejail make` to generate bridge artifacts, return (bridge, jailfile) paths.
fn make_bridge(binary: &str, dir: &std::path::Path) -> (PathBuf, PathBuf) {
    codejail_cmd()
        .current_dir(dir)
        .args(["make", binary, "-o", "test"])
        .assert()
        .success();
    (
        dir.join("test.d/bridge.wasm"),
        dir.join("test.d/JailFile.toml"),
    )
}

/// Helper: run a jailed native binary via codejail run --native-exec.
fn jailed_run(binary: &str, bridge: &PathBuf, jailfile: &PathBuf) -> assert_cmd::Command {
    let mut cmd = codejail_cmd();
    cmd.args([
        "run",
        "--native-exec",
        binary,
        "--jailfile",
    ]);
    cmd.arg(jailfile);
    cmd.args(["--fuel", "0", "--timeout", "0"]);
    cmd.arg(bridge);
    cmd.arg("--");
    cmd
}

// ── Positive: allowed paths are accessible ──────────────────────

#[test]
fn test_landlock_allows_reading_tmp() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/cat", dir.path());

    // /tmp is in fs_write (and therefore readable), so this should work
    let allowed = std::env::temp_dir().join("codejail-ll-test-allowed.txt");
    fs::write(&allowed, "ACCESS GRANTED\n").unwrap();

    jailed_run("/bin/cat", &bridge, &jailfile)
        .arg(&allowed)
        .assert()
        .success()
        .stdout(predicate::str::contains("ACCESS GRANTED"));

    fs::remove_file(&allowed).ok();
}

#[test]
fn test_landlock_allows_listing_etc() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/ls", dir.path());

    // /etc is in the default fs_read mounts
    jailed_run("/bin/ls", &bridge, &jailfile)
        .arg("/etc/hostname")
        .assert()
        .success();
}

// ── Negative: unmounted paths are blocked ───────────────────────

#[test]
fn test_landlock_blocks_reading_outside_mounts() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/cat", dir.path());

    // /var/tmp is NOT in the default mount list
    let blocked = PathBuf::from("/var/tmp/codejail-ll-test-blocked.txt");
    fs::write(&blocked, "TOP SECRET\n").unwrap();

    let result = jailed_run("/bin/cat", &bridge, &jailfile)
        .arg(&blocked)
        .output()
        .expect("command should execute");

    // The cat process should fail — Landlock denies access
    assert!(
        !result.status.success(),
        "jailed cat should NOT be able to read /var/tmp (not in mounts)"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Permission denied") || stderr.contains("Operation not permitted"),
        "should get a permission error, got: {stderr}"
    );

    // Verify the secret did NOT leak to stdout
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        !stdout.contains("TOP SECRET"),
        "secret must not appear in output"
    );

    fs::remove_file(&blocked).ok();
}

#[test]
fn test_landlock_blocks_home_directory() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/ls", dir.path());

    // /home is NOT in the default mounts — this is the exact scenario
    // from the original bug where jailed Claude Code could see all of
    // /home/samantha including RSA keys and ML training data.
    let result = jailed_run("/bin/ls", &bridge, &jailfile)
        .arg("/home")
        .output()
        .expect("command should execute");

    assert!(
        !result.status.success(),
        "jailed ls must NOT be able to list /home"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Permission denied")
            || stderr.contains("Operation not permitted")
            || stderr.contains("cannot open"),
        "should get permission error for /home, got: {stderr}"
    );
}

#[test]
fn test_landlock_blocks_root_directory_listing() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/ls", dir.path());

    // / is not in the mounts — the jailed process cannot enumerate the root
    let result = jailed_run("/bin/ls", &bridge, &jailfile)
        .arg("/")
        .output()
        .expect("command should execute");

    assert!(
        !result.status.success(),
        "jailed ls must NOT be able to list /"
    );
}

// ── Write restrictions ──────────────────────────────────────────

#[test]
fn test_landlock_allows_writing_to_tmp() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/sh", dir.path());

    let outfile = std::env::temp_dir().join("codejail-ll-test-write.txt");

    // /tmp is in fs_write, so writing should succeed
    jailed_run("/bin/sh", &bridge, &jailfile)
        .args(["-c", &format!("echo WRITE_OK > {}", outfile.display())])
        .assert()
        .success();

    let content = fs::read_to_string(&outfile).unwrap_or_default();
    assert!(
        content.contains("WRITE_OK"),
        "jailed process should be able to write to /tmp"
    );

    fs::remove_file(&outfile).ok();
}

#[test]
fn test_landlock_blocks_writing_outside_mounts() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/sh", dir.path());

    // /var/tmp is NOT in the mount list — writes should fail
    let blocked = "/var/tmp/codejail-ll-test-write-blocked.txt";

    let result = jailed_run("/bin/sh", &bridge, &jailfile)
        .args(["-c", &format!("echo LEAKED > {blocked}")])
        .output()
        .expect("command should execute");

    assert!(
        !result.status.success(),
        "jailed sh should NOT be able to write to /var/tmp"
    );

    // Verify the file was NOT created
    assert!(
        !std::path::Path::new(blocked).exists(),
        "blocked write target must not exist"
    );
}

// ── Landlock enforcement metadata ───────────────────────────────

#[test]
fn test_landlock_enforcement_logged() {
    let dir = TempDir::new().unwrap();
    let (bridge, jailfile) = make_bridge("/bin/true", dir.path());

    // Verify the enforcement message appears in stderr
    jailed_run("/bin/true", &bridge, &jailfile)
        .assert()
        .success()
        .stderr(predicate::str::contains("landlock: enforcing"));
}

// ── Contrast: same binary, different mounts ─────────────────────

#[test]
fn test_landlock_same_binary_different_access() {
    let dir = TempDir::new().unwrap();

    // Create a file in /var/tmp
    let target = PathBuf::from("/var/tmp/codejail-ll-contrast.txt");
    fs::write(&target, "CONTRAST\n").unwrap();

    // Generate bridge artifacts
    codejail_cmd()
        .current_dir(dir.path())
        .args(["make", "/bin/cat", "-o", "test"])
        .assert()
        .success();

    let bridge = dir.path().join("test.d/bridge.wasm");
    let jailfile_path = dir.path().join("test.d/JailFile.toml");

    // Test 1: Default JailFile — /var/tmp is NOT mounted, access denied
    let result = jailed_run("/bin/cat", &bridge, &jailfile_path)
        .arg(&target)
        .output()
        .expect("should execute");
    assert!(
        !result.status.success(),
        "default mounts should block /var/tmp"
    );

    // Test 2: Add /var/tmp to JailFile — now access is granted
    let jailfile_content = fs::read_to_string(&jailfile_path).unwrap();
    let modified = jailfile_content.replace(
        "fs_read = [",
        &format!("fs_read = [\n    \"/var/tmp\","),
    );
    fs::write(&jailfile_path, &modified).unwrap();

    jailed_run("/bin/cat", &bridge, &jailfile_path)
        .arg(&target)
        .assert()
        .success()
        .stdout(predicate::str::contains("CONTRAST"));

    fs::remove_file(&target).ok();
}
