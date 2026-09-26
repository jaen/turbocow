use core::fmt;
use core::marker::PhantomData;

use crate::allocator::AllocatorProvider;
use crate::vec::types::EcoVec;

impl<T, A> serde::Serialize for EcoVec<T, A>
where
    T: serde::Serialize,
    A: AllocatorProvider,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.as_slice().serialize(serializer)
    }
}

struct EcoVecVisitor<T, A>(PhantomData<(T, A)>);

impl<'de, T, A> serde::de::Visitor<'de> for EcoVecVisitor<T, A>
where
    T: serde::Deserialize<'de> + Clone,
    A: AllocatorProvider + Default + Clone,
{
    type Value = EcoVec<T, A>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence")
    }

    fn visit_seq<Acc>(self, mut seq: Acc) -> Result<Self::Value, Acc::Error>
    where
        Acc: serde::de::SeqAccess<'de>,
    {
        let len = seq.size_hint().unwrap_or(0);
        let mut values = EcoVec::<T, A>::with_capacity_in(len, A::default());
        while let Some(value) = seq.next_element()? {
            values.push(value);
        }
        Ok(values)
    }
}

impl<'de, T, A> serde::Deserialize<'de> for EcoVec<T, A>
where
    T: serde::Deserialize<'de> + Clone,
    A: AllocatorProvider + Default + Clone,
{
    /// # Allocator requirement
    ///
    /// This impl requires `A: Default` because deserialization creates a
    /// new `EcoVec<T, A>` with a default-constructed allocator. To
    /// deserialize with a specific allocator, deserialize into
    /// `EcoVec<T, Global>` and then move the elements into an
    /// `EcoVec<T, A>` constructed with your allocator.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(EcoVecVisitor(PhantomData))
    }
}
