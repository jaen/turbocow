use core::hash::{BuildHasher, Hash};
use core::mem::MaybeUninit;

use super::types::{Repr, SmallMap};

/// A view into a single entry in an [`SmallMap`], which may either be vacant
/// or occupied.
///
/// This is constructed via [`SmallMap::entry`].
pub enum Entry<'a, K, V, const N: usize, S> {
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, K, V, N, S>),
    /// A vacant entry.
    Vacant(VacantEntry<'a, K, V, N, S>),
}

/// A view into an occupied entry in an [`SmallMap`].
pub struct OccupiedEntry<'a, K, V, const N: usize, S> {
    pub(super) inner: OccupiedInner<'a, K, V, N, S>,
}

pub(super) enum OccupiedInner<'a, K, V, const N: usize, S> {
    Inline { map: &'a mut SmallMap<K, V, N, S>, index: usize },
    Heap(hashbrown::hash_map::OccupiedEntry<'a, K, V, S>),
}

/// A view into a vacant entry in an [`SmallMap`].
pub struct VacantEntry<'a, K, V, const N: usize, S> {
    pub(super) inner: VacantInner<'a, K, V, N, S>,
}

pub(super) enum VacantInner<'a, K, V, const N: usize, S> {
    Inline { map: &'a mut SmallMap<K, V, N, S>, key: K, h2: u8 },
    Heap(hashbrown::hash_map::VacantEntry<'a, K, V, S>),
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry
// ═══════════════════════════════════════════════════════════════════════════

impl<'a, K, V, const N: usize, S> Entry<'a, K, V, N, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Returns a reference to this entry's key.
    #[inline]
    pub fn key(&self) -> &K {
        match self {
            Entry::Occupied(o) => o.key(),
            Entry::Vacant(v) => v.key(),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty,
    /// and returns a mutable reference to the value.
    #[inline]
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the
    /// function if empty, and returns a mutable reference to the value.
    #[inline]
    pub fn or_insert_with(self, f: impl FnOnce() -> V) -> &'a mut V {
        match self {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(f()),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the
    /// function (with the key as argument) if empty, and returns a mutable
    /// reference to the value.
    #[inline]
    pub fn or_insert_with_key(self, f: impl FnOnce(&K) -> V) -> &'a mut V {
        match self {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let value = f(v.key());
                v.insert(value)
            }
        }
    }

    /// Ensures a value is in the entry by inserting the default value if
    /// empty, and returns a mutable reference to the value.
    #[inline]
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        self.or_insert_with(Default::default)
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    #[inline]
    pub fn and_modify(self, f: impl FnOnce(&mut V)) -> Self {
        match self {
            Entry::Occupied(mut o) => {
                f(o.get_mut());
                Entry::Occupied(o)
            }
            Entry::Vacant(v) => Entry::Vacant(v),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OccupiedEntry
// ═══════════════════════════════════════════════════════════════════════════

impl<'a, K, V, const N: usize, S> OccupiedEntry<'a, K, V, N, S> {
    /// Returns a reference to the key in the entry.
    #[inline]
    pub fn key(&self) -> &K {
        match &self.inner {
            OccupiedInner::Inline { map, index } => match &map.0 {
                Repr::Inline(inline) => unsafe {
                    &inline.entries[*index].assume_init_ref().0
                },
                _ => unreachable!(),
            },
            OccupiedInner::Heap(entry) => entry.key(),
        }
    }

    /// Returns a reference to the value in the entry.
    #[inline]
    pub fn get(&self) -> &V {
        match &self.inner {
            OccupiedInner::Inline { map, index } => match &map.0 {
                Repr::Inline(inline) => unsafe {
                    &inline.entries[*index].assume_init_ref().1
                },
                _ => unreachable!(),
            },
            OccupiedInner::Heap(entry) => entry.get(),
        }
    }

    /// Returns a mutable reference to the value in the entry.
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        match &mut self.inner {
            OccupiedInner::Inline { map, index } => match &mut map.0 {
                Repr::Inline(inline) => unsafe {
                    &mut inline.entries[*index].assume_init_mut().1
                },
                _ => unreachable!(),
            },
            OccupiedInner::Heap(entry) => entry.get_mut(),
        }
    }

    /// Converts the entry into a mutable reference to the value, with a
    /// lifetime bound to the map.
    #[inline]
    pub fn into_mut(self) -> &'a mut V {
        match self.inner {
            OccupiedInner::Inline { map, index } => match &mut map.0 {
                Repr::Inline(inline) => unsafe {
                    &mut inline.entries[index].assume_init_mut().1
                },
                _ => unreachable!(),
            },
            OccupiedInner::Heap(entry) => entry.into_mut(),
        }
    }

    /// Sets the value of the entry, returning the old value.
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        match &mut self.inner {
            OccupiedInner::Inline { map, index } => match &mut map.0 {
                Repr::Inline(inline) => {
                    let (_, v) = unsafe { inline.entries[*index].assume_init_mut() };
                    core::mem::replace(v, value)
                }
                _ => unreachable!(),
            },
            OccupiedInner::Heap(entry) => entry.insert(value),
        }
    }

    /// Removes the entry from the map, returning the value.
    #[inline]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Removes the entry from the map, returning the key and value.
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        match self.inner {
            OccupiedInner::Inline { map, index } => match &mut map.0 {
                Repr::Inline(inline) => {
                    let entry = unsafe { inline.entries[index].as_ptr().read() };
                    unsafe { inline.erase(index) };
                    entry
                }
                _ => unreachable!(),
            },
            OccupiedInner::Heap(entry) => entry.remove_entry(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// VacantEntry
// ═══════════════════════════════════════════════════════════════════════════

impl<'a, K, V, const N: usize, S> VacantEntry<'a, K, V, N, S> {
    /// Returns a reference to the key that would be used when inserting
    /// through this entry.
    #[inline]
    pub fn key(&self) -> &K {
        match &self.inner {
            VacantInner::Inline { key, .. } => key,
            VacantInner::Heap(entry) => entry.key(),
        }
    }

    /// Consumes the entry, returning the key.
    #[inline]
    pub fn into_key(self) -> K {
        match self.inner {
            VacantInner::Inline { key, .. } => key,
            VacantInner::Heap(entry) => entry.into_key(),
        }
    }
}

impl<'a, K, V, const N: usize, S> VacantEntry<'a, K, V, N, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Inserts the value into the map and returns a mutable reference to it.
    #[inline]
    pub fn insert(self, value: V) -> &'a mut V {
        match self.inner {
            VacantInner::Inline { map, key, h2 } => {
                // Check fullness via shared borrow first to satisfy the
                // borrow checker (the two branches need independent &mut).
                let is_full = match &map.0 {
                    Repr::Inline(inline) => inline.is_full(),
                    _ => unreachable!(),
                };
                if !is_full {
                    match &mut map.0 {
                        Repr::Inline(inline) => {
                            let idx = inline.len;
                            inline.h2_bytes[idx] = h2;
                            inline.entries[idx] = MaybeUninit::new((key, value));
                            inline.len += 1;
                            unsafe { &mut inline.entries[idx].assume_init_mut().1 }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    // Inline is full — spill to heap, then insert.
                    map.force_spill();
                    match &mut map.0 {
                        Repr::Heap(heap) => match heap.entry(key) {
                            hashbrown::hash_map::Entry::Vacant(v) => v.insert(value),
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }
                }
            }
            VacantInner::Heap(entry) => entry.insert(value),
        }
    }
}
