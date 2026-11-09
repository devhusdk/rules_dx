use std::hint::black_box;
use std::time::Instant;

fn main() {
    let start = Instant::now();
    let rounds = 1000;
    let mut sum = 0u64;
    for n in 0..rounds {
        sum = sum.wrapping_add(black_box(n));
    }
    black_box(sum);
    let per_iter = start.elapsed().as_nanos() / u128::from(rounds);
    println!("test sum_small ... bench: {per_iter} ns/iter (+/- 0)");
}
