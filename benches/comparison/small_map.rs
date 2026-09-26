//! Benchmark: SmallMap, SmallSet, EcoMap, EcoSet vs std/hashbrown baselines
//! and external small-/persistent-map crates.
//!
//! Tests insert, lookup, remove, iterate, clone across sizes 2..512
//! with u64 keys/values to isolate map overhead from element costs.
//! Also tests with large values ([u64; 8]) to measure cache pressure.
//!
//! External competitors:
//!   * `litemap::LiteMap`     — sorted-Vec map (dynamic, needs `Ord`).
//!   * `im::HashMap`/`HashSet` — persistent HAMT with cheap, structural-sharing
//!     clone (the headline `smap_clone` / `sset_clone` contestant).
//!   * `micromap::Map`        — fixed-capacity flat map (panics past `MICRO_CAP`).
//!   * `heapless::LinearMap`  — fixed-capacity linear map (`insert` is fallible).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::collections::HashMap;
use std::hint::black_box;

use turbocow::{EcoMap, EcoSet, SmallMap, SmallSet};

// ── Helpers ──────────────────────────────────────────────────────────────

fn build_small_map(n: usize) -> SmallMap<u64, u64> {
    let mut m = SmallMap::new();
    for i in 0..n {
        m.insert(i as u64, i as u64 * 10);
    }
    m
}

fn build_eco_map(n: usize) -> EcoMap<u64, u64> {
    let mut m = EcoMap::new();
    for i in 0..n {
        m.insert(i as u64, i as u64 * 10);
    }
    m
}

// Same loop as build_eco_map, but via the no-dedup-scan fast path: the only
// difference is insert (O(n²) over the build, scans for an existing key each
// call) vs insert_unique (O(1) append, caller guarantees distinct keys).
fn build_eco_map_unique(n: usize) -> EcoMap<u64, u64> {
    let mut m = EcoMap::new();
    for i in 0..n {
        m.insert_unique(i as u64, i as u64 * 10);
    }
    m
}

fn build_hash_map(n: usize) -> HashMap<u64, u64> {
    let mut m = HashMap::new();
    for i in 0..n {
        m.insert(i as u64, i as u64 * 10);
    }
    m
}

fn build_litemap(n: usize) -> litemap::LiteMap<u64, u64> {
    let mut m = litemap::LiteMap::new();
    for i in 0..n {
        m.insert(i as u64, i as u64 * 10);
    }
    m
}

fn build_im_map(n: usize) -> im::HashMap<u64, u64> {
    let mut m = im::HashMap::new();
    for i in 0..n {
        m.insert(i as u64, i as u64 * 10);
    }
    m
}

// micromap / heapless are fixed-capacity: build helpers are const-generic over
// the capacity and only used at sizes `n <= MICRO_CAP`.
fn build_micromap<const CAP: usize>(n: usize) -> micromap::Map<u64, u64, CAP> {
    let mut m = micromap::Map::new();
    for i in 0..n {
        m.insert(i as u64, i as u64 * 10);
    }
    m
}

fn build_heapless_map<const CAP: usize>(n: usize) -> heapless::LinearMap<u64, u64, CAP> {
    let mut m = heapless::LinearMap::new();
    for i in 0..n {
        // `insert` returns Err((k, v)) when full; `n <= CAP` guarantees room.
        m.insert(i as u64, i as u64 * 10).unwrap();
    }
    m
}

fn build_small_set(n: usize) -> SmallSet<u64> {
    let mut s = SmallSet::new();
    for i in 0..n {
        s.insert(i as u64);
    }
    s
}

fn build_eco_set(n: usize) -> EcoSet<u64> {
    let mut s = EcoSet::new();
    for i in 0..n {
        s.insert(i as u64);
    }
    s
}

fn build_hash_set(n: usize) -> std::collections::HashSet<u64> {
    let mut s = std::collections::HashSet::new();
    for i in 0..n {
        s.insert(i as u64);
    }
    s
}

fn build_im_set(n: usize) -> im::HashSet<u64> {
    let mut s = im::HashSet::new();
    for i in 0..n {
        s.insert(i as u64);
    }
    s
}

const SIZES: &[usize] = &[2, 4, 8, 16, 32, 64, 128, 256, 512];
const SIZES_SMALL: &[usize] = &[4, 8, 16, 32, 64, 128, 256, 512];

// Fixed capacity for micromap / heapless; their arms are gated to `n <= MICRO_CAP`.
const MICRO_CAP: usize = 64;

// ═════════════════════════════════════════════════════════════════════════
// Map benchmarks
// ═════════════════════════════════════════════════════════════════════════

// ── Insert N entries from scratch ────────────────────────────────────────

fn bench_map_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_insert");
    for &n in SIZES {
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, &n| {
            b.iter(|| build_small_map(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, &n| {
            b.iter(|| build_eco_map(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("EcoMap (unique)", n), &n, |b, &n| {
            b.iter(|| build_eco_map_unique(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, &n| {
            b.iter(|| build_hash_map(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, &n| {
            b.iter(|| build_litemap(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, &n| {
            b.iter(|| build_im_map(black_box(n)));
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, &n| {
                b.iter(|| build_micromap::<MICRO_CAP>(black_box(n)));
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, &n| {
                    b.iter(|| build_heapless_map::<MICRO_CAP>(black_box(n)));
                },
            );
        }
    }
    group.finish();
}

// ── Lookup (hit in the middle) ──────────────────────────────────────────

fn bench_map_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_lookup");
    for &n in SIZES {
        let needle = (n / 2) as u64;
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, _| {
            let map = build_hash_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, _| {
            let map = build_litemap(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, _| {
            let map = build_im_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, _| {
                let map = build_micromap::<MICRO_CAP>(n);
                b.iter(|| map.get(black_box(&needle)));
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, _| {
                    let map = build_heapless_map::<MICRO_CAP>(n);
                    b.iter(|| map.get(black_box(&needle)));
                },
            );
        }
    }
    group.finish();
}

// ── Lookup (miss) ───────────────────────────────────────────────────────

fn bench_map_lookup_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_lookup_miss");
    for &n in SIZES {
        let needle = 9999u64;
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, _| {
            let map = build_hash_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, _| {
            let map = build_litemap(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, _| {
            let map = build_im_map(n);
            b.iter(|| map.get(black_box(&needle)));
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, _| {
                let map = build_micromap::<MICRO_CAP>(n);
                b.iter(|| map.get(black_box(&needle)));
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, _| {
                    let map = build_heapless_map::<MICRO_CAP>(n);
                    b.iter(|| map.get(black_box(&needle)));
                },
            );
        }
    }
    group.finish();
}

// ── Remove ──────────────────────────────────────────────────────────────

fn bench_map_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_remove");
    for &n in SIZES_SMALL {
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, &n| {
            b.iter_batched(
                || build_small_map(n),
                |mut map| {
                    black_box(map.remove(black_box(&((n / 2) as u64))));
                    map
                },
                criterion::BatchSize::SmallInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, &n| {
            b.iter_batched(
                || build_eco_map(n),
                |mut map| {
                    black_box(map.remove(black_box(&((n / 2) as u64))));
                    map
                },
                criterion::BatchSize::SmallInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, &n| {
            b.iter_batched(
                || build_hash_map(n),
                |mut map| {
                    black_box(map.remove(black_box(&((n / 2) as u64))));
                    map
                },
                criterion::BatchSize::SmallInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, &n| {
            b.iter_batched(
                || build_litemap(n),
                |mut map| {
                    black_box(map.remove(black_box(&((n / 2) as u64))));
                    map
                },
                criterion::BatchSize::SmallInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, &n| {
            b.iter_batched(
                || build_im_map(n),
                |mut map| {
                    black_box(map.remove(black_box(&((n / 2) as u64))));
                    map
                },
                criterion::BatchSize::SmallInput,
            );
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, &n| {
                b.iter_batched(
                    || build_micromap::<MICRO_CAP>(n),
                    |mut map| {
                        black_box(map.remove(black_box(&((n / 2) as u64))));
                        map
                    },
                    criterion::BatchSize::SmallInput,
                );
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, &n| {
                    b.iter_batched(
                        || build_heapless_map::<MICRO_CAP>(n),
                        |mut map| {
                            black_box(map.remove(black_box(&((n / 2) as u64))));
                            map
                        },
                        criterion::BatchSize::SmallInput,
                    );
                },
            );
        }
    }
    group.finish();
}

// ── Iterate (sum values) ────────────────────────────────────────────────

fn bench_map_iterate(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_iterate");
    for &n in SIZES {
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(n);
            b.iter(|| -> u64 { black_box(map.values().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(n);
            b.iter(|| -> u64 { black_box(map.values().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, _| {
            let map = build_hash_map(n);
            b.iter(|| -> u64 { black_box(map.values().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, _| {
            let map = build_litemap(n);
            b.iter(|| -> u64 { black_box(map.values().copied().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, _| {
            let map = build_im_map(n);
            b.iter(|| -> u64 { black_box(map.values().sum()) });
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, _| {
                let map = build_micromap::<MICRO_CAP>(n);
                b.iter(|| -> u64 { black_box(map.values().sum()) });
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, _| {
                    let map = build_heapless_map::<MICRO_CAP>(n);
                    b.iter(|| -> u64 { black_box(map.values().sum()) });
                },
            );
        }
    }
    group.finish();
}

// ── Clone ───────────────────────────────────────────────────────────────

fn bench_map_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_clone");
    for &n in SIZES {
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let map = build_small_map(n);
            b.iter(|| black_box(&map).clone());
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let map = build_eco_map(n);
            b.iter(|| black_box(&map).clone());
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, _| {
            let map = build_hash_map(n);
            b.iter(|| black_box(&map).clone());
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, _| {
            let map = build_litemap(n);
            b.iter(|| black_box(&map).clone());
        });
        // im::HashMap's structural-sharing clone is its headline strength.
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, _| {
            let map = build_im_map(n);
            b.iter(|| black_box(&map).clone());
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, _| {
                let map = build_micromap::<MICRO_CAP>(n);
                b.iter(|| black_box(&map).clone());
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, _| {
                    let map = build_heapless_map::<MICRO_CAP>(n);
                    b.iter(|| black_box(&map).clone());
                },
            );
        }
    }
    group.finish();
}

// ── Insert existing key (update) ────────────────────────────────────────

fn bench_map_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_update");
    for &n in SIZES_SMALL {
        let key = (n / 2) as u64;
        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, _| {
            let mut map = build_small_map(n);
            b.iter(|| {
                black_box(map.insert(black_box(key), black_box(999)));
            });
        });
        group.bench_with_input(BenchmarkId::new("EcoMap", n), &n, |b, _| {
            let mut map = build_eco_map(n);
            b.iter(|| {
                black_box(map.insert(black_box(key), black_box(999)));
            });
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, _| {
            let mut map = build_hash_map(n);
            b.iter(|| {
                black_box(map.insert(black_box(key), black_box(999)));
            });
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, _| {
            let mut map = build_litemap(n);
            b.iter(|| {
                black_box(map.insert(black_box(key), black_box(999)));
            });
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, _| {
            let mut map = build_im_map(n);
            b.iter(|| {
                black_box(map.insert(black_box(key), black_box(999)));
            });
        });
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, _| {
                let mut map = build_micromap::<MICRO_CAP>(n);
                b.iter(|| {
                    black_box(map.insert(black_box(key), black_box(999)));
                });
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, _| {
                    let mut map = build_heapless_map::<MICRO_CAP>(n);
                    b.iter(|| {
                        black_box(map.insert(black_box(key), black_box(999)).unwrap());
                    });
                },
            );
        }
    }
    group.finish();
}

// ── Lookup with large values (cache pressure test) ──────────────────────

fn bench_map_lookup_large_val(c: &mut Criterion) {
    let mut group = c.benchmark_group("smap_lookup_large_val");
    type BigVal = [u64; 8];

    for &n in SIZES_SMALL {
        let needle = (n / 2) as u64;

        group.bench_with_input(BenchmarkId::new("SmallMap", n), &n, |b, &n| {
            let mut map = SmallMap::<u64, BigVal>::new();
            for i in 0..n {
                map.insert(i as u64, [i as u64; 8]);
            }
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("HashMap", n), &n, |b, &n| {
            let mut map = HashMap::<u64, BigVal>::new();
            for i in 0..n {
                map.insert(i as u64, [i as u64; 8]);
            }
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("litemap", n), &n, |b, &n| {
            let mut map = litemap::LiteMap::<u64, BigVal>::new();
            for i in 0..n {
                map.insert(i as u64, [i as u64; 8]);
            }
            b.iter(|| map.get(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("im::HashMap", n), &n, |b, &n| {
            let mut map = im::HashMap::<u64, BigVal>::new();
            for i in 0..n {
                map.insert(i as u64, [i as u64; 8]);
            }
            b.iter(|| map.get(black_box(&needle)));
        });
        // micromap / heapless: fixed-capacity with large inline [u64; 8] values
        // compiles cleanly; include them at n <= MICRO_CAP.
        if n <= MICRO_CAP {
            group.bench_with_input(BenchmarkId::new("micromap", n), &n, |b, &n| {
                let mut map = micromap::Map::<u64, BigVal, MICRO_CAP>::new();
                for i in 0..n {
                    map.insert(i as u64, [i as u64; 8]);
                }
                b.iter(|| map.get(black_box(&needle)));
            });
            group.bench_with_input(
                BenchmarkId::new("heapless::LinearMap", n),
                &n,
                |b, &n| {
                    let mut map = heapless::LinearMap::<u64, BigVal, MICRO_CAP>::new();
                    for i in 0..n {
                        map.insert(i as u64, [i as u64; 8]).unwrap();
                    }
                    b.iter(|| map.get(black_box(&needle)));
                },
            );
        }
    }
    group.finish();
}

// ═════════════════════════════════════════════════════════════════════════
// Set benchmarks
// ═════════════════════════════════════════════════════════════════════════
//
// Only `im::HashSet` joins the set benches: litemap, micromap, and heapless
// ship no suitable set type (litemap is map-only; micromap/heapless expose
// only maps), so there is nothing to compare against for them here.

fn bench_set_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sset_insert");
    for &n in SIZES {
        group.bench_with_input(BenchmarkId::new("SmallSet", n), &n, |b, &n| {
            b.iter(|| build_small_set(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("EcoSet", n), &n, |b, &n| {
            b.iter(|| build_eco_set(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("HashSet", n), &n, |b, &n| {
            b.iter(|| build_hash_set(black_box(n)));
        });
        group.bench_with_input(BenchmarkId::new("im::HashSet", n), &n, |b, &n| {
            b.iter(|| build_im_set(black_box(n)));
        });
    }
    group.finish();
}

fn bench_set_contains(c: &mut Criterion) {
    let mut group = c.benchmark_group("sset_contains");
    for &n in SIZES {
        let needle = (n / 2) as u64;
        group.bench_with_input(BenchmarkId::new("SmallSet", n), &n, |b, _| {
            let set = build_small_set(n);
            b.iter(|| set.contains(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("EcoSet", n), &n, |b, _| {
            let set = build_eco_set(n);
            b.iter(|| set.contains(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("HashSet", n), &n, |b, _| {
            let set = build_hash_set(n);
            b.iter(|| set.contains(black_box(&needle)));
        });
        group.bench_with_input(BenchmarkId::new("im::HashSet", n), &n, |b, _| {
            let set = build_im_set(n);
            b.iter(|| set.contains(black_box(&needle)));
        });
    }
    group.finish();
}

fn bench_set_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("sset_clone");
    for &n in SIZES {
        group.bench_with_input(BenchmarkId::new("SmallSet", n), &n, |b, _| {
            let set = build_small_set(n);
            b.iter(|| black_box(&set).clone());
        });
        group.bench_with_input(BenchmarkId::new("EcoSet", n), &n, |b, _| {
            let set = build_eco_set(n);
            b.iter(|| black_box(&set).clone());
        });
        group.bench_with_input(BenchmarkId::new("HashSet", n), &n, |b, _| {
            let set = build_hash_set(n);
            b.iter(|| black_box(&set).clone());
        });
        group.bench_with_input(BenchmarkId::new("im::HashSet", n), &n, |b, _| {
            let set = build_im_set(n);
            b.iter(|| black_box(&set).clone());
        });
    }
    group.finish();
}

fn bench_set_iterate(c: &mut Criterion) {
    let mut group = c.benchmark_group("sset_iterate");
    for &n in SIZES {
        group.bench_with_input(BenchmarkId::new("SmallSet", n), &n, |b, _| {
            let set = build_small_set(n);
            b.iter(|| -> u64 { black_box(set.iter().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("EcoSet", n), &n, |b, _| {
            let set = build_eco_set(n);
            b.iter(|| -> u64 { black_box(set.iter().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("HashSet", n), &n, |b, _| {
            let set = build_hash_set(n);
            b.iter(|| -> u64 { black_box(set.iter().sum()) });
        });
        group.bench_with_input(BenchmarkId::new("im::HashSet", n), &n, |b, _| {
            let set = build_im_set(n);
            b.iter(|| -> u64 { black_box(set.iter().copied().sum()) });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_map_insert,
    bench_map_lookup,
    bench_map_lookup_miss,
    bench_map_remove,
    bench_map_iterate,
    bench_map_clone,
    bench_map_update,
    bench_map_lookup_large_val,
    bench_set_insert,
    bench_set_contains,
    bench_set_clone,
    bench_set_iterate,
);
criterion_main!(benches);
