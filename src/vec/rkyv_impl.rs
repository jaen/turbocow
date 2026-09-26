use rkyv::rancor::Fallible;
use rkyv::ser::{Allocator, Writer};
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Deserialize, Place, Serialize};

use crate::allocator::AllocatorProvider;
use crate::vec::types::EcoVec;

// ═══════════════════════════════════════════════════════════════════════════
// Archive — archived as a Vec<T>
// ═══════════════════════════════════════════════════════════════════════════

impl<T, A> Archive for EcoVec<T, A>
where
    T: Archive,
    A: AllocatorProvider,
{
    type Archived = ArchivedVec<T::Archived>;
    type Resolver = VecResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_slice(self.as_slice(), resolver, out);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Serialize
// ═══════════════════════════════════════════════════════════════════════════

impl<T, A, Ser> Serialize<Ser> for EcoVec<T, A>
where
    T: Serialize<Ser>,
    A: AllocatorProvider,
    Ser: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<VecResolver, Ser::Error> {
        ArchivedVec::serialize_from_slice(self.as_slice(), serializer)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Deserialize
// ═══════════════════════════════════════════════════════════════════════════

impl<T, A, D> Deserialize<EcoVec<T, A>, D> for ArchivedVec<T::Archived>
where
    T: Archive + Clone,
    T::Archived: Deserialize<T, D>,
    A: AllocatorProvider + Default + Clone,
    D: Fallible + ?Sized,
{
    fn deserialize(&self, deserializer: &mut D) -> Result<EcoVec<T, A>, D::Error> {
        let mut vec = EcoVec::<T, A>::with_capacity_in(self.len(), A::default());
        for archived_val in self.as_slice() {
            let val: T = archived_val.deserialize(deserializer)?;
            vec.push(val);
        }
        Ok(vec)
    }
}
