fn main() {
    println!("Starting CPU-intensive loop (100M iterations)...");
    let mut sum: u64 = 0;
    for i in 0..100_000_000 {
        sum = sum.wrapping_add(i);
    }
    println!("Sum: {sum}");
}
