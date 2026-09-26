//! Internal allocator abstraction for turbocow.
//!
//! Provides a small internal allocator trait ([`AllocatorProvider`]) that
//! decouples the rest of the crate from the specific external allocator API in
//! use — the stable `allocator-api2` shims or the nightly `allocator_api`.

// The `allocator-api` feature is internal: `allocator-api2-v02`,
// `allocator-api2-v03`, and `nightly-allocator-api` each enable it *together*
// with a single backing `internal` module (selected below). Enabling
// `allocator-api` on its own leaves no backend, and selecting two conflicting
// backends leaves `internal` ambiguous, so reject both up front with a clear
// message instead of a cascade of unresolved-import errors.
#[cfg(all(
    feature = "allocator-api",
    not(feature = "allocator-api-shim"),
    not(feature = "nightly-allocator-api")
))]
compile_error!(
    "the `allocator-api` feature is internal; enable one of `allocator-api2-v02`, \
     `allocator-api2-v03`, or `nightly-allocator-api` instead"
);

#[cfg(all(feature = "nightly-allocator-api", feature = "allocator-api-shim"))]
compile_error!(
    "`nightly-allocator-api` cannot be combined with the `allocator-api2-v02` / \
     `allocator-api2-v03` shims; select a single allocator backend"
);

#[cfg(all(feature = "allocator-api2-v02", feature = "allocator-api2-v03"))]
compile_error!(
    "`allocator-api2-v02` and `allocator-api2-v03` are mutually exclusive; \
     pick one. If a transitive dependency pulls in the other, use \
     `default-features = false` on the offending dependency or use a \
     cargo `[patch]` to pin it to a single version."
);

mod common;
pub(crate) use common::*;

#[cfg(not(feature = "allocator-api"))]
mod default;
#[cfg(not(feature = "allocator-api"))]
pub(crate) use default::*;

#[cfg(feature = "allocator-api")]
#[allow(clippy::module_inception)]
mod allocator;
#[cfg(feature = "allocator-api")]
pub(crate) use allocator::*;

pub(crate) use internal::*;

#[cfg(feature = "allocator-api")]
pub use internal::Global;

#[cfg(test)]
mod tests {
    use core::alloc::Layout;

    use super::*;

    fn get_allocator() -> Global {
        Global
    }

    #[test]
    fn test_global_allocator_zero_sized() {
        let allocator = get_allocator();
        let layout = Layout::new::<()>();

        let ptr = AllocatorProvider::allocate(&allocator, layout).unwrap();
        assert_eq!(ptr.len(), 0);

        // Deallocating a zero-sized allocation must be safe.
        unsafe {
            AllocatorProvider::deallocate(&allocator, ptr.cast(), layout);
        }
    }

    #[test]
    fn test_global_allocator_basic() {
        let allocator = get_allocator();
        let layout = Layout::new::<u64>();

        let ptr = AllocatorProvider::allocate(&allocator, layout).unwrap();
        assert_eq!(ptr.len(), 8);

        unsafe {
            let raw_ptr = ptr.as_ptr().cast::<u64>();
            raw_ptr.write(42);
            assert_eq!(raw_ptr.read(), 42);
            AllocatorProvider::deallocate(&allocator, ptr.cast(), layout);
        }
    }

    #[test]
    fn test_global_allocator_grow() {
        let allocator = get_allocator();
        let old_layout = Layout::from_size_align(8, 8).unwrap();
        let new_layout = Layout::from_size_align(16, 8).unwrap();

        let old_ptr = AllocatorProvider::allocate(&allocator, old_layout).unwrap();

        unsafe {
            old_ptr.as_ptr().cast::<u64>().write(42);

            let new_ptr = AllocatorProvider::grow(
                &allocator,
                old_ptr.cast(),
                old_layout,
                new_layout,
            )
            .unwrap();
            assert_eq!(new_ptr.len(), 16);
            assert_eq!(new_ptr.as_ptr().cast::<u64>().read(), 42);

            AllocatorProvider::deallocate(&allocator, new_ptr.cast(), new_layout);
        }
    }

    #[test]
    fn test_global_allocator_shrink() {
        let allocator = get_allocator();
        let old_layout = Layout::from_size_align(16, 8).unwrap();
        let new_layout = Layout::from_size_align(8, 8).unwrap();

        let old_ptr = AllocatorProvider::allocate(&allocator, old_layout).unwrap();

        unsafe {
            old_ptr.as_ptr().cast::<u64>().write(42);

            let new_ptr = AllocatorProvider::shrink(
                &allocator,
                old_ptr.cast(),
                old_layout,
                new_layout,
            )
            .unwrap();
            assert_eq!(new_ptr.len(), 8);
            assert_eq!(new_ptr.as_ptr().cast::<u64>().read(), 42);

            AllocatorProvider::deallocate(&allocator, new_ptr.cast(), new_layout);
        }
    }
}
