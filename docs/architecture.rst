Architecture
============

This page describes how containment is built. It is written for contributors and anyone who wants to understand what happens when you type ``containment run``.

Overview
--------

Containment is a thin CLI layer on top of wasmtime with an integrated policy enforcement layer (arbiter). The binary is about 30 MB (release build) because it statically links the wasmtime JIT compiler and the arbiter policy engine. There are no other runtime dependencies.

::

   User
    |
    v
   CLI (clap)        Parses flags, dispatches to command handlers
    |
    v
   Capability        Resolves --cap flags, volumes, env vars, Containmentfile
   resolver          into a unified set of capability grants
    |
    v
   Arbiter gate      Evaluates each grant against policy (deny-by-default).
   (if enabled)      Drift detection. Audit logging. Session tracking.
                     Only authorized grants pass through.
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

Without arbiter (simple mode), the capability resolver feeds directly into the runtime with no policy evaluation step. This is not recommended for production use.

Source layout
-------------

.. code-block:: text

   src/
     main.rs          CLI with clap. Each subcommand is a function.
     arbiter.rs       Arbiter MCP Firewall integration. Policy evaluation,
                      drift detection, audit logging for each cap grant.
     capability.rs    Containmentfile parsing, CapGrant enum, ResolvedCaps.
     runtime.rs       SandboxRuntime: builds WASI context, runs modules.
     container.rs     Container struct, ContainerStore (JSON files).
     image.rs         ImageStore: import, list, resolve, remove.
     sandbox.rs       Bubblewrap outer sandbox (optional).

The run command in detail
-------------------------

Here is what happens during ``containment run agent.wasm --arbiter policy.toml --intent "read" --cap fs:read:/project --cap net:*``:

1. **Image resolution.** The CLI checks if ``agent.wasm`` is a file path. If not, it looks in ``~/.containment/images/``. If still not found, it errors out.

2. **Capability parsing.** Each ``--cap`` string is parsed into a ``CapGrant`` variant: ``Fs(FsMount)``, ``Net(String)``, or ``Env(Vec<String>)``. Volume mounts (``-v``) and env flags (``-e``) are also converted to grants.

3. **Arbiter policy evaluation (if --arbiter).** The arbiter gate loads the policy file and initializes the enforcement pipeline:

   a. **Agent registration.** The WASM image is registered as an arbiter agent.
   b. **Session creation.** A new task session is created with call budget and time limit.
   c. **Per-grant evaluation.** Each capability grant is converted to an MCP tool call and evaluated against the deny-by-default policy engine. Only grants with a matching ``allow`` policy pass through.
   d. **Drift detection.** Each grant's operation type (read, write, admin) is compared against the declared ``--intent``. Mismatches are flagged as behavioral anomalies.
   e. **Audit logging.** Every decision (allowed, denied, drift-flagged) is written to the audit log as structured JSONL.
   f. **Authorized capabilities.** Only the grants that survived policy evaluation become the ``ResolvedCaps`` for the runtime.

   Without ``--arbiter``, this step is skipped entirely. All parsed grants become capabilities directly — there is no evaluation, no drift detection, no audit trail.

4. **Capability resolution.** The authorized grants (from arbiter) or all grants (simple mode) are resolved into a ``ResolvedCaps`` with three lists: filesystem mounts, network rules, and environment variables.

5. **Engine creation.** A wasmtime ``Engine`` is created with fuel consumption enabled (unless ``--fuel 0``).

6. **WASI context.** A ``WasiCtxBuilder`` is configured:

   - Each filesystem mount becomes a preopened directory with the appropriate ``DirPerms`` and ``FilePerms``.
   - Network rules are installed as a ``socket_addr_check`` callback.
   - Environment variables are injected.
   - Stdio is connected to the terminal.

7. **Store creation.** A wasmtime ``Store`` wraps the WASI context. Fuel is set on the store.

8. **Module loading.** The ``.wasm`` file is compiled by wasmtime's Cranelift JIT into native code.

9. **Linking.** WASI preview 1 functions are linked into the module. If the module imports something that WASI does not provide, this step fails.

10. **Execution.** The ``_start`` function is called. This is the standard WASI entry point for command-line programs.

11. **Completion.** On success or failure, a container record is written to ``~/.containment/containers/``. The exit status, timing, and fuel usage are reported to stderr.

The arbiter gate
----------------

When ``--arbiter policy.toml`` is passed (or ``ARBITER_POLICY`` is set), the arbiter gate (``src/arbiter.rs``) mediates between capability requests and the runtime. It uses several crates from the ``arbiter-mcp-firewall/`` subdirectory:

- **arbiter-policy**: Deny-by-default policy engine. Evaluates each grant against TOML rules with specificity-based ordering.
- **arbiter-behavior**: Drift detection. Classifies operations into intent tiers (read/write/admin) and flags mismatches.
- **arbiter-session**: Session management. Tracks call budgets, time limits, and session IDs.
- **arbiter-audit**: Structured JSONL logging with automatic argument redaction.
- **arbiter-identity**: Agent registration with trust levels.

This creates the gap between requesting a capability and receiving it. The gap is where every security guarantee lives: policy enforcement, intent verification, session budgets, and audit trails.

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
