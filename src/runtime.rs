use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use wasmtime::*;
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::*;

use crate::capability::{Limits, ResolvedCaps};

pub struct SandboxRuntime {
    engine: Engine,
}

impl SandboxRuntime {
    pub fn new(fuel_enabled: bool) -> anyhow::Result<Self> {
        let mut config = Config::new();
        if fuel_enabled {
            config.consume_fuel(true);
        }
        // Limit memory if needed
        config.memory_guard_size(0);
        config.memory_reservation(0);
        let engine = Engine::new(&config)?;
        Ok(Self { engine })
    }

    pub fn run(
        &self,
        wasm_path: &Path,
        caps: &ResolvedCaps,
        limits: &Limits,
        args: &[String],
    ) -> anyhow::Result<()> {
        let mut builder = WasiCtxBuilder::new();

        // Allow blocking I/O in sync mode
        builder.allow_blocking_current_thread(true);

        // Stdio
        if caps.inherit_stdio {
            builder.inherit_stdio();
        }

        // Args: first arg is typically the program name
        let program_name = wasm_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut full_args = vec![program_name];
        full_args.extend(args.iter().cloned());
        builder.args(&full_args);

        // Environment variables
        for (k, v) in &caps.env_vars {
            builder.env(k, v);
        }

        // Filesystem mounts (preopened directories)
        for mount in &caps.fs_mounts {
            if !mount.host.exists() {
                anyhow::bail!("Mount source does not exist: {}", mount.host.display());
            }
            let dir_perms = if mount.writable {
                DirPerms::all()
            } else {
                DirPerms::READ
            };
            let file_perms = if mount.writable {
                FilePerms::all()
            } else {
                FilePerms::READ
            };
            builder.preopened_dir(&mount.host, &mount.guest, dir_perms, file_perms)?;
        }

        // Network capabilities
        if !caps.net_rules.is_empty() {
            if caps.net_rules.iter().any(|r| r == "*") {
                builder.inherit_network();
            } else {
                let rules = caps.net_rules.clone();
                builder.socket_addr_check(move |addr: SocketAddr, _use_type| {
                    let rules = rules.clone();
                    Box::pin(async move {
                        let ip = addr.ip().to_string();
                        let full = addr.to_string();
                        rules.iter().any(|r| full.contains(r) || ip == *r)
                    })
                });
                builder.allow_ip_name_lookup(true);
                builder.allow_tcp(true);
            }
        }

        // Build WASI preview 1 context
        let wasi_ctx = builder.build_p1();

        // Create store with fuel limits
        let mut store = Store::new(&self.engine, wasi_ctx);
        if let Some(fuel) = limits.fuel {
            store.set_fuel(fuel)?;
        }

        // Load module
        let module = Module::from_file(&self.engine, wasm_path)
            .map_err(|e| anyhow::anyhow!("Failed to load WASM module: {e}"))?;

        // Link WASI
        let mut linker = Linker::<WasiP1Ctx>::new(&self.engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |ctx| ctx)?;

        // Instantiate and run
        linker.module(&mut store, "", &module)?;

        let start = Instant::now();
        let wall_limit = limits
            .wall_time_secs
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(3600));

        // Get the default export (_start for WASI command modules)
        let func = linker
            .get_default(&mut store, "")
            .map_err(|e| anyhow::anyhow!("No _start function found: {e}"))?;

        let typed = func.typed::<(), ()>(&store)?;

        // Set up epoch-based interruption for wall-clock timeout
        // (fuel handles CPU; this handles wall clock)
        let engine = self.engine.clone();
        let timeout_handle = std::thread::spawn(move || {
            std::thread::sleep(wall_limit);
            engine.increment_epoch();
        });

        match typed.call(&mut store, ()) {
            Ok(()) => {}
            Err(e) => {
                // Check if it's a normal WASI exit
                if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
                    if exit.0 != 0 {
                        anyhow::bail!("Process exited with code {}", exit.0);
                    }
                    // Exit code 0 is success
                } else if e.to_string().contains("fuel") {
                    anyhow::bail!("CPU fuel limit exceeded");
                } else if start.elapsed() >= wall_limit {
                    anyhow::bail!("Wall-clock time limit exceeded ({wall_limit:?})");
                } else {
                    return Err(e.into());
                }
            }
        }

        drop(timeout_handle);

        // Report resource usage
        if let Ok(remaining) = store.get_fuel() {
            let used = limits.fuel.unwrap_or(0).saturating_sub(remaining);
            if used > 0 {
                eprintln!(
                    "[codejail] Fuel used: {} / {} ({:.1}%)",
                    used,
                    limits.fuel.unwrap_or(0),
                    (used as f64 / limits.fuel.unwrap_or(1) as f64) * 100.0
                );
            }
        }
        eprintln!(
            "[codejail] Wall time: {:.2}s",
            start.elapsed().as_secs_f64()
        );

        Ok(())
    }
}

/// Inspect a WASM module without running it
pub fn inspect_module(wasm_path: &Path) -> anyhow::Result<ModuleInfo> {
    let engine = Engine::default();
    let module = Module::from_file(&engine, wasm_path)?;

    let exports: Vec<String> = module
        .exports()
        .map(|e| format!("{} ({:?})", e.name(), e.ty()))
        .collect();

    let imports: Vec<String> = module
        .imports()
        .map(|i| format!("{}::{} ({:?})", i.module(), i.name(), i.ty()))
        .collect();

    let size = std::fs::metadata(wasm_path)?.len();

    Ok(ModuleInfo {
        exports,
        imports,
        size,
    })
}

pub struct ModuleInfo {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub size: u64,
}
