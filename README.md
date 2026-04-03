# codejail

A WASM sandbox with policy-enforced access control. Run untrusted programs (like basically any AI model) where every capability is deny-by-default and every grant is checked against policy before it takes effect.

The sandbox is the cell. The policy engine is the guard. Together they form codejail.

Built on [wasmtime](https://wasmtime.dev/) (WASI preview 1). Written in Rust.

## Install

The fastest way to install is the one-liner, which downloads a pre-built binary:

```bash
curl -sSf https://raw.githubusercontent.com/cyrenei/containment/main/install.sh | sh
```

This detects your OS and architecture, downloads the right binary from GitHub Releases, verifies the SHA256 checksum, and drops it into `~/.codejail/bin/`.

You can also pin a version or change the install directory:

```bash
CODEJAIL_VERSION=v0.1.0 CODEJAIL_INSTALL_DIR=/usr/local/bin curl -sSf \
  https://raw.githubusercontent.com/cyrenei/containment/main/install.sh | sh
```

### Other install methods

**Cargo** (if you already have Rust):

```bash
git clone https://github.com/cyrenei/containment.git
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

## How it works

Codejail has two layers that work together:

**The cell: WASM capability isolation.** Programs run as WebAssembly modules inside wasmtime. They start with nothing -- no filesystem, no network, no environment variables. You grant capabilities explicitly with `--cap` flags.

**The guard: policy enforcement.** When you pass `--policy policy.toml`, every capability grant is evaluated against a deny-by-default policy before the sandbox starts. The policy engine can allow, deny, or flag each grant. Drift detection catches when requested capabilities don't match declared intent. Every decision is audit-logged.

Without a policy file, the operator's capability requests are granted unconditionally -- there is no gap between requesting a capability and receiving it, so there is nowhere for security policy to live.

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

The sandbox has three layers of defense:

**Layer 1: WASM capability isolation (always on).** The program runs as a WebAssembly module inside wasmtime. It can only access what the WASI runtime explicitly grants: preopened directories, network sockets, environment variables. Everything else returns "not found" errors. There is no /etc/passwd, no /proc, no home directory unless you mount one.

**Layer 2: Policy enforcement (recommended).** Every capability grant is evaluated against a deny-by-default policy. The policy engine checks agent identity, intent, tool name, and parameter constraints. Drift detection flags when capabilities diverge from declared intent. All decisions are audit-logged. This layer creates the gap between requesting and receiving a capability -- the gap where security policy lives.

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
git clone https://github.com/cyrenei/containment.git
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
