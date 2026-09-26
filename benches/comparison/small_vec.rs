use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use smallvec::SmallVec as ExtSmallVec;
use std::hint::black_box;
use turbocow::SmallVec;

// ── Push N elements ─────────────────────────────────────────────────────

fn bench_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("push");

    for &n in &[4, 8, 16, 64, 256, 1024] {
        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &n, |b, &n| {
            b.iter(|| {
                let mut v = SmallVec::<u64, 8>::new();
                for i in 0..n {
                    v.push(black_box(i as u64));
                }
                v
            });
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &n,
            |b, &n| {
                b.iter(|| {
                    let mut v = ExtSmallVec::<[u64; 8]>::new();
                    for i in 0..n {
                        v.push(black_box(i as u64));
                    }
                    v
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &n, |b, &n| {
            b.iter(|| {
                let mut v = Vec::<u64>::new();
                for i in 0..n {
                    v.push(black_box(i as u64));
                }
                v
            });
        });
    }

    group.finish();
}

// ── Clone ───────────────────────────────────────────────────────────────

fn bench_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone");

    for &n in &[4, 8, 16, 64, 256] {
        let sv: SmallVec<u64, 8> = (0..n).map(|i| i as u64).collect();
        let ext: ExtSmallVec<[u64; 8]> = (0..n).map(|i| i as u64).collect();
        let vec: Vec<u64> = (0..n).map(|i| i as u64).collect();

        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &sv, |b, sv| {
            b.iter(|| black_box(sv.clone()));
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &ext,
            |b, ext| {
                b.iter(|| black_box(ext.clone()));
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &vec, |b, vec| {
            b.iter(|| black_box(vec.clone()));
        });
    }

    group.finish();
}

// ── Clone with a non-Copy (heap-allocating) element ─────────────────────
//
// With `String` elements each clone must deep-clone every element, isolating
// element-clone cost across inline and spilled storage — the regime where
// turbocow's spilled storage and smallvec's eager element clone differ most.
fn bench_clone_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone_string");

    for &n in &[4, 8, 16, 64, 256] {
        let sv: SmallVec<String, 8> = (0..n).map(|i| format!("item-{i:04}")).collect();
        let ext: ExtSmallVec<[String; 8]> =
            (0..n).map(|i| format!("item-{i:04}")).collect();
        let vec: Vec<String> = (0..n).map(|i| format!("item-{i:04}")).collect();

        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &sv, |b, sv| {
            b.iter(|| black_box(sv.clone()));
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &ext,
            |b, ext| {
                b.iter(|| black_box(ext.clone()));
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &vec, |b, vec| {
            b.iter(|| black_box(vec.clone()));
        });
    }

    group.finish();
}

// ── Iterate (sum) ───────────────────────────────────────────────────────

fn bench_iter_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_sum");

    for &n in &[8, 64, 256, 1024] {
        let sv: SmallVec<u64, 8> = (0..n).map(|i| i as u64).collect();
        let ext: ExtSmallVec<[u64; 8]> = (0..n).map(|i| i as u64).collect();
        let vec: Vec<u64> = (0..n).map(|i| i as u64).collect();

        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &sv, |b, sv| {
            b.iter(|| -> u64 { black_box(sv.iter().sum()) });
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &ext,
            |b, ext| {
                b.iter(|| -> u64 { black_box(ext.iter().sum()) });
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &vec, |b, vec| {
            b.iter(|| -> u64 { black_box(vec.iter().sum()) });
        });
    }

    group.finish();
}

// ── Lookup by index ─────────────────────────────────────────────────────

fn bench_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_access");

    for &n in &[4, 8, 64, 256] {
        let sv: SmallVec<u64, 8> = (0..n).map(|i| i as u64).collect();
        let ext: ExtSmallVec<[u64; 8]> = (0..n).map(|i| i as u64).collect();
        let vec: Vec<u64> = (0..n).map(|i| i as u64).collect();

        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &sv, |b, sv| {
            b.iter(|| {
                let mut sum = 0u64;
                for i in 0..sv.len() {
                    sum += black_box(sv[i]);
                }
                sum
            });
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &ext,
            |b, ext| {
                b.iter(|| {
                    let mut sum = 0u64;
                    for i in 0..ext.len() {
                        sum += black_box(ext[i]);
                    }
                    sum
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &vec, |b, vec| {
            b.iter(|| {
                let mut sum = 0u64;
                // Indexed access is the operation being benchmarked here,
                // kept identical to the SmallVec/smallvec cases above;
                // clippy only flags this Vec case because it recognises Vec.
                #[allow(clippy::needless_range_loop)]
                for i in 0..vec.len() {
                    sum += black_box(vec[i]);
                }
                sum
            });
        });
    }

    group.finish();
}

// ── Push then pop all ───────────────────────────────────────────────────

fn bench_push_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_pop");

    for &n in &[4, 8, 16, 64] {
        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &n, |b, &n| {
            b.iter(|| {
                let mut v = SmallVec::<u64, 8>::new();
                for i in 0..n {
                    v.push(black_box(i as u64));
                }
                let mut sum = 0u64;
                while let Some(x) = v.pop() {
                    sum += x;
                }
                sum
            });
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &n,
            |b, &n| {
                b.iter(|| {
                    let mut v = ExtSmallVec::<[u64; 8]>::new();
                    for i in 0..n {
                        v.push(black_box(i as u64));
                    }
                    let mut sum = 0u64;
                    while let Some(x) = v.pop() {
                        sum += x;
                    }
                    sum
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &n, |b, &n| {
            b.iter(|| {
                let mut v = Vec::<u64>::new();
                for i in 0..n {
                    v.push(black_box(i as u64));
                }
                let mut sum = 0u64;
                while let Some(x) = v.pop() {
                    sum += x;
                }
                sum
            });
        });
    }

    group.finish();
}

// ── Extend from slice ───────────────────────────────────────────────────

fn bench_extend_from_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("extend_from_slice");

    for &n in &[4, 8, 16, 64, 256] {
        let data: Vec<u64> = (0..n).map(|i| i as u64).collect();

        group.bench_with_input(BenchmarkId::new("SmallVec<8>", n), &data, |b, data| {
            b.iter(|| {
                let mut v = SmallVec::<u64, 8>::new();
                v.extend_from_slice(black_box(data));
                v
            });
        });

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec<8>", n),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut v = ExtSmallVec::<[u64; 8]>::new();
                    v.extend_from_slice(black_box(data));
                    v
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("Vec", n), &data, |b, data| {
            b.iter(|| {
                let mut v = Vec::<u64>::new();
                v.extend_from_slice(black_box(data));
                v
            });
        });
    }

    group.finish();
}

// ── Construct a view over an existing slice ─────────────────────────────
//
// turbocow's SmallVec has a zero-copy *Referenced* variant: `from_ref` stores
// the borrowed pointer in O(1) with no allocation, so its cost is flat in `n`.
// smallvec and Vec have no borrow variant, so the closest equivalent copies the
// slice in (O(n)). The comparison is borrow-vs-copy by design — it shows what
// the Referenced variant buys for read-only views.
fn bench_from_ref(c: &mut Criterion) {
    let mut group = c.benchmark_group("from_slice_view");

    for &n in &[8, 64, 256, 1024] {
        let data: Vec<u64> = (0..n).map(|i| i as u64).collect();

        group.bench_with_input(
            BenchmarkId::new("turbocow::SmallVec::from_ref (borrow)", n),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(SmallVec::<u64, 8>::from_ref(black_box(data.as_slice())))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("smallvec::SmallVec::from_slice (copy)", n),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(ExtSmallVec::<[u64; 8]>::from_slice(black_box(
                        data.as_slice(),
                    )))
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("Vec::to_vec (copy)", n),
            &data,
            |b, data| {
                b.iter(|| black_box(data.to_vec()));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_push,
    bench_clone,
    bench_clone_string,
    bench_iter_sum,
    bench_index,
    bench_push_pop,
    bench_extend_from_slice,
    bench_from_ref,
);
criterion_main!(benches);
