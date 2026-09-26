//! Compact, inline-optimized hash map.
//!
//! [`SmallMap`] stores up to `N` key-value pairs inline (on the stack) and
//! spills to a heap-allocated [`hashbrown::HashMap`] when the inline capacity
//! is exceeded.

mod entry;
mod group;
mod impls;
mod inline;
#[cfg(feature = "rkyv")]
mod rkyv_impl;
#[cfg(feature = "serde")]
mod serde;
mod types;

pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use impls::{IntoIter, Iter, IterMut, Keys, Values, ValuesMut};
pub use types::SmallMap;
