use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;

use super::SmallSet;

impl<T, const N: usize, S> serde::Serialize for SmallSet<T, N, S>
where
    T: serde::Serialize + Eq + Hash,
    S: BuildHasher,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for v in self.iter() {
            seq.serialize_element(v)?;
        }
        seq.end()
    }
}

impl<'de, T, const N: usize, S> serde::Deserialize<'de> for SmallSet<T, N, S>
where
    T: serde::Deserialize<'de> + Eq + Hash,
    S: BuildHasher + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SetVisitor<T, const N: usize, S>(PhantomData<fn() -> SmallSet<T, N, S>>);

        impl<'de, T, const N: usize, S> serde::de::Visitor<'de> for SetVisitor<T, N, S>
        where
            T: serde::Deserialize<'de> + Eq + Hash,
            S: BuildHasher + Default,
        {
            type Value = SmallSet<T, N, S>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut set = match access.size_hint() {
                    Some(size) if size > N => SmallSet::with_capacity(size),
                    _ => SmallSet::new(),
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
