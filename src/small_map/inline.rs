use core::borrow::Borrow;
use core::mem::MaybeUninit;

use super::group;
use super::types::InlineMap;

impl<K, V, const N: usize, S> InlineMap<K, V, N, S> {
    /// Creates a new empty inline map with the given hasher.
    #[inline]
    pub(super) fn new(hasher: S) -> Self {
        // N must be non-zero (an inline map needs at least one slot). There is
        // no upper bound: `match_h2` scans the h2 sidecar in 16-byte chunks
        // (group.rs), so any N is supported — large N just trades stack space +
        // linear-scan cost for inline storage, the same as `SmallVec`.
        const {
            assert!(N > 0, "SmallMap inline capacity N must be non-zero");
        };
        Self {
            h2_bytes: [group::H2_EMPTY; N],
            // SAFETY: MaybeUninit doesn't require initialization.
            entries: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
            hasher,
        }
    }

    #[inline]
    pub(super) fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub(super) fn is_full(&self) -> bool {
        self.len == N
    }

    /// Returns a shared reference to the hasher.
    #[inline]
    pub(super) fn hasher(&self) -> &S {
        &self.hasher
    }

    /// Returns a slice of the initialized key-value pairs.
    ///
    /// # Safety
    /// `data[0..self.len]` must be initialized.
    #[cfg_attr(not(feature = "rkyv"), allow(dead_code))]
    #[inline]
    pub(super) unsafe fn as_slice(&self) -> &[(K, V)] {
        unsafe {
            // SAFETY: data[0..len] is always initialized by our invariants.
            core::slice::from_raw_parts(self.entries.as_ptr() as *const (K, V), self.len)
        }
    }

    /// Erases the element at `index` using swap-delete.
    /// Copies the last element to `index` to maintain contiguous storage.
    ///
    /// # Safety
    /// - `index < self.len`
    /// - The value at `index` must have been moved out or will be overwritten.
    #[inline]
    pub(super) unsafe fn erase(&mut self, index: usize) {
        unsafe {
            debug_assert!(index < self.len);
            let last = self.len - 1;
            if index != last {
                self.h2_bytes[index] = self.h2_bytes[last];
                core::ptr::copy_nonoverlapping(
                    self.entries[last].as_ptr(),
                    self.entries[index].as_mut_ptr(),
                    1,
                );
            }
            self.len -= 1;
        }
    }
}

impl<K, V, const N: usize, S> InlineMap<K, V, N, S>
where
    K: Eq,
{
    /// Linear-scan lookup by key. Returns a reference to the value.
    #[inline]
    pub(super) fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: core::borrow::Borrow<Q>,
        Q: Eq + ?Sized,
    {
        for i in 0..self.len {
            let (k, v) = unsafe { self.entries[i].assume_init_ref() };
            if key == k.borrow() {
                return Some(v);
            }
        }
        None
    }

    /// Linear-scan lookup by key. Returns a mutable reference to the value.
    #[inline]
    pub(super) fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: core::borrow::Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let base = self.entries.as_mut_ptr();
        for i in 0..self.len {
            // Use raw pointer to avoid reborrowing the array across iterations.
            let entry = unsafe { &mut *base.add(i).cast::<(K, V)>() };
            if key == entry.0.borrow() {
                return Some(&mut entry.1);
            }
        }
        None
    }

    /// Linear-scan lookup returning key-value pair reference.
    #[inline]
    pub(super) fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: core::borrow::Borrow<Q>,
        Q: Eq + ?Sized,
    {
        for i in 0..self.len {
            let (k, v) = unsafe { self.entries[i].assume_init_ref() };
            if key == k.borrow() {
                return Some((k, v));
            }
        }
        None
    }

    /// h2-filtered lookup by key. Returns a reference to the value.
    #[inline]
    pub(super) fn get_h2<Q>(&self, key: &Q, h2: u8) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let mask = unsafe { group::match_h2::<N>(self.h2_bytes.as_ptr(), self.len, h2) };
        for i in mask {
            let (k, v) = unsafe { self.entries[i].assume_init_ref() };
            if key == k.borrow() {
                return Some(v);
            }
        }
        None
    }

    /// h2-filtered lookup by key. Returns a mutable reference to the value.
    #[inline]
    pub(super) fn get_mut_h2<Q>(&mut self, key: &Q, h2: u8) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let base = self.entries.as_mut_ptr();
        let mask = unsafe { group::match_h2::<N>(self.h2_bytes.as_ptr(), self.len, h2) };
        for i in mask {
            let entry = unsafe { &mut *base.add(i).cast::<(K, V)>() };
            if key == entry.0.borrow() {
                return Some(&mut entry.1);
            }
        }
        None
    }

    /// h2-filtered lookup returning key-value pair reference.
    #[inline]
    pub(super) fn get_key_value_h2<Q>(&self, key: &Q, h2: u8) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let mask = unsafe { group::match_h2::<N>(self.h2_bytes.as_ptr(), self.len, h2) };
        for i in mask {
            let (k, v) = unsafe { self.entries[i].assume_init_ref() };
            if key == k.borrow() {
                return Some((k, v));
            }
        }
        None
    }

    /// Remove a key, returning the key-value pair if it existed.
    #[inline]
    pub(super) fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        for i in 0..self.len {
            let k = unsafe { &self.entries[i].assume_init_ref().0 };
            if key == k.borrow() {
                let entry = unsafe { self.entries[i].as_ptr().read() };
                unsafe { self.erase(i) };
                return Some(entry);
            }
        }
        None
    }

    /// h2-filtered remove, returning the key-value pair if found.
    #[inline]
    pub(super) fn remove_entry_h2<Q>(&mut self, key: &Q, h2: u8) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Eq + ?Sized,
    {
        let mask = unsafe { group::match_h2::<N>(self.h2_bytes.as_ptr(), self.len, h2) };
        for i in mask {
            let k = unsafe { &self.entries[i].assume_init_ref().0 };
            if key == k.borrow() {
                let entry = unsafe { self.entries[i].as_ptr().read() };
                unsafe { self.erase(i) };
                return Some(entry);
            }
        }
        None
    }

    /// Clear all entries.
    #[inline]
    pub(super) fn clear(&mut self) {
        if core::mem::needs_drop::<(K, V)>() {
            for i in 0..self.len {
                unsafe { core::ptr::drop_in_place(self.entries[i].as_mut_ptr()) };
            }
        }
        self.len = 0;
    }

    /// Retains only the elements specified by the predicate.
    #[inline]
    pub(super) fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        let mut i = 0;
        while i < self.len {
            let (k, v) = unsafe { self.entries[i].assume_init_mut() };
            if f(k, v) {
                i += 1;
            } else {
                unsafe {
                    core::ptr::drop_in_place(self.entries[i].as_mut_ptr());
                    self.erase(i);
                }
                // Don't increment — check the swapped-in element.
            }
        }
    }
}

impl<K: Clone, V: Clone, const N: usize, S: Clone> Clone for InlineMap<K, V, N, S> {
    #[inline]
    fn clone(&self) -> Self {
        // Panic-safe clone using a drop guard.
        struct DropGuard<'a, K, V> {
            data: &'a mut [MaybeUninit<(K, V)>],
            len: usize,
        }

        impl<K, V> Drop for DropGuard<'_, K, V> {
            fn drop(&mut self) {
                for i in 0..self.len {
                    unsafe { core::ptr::drop_in_place(self.data[i].as_mut_ptr()) };
                }
            }
        }

        // SAFETY: MaybeUninit doesn't require initialization.
        let mut entries: [MaybeUninit<(K, V)>; N] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let mut guard = DropGuard { data: &mut entries, len: 0 };

        for i in 0..self.len {
            let src = unsafe { self.entries[i].assume_init_ref() };
            guard.data[i] = MaybeUninit::new(src.clone());
            guard.len += 1;
        }

        core::mem::forget(guard);

        Self {
            entries,
            h2_bytes: self.h2_bytes,
            len: self.len,
            hasher: self.hasher.clone(),
        }
    }
}

impl<K, V, const N: usize, S> Drop for InlineMap<K, V, N, S> {
    #[inline]
    fn drop(&mut self) {
        if core::mem::needs_drop::<(K, V)>() {
            for i in 0..self.len {
                unsafe { core::ptr::drop_in_place(self.entries[i].as_mut_ptr()) };
            }
        }
    }
}
