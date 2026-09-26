#![allow(clippy::clone_on_copy, clippy::useless_conversion, clippy::explicit_auto_deref)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

mod fixture;

type StringCow<'s> = std::borrow::Cow<'s, str>;

fn bench_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("access");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in fixture::SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("String", len), &len, |b, _| {
            let uut = String::from(*fixture);
            let uut = std::hint::black_box(uut);
            b.iter(|| uut.is_empty())
        });
        group.bench_with_input(BenchmarkId::new("Box<str>", len), &len, |b, _| {
            let uut = Box::<str>::from(*fixture);
            let uut = std::hint::black_box(uut);
            b.iter(|| uut.is_empty())
        });
        group.bench_with_input(BenchmarkId::new("Arc<str>", len), &len, |b, _| {
            let uut = std::sync::Arc::<str>::from(*fixture);
            let uut = std::hint::black_box(uut);
            b.iter(|| uut.is_empty())
        });
        group.bench_with_input(BenchmarkId::new("Cow<str>::Owned", len), &len, |b, _| {
            let uut = StringCow::Owned(String::from(*fixture));
            let uut = std::hint::black_box(uut);
            b.iter(|| uut.is_empty())
        });

        group.bench_with_input(
            BenchmarkId::new("compact_str::CompactString::new", len),
            &len,
            |b, _| {
                let uut = compact_str::CompactString::new(fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.is_empty())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("ecow::EcoString::from", len),
            &len,
            |b, _| {
                let uut = ecow::EcoString::from(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.is_empty())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoString::from", len),
            &len,
            |b, _| {
                let uut = turbocow::EcoString::from(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.is_empty())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("hipstr::HipStr::from", len),
            &len,
            |b, _| {
                let uut = hipstr::HipStr::from(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.is_empty())
            },
        );
        group.bench_with_input(
            BenchmarkId::new("lean_string::LeanString::from", len),
            &len,
            |b, _| {
                let uut = lean_string::LeanString::from(*fixture);
                let uut = std::hint::black_box(uut);
                b.iter(|| uut.is_empty())
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_access);
criterion_main!(benches);
