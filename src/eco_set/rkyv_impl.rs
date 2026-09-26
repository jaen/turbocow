use alloc::vec::Vec;
use rkyv::rancor::Fallible;
use rkyv::ser::{Allocator, Writer};
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Deserialize, Place, Serialize};

use super::EcoSet;

// ═══════════════════════════════════════════════════════════════════════════
// Archive — archived as a Vec of T
// ═══════════════════════════════════════════════════════════════════════════

impl<T> Archive for EcoSet<T>
where
    T: Archive + Clone + PartialEq,
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

impl<T, Ser> Serialize<Ser> for EcoSet<T>
where
    T: Serialize<Ser> + Clone + PartialEq,
    Ser: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<VecResolver, Ser::Error> {
        let entries: Vec<T> = self.iter().cloned().collect();
        ArchivedVec::serialize_from_slice(&entries, serializer)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Deserialize
// ═══════════════════════════════════════════════════════════════════════════

impl<T, D> Deserialize<EcoSet<T>, D> for ArchivedVec<T::Archived>
where
    T: Archive + Clone + PartialEq,
    T::Archived: Deserialize<T, D>,
    D: Fallible + ?Sized,
{
    fn deserialize(&self, deserializer: &mut D) -> Result<EcoSet<T>, D::Error> {
        let mut set = EcoSet::with_capacity(self.len());
        for archived_val in self.as_slice() {
            let val: T = archived_val.deserialize(deserializer)?;
            set.insert(val);
        }
        Ok(set)
    }
}
