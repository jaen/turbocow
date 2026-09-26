//! Refcounted, copy-on-write map backed by two [`EcoVec`]s.
//!
//! [`EcoMap`] provides **O(1) clone** (two atomic increments) at the cost of
//! copy-on-write overhead on mutation. Lookup is a **linear scan** — optimal
//! for small maps (≤ ~16 entries), but it degrades at scale: `get`/`remove`
//! are O(n), and building with [`insert`](EcoMap::insert) is O(n²) because each
//! call scans for an existing key.
//!
//! ## Choosing a map
//!
//! `EcoMap` deliberately requires only `K: Clone + PartialEq` (no `Hash`, no
//! `Ord`) and does **not** spill to a hash table — the minimal-bounds linear
//! scan *is* the design. If lookup cost matters at your sizes, pick by workload
//! rather than fighting the scan:
//!
//! | Workload | Use |
//! |:---------|:----|
//! | Cloned often · small · read-mostly | [`EcoMap`] |
//! | Mutated often, or large, and **not** cloned | [`SmallMap`](crate::SmallMap) — turbocow's inline map that spills to a `hashbrown` hash table |
//! | Cloned often **and** large | a persistent map (e.g. the [`im`](https://crates.io/crates/im) crate's `HashMap`), or `std::collections::HashMap` if you don't need cheap clone |
//!
//! For bulk-loading entries you know have distinct keys, prefer
//! [`insert_unique`](EcoMap::insert_unique) /
//! [`extend_unique`](EcoMap::extend_unique): they skip the dedup scan and build
//! in O(n) instead of O(n²).

mod entry;
#[cfg(feature = "rkyv")]
mod rkyv_impl;
#[cfg(feature = "serde")]
mod serde;

pub use entry::{Entry, OccupiedEntry, VacantEntry};

use core::borrow::Borrow;
use core::fmt;
use core::ops::Index;

use crate::EcoVec;
use crate::allocator::Global;

// ── Type ────────────────────────────────────────────────────────────────

/// A refcounted, copy-on-write map backed by two [`EcoVec`]s (keys + values).
///
/// Clone is **O(1)** — two atomic reference count increments, regardless of
/// entry count.  Mutation triggers copy-on-write.
///
/// ```
/// use turbocow::EcoMap;
///
/// let mut map = EcoMap::new();
/// map.insert("a", 1);
/// map.insert("b", 2);
///
/// // O(1) clone — just bumps two refcounts
/// let snapshot = map.clone();
///
/// // COW: this insert copies on first mutation
/// map.insert("c", 3);
///
/// assert_eq!(snapshot.len(), 2);
/// assert_eq!(map.len(), 3);
/// ```
#[derive(Clone)]
pub struct EcoMap<K: Clone, V: Clone> {
    pub(crate) keys: EcoVec<K>,
    pub(crate) vals: EcoVec<V>,
}

// ── Constructors ────────────────────────────────────────────────────────

impl<K: Clone, V: Clone> EcoMap<K, V> {
    /// Creates an empty map.
    #[inline]
    pub fn new() -> Self {
        Self { keys: EcoVec::new(), vals: EcoVec::new() }
    }

    /// Creates an empty map with space for at least `capacity` entries.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: EcoVec::with_capacity(capacity),
            vals: EcoVec::with_capacity(capacity),
        }
    }

    /// The number of entries in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the map contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Removes all entries from the map.
    #[inline]
    pub fn clear(&mut self) {
        self.keys.clear();
        self.vals.clear();
    }

    /// Returns the total number of entries the map can hold without reallocation.
    ///
    /// Because `EcoMap` is backed by two `EcoVec`s (keys and values), this
    /// returns the minimum of the two backing capacities (they are always equal
    /// after any standard operation).
    #[inline]
    pub fn capacity(&self) -> usize {
        self.keys.capacity().min(self.vals.capacity())
    }

    /// Reserves capacity for at least `additional` more entries.
    ///
    /// Triggers copy-on-write if the backing vecs are shared.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.keys.reserve(additional);
        self.vals.reserve(additional);
    }

    /// Shrinks the capacity as close to the length as possible.
    ///
    /// If either backing vec is shared (reference count > 1), that vec
    /// is not shrunk — shrinking a shared buffer would disturb other owners.
    /// In practice both vecs are always in the same sharing state, so
    /// either both shrink or neither does.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.keys.shrink_to_fit();
        self.vals.shrink_to_fit();
    }
}

// ── Lookup & mutation ───────────────────────────────────────────────────

impl<K: Clone + PartialEq, V: Clone> EcoMap<K, V> {
    /// Returns a reference to the value corresponding to `key`, or `None`.
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.index_of(key).map(|i| &self.vals[i])
    }

    /// Returns the key-value pair corresponding to `key`, or `None`.
    #[inline]
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.index_of(key).map(|i| (&self.keys[i], &self.vals[i]))
    }

    /// Returns a mutable reference to the value corresponding to `key`.
    ///
    /// Triggers copy-on-write if the values buffer is shared.
    #[inline]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.index_of(key).map(|i| &mut self.vals.make_mut()[i])
    }

    /// Returns `true` if the map contains `key`.
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.index_of(key).is_some()
    }

    /// Inserts a key-value pair, returning the previous value if the key
    /// already existed.
    ///
    /// Triggers copy-on-write if buffers are shared.
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        if let Some(i) = self.index_of(&key) {
            // COW first, then swap — avoids cloning the old value.
            let vals = self.vals.make_mut();
            Some(core::mem::replace(&mut vals[i], val))
        } else {
            self.push_unchecked(key, val);
            None
        }
    }

    /// Appends an entry without scanning for an existing key. Internal helper
    /// backing the `*_unique` fast paths and the no-existing-key arm of
    /// [`insert`](Self::insert).
    #[inline]
    fn push_unchecked(&mut self, key: K, val: V) {
        self.keys.push(key);
        self.vals.push(val);
    }

    /// Inserts a key-value pair **without checking whether the key already
    /// exists** — an O(1)-amortized fast path for building a map from data the
    /// caller knows has no duplicate keys.
    ///
    /// [`insert`](Self::insert) scans the whole map on every call to honor
    /// last-wins semantics, so building `n` entries with it is O(n²);
    /// `insert_unique` skips that scan (O(n) over the whole build).
    ///
    /// # Correctness
    ///
    /// The caller must ensure `key` is not already present. If it is, the map
    /// ends up with two entries sharing a key and its length/lookups become
    /// inconsistent. This is a logic error, **not** undefined behavior — the
    /// map stays memory-safe. Debug builds assert the key is absent.
    ///
    /// Triggers copy-on-write if the backing buffers are shared.
    #[inline]
    pub fn insert_unique(&mut self, key: K, val: V) {
        debug_assert!(
            !self.contains_key(&key),
            "insert_unique called with a key already present in the map"
        );
        self.push_unchecked(key, val);
    }

    /// Bulk-appends entries the caller knows are pairwise-distinct and absent
    /// from the map, in O(n) rather than [`insert`](Self::insert)'s O(n²)
    /// dedup-on-each-call. See [`insert_unique`](Self::insert_unique) for the
    /// correctness contract (duplicate keys are a logic error, not UB).
    ///
    /// Reserves up front from the iterator's size hint.
    pub fn extend_unique<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();
        self.reserve(hint);
        for (k, v) in iter {
            self.push_unchecked(k, v);
        }
    }

    /// Removes a key, returning its value if it was present.
    ///
    /// Uses swap-remove for O(1) deletion (does not preserve order).
    /// Triggers copy-on-write if buffers are shared.
    ///
    /// # Panic safety
    ///
    /// Both backing vecs are shrunk via `pop` *before* either `Drop` runs
    /// (the popped values are bound to a tuple). If `K::drop` panics, `V` is
    /// dropped cleanly during unwind and `keys.len() == vals.len()` holds.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        let i = self.index_of(key)?;
        let last = self.len() - 1;
        // COW both vecs *before* either swap. If the map is shared and a clone
        // panics during a copy-on-write, the map is left consistent (neither vec
        // swapped) instead of keys-swapped-but-vals-not — a key/value
        // mispairing. (`make_mut` on the two disjoint fields can be held at once;
        // the borrows end before the `pop`s below.) Matches `retain`, which also
        // COWs both vecs up-front.
        let keys = self.keys.make_mut();
        let vals = self.vals.make_mut();
        keys.swap(i, last);
        vals.swap(i, last);
        // Symmetric pop: both vecs shrunk before either Drop runs.
        let popped: (Option<K>, Option<V>) = (self.keys.pop(), self.vals.pop());
        match popped {
            (Some(k), Some(v)) => {
                drop(k); // K::drop may panic; v is unwound cleanly
                Some(v)
            }
            _ => unreachable!("keys.len() == vals.len() by invariant"),
        }
    }

    /// Retains only the entries for which `f` returns `true`.
    ///
    /// Triggers copy-on-write if any entries are removed.
    ///
    /// # Panic safety
    ///
    /// If `f` panics, every removal that has already been *decided* (i.e. `f`
    /// returned `false`) has also been *applied* via symmetric `pop`. The
    /// map's invariants (`keys.len() == vals.len()`, no duplicate keys) hold
    /// after the panic is caught.
    ///
    /// # Performance note
    ///
    /// COWs both backing vecs eagerly up-front (via `make_mut`) even if no
    /// entries end up removed. Lazy COW would add per-iteration branching;
    /// the current behavior is predictable and matches `EcoMap::get_mut`.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        // COW both vecs eagerly up-front so subsequent `make_mut` calls in
        // the loop hit the already-unique fast path.
        self.keys.make_mut();
        self.vals.make_mut();

        let mut i = 0;
        while i < self.keys.len() {
            let keep = {
                let keys = self.keys.make_mut();
                let vals = self.vals.make_mut();
                f(&keys[i], &mut vals[i])
            };
            if keep {
                i += 1;
            } else {
                let last = self.keys.len() - 1;
                self.keys.make_mut().swap(i, last);
                self.vals.make_mut().swap(i, last);
                // Symmetric pop: bind to a tuple so both vecs are shrunk
                // *before* either Drop runs. If K::drop or V::drop panics,
                // the other element is unwound cleanly and the vecs stay
                // length-aligned.
                let popped: (Option<K>, Option<V>) = (self.keys.pop(), self.vals.pop());
                drop(popped);
                // Don't increment i; re-examine the swapped-in element.
            }
        }
    }

    /// Gets the given key's entry in the map for in-place manipulation.
    ///
    /// Triggers copy-on-write if the entry is modified.
    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        match self.index_of(&key) {
            Some(i) => Entry::Occupied(OccupiedEntry::new(self, i)),
            None => Entry::Vacant(VacantEntry::new(self, key)),
        }
    }

    /// Returns the index of `key` in the keys vec, or `None`.
    #[inline]
    fn index_of<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.keys.iter().position(|k| k.borrow() == key)
    }
}

// ── Iterators ───────────────────────────────────────────────────────────

impl<K: Clone, V: Clone> EcoMap<K, V> {
    /// An iterator over `(&K, &V)` pairs.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter { keys: self.keys.iter(), vals: self.vals.iter() }
    }

    /// A mutable iterator over `(&K, &mut V)` pairs.
    ///
    /// Triggers copy-on-write on the values buffer.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        let vals = self.vals.make_mut().iter_mut();
        let keys = self.keys.iter();
        IterMut { keys, vals }
    }

    /// An iterator over the keys.
    #[inline]
    pub fn keys(&self) -> Keys<'_, K> {
        Keys(self.keys.iter())
    }

    /// An iterator over the values.
    #[inline]
    pub fn values(&self) -> Values<'_, V> {
        Values(self.vals.iter())
    }

    /// A mutable iterator over the values.
    ///
    /// Triggers copy-on-write on the values buffer.
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_, V> {
        ValuesMut(self.vals.make_mut().iter_mut())
    }
}

/// An iterator over `(&K, &V)` pairs of an [`EcoMap`].
pub struct Iter<'a, K, V> {
    keys: core::slice::Iter<'a, K>,
    vals: core::slice::Iter<'a, V>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((self.keys.next()?, self.vals.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        Some((self.keys.next_back()?, self.vals.next_back()?))
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.keys.as_slice().iter().zip(self.vals.as_slice()))
            .finish()
    }
}

impl<K, V> Clone for Iter<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { keys: self.keys.clone(), vals: self.vals.clone() }
    }
}

/// A mutable iterator over `(&K, &mut V)` pairs of an [`EcoMap`].
pub struct IterMut<'a, K, V> {
    keys: core::slice::Iter<'a, K>,
    vals: core::slice::IterMut<'a, V>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((self.keys.next()?, self.vals.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}

impl<K, V> DoubleEndedIterator for IterMut<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        Some((self.keys.next_back()?, self.vals.next_back()?))
    }
}

/// A consuming iterator over `(K, V)` pairs of an [`EcoMap`].
pub struct IntoIter<K: Clone, V: Clone> {
    keys: crate::vec::IntoIter<K, Global>,
    vals: crate::vec::IntoIter<V, Global>,
}

impl<K: Clone, V: Clone> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some((self.keys.next()?, self.vals.next()?))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}

impl<K: Clone, V: Clone> ExactSizeIterator for IntoIter<K, V> {}

impl<K: Clone, V: Clone> DoubleEndedIterator for IntoIter<K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        Some((self.keys.next_back()?, self.vals.next_back()?))
    }
}

impl<K: Clone + fmt::Debug, V: Clone + fmt::Debug> fmt::Debug for IntoIter<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.keys.as_slice().iter().zip(self.vals.as_slice()))
            .finish()
    }
}

/// An iterator over the keys of an [`EcoMap`].
pub struct Keys<'a, K>(core::slice::Iter<'a, K>);

impl<'a, K> Iterator for Keys<'a, K> {
    type Item = &'a K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K> ExactSizeIterator for Keys<'_, K> {}
impl<K> DoubleEndedIterator for Keys<'_, K> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

/// An iterator over the values of an [`EcoMap`].
pub struct Values<'a, V>(core::slice::Iter<'a, V>);

impl<'a, V> Iterator for Values<'a, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<V> ExactSizeIterator for Values<'_, V> {}
impl<V> DoubleEndedIterator for Values<'_, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

/// A mutable iterator over the values of an [`EcoMap`].
pub struct ValuesMut<'a, V>(core::slice::IterMut<'a, V>);

impl<'a, V> Iterator for ValuesMut<'a, V> {
    type Item = &'a mut V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<V> ExactSizeIterator for ValuesMut<'_, V> {}
impl<V> DoubleEndedIterator for ValuesMut<'_, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

// ── Trait impls ─────────────────────────────────────────────────────────

impl<K: Clone, V: Clone> Default for EcoMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone + fmt::Debug, V: Clone + fmt::Debug> fmt::Debug for EcoMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Clone + PartialEq, V: Clone + PartialEq> PartialEq for EcoMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        // Pointer-identity fast-path: if both EcoVecs share the same
        // backing allocation, the maps are trivially equal.
        if self.keys == other.keys && self.vals == other.vals {
            return true;
        }
        // Fall back to element-wise comparison (order-independent).
        self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl<K: Clone + Eq, V: Clone + Eq> Eq for EcoMap<K, V> {}

impl<K: Clone + PartialEq, V: Clone> FromIterator<(K, V)> for EcoMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();
        let mut map = Self::with_capacity(hint);
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl<K: Clone + PartialEq, V: Clone> Extend<(K, V)> for EcoMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K: Clone + PartialEq, V: Clone> Extend<(&'a K, &'a V)> for EcoMap<K, V> {
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k.clone(), v.clone());
        }
    }
}

impl<'a, K: Clone, V: Clone> IntoIterator for &'a EcoMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K: Clone, V: Clone> IntoIterator for EcoMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            keys: self.keys.into_iter(),
            vals: self.vals.into_iter(),
        }
    }
}

impl<K, Q, V> Index<&Q> for EcoMap<K, V>
where
    K: Clone + PartialEq + Borrow<Q>,
    V: Clone,
    Q: PartialEq + ?Sized,
{
    type Output = V;

    #[inline]
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

impl<K: Clone + PartialEq, V: Clone, const N: usize> From<[(K, V); N]> for EcoMap<K, V> {
    fn from(arr: [(K, V); N]) -> Self {
        arr.into_iter().collect()
    }
}

// ── Consuming iterators ─────────────────────────────────────────────────

impl<K: Clone, V: Clone> EcoMap<K, V> {
    /// Creates a consuming iterator over the keys, discarding the values.
    #[inline]
    pub fn into_keys(self) -> IntoKeys<K> {
        IntoKeys(self.keys.into_iter())
    }

    /// Creates a consuming iterator over the values, discarding the keys.
    #[inline]
    pub fn into_values(self) -> IntoValues<V> {
        IntoValues(self.vals.into_iter())
    }

    /// Clears the map, returning all key-value pairs as an iterator.
    ///
    /// Keeps the allocated memory for reuse (if unique owner).
    #[inline]
    pub fn drain(&mut self) -> Drain<K, V> {
        let keys = core::mem::take(&mut self.keys);
        let vals = core::mem::take(&mut self.vals);
        Drain { keys: keys.into_iter(), vals: vals.into_iter() }
    }
}

/// A consuming iterator over the keys of an [`EcoMap`].
pub struct IntoKeys<K: Clone>(crate::vec::IntoIter<K, Global>);

impl<K: Clone> Iterator for IntoKeys<K> {
    type Item = K;
    #[inline]
    fn next(&mut self) -> Option<K> {
        self.0.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<K: Clone> ExactSizeIterator for IntoKeys<K> {}
impl<K: Clone> DoubleEndedIterator for IntoKeys<K> {
    #[inline]
    fn next_back(&mut self) -> Option<K> {
        self.0.next_back()
    }
}

/// A consuming iterator over the values of an [`EcoMap`].
pub struct IntoValues<V: Clone>(crate::vec::IntoIter<V, Global>);

impl<V: Clone> Iterator for IntoValues<V> {
    type Item = V;
    #[inline]
    fn next(&mut self) -> Option<V> {
        self.0.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<V: Clone> ExactSizeIterator for IntoValues<V> {}
impl<V: Clone> DoubleEndedIterator for IntoValues<V> {
    #[inline]
    fn next_back(&mut self) -> Option<V> {
        self.0.next_back()
    }
}

/// A draining iterator over key-value pairs of an [`EcoMap`].
pub struct Drain<K: Clone, V: Clone> {
    keys: crate::vec::IntoIter<K, Global>,
    vals: crate::vec::IntoIter<V, Global>,
}

impl<K: Clone, V: Clone> Iterator for Drain<K, V> {
    type Item = (K, V);
    #[inline]
    fn next(&mut self) -> Option<(K, V)> {
        Some((self.keys.next()?, self.vals.next()?))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.keys.size_hint()
    }
}
impl<K: Clone, V: Clone> ExactSizeIterator for Drain<K, V> {}
impl<K: Clone, V: Clone> DoubleEndedIterator for Drain<K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<(K, V)> {
        Some((self.keys.next_back()?, self.vals.next_back()?))
    }
}
