fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("Got {} args:", args.len());
    for (i, arg) in args.iter().enumerate() {
        println!("  [{i}] {arg}");
    }

    if args.len() < 2 {
        eprintln!("Expected at least 1 argument");
        std::process::exit(1);
    }
    println!("\nFirst real arg: {}", args[1]);
}
