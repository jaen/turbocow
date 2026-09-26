use crate::allocator::AllocatorProvider;
use crate::dynamic::common::limit::LEN_VALUE_MASK_USIZE;
use crate::dynamic::types::{DynamicVec, Variant, VariantMut};
use crate::vec::types::EcoVec;

#[cold]
#[track_caller]
fn out_of_bounds(index: usize, len: usize) -> ! {
    // Same message as `EcoVec`'s bounds check.
    panic!("index is out bounds (index: {index}, len: {len})");
}

#[cold]
#[track_caller]
fn capacity_overflow() -> ! {
    panic!("capacity overflow");
}

impl<'a, const N: usize, A> DynamicVec<'a, N, A>
where
    A: AllocatorProvider,
{
    #[inline]
    pub fn len(&self) -> usize {
        if self.is_pure_inline() {
            unsafe { (*self.0.inline).len() }
        } else {
            // Referenced and Spilled are both repr(C) with `len: usize`
            // at offset size_of::<usize>().  The mask strips Referenced
            // tag bits and is a no-op for Spilled (len ≤ isize::MAX).
            unsafe { (*self.0.referenced).len & LEN_VALUE_MASK_USIZE }
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        if self.is_pure_inline() {
            unsafe { (*self.0.inline).as_slice() }
        } else {
            // Referenced and Spilled are both repr(C) with ptr at offset 0
            // and len at offset size_of::<usize>().  Reading through the
            // Referenced arm is sound for Spilled because the fields have
            // the same type and offset.  The mask strips Referenced tag
            // bits and is a no-op for Spilled (len ≤ isize::MAX).
            let r = unsafe { &*self.0.referenced };
            let len = r.len & LEN_VALUE_MASK_USIZE;
            unsafe { core::slice::from_raw_parts(r.ptr, len) }
        }
    }

    #[inline]
    pub fn make_mut(&mut self, size_hint: usize) -> &mut [u8]
    where
        A: Clone,
    {
        // Materialise Referenced inline — reads data directly from the
        // union member (one tag check, no separate helper function).
        // Borrows &A instead of cloning upfront; from_slice_in_owned
        // defers the clone until the inline/spill decision is made.
        if self.is_referenced() {
            let r = unsafe { &*self.0.referenced };
            let len = r.len & LEN_VALUE_MASK_USIZE;
            let ptr = r.ptr;
            // Safety: from_referenced guarantees ptr/len are valid.
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            *self = Self::from_slice_in_owned(slice, &r.allocator, size_hint);
        }
        match self.variant_mut() {
            VariantMut::Inline(inline) => inline.as_mut_slice(),
            VariantMut::Spilled(spilled) => {
                // Ensure uniqueness via the u8-specialised memcpy path
                // instead of the generic element-by-element clone.
                // The subsequent make_mut() sees is_unique()=true and
                // returns the mutable slice without re-cloning.
                spilled.make_unique_bytes();
                spilled.make_mut()
            }
            VariantMut::Referenced(_) => unreachable!(),
        }
    }

    #[inline]
    pub fn push(&mut self, byte: u8)
    where
        A: Clone,
    {
        if self.is_referenced() {
            self.make_mut(1);
            // Fall through — now Inline or Spilled.
        }
        match self.variant_mut() {
            VariantMut::Inline(inline) => {
                if inline.push(byte).is_err() {
                    let alloc = inline.allocator.clone();
                    // NLL: inline borrow is dead after clone.
                    self.spill_push(byte, alloc);
                }
            }
            VariantMut::Spilled(spilled) => {
                spilled.push(byte);
            }
            // make_mut() above guarantees the variant is no longer Referenced.
            VariantMut::Referenced(_) => unreachable!(),
        }
    }

    #[inline]
    pub fn extend_from_slice(&mut self, bytes: &[u8])
    where
        A: Clone,
    {
        if self.is_referenced() {
            self.make_mut(bytes.len());
            // Fall through — now Inline or Spilled.
        }
        match self.variant_mut() {
            VariantMut::Inline(inline) => {
                if inline.extend_from_slice(bytes).is_err() {
                    let needed = inline.len() + bytes.len();
                    let alloc = inline.allocator.clone();
                    // NLL: inline borrow is dead after clone.
                    self.spill_extend(bytes, needed, alloc);
                }
            }
            VariantMut::Spilled(spilled) => {
                spilled.extend_from_byte_slice(bytes);
            }
            // make_mut() above guarantees the variant is no longer Referenced.
            VariantMut::Referenced(_) => unreachable!(),
        }
    }

    #[inline]
    pub fn clear(&mut self)
    where
        A: Clone,
    {
        if self.is_referenced() {
            // Skip materialisation — copying the borrowed data just to
            // discard it is wasteful.  Replace with a fresh empty inline.
            let alloc = unsafe { (*self.0.referenced).allocator.clone() };
            *self = Self::new_in(alloc);
            return;
        }
        match self.variant_mut() {
            VariantMut::Inline(inline) => inline.clear(),
            VariantMut::Spilled(spilled) => spilled.clear(),
            VariantMut::Referenced(_) => unreachable!(),
        }
    }

    #[inline]
    pub fn truncate(&mut self, target: usize)
    where
        A: Clone,
    {
        match self.variant_mut() {
            VariantMut::Referenced(r) => {
                let cur_len = r.len & LEN_VALUE_MASK_USIZE;
                if target < cur_len {
                    use crate::dynamic::common::limit::LEN_TAGS_USIZE;
                    r.len = target | LEN_TAGS_USIZE;
                }
            }
            VariantMut::Inline(inline) => inline.truncate(target),
            VariantMut::Spilled(spilled) => spilled.truncate(target),
        }
    }

    /// How many bytes can be held without (re-)allocating.
    ///
    /// Inline storage holds `N` bytes; a spilled vector reports its
    /// `EcoVec`'s capacity (mutating it can still allocate if the allocation
    /// is shared). A Referenced vector owns no storage, so this is just its
    /// length — any mutation first copies the borrowed bytes.
    #[inline]
    pub fn capacity(&self) -> usize {
        match self.variant() {
            Variant::Referenced(r) => r.len & LEN_VALUE_MASK_USIZE,
            Variant::Inline(_) => N,
            Variant::Spilled(spilled) => spilled.capacity(),
        }
    }

    /// Inserts `bytes` at `index`, shifting everything after it to the right.
    ///
    /// Panics if `index > len`.
    #[inline]
    #[track_caller]
    pub fn insert_slice(&mut self, index: usize, bytes: &[u8])
    where
        A: Clone,
    {
        let len = self.len();
        if index > len {
            out_of_bounds(index, len);
        }
        if bytes.is_empty() {
            return;
        }
        if self.is_referenced() {
            self.make_mut(bytes.len());
            // Fall through — now Inline or Spilled.
        }
        let spilled = match self.variant_mut() {
            VariantMut::Inline(inline) => {
                if inline.insert_slice(index, bytes).is_ok() {
                    return;
                }
                let needed = inline.len() + bytes.len();
                let mut eco = EcoVec::with_capacity_in(
                    needed.next_power_of_two(),
                    inline.allocator.clone(),
                );
                let (head, tail) = inline.as_slice().split_at(index);
                // Safety: capacity ≥ needed = head + bytes + tail, and the
                // fresh vec is unique (refcount = 1).
                unsafe {
                    eco.extend_from_byte_slice_unchecked(head);
                    eco.extend_from_byte_slice_unchecked(bytes);
                    eco.extend_from_byte_slice_unchecked(tail);
                }
                eco
            }
            VariantMut::Spilled(spilled) => {
                spilled.splice(index..index, bytes.iter().copied());
                return;
            }
            // make_mut() above guarantees the variant is no longer Referenced.
            VariantMut::Referenced(_) => unreachable!(),
        };
        // Assigning (rather than emplacing) drops the old Inline variant,
        // including a non-ZST allocator.
        *self = Self::from_eco(spilled);
    }

    /// Removes and returns the byte at `index`, shifting everything after it
    /// to the left.
    ///
    /// Panics if `index >= len`.
    #[inline]
    #[track_caller]
    pub fn remove(&mut self, index: usize) -> u8
    where
        A: Clone,
    {
        let len = self.len();
        if index >= len {
            out_of_bounds(index, len);
        }
        if self.is_referenced() {
            let byte = self.as_slice()[index];
            if index + 1 == len {
                // Dropping the last byte only shortens the borrowed view.
                self.truncate(index);
                return byte;
            }
            self.make_mut(0);
        }
        match self.variant_mut() {
            VariantMut::Inline(inline) => inline.remove(index),
            VariantMut::Spilled(spilled) => {
                // Unshare via the u8 memcpy path before the generic remove.
                spilled.make_unique_bytes();
                spilled.remove(index)
            }
            // make_mut() above guarantees the variant is no longer Referenced.
            VariantMut::Referenced(_) => unreachable!(),
        }
    }

    /// Reserves space for at least `additional` more bytes.
    ///
    /// Afterwards the vector owns its storage (a Referenced vector is copied,
    /// a shared spilled vector is unshared) and `capacity() - len()` is at
    /// least `additional`.
    #[inline]
    pub fn reserve(&mut self, additional: usize)
    where
        A: Clone,
    {
        if self.is_referenced() {
            // Materialises with room for `additional` more bytes.
            self.make_mut(additional);
        }
        let spilled = match self.variant_mut() {
            VariantMut::Inline(inline) => {
                let len = inline.len();
                if additional <= N - len {
                    return;
                }
                let capacity = len
                    .checked_add(additional)
                    .unwrap_or_else(|| capacity_overflow())
                    .max(2 * N);
                let mut eco =
                    EcoVec::with_capacity_in(capacity, inline.allocator.clone());
                // Safety: capacity ≥ len and the fresh vec is unique.
                unsafe { eco.extend_from_byte_slice_unchecked(inline.as_slice()) };
                eco
            }
            VariantMut::Spilled(spilled) => {
                spilled.reserve(additional);
                return;
            }
            // make_mut() above guarantees the variant is no longer Referenced.
            VariantMut::Referenced(_) => unreachable!(),
        };
        *self = Self::from_eco(spilled);
    }

    /// Converts into an `EcoVec<u8, A>`.
    ///
    /// A spilled vector hands over its allocation without copying; inline and
    /// Referenced data is copied into a new allocation.
    #[inline]
    pub fn into_eco(self) -> EcoVec<u8, A>
    where
        A: Clone,
    {
        if !self.is_inline() {
            let mut this = core::mem::ManuallyDrop::new(self);
            // Safety: `!is_inline()` means the Spilled variant is active, and
            // `this` is never dropped, so the EcoVec is moved out exactly once.
            return unsafe { core::mem::ManuallyDrop::take(&mut this.0.spilled) };
        }
        let bytes = self.as_slice();
        let mut eco = EcoVec::with_capacity_in(bytes.len(), self.get_allocator().clone());
        // Safety: capacity ≥ bytes.len() and the fresh vec is unique.
        unsafe { eco.extend_from_byte_slice_unchecked(bytes) };
        eco
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn get_allocator(&self) -> &A
    where
        A: Clone,
    {
        // ZST allocators (e.g. Global) have no data — skip the dispatch.
        if core::mem::size_of::<A>() == 0 {
            unsafe { &*core::ptr::NonNull::<A>::dangling().as_ptr() }
        } else {
            match self.variant() {
                Variant::Referenced(r) => &r.allocator,
                Variant::Inline(inline) => inline.get_allocator(),
                Variant::Spilled(spilled) => spilled.get_allocator(),
            }
        }
    }

    /// Spill from inline to heap and push one byte.
    /// Allocator is passed directly from the match arm — no re-dispatch.
    #[cold]
    #[inline(never)]
    fn spill_push(&mut self, byte: u8, alloc: A)
    where
        A: Clone,
    {
        let mut eco = EcoVec::with_capacity_in(N * 2, alloc);
        // Safety: with_capacity_in(N * 2) guarantees capacity ≥ N + 1.
        // The current inline data is at most N bytes, plus 1 byte to push.
        // The fresh vec is unique (refcount = 1).
        // Read directly from the inline union member — this function is only
        // called from the Inline match arm, so skip the as_slice() dispatch.
        unsafe {
            eco.extend_from_byte_slice_unchecked((*self.0.inline).as_slice());
            eco.push_unchecked(byte);
            // Write EcoVec directly into *self — skip from_eco intermediate.
            self.emplace_eco(eco);
        }
    }

    /// Spill from inline to heap and extend with a byte slice.
    /// Allocator and needed capacity passed directly — no re-dispatch.
    #[cold]
    #[inline(never)]
    fn spill_extend(&mut self, bytes: &[u8], needed: usize, alloc: A)
    where
        A: Clone,
    {
        let mut eco = EcoVec::with_capacity_in(needed.next_power_of_two(), alloc);
        // Safety: capacity ≥ needed = self.len() + bytes.len().
        // The fresh vec is unique (refcount = 1).
        // Read directly from the inline union member — this function is only
        // called from the Inline match arm, so skip the as_slice() dispatch.
        unsafe {
            eco.extend_from_byte_slice_unchecked((*self.0.inline).as_slice());
            eco.extend_from_byte_slice_unchecked(bytes);
            // Write EcoVec directly into *self — skip from_eco intermediate.
            self.emplace_eco(eco);
        }
    }

    /// Write an EcoVec directly into `*self`, transitioning to the Spilled
    /// variant without an intermediate `MaybeUninit` or `from_eco` call.
    ///
    /// # Safety
    /// - The current variant must be Inline or Referenced (Drop for these
    ///   is a no-op — no heap resources to clean up).
    /// - The caller must have already copied any needed data from *self.
    #[inline(always)]
    unsafe fn emplace_eco(&mut self, eco: EcoVec<u8, A>) {
        use crate::dynamic::common::limit::TAG_OVERLAPS_LEN;
        use core::mem::ManuallyDrop;
        use core::ptr::addr_of_mut;

        let self_ptr = self as *mut Self;
        // On 64-bit LE, the high byte of EcoVec::len is at the tagged_len
        // position and is guaranteed 0 (len ≤ isize::MAX).  On other
        // layouts we must explicitly clear the tag byte.
        if !TAG_OVERLAPS_LEN {
            // Safety:
            // The tag byte lives at offset `N` within the inline union member,
            // which is in bounds of `*self`. Writing it does not touch any
            // initialized field the caller still needs (caller copied data out).
            unsafe {
                let tag_ptr = (addr_of_mut!((*self_ptr).0.inline) as *mut u8).add(N);
                tag_ptr.write(0);
            }
        }
        // Safety:
        // The current variant is Inline/Referenced (Drop is a no-op for these),
        // so overwriting the union with the Spilled member leaks nothing, and
        // `spilled_ptr` points to the correctly-aligned, in-bounds union slot.
        unsafe {
            let spilled_ptr = addr_of_mut!((*self_ptr).0.spilled);
            spilled_ptr.write(ManuallyDrop::new(eco));
        }
    }
}
