# WASM-First Agent Tool Architecture

## 1. The Constraint

Agent tool implementations MUST compile to `wasm32-wasip1`. This is not a preference -- it is the security boundary.

A WASM module executes within wasmtime's sandbox. It gets access to the host only through WASI capabilities that codejail explicitly grants. There is no ambient authority. The module cannot:

- Execute arbitrary system calls
- Shell out to other processes
- Access the filesystem outside preopened directories
- Open network connections outside declared allowlists
- Read environment variables not explicitly passed

This is not enforced by a permission layer on top of native code. The instructions to do these things **do not exist** in the WASM instruction set. The sandbox is the absence of capability, not the denial of it.

WASI preview 1 (wasip1) provides the host interface: file I/O through preopened directories, stdin/stdout/stderr, environment variables, clocks, and (via wasmtime extensions) TCP/UDP sockets. This is sufficient for the vast majority of agent tools.

## 2. Tool Contract

Every WASM agent tool follows a uniform contract:

### Module requirements

- Compiled to `wasm32-wasip1` (a `.wasm` file)
- Exports a `_start` entry point (standard WASI command module)
- Reads structured JSON from **stdin** (the tool invocation)
- Writes structured JSON to **stdout** (the tool result)
- Writes diagnostic/log output to **stderr** (captured by supervisor)

### Invocation format (stdin)

```json
{
  "action": "...",
  "params": { ... }
}
```

The `action` field identifies the operation. The `params` object carries action-specific arguments. Each tool defines its own action vocabulary.

### Result format (stdout)

Success:
```json
{
  "ok": true,
  "data": ...
}
```

Failure:
```json
{
  "ok": false,
  "error": "human-readable error description"
}
```

Tools MUST NOT panic. All errors are caught and returned as JSON error responses. A panic is a supervisor-level fault, not an application-level error.

### Capability declaration (JailFile.toml)

Every tool ships with a `JailFile.toml` that declares the capabilities it needs:

```toml
[sandbox]
name = "tool-name"
entrypoint = "tool_name.wasm"

[capabilities]
fs_read = ["/workspace"]
fs_write = ["/workspace/output"]
net_allow = ["api.example.com:443"]
env = ["API_KEY"]
inherit_env = false

[limits]
memory_mb = 64
fuel = 100_000_000
wall_time_secs = 30
```

The supervisor reads this manifest, validates it against policy, and configures wasmtime's WASI context to grant exactly these capabilities -- nothing more.

## 3. WASI Capability Mapping

Each capability declared in JailFile.toml maps to a specific WASI configuration:

### `fs_read` -> Preopened directory (read-only)

```rust
// For each path in capabilities.fs_read:
wasi_ctx.preopened_dir(
    host_path,
    guest_path,
    DirPerms::READ,
    FilePerms::READ,
)?;
```

The WASM module sees the directory at its guest path. It can read files and list directories. It cannot write, create, delete, or modify anything.

### `fs_write` -> Preopened directory (read-write)

```rust
// For each path in capabilities.fs_write:
wasi_ctx.preopened_dir(
    host_path,
    guest_path,
    DirPerms::all(),
    FilePerms::all(),
)?;
```

Full read-write access within the declared directory tree. The module can create, modify, and delete files. It still cannot escape the preopened boundary.

### `net_allow` -> WASI socket allowlist

Wasmtime's WASI implementation supports TCP and UDP sockets. The supervisor interposes on socket creation and connect/bind operations, checking the destination against the declared allowlist:

```
net_allow = ["api.example.com:443", "*.internal.corp:8080"]
```

Only connections to listed host:port pairs succeed. All others return a WASI error. No DNS resolution to unexpected hosts. No port scanning.

### `env` -> Explicit environment variables

```rust
// Only these env vars are injected into WASI context:
for key in capabilities.env {
    if let Ok(val) = std::env::var(key) {
        wasi_ctx.env(key, &val);
    }
}
```

`inherit_env = false` (the default and strongly recommended setting) means the module gets an empty environment except for the explicitly listed variables.

### Everything else: denied by absence

There is no capability for "execute subprocess." There is no capability for "load shared library." There is no capability for "access /proc." These operations do not exist in WASI preview 1. The module physically cannot request them. This is the fundamental advantage over native sandboxing: the attack surface is defined by what the sandbox provides, not by what it blocks.

## 4. Reference Tool Patterns

### File tool (Tier A -- direct WASI)

Purpose: Read, write, and list files within preopened directories.

Capabilities needed: `fs_read`, `fs_write`

Pattern:
- Standard filesystem APIs (`std::fs`) work unchanged when compiled to wasip1
- Paths are relative to preopened directories
- WASI traps any attempt to escape the preopened root
- No platform-specific code needed

Actions: `read`, `write`, `list`

See: `tools/file-tool/`

### HTTP tool (Tier A -- WASI sockets)

Purpose: Make HTTP requests to declared hosts.

Capabilities needed: `net_allow`

Pattern:
- Raw TCP sockets are available via WASI in wasmtime
- Build HTTP/1.1 requests over the socket
- Or use a WASI-compatible HTTP client crate
- TLS requires a WASM-compiled TLS library (rustls compiles to wasm32-wasip1)
- The supervisor enforces the host allowlist at connect time

Actions: `request` (with method, url, headers, body)

See: `tools/http-tool/`

### Transform tool (Tier A -- pure computation)

Purpose: JSON/text processing with no I/O beyond stdin/stdout.

Capabilities needed: none (pure computation)

Pattern:
- Read input from stdin, write output to stdout
- No filesystem access, no network access
- Fuel limits prevent infinite loops
- Memory limits prevent allocation bombs
- The simplest and most secure tool category

Actions: tool-specific (jq-like queries, regex transforms, template rendering)

## 5. WASM Feasibility Matrix

| Tool category | WASM feasibility | Strategy |
|---|---|---|
| File I/O | Native WASI | Direct implementation. `std::fs` works. |
| HTTP API calls | WASI sockets (wasmtime) | Direct implementation over TCP. TLS via rustls. |
| JSON/text processing | Pure computation | Direct implementation. Zero capabilities needed. |
| Template rendering | Pure computation | Direct implementation. Embed engine in WASM. |
| Git operations | Hard (needs subprocess) | Implement git smart HTTP protocol client in WASM. Talks to remotes over HTTP, reads/writes pack files to preopened dirs. No `git` binary needed. |
| Python execution | Hard but possible | Compile CPython to wasm32-wasi. RustPython also targets WASM. Performance overhead is real but acceptable for agent tasks. |
| Shell commands | Impossible in WASM | Decompose into purpose-built WASM tools. `curl` becomes the HTTP tool. `jq` becomes the transform tool. `find` becomes the file tool's list action. |
| Package managers (npm, cargo) | Very hard | Implement HTTP-based registry clients. npm registry is a REST API. crates.io is a REST API + git index. Resolution logic reimplemented in WASM. |
| Database queries | Hard | Implement wire protocol clients (PostgreSQL, MySQL, Redis protocols are well-documented). Connect over WASI sockets. |
| Docker operations | Impossible in WASM | Docker requires host kernel interaction (namespaces, cgroups). Must remain native bridge. |

## 6. The Incentive Structure

Two tiers of tool execution exist in codejail:

### Tier 1: WASM-native tools

- Full policy integration via JailFile.toml
- Capability proofs: the supervisor can **prove** what a tool had access to
- Complete audit trail: every WASI call is observable
- Fuel metering: deterministic resource limits
- Memory isolation: linear memory with hard bounds
- No ambient authority: capabilities are granted, not inherited
- Composable: tools can be chained without privilege escalation

### Tier 2: Native bridge (`codejail make`)

- Partial policy integration: Linux namespaces and seccomp filters
- No capability proofs: the supervisor knows what it blocked, not what the tool tried
- Limited audit: syscall-level logging is noisy and incomplete
- Resource limits via cgroups: less precise than fuel metering
- Ambient authority leaks: difficult to enumerate all possible side channels
- Warning emitted on every invocation: "This tool runs outside the WASM sandbox"

### The gradient

The architecture makes WASM tools strictly better on every axis that matters for agent safety:

1. **Policy authors** prefer Tier 1 because capability declarations are complete and verifiable
2. **Auditors** prefer Tier 1 because the execution trace is deterministic
3. **Users** prefer Tier 1 because the security guarantees are stronger
4. **Tool authors** prefer Tier 1 once the ecosystem exists, because the contract is simpler (no platform-specific code, no capability negotiation)

The native bridge exists as a pragmatic escape hatch, not as an alternative architecture. Every tool that moves from Tier 2 to Tier 1 is a measurable improvement in the security posture of the system. The migration path is documented in `docs/tool-migration.md`.
