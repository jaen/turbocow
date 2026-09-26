# turbocow
[![Crates.io](https://img.shields.io/crates/v/turbocow.svg)](https://crates.io/crates/turbocow)
[![Documentation](https://docs.rs/turbocow/badge.svg)](https://docs.rs/turbocow)

turbocow is a collection of compact, clone-on-write data structures — a
**strict superset of [`ecow`][ecow] 0.3.1**. All types present in ecow 0.3.1
(`EcoVec`, `EcoString`, `EcoBytes`) are re-exported with identical APIs, so
existing ecow code can be migrated by replacing `ecow` with `turbocow` in
`Cargo.toml` and updating imports (`ecow::EcoVec` → `turbocow::EcoVec`,
`ecow::EcoString` → `turbocow::EcoString`, `ecow::EcoBytes` →
`turbocow::EcoBytes`). On top of that, turbocow adds map/set variants,
inline-storage collections, and a zero-copy *Referenced* storage variant for
strings, byte buffers and vecs.

## Install

```toml
[dependencies]
turbocow = "0.3.1-beta.1"
```

or via `cargo add`:

```sh
cargo add turbocow
```

## Types

| Type | Description |
|:-----|:------------|
| [`EcoVec<T>`][ecov-docs] | Compact (2-word) reference-counted clone-on-write vector; same memory layout as `&[T]`. O(1) clone. |
| [`EcoString`][ecos-docs] | Clone-on-write string, 15 bytes inline, spills to `EcoVec<u8>`. Strict superset of ecow 0.3.1's `EcoString` (adds `insert`/`insert_str`/`remove`, `ToEcoString`, `AsRef<OsStr/Path>`, `Deref<Target=str>`). |
| [`EcoStr<'a>`][ecostr-docs] | Lifetime-carrying string; supports a zero-copy *Referenced* (borrowed) storage variant. `EcoString` is `EcoStr<'static>` (owned). |
| [`EcoBytes`][ecobs-docs] | Clone-on-write byte buffer, 15 bytes inline, spills to `EcoVec<u8>`. Strict superset of ecow 0.3.1's `EcoBytes` (adds zero-copy `from_static`, `into_eco_string_lossy`, concatenating `FromIterator`/`Extend`). |
| [`EcoMap<K, V>`][ecom-docs] | Reference-counted clone-on-write map backed by two `EcoVec`s (keys + values). O(1) clone. Linear-scan lookup — optimal for small maps. |
| [`EcoSet<T>`][ecoset-docs] | Reference-counted clone-on-write set backed by `EcoMap<T, ()>`. O(1) clone. |
| [`SmallMap<K, V, N>`][smm-docs] | Inline-storage map (`N` entries on the stack with an h2 sidecar, spills to `hashbrown::HashMap`). O(N) clone. |
| [`SmallSet<T, N>`][sms-docs] | Inline-storage set backed by `SmallMap<T, (), N>`. O(N) clone. |
| [`SmallVec<'a, T, N>`][smv-docs] | Inline-storage vec (`N` elements on the stack, spills to heap); supports a zero-copy *Referenced* variant. O(N) clone. |
| [`eco_vec!`][ecov-macro] | Macro to construct an `EcoVec` from a list of elements (like `vec!`). |
| [`eco_format!`][ecof-macro] | Macro to create an `EcoString` from a format string (like `format!`). |
| [`ToEcoString`][toes-docs] | Trait auto-implemented for any `Display` type; provides `.to_eco_string()`. |

[ecov-docs]: https://docs.rs/turbocow/latest/turbocow/struct.EcoVec.html
[ecos-docs]: https://docs.rs/turbocow/latest/turbocow/struct.EcoString.html
[ecostr-docs]: https://docs.rs/turbocow/latest/turbocow/struct.EcoStr.html
[ecobs-docs]: https://docs.rs/turbocow/latest/turbocow/struct.EcoBytes.html
[ecom-docs]: https://docs.rs/turbocow/latest/turbocow/struct.EcoMap.html
[ecoset-docs]: https://docs.rs/turbocow/latest/turbocow/struct.EcoSet.html
[smm-docs]: https://docs.rs/turbocow/latest/turbocow/struct.SmallMap.html
[sms-docs]: https://docs.rs/turbocow/latest/turbocow/struct.SmallSet.html
[smv-docs]: https://docs.rs/turbocow/latest/turbocow/struct.SmallVec.html
[ecov-macro]: https://docs.rs/turbocow/latest/turbocow/macro.eco_vec.html
[ecof-macro]: https://docs.rs/turbocow/latest/turbocow/macro.eco_format.html
[toes-docs]: https://docs.rs/turbocow/latest/turbocow/trait.ToEcoString.html

## Example

```rust
// This is stored inline.
let small = turbocow::EcoString::from("Welcome");

// This spills to the heap, but only once: `big` and `third` share the
// same underlying allocation. Vectors and spilled strings are only
// really cloned upon mutation.
let big = small + " to earth! 🌱";
let mut third = big.clone();

// This allocates again to mutate `third` without affecting `big`.
assert_eq!(third.pop(), Some('🌱'));
assert_eq!(third, "Welcome to earth! ");
```

## Why should I use this instead of ...

| Type | Details |
|:-----|:--------|
| `Vec<T>` / `String` | Normal vectors are a great general purpose data structure. But they have a quite big footprint (3 machine words) and are expensive to clone. `EcoVec` is cheap to clone and only takes two words. |
| `Arc<Vec<T>>` / `Arc<String>` | These require two allocations instead of one and are less convenient to mutate. |
| `Arc<[T]>` / `Arc<str>` | While these require only one allocation, they aren't mutable. |
| Small vector | Different trade-off. Great when there are few, small `T`s, but expensive to clone when spilled. `SmallVec` adds a zero-copy Referenced variant for borrowed data. |
| Small string | `EcoString` combines inline storage, a small footprint, efficient clone-on-write, and full mutability. `EcoStr<'a>` additionally supports zero-copy borrows. |
| `HashMap` / `BTreeMap` | Great general purpose maps, but expensive to clone. `EcoMap` gives O(1) clone via refcounting. `SmallMap` keeps small maps on the stack. |

## Feature flags

| Feature | Default | Description |
|:--------|:-------:|:------------|
| `std` | yes | Enables std integration: `io::Write` for `EcoVec<u8>`, `AsRef<OsStr>` / `AsRef<Path>` for `EcoString`. Implies `alloc`. |
| `alloc` | yes | Core heap support (required; turbocow is a heap-collections crate). Disable `std` but keep `alloc` for no_std builds. |
| `serde` | no | `Serialize`/`Deserialize` for all collection types. |
| `rkyv` | no | rkyv 0.8 `Archive`/`Serialize`/`Deserialize` for `EcoVec`, `EcoString`, `EcoBytes`. |
| `allocator-api2-v02` | no | Custom-allocator support via the [`allocator-api2`](https://crates.io/crates/allocator-api2) 0.2 shim (stable). |
| `allocator-api2-v03` | no | Custom-allocator support via the [`allocator-api2`](https://crates.io/crates/allocator-api2) 0.3 shim (stable). |
| `nightly-allocator-api` | no | Custom-allocator support via the real nightly `allocator_api` feature. See caveat below. |
| `nightly-fmt` | no | Enables the `fmt::Arguments::as_str()` fast-path in `eco_format!`. |
| `nightly-phantom-variance` | no | Uses `PhantomCovariantLifetime` from stdlib instead of a polyfill. |
| `loom` | no | Enables [loom](https://crates.io/crates/loom) concurrency testing mode. |

> **Allocator feature notes:** the stable `allocator-api2-v02` and
> `allocator-api2-v03` shims are the supported way to plug in a custom
> allocator and build on stable Rust. `nightly-allocator-api` requires a
> nightly toolchain; the library itself compiles with it, but turbocow's own
> integration tests (which use `bump-scope` as a dev-dependency) currently fail
> on recent nightlies due to an upstream `bump-scope` vs `core::alloc::Allocator`
> trait conflict — a dev-only issue that does not affect consumers. The
> `nightly-allocator-api` and `allocator-api2-*` backends are mutually
> exclusive; enabling them together (e.g. via `--all-features`) is unsupported.

### no_std

turbocow supports `no_std` environments. Disable the `std` feature while
keeping `alloc` (bare no-alloc is not supported — turbocow is a
heap-collections crate):

```toml
[dependencies]
turbocow = { version = "0.3", default-features = false, features = ["alloc"] }
```

### MSRV

turbocow requires **Rust 1.85** (edition 2024). This is intentionally higher
than ecow's MSRV of 1.73; the "strict superset of ecow 0.3.1" guarantee
applies to the **public API**, not to MSRV compatibility.

## FAQ

### Is it any good?

Yes.

### No, really?

Ehhh~ this is half-mess, half-slop. I wanted a faster `ecow` with `Allocator` support for a very fast big scale parallel ninja parser I was prototyping, so I kind of vendored `ecow` and started messing around with the profiler until I've gotten it to do what I wanted.

Unfortunately I'm a rather messy programmer with an executive dysfunction, so it's been sitting an unfinished mess for over a year. But, given that since Opus 4.6 LLMs became kinda usable and I started needing it in more things, I have eventually poked the slop generator enough to end up with something that seemed publishable enough. It probably needs some actual polish on top of that (Allocator API support might've bitrotten a bit), but at least it works and does what I want. Treat is as an early-access beta, or something.

And at least, it's [fast enough](./benches/README.md) for what I needed it for.

## Links

- **Source**: <https://github.com/jaen/turbocow>
- **Based on**: [`ecow`][ecow] by [Typst][typst] — the `EcoVec`, `EcoString`
  and `EcoBytes` types are derived from ecow 0.3.0/0.3.1 and remain
  API-compatible.

[ecow]: https://github.com/typst/ecow
[typst]: https://github.com/typst

## License

This crate is dual-licensed under the MIT and Apache 2.0 licenses; see
[`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE).

Portions are derived from [`ecow`][ecow] 0.3.0/0.3.1 and from the Rust standard
library, which remain under their original `MIT OR Apache-2.0` terms. Those
attributions are recorded in [`NOTICE`](NOTICE).
