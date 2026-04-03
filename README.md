# codejail

A WASM sandbox for AI agent tool execution. Run tool implementations as WebAssembly modules with deny-by-default capabilities, policy-controlled authorization, intent-drift detection, and full audit trails. Every capability grant is evaluated against policy before the sandbox starts.

Built on the same governance engine as [Arbiter](https://github.com/cyrenei/mcp-proxy-firewall). Arbiter is the firewall — it controls which MCP tool calls an agent is allowed to make. Codejail is the sandbox — it controls what a tool can touch when it runs. Same policy language, different enforcement layer.

Built on [wasmtime](https://wasmtime.dev/) (WASI preview 1). Written in Rust.

## Install

The fastest way to install is the one-liner, which downloads a pre-built binary:

```bash
curl -sSf https://raw.githubusercontent.com/cyrenei/code-jail/main/install.sh | sh
```

This detects your OS and architecture, downloads the right binary from GitHub Releases, verifies the SHA256 checksum, and drops it into `~/.codejail/bin/`.

You can also pin a version or change the install directory:

```bash
CODEJAIL_VERSION=v0.1.0 CODEJAIL_INSTALL_DIR=/usr/local/bin curl -sSf \
  https://raw.githubusercontent.com/cyrenei/code-jail/main/install.sh | sh
```

### Other install methods

**Cargo** (if you already have Rust):

```bash
git clone https://github.com/cyrenei/code-jail.git
cd containment
cargo install --path .
```

**Docker** (no install needed):

```bash
docker run --rm codejail info
```

If you want to build Rust source files into WASM (using `codejail build`), you also need the compilation target:

```bash
rustup target add wasm32-wasip1
```

## Quick start

```bash
# Run a WASM module with policy enforcement.
# Every capability grant is evaluated against policy before the sandbox starts.
codejail run program.wasm --policy policy.toml

# Grant read access -- policy evaluates the grant before allowing it
codejail run agent.wasm \
  --policy policy.toml \
  --cap fs:read:/home/you/project

# Declare intent so drift detection can flag mismatches
codejail run agent.wasm \
  --policy policy.toml \
  --intent "read and analyze source code" \
  --audit-log audit.jsonl \
  -v ./project:/workspace \
  --cap net:api.openai.com:443 \
  -e API_KEY

# Build a Rust source file into a WASM image
codejail build .

# List your images and containers
codejail images
codejail ps -a
```

## Why?

AI agents act autonomously at machine speed. A single misconfigured agent
can run DDL on production databases, export customer data, or escalate
privileges — with nobody in the loop to stop it. [Arbiter](https://github.com/cyrenei/mcp-proxy-firewall) controls which tool calls get through. But when the tool itself is untrusted code, you also need to control what it can access. That's codejail.

Codejail enforces:

- **What** a tool can access (deny-by-default filesystem, network, environment grants)
- **How much** it can consume (CPU fuel budgets, wall-clock timeouts)
- **Whether it should** (policy evaluation + drift detection against declared intent)
- **That you'll know** (structured JSONL audit trail of every capability decision)

## How it works

Codejail has two layers that work together:

**The gate: policy authorization.** When you pass `--policy policy.toml`, every capability request is evaluated against a deny-by-default policy before execution begins. The policy engine checks tool name, parameters, and declared intent. Drift detection flags when requested capabilities don't match the agent's stated purpose. Every decision is audit-logged as structured JSONL.

**The cell: WASM isolation.** Authorized capabilities are enforced by running the program as a WebAssembly module inside wasmtime. The program starts with nothing — no filesystem, no network, no environment variables. Only policy-approved capabilities are wired in. The WASM boundary guarantees the program cannot exceed what was granted.

Without a policy file, the operator's capability requests are granted unconditionally — there is no gap between requesting a capability and receiving it, so there is nowhere for security policy to live.

## Policy enforcement

A policy file controls what capabilities are allowed:

```toml
[[policies]]
id = "allow-read-basic"
effect = "allow"
allowed_tools = ["fs_read", "env_read"]

[policies.intent_match]
keywords = ["read", "analyze"]

[[policies]]
id = "deny-write-default"
effect = "deny"
allowed_tools = ["fs_write"]
reason = "Write access requires explicit policy approval"
```

With this policy:
- `--cap fs:read:/project` with `--intent "read and analyze"` is **allowed**
- `--cap fs:write:/project` is **denied** (no matching allow rule)

Full policy language reference: [Policy Guide](https://github.com/cyrenei/arbiter-mcp-firewall/blob/main/docs/sphinx/guides/policy.md)

## What the policy gate enforces

| Enforcement | Without policy | With policy |
|---|---|---|
| Capability authorization | Operator's flags are final | Policy evaluates every grant |
| Intent verification | Not tracked | Drift detection flags mismatches |
| Session budgets | No per-call tracking | Call budgets and rate limits |
| Audit trail | Exit code only | Structured JSONL of every decision |
| Parameter constraints | None | Policy-defined bounds and allowlists |
| Session expiry | Capabilities last container lifetime | Sessions expire on time limit |
| Credential hygiene | Agent sees raw secrets | Response scrubbing across encodings |

## Capability model

Codejail uses a deny-by-default capability model. When you run a module with no flags, it gets nothing. Every permission is an explicit grant.

| Flag | What it grants |
|------|---------------|
| `--cap fs:read:/path` | Read-only access to a directory |
| `--cap fs:write:/path` | Read and write access to a directory |
| `--cap fs:/path` | Shorthand for read+write |
| `--cap net:host:port` | Network access to a specific destination |
| `--cap net:*` | Network access to everything |
| `--cap env:VAR1,VAR2` | Pass specific environment variables |
| `-v host:guest` | Mount a directory (read+write) |
| `-e KEY=VALUE` | Set an environment variable |
| `--net` | Allow all network access |
| `--bwrap` | Wrap in a bubblewrap namespace sandbox (defense in depth) |
| `--policy policy.toml` | Evaluate all grants against policy |
| `--intent "description"` | Declare session intent for drift detection |
| `--audit-log path.jsonl` | Write policy decisions to audit log |

Capabilities compose. You can pass as many `--cap` flags as you need. With policy enabled, each capability is individually evaluated before being granted to the runtime. Denied capabilities are removed. The audit log records every decision.

## Resource limits

| Flag | Default | What it does |
|------|---------|-------------|
| `--fuel N` | 1,000,000,000 | CPU fuel limit (wasmtime fuel units). Set to 0 for unlimited. |
| `--timeout N` | 300 | Wall-clock timeout in seconds |

When a program exceeds its fuel budget, it gets terminated immediately. The timeout kills it on a wall-clock basis regardless of fuel.

## Simple mode (not recommended)

You can run codejail without a policy file:

```bash
codejail run program.wasm --cap fs:read:/project
```

This grants capabilities directly to the sandbox with no policy evaluation, no drift detection, no session tracking, and no audit trail. The operator's flags are the only authorization layer.

Simple mode is useful for quick local testing but is **not recommended for production use or when running untrusted agents**. Without policy enforcement, there is no separation between who requests capabilities and who approves them.

## CLI reference

```
codejail run <image> [flags] [-- args...]         Run a WASM module in a sandbox
codejail build [dir] [-f JailFile.toml]           Build from a JailFile
codejail ps [-a]                                  List containers
codejail stop <id>                                Stop a running container
codejail rm <id>                                  Remove a stopped container
codejail prune                                    Remove all stopped containers
codejail images                                   List images
codejail import <name> <path.wasm>                Import a WASM module as an image
codejail rmi <name>                               Remove an image
codejail inspect <image>                          Show module exports and imports
codejail info                                     Show system info and capabilities
```

## JailFile

A JailFile is a TOML manifest that declares what a sandbox is allowed to do. Think of it like a Dockerfile but for permissions.

```toml
[sandbox]
name = "my-agent"
entrypoint = "agent.wasm"

[capabilities]
fs_read = ["/project"]
fs_write = ["/project/output", "/tmp"]
net_allow = ["api.openai.com:443", "github.com:443"]
env = ["HOME", "PATH", "API_KEY"]
stdin = true
stdout = true
stderr = true

[limits]
fuel = 1_000_000_000
wall_time_secs = 300
memory_mb = 512
```

Use it with `codejail run <image> -f JailFile.toml --policy policy.toml` or build with `codejail build`.

## Security model

### Trust model

| Actor | Trust level | Rationale |
|-------|------------|-----------|
| **Operator** | Trusted | Selects the policy file, declares intent, chooses capability grants |
| **Policy file** | Authoritative | Defines what capabilities are allowed; not validated for correctness |
| **WASM module** | Untrusted | Runs inside WASM isolation; can only access policy-approved capabilities |
| **Declared intent** | Advisory | Used for drift detection, not enforcement; an adversarial agent would lie |

Codejail is designed for scenarios where the operator is trusted but the code being executed is not — for example, running AI-generated tool implementations where the platform controls the policy but doesn't control the code.

### Isolation layers

**Layer 1: WASM capability isolation (always on).** The program runs as a WebAssembly module inside wasmtime. It can only access what the WASI runtime explicitly grants: preopened directories, network sockets, environment variables. Everything else returns "not found" errors. There is no /etc/passwd, no /proc, no home directory unless you mount one.

**Layer 2: Policy enforcement (recommended).** Every capability grant is evaluated against a deny-by-default policy. The policy engine checks agent identity, intent, tool name, and parameter constraints. Drift detection flags when capabilities diverge from declared intent. All decisions are audit-logged. This layer creates the gap between requesting and receiving a capability — the gap where security policy lives.

**Layer 3: Linux namespace isolation (opt-in with --bwrap).** Wraps the entire wasmtime process in a bubblewrap sandbox with unshared namespaces (PID, network, IPC, UTS, cgroup). This is defense in depth against wasmtime runtime bugs.

### What the sandbox blocks

Without explicit grants, a sandboxed program cannot:

- Read or write any file on the host
- Access the network
- See environment variables
- Read /proc or /sys
- Discover other processes
- Access your home directory
- Phone home or exfiltrate data

With policy enabled, even explicitly requested capabilities can be denied by policy, flagged by drift detection, or limited by session budgets.

### Known limitations

- WASM cannot spawn subprocesses. If your tool needs to run `git`, `cargo`, or `python`, those tools need to be compiled to WASM too (or called over network).
- No GPU passthrough. Programs that need GPU access should use API calls over the network instead.
- WASI preview 1 only. Preview 2 (component model) support is planned.
- No detached/background execution yet.
- Policy evaluation happens before execution, not during. Once a capability is granted, it's available for the entire session.
- **Only as secure as WASM.** While WASM is secure-by-design, codejail's isolation ceiling is wasmtime's. Layer 3 (bubblewrap) exists as defense in depth.

### When to use something else

- **Sandboxing arbitrary executables** (Python scripts, shell commands, native binaries) — codejail runs WASM modules only. See [gVisor](https://gvisor.dev/), [bubblewrap](https://github.com/containers/bubblewrap), or [nsjail](https://github.com/google/nsjail).
- **Container-level isolation** — codejail is not a container runtime. See [Firecracker](https://firecracker-microvm.github.io/) or [Kata Containers](https://katacontainers.io/).
- **Runtime per-call policy enforcement at the network layer** — codejail's policy runs before execution. If you need to gate every MCP tool call as it flows through, use [Arbiter](https://github.com/cyrenei/mcp-proxy-firewall).

## Docker

```bash
# Build the image
docker build -t codejail .

# Run a command
docker run --rm codejail info

# Run a WASM module from a host directory
docker run --rm -v ./workspace:/data/workspace codejail run /data/workspace/program.wasm

# For --bwrap support, the container needs extra privileges:
docker run --rm --cap-add SYS_ADMIN --security-opt apparmor=unconfined \
  codejail run --bwrap program.wasm

# Or use docker compose
docker compose run --rm codejail info
```

## Building from source

```bash
git clone https://github.com/cyrenei/code-jail.git
cd containment
cargo build --release

# Run tests (requires wasm32-wasip1 target)
rustup target add wasm32-wasip1
cargo test
```

## Documentation

Full documentation is available at the [project docs site](https://cyrenei.github.io/containment/).

Build locally with:

```bash
pip install -r docs/requirements.txt
sphinx-build docs docs/_build/html
```

## License

Apache-2.0. See [LICENSE](LICENSE).
