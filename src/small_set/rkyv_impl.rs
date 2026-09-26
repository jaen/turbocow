use alloc::vec::Vec;
use core::hash::{BuildHasher, Hash};

use rkyv::rancor::Fallible;
use rkyv::ser::{Allocator, Writer};
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Deserialize, Place, Serialize};

use super::SmallSet;

// ═══════════════════════════════════════════════════════════════════════════
// Archive — archived as a Vec of T
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S> Archive for SmallSet<T, N, S>
where
    T: Archive,
{
    type Archived = ArchivedVec<T::Archived>;
    type Resolver = VecResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(self.len(), resolver, out);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Serialize
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S, Ser> Serialize<Ser> for SmallSet<T, N, S>
where
    T: Serialize<Ser> + Clone,
    Ser: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<VecResolver, Ser::Error> {
        // SmallSet wraps SmallMap<T, (), N, S>. We serialize just the keys.
        // Collect into Vec<T> since the inner map's entry layout is (T, ()),
        // and we only want to archive T values.
        let entries: Vec<T> = self.iter().cloned().collect();
        ArchivedVec::serialize_from_slice(&entries, serializer)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Deserialize
// ═══════════════════════════════════════════════════════════════════════════

impl<T, const N: usize, S, D> Deserialize<SmallSet<T, N, S>, D>
    for ArchivedVec<T::Archived>
where
    T: Archive + Eq + Hash,
    S: BuildHasher + Default,
    T::Archived: Deserialize<T, D>,
    D: Fallible + ?Sized,
{
    fn deserialize(&self, deserializer: &mut D) -> Result<SmallSet<T, N, S>, D::Error> {
        let mut set = if self.len() > N {
            SmallSet::with_capacity(self.len())
        } else {
            SmallSet::new()
        };
        for archived_val in self.as_slice() {
            let val: T = archived_val.deserialize(deserializer)?;
            set.insert(val);
        }
        Ok(set)
    }
}
