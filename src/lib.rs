/*!
Compact, clone-on-write collections — a strict superset of [`ecow`][ecow] 0.3.1.

turbocow extends `ecow` with additional data structures and a zero-copy
*Referenced* storage variant. Existing `ecow` code works by swapping the crate:
replace `ecow` with `turbocow` in `Cargo.toml` and update the imports from
`ecow::{EcoVec, EcoString, EcoBytes}` to `turbocow::{EcoVec, EcoString, EcoBytes}`.

## Types

### Vectors & Strings

- [`EcoVec<T>`] — compact (2-word) reference-counted clone-on-write vector; same layout as `&[T]`.
- [`EcoString`] — clone-on-write string, 15 bytes inline, spills to `EcoVec<u8>`; fully owns its data.
- [`EcoStr<'a>`] — lifetime-carrying string; supports a zero-copy *Referenced* (borrowed) storage variant; `EcoString = EcoStr<'static>` (owned).
- [`EcoBytes`] — clone-on-write byte buffer, 15 bytes inline, spills to `EcoVec<u8>`; supports the zero-copy *Referenced* variant via `from_static`.
- Macros: [`eco_vec!`], [`eco_format!`]. Trait: [`ToEcoString`].

### Maps & Sets

turbocow provides two families, optimised for different workloads:

| Type | Clone cost | Best for |
|:-----|:-----------|:---------|
| [`EcoMap<K, V>`] | O(1) — atomic refcount bump | Clone-heavy, read-mostly |
| [`EcoSet<T>`]    | O(1) | Clone-heavy, read-mostly |
| [`SmallMap<K, V, N>`] | O(N) copy | Mutation-heavy, unique-owner |
| [`SmallSet<T, N>`]    | O(N) copy | Mutation-heavy, unique-owner |

[`SmallVec<'a, T, N>`] is an inline-storage vec (N slots on the stack, spills to heap)
with its own zero-copy Referenced variant.

See the README for feature flags.

## Example
```
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

[ecow]: https://github.com/typst/ecow
*/

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]
#![cfg_attr(feature = "nightly-phantom-variance", feature(phantom_variance_markers))]
// "Available on crate feature X" badges on docs.rs. `feature(doc_auto_cfg)` was
// removed upstream; the current form is `feature(doc_cfg)` (which gates the
// `doc(auto_cfg)` attribute). Gated on `--cfg docsrs` (set by the docs.rs
// metadata), so normal/stable builds are unaffected and need no nightly.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]
// See https://github.com/tokio-rs/loom/issues/352
#![allow(unknown_lints, unexpected_cfgs)]

extern crate alloc;

pub mod bytes;
pub(crate) mod dynamic;
pub mod eco_map;
pub mod eco_set;
pub mod small_map;
pub mod small_set;
pub mod small_vec;
pub mod string;
pub mod vec;

mod allocator;
mod utils;

pub use self::bytes::EcoBytes;
pub use self::eco_map::EcoMap;
pub use self::eco_set::EcoSet;
pub use self::small_map::SmallMap;
pub use self::small_set::SmallSet;
pub use self::small_vec::SmallVec;
pub use self::string::{EcoStr, EcoString, ToEcoString};
pub use self::vec::{EcoVec, IntoIter};

// Re-export allocator types when features are enabled
#[cfg(feature = "allocator-api")]
pub use allocator::Global;

// Run doctests on the README too
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

/// Loom needs its own synchronization types to be used in order to work
mod sync {
    /// Atomics stub
    pub mod atomic {
        #[cfg(not(loom))]
        pub use core::sync::atomic::*;

        #[cfg(loom)]
        pub use loom::sync::atomic::*;
    }
}
