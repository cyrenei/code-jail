Architecture
============

This page describes how containment is built. It is written for contributors and anyone who wants to understand what happens when you type ``containment run``.

Overview
--------

Containment is a thin CLI layer on top of wasmtime. The binary is about 30 MB (release build) because it statically links the wasmtime JIT compiler. There are no other runtime dependencies.

::

   User
    |
    v
   CLI (clap)        Parses flags, dispatches to command handlers
    |
    v
   Capability        Resolves --cap flags, volumes, env vars, Containmentfile
   resolver          into a unified ResolvedCaps
    |
    v
   Runtime           Configures wasmtime: Engine, Store, WASI context,
   (wasmtime)        fuel limits, preopened dirs, socket checks
    |
    v
   WASM module       Loaded, linked against WASI, _start called
    |
    v
   Container         Records run metadata (ID, status, image, time)
   store

Source layout
-------------

.. code-block:: text

   src/
     main.rs          CLI with clap. Each subcommand is a function.
     capability.rs    Containmentfile parsing, CapGrant enum, ResolvedCaps.
     runtime.rs       SandboxRuntime: builds WASI context, runs modules.
     container.rs     Container struct, ContainerStore (JSON files).
     image.rs         ImageStore: import, list, resolve, remove.
     sandbox.rs       Bubblewrap outer sandbox (optional).

The run command in detail
-------------------------

Here is what happens during ``containment run agent.wasm --cap fs:read:/project --cap net:*``:

1. **Image resolution.** The CLI checks if ``agent.wasm`` is a file path. If not, it looks in ``~/.containment/images/``. If still not found, it errors out.

2. **Capability parsing.** Each ``--cap`` string is parsed into a ``CapGrant`` variant: ``Fs(FsMount)``, ``Net(String)``, or ``Env(Vec<String>)``. Volume mounts (``-v``) and env flags (``-e``) are also converted to grants.

3. **Capability resolution.** If a Containmentfile is loaded (``-f``), its capabilities are the base. CLI grants are merged on top. The result is a ``ResolvedCaps`` with three lists: filesystem mounts, network rules, and environment variables.

4. **Engine creation.** A wasmtime ``Engine`` is created with fuel consumption enabled (unless ``--fuel 0``).

5. **WASI context.** A ``WasiCtxBuilder`` is configured:

   - Each filesystem mount becomes a preopened directory with the appropriate ``DirPerms`` and ``FilePerms``.
   - Network rules are installed as a ``socket_addr_check`` callback.
   - Environment variables are injected.
   - Stdio is connected to the terminal.

6. **Store creation.** A wasmtime ``Store`` wraps the WASI context. Fuel is set on the store.

7. **Module loading.** The ``.wasm`` file is compiled by wasmtime's Cranelift JIT into native code.

8. **Linking.** WASI preview 1 functions are linked into the module. If the module imports something that WASI does not provide, this step fails.

9. **Execution.** The ``_start`` function is called. This is the standard WASI entry point for command-line programs.

10. **Completion.** On success or failure, a container record is written to ``~/.containment/containers/``. The exit status, timing, and fuel usage are reported to stderr.

Data directory
--------------

Containment stores state in ``~/.containment/`` (or ``$CONTAINMENT_HOME`` if set):

.. code-block:: text

   ~/.containment/
     images/         Imported .wasm files
     containers/     JSON records of past runs

Container records are JSON files named by UUID. They are lightweight and can be cleaned up with ``containment prune``.

WASI preview 1
--------------

Containment targets WASI preview 1 (``wasm32-wasip1``). This is the stable, widely supported version. Programs compiled with ``rustc --target wasm32-wasip1`` or equivalent work out of the box.

WASI preview 2 (the component model) is not yet supported. It would bring typed interfaces between modules and better composability, but the toolchain support is still maturing.

Why wasmtime?
-------------

Wasmtime is the reference implementation for WASI, maintained by the Bytecode Alliance. It has:

- Best-in-class security (fuzzing, sandboxing, formal verification work)
- Cranelift JIT (fast compilation, good runtime performance)
- Complete WASI preview 1 support
- Rust-native API (containment is also Rust, so the integration is straightforward)
- Fuel metering for CPU limits
- Epoch-based interruption for timeouts

The optional bubblewrap layer
-----------------------------

When you pass ``--bwrap``, containment wraps the wasmtime process in a Linux namespace sandbox using bubblewrap. This provides:

- PID namespace isolation (the sandboxed process cannot see host processes)
- Network namespace isolation (no network access even if wasmtime has a bug)
- IPC namespace isolation
- Separate /tmp, /dev, /proc

This is defense in depth. The WASM sandbox should be sufficient on its own, but if there is ever a wasmtime escape vulnerability, the bubblewrap layer is a second barrier.
