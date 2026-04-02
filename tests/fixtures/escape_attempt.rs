use std::fs;

/// Attempts to escape the sandbox — all should fail
fn main() {
    println!("=== Sandbox escape test ===");
    println!("All of these should fail:\n");

    // Try to read root filesystem
    print!("Read /etc/passwd:        ");
    match fs::read_to_string("/etc/passwd") {
        Ok(_) => println!("ESCAPED! (read succeeded)"),
        Err(e) => println!("BLOCKED ({e})"),
    }

    // Try to read home directory
    print!("Read /home:              ");
    match fs::read_dir("/home") {
        Ok(_) => println!("ESCAPED! (listing succeeded)"),
        Err(e) => println!("BLOCKED ({e})"),
    }

    // Try to write to /tmp
    print!("Write /tmp/escape.txt:   ");
    match fs::write("/tmp/escape.txt", "escaped") {
        Ok(_) => println!("ESCAPED! (write succeeded)"),
        Err(e) => println!("BLOCKED ({e})"),
    }

    // Try to read environment (should be empty unless granted)
    print!("Read $HOME:              ");
    match std::env::var("HOME") {
        Ok(v) => println!("LEAKED ({v})"),
        Err(_) => println!("BLOCKED (not set)"),
    }

    // Try to list /proc (would reveal host info)
    print!("Read /proc:              ");
    match fs::read_dir("/proc") {
        Ok(_) => println!("ESCAPED! (proc visible)"),
        Err(e) => println!("BLOCKED ({e})"),
    }

    println!("\n=== Done ===");
}
