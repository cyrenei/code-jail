/// Burns CPU to test fuel limits.
fn main() {
    let mut sum: u64 = 0;
    for i in 0..100_000_000u64 {
        sum = sum.wrapping_add(i);
    }
    println!("Sum: {sum}");
}
