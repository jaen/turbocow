use alloc::vec::Vec;
use core::hash::{BuildHasher, Hash};

use rkyv::rancor::Fallible;
use rkyv::ser::{Allocator, Writer};
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Archived, Deserialize, Place, Serialize};

use super::types::{Repr, SmallMap};

/// Alias for the archived entry type — `<(K, V) as Archive>::Archived`.
type ArchivedEntry<K, V> = Archived<(K, V)>;

// ═══════════════════════════════════════════════════════════════════════════
// Archive — archived as a Vec of (K, V) pairs
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S> Archive for SmallMap<K, V, N, S>
where
    K: Archive,
    V: Archive,
{
    type Archived = ArchivedVec<ArchivedEntry<K, V>>;
    type Resolver = VecResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(self.len(), resolver, out);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Serialize
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S, Ser> Serialize<Ser> for SmallMap<K, V, N, S>
where
    K: Serialize<Ser> + Clone,
    V: Serialize<Ser> + Clone,
    Ser: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<VecResolver, Ser::Error> {
        match &self.0 {
            // Fast path: inline storage is a contiguous &[(K, V)] slice.
            Repr::Inline(inline) => {
                let slice = unsafe { inline.as_slice() };
                ArchivedVec::serialize_from_slice(slice, serializer)
            }
            // Heap path: hashbrown exposes (&K, &V) not &(K, V),
            // so we collect into a temporary Vec<(K, V)>.
            Repr::Heap(heap) => {
                let entries: Vec<(K, V)> =
                    heap.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                ArchivedVec::serialize_from_slice(&entries, serializer)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Deserialize
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, const N: usize, S, D> Deserialize<SmallMap<K, V, N, S>, D>
    for ArchivedVec<ArchivedEntry<K, V>>
where
    K: Archive + Eq + Hash,
    V: Archive,
    S: BuildHasher + Default,
    ArchivedEntry<K, V>: Deserialize<(K, V), D>,
    D: Fallible + ?Sized,
{
    fn deserialize(
        &self,
        deserializer: &mut D,
    ) -> Result<SmallMap<K, V, N, S>, D::Error> {
        let mut map = if self.len() > N {
            SmallMap::with_capacity(self.len())
        } else {
            SmallMap::new()
        };
        for archived_entry in self.as_slice() {
            let (k, v): (K, V) = archived_entry.deserialize(deserializer)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}
