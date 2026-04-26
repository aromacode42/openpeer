// benches/bench.rs — criterion benchmarks
//
// These benchmarks are run with `cargo bench`. During `cargo test` they are
// compiled as an integration test suite. They use #[ignore] to avoid polluting
// the test output.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[allow(dead_code)]
fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[ignore]
#[test]
fn bench_fibonacci_10() {
    let result = fibonacci(black_box(10));
    assert_eq!(result, 55);
}

#[ignore]
#[test]
fn bench_fibonacci_20() {
    let result = fibonacci(black_box(20));
    assert_eq!(result, 6765);
}

pub fn bench_noop(_c: &mut Criterion) {}

criterion_group! { benches, bench_noop }
criterion_main!(benches);
