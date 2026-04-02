use std::fs;

fn main() {
    // Try to read from a preopened directory
    match fs::read_dir("/sandbox") {
        Ok(entries) => {
            println!("Directory listing of /sandbox:");
            for entry in entries {
                match entry {
                    Ok(e) => println!("  {}", e.file_name().to_string_lossy()),
                    Err(e) => eprintln!("  Error: {e}"),
                }
            }
        }
        Err(e) => {
            eprintln!("Cannot read /sandbox: {e}");
            std::process::exit(1);
        }
    }
}
