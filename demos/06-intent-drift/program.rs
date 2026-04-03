use std::fs;

fn main() {
    let path = "/workspace/report.txt";
    let content = "Containment report: all systems nominal.";

    println!("Writing to {path}...");
    match fs::write(path, content) {
        Ok(()) => println!("Write succeeded."),
        Err(e) => {
            println!("Write failed: {e}");
            return;
        }
    }

    println!("Reading back from {path}...");
    match fs::read_to_string(path) {
        Ok(data) => println!("Contents: {data}"),
        Err(e) => println!("Read failed: {e}"),
    }
}
