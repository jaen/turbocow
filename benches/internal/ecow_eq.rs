#![allow(
    clippy::clone_on_copy,
    clippy::useless_conversion,
    clippy::explicit_auto_deref,
    noop_method_call
)]

//! Head-to-head benchmarks: turbocow vs ecow — self-equality.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

const SAMPLES: &[&str] = &[
    "",
    "1",
    "123456789012345",
    "1234567890123456",
    "1234567890123456789012",
    "12345678901234567890123",
    "123456789012345678901234",
    "1234567890123456789012345",
    "1234567890123456789012345678901234567890123456789012345678901234",
    "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890",
];

// ─── self equality (owned) ─────────────────────────────────────────────────

fn bench_self_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/self_eq");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("ecow", len), &len, |b, _| {
            let uut = ecow::EcoString::from(*fixture);
            let uut = black_box(uut);
            let copy = uut.clone();
            let copy = black_box(copy);
            b.iter(|| uut == copy)
        });
        group.bench_with_input(BenchmarkId::new("turbocow", len), &len, |b, _| {
            let uut = turbocow::EcoString::from(*fixture);
            let uut = black_box(uut);
            let copy = uut.clone();
            let copy = black_box(copy);
            b.iter(|| uut == copy)
        });
    }
    group.finish();
}

// ─── self equality (static / referenced) ────────────────────────────────────

fn bench_self_eq_static(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/self_eq_static");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("ecow::from", len), &len, |b, _| {
            let uut = ecow::EcoString::from(*fixture);
            let uut = black_box(uut);
            let copy = uut.clone();
            let copy = black_box(copy);
            b.iter(|| uut == copy)
        });
        group.bench_with_input(
            BenchmarkId::new("turbocow::from_static", len),
            &len,
            |b, _| {
                let uut = turbocow::EcoString::from_static(*fixture);
                let uut = black_box(uut);
                let copy = uut.clone();
                let copy = black_box(copy);
                b.iter(|| uut == copy)
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_self_eq, bench_self_eq_static);
criterion_main!(benches);
