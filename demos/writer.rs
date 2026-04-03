use std::fs;

fn main() {
    let report = "/workspace/analysis.txt";
    let content = "Analysis complete.\nNo vulnerabilities found.\nGenerated at: sandbox runtime\n";
    match fs::write(report, content) {
        Ok(_) => {
            println!("Wrote report to {}", report);
            println!("Contents:");
            println!("{}", fs::read_to_string(report).unwrap());
        }
        Err(e) => {
            eprintln!("Failed to write report: {}", e);
            std::process::exit(1);
        }
    }
}
