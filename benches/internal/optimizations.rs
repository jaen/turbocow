#![allow(
    clippy::clone_on_copy,
    clippy::useless_conversion,
    clippy::explicit_auto_deref,
    noop_method_call
)]

//! Benchmarks for the optimization pass.
//! Each group targets a specific optimization item.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

// ─── Opt 1: extend_from_copy_slice vs extend_from_slice for Copy types ──────

fn bench_extend_copy_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/extend_copy_slice");
    let data: Vec<u32> = (0..256).collect();

    group.bench_function("extend_from_slice/256xu32", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::<u32>::new();
            v.extend_from_slice(black_box(&data));
            black_box(v)
        })
    });
    group.bench_function("extend_from_copy_slice/256xu32", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::<u32>::new();
            v.extend_from_copy_slice(black_box(&data));
            black_box(v)
        })
    });

    let data_small: Vec<u32> = (0..8).collect();
    group.bench_function("extend_from_slice/8xu32", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::<u32>::new();
            v.extend_from_slice(black_box(&data_small));
            black_box(v)
        })
    });
    group.bench_function("extend_from_copy_slice/8xu32", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::<u32>::new();
            v.extend_from_copy_slice(black_box(&data_small));
            black_box(v)
        })
    });

    group.finish();
}

// ─── Opt 2: make_unique growth headroom ─────────────────────────────────────

fn bench_make_unique_headroom(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/make_unique_headroom");

    // Shared vec → make_unique → push should not realloc immediately.
    let base: turbocow::EcoVec<u32> = (0..64).collect();
    group.bench_function("cow_then_push/64", |b| {
        b.iter(|| {
            let _keep = base.clone();
            let mut v = base.clone();
            v.push(999);
            black_box(v)
        })
    });

    group.finish();
}

// ─── Opt 3: From<&[T]> pre-allocate ────────────────────────────────────────

fn bench_from_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/from_slice");
    let data: Vec<u32> = (0..128).collect();

    group.bench_function("EcoVec::from(&[u32;128])", |b| {
        b.iter(|| {
            let v: turbocow::EcoVec<u32> =
                turbocow::EcoVec::from(black_box(data.as_slice()));
            black_box(v)
        })
    });

    group.finish();
}

// ─── Opt 4: EcoMap insert (make_mut + mem::replace) ────────────────────────

fn bench_eco_map_insert(c: &mut Criterion) {
    use turbocow::EcoMap;
    let mut group = c.benchmark_group("opt/eco_map_insert");

    // Insert into shared map (triggers COW).
    let mut base = EcoMap::<u32, u32>::new();
    for i in 0..16 {
        base.insert(i, i * 10);
    }
    group.bench_function("insert_existing_shared/16", |b| {
        b.iter(|| {
            let _keep = base.clone();
            let mut m = base.clone();
            m.insert(5, 999);
            black_box(m)
        })
    });

    group.bench_function("insert_new/16", |b| {
        b.iter(|| {
            let mut m = EcoMap::<u32, u32>::new();
            for i in 0u32..16 {
                m.insert(i, i);
            }
            black_box(m)
        })
    });

    group.finish();
}

// ─── Opt 5: DynamicVec push/extend Referenced ──────────────────────────────

fn bench_referenced_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/referenced_push");

    group.bench_function("push_after_from_static", |b| {
        b.iter(|| {
            let mut s = turbocow::EcoString::from_static("hello world!!");
            s.push('!');
            black_box(s)
        })
    });

    group.bench_function("extend_after_from_static", |b| {
        b.iter(|| {
            let mut s = turbocow::EcoString::from_static("hello world!!");
            s.push_str(" extended");
            black_box(s)
        })
    });

    group.finish();
}

// ─── Opt 6: Extend<T> push_unchecked fast path ────────────────────────────

fn bench_extend_iter(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/extend_iter");

    group.bench_function("extend/256_items", |b| {
        b.iter(|| {
            let mut v = turbocow::EcoVec::<u32>::new();
            v.extend(0u32..256);
            black_box(v)
        })
    });

    group.bench_function("from_iter/256_items", |b| {
        b.iter(|| {
            let v: turbocow::EcoVec<u32> = (0u32..256).collect();
            black_box(v)
        })
    });

    group.finish();
}

// ─── Opt 7: EcoMap retain single make_mut ──────────────────────────────────

fn bench_eco_map_retain(c: &mut Criterion) {
    use turbocow::EcoMap;
    let mut group = c.benchmark_group("opt/eco_map_retain");

    let mut base = EcoMap::<u32, u32>::new();
    for i in 0..32 {
        base.insert(i, i);
    }

    group.bench_function("retain_shared/32", |b| {
        b.iter(|| {
            let _keep = base.clone();
            let mut m = base.clone();
            m.retain(|k, _| *k % 2 == 0);
            black_box(m)
        })
    });

    group.finish();
}

// ─── Opt 8: SmallMap from_iter size_hint ───────────────────────────────────

fn bench_smallmap_from_iter(c: &mut Criterion) {
    use turbocow::SmallMap;
    let mut group = c.benchmark_group("opt/smallmap_from_iter");

    let data: Vec<(u32, u32)> = (0..32).map(|i| (i, i * 10)).collect();

    group.bench_function("from_iter/32_pairs", |b| {
        b.iter(|| {
            let m: SmallMap<u32, u32> = black_box(data.clone()).into_iter().collect();
            black_box(m)
        })
    });

    let data_small: Vec<(u32, u32)> = (0..4).map(|i| (i, i * 10)).collect();
    group.bench_function("from_iter/4_pairs", |b| {
        b.iter(|| {
            let m: SmallMap<u32, u32> =
                black_box(data_small.clone()).into_iter().collect();
            black_box(m)
        })
    });

    group.finish();
}

// ─── Opt 9: InlineMap clear/Drop needs_drop ────────────────────────────────

fn bench_smallmap_clear(c: &mut Criterion) {
    use turbocow::SmallMap;
    let mut group = c.benchmark_group("opt/smallmap_clear");

    group.bench_function("clear_trivial/8xu32", |b| {
        b.iter(|| {
            let mut m = SmallMap::<u32, u32>::new();
            for i in 0u32..8 {
                m.insert(i, i);
            }
            m.clear();
            black_box(m)
        })
    });

    group.bench_function("drop_trivial/8xu32", |b| {
        b.iter(|| {
            let mut m = SmallMap::<u32, u32>::new();
            for i in 0u32..8 {
                m.insert(i, i);
            }
            black_box(&m);
            drop(m);
        })
    });

    group.finish();
}

// ─── Opt 10: From<String> for EcoString ────────────────────────────────────

fn bench_from_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/from_string");

    let short = "hello".to_string();
    let long = "a]".repeat(32); // 64 bytes, well past inline limit

    group.bench_function("short/5B", |b| {
        b.iter(|| {
            let s = turbocow::EcoString::from(black_box(short.clone()));
            black_box(s)
        })
    });
    group.bench_function("long/64B", |b| {
        b.iter(|| {
            let s = turbocow::EcoString::from(black_box(long.clone()));
            black_box(s)
        })
    });

    group.finish();
}

// ─── Opt 11: to_lowercase/to_uppercase ASCII fast path ─────────────────────

fn bench_case_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/case_conversion");

    let ascii =
        turbocow::EcoString::from("HELLO WORLD THIS IS A TEST STRING FOR BENCHMARKING");
    let mixed = turbocow::EcoString::from("Hello Wörld Thïs Ís A Tëst Strïng");

    group.bench_function("to_lowercase/ascii", |b| {
        b.iter(|| black_box(ascii.to_lowercase()))
    });
    group.bench_function("to_lowercase/mixed", |b| {
        b.iter(|| black_box(mixed.to_lowercase()))
    });
    group.bench_function("to_uppercase/ascii", |b| {
        b.iter(|| black_box(ascii.to_uppercase()))
    });

    group.finish();
}

// ─── Opt 13: replacen capacity hint ────────────────────────────────────────

fn bench_replacen(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/replacen");

    let s = turbocow::EcoString::from("the quick brown fox jumps over the lazy dog");
    group.bench_function("replacen/1", |b| {
        b.iter(|| black_box(s.replacen("the", "a", 1)))
    });
    group.bench_function("replace_all", |b| b.iter(|| black_box(s.replace("the", "a"))));

    group.finish();
}

// ─── Opt 15: from_elem_copy_in ─────────────────────────────────────────────

fn bench_from_elem(c: &mut Criterion) {
    let mut group = c.benchmark_group("opt/from_elem");

    group.bench_function("from_elem/1024xu32", |b| {
        b.iter(|| {
            let v = turbocow::EcoVec::<u32>::from_elem(black_box(42u32), 1024);
            black_box(v)
        })
    });
    group.bench_function("from_elem_copy_in/1024xu32", |b| {
        b.iter(|| {
            let v = turbocow::EcoVec::<u32>::from_elem_copy_in(
                black_box(42u32),
                1024,
                Default::default(),
            );
            black_box(v)
        })
    });

    group.finish();
}

// ─── entry point ────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_extend_copy_slice,
    bench_make_unique_headroom,
    bench_from_slice,
    bench_eco_map_insert,
    bench_referenced_push,
    bench_extend_iter,
    bench_eco_map_retain,
    bench_smallmap_from_iter,
    bench_smallmap_clear,
    bench_from_string,
    bench_case_conversion,
    bench_replacen,
    bench_from_elem,
);
criterion_main!(benches);
