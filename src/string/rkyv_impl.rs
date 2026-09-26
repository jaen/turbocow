use rkyv::rancor::{Fallible, Source};
use rkyv::string::{ArchivedString, StringResolver};
use rkyv::{Archive, Deserialize, Place, Serialize, SerializeUnsized};

use super::EcoString;

// ═══════════════════════════════════════════════════════════════════════════
// EcoString — archived as an ArchivedString (same as String)
// ═══════════════════════════════════════════════════════════════════════════

impl Archive for EcoString {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(self.as_str(), resolver, out);
    }
}

impl<Ser> Serialize<Ser> for EcoString
where
    Ser: Fallible + ?Sized,
    Ser::Error: Source,
    str: SerializeUnsized<Ser>,
{
    fn serialize(&self, serializer: &mut Ser) -> Result<StringResolver, Ser::Error> {
        ArchivedString::serialize_from_str(self.as_str(), serializer)
    }
}

impl<D> Deserialize<EcoString, D> for ArchivedString
where
    D: Fallible + ?Sized,
{
    fn deserialize(&self, _: &mut D) -> Result<EcoString, D::Error> {
        Ok(EcoString::from(self.as_str()))
    }
}
