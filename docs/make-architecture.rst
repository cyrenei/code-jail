``codejail make`` — Architecture
=================================

The WASM Supervisor Pattern
---------------------------

``codejail make`` packages native binaries into WASM-supervised sandboxes.
The key architectural insight: **the .wasm module is the control plane,
not the execution target.**

Native binaries cannot be compiled to WASM. x86_64 ELF and WASM are
different instruction set architectures. There is no general-purpose
translator. This constraint is fundamental and irreducible.

Instead, ``codejail make`` uses the **WASM Supervisor Pattern**:

.. code-block:: text

   codejail make /path/to/binary -o jailed-binary

   ┌──────────────────────────────────────────────────┐
   │               codejail make                      │
   │                                                  │
   │  1. Analyze binary (ELF/script detection, ldd)   │
   │  2. Generate bridge.wasm (WASM supervisor)       │
   │  3. Generate JailFile.toml (capability manifest) │
   │  4. Generate launcher script                     │
   └──────────────────────────────────────────────────┘
                        │
                        ▼
   ┌─────────────────────────────────────────────────────┐
   │              ./jailed-binary                        │
   │                                                     │
   │  launcher script                                    │
   │    └─ codejail run --native-exec <binary>           │
   │         └─ NativeBridgeRuntime                      │
   │              ├─ loads bridge.wasm                    │
   │              ├─ provides codejail_host.exec          │
   │              └─ bridge.wasm calls exec               │
   │                   └─ fork + exec native binary      │
   │                        └─ inherits terminal (PTY)   │
   └─────────────────────────────────────────────────────┘

The .wasm file contains exactly three elements:

1. An import of ``codejail_host.exec`` (the bridge to native execution)
2. An import of ``wasi_snapshot_preview1.proc_exit`` (clean shutdown)
3. A ``_start`` function that calls exec then proc_exit

This is the minimal possible WASM module. All intelligence lives in the
runtime (capability enforcement) and the JailFile (capability specification).

Why WASM At All?
~~~~~~~~~~~~~~~~

If the binary runs natively, why involve WASM?

1. **Architectural consistency**: Every codejail execution goes through a
   WASM module. The module IS the jail. Without it, native execution would
   bypass the entire WASM-based policy model.

2. **Policy evaluation point**: The WASM module is where the runtime decides
   whether to allow execution. The bridge module currently calls exec
   unconditionally, but it can be extended to check conditions, enforce
   time-of-day restrictions, or implement approval workflows.

3. **Audit trail**: Loading and executing a WASM module creates a container
   record, which provides the same observability as standard WASM execution.

4. **Future extensibility**: The bridge module can be enhanced to:
   - Intercept and filter syscalls via seccomp-bpf
   - Implement capability-based access control at the WASI level
   - Add pre/post-execution hooks
   - Support component model (WASI preview 2) when available

Tensions (Preserved, Not Resolved)
-----------------------------------

These tensions are structural — they exist because the problem has
genuinely competing requirements. They are documented here as search
drivers for future development, not as bugs to fix.

T-001: Security vs. Functionality
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The more capabilities granted (filesystem, network, env), the less
meaningful the sandbox. For Claude Code to show its welcome screen,
it needs broad capabilities. The POC uses ``--permissive`` to grant
everything, which makes the sandbox permeable.

**Current resolution**: Functionality wins for POC. The JailFile is
generated with inferred capabilities, not least-privilege. Users
should review and tighten before production use.

**Future path**: Capability profiling (run the binary, observe what
it actually accesses, generate a minimal JailFile from traces).

T-002: WASM Mediation vs. Performance
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The WASM bridge adds measurable overhead:

- ~20-100ms for wasmtime JIT compilation of the bridge module
- ~0ms for the exec host function itself (just fork+exec)
- Total overhead: <100ms for first invocation

**Current resolution**: The bridge module is tiny (3 functions), so
JIT compilation is fast. The exec host function is zero-copy
(inherits stdio directly). For interactive applications, <100ms
startup overhead is imperceptible.

**Future path**: AOT-compile the bridge module. Cache compiled modules.

T-003: Auto-Inference vs. Least Privilege
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Binary analysis reveals what the binary **needs** (linked libraries,
network libraries, etc.). But good security grants what it **should
have**, which is a strict subset.

Auto-generated JailFiles are permissive by design because:

- ``ldd`` output tells us library paths, not which operations use them
- Network library presence doesn't tell us which hosts are contacted
- Environment variable inference is keyword-based, not semantic

**Current resolution**: Generate permissive defaults, document the gap,
provide ``--analyze-only`` for manual refinement.

**Future path**: eBPF-based runtime profiling to observe actual syscalls
and generate tight capability sets from traces.

T-004: Self-Contained Launcher vs. Dependencies
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The launcher script requires ``codejail`` to be installed and in PATH
(or at the path where it was at ``make`` time). This is not truly
self-contained.

**Current resolution**: The launcher embeds the absolute path to the
codejail binary found at make time. This works on the same machine
but is not portable.

**Future path**: Static linking of wasmtime into the launcher. Produce
a single binary that contains the WASM runtime, bridge module, JailFile,
and native binary path — no external dependencies.

T-005: Portability vs. Enforcement
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The .wasm bridge module is portable (runs on any wasmtime). The native
binary is not (x86_64 ELF is Linux-specific). OS-level sandboxing
(namespaces, seccomp, Landlock) is Linux-only.

**Current resolution**: Linux-only for POC. The architecture cleanly
separates portable policy (.wasm + JailFile) from non-portable
enforcement (native execution + OS sandboxing).

**Future path**: Platform-specific enforcement backends (macOS sandbox,
Windows AppContainer) with shared .wasm + JailFile specification.

Components
----------

Binary Analyzer (``src/analyzer.rs``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Inspects the target binary to determine:

- **Type**: ELF, script (shebang), symlink, unknown
- **Interpreter**: ELF program interpreter or script interpreter
- **Libraries**: Dynamically linked shared libraries (via ``ldd``)
- **Inferred capabilities**: Filesystem paths, network requirements,
  environment variables needed for execution

The analyzer uses heuristics for capability inference:

- Node.js scripts → needs network, node_modules, broad env
- Python scripts → needs PYTHONPATH, virtual environments
- ELF with libssl/libcurl → likely needs network
- All binaries → need /dev, /proc/self, /etc, /tmp

WASM Bridge Generator (``src/make.rs``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Generates the minimal WASM supervisor module from WAT (WebAssembly Text):

.. code-block:: wat

   (module
     (import "codejail_host" "exec" (func $exec (result i32)))
     (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
     (memory (export "memory") 1)
     (func (export "_start")
       call $exec
       call $proc_exit
       unreachable))

The WAT is compiled to WASM binary using the ``wat`` crate at make time.

Native Bridge Runtime (``src/native_bridge.rs``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Provides the ``codejail_host.exec`` host function:

1. Creates a ``NativeBridgeState`` (WasiP1Ctx + NativeExecConfig)
2. Registers the ``exec`` host function with the wasmtime linker
3. The host function:
   - Builds a ``Command`` from the config (binary path, args, env)
   - Inherits stdio from the codejail process (terminal passthrough)
   - Waits for the child process
   - Returns the exit code

Terminal passthrough works because the child process inherits the
parent's file descriptors. Since codejail runs in the user's terminal,
the native binary gets the same terminal — ``isatty()`` returns true,
terminal control codes work, signals propagate via the kernel.

JailFile Generator (``src/make.rs``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Generates a JailFile.toml with auto-inferred capabilities:

.. code-block:: toml

   [sandbox]
   name = "jailed-binary"
   entrypoint = "bridge.wasm"

   [capabilities]
   fs_read = ["/binary/path", "/usr/lib", ...]
   fs_write = ["/tmp"]
   net_allow = []  # or ["*"] with --permissive
   env = ["PATH", "HOME", "TERM", ...]
   inherit_env = false  # or true with --permissive
   stdin = true
   stdout = true
   stderr = true

   [limits]
   fuel = 0          # no CPU metering for native exec
   wall_time_secs = 0  # no timeout for interactive apps

Launcher Generator (``src/make.rs``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Generates a shell script that:

1. Verifies the staging directory exists
2. Invokes ``codejail run --native-exec <binary> --jailfile <path> bridge.wasm``
3. Passes through all arguments (``"$@"``)

The launcher embeds absolute paths to the codejail binary and staging
directory. It is executable (mode 755) and self-documenting (contains
comments with the binary path, type, and staging location).
