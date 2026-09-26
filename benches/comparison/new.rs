#![allow(clippy::clone_on_copy, clippy::useless_conversion)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

mod fixture;

type StringCow<'s> = std::borrow::Cow<'s, str>;

fn bench_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("new");
    group.plot_config(
        criterion::PlotConfiguration::default()
            .summary_scale(criterion::AxisScale::Logarithmic),
    );
    for fixture in fixture::SAMPLES {
        let len = fixture.len();
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("String", len), &len, |b, _| {
            let fixture = std::hint::black_box(*fixture);
            b.iter(|| String::from(fixture))
        });
        group.bench_with_input(BenchmarkId::new("Box<str>", len), &len, |b, _| {
            let fixture = std::hint::black_box(*fixture);
            b.iter(|| Box::<str>::from(fixture))
        });
        group.bench_with_input(BenchmarkId::new("Arc<str>", len), &len, |b, _| {
            let fixture = std::hint::black_box(*fixture);
            b.iter(|| std::sync::Arc::<str>::from(fixture))
        });
        group.bench_with_input(BenchmarkId::new("Cow<str>::Owned", len), &len, |b, _| {
            let fixture = std::hint::black_box(*fixture);
            b.iter(|| StringCow::Owned(String::from(fixture)))
        });

        group.bench_with_input(
            BenchmarkId::new("compact_str::CompactString::new", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| compact_str::CompactString::new(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("ecow::EcoString::from", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| ecow::EcoString::from(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoString::from", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| turbocow::EcoString::from(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("hipstr::HipStr::from", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| hipstr::HipStr::from(fixture))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("lean_string::LeanString::from", len),
            &len,
            |b, _| {
                let fixture = std::hint::black_box(*fixture);
                b.iter(|| lean_string::LeanString::from(fixture))
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_new);
criterion_main!(benches);
