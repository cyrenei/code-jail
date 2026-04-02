Security model
==============

Containment exists to run untrusted code safely. This page describes what it protects against, what it does not, and how the protection works.

Threat model
------------

Containment is designed for this scenario: you have a program you do not fully trust, and you want to run it without giving it access to your system. Examples:

- An AI coding agent that might read your SSH keys
- A build script from an unfamiliar project
- A tool you downloaded that you have not audited

The sandbox prevents the program from accessing anything you did not explicitly grant.

What the sandbox blocks
-----------------------

Without capability grants, a sandboxed program **cannot**:

- Read or write any file on the host filesystem
- Open network connections
- See environment variables (not even HOME or PATH)
- Access /proc, /sys, or /dev
- Discover other running processes
- Read the hostname or system clock (unless WASI clock access is enabled)
- Spawn subprocesses
- Modify the host system in any way

All of these return errors inside the sandbox, typically "No such file or directory" (WASI errno 44).

How isolation works
-------------------

**WASM memory isolation.** Each WASM module runs in its own linear memory space. It cannot read or write memory outside its allocation. This is enforced by the wasmtime runtime at the hardware level (using guard pages and bounds checking).

**WASI capability model.** WASM modules interact with the outside world through imported WASI functions (like ``fd_read``, ``fd_write``, ``sock_open``). These functions only operate on resources the host has explicitly provided. If you do not preopened a directory, the module has no file descriptor for it and cannot access it.

**No ambient authority.** Unlike a regular process on Linux, a WASM module does not inherit any capabilities from its parent. There is no implicit access to the filesystem, no implicit network access, no implicit environment. Everything starts empty.

Defense in depth with bubblewrap
--------------------------------

The ``--bwrap`` flag adds a second layer: Linux namespace isolation around the wasmtime process itself. Even if a bug in wasmtime allowed a WASM module to escape the sandbox, the process would still be inside a namespace jail with:

- No network access (unless ``--net`` was also granted)
- No visibility into host processes
- Read-only access to system libraries
- Its own /tmp and /dev

This is the same technology used by Flatpak for desktop application sandboxing.

What the sandbox does NOT protect against
------------------------------------------

**CPU exhaustion.** A program can burn CPU until its fuel runs out or the timeout fires. It cannot be truly preempted mid-instruction. The fuel limit and wall-clock timeout are the mitigations.

**Memory exhaustion.** A WASM module can allocate up to 4 GB of linear memory (the 32-bit WASM limit). This could cause the host to swap. Memory limits are a planned feature.

**Wasmtime bugs.** If there is a sandbox escape vulnerability in wasmtime itself, a malicious module could break out. This is what ``--bwrap`` mitigates. The wasmtime team takes security seriously and runs extensive fuzzing, but no software is bug-free.

**Side channels.** Timing-based side channels and other covert channels are not addressed. If this matters for your use case, you need hardware-level isolation (VMs).

**Granted capabilities.** If you grant ``--cap net:*``, the program can phone home. If you grant ``--cap fs:write:/``, it can delete your files. The sandbox only restricts what you do not grant.

Comparison with other isolation tools
-------------------------------------

.. list-table::
   :header-rows: 1

   * - Tool
     - Isolation mechanism
     - Compatibility
     - Overhead
   * - **containment (WASM)**
     - Capability-based (deny by default)
     - Programs must target wasm32-wasip1
     - ~1.5x native
   * - **Docker**
     - Linux namespaces + cgroups
     - Any Linux binary
     - ~1x native
   * - **gVisor**
     - Userspace kernel
     - Any Linux binary
     - ~1.2x native (syscall overhead)
   * - **Firecracker**
     - Hardware VM
     - Any Linux binary
     - ~1x native (startup cost)
   * - **bubblewrap**
     - Linux namespaces
     - Any Linux binary
     - ~1x native

Containment trades compatibility (programs must be compiled to WASM) for a stronger default security posture (deny by default, capability-based). For programs that can target WASM, this is a good trade.

Reporting vulnerabilities
-------------------------

If you find a security issue in containment, please open an issue on GitHub. If the issue is sensitive (sandbox escape), email the maintainers instead of posting publicly.
