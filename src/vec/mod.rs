//! A clone-on-write alternative to [`Vec`].

mod common;
mod drain;
mod impls;
mod splice;
pub(crate) mod types;

pub use drain::Drain;
pub use impls::IntoIter;
pub use splice::Splice;
pub use types::EcoVec;

#[allow(unused)]
pub use common::*;

#[cfg(feature = "std")]
mod io;

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "rkyv")]
mod rkyv_impl;

/// Create a new [`EcoVec`] with the given elements.
/// ```
/// # use turbocow::eco_vec;
/// assert_eq!(eco_vec![1; 4], [1; 4]);
/// assert_eq!(eco_vec![1, 2, 3], [1, 2, 3]);
/// ```
///
/// Optionally, you can specify an [`Allocator`](core::alloc::Allocator) to use.
/// It must be `Clone`; see [`EcoVec`] for how to pass one that isn't by
/// reference.
/// ```
/// # use turbocow::eco_vec;
/// use std::alloc::System;
/// assert_eq!(eco_vec![in System; 1; 4], [1; 4]);
/// assert_eq!(eco_vec![in System; 1, 2, 3], [1, 2, 3]);
/// ```
#[macro_export]
macro_rules! eco_vec {
    () => {
        $crate::EcoVec::new()
    };

    (in $alloc:expr) => {
        $crate::EcoVec::new_in($alloc)
    };

    ($elem:expr; $n:expr) => {
        $crate::EcoVec::from_elem($elem, $n)
    };

    (in $alloc:expr; $elem:expr; $n:expr) => {
        $crate::EcoVec::from_elem_in($elem, $n, $alloc)
    };

    ($($value:expr),+ $(,)?) => {
        $crate::EcoVec::from([$($value),+])
    };

    (in $alloc:expr; $($value:expr),+ $(,)?) => {
        $crate::EcoVec::from_slice_in(&[$($value),+], $alloc)
    };
}
