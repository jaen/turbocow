# Changelog

All notable changes to turbocow will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
turbocow does not follow Semantic Versioning: minor versions may introduce
breaking changes.

## [0.3.1-beta.1] - Unreleased

turbocow is now a strict superset of ecow **0.3.1** (previously 0.3.0), and
moved from Codeberg to GitHub (<https://github.com/jaen/turbocow>), with CI on
GitHub Actions.

### Changed (breaking) — ecow 0.3.1 parity

- **`EcoByteString` is renamed to `EcoBytes`** and moved to the new
  `turbocow::bytes` module (re-exported at the crate root), matching ecow
  0.3.1's public `ecow::EcoBytes`. There is no deprecated alias.
- Removed `EcoByteString` methods that duplicated ecow's API under another
  name: `from_bytes` (use `EcoBytes::from(&[u8])`), `as_bytes` (use
  `as_slice`), and `into_eco_string` (use `EcoString::try_from`).
- **`TryFrom<EcoBytes> for EcoString`** now fails with `core::str::Utf8Error`
  (as in ecow) instead of returning the input bytes. `EcoBytes` clones are
  O(1), so `EcoString::try_from(bytes.clone())` keeps the original around.
- **`EcoString::from_str` is no longer an inherent method.** It shadowed
  `FromStr::from_str`, so ecow code such as `EcoString::from_str(s).unwrap()`
  did not compile against turbocow. The trait method is unchanged.

### Added — ecow 0.3.1 parity

- `EcoBytes`: `const fn inline`, `const fn try_inline`, `capacity`,
  `is_inline`, `as_slice`, `insert`, `remove`, `insert_slice`, `reserve`;
  `PartialEq` with `Vec<u8>`/`EcoVec<u8>` (both directions) and `[u8; N] ==
  EcoBytes`; `From<[u8; N]>`, `From<EcoVec<u8>>` (zero-copy),
  `From<Cow<[u8]>>`; `EcoVec<u8>: From<EcoBytes>` (zero-copy when spilled) and
  `From<&EcoBytes>`; `Extend<&u8>`; `IntoIterator for &EcoBytes`;
  `std::io::Write`.
- `EcoString`: `const fn try_inline`, `capacity`, `is_inline`, `reserve`;
  `From<EcoString> for EcoVec<u8>`; `TryFrom<EcoVec<u8>> for EcoString`.
  `EcoStr<'a>` gains `capacity`, `is_inline` and `reserve` too.
- For a borrowed (`from_static` / Referenced) value, `is_inline()` is `false`
  and `capacity()` equals its length: it owns no storage, and any mutation
  copies it first. `reserve` always leaves the value owning its storage.

### Fixed

- **`EcoBytes` serde deserialization** now accepts sequences and strings in
  addition to byte strings (like ecow). Previously a value serialized to JSON
  (`[97,98]`) could not be deserialized back.
- `tools/loom-tests/Cargo.lock` was stale after the `hotpath` 0.17 bump, which
  broke the `--locked` loom CI job.

### Tests

- `tests/ecow_superset.rs` now pins every public item of ecow 0.3.1 (including
  the `ecow::bytes` module and `EcoString::from_str` call syntax), and
  `tests/proptest_ecow_diff.rs` differentially tests `EcoBytes` against
  `ecow::EcoBytes` (including `is_inline()` agreement) and `insert_str` /
  `reserve` against ecow's `EcoString`. The `BytesOp` model gained
  `Insert`/`InsertSlice`/`Remove`/`Reserve`/`CloneMutateCheck` and a
  Referenced-start entry point, shared by proptest and the `ecobytes` fuzz
  target.

### Fixed (critical)

- **Big-endian support.** The `DynamicVec` union (backing `EcoString`,
  `EcoStr<'a>`, and `EcoBytes`) failed to compile on big-endian targets.
  The big-endian layout is correct and matches `ecow` (the inline tag byte is
  pushed *past* the `EcoVec` and written explicitly, so the union is
  intentionally larger than `EcoVec` there) — but turbocow had added a
  `size_of::<InlineVec>() == size_of::<EcoVec>()` assertion that this layout
  necessarily violates, so the crate never built on big-endian. The assertion
  is replaced with the actual invariants (the variants fit the union;
  `tagged_len` is at offset `LIMIT`; the discriminant is reliable via either the
  64-bit-LE `len`-MSB overlap or an explicit tag write past the `EcoVec`), and
  the tautological `size_of::<[u8; LIMIT]>()` check becomes a real `offset_of!`
  one. Inline capacity is unchanged. The crate now compiles and passes Miri on
  `sparc64-unknown-linux-gnu` (and `i686-unknown-linux-gnu`).
- **`SmallMap` SIMD out-of-bounds read** for `N ∈ [17, 31]` with `len > 16` on
  x86_64: the SSE2 h2 matcher's second `_mm_loadu_si128` read 16 bytes from
  offset 16 of an N-byte array (masked-but-real OOB; only Miri caught it). The
  h2 sidecar is now scanned in 16-byte chunks — SSE2 where a full chunk is in
  bounds, a scalar tail otherwise — so no load runs past the array. (#1)
- **`SmallMap` lookups corrupted for `N > 32`**: the fixed 32-bit h2 bitmask
  covered only the first 32 inline slots (and `1u32 << i` overflowed for
  `i ≥ 32`), so keys stored at slot ≥ 32 were silently lost and `insert`
  appended duplicates. The chunked matcher walks every slot, so any `N > 0`
  now works correctly — no cap (consistent with `SmallVec`). (#2)
- **`EcoVec::replace` panic-safety.** It dropped the old element in place and
  *then* wrote the new one, so if `T::drop` panicked the slot was left
  uninitialised while `len` was unchanged — `EcoVec::drop` then dropped it
  again (double-drop → abort). It now `ptr::read`s the old value out, writes
  the new value, and drops the old value last, so an unwinding `T::drop` leaves
  a valid element in the slot. (#3)
- **`SmallMap` spill panic-safety with `Drop`-bearing hashers.** Spilling inline
  storage to the heap bitwise-copied a non-`Copy` hasher and then double-dropped
  it (#4), and a panicking `K::hash`/`K::eq` mid-spill could double-drop entries
  (#5) or — even with those addressed — still double-drop a `Drop`-bearing
  hasher, because the inline shell (whose hasher was already moved into the heap)
  was only forgotten *after* the fallible inserts. `force_spill` now moves the
  hasher and entries out and installs the heap into the map *before* inserting,
  with a guard that drops any not-yet-inserted entries on unwind. `SmallMap`'s
  `IntoIter` also no longer leaks a non-`Copy` hasher. (#4, #5, #11)
- **`EcoMap` panic-safety.** `retain` truncated only after its loop, so a
  panicking predicate left removed keys visible (#6); `remove` could leave
  `keys.len() != vals.len()` if `K::drop` panicked (#14); and
  `OccupiedEntry::insert` cloned the old value instead of `mem::replace`ing it
  (#13). `retain` and `remove` now use a symmetric swap + tuple-`pop` so both
  backing vecs shrink together, and `remove` copies-on-write **both** vecs
  *before* either swap, so a panicking `V::clone` during the COW of a shared map
  can no longer leave `keys` swapped but `vals` not (a key/value mispairing).
  `OccupiedEntry::insert` now uses `mem::replace`. (#6, #13, #14)

### Fixed (robustness)

- **`SmallVec::materialise_referenced` (heap path) leaked cloned elements** if
  `T::clone` panicked mid-loop — `tagged_len` stayed 0 so `SmallVec::drop`
  skipped them. It now uses a `CloneGuard` that drops the already-cloned
  elements on unwind. (#12)
- **`InlineVec::emplace_from_slice_in` now supports any inline limit.** It
  assumed the inline area fits in two words (true for the 64-bit little-endian
  limit of 15, false for the 23-byte big-endian limit, where it would have left
  uninitialised tail bytes). It now zeroes the whole buffer, so it is correct on
  every target. (#17)
- **`DynamicVec::drop` leaked a non-ZST allocator** for the Inline/Referenced
  variants (only Spilled, which stores the allocator in its `EcoVec`, dropped
  it). It now drops the allocator field for those variants, guarded by
  `needs_drop::<A>()` so it is zero-cost for the default `Global`. (#18)

### Changed

- **`ref_count_overflow` rolls back the `fetch_add`** before panicking, matching
  ecow 0.3.0 (the previous code panicked without undoing the clone's increment).
  (#15, P1)
- **`EcoVec::Extend` caches `is_unique()`** once after `reserve` rather than
  re-checking on every iteration — it cannot change while `&mut self` is held.
  (#20)
- **`From<Vec<u8>>` for `EcoBytes`** now mirrors `From<String>` for
  `EcoString` (one alloc + memcpy for long inputs instead of two). (P2)
- **`SmallMap` is now an opaque struct** instead of a `pub enum`; its `Inline`
  and `Heap` variants are no longer part of the public API (they leaked
  implementation details — every other type in turbocow and ecow keeps its
  representation private). Code that matched on `SmallMap::{Inline, Heap}` must
  use the public methods instead. (Breaking; permitted by the no-SemVer policy.)

### Documented

- `SmallMap::Extend`/`FromIterator` worst-case complexity (#19); `make_unique`'s
  empty-vec branch (P3); `serde::Deserialize`'s `A: Default` bound (P4);
  `io::Write`'s `Global`-only impl (P5); `SmallVec`'s size-formula alignment
  caveat.

### Internal

- Replaced `mem::transmute::<usize, *mut T>` with `ptr::without_provenance_mut`
  in `EcoVec`'s dangling pointer (stable since 1.84). Removed dead `InlineMap`
  methods and the stale `tests/loom.rs` stub (the real loom tests live in
  `tools/loom-tests`).

### Tests

- Expanded the differential model: `VecOp::Replace`/`Drain`, `MapOp::Entry`, a
  non-`Copy` `Box<u32>` element variant (real `Drop` glue under Miri), and
  `Referenced`-variant entry points. Added `EcoStr<'a>` and `EcoBytes`
  `Referenced` test suites, `SmallVec` `Referenced` + ZST-value
  (`EcoSet = EcoMap<T, ()>`) tests, and a `Drain` `offset_from`-provenance Miri
  test. The `ecobytes` fuzz target now drives a real `EcoBytes` model
  (it previously duplicated the `EcoString` target and exercised no
  `EcoBytes`); it shares the model with a new `proptest` entry point.

## [0.3.0-beta.2] - 2026-06-16

First release, first misconfigurations ;' )

### Fixed

#### Documentation generation

Docs.rs should now be able to actually publish the docs, hopefully.

#### CI config

Commented out Codecov publish and bumpe MIRI machine tags.


## [0.3.0-beta.1] - 2026-06-16

First public preview release of turbocow.

### Added

#### ecow 0.3.0 superset — shared types

turbocow's public API is a strict superset of [ecow 0.3.0][ecow]. Existing
ecow code migrates by replacing `ecow` with `turbocow` in `Cargo.toml` and
updating imports. All items below that complete ecow 0.3.0 parity are noted:

- **`EcoVec<T>`** — compact (2-word) reference-counted clone-on-write vector;
  same memory layout as `&[T]`. O(1) clone via atomic refcount. New vs ecow:
  `drain`, `splice`, `serde` Serialize/Deserialize, `io::Write` for
  `EcoVec<u8>`, `const fn new`, inline `#[inline]` attribute parity.
- **`EcoString`** — clone-on-write string with 15 bytes of inline storage;
  spills to `EcoVec<u8>` for longer content. New vs ecow: `insert`,
  `insert_str`, `remove`, `From<Cow<str>>`, `Deref<Target=str>` (exact
  parity — previously `Deref` only went to `EcoStr`), `AsRef<OsStr>` /
  `AsRef<Path>`, `const fn inline`.
- **`ToEcoString`** — trait auto-implemented for any `Display` type; provides
  `.to_eco_string()`. (ecow 0.3.0 parity.)

#### turbocow-only types

- **`EcoStr<'a>`** — lifetime-carrying string view; supports a zero-copy
  *Referenced* (borrowed) storage variant. `EcoString` is `EcoStr<'static>`.
- **`EcoByteString`** — byte-oriented counterpart of `EcoString` for arbitrary
  `[u8]` data, with conversions to/from `EcoString`.
- **`EcoMap<K, V>`** — map backed by two `EcoVec`s (keys + values); O(1)
  clone via atomic refcount; copy-on-write on mutation; linear-scan lookup
  optimal for small maps. Includes `reserve`, `capacity`, `shrink_to_fit`.
- **`EcoSet<T>`** — set backed by `EcoMap<T, ()>`; same O(1) clone semantics.
  Includes set-algebra operations: `union`, `intersection`, `difference`,
  `symmetric_difference`. Includes `reserve`, `capacity`, `shrink_to_fit`.
- **`SmallMap<K, V, N>`** — map with inline stack storage for up to N entries
  (default N=8) with h2-filtered lookup; spills to `hashbrown::HashMap` when
  capacity is exceeded. Includes `reserve`, `capacity`, `shrink_to_fit`.
- **`SmallSet<T, N>`** — set backed by `SmallMap<T, (), N>`. Includes
  `reserve`, `capacity`, `shrink_to_fit`.
- **`SmallVec<'a, T, N>`** — vector with inline stack storage (N slots on
  the stack, spills to heap) and a zero-copy *Referenced* variant for borrowed
  data.

#### Other

- `serde` feature: `Serialize`/`Deserialize` for all collection types.
- `rkyv` feature: rkyv 0.8 `Archive`/`Serialize`/`Deserialize` for `EcoVec`,
  `EcoString`, `EcoByteString`.
- `allocator-api2-v02` / `allocator-api2-v03` features: custom-allocator
  support via stable `allocator-api2` shims (v0.2 and v0.3 respectively).
- `no_std` support: use `default-features = false, features = ["alloc"]`.
  Bare no-alloc is not supported (turbocow is a heap-collections crate).
- Edition 2024, MSRV 1.85.

### Notes

- **Behavior change — `EcoVec::with_capacity` / `from_elem` / `from_slice`
  are now `Global`-only.** These constructors are restricted to the `Global`
  allocator. Use the corresponding `*_in` variants (`with_capacity_in`,
  `from_elem_in`, `from_slice_in`) to supply a custom allocator. This matches
  the standard library pattern for allocator-generic types.
- **`nightly-allocator-api` dev-test caveat.** The library compiles with
  `nightly-allocator-api` enabled, but turbocow's own integration tests (which
  use `bump-scope` as a dev-dependency) currently fail to build on recent
  nightlies due to an upstream `bump-scope` vs `core::alloc::Allocator` trait
  conflict. This is a dev-only build issue; consumers of turbocow are
  unaffected.
- **MSRV vs ecow.** The "strict superset of ecow 0.3.0" guarantee is about
  the **public API**, not MSRV. turbocow requires Rust 1.85 (edition 2024),
  which is intentionally higher than ecow's MSRV of 1.73.

[ecow]: https://github.com/typst/ecow
