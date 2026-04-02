use std::fs;

fn main() {
    let test_file = "/workspace/test_output.txt";
    match fs::write(test_file, "Written from WASM sandbox!\n") {
        Ok(()) => println!("Successfully wrote to {test_file}"),
        Err(e) => {
            eprintln!("Cannot write to {test_file}: {e}");
            std::process::exit(1);
        }
    }

    // Read it back
    match fs::read_to_string(test_file) {
        Ok(content) => println!("Read back: {content}"),
        Err(e) => eprintln!("Cannot read back: {e}"),
    }
}
