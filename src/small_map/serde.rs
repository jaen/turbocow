use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;

use super::types::SmallMap;

impl<K, V, const N: usize, S> serde::Serialize for SmallMap<K, V, N, S>
where
    K: serde::Serialize + Eq + Hash,
    V: serde::Serialize,
    S: BuildHasher,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de, K, V, const N: usize, S> serde::Deserialize<'de> for SmallMap<K, V, N, S>
where
    K: serde::Deserialize<'de> + Eq + Hash,
    V: serde::Deserialize<'de>,
    S: BuildHasher + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MapVisitor<K, V, const N: usize, S>(
            PhantomData<fn() -> SmallMap<K, V, N, S>>,
        );

        impl<'de, K, V, const N: usize, S> serde::de::Visitor<'de> for MapVisitor<K, V, N, S>
        where
            K: serde::Deserialize<'de> + Eq + Hash,
            V: serde::Deserialize<'de>,
            S: BuildHasher + Default,
        {
            type Value = SmallMap<K, V, N, S>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut map = match access.size_hint() {
                    Some(size) if size > N => SmallMap::with_capacity(size),
                    _ => SmallMap::new(),
                };
                while let Some((k, v)) = access.next_entry()? {
                    map.insert(k, v);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}
