#![allow(
    clippy::clone_on_copy,
    clippy::useless_conversion,
    clippy::explicit_auto_deref,
    noop_method_call
)]

//! Head-to-head benchmarks: turbocow vs ecow — mutation operations
//! (push, push_str, make_mut, clone+mutate).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const SHORT: &str = "hello";
const MEDIUM: &str = "hello, world!!!";
const LONG: &str = "the quick brown fox jumps over the lazy dog and more text here";

fn sized_inputs() -> Vec<(&'static str, &'static str)> {
    vec![("short", SHORT), ("medium", MEDIUM), ("long", LONG)]
}

// ─── push (owned) ──────────────────────────────────────────────────────────

fn bench_push(c: &mut Criterion) {
    for (name, input) in sized_inputs() {
        let mut group = c.benchmark_group(format!("ecow_cmp/push/{name}"));
        group.bench_with_input(BenchmarkId::new("ecow", ""), &input, |b, s| {
            b.iter(|| {
                let mut v = ecow::EcoString::from(black_box(*s));
                v.push('!');
                black_box(v)
            })
        });
        group.bench_with_input(BenchmarkId::new("turbocow", ""), &input, |b, s| {
            b.iter(|| {
                let mut v = turbocow::EcoString::from(black_box(*s));
                v.push('!');
                black_box(v)
            })
        });
        group.finish();
    }
}

// ─── push (static / referenced) ────────────────────────────────────────────

fn bench_push_static(c: &mut Criterion) {
    for (name, input) in sized_inputs() {
        let mut group = c.benchmark_group(format!("ecow_cmp/push_static/{name}"));
        group.bench_with_input(BenchmarkId::new("ecow::from", ""), &input, |b, s| {
            b.iter(|| {
                let mut v = ecow::EcoString::from(black_box(*s));
                v.push('!');
                black_box(v)
            })
        });
        group.bench_with_input(
            BenchmarkId::new("turbocow::from_static", ""),
            &input,
            |b, s| {
                b.iter(|| {
                    let mut v = turbocow::EcoString::from_static(black_box(*s));
                    v.push('!');
                    black_box(v)
                })
            },
        );
        group.finish();
    }
}

// ─── push_str (incremental building) ────────────────────────────────────────

fn bench_push_str(c: &mut Criterion) {
    for (name, input) in sized_inputs() {
        let mut group = c.benchmark_group(format!("ecow_cmp/push_str/{name}"));
        group.bench_function("ecow", |b| {
            b.iter(|| {
                let mut s = ecow::EcoString::new();
                for _ in 0..8 {
                    s.push_str(black_box(input));
                }
                black_box(s)
            })
        });
        group.bench_function("turbocow", |b| {
            b.iter(|| {
                let mut s = turbocow::EcoString::new();
                for _ in 0..8 {
                    s.push_str(black_box(input));
                }
                black_box(s)
            })
        });
        group.finish();
    }
}

// ─── clone then mutate (owned) ─────────────────────────────────────────────

fn bench_clone_mutate(c: &mut Criterion) {
    for (name, input) in sized_inputs() {
        let mut group = c.benchmark_group(format!("ecow_cmp/clone_mutate/{name}"));
        {
            let s = ecow::EcoString::from(input);
            group.bench_with_input(BenchmarkId::new("ecow", ""), &s, |b, s| {
                b.iter(|| {
                    let mut c = s.clone();
                    c.push('!');
                    black_box(c)
                })
            });
        }
        {
            let s = turbocow::EcoString::from(input);
            group.bench_with_input(BenchmarkId::new("turbocow", ""), &s, |b, s| {
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

// ─── clone then mutate (static / referenced) ───────────────────────────────

fn bench_clone_mutate_static(c: &mut Criterion) {
    for (name, input) in sized_inputs() {
        let mut group = c.benchmark_group(format!("ecow_cmp/clone_mutate_static/{name}"));
        {
            let s = ecow::EcoString::from(input);
            group.bench_with_input(BenchmarkId::new("ecow::from", ""), &s, |b, s| {
                b.iter(|| {
                    let mut c = s.clone();
                    c.push('!');
                    black_box(c)
                })
            });
        }
        {
            let s = turbocow::EcoString::from_static(input);
            group.bench_with_input(
                BenchmarkId::new("turbocow::from_static", ""),
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

// ─── make_mut (COW behaviour) ───────────────────────────────────────────────

fn bench_make_mut(c: &mut Criterion) {
    for (name, input) in sized_inputs() {
        let mut group = c.benchmark_group(format!("ecow_cmp/make_mut/{name}"));

        group.bench_function("ecow/unique", |b| {
            b.iter(|| {
                let mut s = ecow::EcoString::from(black_box(input));
                s.make_mut();
                s.push_str(" test");
                black_box(s)
            })
        });
        group.bench_function("turbocow/unique", |b| {
            b.iter(|| {
                let mut s = turbocow::EcoString::from(black_box(input));
                s.make_mut();
                s.push_str(" test");
                black_box(s)
            })
        });

        {
            let orig_ecow = ecow::EcoString::from(input);
            group.bench_function("ecow/shared", |b| {
                b.iter(|| {
                    let mut s = orig_ecow.clone();
                    s.make_mut();
                    s.push_str(" test");
                    black_box(s)
                })
            });
        }
        {
            let orig_tc = turbocow::EcoString::from(input);
            group.bench_function("turbocow/shared", |b| {
                b.iter(|| {
                    let mut s = orig_tc.clone();
                    s.make_mut();
                    s.push_str(" test");
                    black_box(s)
                })
            });
        }

        group.finish();
    }
}

criterion_group!(
    benches,
    bench_push,
    bench_push_static,
    bench_push_str,
    bench_clone_mutate,
    bench_clone_mutate_static,
    bench_make_mut,
);
criterion_main!(benches);
