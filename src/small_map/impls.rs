use core::borrow::Borrow;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::iter::FusedIterator;
use core::mem::MaybeUninit;
use core::ops::Index;

use hashbrown::HashMap;

use super::group;
use super::types::{InlineMap, Repr, SmallMap};

/// Below this entry count, lookups use linear scan (no hashing overhead).
const LINEAR_THRESHOLD: usize = 3;

// ═══════════════════════════════════════════════════════════════════════════
// Constructors
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S: Default> SmallMap<K, V, N, S> {
    /// Creates an empty `SmallMap`.
    ///
    /// The map starts with inline storage and will not allocate until
    /// more than `N` entries are inserted.
    #[inline]
    pub fn new() -> Self {
        SmallMap(Repr::Inline(InlineMap::new(S::default())))
    }
}

impl<K, V, const N: usize, S> SmallMap<K, V, N, S> {
    /// Creates an empty `SmallMap` with the given hasher.
    #[inline]
    pub fn with_hasher(hasher: S) -> Self {
        SmallMap(Repr::Inline(InlineMap::new(hasher)))
    }

    /// Creates an empty `SmallMap` that will immediately use heap storage
    /// with at least the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self
    where
        S: Default,
    {
        if capacity <= N {
            Self::new()
        } else {
            SmallMap(Repr::Heap(HashMap::with_capacity_and_hasher(
                capacity,
                S::default(),
            )))
        }
    }

    /// Returns the number of entries in the map.
    #[inline]
    pub fn len(&self) -> usize {
        match &self.0 {
            Repr::Inline(inline) => inline.len(),
            Repr::Heap(heap) => heap.len(),
        }
    }

    /// Returns `true` if the map contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match &self.0 {
            Repr::Inline(inline) => inline.is_empty(),
            Repr::Heap(heap) => heap.is_empty(),
        }
    }

    /// Returns `true` if the map is currently using inline storage.
    #[inline]
    pub fn is_inline(&self) -> bool {
        matches!(self.0, Repr::Inline(_))
    }

    /// Returns a reference to the map's hasher.
    #[inline]
    pub fn hasher(&self) -> &S {
        match &self.0 {
            Repr::Inline(inline) => inline.hasher(),
            Repr::Heap(heap) => heap.hasher(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Core operations
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S> SmallMap<K, V, N, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Returns a reference to the value corresponding to the key.
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match &self.0 {
            Repr::Inline(inline) => {
                if inline.len() <= LINEAR_THRESHOLD {
                    inline.get(key)
                } else {
                    let hash = group::make_hash(inline.hasher(), key);
                    inline.get_h2(key, group::h2(hash))
                }
            }
            Repr::Heap(heap) => heap.get(key),
        }
    }

    /// Returns a mutable reference to the value corresponding to the key.
    #[inline]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match &mut self.0 {
            Repr::Inline(inline) => {
                if inline.len() <= LINEAR_THRESHOLD {
                    inline.get_mut(key)
                } else {
                    let hash = group::make_hash(inline.hasher(), key);
                    inline.get_mut_h2(key, group::h2(hash))
                }
            }
            Repr::Heap(heap) => heap.get_mut(key),
        }
    }

    /// Returns the key-value pair corresponding to the key.
    #[inline]
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match &self.0 {
            Repr::Inline(inline) => {
                if inline.len() <= LINEAR_THRESHOLD {
                    inline.get_key_value(key)
                } else {
                    let hash = group::make_hash(inline.hasher(), key);
                    inline.get_key_value_h2(key, group::h2(hash))
                }
            }
            Repr::Heap(heap) => heap.get_key_value(key),
        }
    }

    /// Returns `true` if the map contains the given key.
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the key already exists, the value is replaced and the old value
    /// is returned. If the inline storage is full and the key is new,
    /// the map spills to a heap-allocated `HashMap`.
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match &mut self.0 {
            Repr::Inline(inline) => {
                // Always hash on insert — we need the h2 for the new entry.
                let hash = group::make_hash(inline.hasher(), &key);
                let h2 = group::h2(hash);

                // h2-filtered scan for existing key.
                let base = inline.entries.as_mut_ptr();
                let mask = unsafe {
                    group::match_h2::<N>(inline.h2_bytes.as_ptr(), inline.len, h2)
                };
                for i in mask {
                    let entry = unsafe { &mut *base.add(i).cast::<(K, V)>() };
                    if entry.0 == key {
                        return Some(core::mem::replace(&mut entry.1, value));
                    }
                }

                // New key — if there's room, append inline.
                if !inline.is_full() {
                    inline.h2_bytes[inline.len] = h2;
                    inline.entries[inline.len] = MaybeUninit::new((key, value));
                    inline.len += 1;
                    None
                } else {
                    // Spill to heap.
                    self.spill_and_insert(key, value)
                }
            }
            Repr::Heap(heap) => heap.insert(key, value),
        }
    }

    /// Removes a key from the map, returning the value if it existed.
    #[inline]
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.remove_entry(key).map(|(_, v)| v)
    }

    /// Removes a key from the map, returning the key-value pair if it existed.
    #[inline]
    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match &mut self.0 {
            Repr::Inline(inline) => {
                if inline.len() <= LINEAR_THRESHOLD {
                    inline.remove_entry(key)
                } else {
                    let hash = group::make_hash(inline.hasher(), key);
                    inline.remove_entry_h2(key, group::h2(hash))
                }
            }
            Repr::Heap(heap) => heap.remove_entry(key),
        }
    }

    /// Clears the map, removing all key-value pairs.
    ///
    /// If the map was heap-allocated, it remains heap-allocated but empty.
    #[inline]
    pub fn clear(&mut self) {
        match &mut self.0 {
            Repr::Inline(inline) => inline.clear(),
            Repr::Heap(heap) => heap.clear(),
        }
    }

    /// Retains only the elements specified by the predicate.
    #[inline]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        match &mut self.0 {
            Repr::Inline(inline) => inline.retain(f),
            Repr::Heap(heap) => heap.retain(f),
        }
    }

    /// Gets the given key's corresponding entry in the map for in-place
    /// manipulation.
    #[inline]
    pub fn entry(&mut self, key: K) -> super::entry::Entry<'_, K, V, N, S> {
        use super::entry::*;

        // Phase 1: search (borrows self temporarily).
        let probe = match &mut self.0 {
            Repr::Inline(inline) => {
                let hash = group::make_hash(inline.hasher(), &key);
                let h2 = group::h2(hash);
                let mask = unsafe {
                    group::match_h2::<N>(inline.h2_bytes.as_ptr(), inline.len, h2)
                };
                let mut found = None;
                for i in mask {
                    let k = unsafe { &inline.entries[i].assume_init_ref().0 };
                    if *k == key {
                        found = Some(i);
                        break;
                    }
                }
                Some((found, h2))
            }
            Repr::Heap(_) => None,
        };

        // Phase 2: construct entry (borrows self for 'a).
        match probe {
            Some((Some(index), _)) => Entry::Occupied(OccupiedEntry {
                inner: OccupiedInner::Inline { map: self, index },
            }),
            Some((None, h2)) => Entry::Vacant(VacantEntry {
                inner: VacantInner::Inline { map: self, key, h2 },
            }),
            None => match &mut self.0 {
                Repr::Heap(heap) => match heap.entry(key) {
                    hashbrown::hash_map::Entry::Occupied(o) => {
                        Entry::Occupied(OccupiedEntry { inner: OccupiedInner::Heap(o) })
                    }
                    hashbrown::hash_map::Entry::Vacant(v) => {
                        Entry::Vacant(VacantEntry { inner: VacantInner::Heap(v) })
                    }
                },
                _ => unreachable!(),
            },
        }
    }

    /// Spill from inline to heap and insert a new key-value pair.
    /// Called when inline storage is full and a new key must be added.
    #[cold]
    #[inline(never)]
    fn spill_and_insert(&mut self, key: K, value: V) -> Option<V> {
        self.force_spill();
        match &mut self.0 {
            Repr::Heap(heap) => heap.insert(key, value),
            _ => unreachable!(),
        }
    }

    /// Move all inline entries to a heap HashMap. No-op if already heap.
    ///
    /// # Panic safety
    ///
    /// The hasher and entries are bitwise-moved out of the inline storage, and
    /// the moved-out inline shell is installed-as-`Heap`-and-forgotten *before*
    /// the fallible inserts run. So if `K::hash`/`K::eq` panics during an
    /// insert: the hasher (now owned by `heap`, which lives in `*self`) is
    /// dropped exactly once; already-inserted entries are dropped by `heap`;
    /// the entry being inserted is dropped by the failed `insert`; and a guard
    /// drops the not-yet-inserted entries. Reading the hasher out before the
    /// loop and only forgetting the shell *after* it (the previous approach)
    /// double-dropped a `Drop`-bearing `S` on this path.
    #[cold]
    #[inline(never)]
    pub(super) fn force_spill(&mut self) {
        let inline = match &mut self.0 {
            Repr::Inline(inline) => inline,
            Repr::Heap(_) => return,
        };
        let len = inline.len;

        // Bitwise-move the hasher and entries out of the inline storage. The
        // shell now owns nothing, so we install the (empty) heap into `*self`
        // and forget the shell — crucially, so its field-drop glue never drops
        // the hasher a second time (the heap owns it now). This happens BEFORE
        // the fallible inserts, so a panicking insert can never touch the shell.
        let hasher = unsafe { core::ptr::read(&inline.hasher) };
        let entries = unsafe { core::ptr::read(&inline.entries) };
        // (the `&mut *self` borrow held by `inline` ends at its last use above)

        let heap = HashMap::with_capacity_and_hasher(N * 2, hasher);
        let old = core::mem::replace(&mut self.0, Repr::Heap(heap));
        core::mem::forget(old);
        let Repr::Heap(heap) = &mut self.0 else {
            unreachable!("just replaced *self with Heap")
        };

        // Drops the entries not yet moved into `heap` if an insert unwinds.
        struct EntryGuard<K, V, const M: usize> {
            entries: [MaybeUninit<(K, V)>; M],
            start: usize,
            len: usize,
        }
        impl<K, V, const M: usize> Drop for EntryGuard<K, V, M> {
            fn drop(&mut self) {
                let (start, len) = (self.start, self.len);
                for slot in &mut self.entries[start..len] {
                    // SAFETY: entries[start..len] are still initialized.
                    unsafe { slot.as_mut_ptr().drop_in_place() };
                }
            }
        }

        let mut guard = EntryGuard::<K, V, N> { entries, start: 0, len };
        while guard.start < guard.len {
            let i = guard.start;
            // SAFETY: entries[i] is initialized. Mark it consumed BEFORE the
            // insert so the guard won't also drop it if `insert` panics.
            let (k, v) = unsafe { guard.entries[i].as_ptr().read() };
            guard.start = i + 1;
            heap.insert(k, v);
        }
        core::mem::forget(guard);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Capacity
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S> SmallMap<K, V, N, S> {
    /// Returns the number of entries the map can hold without reallocation.
    ///
    /// When using inline storage, this is always `N` (the fixed stack
    /// capacity). When using heap storage, this is the hashbrown `HashMap`
    /// capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        match &self.0 {
            Repr::Inline(_) => N,
            Repr::Heap(heap) => heap.capacity(),
        }
    }
}

impl<K, V, const N: usize, S> SmallMap<K, V, N, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    /// Reserves capacity for at least `additional` more entries.
    ///
    /// If the map is inline and `len + additional > N`, the map is spilled
    /// to heap storage first. Otherwise the heap's `reserve` is called.
    /// Inline maps with sufficient remaining inline capacity need no action.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let needs_spill =
            matches!(&self.0, Repr::Inline(inline) if inline.len() + additional > N);
        if needs_spill {
            self.force_spill();
        }
        if let Repr::Heap(heap) = &mut self.0 {
            heap.reserve(additional);
        }
        // If inline and len + additional <= N, the inline array already
        // has sufficient capacity — no action needed.
    }

    /// Shrinks the capacity as close to the length as possible.
    ///
    /// For inline storage, this is a no-op — the inline array has a fixed
    /// size of `N` and cannot be resized.
    ///
    /// For heap storage, delegates to hashbrown's `shrink_to_fit`, which
    /// reduces bucket count toward the minimum needed to hold the current
    /// entries. Hashbrown may retain slightly more capacity than `len` for
    /// its load-factor invariant.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        if let Repr::Heap(heap) = &mut self.0 {
            heap.shrink_to_fit();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Iterators
// ═══════════════════════════════════════════════════════════════════════════

/// An iterator over the entries of an [`SmallMap`].
pub enum Iter<'a, K, V> {
    /// Iterating over inline storage.
    Inline {
        /// Pointer to the data.
        ptr: *const (K, V),
        /// Current index.
        index: usize,
        /// Number of initialized elements.
        len: usize,
        /// Lifetime marker.
        _marker: core::marker::PhantomData<&'a (K, V)>,
    },
    /// Iterating over heap storage.
    Heap(hashbrown::hash_map::Iter<'a, K, V>),
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Inline { ptr, index, len, .. } => {
                if *index < *len {
                    let kv = unsafe { &*ptr.add(*index) };
                    *index += 1;
                    Some((&kv.0, &kv.1))
                } else {
                    None
                }
            }
            Iter::Heap(iter) => iter.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            Iter::Inline { index, len, .. } => len - index,
            Iter::Heap(iter) => iter.len(),
        }
    }
}

impl<K, V> FusedIterator for Iter<'_, K, V> {}

/// A mutable iterator over the entries of an [`SmallMap`].
pub enum IterMut<'a, K, V> {
    /// Iterating over inline storage.
    Inline {
        /// Pointer to the data.
        ptr: *mut (K, V),
        /// Current index.
        index: usize,
        /// Number of initialized elements.
        len: usize,
        /// Lifetime marker.
        _marker: core::marker::PhantomData<&'a mut (K, V)>,
    },
    /// Iterating over heap storage.
    Heap(hashbrown::hash_map::IterMut<'a, K, V>),
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IterMut::Inline { ptr, index, len, .. } => {
                if *index < *len {
                    let kv = unsafe { &mut *ptr.add(*index) };
                    *index += 1;
                    Some((&kv.0, &mut kv.1))
                } else {
                    None
                }
            }
            IterMut::Heap(iter) => iter.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            IterMut::Inline { index, len, .. } => len - index,
            IterMut::Heap(iter) => iter.len(),
        }
    }
}

impl<K, V> FusedIterator for IterMut<'_, K, V> {}

/// An owning iterator over the entries of an [`SmallMap`].
pub enum IntoIter<K, V, const N: usize> {
    /// Consuming inline storage.
    Inline {
        /// The inline data array.
        data: [MaybeUninit<(K, V)>; N],
        /// Current index.
        index: usize,
        /// Number of initialized elements.
        len: usize,
    },
    /// Consuming heap storage.
    Heap(hashbrown::hash_map::IntoIter<K, V>),
}

impl<K, V, const N: usize> Iterator for IntoIter<K, V, N> {
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Inline { data, index, len } => {
                if *index < *len {
                    let kv = unsafe { data[*index].as_ptr().read() };
                    *index += 1;
                    Some(kv)
                } else {
                    None
                }
            }
            IntoIter::Heap(iter) => iter.next(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl<K, V, const N: usize> ExactSizeIterator for IntoIter<K, V, N> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            IntoIter::Inline { index, len, .. } => len - index,
            IntoIter::Heap(iter) => iter.len(),
        }
    }
}

impl<K, V, const N: usize> FusedIterator for IntoIter<K, V, N> {}

impl<K, V, const N: usize> Drop for IntoIter<K, V, N> {
    fn drop(&mut self) {
        // Drop remaining elements for the inline variant.
        if let IntoIter::Inline { data, index, len } = self {
            for slot in data.iter_mut().take(*len).skip(*index) {
                unsafe { core::ptr::drop_in_place(slot.as_mut_ptr()) };
            }
        }
        // Heap variant: hashbrown's IntoIter handles its own drop.
    }
}

/// An iterator over the keys of an [`SmallMap`].
pub struct Keys<'a, K, V>(Iter<'a, K, V>);

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Keys<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<K, V> FusedIterator for Keys<'_, K, V> {}

/// An iterator over the values of an [`SmallMap`].
pub struct Values<'a, K, V>(Iter<'a, K, V>);

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Values<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<K, V> FusedIterator for Values<'_, K, V> {}

/// A mutable iterator over the values of an [`SmallMap`].
pub struct ValuesMut<'a, K, V>(IterMut<'a, K, V>);

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}

// ═══════════════════════════════════════════════════════════════════════════
// Iterator constructors on SmallMap
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S> SmallMap<K, V, N, S> {
    /// An iterator visiting all key-value pairs in arbitrary order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        match &self.0 {
            Repr::Inline(inline) => Iter::Inline {
                ptr: inline.entries.as_ptr() as *const (K, V),
                index: 0,
                len: inline.len,
                _marker: core::marker::PhantomData,
            },
            Repr::Heap(heap) => Iter::Heap(heap.iter()),
        }
    }

    /// A mutable iterator visiting all key-value pairs in arbitrary order.
    /// The keys are immutable; only the values can be mutated.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        match &mut self.0 {
            Repr::Inline(inline) => IterMut::Inline {
                ptr: inline.entries.as_mut_ptr() as *mut (K, V),
                index: 0,
                len: inline.len,
                _marker: core::marker::PhantomData,
            },
            Repr::Heap(heap) => IterMut::Heap(heap.iter_mut()),
        }
    }

    /// An iterator visiting all keys in arbitrary order.
    #[inline]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.iter())
    }

    /// An iterator visiting all values in arbitrary order.
    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        Values(self.iter())
    }

    /// A mutable iterator visiting all values in arbitrary order.
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut(self.iter_mut())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// IntoIterator
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S> IntoIterator for SmallMap<K, V, N, S> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match self.0 {
            Repr::Inline(inline) => {
                let len = inline.len;
                let data = unsafe { core::ptr::read(&inline.entries) };
                // Bitwise-move hasher out so we can drop it explicitly. forget(inline)
                // skips Drop on the whole struct (correct for entries, which are now
                // owned by `data`), but would also skip the hasher's Drop → leak.
                let hasher = unsafe { core::ptr::read(&inline.hasher) };
                core::mem::forget(inline);
                drop(hasher); // drop the real hasher once
                IntoIter::Inline { data, index: 0, len }
            }
            Repr::Heap(heap) => IntoIter::Heap(heap.into_iter()),
        }
    }
}

impl<'a, K, V, const N: usize, S> IntoIterator for &'a SmallMap<K, V, N, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, const N: usize, S> IntoIterator for &'a mut SmallMap<K, V, N, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Trait implementations
// ═══════════════════════════════════════════════════════════════════════════

impl<K: Clone, V: Clone, const N: usize, S: Clone> Clone for SmallMap<K, V, N, S> {
    #[inline]
    fn clone(&self) -> Self {
        match &self.0 {
            Repr::Inline(inline) => SmallMap(Repr::Inline(inline.clone())),
            Repr::Heap(heap) => SmallMap(Repr::Heap(heap.clone())),
        }
    }
}

impl<K: fmt::Debug, V: fmt::Debug, const N: usize, S> fmt::Debug
    for SmallMap<K, V, N, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for (k, v) in self.iter() {
            map.entry(k, v);
        }
        map.finish()
    }
}

impl<K, V, const N: usize, S> PartialEq for SmallMap<K, V, N, S>
where
    K: Eq + Hash,
    V: PartialEq,
    S: BuildHasher,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl<K, V, const N: usize, S> Eq for SmallMap<K, V, N, S>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
{
}

impl<K, V, const N: usize, S: Default> Default for SmallMap<K, V, N, S> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, Q, const N: usize, S> Index<&Q> for SmallMap<K, V, N, S>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
    S: BuildHasher,
{
    type Output = V;

    #[inline]
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

/// # Complexity
///
/// O(n × len) in the worst case (many duplicate keys requiring linear
/// scans on the inline array). For unique keys, O(n) amortized after the
/// inline→heap spill. SmallMap is a small-map type where `len` is bounded
/// by `N`, so this is acceptable for its use case; use [`SmallMap::insert`]
/// directly if you need per-element control.
impl<K, V, const N: usize, S> Extend<(K, V)> for SmallMap<K, V, N, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<K, V, const N: usize, S> FromIterator<(K, V)> for SmallMap<K, V, N, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();
        // If the hint exceeds inline capacity, skip inline entirely.
        let mut map = if hint > N { Self::with_capacity(hint) } else { Self::new() };
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

#[cfg(feature = "std")]
impl<K, V, const N: usize> From<std::collections::HashMap<K, V>> for SmallMap<K, V, N>
where
    K: Eq + Hash,
{
    fn from(map: std::collections::HashMap<K, V>) -> Self {
        let mut eco = Self::new();
        for (k, v) in map {
            eco.insert(k, v);
        }
        eco
    }
}
