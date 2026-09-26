use rkyv::rancor::Fallible;
use rkyv::ser::{Allocator, Writer};
use rkyv::vec::{ArchivedVec, VecResolver};
use rkyv::{Archive, Deserialize, Place, Serialize};

use super::EcoBytes;

// ═══════════════════════════════════════════════════════════════════════════
// EcoBytes — archived as an ArchivedVec<u8> (same as Vec<u8>)
// ═══════════════════════════════════════════════════════════════════════════

impl Archive for EcoBytes {
    type Archived = ArchivedVec<u8>;
    type Resolver = VecResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_slice(self.as_slice(), resolver, out);
    }
}

impl<Ser> Serialize<Ser> for EcoBytes
where
    Ser: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<VecResolver, Ser::Error> {
        ArchivedVec::<u8>::serialize_from_slice(self.as_slice(), serializer)
    }
}

impl<D> Deserialize<EcoBytes, D> for ArchivedVec<u8>
where
    D: Fallible + ?Sized,
{
    fn deserialize(&self, _: &mut D) -> Result<EcoBytes, D::Error> {
        Ok(EcoBytes::from(self.as_slice()))
    }
}
