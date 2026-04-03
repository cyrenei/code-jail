//! Performance benchmarks for `codejail make` and native bridge execution.
//!
//! Measures:
//! - make command execution time (artifact generation)
//! - Bridge overhead (WASM → native exec latency)
//! - Launcher startup time vs direct execution

use std::fs;
use std::process::Command;
use std::time::Instant;

use tempfile::TempDir;

fn codejail_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("codejail")
}

fn mean_and_stddev(samples: &[f64]) -> (f64, f64) {
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<f64>() / n;
    let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

// ── Make command performance ────────────────────────────────────

#[test]
fn bench_make_command_time() {
    let mut times = Vec::new();
    let iterations = 5;

    for _ in 0..iterations {
        let dir = TempDir::new().unwrap();
        let start = Instant::now();

        let status = Command::new(codejail_bin())
            .current_dir(dir.path())
            .args(["make", "/bin/echo", "-o", "bench-echo"])
            .env(
                "CODEJAIL_HOME",
                std::env::temp_dir().join("codejail-bench"),
            )
            .output()
            .unwrap();

        let elapsed = start.elapsed();
        assert!(status.status.success());
        times.push(elapsed.as_secs_f64() * 1000.0); // ms
    }

    let (mean, stddev) = mean_and_stddev(&times);
    eprintln!();
    eprintln!("=== codejail make /bin/echo ===");
    eprintln!("  iterations: {iterations}");
    eprintln!("  mean:   {mean:.1} ms");
    eprintln!("  stddev: {stddev:.1} ms");
    eprintln!("  min:    {:.1} ms", times.iter().cloned().reduce(f64::min).unwrap());
    eprintln!("  max:    {:.1} ms", times.iter().cloned().reduce(f64::max).unwrap());

    // Make should complete within 500ms
    assert!(
        mean < 500.0,
        "make command should be fast (mean={mean:.1}ms)"
    );
}

// ── Bridge overhead ─────────────────────────────────────────────

#[test]
fn bench_bridge_overhead() {
    let dir = TempDir::new().unwrap();

    // Setup: generate the artifacts
    let status = Command::new(codejail_bin())
        .current_dir(dir.path())
        .args(["make", "/bin/true", "-o", "bench-true", "--permissive"])
        .env(
            "CODEJAIL_HOME",
            std::env::temp_dir().join("codejail-bench-overhead"),
        )
        .output()
        .unwrap();
    assert!(status.status.success());

    let launcher = dir.path().join("bench-true");

    // Measure bridge overhead: time to run /bin/true through the bridge
    let mut bridge_times = Vec::new();
    let iterations = 10;

    for _ in 0..iterations {
        let start = Instant::now();
        let status = Command::new(&launcher).output().unwrap();
        let elapsed = start.elapsed();
        assert!(status.status.success());
        bridge_times.push(elapsed.as_secs_f64() * 1000.0);
    }

    // Measure direct execution: time to run /bin/true directly
    let mut direct_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let status = Command::new("/bin/true").output().unwrap();
        let elapsed = start.elapsed();
        assert!(status.status.success());
        direct_times.push(elapsed.as_secs_f64() * 1000.0);
    }

    let (bridge_mean, bridge_std) = mean_and_stddev(&bridge_times);
    let (direct_mean, direct_std) = mean_and_stddev(&direct_times);
    let overhead = bridge_mean - direct_mean;

    eprintln!();
    eprintln!("=== Bridge overhead (/bin/true) ===");
    eprintln!("  iterations: {iterations}");
    eprintln!("  direct:  {direct_mean:.1} +/- {direct_std:.1} ms");
    eprintln!("  bridged: {bridge_mean:.1} +/- {bridge_std:.1} ms");
    eprintln!("  overhead: {overhead:.1} ms ({:.0}x)", bridge_mean / direct_mean.max(0.01));
    eprintln!(
        "  min bridge: {:.1} ms",
        bridge_times.iter().cloned().reduce(f64::min).unwrap()
    );
    eprintln!(
        "  max bridge: {:.1} ms",
        bridge_times.iter().cloned().reduce(f64::max).unwrap()
    );

    // Bridge overhead should be under 500ms (wasmtime JIT compilation)
    assert!(
        overhead < 500.0,
        "bridge overhead should be reasonable ({overhead:.1}ms)"
    );
}

// ── Argument passing overhead ───────────────────────────────────

#[test]
fn bench_arg_passing() {
    let dir = TempDir::new().unwrap();

    let status = Command::new(codejail_bin())
        .current_dir(dir.path())
        .args(["make", "/bin/echo", "-o", "bench-echo", "--permissive"])
        .env(
            "CODEJAIL_HOME",
            std::env::temp_dir().join("codejail-bench-args"),
        )
        .output()
        .unwrap();
    assert!(status.status.success());

    let launcher = dir.path().join("bench-echo");

    // Run with varying argument counts
    for arg_count in [0, 1, 10, 100] {
        let args: Vec<String> = (0..arg_count).map(|i| format!("arg{i}")).collect();

        let mut times = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let status = Command::new(&launcher)
                .args(&args)
                .output()
                .unwrap();
            let elapsed = start.elapsed();
            assert!(status.status.success());
            times.push(elapsed.as_secs_f64() * 1000.0);
        }

        let (mean, stddev) = mean_and_stddev(&times);
        eprintln!();
        eprintln!("  echo with {arg_count} args: {mean:.1} +/- {stddev:.1} ms");
    }
}
