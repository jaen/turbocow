#![allow(clippy::clone_on_copy, clippy::useless_conversion)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

mod fixture;

type StringCow<'s> = std::borrow::Cow<'s, str>;

fn bench_new_static(c: &mut Criterion) {
    let mut group = c.benchmark_group("new_static");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in fixture::SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(
            BenchmarkId::new("Cow<str>::Borrowed", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| StringCow::Borrowed(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("compact_str::CompactString::const_new", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| compact_str::CompactString::const_new(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("hipstr::HipStr::borrowed", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| hipstr::HipStr::borrowed(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoString::from_static", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| turbocow::EcoString::from_static(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoStr::from", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| turbocow::EcoStr::from(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("lean_string::LeanString::from_static_str", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| lean_string::LeanString::from_static_str(fixture))
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_new_static);
criterion_main!(benches);
