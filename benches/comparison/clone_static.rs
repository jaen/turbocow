#![allow(
    clippy::clone_on_copy,
    clippy::useless_conversion,
    clippy::explicit_auto_deref,
    noop_method_call
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

mod fixture;

type StringCow<'s> = std::borrow::Cow<'s, str>;

fn bench_clone_static(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone_static");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in fixture::SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("&'static str", len), &len, |b, _| {
            let uut = *fixture;
            let uut = std::hint::black_box(uut);
            b.iter(|| uut.clone())
        });
        group.bench_with_input(
            BenchmarkId::new("Cow<str>::Borrowed", len),
            &len,
            |b, _| {
                let uut = StringCow::Borrowed(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.clone())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("compact_str::CompactString::const_new", len),
            &len,
            |b, _| {
                let uut = compact_str::CompactString::const_new(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.clone())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("hipstr::HipStr::borrowed", len),
            &len,
            |b, _| {
                let uut = hipstr::HipStr::borrowed(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.clone())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoString::from_static", len),
            &len,
            |b, _| {
                let uut = turbocow::EcoString::from_static(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.clone())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("lean_string::LeanString::from_static_str", len),
            &len,
            |b, _| {
                let uut = lean_string::LeanString::from_static_str(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.clone())
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_clone_static);
criterion_main!(benches);
