use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read as _};
use std::path::Path;

#[derive(Deserialize)]
struct Request {
    action: String,
    path: String,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn success(data: serde_json::Value) -> Self {
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

fn handle_read(path: &str) -> Response {
    match fs::read_to_string(path) {
        Ok(contents) => Response::success(serde_json::Value::String(contents)),
        Err(e) => Response::error(format!("failed to read {path}: {e}")),
    }
}

fn handle_write(path: &str, content: Option<&str>) -> Response {
    let Some(content) = content else {
        return Response::error("write action requires 'content' field");
    };

    // Ensure parent directory exists.
    if let Some(parent) = Path::new(path).parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Response::error(format!("failed to create parent directory: {e}"));
            }
        }
    }

    match fs::write(path, content) {
        Ok(()) => Response::success(serde_json::Value::String(format!("wrote {} bytes to {path}", content.len()))),
        Err(e) => Response::error(format!("failed to write {path}: {e}")),
    }
}

fn handle_list(path: &str) -> Response {
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut items = Vec::new();
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let is_dir = entry
                            .file_type()
                            .map(|ft| ft.is_dir())
                            .unwrap_or(false);
                        items.push(serde_json::json!({
                            "name": name,
                            "is_dir": is_dir,
                        }));
                    }
                    Err(e) => {
                        items.push(serde_json::json!({
                            "error": format!("{e}"),
                        }));
                    }
                }
            }
            Response::success(serde_json::Value::Array(items))
        }
        Err(e) => Response::error(format!("failed to list {path}: {e}")),
    }
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

    match req.action.as_str() {
        "read" => handle_read(&req.path),
        "write" => handle_write(&req.path, req.content.as_deref()),
        "list" => handle_list(&req.path),
        other => Response::error(format!("unknown action: {other}. expected: read, write, list")),
    }
}

fn main() {
    let response = run();
    // Serialization of our own Response type should never fail, but if it does,
    // write a raw JSON error so the supervisor always gets valid JSON.
    match serde_json::to_string(&response) {
        Ok(json) => println!("{json}"),
        Err(e) => println!(r#"{{"ok":false,"error":"serialization failed: {e}"}}"#),
    }
}
