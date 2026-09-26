//! A clone-on-write alternative to [`Vec`].

mod common;
mod drain;
mod impls;
mod splice;
pub(crate) mod types;

pub use drain::Drain;
pub use impls::IntoIter;
pub use splice::Splice;

#[cfg(feature = "allocator-api")]
mod allocator;
#[cfg(not(feature = "allocator-api"))]
mod default;

#[cfg(feature = "allocator-api")]
pub use allocator::*;
#[cfg(not(feature = "allocator-api"))]
pub use default::*;

#[allow(unused)]
pub use common::*;

#[cfg(feature = "std")]
mod io;

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "rkyv")]
mod rkyv_impl;
