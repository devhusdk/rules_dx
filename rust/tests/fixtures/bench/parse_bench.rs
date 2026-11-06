use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

fn parse(input: &str) -> u32 {
    input.bytes().filter(u8::is_ascii_digit).map(|b| u32::from(b - b'0')).sum()
}

fn parse_small(c: &mut Criterion) {
    c.bench_function("parse_small", |b| b.iter(|| parse(black_box("a1b2c3d4"))));
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(300));
    targets = parse_small
}
criterion_main!(benches);
