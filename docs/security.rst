Security model
==============

Codejail exists to run untrusted code safely. This page describes the three layers of defense, what they protect against, what they do not, and how they work together.

Threat model
------------

Codejail is designed for this scenario: you have a program you do not fully trust, and you want to run it without giving it access to your system. Examples:

- An AI coding agent that might read your SSH keys
- A build script from an unfamiliar project
- A tool you downloaded that you have not audited

The sandbox prevents the program from accessing anything you did not explicitly grant. The policy engine ensures that even explicit grants are evaluated against rules before taking effect.

Three layers of defense
-----------------------

**Layer 1: WASM capability isolation (always on).** The cell. Programs run as WebAssembly modules inside wasmtime with no ambient authority. They cannot access any resource the host did not explicitly provide.

**Layer 2: Policy enforcement (recommended).** The guard. Every capability grant is evaluated against a deny-by-default policy before the sandbox starts. The policy engine checks agent identity, intent, tool name, and parameter constraints. Drift detection flags when capabilities diverge from declared intent. All decisions are audit-logged.

**Layer 3: Linux namespace isolation (opt-in with --bwrap).** The outer wall. Wraps the entire wasmtime process in a bubblewrap sandbox with unshared namespaces (PID, network, IPC, UTS, cgroup). Defense in depth against wasmtime runtime bugs.

Layer 1 keeps the code in. Layer 2 controls what crosses the boundary. Layer 3 is a backup in case Layer 1 has a bug.

What each layer blocks
----------------------

**Layer 1 (WASM isolation) blocks:**

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

**Layer 2 (policy enforcement) blocks:**

With a policy active, even explicitly requested capabilities can be denied:

- A ``--cap fs:write:/path`` grant with no matching allow policy is **denied by default**
- A write capability in a read-intent session is **flagged as drift**
- Parameter values outside policy-defined bounds are **rejected**
- Calls exceeding session budget are **blocked**
- Sessions past their time limit are **expired**
- Agent trust levels that do not meet policy requirements are **denied**

Without a policy, none of these checks exist. The operator's flags are granted unconditionally.

**Layer 3 (bubblewrap) blocks:**

Even if wasmtime has a sandbox escape vulnerability:

- No network access (unless ``--net`` was also granted)
- No visibility into host processes
- Read-only access to system libraries
- Separate /tmp and /dev

How isolation works
-------------------

**WASM memory isolation.** Each WASM module runs in its own linear memory space. It cannot read or write memory outside its allocation. This is enforced by the wasmtime runtime at the hardware level (using guard pages and bounds checking).

**WASI capability model.** WASM modules interact with the outside world through imported WASI functions (like ``fd_read``, ``fd_write``, ``sock_open``). These functions only operate on resources the host has explicitly provided. If you do not preopened a directory, the module has no file descriptor for it and cannot access it.

**No ambient authority.** Unlike a regular process on Linux, a WASM module does not inherit any capabilities from its parent. There is no implicit access to the filesystem, no implicit network access, no implicit environment. Everything starts empty.

**Policy mediation.** When ``--policy`` is active, the policy gate sits between capability requests and the WASI context builder. Each grant is evaluated against policy rules with specificity-based ordering. Deny-by-default means no matching policy results in denial. Drift detection compares each grant's operation type against the declared session intent. The audit log records every decision with full context: timestamp, tool name, arguments, matched policy, agent ID, session ID.

This creates a structural separation between requesting and receiving a capability. Without a policy, there is no such separation -- the operator is simultaneously the requester and the approver.

The trust model
---------------

**With policy enforcement (recommended):**

- The policy file is the authorization authority
- The operator requests capabilities; the policy decides
- Drift detection independently monitors intent-action alignment
- All decisions are recorded for auditing
- Assumption: the policy file is reviewed and version-controlled

**Without policy enforcement (simple mode, not recommended):**

- The operator is the authorization authority
- The operator requests capabilities and grants them to themselves
- No independent monitoring of intent-action alignment
- No structured decision log
- Assumption: the operator is trusted and makes good decisions every time

What the sandbox does NOT protect against
------------------------------------------

**CPU exhaustion.** A program can burn CPU until its fuel runs out or the timeout fires. It cannot be truly preempted mid-instruction. The fuel limit and wall-clock timeout are the mitigations.

**Memory exhaustion.** A WASM module can allocate up to 4 GB of linear memory (the 32-bit WASM limit). This could cause the host to swap. Memory limits are a planned feature.

**Wasmtime bugs.** If there is a sandbox escape vulnerability in wasmtime itself, a malicious module could break out. This is what ``--bwrap`` mitigates. The wasmtime team takes security seriously and runs extensive fuzzing, but no software is bug-free.

**Side channels.** Timing-based side channels and other covert channels are not addressed. If this matters for your use case, you need hardware-level isolation (VMs).

**Granted capabilities.** If you grant ``--cap net:*``, the program can phone home. If you grant ``--cap fs:write:/``, it can delete your files. The sandbox only restricts what you do not grant. Policy enforcement can prevent overly broad grants from being approved, but only if the policy rules cover those cases.

Comparison with other isolation tools
-------------------------------------

.. list-table::
   :header-rows: 1

   * - Tool
     - Isolation mechanism
     - Policy enforcement
     - Compatibility
     - Overhead
   * - **codejail + policy**
     - Capability-based + policy
     - Deny-by-default with drift detection
     - Programs must target wasm32-wasip1
     - ~1.5x native
   * - **codejail (simple mode)**
     - Capability-based
     - None (operator-granted only)
     - Programs must target wasm32-wasip1
     - ~1.5x native
   * - **Docker**
     - Linux namespaces + cgroups
     - Optional (AppArmor, seccomp)
     - Any Linux binary
     - ~1x native
   * - **gVisor**
     - Userspace kernel
     - Seccomp-based
     - Any Linux binary
     - ~1.2x native (syscall overhead)
   * - **Firecracker**
     - Hardware VM
     - VM-level isolation
     - Any Linux binary
     - ~1x native (startup cost)
   * - **bubblewrap**
     - Linux namespaces
     - None
     - Any Linux binary
     - ~1x native

Codejail with policy enforcement trades compatibility (programs must be compiled to WASM) for a stronger security posture: deny-by-default capabilities, policy-enforced authorization, drift detection, and structured auditing.

Reporting vulnerabilities
-------------------------

If you find a security issue in codejail, please open an issue on GitHub. If the issue is sensitive (sandbox escape), email the maintainers instead of posting publicly.
