#[cfg(all(
    feature = "allocator-api2-v02",
    not(feature = "allocator-api2-v03"),
    not(feature = "nightly-allocator-api")
))]
pub(crate) mod internal {
    extern crate alloc as alloc_lib;

    #[allow(unused)]
    pub use self::alloc_lib::{borrow, string, sync};

    /// Re-export allocator-api2 v0.2 types
    #[allow(unused)]
    pub use ::allocator_api2_02::{boxed, collections, vec};

    pub use ::allocator_api2_02::alloc;

    pub use ::allocator_api2_02::alloc::Global;

    /// Internal allocator trait that wraps allocator-api2 v0.2 allocators.
    #[allow(unused)]
    pub trait Allocator: alloc::Allocator + Clone {}

    /// Blanket implementation for all allocator-api2 v0.2 allocators.
    impl<A: alloc::Allocator + Clone> Allocator for A {}
}

#[cfg(all(
    feature = "allocator-api2-v03",
    not(feature = "allocator-api2-v02"),
    not(feature = "nightly-allocator-api"),
))]
pub(crate) mod internal {
    extern crate alloc as alloc_lib;

    #[allow(unused)]
    pub use self::alloc_lib::{borrow, string, sync};

    /// Re-export allocator-api2 v0.3 types
    #[allow(unused)]
    pub use ::allocator_api2_03::{boxed, collections, vec};

    /// The `alloc` module providing `Layout`, `LayoutError`, `AllocError`,
    /// `Allocator` and `Global`. Consumed crate-wide as `crate::allocator::alloc`.
    pub use ::allocator_api2_03::alloc;

    pub use ::allocator_api2_03::alloc::Global;

    /// Internal allocator trait that wraps allocator-api2 v0.3 allocators.
    #[allow(unused)]
    pub trait Allocator: alloc::Allocator + Clone {}

    /// Blanket implementation for all allocator-api2 v0.3 allocators.
    impl<A: alloc::Allocator + Clone> Allocator for A {}
}

#[cfg(all(feature = "nightly-allocator-api", not(feature = "allocator-api-shim")))]
pub(crate) mod internal {
    /// The `alloc` module providing `Layout`, `LayoutError`, `AllocError`,
    /// `Allocator` and `Global`. Consumed crate-wide as `crate::allocator::alloc`.
    pub use std::alloc;

    pub use std::alloc::Global;

    /// Internal allocator trait that wraps nightly std allocators.
    #[allow(unused)]
    pub trait Allocator: alloc::Allocator + Clone {}

    /// Blanket implementation for all nightly std allocators.
    impl<A: alloc::Allocator + Clone> Allocator for A {}
}
