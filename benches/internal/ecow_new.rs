#![allow(
    clippy::clone_on_copy,
    clippy::useless_conversion,
    clippy::explicit_auto_deref,
    noop_method_call
)]

//! Head-to-head benchmarks: turbocow vs ecow — creation (new / from / from_static).

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

// ─── new (owned) ───────────────────────────────────────────────────────────

fn bench_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/new");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("ecow", len), &len, |b, _| {
            let f = black_box(*fixture);
            b.iter(|| ecow::EcoString::from(f))
        });
        group.bench_with_input(BenchmarkId::new("turbocow", len), &len, |b, _| {
            let f = black_box(*fixture);
            b.iter(|| turbocow::EcoString::from(f))
        });
    }
    group.finish();
}

// ─── new (static / referenced) ─────────────────────────────────────────────

fn bench_new_static(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/new_static");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        // ecow has no from_static — it always copies
        group.bench_with_input(BenchmarkId::new("ecow::from", len), &len, |b, _| {
            let f = black_box(*fixture);
            b.iter(|| ecow::EcoString::from(f))
        });
        group.bench_with_input(
            BenchmarkId::new("turbocow::from_static", len),
            &len,
            |b, _| {
                let f = black_box(*fixture);
                b.iter(|| turbocow::EcoString::from_static(f))
            },
        );
    }
    group.finish();
}

// ─── new (borrow via EcoStr) ──────────────────────────────────────────────

fn bench_new_borrow(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/new_borrow");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        // ecow always copies — no borrow path
        group.bench_with_input(BenchmarkId::new("ecow::from", len), &len, |b, _| {
            let f = black_box(*fixture);
            b.iter(|| ecow::EcoString::from(f))
        });
        // turbocow EcoString::from — copies (backward compat)
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoString::from", len),
            &len,
            |b, _| {
                let f = black_box(*fixture);
                b.iter(|| turbocow::EcoString::from(f))
            },
        );
        // turbocow EcoStr::from — zero-copy borrow (Referenced)
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoStr::from", len),
            &len,
            |b, _| {
                let f = black_box(*fixture);
                b.iter(|| turbocow::EcoStr::from(f))
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_new, bench_new_static, bench_new_borrow);
criterion_main!(benches);
