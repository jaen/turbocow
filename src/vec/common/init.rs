use core::marker::PhantomData;

use crate::allocator::{AllocatorProvider, Global};
use crate::vec::types::EcoVec;

impl<T> EcoVec<T, Global> {
    /// Create a new, empty vector using the global allocator.
    ///
    /// This is a `const fn` so it can be used in constant contexts.
    ///
    /// # Example
    /// ```
    /// use turbocow::EcoVec;
    /// const EMPTY: EcoVec<i32> = EcoVec::new();
    /// assert!(EMPTY.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> EcoVec<T, Global> {
        EcoVec::<T, Global>::new_in(Global)
    }

    /// Create a new, empty vector of at least the specified capacity using the default allocator.
    ///
    /// Pinned to [`Global`] so that `EcoVec::with_capacity(..)` infers the
    /// allocator without an explicit type annotation. Use
    /// [`with_capacity_in`](EcoVec::with_capacity_in) for a custom allocator.
    #[inline]
    pub fn with_capacity(capacity: usize) -> EcoVec<T, Global> {
        EcoVec::<T, Global>::with_capacity_in(capacity, Global)
    }
}

impl<T, A> EcoVec<T, A>
where
    A: AllocatorProvider,
{
    /// Create a new, empty vector using the specified allocator.
    #[inline(always)]
    pub const fn new_in(alloc: A) -> Self {
        Self {
            ptr: Self::dangling(),
            len: 0,
            alloc,
            phantom: PhantomData,
        }
    }

    /// Create a new, empty vector of at least the specified capacity using the specified allocator.
    #[inline(always)]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        let mut vec = Self::new_in(alloc);

        if capacity > 0 {
            unsafe {
                // Safety:
                // - The reference count starts at 1.
                // - The capacity starts at 0 and the target capacity is checked
                //   to be `> 0`.
                vec.grow(capacity);
            }
        }

        vec
    }
}

impl<T> EcoVec<T, Global>
where
    T: Clone,
{
    /// Create a new vector with `n` copies of `value` using the default allocator.
    ///
    /// Pinned to [`Global`] so the allocator is inferred without an explicit
    /// type annotation. Use [`from_elem_in`](EcoVec::from_elem_in) for a
    /// custom allocator.
    pub fn from_elem(value: T, n: usize) -> EcoVec<T, Global> {
        EcoVec::<T, Global>::from_elem_in(value, n, Global)
    }

    /// Create a new vector from a slice using the default allocator.
    ///
    /// Pinned to [`Global`] so the allocator is inferred without an explicit
    /// type annotation. Use [`from_slice_in`](EcoVec::from_slice_in) for a
    /// custom allocator.
    pub fn from_slice(slice: &[T]) -> EcoVec<T, Global> {
        EcoVec::<T, Global>::from_slice_in(slice, Global)
    }
}

impl<T, A> EcoVec<T, A>
where
    T: Clone,
    A: AllocatorProvider + Clone,
{
    /// Create a new vector with `n` copies of `value` using the specified allocator.
    pub fn from_elem_in(value: T, n: usize, alloc: A) -> Self {
        let mut vec = Self::with_capacity_in(n, alloc);

        for _ in 0..n {
            // Safety: we just called `with_capacity_in()`
            unsafe { vec.push_unchecked(value.clone()) }
        }

        vec
    }

    /// Create a new vector from a slice using the specified allocator.
    pub fn from_slice_in(slice: &[T], alloc: A) -> Self {
        let mut vec = Self::with_capacity_in(slice.len(), alloc);

        vec.extend_from_slice(slice);

        vec
    }
}

impl<T, A> EcoVec<T, A>
where
    T: Copy + Clone,
    A: AllocatorProvider + Clone,
{
    /// Create a new vector with `n` copies of `value` using memcpy.
    ///
    /// Faster than `from_elem_in` for `Copy` types: writes the first
    /// element then bulk-copies the rest.
    pub fn from_elem_copy_in(value: T, n: usize, alloc: A) -> Self {
        if n == 0 {
            return Self::new_in(alloc);
        }
        let mut vec = Self::with_capacity_in(n, alloc);
        unsafe {
            // Write the first element.
            core::ptr::write(vec.data_mut(), value);
            vec.len = 1;
            // Copy it to fill the rest.
            let mut filled = 1;
            while filled < n {
                let batch = (n - filled).min(filled);
                core::ptr::copy_nonoverlapping(
                    vec.data(),
                    vec.data_mut().add(filled),
                    batch,
                );
                filled += batch;
            }
            vec.len = n;
        }
        vec
    }

    /// Create a new vector from a slice using memcpy.
    ///
    /// Faster than `from_slice_in` for `Copy` types.
    pub fn from_copy_slice_in(slice: &[T], alloc: A) -> Self {
        let mut vec = Self::with_capacity_in(slice.len(), alloc);
        vec.extend_from_copy_slice(slice);
        vec
    }
}
