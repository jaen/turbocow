//! Benchmark: SmallMap (inline array) vs EcoMap (refcounted 2×EcoVec).
//!
//! # Background
//!
//! `SmallMap<K, V, N>` stores up to N entries in an inline `[MaybeUninit<(K,V)>; N]`
//! array.  `Clone` copies every element — O(N) element clones.
//!
//! `EcoMap<K, V>` is backed by two `EcoVec`s (keys + values) and gets O(1) clone
//! via atomic refcount increment — regardless of entry count.  The trade-off is
//! that mutation triggers copy-on-write (COW), so inserts into a shared map
//! are more expensive.
//!
//! This benchmark quantifies the difference across the operations that matter:
//!
//! - **clone**: EcoMap wins — O(1) vs O(N) element copies.
//! - **insert (unique owner)**: SmallMap wins — direct array write vs EcoVec push.
//! - **lookup**: Comparable for small N; SmallMap has h2 sidecar for N>3.
//! - **insert-after-clone (shared)**: EcoMap pays COW; SmallMap already copied.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use turbocow::{EcoMap, EcoString, SmallMap};

// ── Fixture: realistic ninja-like bindings ───────────────────────────────

/// Build a set of (key, value) pairs resembling ninja rule bindings.
fn bindings(n: usize) -> Vec<(EcoString, EcoString)> {
    let names = [
        "command",
        "depfile",
        "deps",
        "description",
        "dyndep",
        "generator",
        "restat",
        "rspfile",
        "rspfile_content",
        "msvc_deps_prefix",
        "pool",
        "symlink_outputs",
    ];
    names[..n]
        .iter()
        .map(|&k| (EcoString::from(k), EcoString::from("some_value")))
        .collect()
}

fn build_small_map(entries: &[(EcoString, EcoString)]) -> SmallMap<EcoString, EcoString> {
    let mut m = SmallMap::new();
    for (k, v) in entries {
        m.insert(k.clone(), v.clone());
    }
    m
}

fn build_eco_map(entries: &[(EcoString, EcoString)]) -> EcoMap<EcoString, EcoString> {
    let mut m = EcoMap::new();
    for (k, v) in entries {
        m.insert(k.clone(), v.clone());
    }
    m
}

// ── Benchmarks ──────────────────────────────────────────────────────────

fn bench_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_clone");
    for n in [2, 4, 6, 8] {
        let entries = bindings(n);

        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(&entries);
            let map = black_box(&map);
            b.iter(|| map.clone())
        });

        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(&entries);
            let map = black_box(&map);
            b.iter(|| map.clone())
        });
    }
    group.finish();
}

fn bench_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_lookup");
    for n in [2, 4, 6, 8] {
        let entries = bindings(n);
        let needle = &entries[n / 2].0;

        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(&entries);
            b.iter(|| map.get(black_box(needle.as_str())))
        });

        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(&entries);
            b.iter(|| map.get(black_box(needle.as_str())))
        });
    }
    group.finish();
}

fn bench_insert_unique_owner(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_insert_unique");
    for n in [2, 4, 6, 8] {
        let entries = bindings(n);

        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            b.iter(|| build_small_map(black_box(&entries)))
        });

        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            b.iter(|| build_eco_map(black_box(&entries)))
        });
    }
    group.finish();
}

fn bench_clone_then_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_clone_then_lookup");
    for n in [2, 4, 6, 8] {
        let entries = bindings(n);
        let needle = &entries[n / 2].0;

        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(&entries);
            b.iter(|| {
                let cloned = black_box(&map).clone();
                black_box(cloned.get(needle.as_str()));
            })
        });

        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(&entries);
            b.iter(|| {
                let cloned = black_box(&map).clone();
                black_box(cloned.get(needle.as_str()));
            })
        });
    }
    group.finish();
}

fn bench_clone_then_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_clone_then_insert");
    for n in [2, 4, 6, 8] {
        let entries = bindings(n);
        let new_key = EcoString::from("extra_key");
        let new_val = EcoString::from("extra_val");

        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(&entries);
            b.iter(|| {
                let mut cloned = black_box(&map).clone();
                cloned.insert(new_key.clone(), new_val.clone())
            })
        });

        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(&entries);
            b.iter(|| {
                let mut cloned = black_box(&map).clone();
                cloned.insert(new_key.clone(), new_val.clone())
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_clone,
    bench_lookup,
    bench_insert_unique_owner,
    bench_clone_then_lookup,
    bench_clone_then_insert,
);
criterion_main!(benches);
