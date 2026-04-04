use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read as _, Write as _};
use std::net::TcpStream;

#[derive(Deserialize)]
struct Request {
    method: String,
    url: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<HttpResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct HttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

impl Response {
    fn success(data: HttpResponse) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

/// Parse a URL into (host, port, path) for plain HTTP.
/// TLS support requires a WASM-compatible TLS library (e.g., rustls compiled to wasip1).
/// This implementation handles HTTP only; HTTPS is documented below.
fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else if url.starts_with("https://") {
        return Err(
            "HTTPS requires a WASM-compatible TLS library (rustls). \
             This scaffold implements HTTP/1.1 over plain TCP only. \
             To add TLS: depend on rustls + webpki-roots, wrap the TcpStream in a TLS session."
                .to_string(),
        );
    } else {
        return Err(format!("unsupported URL scheme in: {url}"));
    };

    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(i) => {
            let port_str = &host_port[i + 1..];
            let port: u16 = port_str
                .parse()
                .map_err(|_| format!("invalid port: {port_str}"))?;
            (host_port[..i].to_string(), port)
        }
        None => (host_port.to_string(), 80),
    };

    Ok((host, port, path.to_string()))
}

fn do_request(req: &Request) -> Result<HttpResponse, String> {
    let (host, port, path) = parse_url(&req.url)?;
    let addr = format!("{host}:{port}");

    // WASI preview 1 in wasmtime supports std::net::TcpStream.
    // The supervisor's net_allow policy is enforced at connect time --
    // if the host:port is not in the allowlist, connect() returns an error.
    let mut stream =
        TcpStream::connect(&addr).map_err(|e| format!("connection to {addr} failed: {e}"))?;

    // Build HTTP/1.1 request.
    let method = req.method.to_uppercase();
    let body_bytes = req.body.as_deref().unwrap_or("").as_bytes();

    let mut request_buf = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");

    for (key, value) in &req.headers {
        request_buf.push_str(&format!("{key}: {value}\r\n"));
    }

    if !body_bytes.is_empty() {
        request_buf.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    }

    request_buf.push_str("\r\n");

    stream
        .write_all(request_buf.as_bytes())
        .map_err(|e| format!("failed to send request: {e}"))?;

    if !body_bytes.is_empty() {
        stream
            .write_all(body_bytes)
            .map_err(|e| format!("failed to send body: {e}"))?;
    }

    // Read response.
    let mut reader = BufReader::new(&stream);

    // Parse status line.
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .map_err(|e| format!("failed to read status line: {e}"))?;

    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .ok_or("malformed status line")?
        .parse()
        .map_err(|_| "non-numeric HTTP status code")?;

    // Parse headers.
    let mut headers = HashMap::new();
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("failed to read header: {e}"))?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim().to_string();
            if key == "content-length" {
                content_length = value.parse().ok();
            }
            headers.insert(key, value);
        }
    }

    // Read body.
    let body = if let Some(len) = content_length {
        let mut buf = vec![0u8; len];
        reader
            .read_exact(&mut buf)
            .map_err(|e| format!("failed to read body: {e}"))?;
        String::from_utf8_lossy(&buf).into_owned()
    } else {
        // Read until connection close.
        let mut buf = Vec::new();
        reader
            .read_to_end(&mut buf)
            .map_err(|e| format!("failed to read body: {e}"))?;
        String::from_utf8_lossy(&buf).into_owned()
    };

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn run() -> Response {
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        return Response::error(format!("failed to read stdin: {e}"));
    }

    let req: Request = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => return Response::error(format!("invalid JSON input: {e}")),
    };

    match do_request(&req) {
        Ok(http_resp) => Response::success(http_resp),
        Err(e) => Response::error(e),
    }
}

fn main() {
    let response = run();
    match serde_json::to_string(&response) {
        Ok(json) => println!("{json}"),
        Err(e) => println!(r#"{{"ok":false,"error":"serialization failed: {e}"}}"#),
    }
}
