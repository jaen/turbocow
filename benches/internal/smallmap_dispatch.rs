//! Benchmark the SmallMap h2 matcher across N values.
//!
//! `match_h2` (src/small_map/group.rs) scans the h2 sidecar in 16-byte chunks:
//! SSE2 per in-bounds chunk on x86_64, scalar for the final partial chunk (and
//! everywhere else). This measures lookup cost as N grows across chunk
//! boundaries (16 / 32 / 48 / 64) for the hit (early-exit), miss (full scan),
//! and build+scan cases.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use turbocow::SmallMap;

fn bench_match_h2<const N: usize>(c: &mut Criterion, name: &str) {
    let mut group = c.benchmark_group(name);

    // Hit case: lookup an existing key in the middle of the inline storage.
    group.bench_function("hit", |b| {
        let mut map: SmallMap<u64, u32, N> = SmallMap::new();
        for k in 0..N as u64 {
            map.insert(k, k as u32);
        }
        let needle = N as u64 / 2;
        b.iter(|| {
            black_box(map.get(black_box(&needle)));
        });
    });

    // Miss case: lookup an absent key (forces a full h2 scan over all N slots).
    group.bench_function("miss", |b| {
        let mut map: SmallMap<u64, u32, N> = SmallMap::new();
        for k in 0..N as u64 {
            map.insert(k, k as u32);
        }
        b.iter(|| {
            black_box(map.get(black_box(&u64::MAX)));
        });
    });

    // Build + scan cycle.
    group.bench_function("insert_get_cycle", |b| {
        b.iter(|| {
            let mut map: SmallMap<u64, u32, N> = SmallMap::new();
            for k in 0..N as u64 {
                map.insert(k, k as u32);
            }
            for k in 0..N as u64 {
                black_box(map.get(&k));
            }
        });
    });

    group.finish();
}

fn criterion_bench(c: &mut Criterion) {
    bench_match_h2::<8>(c, "n8_scalar");
    bench_match_h2::<15>(c, "n15_scalar_upper");
    bench_match_h2::<16>(c, "n16_one_chunk");
    bench_match_h2::<24>(c, "n24_chunk_plus_tail");
    bench_match_h2::<32>(c, "n32_two_chunks");
    bench_match_h2::<48>(c, "n48_three_chunks");
    bench_match_h2::<64>(c, "n64_four_chunks");
}

criterion_group!(benches, criterion_bench);
criterion_main!(benches);
