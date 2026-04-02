# Contributing to containment

Thanks for your interest. Here is how to get set up and what we expect from pull requests.

## Setup

```bash
git clone https://github.com/cyrenei/agent-wasm-containers.git
cd agent-wasm-containers

# You need Rust stable and the WASM target
rustup target add wasm32-wasip1

# Build and run tests
cargo build
cargo test

# Check formatting and lints before pushing
cargo fmt --check
cargo clippy -- -D warnings
```

## Running tests

Tests compile Rust fixtures to WASM and run them through the containment binary. The wasm32-wasip1 target must be installed or tests will fail.

```bash
cargo test
```

Individual tests:

```bash
cargo test test_hello
cargo test test_escape
```

## Pull request checklist

- `cargo fmt` passes with no changes
- `cargo clippy -- -D warnings` is clean
- `cargo test` passes
- New features have tests
- If you add a CLI flag, update the docs (both README and Sphinx)

## Code style

We follow standard Rust conventions. A few project-specific things:

- Keep the CLI commands in `main.rs` as thin dispatchers. Logic goes in the modules.
- Capability parsing lives in `capability.rs`. If you add a new capability type, add it there.
- Container state is JSON files in ~/.containment/containers/. Keep it simple.
- Error messages should be clear and actionable. Tell the user what went wrong and what they can do about it.

## Architecture

```
src/
  main.rs          CLI entry point (clap). Command dispatching.
  capability.rs    Containmentfile parsing, capability types, resolution.
  runtime.rs       Wasmtime wrapper. Builds WASI context, runs modules.
  container.rs     Container state (create, list, stop, remove).
  image.rs         Image store (import, list, resolve, remove).
  sandbox.rs       Bubblewrap outer sandbox (optional defense in depth).
```

The flow for `containment run`:

1. Resolve the image path (direct path or image store lookup)
2. Parse capability grants from --cap flags, -v volumes, -e env vars
3. Load Containmentfile if -f is provided
4. Merge all capabilities into a ResolvedCaps
5. Build a WasiCtxBuilder with the resolved capabilities
6. Create a wasmtime Store with fuel limits
7. Load the module, link WASI, run _start
8. Record container state (exited / failed)

## Adding new capabilities

1. Add the new type to `CapGrant` enum in `capability.rs`
2. Add parsing in `CapGrant::parse()`
3. Handle the grant in `ResolvedCaps::from_parts()`
4. Wire it into the WASI context builder in `runtime.rs`
5. Add a CLI flag in `main.rs` if needed
6. Write a test fixture and integration test
7. Document it

## Questions

Open an issue. We will get back to you.
