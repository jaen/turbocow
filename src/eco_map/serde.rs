use core::fmt;
use core::marker::PhantomData;

use super::EcoMap;

impl<K, V> serde::Serialize for EcoMap<K, V>
where
    K: serde::Serialize + Clone + PartialEq,
    V: serde::Serialize + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de, K, V> serde::Deserialize<'de> for EcoMap<K, V>
where
    K: serde::Deserialize<'de> + Clone + PartialEq,
    V: serde::Deserialize<'de> + Clone,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MapVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K, V> serde::de::Visitor<'de> for MapVisitor<K, V>
        where
            K: serde::Deserialize<'de> + Clone + PartialEq,
            V: serde::Deserialize<'de> + Clone,
        {
            type Value = EcoMap<K, V>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut map = match access.size_hint() {
                    Some(size) => EcoMap::with_capacity(size),
                    None => EcoMap::new(),
                };
                while let Some((k, v)) = access.next_entry()? {
                    map.insert(k, v);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(MapVisitor::<K, V>(PhantomData))
    }
}
