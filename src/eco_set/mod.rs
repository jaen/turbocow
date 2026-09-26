//! Refcounted, copy-on-write set backed by an [`EcoMap`].
//!
//! [`EcoSet`] provides **O(1) clone** (via the underlying [`EcoMap`]'s
//! refcounted `EcoVec`s). It is a thin newtype over
//! [`EcoMap<T, ()>`](crate::EcoMap), so it inherits the same trade-off:
//! membership is a **linear scan** (O(n)) — optimal for small sets but
//! degrading at scale.
//!
//! ## Choosing a set
//!
//! | Workload | Use |
//! |:---------|:----|
//! | Cloned often · small · read-mostly | [`EcoSet`] |
//! | Mutated often, or large, and **not** cloned | [`SmallSet`](crate::SmallSet) — turbocow's inline set that spills to a `hashbrown` hash table |
//! | Cloned often **and** large | a persistent set (e.g. the [`im`](https://crates.io/crates/im) crate's `HashSet`), or `std::collections::HashSet` if you don't need cheap clone |
//!
//! Like [`EcoMap`], `EcoSet` requires only `T: Clone + PartialEq` and does not
//! spill to a hash table — see the [`eco_map`] module docs for the full
//! rationale.

#[cfg(feature = "rkyv")]
mod rkyv_impl;
#[cfg(feature = "serde")]
mod serde;

use core::borrow::Borrow;
use core::fmt;

use crate::EcoMap;
use crate::eco_map;

// ── Type ────────────────────────────────────────────────────────────────

/// A refcounted, copy-on-write set backed by an [`EcoMap<T, ()>`].
///
/// Clone is **O(1)** — just atomic reference count increments.
///
/// ```
/// use turbocow::EcoSet;
///
/// let mut set = EcoSet::new();
/// set.insert("hello");
/// set.insert("world");
///
/// // O(1) clone
/// let snapshot = set.clone();
///
/// set.insert("!");
/// assert_eq!(snapshot.len(), 2);
/// assert_eq!(set.len(), 3);
/// ```
#[derive(Clone)]
pub struct EcoSet<T: Clone + PartialEq>(EcoMap<T, ()>);

// ── Core API ────────────────────────────────────────────────────────────

impl<T: Clone + PartialEq> EcoSet<T> {
    /// Creates an empty set.
    #[inline]
    pub fn new() -> Self {
        Self(EcoMap::new())
    }

    /// Creates an empty set with space for at least `capacity` entries.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(EcoMap::with_capacity(capacity))
    }

    /// The number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the set contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Removes all elements from the set.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Returns `true` if the set contains the value.
    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.0.contains_key(value)
    }

    /// Returns a reference to the value in the set, if any, that is equal
    /// to the given value.
    #[inline]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.0.get_key_value(value).map(|(k, _)| k)
    }

    /// Adds a value to the set. Returns whether the value was newly inserted.
    #[inline]
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value, ()).is_none()
    }

    /// Removes a value from the set. Returns whether the value was present.
    #[inline]
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.0.remove(value).is_some()
    }

    /// Retains only the elements for which `f` returns `true`.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.0.retain(|k, _| f(k));
    }

    /// An iterator over the elements.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.0.keys())
    }

    /// Returns the total number of elements the set can hold without reallocation.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Reserves capacity for at least `additional` more elements.
    ///
    /// Triggers copy-on-write if the backing storage is shared.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Shrinks the capacity as close to the length as possible.
    ///
    /// If the backing storage is shared (reference count > 1), this is
    /// a no-op for that storage — see [`EcoMap::shrink_to_fit`] for details.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }
}

// ── Set operations ──────────────────────────────────────────────────────

impl<T: Clone + PartialEq> EcoSet<T> {
    /// Returns `true` if `self` has no elements in common with `other`.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.iter().all(|v| !other.contains(v))
    }

    /// Returns `true` if every element in `self` is in `other`.
    pub fn is_subset(&self, other: &Self) -> bool {
        self.iter().all(|v| other.contains(v))
    }

    /// Returns `true` if every element in `other` is in `self`.
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    /// Visits the values representing the intersection, i.e., elements
    /// in both `self` and `other`.
    pub fn intersection<'a>(&'a self, other: &'a Self) -> Intersection<'a, T> {
        Intersection { iter: self.iter(), other }
    }

    /// Visits the values representing the union, i.e., elements in `self`
    /// or `other` or both. Elements from `self` come first, then elements
    /// from `other` that are not in `self`.
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T> {
        Union {
            iter: self.iter(),
            other_iter: other.iter(),
            current: self,
        }
    }

    /// Visits the values in `self` that are not in `other`.
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T> {
        Difference { iter: self.iter(), other }
    }

    /// Visits the values in `self` or `other` but not both.
    pub fn symmetric_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> SymmetricDifference<'a, T> {
        SymmetricDifference {
            iter_a: self.difference(other),
            iter_b: other.difference(self),
        }
    }
}

// ── Iterators ───────────────────────────────────────────────────────────

/// An iterator over the elements of an [`EcoSet`].
pub struct Iter<'a, T>(eco_map::Keys<'a, T>);

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

/// A consuming iterator over the elements of an [`EcoSet`].
pub struct IntoIter<T: Clone + PartialEq>(eco_map::IntoIter<T, ()>);

impl<T: Clone + PartialEq> Iterator for IntoIter<T> {
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

impl<T: Clone + PartialEq> ExactSizeIterator for IntoIter<T> {}

impl<T: Clone + PartialEq> DoubleEndedIterator for IntoIter<T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, _)| k)
    }
}

// ── Set-algebra iterators ───────────────────────────────────────────────

/// An iterator over the intersection of two [`EcoSet`]s.
///
/// Elements yielded are those present in both sets.
pub struct Intersection<'a, T: Clone + PartialEq> {
    iter: Iter<'a, T>,
    other: &'a EcoSet<T>,
}

impl<'a, T: Clone + PartialEq> Iterator for Intersection<'a, T> {
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

/// An iterator over the union of two [`EcoSet`]s.
///
/// Yields all elements from `self`, then elements from `other` not in `self`.
pub struct Union<'a, T: Clone + PartialEq> {
    iter: Iter<'a, T>,
    other_iter: Iter<'a, T>,
    current: &'a EcoSet<T>,
}

impl<'a, T: Clone + PartialEq> Iterator for Union<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
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

/// An iterator over elements in one [`EcoSet`] but not another.
pub struct Difference<'a, T: Clone + PartialEq> {
    iter: Iter<'a, T>,
    other: &'a EcoSet<T>,
}

impl<'a, T: Clone + PartialEq> Iterator for Difference<'a, T> {
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

/// An iterator over the symmetric difference of two [`EcoSet`]s.
///
/// Yields elements in exactly one of `self` or `other`.
pub struct SymmetricDifference<'a, T: Clone + PartialEq> {
    iter_a: Difference<'a, T>,
    iter_b: Difference<'a, T>,
}

impl<'a, T: Clone + PartialEq> Iterator for SymmetricDifference<'a, T> {
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

// ── Trait impls ─────────────────────────────────────────────────────────

impl<T: Clone + PartialEq> Default for EcoSet<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + fmt::Debug> fmt::Debug for EcoSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T: Clone + PartialEq> PartialEq for EcoSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.is_subset(other)
    }
}

impl<T: Clone + Eq> Eq for EcoSet<T> {}

impl<T: Clone + PartialEq> FromIterator<T> for EcoSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::new();
        set.extend(iter);
        set
    }
}

impl<T: Clone + PartialEq> Extend<T> for EcoSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}

impl<'a, T: Clone + PartialEq> Extend<&'a T> for EcoSet<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value.clone());
        }
    }
}

impl<'a, T: Clone + PartialEq> IntoIterator for &'a EcoSet<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Clone + PartialEq> IntoIterator for EcoSet<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl<T: Clone + PartialEq, const N: usize> From<[T; N]> for EcoSet<T> {
    fn from(arr: [T; N]) -> Self {
        arr.into_iter().collect()
    }
}
