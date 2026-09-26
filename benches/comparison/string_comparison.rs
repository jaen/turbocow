use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

// ─── helpers ───────────────────────────────────────────────────────────────

const SHORT: &str = "hello"; // 5 bytes — inline for all
const MEDIUM: &str = "hello, world!!!"; // 15 bytes — at inline limit
const LONG: &str = "the quick brown fox jumps over the lazy dog and more text here"; // 63 bytes — heap

fn inputs() -> Vec<(&'static str, &'static str)> {
    vec![("short", SHORT), ("medium", MEDIUM), ("long", LONG)]
}

// ─── macros ────────────────────────────────────────────────────────────────

macro_rules! bench_create {
    ($group:expr, $label:expr, $ty:ty, $input:expr) => {
        $group.bench_with_input(BenchmarkId::new($label, ""), &$input, |b, s| {
            b.iter(|| <$ty>::from(black_box(*s)))
        });
    };
}

macro_rules! bench_clone {
    ($group:expr, $label:expr, $ty:ty, $input:expr) => {
        let s = <$ty>::from($input);
        $group.bench_with_input(BenchmarkId::new($label, ""), &s, |b, s| {
            b.iter(|| black_box(s.clone()))
        });
    };
}

macro_rules! bench_push_char {
    ($group:expr, $label:expr, $ty:ty, $input:expr) => {
        $group.bench_with_input(BenchmarkId::new($label, ""), &$input, |b, s| {
            b.iter(|| {
                let mut v = <$ty>::from(black_box(*s));
                v.push('!');
                black_box(v)
            })
        });
    };
}

macro_rules! bench_eq {
    ($group:expr, $label:expr, $ty:ty, $input:expr) => {
        let a = <$ty>::from($input);
        let b = a.clone();
        $group.bench_with_input(
            BenchmarkId::new($label, ""),
            &(a, b),
            |bench, (a, b)| bench.iter(|| black_box(a == b)),
        );
    };
}

// ─── benchmarks ────────────────────────────────────────────────────────────

fn bench_create_from_str(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("create/{name}"));
        bench_create!(group, "turbocow::EcoString", turbocow::EcoString, input);
        bench_create!(group, "ecow::EcoString", ecow::EcoString, input);
        bench_create!(
            group,
            "compact_str::CompactString",
            compact_str::CompactString,
            input
        );
        bench_create!(group, "hipstr::HipStr", hipstr::HipStr, input);
        bench_create!(group, "lean_string::LeanString", lean_string::LeanString, input);
        bench_create!(group, "String", String, input);
        group.finish();
    }
}

fn bench_clone_string(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("clone/{name}"));
        bench_clone!(group, "turbocow::EcoString", turbocow::EcoString, input);
        bench_clone!(group, "ecow::EcoString", ecow::EcoString, input);
        bench_clone!(
            group,
            "compact_str::CompactString",
            compact_str::CompactString,
            input
        );
        bench_clone!(group, "hipstr::HipStr", hipstr::HipStr, input);
        bench_clone!(group, "lean_string::LeanString", lean_string::LeanString, input);
        bench_clone!(group, "String", String, input);
        group.finish();
    }
}

fn bench_push(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("push/{name}"));
        bench_push_char!(group, "turbocow::EcoString", turbocow::EcoString, input);
        bench_push_char!(group, "ecow::EcoString", ecow::EcoString, input);
        bench_push_char!(
            group,
            "compact_str::CompactString",
            compact_str::CompactString,
            input
        );
        bench_push_char!(group, "String", String, input);
        group.finish();
    }
}

fn bench_equality(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("eq/{name}"));
        bench_eq!(group, "turbocow::EcoString", turbocow::EcoString, input);
        bench_eq!(group, "ecow::EcoString", ecow::EcoString, input);
        bench_eq!(group, "compact_str::CompactString", compact_str::CompactString, input);
        bench_eq!(group, "hipstr::HipStr", hipstr::HipStr, input);
        bench_eq!(group, "lean_string::LeanString", lean_string::LeanString, input);
        bench_eq!(group, "String", String, input);
        group.finish();
    }
}

fn bench_push_static(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("push_static/{name}"));
        group.bench_with_input(
            BenchmarkId::new("turbocow::EcoString::from_static", ""),
            &input,
            |b, s| {
                b.iter(|| {
                    let mut v = turbocow::EcoString::from_static(black_box(*s));
                    v.push('!');
                    black_box(v)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("hipstr::HipStr::borrowed", ""),
            &input,
            |b, s| {
                b.iter(|| {
                    let mut v = hipstr::HipStr::borrowed(black_box(*s));
                    v.push('!');
                    black_box(v)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("lean_string::LeanString::from_static_str", ""),
            &input,
            |b, s| {
                b.iter(|| {
                    let mut v = lean_string::LeanString::from_static_str(black_box(*s));
                    v.push('!');
                    black_box(v)
                })
            },
        );
        group.finish();
    }
}

fn bench_clone_mutate_static(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("clone_mutate_static/{name}"));
        // turbocow Referenced
        {
            let s = turbocow::EcoString::from_static(input);
            group.bench_with_input(
                BenchmarkId::new("turbocow::EcoString::from_static", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        // HipStr borrowed
        {
            let s = hipstr::HipStr::borrowed(input);
            group.bench_with_input(
                BenchmarkId::new("hipstr::HipStr::borrowed", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        // LeanString from_static_str
        {
            let s = lean_string::LeanString::from_static_str(input);
            group.bench_with_input(
                BenchmarkId::new("lean_string::LeanString::from_static_str", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        group.finish();
    }
}

fn bench_clone_then_mutate(c: &mut Criterion) {
    for (name, input) in inputs() {
        let mut group = c.benchmark_group(format!("clone_mutate/{name}"));

        // turbocow
        {
            let s = turbocow::EcoString::from(input);
            group.bench_with_input(
                BenchmarkId::new("turbocow::EcoString", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        // ecow
        {
            let s = ecow::EcoString::from(input);
            group.bench_with_input(
                BenchmarkId::new("ecow::EcoString", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        // CompactString
        {
            let s = compact_str::CompactString::from(input);
            group.bench_with_input(
                BenchmarkId::new("compact_str::CompactString", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        // HipStr
        {
            let s = hipstr::HipStr::from(input);
            group.bench_with_input(BenchmarkId::new("hipstr::HipStr", ""), &s, |b, s| {
                b.iter(|| {
                    let mut c = s.clone();
                    c.push('!');
                    black_box(c)
                })
            });
        }
        // LeanString
        {
            let s = lean_string::LeanString::from(input);
            group.bench_with_input(
                BenchmarkId::new("lean_string::LeanString", ""),
                &s,
                |b, s| {
                    b.iter(|| {
                        let mut c = s.clone();
                        c.push('!');
                        black_box(c)
                    })
                },
            );
        }
        // String
        {
            let s = String::from(input);
            group.bench_with_input(BenchmarkId::new("String", ""), &s, |b, s| {
                b.iter(|| {
                    let mut c = s.clone();
                    c.push('!');
                    black_box(c)
                })
            });
        }
        group.finish();
    }
}

criterion_group!(
    benches,
    bench_create_from_str,
    bench_clone_string,
    bench_push,
    bench_push_static,
    bench_equality,
    bench_clone_then_mutate,
    bench_clone_mutate_static,
);
criterion_main!(benches);
