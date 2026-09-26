#![allow(
    clippy::clone_on_copy,
    clippy::useless_conversion,
    clippy::explicit_auto_deref,
    noop_method_call
)]

//! Head-to-head benchmarks: turbocow vs ecow — EcoVec operations.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

// ─── new / with_capacity ────────────────────────────────────────────────────

fn bench_ecovec_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/ecovec/new");

    group.bench_function("ecow/empty", |b| {
        b.iter(|| black_box(ecow::EcoVec::<u32>::new()))
    });
    group.bench_function("turbocow/empty", |b| {
        b.iter(|| black_box(turbocow::EcoVec::<u32>::new()))
    });
    group.bench_function("ecow/capacity_16", |b| {
        b.iter(|| black_box(ecow::EcoVec::<u32>::with_capacity(16)))
    });
    group.bench_function("turbocow/capacity_16", |b| {
        b.iter(|| black_box(turbocow::EcoVec::<u32>::with_capacity(16)))
    });
    group.bench_function("ecow/capacity_1024", |b| {
        b.iter(|| black_box(ecow::EcoVec::<u32>::with_capacity(1024)))
    });
    group.bench_function("turbocow/capacity_1024", |b| {
        b.iter(|| black_box(turbocow::EcoVec::<u32>::with_capacity(1024)))
    });

    group.finish();
}

// ─── push ───────────────────────────────────────────────────────────────────

fn bench_ecovec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/ecovec/push");

    group.bench_function("ecow/16", |b| {
        b.iter(|| {
            let mut v = ecow::EcoVec::new();
            for i in 0u32..16 {
                v.push(i);
            }
            black_box(v)
        })
    });
    group.bench_function("turbocow/16", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::new();
            for i in 0u32..16 {
                v.push(i);
            }
            black_box(v)
        })
    });
    group.bench_function("ecow/realloc_64", |b| {
        b.iter(|| {
            let mut v = ecow::EcoVec::with_capacity(8);
            for i in 0u32..64 {
                v.push(i);
            }
            black_box(v)
        })
    });
    group.bench_function("turbocow/realloc_64", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::with_capacity(8);
            for i in 0u32..64 {
                v.push(i);
            }
            black_box(v)
        })
    });

    group.finish();
}

// ─── clone ──────────────────────────────────────────────────────────────────

fn bench_ecovec_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/ecovec/clone");

    let ecow_small: ecow::EcoVec<u32> = (0..16).collect();
    let tc_small: turbocow::EcoVec<u32> = (0..16).collect();
    let ecow_large: ecow::EcoVec<u32> = (0..1024).collect();
    let tc_large: turbocow::EcoVec<u32> = (0..1024).collect();

    group.bench_function("ecow/small_16", |b| b.iter(|| black_box(ecow_small.clone())));
    group.bench_function("turbocow/small_16", |b| b.iter(|| black_box(tc_small.clone())));
    group.bench_function("ecow/large_1024", |b| b.iter(|| black_box(ecow_large.clone())));
    group.bench_function("turbocow/large_1024", |b| {
        b.iter(|| black_box(tc_large.clone()))
    });

    group.finish();
}

// ─── make_mut (COW) ─────────────────────────────────────────────────────────

fn bench_ecovec_make_mut(c: &mut Criterion) {
    let mut group = c.benchmark_group("ecow_cmp/ecovec/make_mut");

    // unique
    group.bench_function("ecow/unique", |b| {
        b.iter(|| {
            let mut v: ecow::EcoVec<u32> = (0..64).collect();
            v.make_mut();
            v.push(999);
            black_box(v)
        })
    });
    group.bench_function("turbocow/unique", |b| {
        b.iter(|| {
            let mut v: turbocow::EcoVec<u32> = (0..64).collect();
            v.make_mut();
            v.push(999);
            black_box(v)
        })
    });

    // shared — triggers COW
    {
        let orig_ecow: ecow::EcoVec<u32> = (0..64).collect();
        let orig_tc: turbocow::EcoVec<u32> = (0..64).collect();
        group.bench_function("ecow/shared", |b| {
            b.iter(|| {
                let mut v = orig_ecow.clone();
                v.make_mut();
                v.push(999);
                black_box(v)
            })
        });
        group.bench_function("turbocow/shared", |b| {
            b.iter(|| {
                let mut v = orig_tc.clone();
                v.make_mut();
                v.push(999);
                black_box(v)
            })
        });
    }

    group.finish();
}

// ─── entry point ───────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_ecovec_new,
    bench_ecovec_push,
    bench_ecovec_clone,
    bench_ecovec_make_mut,
);
criterion_main!(benches);
