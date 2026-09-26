use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut, Index, IndexMut};
use core::ptr;
use core::slice;

use super::types::{
    HeapHeader, SmallVec, SmallVecData, TAG_HEAP, TAG_INLINE, TAG_REFERENCED, Variant,
};

// ── Construction ────────────────────────────────────────────────────────

impl<T, const N: usize> SmallVec<'_, T, N> {
    /// Creates a new, empty `SmallVec`.
    #[inline]
    pub fn new() -> Self {
        SmallVec {
            tagged_len: SmallVec::<T, N>::encode(TAG_INLINE, 0),
            data: SmallVecData {
                // SAFETY: MaybeUninit doesn't require initialization.
                inline: ManuallyDrop::new(unsafe { MaybeUninit::uninit().assume_init() }),
            },
            _marker: PhantomData,
        }
    }

    /// Creates a `SmallVec` with at least `capacity` elements of space.
    /// Immediately spills to heap if `capacity > N`.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity <= N { Self::new() } else { Self::new_heap(capacity) }
    }
}

impl<'a, T, const N: usize> SmallVec<'a, T, N> {
    /// Creates a Referenced variant that borrows the given slice.
    /// This is a zero-copy operation.
    #[inline]
    pub fn from_ref(slice: &'a [T]) -> Self {
        SmallVec {
            tagged_len: Self::encode(TAG_REFERENCED, slice.len()),
            data: SmallVecData { referenced: slice.as_ptr() },
            _marker: PhantomData,
        }
    }

    /// Converts a `SmallVec` with any lifetime into an owned `SmallVec<'static>`.
    ///
    /// If currently Referenced, materializes the data by cloning into
    /// inline or heap storage.
    pub fn into_owned(self) -> SmallVec<'static, T, N>
    where
        T: Clone,
    {
        match self.variant() {
            Variant::Inline => {
                // Inline data is already owned. Transmute the lifetime away.
                // SAFETY: Inline variant has no borrows — lifetime is phantom.
                unsafe {
                    mem::transmute::<SmallVec<'_, T, N>, SmallVec<'static, T, N>>(self)
                }
            }
            Variant::Referenced => {
                let slice = self.as_slice();
                let mut owned = if slice.len() <= N {
                    SmallVec::<'static, T, N>::new()
                } else {
                    SmallVec::<'static, T, N>::new_heap(slice.len())
                };
                for item in slice {
                    owned.push(item.clone());
                }
                // Don't drop self — Referenced has nothing to clean up.
                mem::forget(self);
                owned
            }
            Variant::Heap => {
                // Heap data is already owned.
                // SAFETY: Heap variant owns its allocation and holds no borrow —
                // the lifetime is phantom, so erasing it to 'static is sound.
                unsafe {
                    mem::transmute::<SmallVec<'_, T, N>, SmallVec<'static, T, N>>(self)
                }
            }
        }
    }
}

// ── Internal construction helpers ───────────────────────────────────────

impl<T, const N: usize> SmallVec<'_, T, N> {
    /// Allocate a new heap-backed SmallVec with the given capacity.
    fn new_heap(capacity: usize) -> Self {
        let layout = Self::heap_layout(capacity);
        // SAFETY: heap_layout always includes the HeapHeader, so the layout is
        // non-zero-sized; a null result is routed to handle_alloc_error.
        let alloc_ptr = unsafe {
            let ptr = alloc::alloc::alloc(layout);
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            ptr
        };
        // SAFETY: heap_offset() is in-bounds of the allocation; the element
        // storage begins there per the heap layout.
        let data_ptr = unsafe { alloc_ptr.add(Self::heap_offset()) as *mut T };
        // SAFETY: the header occupies [base, heap_offset()) at the allocation
        // base, which is allocated, aligned for HeapHeader, and uninitialized.
        unsafe {
            ptr::write(alloc_ptr as *mut HeapHeader, HeapHeader { capacity });
        }
        SmallVec {
            tagged_len: Self::encode(TAG_HEAP, 0),
            data: SmallVecData { heap: data_ptr },
            _marker: PhantomData,
        }
    }
}

// ── Access ──────────────────────────────────────────────────────────────

impl<T, const N: usize> SmallVec<'_, T, N> {
    /// Returns `true` if elements are stored inline (no heap allocation).
    #[inline]
    pub fn is_inline(&self) -> bool {
        self.tag() == TAG_INLINE
    }

    /// Returns `true` if this is a zero-copy borrow.
    #[inline]
    pub fn is_referenced(&self) -> bool {
        self.tag() == TAG_REFERENCED
    }

    /// Returns the number of elements.
    #[inline]
    pub fn len(&self) -> usize {
        self.raw_len()
    }

    /// Returns `true` if the vector contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw_len() == 0
    }

    /// Returns the current capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        match self.variant() {
            Variant::Inline => N,
            Variant::Referenced => self.raw_len(), // no extra capacity
            // SAFETY: Heap variant — data.heap is the live heap data pointer,
            // so the header sits heap_offset() bytes before it.
            Variant::Heap => unsafe { Self::header_from_ptr(self.data.heap).capacity },
        }
    }

    /// Returns a slice of all elements.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        let (ptr, len) = self.ptr_len();
        // SAFETY: ptr_len returns the variant's data pointer with its initialized
        // element count (raw_len); &self borrows them for the slice's lifetime.
        unsafe { slice::from_raw_parts(ptr, len) }
    }

    /// Returns a mutable slice of all elements.
    ///
    /// # Panics
    /// Panics if the vector is currently in the Referenced variant.
    /// Call `make_mut()` or `into_owned()` first.
    #[inline]
    #[track_caller]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        assert!(
            self.variant() != Variant::Referenced,
            "cannot mutably borrow a Referenced SmallVec; call make_mut() first"
        );
        let len = self.raw_len();
        let ptr = self.mut_data_ptr();
        // SAFETY: the assert above ruled out Referenced, so mut_data_ptr returns
        // the owned (Inline/Heap) data pointer with raw_len initialized elements;
        // &mut self guarantees exclusive access for the slice's lifetime.
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }

    /// Returns a raw pointer to the element storage.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.ptr_len().0
    }

    /// (ptr, len) pair for the current variant.
    #[inline]
    fn ptr_len(&self) -> (*const T, usize) {
        let len = self.raw_len();
        let ptr = match self.variant() {
            // SAFETY: each arm reads the union field the active variant selects:
            // Inline → the inline MaybeUninit array, Referenced → the borrowed
            // pointer, Heap → the heap data pointer.
            Variant::Inline => unsafe { (*self.data.inline).as_ptr() as *const T },
            Variant::Referenced => unsafe { self.data.referenced },
            Variant::Heap => unsafe { self.data.heap as *const T },
        };
        (ptr, len)
    }

    /// Mutable data pointer. Only valid for Inline and Heap.
    #[inline]
    fn mut_data_ptr(&mut self) -> *mut T {
        match self.variant() {
            // SAFETY: reads the union field the active variant selects: Inline →
            // the inline array, Heap → the heap data pointer. Referenced has no
            // mutable owned storage and is excluded via unreachable!.
            Variant::Inline => unsafe { (*self.data.inline).as_mut_ptr() as *mut T },
            Variant::Heap => unsafe { self.data.heap },
            Variant::Referenced => unreachable!("mut_data_ptr called on Referenced"),
        }
    }
}

// ── Mutation ────────────────────────────────────────────────────────────

impl<T, const N: usize> SmallVec<'_, T, N> {
    /// Ensures the vector is in an owned, mutable state.
    /// If Referenced, materializes the data.
    #[inline]
    pub fn make_mut(&mut self)
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
    }

    /// Materialise a Referenced variant into owned (Inline or Heap).
    #[cold]
    fn materialise_referenced(&mut self)
    where
        T: Clone,
    {
        debug_assert!(self.variant() == Variant::Referenced);
        let slice = self.as_slice();
        let len = slice.len();

        if len <= N {
            // Clone into inline storage.
            let mut new = Self::new();
            for item in slice {
                new.push(item.clone());
            }
            *self = new;
        } else {
            // Clone into heap storage.
            let mut new = Self::new_heap(len);
            // SAFETY: new is a freshly allocated Heap variant, so data.heap is
            // the active field and points at storage for `len` (== capacity) elements.
            let dst = unsafe { new.data.heap };

            // Panic-safety: if T::clone panics on iteration i, the i already-written
            // elements must be dropped during unwind. CloneGuard tracks the count
            // and drops them via slice drop_in_place; mem::forget(guard) on the
            // success path skips the drop. Without this guard, new.tagged_len stays
            // at encode(TAG_HEAP, 0) and SmallVec::drop skips the cloned elements,
            // leaking them. Mirrors the Clone impl's CloneGuard at lines ~1040-1066.
            struct CloneGuard<T> {
                ptr: *mut T,
                len: usize,
            }
            impl<T> Drop for CloneGuard<T> {
                fn drop(&mut self) {
                    // SAFETY: ptr[0..len] hold the elements cloned so far (guard.len
                    // bumped only after each successful clone), so they are dropped
                    // exactly once on panic.
                    unsafe {
                        ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                            self.ptr, self.len,
                        ));
                    }
                }
            }

            let mut guard = CloneGuard { ptr: dst, len: 0 };
            for (i, item) in slice.iter().enumerate() {
                // SAFETY: i < len <= capacity, so dst.add(i) is in-bounds and the
                // slot is uninitialized; ptr::write does not drop the prior contents.
                unsafe { ptr::write(dst.add(i), item.clone()) };
                guard.len += 1;
            }
            mem::forget(guard);

            new.tagged_len = Self::encode(TAG_HEAP, len);
            *self = new;
        }
    }

    /// Appends an element to the back.
    #[inline]
    pub fn push(&mut self, value: T)
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        self.push_owned(value);
    }

    /// Push onto an owned (Inline or Heap) variant.
    ///
    /// # Panics
    /// Panics (debug) if called on a Referenced variant.
    #[inline]
    pub fn push_owned(&mut self, value: T) {
        let len = self.raw_len();
        match self.variant() {
            Variant::Inline => {
                if len < N {
                    // SAFETY: Inline variant, so data.inline is active; len < N
                    // bounds the index into the [MaybeUninit<T>; N] array and the
                    // overwritten slot is uninitialized (no drop needed).
                    unsafe {
                        let arr = &mut *self.data.inline;
                        arr[len] = MaybeUninit::new(value);
                    }
                    self.tagged_len = Self::encode(TAG_INLINE, len + 1);
                } else {
                    self.spill_and_push(value);
                }
            }
            Variant::Heap => {
                // SAFETY: Heap variant — data.heap is the live data pointer, so
                // its header sits heap_offset() bytes before it.
                let cap = unsafe { Self::header_from_ptr(self.data.heap).capacity };
                if len < cap {
                    // SAFETY: len < cap, so data.heap.add(len) is an in-bounds,
                    // uninitialized slot; ptr::write does not drop prior contents.
                    unsafe {
                        ptr::write(self.data.heap.add(len), value);
                    }
                    self.tagged_len = Self::encode(TAG_HEAP, len + 1);
                } else {
                    self.grow_and_push(value);
                }
            }
            Variant::Referenced => unreachable!(),
        }
    }

    #[cold]
    #[inline(never)]
    fn spill_and_push(&mut self, value: T) {
        self.spill_with_extra(1);
        let len = self.raw_len();
        // SAFETY: spill_with_extra left us Heap with capacity for `len + 1`, so
        // data.heap.add(len) is an in-bounds, uninitialized slot.
        unsafe {
            ptr::write(self.data.heap.add(len), value);
        }
        self.tagged_len = Self::encode(TAG_HEAP, len + 1);
    }

    #[cold]
    #[inline(never)]
    fn grow_and_push(&mut self, value: T) {
        self.grow_heap(1);
        let len = self.raw_len();
        // SAFETY: grow_heap left us Heap with capacity for `len + 1`, so
        // data.heap.add(len) is an in-bounds, uninitialized slot.
        unsafe {
            ptr::write(self.data.heap.add(len), value);
        }
        self.tagged_len = Self::encode(TAG_HEAP, len + 1);
    }

    /// Removes the last element and returns it, or `None` if empty.
    #[inline]
    pub fn pop(&mut self) -> Option<T>
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        let len = self.raw_len();
        if len == 0 {
            return None;
        }
        let new_len = len - 1;
        // SAFETY: materialise_referenced above left us owned, so mut_data_ptr is
        // valid; new_len < len indexes an initialized element, and read moves it
        // out — the length is dropped to new_len below so it is not double-freed.
        let val = unsafe {
            let ptr = self.mut_data_ptr();
            ptr.add(new_len).read()
        };
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, new_len);
        Some(val)
    }

    /// Inserts an element at position `index`, shifting all elements after
    /// it to the right.
    ///
    /// # Panics
    /// Panics if `index > len`.
    #[track_caller]
    pub fn insert(&mut self, index: usize, value: T)
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        let len = self.raw_len();
        assert!(index <= len, "index out of bounds");

        // Ensure capacity.
        match self.variant() {
            Variant::Inline => {
                if len >= N {
                    self.spill_with_extra(1);
                }
            }
            Variant::Heap => {
                // SAFETY: Heap variant — data.heap is the live data pointer, so
                // its header sits heap_offset() bytes before it.
                let cap = unsafe { Self::header_from_ptr(self.data.heap).capacity };
                if len >= cap {
                    self.grow_heap(1);
                }
            }
            Variant::Referenced => unreachable!(),
        }

        let ptr = self.mut_data_ptr();
        // SAFETY: owned (not Referenced) with capacity >= len + 1 ensured above.
        // index <= len, so p..p+(len-index) are initialized and p+1..p+1+(len-index)
        // stay in-bounds; the shift opens an uninitialized slot at index that the
        // following write fills (no drop of prior contents).
        unsafe {
            let p = ptr.add(index);
            ptr::copy(p, p.add(1), len - index);
            ptr::write(p, value);
        }
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, len + 1);
    }

    /// Removes and returns the element at position `index`.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> T
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        let len = self.raw_len();
        assert!(index < len, "index out of bounds");

        let ptr = self.mut_data_ptr();
        // SAFETY: owned (materialised above) with index < len, so p indexes an
        // initialized element. read moves it out, then the tail [index+1, len) is
        // shifted down to fill the gap; len is decremented below so the vacated
        // last slot is no longer considered live.
        let val = unsafe {
            let p = ptr.add(index);
            let val = p.read();
            ptr::copy(p.add(1), p, len - index - 1);
            val
        };
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, len - 1);
        val
    }

    /// Removes an element at `index` by swapping it with the last element.
    /// O(1) but does not preserve order.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    #[track_caller]
    pub fn swap_remove(&mut self, index: usize) -> T
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        let len = self.raw_len();
        assert!(index < len, "index out of bounds");

        let ptr = self.mut_data_ptr();
        let new_len = len - 1;
        // SAFETY: owned (materialised above) with index < len. read moves out the
        // element at index, then the (distinct, hence non-overlapping) last element
        // is moved into the vacated slot; len drops to new_len below so the old
        // last slot is no longer live.
        let val = unsafe {
            let val = ptr.add(index).read();
            if index != new_len {
                ptr::copy_nonoverlapping(ptr.add(new_len), ptr.add(index), 1);
            }
            val
        };
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, new_len);
        val
    }

    /// Like [`swap_remove`](Self::swap_remove), but only works on owned
    /// (Inline or Heap) variants.  Does **not** require `T: Clone`.
    ///
    /// # Panics
    /// - Panics if `index >= len`.
    /// - Debug-panics if called on a Referenced variant.
    #[track_caller]
    pub fn swap_remove_owned(&mut self, index: usize) -> T {
        debug_assert_ne!(self.variant(), Variant::Referenced);
        let len = self.raw_len();
        assert!(index < len, "index out of bounds");

        let ptr = self.mut_data_ptr();
        let new_len = len - 1;
        // SAFETY: caller-owned (debug-asserted not Referenced) with index < len.
        // read moves out the element at index, then the (distinct, non-overlapping)
        // last element fills the vacated slot; len drops to new_len below so the
        // old last slot is no longer live.
        let val = unsafe {
            let val = ptr.add(index).read();
            if index != new_len {
                ptr::copy_nonoverlapping(ptr.add(new_len), ptr.add(index), 1);
            }
            val
        };
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, new_len);
        val
    }

    /// Clears the vector, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        match self.variant() {
            Variant::Inline => {
                let len = self.raw_len();
                // SAFETY: Inline variant — data.inline is the active field.
                let ptr = unsafe { (*self.data.inline).as_mut_ptr() as *mut T };
                // Set len to 0 first for panic safety.
                self.tagged_len = Self::encode(TAG_INLINE, 0);
                // SAFETY: ptr[0..len] were initialized; dropping in place is sound
                // and len is already 0 so a panicking Drop won't re-drop them.
                unsafe {
                    ptr::drop_in_place(ptr::slice_from_raw_parts_mut(ptr, len));
                }
            }
            Variant::Referenced => {
                // Nothing to drop — just reset to empty inline.
                self.tagged_len = Self::encode(TAG_INLINE, 0);
                self.data = SmallVecData {
                    // SAFETY: MaybeUninit doesn't require initialization.
                    inline: ManuallyDrop::new(unsafe {
                        MaybeUninit::uninit().assume_init()
                    }),
                };
            }
            Variant::Heap => {
                let len = self.raw_len();
                // SAFETY: Heap variant — data.heap is the active field.
                let ptr = unsafe { self.data.heap };
                self.tagged_len = Self::encode(TAG_HEAP, 0);
                // SAFETY: ptr[0..len] were initialized; len is already 0 so a
                // panicking Drop won't re-drop them. Allocation is freed by Drop.
                unsafe {
                    ptr::drop_in_place(ptr::slice_from_raw_parts_mut(ptr, len));
                }
            }
        }
    }

    /// Shortens the vector, keeping the first `new_len` elements.
    pub fn truncate(&mut self, new_len: usize)
    where
        T: Clone,
    {
        let old_len = self.raw_len();
        if new_len >= old_len {
            return;
        }
        if self.variant() == Variant::Referenced {
            // Just shorten the view.
            self.tagged_len = Self::encode(TAG_REFERENCED, new_len);
            return;
        }
        let ptr = self.mut_data_ptr();
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, new_len);
        // SAFETY: owned (Referenced returned early), new_len < old_len, so
        // ptr[new_len..old_len] are initialized and in-bounds; len is already
        // new_len so these dropped elements are no longer considered live.
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                ptr.add(new_len),
                old_len - new_len,
            ));
        }
    }

    /// Reserves capacity for at least `additional` more elements.
    #[inline]
    pub fn reserve(&mut self, additional: usize)
    where
        T: Clone,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        let len = self.raw_len();
        let needed = len + additional;
        match self.variant() {
            Variant::Inline => {
                if needed > N {
                    self.spill_with_extra(additional);
                }
            }
            Variant::Heap => {
                // SAFETY: Heap variant — data.heap is the live data pointer, so
                // its header sits heap_offset() bytes before it.
                let cap = unsafe { Self::header_from_ptr(self.data.heap).capacity };
                if needed > cap {
                    self.grow_heap(additional);
                }
            }
            Variant::Referenced => unreachable!(),
        }
    }

    /// Retains only the elements specified by the predicate.
    pub fn retain<F>(&mut self, mut f: F)
    where
        T: Clone,
        F: FnMut(&T) -> bool,
    {
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        let ptr = self.mut_data_ptr();
        let mut len = self.raw_len();
        let tag = self.tag();
        let mut i = 0;
        while i < len {
            // SAFETY: owned (materialised above), i < len, so ptr.add(i) is an
            // initialized element borrowed only for the predicate call.
            let keep = unsafe { f(&*ptr.add(i)) };
            if keep {
                i += 1;
            } else {
                // SAFETY: ptr.add(i) is the initialized element being dropped;
                // the remaining tail [i+1, len) is then shifted down to fill the
                // gap, and len decrements so the vacated last slot becomes dead.
                unsafe {
                    ptr::drop_in_place(ptr.add(i));
                    ptr::copy(ptr.add(i + 1), ptr.add(i), len - i - 1);
                }
                len -= 1;
                // Keep the live length current. The shift above leaves a bitwise
                // duplicate of the old last element in the vacated tail slot, so
                // if `f` panics on a later iteration, `Drop` must not treat that
                // slot as live (it would double-drop). Bounding `tagged_len` to
                // the [0, len) live prefix now makes the unwind path sound.
                self.tagged_len = Self::encode(tag, len);
            }
        }
        self.tagged_len = Self::encode(tag, len);
    }

    /// Extends the vector from a slice where `T: Clone`.
    pub fn extend_from_slice(&mut self, slice: &[T])
    where
        T: Clone,
    {
        if slice.is_empty() {
            return;
        }
        self.reserve(slice.len());
        let len = self.raw_len();
        // SAFETY: reserve ensured owned (not Referenced) with capacity for
        // len + slice.len(); dst = data_ptr + len is the first spare slot, in-bounds.
        let dst = unsafe { self.mut_data_ptr().add(len) };

        // Clone each element directly into the destination.
        // Use a guard to drop already-cloned elements on panic.
        let mut cloned = 0usize;
        struct CloneGuard<T> {
            dst: *mut T,
            cloned: *mut usize,
        }
        impl<T> Drop for CloneGuard<T> {
            fn drop(&mut self) {
                // SAFETY: `cloned` points at the live local `cloned` counter,
                // valid for the guard's lifetime.
                let n = unsafe { *self.cloned };
                if n > 0 {
                    // SAFETY: on panic, dst[0..n] hold the n elements cloned so
                    // far; dropping them in place undoes the partial extension
                    // (the vec's length was not yet bumped to include them).
                    unsafe {
                        ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.dst, n));
                    }
                }
            }
        }
        let guard = CloneGuard { dst, cloned: &mut cloned };
        #[allow(unused_assignments)] // read through raw pointer in CloneGuard::drop
        for (i, item) in slice.iter().enumerate() {
            // SAFETY: i < slice.len(), so dst.add(i) is an in-bounds spare slot
            // (capacity reserved above); write does not drop prior contents.
            unsafe {
                dst.add(i).write(item.clone());
            }
            cloned += 1;
        }
        mem::forget(guard);

        let tag = self.tag();
        self.tagged_len = Self::encode(tag, len + slice.len());
    }

    /// Returns a draining iterator over the given range.
    ///
    /// # Panics
    /// Panics if the range is out of bounds or if Referenced.
    #[track_caller]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T, N>
    where
        R: core::ops::RangeBounds<usize>,
    {
        use core::ops::Bound;

        assert!(
            self.variant() != Variant::Referenced,
            "cannot drain a Referenced SmallVec"
        );

        let len = self.raw_len();
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };
        assert!(start <= end && end <= len, "drain range out of bounds");

        let tag = self.tag();
        self.tagged_len = Self::encode(tag, start);

        // SAFETY: We've asserted this is not Referenced, so the SmallVec
        // is either Inline or Heap — both are owned with 'static-safe data.
        // The lifetime erasure is safe because Drain holds &mut self which
        // prevents any other access, and we restore the correct state on drop.
        // clippy reads the `'_` -> `'static` pointer cast as a no-op (it
        // erases lifetimes), but the cast is the whole point: it launders
        // the lifetime per the SAFETY note above.
        #[allow(clippy::unnecessary_cast)]
        let vec_static: &mut SmallVec<'static, T, N> = unsafe {
            &mut *(self as *mut SmallVec<'_, T, N> as *mut SmallVec<'static, T, N>)
        };

        Drain {
            vec: vec_static,
            drain_start: start,
            drain_end: end,
            original_len: len,
            current: start,
        }
    }

    // ── Spill / grow helpers ────────────────────────────────────────

    /// Spill inline data to a heap allocation with room for `extra` more.
    fn spill_with_extra(&mut self, extra: usize) {
        debug_assert!(self.variant() == Variant::Inline);
        let len = self.raw_len();
        let new_cap = (len + extra).max(N * 2);
        let layout = Self::heap_layout(new_cap);
        // SAFETY: heap_layout always includes the HeapHeader, so the layout is
        // non-zero-sized; a null result is routed to handle_alloc_error.
        let alloc_ptr = unsafe {
            let p = alloc::alloc::alloc(layout);
            if p.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            p
        };
        // SAFETY: heap_offset() is in-bounds of the allocation; element storage
        // begins there per the heap layout.
        let data_ptr = unsafe { alloc_ptr.add(Self::heap_offset()) as *mut T };

        // Move inline elements to the heap allocation.
        // SAFETY: still Inline (debug-asserted), so data.inline is active and
        // src[0..len] are initialized; the fresh allocation is disjoint from the
        // inline array (non-overlapping copy) and has capacity new_cap >= len. The
        // header write targets the allocation base, aligned and uninitialized.
        unsafe {
            let src = (*self.data.inline).as_ptr() as *const T;
            ptr::copy_nonoverlapping(src, data_ptr, len);
            ptr::write(alloc_ptr as *mut HeapHeader, HeapHeader { capacity: new_cap });
        }

        self.tagged_len = Self::encode(TAG_HEAP, len);
        self.data = SmallVecData { heap: data_ptr };
    }

    /// Grow an existing heap allocation to hold `extra` more elements.
    fn grow_heap(&mut self, extra: usize) {
        debug_assert!(self.variant() == Variant::Heap);
        let len = self.raw_len();
        // SAFETY: Heap variant (debug-asserted) — data.heap is the live data
        // pointer, so its header sits heap_offset() bytes before it.
        let old_cap = unsafe { Self::header_from_ptr(self.data.heap).capacity };
        let new_cap = (len + extra).max(old_cap * 2);
        let old_layout = Self::heap_layout(old_cap);
        let new_layout = Self::heap_layout(new_cap);

        // SAFETY: data.heap is the live Heap data pointer, so subtracting
        // heap_offset() recovers the allocation base.
        let old_alloc = unsafe { Self::alloc_ptr_from_data(self.data.heap) };
        // SAFETY: old_alloc/old_layout describe the current allocation; realloc to
        // the non-zero-sized new_layout preserves the existing bytes (header +
        // len elements). A null result is routed to handle_alloc_error.
        let new_alloc = unsafe {
            let p = alloc::alloc::realloc(old_alloc, old_layout, new_layout.size());
            if p.is_null() {
                alloc::alloc::handle_alloc_error(new_layout);
            }
            p
        };
        // SAFETY: heap_offset() is in-bounds of the reallocated block.
        let data_ptr = unsafe { new_alloc.add(Self::heap_offset()) as *mut T };
        // SAFETY: the header lives at the allocation base, preserved by realloc;
        // updating its capacity field in place is sound.
        unsafe {
            (*(new_alloc as *mut HeapHeader)).capacity = new_cap;
        }
        self.data = SmallVecData { heap: data_ptr };
    }

    /// Converts into a `Vec<T>`, moving elements out.
    pub fn into_vec(mut self) -> Vec<T>
    where
        T: Clone,
    {
        match self.variant() {
            Variant::Referenced => {
                let slice = self.as_slice();
                let v = slice.to_vec();
                mem::forget(self);
                v
            }
            Variant::Inline | Variant::Heap => {
                let len = self.raw_len();
                let mut v = Vec::with_capacity(len);
                let src = self.ptr_len().0;
                // SAFETY: owned (Inline/Heap) so src[0..len] are initialized; v was
                // allocated with capacity >= len and is disjoint from src, so the
                // non-overlapping bitwise move into it and the matching set_len are
                // sound. self's length is zeroed below so the moved-out elements are
                // not double-freed by its Drop.
                unsafe {
                    ptr::copy_nonoverlapping(src, v.as_mut_ptr(), len);
                    v.set_len(len);
                }
                // Prevent dropping of moved elements.
                let tag = self.tag();
                self.tagged_len = Self::encode(tag, 0);
                v
            }
        }
    }
}

// ── Deref / DerefMut ────────────────────────────────────────────────────

impl<T, const N: usize> Deref for SmallVec<'_, T, N> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for SmallVec<'_, T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T, const N: usize> AsRef<[T]> for SmallVec<'_, T, N> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> Borrow<[T]> for SmallVec<'_, T, N> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}

// ── Index ───────────────────────────────────────────────────────────────

impl<T, const N: usize> Index<usize> for SmallVec<'_, T, N> {
    type Output = T;

    #[inline]
    #[track_caller]
    fn index(&self, index: usize) -> &T {
        &self.as_slice()[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for SmallVec<'_, T, N> {
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.as_mut_slice()[index]
    }
}

// ── Comparison ──────────────────────────────────────────────────────────

impl<T: PartialEq, const N: usize> PartialEq for SmallVec<'_, T, N> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq, const N: usize> Eq for SmallVec<'_, T, N> {}

impl<T: PartialEq, const N: usize> PartialEq<[T]> for SmallVec<'_, T, N> {
    #[inline]
    fn eq(&self, other: &[T]) -> bool {
        self.as_slice() == other
    }
}

impl<T: PartialEq, const N: usize> PartialEq<Vec<T>> for SmallVec<'_, T, N> {
    #[inline]
    fn eq(&self, other: &Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: PartialEq, const N: usize, const M: usize> PartialEq<[T; M]>
    for SmallVec<'_, T, N>
{
    #[inline]
    fn eq(&self, other: &[T; M]) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: PartialOrd, const N: usize> PartialOrd for SmallVec<'_, T, N> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord, const N: usize> Ord for SmallVec<'_, T, N> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: Hash, const N: usize> Hash for SmallVec<'_, T, N> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

// ── Clone ───────────────────────────────────────────────────────────────

impl<T: Clone, const N: usize> Clone for SmallVec<'_, T, N> {
    fn clone(&self) -> Self {
        match self.variant() {
            Variant::Inline => {
                let len = self.raw_len();
                struct CloneGuard<'a, T> {
                    data: &'a mut [MaybeUninit<T>],
                    len: usize,
                }
                impl<T> Drop for CloneGuard<'_, T> {
                    fn drop(&mut self) {
                        for i in 0..self.len {
                            // SAFETY: data[0..len] hold the elements cloned so far
                            // (guard.len is bumped only after each successful clone),
                            // so each is initialized and dropped exactly once on panic.
                            unsafe { ptr::drop_in_place(self.data[i].as_mut_ptr()) };
                        }
                    }
                }

                // SAFETY: MaybeUninit doesn't require initialization.
                let mut new_data: [MaybeUninit<T>; N] =
                    unsafe { MaybeUninit::uninit().assume_init() };
                let mut guard = CloneGuard { data: &mut new_data, len: 0 };

                // SAFETY: Inline variant — data.inline is the active field.
                let src = unsafe { &*self.data.inline };
                for (i, slot) in src.iter().enumerate().take(len) {
                    // SAFETY: i < len, so src[i] is one of the initialized elements.
                    let item = unsafe { slot.assume_init_ref() };
                    guard.data[i] = MaybeUninit::new(item.clone());
                    guard.len += 1;
                }
                mem::forget(guard);

                SmallVec {
                    tagged_len: self.tagged_len,
                    data: SmallVecData { inline: ManuallyDrop::new(new_data) },
                    _marker: PhantomData,
                }
            }
            Variant::Referenced => {
                // Clone a Referenced is just copying the pointer + tagged_len.
                SmallVec {
                    tagged_len: self.tagged_len,
                    // SAFETY: Referenced variant — data.referenced is the active
                    // field; copying the borrowed pointer keeps the same borrow.
                    data: SmallVecData { referenced: unsafe { self.data.referenced } },
                    _marker: PhantomData,
                }
            }
            Variant::Heap => {
                let len = self.raw_len();
                let mut new = Self::new_heap(len);

                // SAFETY: self is Heap, so data.heap is the active field and
                // src[0..len] are initialized.
                let src = unsafe { self.data.heap as *const T };
                // SAFETY: new is a fresh Heap variant with capacity >= len, so
                // data.heap is active and points at storage for `len` elements.
                let dst = unsafe { new.data.heap };

                struct CloneGuard<T> {
                    ptr: *mut T,
                    len: usize,
                }
                impl<T> Drop for CloneGuard<T> {
                    fn drop(&mut self) {
                        // SAFETY: ptr[0..len] hold the elements cloned so far
                        // (guard.len bumped only after each successful clone), so
                        // they are dropped exactly once on panic.
                        unsafe {
                            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                                self.ptr, self.len,
                            ));
                        }
                    }
                }

                let mut guard = CloneGuard { ptr: dst, len: 0 };
                for i in 0..len {
                    // SAFETY: i < len, so src.add(i) reads an initialized element
                    // and dst.add(i) is an in-bounds uninitialized destination slot.
                    unsafe {
                        ptr::write(dst.add(i), (*src.add(i)).clone());
                    }
                    guard.len += 1;
                }
                mem::forget(guard);

                new.tagged_len = Self::encode(TAG_HEAP, len);
                new
            }
        }
    }
}

// ── Drop ────────────────────────────────────────────────────────────────

impl<T, const N: usize> Drop for SmallVec<'_, T, N> {
    fn drop(&mut self) {
        match self.variant() {
            Variant::Inline => {
                let len = self.raw_len();
                if len > 0 && mem::needs_drop::<T>() {
                    // SAFETY: Inline variant — data.inline is the active field.
                    let ptr = unsafe { (*self.data.inline).as_mut_ptr() as *mut T };
                    // SAFETY: ptr[0..len] are initialized; this is the owner's
                    // final drop so each is dropped exactly once.
                    unsafe {
                        ptr::drop_in_place(ptr::slice_from_raw_parts_mut(ptr, len));
                    }
                }
            }
            Variant::Referenced => {
                // Nothing to drop — borrowed data.
            }
            Variant::Heap => {
                let len = self.raw_len();
                // SAFETY: Heap variant — data.heap is the active field.
                let data_ptr = unsafe { self.data.heap };

                if len > 0 && mem::needs_drop::<T>() {
                    // SAFETY: data_ptr[0..len] are initialized; final drop, so
                    // each element is dropped exactly once.
                    unsafe {
                        ptr::drop_in_place(ptr::slice_from_raw_parts_mut(data_ptr, len));
                    }
                }

                // Deallocate.
                // SAFETY: data_ptr is the live Heap data pointer, so its header
                // sits heap_offset() bytes before it.
                let cap = unsafe { Self::header_from_ptr(data_ptr).capacity };
                let layout = Self::heap_layout(cap);
                // SAFETY: recovers the allocation base from the data pointer.
                let alloc_ptr = unsafe { Self::alloc_ptr_from_data(data_ptr) };
                // SAFETY: alloc_ptr/layout match the allocation made by
                // new_heap/spill/grow (capacity from the header), so freeing it
                // with the same layout is sound.
                unsafe {
                    alloc::alloc::dealloc(alloc_ptr, layout);
                }
            }
        }
    }
}

// ── Debug ───────────────────────────────────────────────────────────────

impl<T: fmt::Debug, const N: usize> fmt::Debug for SmallVec<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

// ── Default ─────────────────────────────────────────────────────────────

impl<T, const N: usize> Default for SmallVec<'_, T, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ── From conversions ────────────────────────────────────────────────────

impl<T: Clone, const N: usize> From<Vec<T>> for SmallVec<'_, T, N> {
    fn from(v: Vec<T>) -> Self {
        if v.len() <= N {
            let mut sv = SmallVec::new();
            for item in &v {
                sv.push_owned(item.clone());
            }
            sv
        } else {
            // Move Vec data into our header-allocation.
            let len = v.len();
            let mut sv = Self::new_heap(len);
            // SAFETY: sv is a fresh Heap variant with capacity >= len, so
            // data.heap points at storage for `len` elements; it is disjoint from
            // v's buffer, so the non-overlapping bitwise move of v[0..len] is sound.
            unsafe {
                ptr::copy_nonoverlapping(v.as_ptr(), sv.data.heap, len);
            }
            sv.tagged_len = Self::encode(TAG_HEAP, len);
            // Prevent Vec from dropping the elements we just moved.
            let mut v = v;
            // SAFETY: the len elements were moved into sv; setting v's length to 0
            // stops it from double-dropping them (the buffer is still freed by v).
            unsafe { v.set_len(0) };
            sv
        }
    }
}

impl<T: Clone, const N: usize> From<&[T]> for SmallVec<'_, T, N> {
    fn from(slice: &[T]) -> Self {
        let mut sv = SmallVec::with_capacity(slice.len());
        sv.extend_from_slice(slice);
        sv
    }
}

impl<T, const N: usize, const M: usize> From<[T; M]> for SmallVec<'_, T, N> {
    fn from(array: [T; M]) -> Self {
        if M <= N {
            let mut sv = SmallVec::<T, N>::new();
            let src = &array as *const [T; M] as *const T;
            // SAFETY: M <= N, so the array's M elements fit the inline storage;
            // sv is fresh Inline (data.inline active) and disjoint from `array`, so
            // the non-overlapping bitwise move of all M elements is sound.
            unsafe {
                let dst = (*sv.data.inline).as_mut_ptr() as *mut T;
                ptr::copy_nonoverlapping(src, dst, M);
            }
            sv.tagged_len = Self::encode(TAG_INLINE, M);
            // array's elements were moved out; forget it so they are not dropped.
            mem::forget(array);
            sv
        } else {
            let mut sv = Self::new_heap(M);
            let src = &array as *const [T; M] as *const T;
            // SAFETY: sv is a fresh Heap variant with capacity >= M (data.heap
            // active), disjoint from `array`, so the non-overlapping bitwise move
            // of all M elements is sound.
            unsafe {
                ptr::copy_nonoverlapping(src, sv.data.heap, M);
            }
            sv.tagged_len = Self::encode(TAG_HEAP, M);
            // array's elements were moved out; forget it so they are not dropped.
            mem::forget(array);
            sv
        }
    }
}

impl<T: Clone, const N: usize> From<SmallVec<'_, T, N>> for Vec<T> {
    fn from(sv: SmallVec<'_, T, N>) -> Vec<T> {
        sv.into_vec()
    }
}

// ── Iteration ───────────────────────────────────────────────────────────

impl<T: Clone, const N: usize> Extend<T> for SmallVec<'_, T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();
        if hint > 0 {
            self.reserve(hint);
        }
        for item in iter {
            self.push_owned(item);
        }
    }
}

impl<T: Clone, const N: usize> FromIterator<T> for SmallVec<'_, T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut sv = SmallVec::new();
        sv.extend(iter);
        sv
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a SmallVec<'_, T, N> {
    type IntoIter = slice::Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut SmallVec<'_, T, N> {
    type IntoIter = slice::IterMut<'a, T>;
    type Item = &'a mut T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

// `T: Clone` is required because owning iteration over a Referenced
// (zero-copy borrow) variant must produce *owned* values, which means the
// borrowed elements have to be cloned into owned storage first. This mirrors
// the clone-on-write design of the other owning conversions
// (`make_mut` / `into_owned` / `into_vec` / `clone`), all of which require
// `T: Clone`. Inline and Heap variants already own their elements and would
// not strictly need the bound, but a single uniform impl keeps the API
// coherent and matches the rest of the type. This does not affect the ecow
// superset: `EcoVec` is the ecow-relevant type and has its own iterator with
// no Referenced variant.
impl<T: Clone, const N: usize> IntoIterator for SmallVec<'_, T, N> {
    type IntoIter = IntoIter<T, N>;
    type Item = T;

    fn into_iter(mut self) -> IntoIter<T, N> {
        // Materialise a Referenced variant into owned (Inline/Heap) storage
        // BEFORE erasing the lifetime. Without this, `IntoIter` would hold a
        // borrowed pointer (`data.referenced`) and `next`/`next_back` would
        // `ptr::read` (bitwise-move) owned values out of memory the caller
        // still owns — a move-from-borrow that double-frees / use-after-frees
        // when both the collected container and the original slice drop.
        // After this, the variant is guaranteed to be Inline or Heap.
        if self.variant() == Variant::Referenced {
            self.materialise_referenced();
        }
        debug_assert!(self.variant() != Variant::Referenced);

        let len = self.raw_len();
        // Set length to 0 so the source SmallVec's Drop doesn't also try to
        // drop the elements that `IntoIter` now owns and will drop itself.
        let tag = self.tag();
        self.tagged_len = Self::encode(tag, 0);

        // SAFETY: The variant is now Inline or Heap (never Referenced — we
        // materialised it above), so the element data is owned in-place
        // (Inline array or Heap alloc) and self-contained — there is no
        // borrow tied to the original lifetime. Erasing the lifetime to
        // 'static is therefore sound, and `IntoIter` never owns a borrowed
        // pointer.
        let vec: SmallVec<'static, T, N> = unsafe { mem::transmute(self) };

        IntoIter { vec, front: 0, back: len }
    }
}

// ── IntoIter ────────────────────────────────────────────────────────────

/// Owned iterator over a [`SmallVec`].
pub struct IntoIter<T, const N: usize> {
    vec: SmallVec<'static, T, N>,
    front: usize,
    back: usize,
}

impl<T, const N: usize> IntoIter<T, N> {
    /// Returns the remaining elements as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        let ptr = self.vec.ptr_len().0;
        // SAFETY: vec is owned (Inline/Heap); [front, back) is the not-yet-yielded
        // range and every element in it is still initialized, so the slice is valid.
        unsafe { slice::from_raw_parts(ptr.add(self.front), self.back - self.front) }
    }
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.front >= self.back {
            return None;
        }
        let ptr = self.vec.ptr_len().0;
        // SAFETY: front < back, so ptr.add(front) is an initialized element; read
        // moves it out and front advances so it is never read or dropped again.
        let val = unsafe { ptr.add(self.front).read() };
        self.front += 1;
        Some(val)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.back - self.front;
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize {
        self.back - self.front
    }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        if self.back <= self.front {
            return None;
        }
        self.back -= 1;
        let ptr = self.vec.ptr_len().0;
        // SAFETY: back was > front, now decremented, so ptr.add(back) is an
        // initialized element; read moves it out and back already excludes it, so
        // it is never read or dropped again.
        Some(unsafe { ptr.add(self.back).read() })
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T, const N: usize> Drop for IntoIter<T, N> {
    fn drop(&mut self) {
        // Drop remaining elements that haven't been yielded.
        if self.back > self.front {
            let ptr = self.vec.mut_data_ptr();
            // SAFETY: vec is owned (Inline/Heap); the un-yielded range [front, back)
            // is still initialized, and each element is dropped exactly once here
            // (the vec's len was set to 0 in into_iter, so its Drop won't re-drop).
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                    ptr.add(self.front),
                    self.back - self.front,
                ));
            }
        }
        // The SmallVec's Drop will handle deallocation (len is already 0).
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for IntoIter<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

// ── Drain ───────────────────────────────────────────────────────────────

/// A draining iterator over a [`SmallVec`].
pub struct Drain<'a, T, const N: usize> {
    vec: &'a mut SmallVec<'static, T, N>,
    drain_start: usize,
    drain_end: usize,
    original_len: usize,
    current: usize,
}

impl<T, const N: usize> Iterator for Drain<'_, T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.current >= self.drain_end {
            return None;
        }
        let ptr = self.vec.ptr_len().0;
        // SAFETY: vec is owned (drain asserted not Referenced); current < drain_end
        // and the drain range lies within the original elements, so ptr.add(current)
        // is initialized. read moves it out and current advances so it is never
        // read again; Drain::drop will not re-touch [drain_start, drain_end).
        let val = unsafe { ptr.add(self.current).read() };
        self.current += 1;
        Some(val)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.drain_end - self.current;
        (len, Some(len))
    }
}

impl<T, const N: usize> ExactSizeIterator for Drain<'_, T, N> {}

impl<T, const N: usize> Drop for Drain<'_, T, N> {
    fn drop(&mut self) {
        // Drop un-yielded elements in the drain range.
        if self.current < self.drain_end {
            let ptr = self.vec.mut_data_ptr();
            // SAFETY: vec is owned; [current, drain_end) holds the drained elements
            // not yet yielded, all still initialized, dropped exactly once here.
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                    ptr.add(self.current),
                    self.drain_end - self.current,
                ));
            }
        }

        // Shift tail to fill gap.
        let tail_len = self.original_len - self.drain_end;
        if tail_len > 0 {
            let ptr = self.vec.mut_data_ptr();
            // SAFETY: vec is owned; the tail [drain_end, original_len) is still
            // initialized, and copying it down to drain_start fills the drained gap
            // (the source elements at the now-vacated high slots become dead once
            // tagged_len is set to new_len below). ptr::copy handles overlap.
            unsafe {
                ptr::copy(ptr.add(self.drain_end), ptr.add(self.drain_start), tail_len);
            }
        }

        let new_len = self.drain_start + tail_len;
        let tag = self.vec.tag();
        self.vec.tagged_len = SmallVec::<T, N>::encode(tag, new_len);
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for Drain<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr = self.vec.ptr_len().0;
        // SAFETY: vec is owned; [current, drain_end) are the not-yet-yielded
        // drained elements, all still initialized, borrowed only for formatting.
        let remaining = unsafe {
            slice::from_raw_parts(ptr.add(self.current), self.drain_end - self.current)
        };
        f.debug_tuple("Drain").field(&remaining).finish()
    }
}

// ── alloc import ────────────────────────────────────────────────────────

extern crate alloc;
