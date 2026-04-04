# Tool Migration Guide: Native to WASM

This document describes how to migrate agent tools from native execution (Tier 2, via `codejail make`) to WASM-native execution (Tier 1, compiled to `wasm32-wasip1`).

## 1. Assessment

Before migrating a tool, determine its feasibility class by answering three questions:

### Does it fork or exec subprocesses?

WASM modules cannot spawn processes. If the tool shells out to other binaries, it needs decomposition: identify what the subprocess does and implement that capability directly.

Example: A tool that runs `curl` to fetch data does not need curl. It needs an HTTP client. An HTTP client compiles to WASM.

### Does it use FFI or native libraries?

WASM has no dynamic linking (in wasip1). If the tool links against native C libraries, you need WASM-compatible alternatives:

- OpenSSL -> rustls (pure Rust, compiles to wasm32-wasip1)
- SQLite -> sqlite-wasm or implement the specific query protocol
- libgit2 -> git smart HTTP protocol client

If no alternative exists, the tool stays on the native bridge.

### Is it pure I/O + computation?

If the tool only reads input, processes it, and writes output -- using files, network, or stdin/stdout -- it is a direct port candidate. Most agent tools fall into this category.

## 2. Migration Tiers

### Tier A: Direct port

**What**: The tool's logic compiles directly to wasm32-wasip1 with no architectural changes.

**How**: Rewrite in Rust (or any language with wasip1 support), use `std::fs` for file I/O, `std::net::TcpStream` for network, serde for JSON. Follow the tool contract (JSON stdin -> JSON stdout).

**Time**: Hours to days.

**Examples**: File operations, HTTP API calls, JSON processing, text transforms, template rendering, data validation, encoding/decoding.

### Tier B: Decomposition

**What**: The tool relies on capabilities that don't exist in WASM, but those capabilities can be reimplemented using WASI primitives (files + sockets).

**How**: Identify the protocol or format the native tool uses. Implement a purpose-built client in WASM that speaks that protocol.

**Time**: Days to weeks, depending on protocol complexity.

**Examples**:
- Git operations -> implement git smart HTTP protocol (clone, fetch, push over HTTP)
- Database queries -> implement wire protocol client (PostgreSQL, MySQL, Redis)
- Package resolution -> implement registry HTTP client (npm registry API, crates.io API)
- S3 operations -> implement S3 REST API client

### Tier C: Interpreter embedding

**What**: The tool runs code in an interpreted language. Compile the interpreter itself to WASM.

**How**: Use an existing wasm32-wasi build of the interpreter, or cross-compile one. The interpreted code runs inside the WASM module, which runs inside wasmtime.

**Time**: Weeks (getting the interpreter build right, handling limitations).

**Limitations**: Performance overhead (interpreting inside an interpreter). Some interpreter features may not work (subprocess, FFI from the guest side). Available standard library may be reduced.

**Examples**:
- Python scripts -> CPython compiled to wasm32-wasi (builds exist), or RustPython
- Lua scripts -> Lua interpreter compiles trivially to WASM
- JavaScript -> QuickJS compiles to WASM

### Tier D: Native bridge required

**What**: The tool fundamentally requires host kernel features that cannot be expressed through WASI.

**How**: Keep on `codejail make` (the native bridge). Apply Linux namespace isolation, seccomp-bpf filters, and capability restrictions. Accept the weaker security properties.

**Time**: N/A (no migration, but harden the bridge).

**Examples**: Docker operations (needs kernel namespaces/cgroups), tools requiring GPU access, tools that must interact with host hardware, tools that run other containerized workloads.

## 3. Common Agent Tools: Classification and Migration Path

### File read/write -- Tier A, direct

Current: native `cat`, `cp`, `tee`, etc.

Migration: Implement as a WASM tool with `std::fs` operations. WASI preopened directories enforce the sandbox boundary. The file-tool reference implementation (`tools/file-tool/`) demonstrates this pattern.

Effort: Done. See `tools/file-tool/`.

### HTTP API calls -- Tier A, direct

Current: native `curl`, `wget`, or language HTTP libraries.

Migration: Implement as a WASM tool using `std::net::TcpStream` for HTTP/1.1. Add rustls for TLS. The `net_allow` JailFile capability restricts which hosts the tool can reach.

Effort: Small. The http-tool scaffold (`tools/http-tool/`) demonstrates the TCP-based approach. TLS requires adding rustls as a dependency.

### JSON processing -- Tier A, direct

Current: native `jq`, Python one-liners.

Migration: Implement as a WASM tool using serde_json. Read JSON from stdin or from a preopened file, apply transforms, write result. Zero WASI capabilities needed (pure computation).

Effort: Small. A jq-subset can be implemented in a few hundred lines of Rust.

### Text search and transform -- Tier A, direct

Current: native `grep`, `sed`, `awk`.

Migration: Implement as a WASM tool using the `regex` crate. Read input, apply pattern, write output.

Effort: Small.

### Template rendering -- Tier A, direct

Current: native Jinja2, Handlebars, etc.

Migration: Embed a template engine in the WASM module (e.g., the `handlebars` or `tera` Rust crates). Read template + data from stdin, write rendered output.

Effort: Small.

### Git clone/pull/push -- Tier B, git smart HTTP

Current: native `git` binary.

Migration: Implement a git smart HTTP protocol client in WASM. The protocol is well-documented (https://git-scm.com/docs/http-protocol). Key operations:

1. **Discovery**: `GET /info/refs?service=git-upload-pack` to list remote refs
2. **Fetch**: `POST /git-upload-pack` with want/have negotiation, receive packfile
3. **Push**: `POST /git-receive-pack` with packfile

The client reads/writes the local repo structure (loose objects, pack files, refs) to preopened directories. No `git` binary needed.

Effort: Significant (2-4 weeks for basic clone/fetch/push). Consider using an existing pure-Rust git implementation as a starting point (e.g., `gitoxide` components that compile to WASM).

### Python script execution -- Tier C, embedded CPython

Current: native `python3` binary.

Migration: Compile CPython to wasm32-wasi. Projects like `pywasm` and the official CPython wasm32-wasi build target make this possible. The Python script is bundled with or passed to the WASM module, which runs the interpreter internally.

Limitations:
- No subprocess (`os.system`, `subprocess` module)
- No FFI (`ctypes`, C extensions)
- Reduced standard library (no `socket` module unless WASI sockets are wired through)
- Performance: 2-5x slower than native CPython

Effort: Medium (1-2 weeks to get a working build, longer to handle edge cases).

### Shell commands -- Tier D or decompose

Current: native `/bin/sh -c "..."`.

Migration: Shell commands are not a single tool -- they are an unbounded set of tools composed through pipes and control flow. The migration strategy is **decomposition**:

1. Identify what the shell command actually does
2. Map each operation to a specific WASM tool
3. Compose the WASM tools at the agent level

Common decompositions:
- `curl URL | jq '.field'` -> http-tool + json-tool
- `find . -name '*.py' | xargs grep pattern` -> file-tool list + text-search-tool
- `cat file | sed 's/old/new/g' > output` -> file-tool read + text-transform-tool + file-tool write

If the shell command cannot be decomposed into existing tool categories, it stays on the native bridge (Tier D).

### Docker operations -- Tier D, native bridge

Current: native `docker` CLI.

Migration: Not feasible. Docker requires kernel-level namespace and cgroup management. The Docker CLI communicates with dockerd over a Unix socket using a complex API. Even implementing the REST API client in WASM would require granting the tool access to the Docker socket, which is effectively root access to the host.

Strategy: Keep on native bridge with maximum restriction. The codejail native bridge should mount the Docker socket read-only if the tool only needs to inspect containers, or deny it entirely if possible.

### Database queries -- Tier B, wire protocol client

Current: native `psql`, `mysql`, `redis-cli`, etc.

Migration: Implement wire protocol clients over WASI TCP sockets:

- **PostgreSQL**: The wire protocol is well-documented. A minimal client needs startup, simple query, and row parsing. The `rust-postgres` crate's protocol layer may compile to WASM with work.
- **MySQL**: Similar story. The COM_QUERY protocol is straightforward.
- **Redis**: RESP protocol is trivial to implement (it's a text protocol). A WASM Redis client is an afternoon's work.

`net_allow` in the JailFile restricts which database hosts the tool can reach.

Effort: Medium (1-2 weeks per database protocol for basic query support).

## 4. The codejail make Bridge as Migration Tool

`codejail make` wraps native binaries in a Linux namespace sandbox. It is the Tier 2 execution path: functional but with weaker security properties than WASM.

### Use it as scaffolding, not as architecture

During migration, `codejail make` serves a specific role:

1. **Inventory**: Wrap all current agent tools with `codejail make`. This establishes the capability baseline -- what does each tool actually need?
2. **Prioritize**: Sort tools by migration tier. Start with Tier A (direct ports), which give the most security improvement for the least effort.
3. **Migrate incrementally**: Replace native-bridge tools with WASM equivalents one at a time. Each replacement is a measurable security improvement.
4. **Sunset**: As WASM equivalents mature, deprecate the native-bridge versions. The bridge should shrink over time.

### Signals that communicate the tier difference

The codejail supervisor should make the tier difference visible:

- **Tier 1 (WASM)**: Clean execution. Capability proof in the audit log. No warnings.
- **Tier 2 (native bridge)**: Warning on every invocation. No capability proof. Audit log notes the weaker isolation.
- **Policy files**: Can require Tier 1 for specific tool categories. A policy that says "file operations must be WASM-native" forces migration of the file tool while allowing the Docker tool to remain on the bridge.

### The end state

The native bridge should converge toward handling only Tier D tools -- the genuinely un-WASM-ifiable cases (Docker, GPU, host hardware). Everything else runs in WASM with full capability proofs and instruction-set isolation.
