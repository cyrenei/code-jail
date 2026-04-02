# cask

A WASM sandbox that works like Docker. Run untrusted programs with deny-by-default capabilities.

Nothing is allowed unless you say so. No filesystem access, no network, no environment variables. You grant what you want, and the sandbox enforces the rest.

Built on [wasmtime](https://wasmtime.dev/) (WASI preview 1). Written in Rust.

## Install

```bash
# From source
git clone https://github.com/cyrenei/agent-wasm-containers.git
cd agent-wasm-containers
cargo install --path .

# You also need the WASM compilation target for building .rs files
rustup target add wasm32-wasip1
```

## Quick start

```bash
# Run a WASM module in a fully isolated sandbox.
# No filesystem, no network, no env vars. Just stdout/stderr.
cask run program.wasm

# Give it read access to your project directory
cask run agent.wasm --cap fs:read:/home/you/project

# Mount a working directory and allow one API endpoint
cask run agent.wasm \
  -v ./project:/workspace \
  --cap net:api.openai.com:443 \
  -e API_KEY

# Build a Rust source file into a WASM image
cask build .

# List your images and containers
cask images
cask ps -a
```

## Capability model

Cask uses a deny-by-default capability model. When you run a module with no flags, it gets nothing. Every permission is an explicit grant.

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

Capabilities compose. You can pass as many `--cap` flags as you need.

## Resource limits

| Flag | Default | What it does |
|------|---------|-------------|
| `--fuel N` | 1,000,000,000 | CPU fuel limit (wasmtime fuel units). Set to 0 for unlimited. |
| `--timeout N` | 300 | Wall-clock timeout in seconds |

When a program exceeds its fuel budget, it gets terminated immediately. The timeout kills it on a wall-clock basis regardless of fuel.

## CLI reference

```
cask run <image> [flags] [-- args...]    Run a WASM module in a sandbox
cask build [dir] [-f Caskfile.toml]      Build from a Caskfile
cask ps [-a]                             List containers
cask stop <id>                           Stop a running container
cask rm <id>                             Remove a stopped container
cask prune                               Remove all stopped containers
cask images                              List images
cask import <name> <path.wasm>           Import a WASM module as an image
cask rmi <name>                          Remove an image
cask inspect <image>                     Show module exports and imports
cask info                                Show system info and capabilities
```

## Caskfile

A Caskfile is a TOML manifest that declares what a sandbox is allowed to do. Think of it like a Dockerfile but for permissions.

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

Use it with `cask run <image> -f Caskfile.toml` or build with `cask build`.

## Security model

The sandbox has two layers of isolation:

**Layer 1: WASM capability isolation (always on).** The program runs as a WebAssembly module inside wasmtime. It can only access what the WASI runtime explicitly grants: preopened directories, network sockets, environment variables. Everything else returns "not found" errors. There is no /etc/passwd, no /proc, no home directory unless you mount one.

**Layer 2: Linux namespace isolation (opt-in with --bwrap).** Wraps the entire wasmtime process in a bubblewrap sandbox with unshared namespaces (PID, network, IPC, UTS, cgroup). This is defense in depth against wasmtime runtime bugs.

### What the sandbox blocks

Without explicit grants, a sandboxed program cannot:

- Read or write any file on the host
- Access the network
- See environment variables
- Read /proc or /sys
- Discover other processes
- Access your home directory
- Phone home or exfiltrate data

### Known limitations

- WASM cannot spawn subprocesses. If your tool needs to run `git`, `cargo`, or `python`, those tools need to be compiled to WASM too (or called over network).
- No GPU passthrough. Programs that need GPU access should use API calls over the network instead.
- WASI preview 1 only. Preview 2 (component model) support is planned.
- No detached/background execution yet.

## Building from source

```bash
git clone https://github.com/cyrenei/agent-wasm-containers.git
cd agent-wasm-containers
cargo build --release

# Run tests (requires wasm32-wasip1 target)
rustup target add wasm32-wasip1
cargo test
```

## Documentation

Full documentation is available at the [project docs site](https://cyrenei.github.io/agent-wasm-containers/).

Build locally with:

```bash
pip install -r docs/requirements.txt
sphinx-build docs docs/_build/html
```

## License

MIT. See [LICENSE](LICENSE).
