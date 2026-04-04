# http-tool

A WASM agent tool for making HTTP requests within sandboxed network boundaries.

## Build

```sh
cargo build --target wasm32-wasip1 --release
```

## Usage

The tool reads JSON from stdin and writes JSON to stdout.

### Make a GET request

```json
{"method": "GET", "url": "http://api.example.com/data", "headers": {"Authorization": "Bearer tok_..."}}
```

### Make a POST request

```json
{"method": "POST", "url": "http://api.example.com/submit", "headers": {"Content-Type": "application/json"}, "body": "{\"key\": \"value\"}"}
```

### Response format

```json
{
  "ok": true,
  "data": {
    "status": 200,
    "headers": {"content-type": "application/json"},
    "body": "{\"result\": \"success\"}"
  }
}
```

## Limitations

This implementation uses HTTP/1.1 over plain TCP sockets (WASI preview 1). To support HTTPS, add a WASM-compatible TLS library such as rustls (which compiles to wasm32-wasip1) and wrap the TCP stream in a TLS session.

The supervisor enforces `net_allow` at the TCP connect boundary. Connections to hosts not listed in JailFile.toml are rejected before any data is sent.

## Running with codejail

```sh
codejail run --jailfile JailFile.toml -- http_tool.wasm
```
