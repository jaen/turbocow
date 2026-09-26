//! Inline-spillable vector for arbitrary `T`s.
//!
//! `SmallVec<'a, T, N>` stores up to `N` elements inline on the stack. When
//! the capacity is exceeded it spills to a heap-allocated buffer with a
//! single-word header storing the capacity. It can also hold a zero-copy
//! borrow of an existing `&'a [T]` (Referenced variant).
//!
//! # Layout
//! Uses a `repr(C)` struct with a `tagged_len` (2 tag bits + 62-bit length)
//! and a union overlapping the three variants. Total size depends on `T`'s
//! alignment — see `expected_size()` in `tests/small_vec.rs` for the exact
//! formula. For `T` with `align_of::<T>() <= align_of::<usize>()`, the size
//! is `8 + max(N * size_of::<T>(), 8)` bytes; types with larger alignment
//! may require padding between the `tagged_len` and the data union.

mod impls;
mod types;

pub use impls::{Drain, IntoIter};
pub use types::SmallVec;
