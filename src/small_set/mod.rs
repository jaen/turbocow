//! Compact, inline-optimized hash set.
//!
//! [`SmallSet`] stores up to `N` values inline (on the stack) and spills to a
//! heap-allocated [`hashbrown::HashSet`](hashbrown) when the inline capacity
//! is exceeded. It is a thin newtype over [`SmallMap<T, (), N, S>`](crate::SmallMap).

mod impls;
#[cfg(feature = "rkyv")]
mod rkyv_impl;
#[cfg(feature = "serde")]
mod serde;

pub use impls::{Difference, Intersection, IntoIter, Iter, SymmetricDifference, Union};

use crate::small_map::SmallMap;

/// A compact hash set that stores up to `N` values inline and spills to the
/// heap when the inline capacity is exceeded.
///
/// This is a newtype over [`SmallMap<T, (), N, S>`](crate::SmallMap). For small
/// sets (≤ `N` entries), all data lives on the stack. When the set grows
/// beyond `N`, it transparently spills to a heap-allocated hash set.
///
/// The default inline capacity is 8 entries.
///
/// # Examples
/// ```
/// use turbocow::SmallSet;
///
/// let mut set = SmallSet::<&str>::new();
/// set.insert("hello");
/// set.insert("world");
/// assert!(set.contains("hello"));
/// assert_eq!(set.len(), 2);
/// ```
pub struct SmallSet<T, const N: usize = 8, S = hashbrown::DefaultHashBuilder>(
    pub(crate) SmallMap<T, (), N, S>,
);
