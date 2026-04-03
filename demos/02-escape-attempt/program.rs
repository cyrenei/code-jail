use std::fs;

fn main() {
    println!("=== Attempting 8 escape vectors ===");
    println!();

    let tests: Vec<(&str, bool)> = vec![
        ("Read /etc/passwd", fs::read_to_string("/etc/passwd").is_ok()),
        ("Read /etc/shadow", fs::read_to_string("/etc/shadow").is_ok()),
        ("Read /home", fs::read_dir("/home").is_ok()),
        ("Read /root/.ssh", fs::read_dir("/root/.ssh").is_ok()),
        ("Read /proc/self/environ", fs::read_to_string("/proc/self/environ").is_ok()),
        ("Write /tmp/pwned.txt", fs::write("/tmp/pwned.txt", "got you").is_ok()),
        ("Read $HOME", std::env::var("HOME").is_ok()),
        ("Read $SSH_AUTH_SOCK", std::env::var("SSH_AUTH_SOCK").is_ok()),
    ];

    let mut escaped = 0;
    for (name, succeeded) in &tests {
        if *succeeded {
            println!("[LEAK]    {}", name);
            escaped += 1;
        } else {
            println!("[BLOCKED] {}", name);
        }
    }

    println!();
    println!("{}/8 vectors blocked", 8 - escaped);
}
