use core::alloc::Allocator;
use core::ptr::NonNull;

// #[cfg(feature = "phantom-variance-markers")]
use core::marker::PhantomData;

use crate::Global;
use crate::sync::atomic::AtomicUsize;

/// An economical vector with clone-on-write semantics.
///
/// With the default [`Global`] allocator (or any other zero-sized one), this
/// type has the same layout as a slice `&[T]`: It consists of a pointer
/// and a length. The pointer is null-pointer optimized (meaning that
///  [`Option<EcoVec<T>>`] has the same size as `EcoVec<T>`). Dereferencing an
/// `EcoVec` to a slice is a no-op. A non-zero-sized allocator is stored
/// alongside and adds its own size.
///
/// Within its allocation, an `EcoVec` stores a reference count and its
/// capacity. In contrast to an [`Arc<Vec<T>>`](alloc::sync::Arc), it only
/// requires a single allocation for both the reference count and the elements.
/// The internal reference counter is atomic, making this type [`Sync`] and
/// [`Send`] when both `T` and the allocator are `Send + Sync`.
///
/// Note that most mutating methods require [`T: Clone`](Clone) due to
/// clone-on-write semantics.
///
/// # Example
/// ```
/// use turbocow::EcoVec;
///
/// // Empty vector does not allocate, but first push does.
/// let mut first = EcoVec::new();
/// first.push(1);
/// first.push(2);
/// assert_eq!(first, [1, 2]);
///
/// // This clone is cheap, it references the same allocation.
/// let mut second = first.clone();
///
/// // This makes a real copy (clone-on-write).
/// second.push(3);
/// assert_eq!(second, [1, 2, 3]);
///
/// // As `second` was cloned upon mutation, this iterator can
/// // move the elements. If the allocation was still shared with
/// // `first`, this would clone lazily.
/// assert_eq!(second.into_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
/// ```
///
/// # Custom allocators
///
/// Any [`Allocator`] can back an `EcoVec`: pass it to one of the `*_in`
/// constructors (`new_in`, `with_capacity_in`, `from_elem_in`,
/// `from_slice_in`, ...) or to `eco_vec![in alloc; ..]`. Mutating methods also
/// need `A: Clone`, because clone-on-write may allocate a fresh buffer with a
/// clone of the allocator.
///
/// Cloning the vector itself shares its buffer, and whichever clone is dropped
/// last frees it with *its* copy of the allocator. That is only sound if all
/// clones of the allocator are interchangeable, which stable Rust does not
/// promise for an arbitrary `A: Clone` (that is what the still-unstable
/// `AllocatorClone` trait is for). So `EcoVec<T, A>` implements [`Clone`]
/// exactly for the allocators the standard library guarantees this for:
/// [`Global`], `std::alloc::System` (with the `std` feature), and `&B`,
/// [`Rc<B>`](alloc::rc::Rc) and [`Arc<B>`](alloc::sync::Arc) for any
/// `B: Allocator`. Generic code can ask for this with a
/// `where EcoVec<T, A>: Clone` bound. To share a vector backed by any other
/// allocator, pass the allocator by reference:
/// ```
/// use std::alloc::{AllocError, Allocator, Layout, System};
/// use std::ptr::NonNull;
/// use turbocow::EcoVec;
///
/// // Stands in for an arena or another allocator with state.
/// struct Arena;
///
/// unsafe impl Allocator for Arena {
///     fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
///         System.allocate(layout)
///     }
///
///     unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
///         unsafe { System.deallocate(ptr, layout) }
///     }
/// }
///
/// let arena = Arena;
/// let mut first = EcoVec::new_in(&arena);
/// first.extend_from_slice(&[1, 2, 3]);
///
/// // `EcoVec<i32, Arena>` is not `Clone`, but `EcoVec<i32, &Arena>` is: this
/// // shares the buffer, as all copies of `&Arena` are interchangeable.
/// let second = first.clone();
/// assert_eq!(first, second);
/// ```
#[repr(C)]
pub struct EcoVec<T, A = Global>
where
    A: Allocator,
{
    /// Is `Self::dangling()` when the vector is unallocated.
    ///
    /// Otherwise, points `Self::offset()` bytes after a valid allocation and
    /// header, to the start of the vector's elements. It is then aligned to the
    /// maximum of the header's alignment and T's alignment. The pointer is
    /// valid for `len` reads and `capacity` writes of T. The elements may only
    /// be accessed mutably if the reference-count is `1`.
    pub(super) ptr: NonNull<T>,
    /// The number of elements in the vector.
    ///
    /// Invariant: `len <= capacity`.
    pub(super) len: usize,
    /// The allocator used for this vector.
    pub(super) alloc: A,
    /// See Vec's impl for more details.
    pub(super) phantom: PhantomData<T>,
}

/// The start of the backing allocation.
///
/// This is followed by padding, if necessary, and then the actual data.
#[derive(Debug)]
pub(super) struct Header {
    /// The vector's reference count. Starts at 1 and only drops to zero
    /// when the last vector is dropped.
    ///
    /// Invariant: `refs <= dynamic::LEN_MAX`.
    pub(super) refs: AtomicUsize,
    /// The number of elements the backing allocation can hold. Zero if there
    /// is no backing allocation.
    ///
    /// May only be mutated if `refs == 1`.
    ///
    /// Invariant: `capacity <= dynamic::LEN_MAX`.
    pub(super) capacity: usize,
}

unsafe impl<T: Send + Sync, A: Allocator + Send + Sync> Send for EcoVec<T, A> {}

unsafe impl<T: Send + Sync, A: Allocator + Send + Sync> Sync for EcoVec<T, A> {}
