# file-tool

A WASM agent tool for file operations within sandboxed directories.

## Build

```sh
cargo build --target wasm32-wasip1 --release
```

The output WASM module will be at `target/wasm32-wasip1/release/file-tool.wasm`.

## Usage

The tool reads JSON from stdin and writes JSON to stdout.

### Read a file

```json
{"action": "read", "path": "/workspace/file.txt"}
```

### Write a file

```json
{"action": "write", "path": "/workspace/output/result.txt", "content": "hello world"}
```

### List a directory

```json
{"action": "list", "path": "/workspace/"}
```

### Response format

```json
{"ok": true, "data": "..."}
{"ok": false, "error": "description of what went wrong"}
```

## Running with codejail

```sh
codejail run --jailfile JailFile.toml -- file_tool.wasm
```

The JailFile.toml declares the filesystem capabilities the tool needs. codejail grants preopened directories matching those declarations and nothing else.
