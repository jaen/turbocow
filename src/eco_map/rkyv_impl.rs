use alloc::vec::Vec;
use rkyv::rancor::Fallible;
use rkyv::ser::{Allocator, Writer};
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Archived, Deserialize, Place, Serialize};

use super::EcoMap;

/// Alias for the archived entry type — `<(K, V) as Archive>::Archived`.
type ArchivedEntry<K, V> = Archived<(K, V)>;

// ═══════════════════════════════════════════════════════════════════════════
// Archive — archived as a Vec of (K, V) pairs
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V> Archive for EcoMap<K, V>
where
    K: Archive + Clone,
    V: Archive + Clone,
{
    type Archived = ArchivedVec<ArchivedEntry<K, V>>;
    type Resolver = VecResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_len(self.len(), resolver, out);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Serialize — collect into Vec<(K, V)> and serialize as slice
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, Ser> Serialize<Ser> for EcoMap<K, V>
where
    K: Serialize<Ser> + Clone,
    V: Serialize<Ser> + Clone,
    Ser: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<VecResolver, Ser::Error> {
        let entries: Vec<(K, V)> =
            self.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        ArchivedVec::serialize_from_slice(&entries, serializer)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Deserialize
// ═══════════════════════════════════════════════════════════════════════════

impl<K, V, D> Deserialize<EcoMap<K, V>, D> for ArchivedVec<ArchivedEntry<K, V>>
where
    K: Archive + Clone + PartialEq,
    V: Archive + Clone,
    ArchivedEntry<K, V>: Deserialize<(K, V), D>,
    D: Fallible + ?Sized,
{
    fn deserialize(&self, deserializer: &mut D) -> Result<EcoMap<K, V>, D::Error> {
        let mut map = EcoMap::with_capacity(self.len());
        for archived_entry in self.as_slice() {
            let (k, v): (K, V) = archived_entry.deserialize(deserializer)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}
