use std::fs;

fn main() {
    println!("=== I will try to escape the sandbox ===");
    println!();

    for path in &["/etc/passwd", "/etc/shadow", "/home", "/root/.ssh", "/proc/self/environ"] {
        match fs::read_to_string(path) {
            Ok(content) => println!("[LEAK] {} ({} bytes)", path, content.len()),
            Err(_) => println!("[BLOCKED] {}", path),
        }
    }

    match fs::write("/tmp/pwned.txt", "got you") {
        Ok(_) => println!("[LEAK] wrote to /tmp/pwned.txt"),
        Err(_) => println!("[BLOCKED] write to /tmp/pwned.txt"),
    }

    match std::env::var("HOME") {
        Ok(v) => println!("[LEAK] HOME={}", v),
        Err(_) => println!("[BLOCKED] HOME not visible"),
    }
    match std::env::var("SSH_AUTH_SOCK") {
        Ok(v) => println!("[LEAK] SSH_AUTH_SOCK={}", v),
        Err(_) => println!("[BLOCKED] SSH_AUTH_SOCK not visible"),
    }

    println!();
    println!("=== Done ===");
}
