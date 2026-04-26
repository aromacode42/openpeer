// benches/bench.rs placeholder
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn bench_empty(c: &mut Criterion) {
    c.bench_function("noop", |b| b.iter(|| black_box(())));
}

criterion_group! { benches, bench_empty }
criterion_main!(benches);
