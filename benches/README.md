# turbocow benchmarks

Criterion-based benchmarks, in two flavours:

- **Cross-library comparisons** (`benches/comparison/`) — turbocow types against
  the std types and the established crates in the same niche.
- **Internal benchmarks** (`benches/internal/`) — turbocow against upstream
  `ecow` (superset regression guard), turbocow's own map families head-to-head,
  and micro-benchmarks that pin specific optimized code paths.

## Running

```sh
cargo bench                          # everything
cargo bench --bench small_map        # one target
cargo bench --bench small_map -- smap_clone     # one group (regex filter)
cargo bench --bench clone -- turbocow           # one competitor
```

HTML reports (plots, comparisons against the previous run) land at
`target/criterion/report/index.html`. Criterion keeps the prior run as a
baseline, so a second `cargo bench` reports deltas.

> The repo's `.cargo/config.toml` pins the `clang` + `wild` linker and benches
> build in the `bench` profile (`lto = "thin"`, `codegen-units = 1`). First
> build is slow; reruns are cached.

## Targets

### 1. Cross-library string comparison (rosetta)

turbocow `EcoString`/`EcoStr` vs `String`, `Box<str>`, `Arc<str>`, `Cow<str>`,
[`compact_str`], [`ecow`], [`hipstr`], [`lean_string`]. Inputs come from a shared
fixture (`benches/comparison/fixture.rs`) at short / medium / long lengths that
straddle the inline↔heap boundary.

| Target | Group(s) | Measures |
|:--|:--|:--|
| `new` / `new_static` | `new`, `new_static` | construct from a `&str` / from a static or borrowed source |
| `clone` / `clone_static` | `clone`, `clone_static` | clone an owned / static string |
| `access` / `access_static` | `access`, `access_static` | read bytes / chars |
| `self_eq` / `self_eq_static` | `self_eq`, `self_eq_static` | equality |
| `string_comparison` | `create_*`, `clone_*`, `clone_mutate_*`, `eq_*`, `push_*`, `push_str_*` (each `_short`/`_medium`/`_long`, owned + static) | the combined multi-op × multi-size string shootout |

### 2. SmallVec vs `smallvec` / `Vec` (`--bench small_vec`)

turbocow `SmallVec<u64, 8>` vs [`smallvec`]`::SmallVec<[u64; 8]>` vs `Vec`, sizes
spanning the inline (≤8) and spilled (>8) regimes.

| Group | Measures |
|:--|:--|
| `push` | push N elements from empty |
| `clone` | clone (`u64`, Copy element) |
| `clone_string` | clone with a non-`Copy` `String` element (element-clone cost) |
| `iter_sum` | iterate + sum |
| `index_access` | indexed reads |
| `push_pop` | push N then pop all |
| `extend_from_slice` | bulk extend |
| `from_slice_view` | turbocow `from_ref` (zero-copy borrow) vs `smallvec::from_slice` / `Vec::to_vec` (copy) — the Referenced-variant differentiator |

### 3. SmallMap / SmallSet vs std + small-map crates (`--bench small_map`)

Maps compare turbocow `SmallMap` / `EcoMap` vs `std::HashMap`, [`litemap`]
(sorted-Vec), [`im`]`::HashMap` (persistent HAMT), [`micromap`] and
[`heapless`]`::LinearMap` (fixed-capacity — only run at `n ≤ 64`). Sets compare
`SmallSet` / `EcoSet` vs `std::HashSet` and `im::HashSet` (the other three ship
no set type). `u64` keys/values; sizes 2–512.

| Map groups | Set groups |
|:--|:--|
| `smap_insert`, `smap_lookup`, `smap_lookup_miss`, `smap_remove`, `smap_iterate`, `smap_clone`, `smap_update`, `smap_lookup_large_val` (`[u64; 8]` values) | `sset_insert`, `sset_contains`, `sset_clone`, `sset_iterate` |

### 4. turbocow vs ecow micro-benchmarks (`--bench ecow_*`)

Op-for-op turbocow-vs-upstream-`ecow`, a perf companion to the
`ecow_superset.rs` compatibility proof. Targets: `ecow_new`, `ecow_clone`,
`ecow_access`, `ecow_eq`, `ecow_mutate`, `ecow_vec` (the `EcoVec`
new/push/clone/make_mut paths). Groups are under `ecow_cmp/*`.

### 5. turbocow map families head-to-head (`--bench map_clone`)

`SmallMap` vs `EcoMap` only (no external crate): `map_clone`,
`map_clone_then_insert`, `map_clone_then_lookup`, `map_insert_unique`,
`map_lookup` — quantifies the O(1)-clone-but-linear-scan (`EcoMap`) vs
O(N)-clone-but-faster-mutation (`SmallMap`) trade-off.

### 6. Optimization regression benches (`--bench optimizations`)

turbocow-only micro-benchmarks under `opt/*` that pin specific optimized paths
so regressions show up: `case_conversion`, `eco_map_insert`, `eco_map_retain`,
`extend_copy_slice`, `extend_iter`, `from_elem`, `from_slice`, `from_string`,
`make_unique_headroom`, `referenced_push`, `replacen`, `smallmap_clear`,
`smallmap_from_iter`.

## Competitor crates (dev-only deps)

`ecow 0.3.1`, `compact_str 0.9.0`, `hipstr 0.8.0`, `lean_string 0.5.1`,
`smallvec 1.13`, `litemap 0.8.2`, `im 15.1.0`, `micromap 0.3.0`,
`heapless 0.9.1`. All are `[dev-dependencies]`, so the published crate's
dependency set is unaffected.

## Results

Criterion **median** time per operation, **lower is better**. These are
*relative* comparisons — absolute nanoseconds are machine-specific, so rerun
`cargo bench` on your own hardware for figures that mean anything in isolation.

> Methodology: single dev machine, nightly + `bench` profile, captured
> 2026-06-16 at reduced settings (`--warm-up-time 0.3 --measurement-time 0.8
> --sample-size 50`) to keep the full sweep tractable. Treat sub-nanosecond
> deltas as noise. The full size sweep and plots live in
> `target/criterion/report/index.html` after a run.

### Strings — `clone` (the O(1)-clone story)

Sizes are source-string byte lengths; turbocow's inline limit is 15 B (≤15 B
stays inline, ≥16 B spills to a refcounted heap buffer).

| clone (ns) | 15 B (inline) | 16 B | 64 B | 500 B |
|:--|--:|--:|--:|--:|
| **turbocow::EcoString** | 0.2 | 7.7 | 7.3 | 7.3 |
| ecow::EcoString | 0.2 | 7.3 | 7.3 | 7.3 |
| compact_str | 0.3 | 0.3 | 10.5 | 16.4 |
| hipstr | 0.3 | 0.3 | 7.8 | 8.2 |
| lean_string | 0.2 | 0.2 | 7.5 | 7.1 |
| String | 4.5 | 4.5 | 4.6 | 8.9 |
| Box\<str\> | 4.8 | 4.5 | 4.6 | 8.9 |
| Arc\<str\> | 7.3 | 7.3 | 7.6 | 7.3 |
| Cow\<str\>::Owned | 4.7 | 4.4 | 4.6 | 7.4 |

Once spilled, turbocow/ecow clone is **flat ~7.3 ns** (refcount bump), like
`Arc<str>`; `String`/`Box`/`Cow` and `compact_str` deep-copy and grow with
length. (`compact_str`/`hipstr`/`lean_string` keep short strings inline → ~0.3 ns
at ≤16–24 B.)

### Strings — `self_eq` (equality of equal strings)

| self_eq (ns) | 15 B | 64 B | 500 B |
|:--|--:|--:|--:|
| **turbocow::EcoString** | 0.2 | 0.2 | 0.2 |
| ecow::EcoString | 1.3 | 1.1 | 3.6 |
| compact_str | 0.2 | 1.1 | 3.7 |
| hipstr | 2.0 | 1.0 | 1.0 |
| lean_string | 0.9 | 1.1 | 3.7 |
| String | 0.9 | 1.1 | 3.8 |

turbocow short-circuits equal strings that share a buffer via a pointer/length
check — **0.2 ns regardless of length**. Construction (`new`) and char access
are roughly at parity across crates; turbocow's `new` is marginally slower than
ecow's (it writes a refcount header).

### SmallVec vs `smallvec` vs `Vec` (`u64`, inline cap 8)

| push (ns) | n=8 | n=64 | n=256 | n=1024 |
|:--|--:|--:|--:|--:|
| **turbocow::SmallVec** | 8.9 | 101 | 338 | 1142 |
| smallvec | 7.2 | 127 | 254 | 668 |
| Vec | 44.2 | 156 | 309 | 642 |

| clone (ns) | n=8 | n=64 | n=256 |
|:--|--:|--:|--:|
| **turbocow::SmallVec** | 1.6 | 9.8 | 43.1 |
| smallvec | 7.2 | 14.1 | 45.8 |
| Vec | 4.9 | 6.2 | 36.9 |

| **from_ref view** (ns) | n=8 | n=64 | n=256 | n=1024 |
|:--|--:|--:|--:|--:|
| **turbocow `from_ref` (borrow)** | 1.0 | 1.1 | 1.1 | 1.0 |
| smallvec `from_slice` (copy) | 7.9 | 20.5 | 42.1 | 59.0 |
| Vec `to_vec` (copy) | 5.1 | 14.7 | 32.9 | 57.7 |

turbocow wins on small inline construction (no eager alloc — `push` n=8 is 8.9 ns
vs `Vec`'s 44 ns), inline `clone`, and especially the **zero-copy `from_ref`
view (~1 ns flat** vs O(n) copies). The honest trade-off: turbocow's grow path
lags `smallvec`/`Vec` when pushing well past spill (1142 ns vs ~650 ns at 1024).
`iter_sum`, `index_access`, and `extend_from_slice` are within noise of each
other.

### SmallMap / EcoMap vs std + small-map crates (`u64` → `u64`)

| clone (ns) | n=8 | n=64 | n=512 |
|:--|--:|--:|--:|
| **EcoMap** | 15.9 | 15.3 | 15.1 |
| im::HashMap | 15.1 | 14.4 | 14.4 |
| **SmallMap** | 3.5 | 50.8 | 218 |
| std HashMap | 5.6 | 33.7 | 75.6 |
| litemap | 5.0 | 8.4 | 50.4 |
| micromap (cap 64) | 7.3 | 14.0 | — |
| heapless (cap 64) | 7.3 | 13.4 | — |

| lookup hit (ns) | n=8 | n=64 | n=512 |
|:--|--:|--:|--:|
| **SmallMap** | 2.1 | 1.6 | 1.5 |
| **EcoMap** | 1.1 | 6.1 | 64.7 |
| std HashMap | 5.9 | 6.0 | 5.8 |
| im::HashMap | 5.8 | 6.4 | 6.4 |
| litemap | 1.7 | 3.4 | 5.8 |

| insert (build n) (ns) | n=8 | n=64 | n=512 |
|:--|--:|--:|--:|
| **SmallMap** | 17.8 | 676 | 5092 |
| **EcoMap** (`insert`) | 95.4 | 737 | 29802 |
| **EcoMap** (`insert_unique`) | 85 | 266 | 1029 |
| std HashMap | 147 | 1233 | 9619 |
| litemap | 58.7 | 865 | 9168 |
| im::HashMap | 246 | 3192 | 23148 |

The two turbocow maps make opposite trade-offs, and the numbers show it:

- **`EcoMap`** clones in **O(1) (~15 ns flat)** — only `im::HashMap`'s persistent
  HAMT keeps up — but it's a **linear scan**, so lookup degrades (65 ns at 512).
  `insert`/`FromIterator` scan for an existing key on every call to keep
  last-wins dedup, which is O(n²) over a build; for known-distinct bulk loads,
  `insert_unique` / `extend_unique` skip that scan and build in O(n)
  (n=512: ~30× faster, above). It's for clone-heavy, read-mostly, smallish maps.
- **`SmallMap`** has **~1.5 ns lookup at any size** (inline h2 sidecar) and cheap
  inserts, but clone is O(N). It's for mutation-heavy, unique-owner maps.

(`SmallSet`/`EcoSet` mirror the maps: `EcoSet`/`im::HashSet` clone ~15 ns flat;
`SmallSet` clone is O(N).)

### turbocow vs upstream ecow — perf parity on the shared API

A companion to the compile-time `ecow_superset.rs` proof: same op, both crates.

| EcoString (ns) | turbocow | ecow |
|:--|--:|--:|
| clone @64 B | 7.3 | 7.4 |
| clone @500 B | 7.3 | 7.6 |
| new @64 B | 5.0 | 8.2 |
| self_eq @500 B | 0.2 | 3.6 |

| EcoVec\<u64\> (ns) | turbocow | ecow |
|:--|--:|--:|
| new (empty) | 4.1 | 4.3 |
| new (cap 1024) | 34.0 | 36.2 |
| clone (len 1024) | 7.8 | 7.5 |
| push (into cap 16) | 69.9 | 84.6 |
| push (realloc at 64) | 116 | 172 |
| make_mut (shared) | 51.1 | 63.8 |
| make_mut (unique) | 79.6 | 153 |

turbocow matches ecow on clone and is **faster on `EcoString::new`** (after the
by-value construct fix — ~2× at spilled sizes, e.g. 5.0 ns vs 8.2 ns @64 B),
**`EcoVec` push, and `make_mut`** (the perf work paid off); `self_eq` is much
faster thanks to the pointer fast-path.
