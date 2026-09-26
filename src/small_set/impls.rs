use core::borrow::Borrow;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::iter::FusedIterator;

use super::SmallSet;
use crate::small_map::SmallMap;

// ═══════════════════════════════════════════════════════════════════════════
// Constructors
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S: Default> SmallSet<T, N, S> {
    /// Creates an empty `SmallSet`.
    #[inline]
    pub fn new() -> Self {
        SmallSet(SmallMap::new())
    }
}

impl<T, const N: usize, S> SmallSet<T, N, S> {
    /// Creates an empty `SmallSet` with the given hasher.
    #[inline]
    pub fn with_hasher(hasher: S) -> Self {
        SmallSet(SmallMap::with_hasher(hasher))
    }

    /// Creates an empty `SmallSet` that will immediately use heap storage
    /// with at least the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self
    where
        S: Default,
    {
        SmallSet(SmallMap::with_capacity(capacity))
    }

    /// Returns the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the set contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns `true` if the set is currently using inline storage.
    #[inline]
    pub fn is_inline(&self) -> bool {
        self.0.is_inline()
    }

    /// Returns a reference to the set's hasher.
    #[inline]
    pub fn hasher(&self) -> &S {
        self.0.hasher()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Core operations
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S> SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    /// Adds a value to the set. Returns `true` if the value was newly
    /// inserted, `false` if it was already present.
    #[inline]
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value, ()).is_none()
    }

    /// Removes a value from the set. Returns `true` if the value was present.
    #[inline]
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.remove(value).is_some()
    }

    /// Removes and returns the value equal to the given one, if present.
    #[inline]
    pub fn take<Q>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.remove_entry(value).map(|(k, _)| k)
    }

    /// Returns `true` if the set contains the given value.
    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.contains_key(value)
    }

    /// Returns a reference to the value in the set equal to the given one.
    #[inline]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.get_key_value(value).map(|(k, _)| k)
    }

    /// Clears the set, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Retains only the elements specified by the predicate.
    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.0.retain(|k, _| f(k));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Set operations
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S> SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    /// Returns `true` if `self` has no elements in common with `other`.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        if self.len() <= other.len() {
            self.iter().all(|v| !other.contains(v))
        } else {
            other.iter().all(|v| !self.contains(v))
        }
    }

    /// Returns `true` if every element in `self` is also in `other`.
    pub fn is_subset(&self, other: &Self) -> bool {
        if self.len() > other.len() {
            return false;
        }
        self.iter().all(|v| other.contains(v))
    }

    /// Returns `true` if every element in `other` is also in `self`.
    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Visits the values representing the intersection (elements in both sets).
    pub fn intersection<'a>(&'a self, other: &'a Self) -> Intersection<'a, T, N, S> {
        let (smaller, larger) =
            if self.len() <= other.len() { (self, other) } else { (other, self) };
        Intersection { iter: smaller.iter(), other: larger }
    }

    /// Visits the values representing the union (elements in either set).
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T, N, S> {
        Union {
            iter: self.iter(),
            other_iter: other.iter(),
            current: self,
        }
    }

    /// Visits the values in `self` but not in `other`.
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T, N, S> {
        Difference { iter: self.iter(), other }
    }

    /// Visits the values in `self` or `other` but not both.
    pub fn symmetric_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> SymmetricDifference<'a, T, N, S> {
        SymmetricDifference {
            iter_a: self.difference(other),
            iter_b: other.difference(self),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Capacity
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S> SmallSet<T, N, S> {
    /// Returns the number of elements the set can hold without reallocation.
    ///
    /// When using inline storage, this is always `N`. When using heap
    /// storage, this is the underlying `HashMap` capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Shrinks the capacity as close to the length as possible.
    ///
    /// For inline storage, this is a no-op (fixed size `N`). For heap
    /// storage, delegates to hashbrown's `shrink_to_fit`.
    #[inline]
    pub fn shrink_to_fit(&mut self)
    where
        T: Eq + Hash,
        S: BuildHasher + Default,
    {
        self.0.shrink_to_fit();
    }
}

impl<T, const N: usize, S> SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    /// Reserves capacity for at least `additional` more elements.
    ///
    /// If the set is inline and `len + additional > N`, the set is spilled
    /// to heap storage first.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Iterators
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S> SmallSet<T, N, S> {
    /// An iterator visiting all values in arbitrary order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.0.iter())
    }
}

/// An iterator over the values of an [`SmallSet`].
pub struct Iter<'a, T>(crate::small_map::Iter<'a, T, ()>);

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> FusedIterator for Iter<'_, T> {}

/// An owning iterator over the values of an [`SmallSet`].
pub struct IntoIter<T, const N: usize>(crate::small_map::IntoIter<T, (), N>);

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

// ── Set operation iterators ──────────────────────────────────────────────

/// An iterator over the intersection of two [`SmallSet`]s.
pub struct Intersection<'a, T, const N: usize, S> {
    iter: Iter<'a, T>,
    other: &'a SmallSet<T, N, S>,
}

impl<'a, T, const N: usize, S> Iterator for Intersection<'a, T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.by_ref().find(|v| self.other.contains(v))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<T: Eq + Hash, const N: usize, S: BuildHasher> FusedIterator
    for Intersection<'_, T, N, S>
{
}

/// An iterator over the union of two [`SmallSet`]s.
pub struct Union<'a, T, const N: usize, S> {
    iter: Iter<'a, T>,
    other_iter: Iter<'a, T>,
    current: &'a SmallSet<T, N, S>,
}

impl<'a, T, const N: usize, S> Iterator for Union<'a, T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Yield all of self first, then elements of other not in self.
        self.iter
            .next()
            .or_else(|| self.other_iter.by_ref().find(|v| !self.current.contains(v)))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lo, hi) = self.iter.size_hint();
        let (_, other_hi) = self.other_iter.size_hint();
        (lo, hi.and_then(|a| other_hi.map(|b| a + b)))
    }
}

impl<T: Eq + Hash, const N: usize, S: BuildHasher> FusedIterator for Union<'_, T, N, S> {}

/// An iterator over elements in one [`SmallSet`] but not another.
pub struct Difference<'a, T, const N: usize, S> {
    iter: Iter<'a, T>,
    other: &'a SmallSet<T, N, S>,
}

impl<'a, T, const N: usize, S> Iterator for Difference<'a, T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.by_ref().find(|v| !self.other.contains(v))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<T: Eq + Hash, const N: usize, S: BuildHasher> FusedIterator
    for Difference<'_, T, N, S>
{
}

/// An iterator over the symmetric difference of two [`SmallSet`]s.
pub struct SymmetricDifference<'a, T, const N: usize, S> {
    iter_a: Difference<'a, T, N, S>,
    iter_b: Difference<'a, T, N, S>,
}

impl<'a, T, const N: usize, S> Iterator for SymmetricDifference<'a, T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter_a.next().or_else(|| self.iter_b.next())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a_lo, a_hi) = self.iter_a.size_hint();
        let (b_lo, b_hi) = self.iter_b.size_hint();
        (a_lo + b_lo, a_hi.and_then(|a| b_hi.map(|b| a + b)))
    }
}

impl<T: Eq + Hash, const N: usize, S: BuildHasher> FusedIterator
    for SymmetricDifference<'_, T, N, S>
{
}

// ═══════════════════════════════════════════════════════════════════════════
// IntoIterator
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S> IntoIterator for SmallSet<T, N, S> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl<'a, T, const N: usize, S> IntoIterator for &'a SmallSet<T, N, S> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Trait implementations
// ═══════════════════════════════════════════════════════════════════════════

impl<T: Clone, const N: usize, S: Clone> Clone for SmallSet<T, N, S> {
    #[inline]
    fn clone(&self) -> Self {
        SmallSet(self.0.clone())
    }
}

impl<T: fmt::Debug, const N: usize, S> fmt::Debug for SmallSet<T, N, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T, const N: usize, S> PartialEq for SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter().all(|v| other.contains(v))
    }
}

impl<T, const N: usize, S> Eq for SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

impl<T, const N: usize, S: Default> Default for SmallSet<T, N, S> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize, S> Extend<T> for SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for v in iter {
            self.insert(v);
        }
    }
}

impl<'a, T, const N: usize, S> Extend<&'a T> for SmallSet<T, N, S>
where
    T: Eq + Hash + Copy,
    S: BuildHasher + Default,
{
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        for &v in iter {
            self.insert(v);
        }
    }
}

impl<T, const N: usize, S> FromIterator<T> for SmallSet<T, N, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

// ── Bitwise set operators ────────────────────────────────────────────────

impl<T, const N: usize, S> core::ops::BitAnd<&SmallSet<T, N, S>> for &SmallSet<T, N, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = SmallSet<T, N, S>;

    /// Returns the intersection: `&self & &other`.
    fn bitand(self, other: &SmallSet<T, N, S>) -> SmallSet<T, N, S> {
        self.intersection(other).cloned().collect()
    }
}

impl<T, const N: usize, S> core::ops::BitOr<&SmallSet<T, N, S>> for &SmallSet<T, N, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = SmallSet<T, N, S>;

    /// Returns the union: `&self | &other`.
    fn bitor(self, other: &SmallSet<T, N, S>) -> SmallSet<T, N, S> {
        self.union(other).cloned().collect()
    }
}

impl<T, const N: usize, S> core::ops::Sub<&SmallSet<T, N, S>> for &SmallSet<T, N, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = SmallSet<T, N, S>;

    /// Returns the difference: `&self - &other`.
    fn sub(self, other: &SmallSet<T, N, S>) -> SmallSet<T, N, S> {
        self.difference(other).cloned().collect()
    }
}

impl<T, const N: usize, S> core::ops::BitXor<&SmallSet<T, N, S>> for &SmallSet<T, N, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = SmallSet<T, N, S>;

    /// Returns the symmetric difference: `&self ^ &other`.
    fn bitxor(self, other: &SmallSet<T, N, S>) -> SmallSet<T, N, S> {
        self.symmetric_difference(other).cloned().collect()
    }
}
