fn main() {
    println!("Hello! I'm running inside a WASM sandbox.");
    println!("I have no access to your filesystem, network, or env.");

    match std::env::var("HOME") {
        Ok(v) => println!("HOME={} (this should not happen)", v),
        Err(_) => println!("HOME is not visible. Good."),
    }

    match std::fs::read_dir("/") {
        Ok(_) => println!("Root filesystem is visible (this should not happen)"),
        Err(_) => println!("Root filesystem is not visible. Good."),
    }
}
