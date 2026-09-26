use core::fmt;
use core::marker::PhantomData;

use super::EcoSet;

impl<T> serde::Serialize for EcoSet<T>
where
    T: serde::Serialize + Clone + PartialEq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for v in self.iter() {
            seq.serialize_element(v)?;
        }
        seq.end()
    }
}

impl<'de, T> serde::Deserialize<'de> for EcoSet<T>
where
    T: serde::Deserialize<'de> + Clone + PartialEq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SetVisitor<T>(PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for SetVisitor<T>
        where
            T: serde::Deserialize<'de> + Clone + PartialEq,
        {
            type Value = EcoSet<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut set = match access.size_hint() {
                    Some(size) => EcoSet::with_capacity(size),
                    None => EcoSet::new(),
                };
                while let Some(v) = access.next_element()? {
                    set.insert(v);
                }
                Ok(set)
            }
        }

        deserializer.deserialize_seq(SetVisitor(PhantomData))
    }
}
