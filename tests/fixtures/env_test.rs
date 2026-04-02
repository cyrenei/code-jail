fn main() {
    println!("Environment variables:");
    for (key, value) in std::env::vars() {
        println!("  {key}={value}");
    }

    // Check for specific expected vars
    match std::env::var("SANDBOX_TEST") {
        Ok(v) => println!("\nSANDBOX_TEST = {v}"),
        Err(_) => {
            eprintln!("\nSANDBOX_TEST not set!");
            std::process::exit(1);
        }
    }
}
