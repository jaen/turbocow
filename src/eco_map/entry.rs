use super::EcoMap;

/// A view into a single entry in an [`EcoMap`], which may either be vacant
/// or occupied.
///
/// This is constructed via [`EcoMap::entry`].
pub enum Entry<'a, K: Clone + PartialEq, V: Clone> {
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, K, V>),
    /// A vacant entry.
    Vacant(VacantEntry<'a, K, V>),
}

/// A view into an occupied entry in an [`EcoMap`].
pub struct OccupiedEntry<'a, K: Clone + PartialEq, V: Clone> {
    map: &'a mut EcoMap<K, V>,
    index: usize,
}

/// A view into a vacant entry in an [`EcoMap`].
pub struct VacantEntry<'a, K: Clone + PartialEq, V: Clone> {
    map: &'a mut EcoMap<K, V>,
    key: K,
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry
// ═══════════════════════════════════════════════════════════════════════════

impl<'a, K: Clone + PartialEq, V: Clone> Entry<'a, K, V> {
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

impl<'a, K: Clone + PartialEq, V: Clone> OccupiedEntry<'a, K, V> {
    pub(super) fn new(map: &'a mut EcoMap<K, V>, index: usize) -> Self {
        Self { map, index }
    }

    /// Returns a reference to the key in the entry.
    #[inline]
    pub fn key(&self) -> &K {
        &self.map.keys[self.index]
    }

    /// Returns a reference to the value in the entry.
    #[inline]
    pub fn get(&self) -> &V {
        &self.map.vals[self.index]
    }

    /// Returns a mutable reference to the value in the entry.
    ///
    /// Triggers copy-on-write if the values buffer is shared.
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.map.vals.make_mut()[self.index]
    }

    /// Converts the entry into a mutable reference to the value, with a
    /// lifetime bound to the map.
    ///
    /// Triggers copy-on-write if the values buffer is shared.
    #[inline]
    pub fn into_mut(self) -> &'a mut V {
        &mut self.map.vals.make_mut()[self.index]
    }

    /// Sets the value of the entry, returning the old value.
    ///
    /// Triggers copy-on-write if the values buffer is shared.
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        // mem::replace: move the old out, write the new, return the old.
        // Avoids a needless clone of the old value (the previous impl did
        // `vals[i].clone()` then assignment). Matches top-level
        // EcoMap::insert's behavior.
        core::mem::replace(&mut self.map.vals.make_mut()[self.index], value)
    }

    /// Removes the entry from the map, returning the value.
    #[inline]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Removes the entry from the map, returning the key and value.
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        let last = self.map.len() - 1;
        let keys = self.map.keys.make_mut();
        let vals = self.map.vals.make_mut();
        keys.swap(self.index, last);
        vals.swap(self.index, last);
        let k = self.map.keys.pop().unwrap();
        let v = self.map.vals.pop().unwrap();
        (k, v)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// VacantEntry
// ═══════════════════════════════════════════════════════════════════════════

impl<'a, K: Clone + PartialEq, V: Clone> VacantEntry<'a, K, V> {
    pub(super) fn new(map: &'a mut EcoMap<K, V>, key: K) -> Self {
        Self { map, key }
    }

    /// Returns a reference to the key that would be used when inserting
    /// through this entry.
    #[inline]
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Consumes the entry, returning the key.
    #[inline]
    pub fn into_key(self) -> K {
        self.key
    }

    /// Inserts the value into the map and returns a mutable reference to it.
    ///
    /// Triggers copy-on-write if buffers are shared.
    #[inline]
    pub fn insert(self, value: V) -> &'a mut V {
        self.map.keys.push(self.key);
        self.map.vals.push(value);
        let last = self.map.vals.len() - 1;
        &mut self.map.vals.make_mut()[last]
    }
}
