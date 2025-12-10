use std::io::Cursor;

use serde::de::{DeserializeSeed, Deserializer, MapAccess, Visitor};

use super::FieldValueDeserializer;
use crate::error::Error;
use crate::internal::gob::Message;
use crate::internal::types::{MapType, Types};

struct MapMapAccess<'t, 'de>
where
    'de: 't,
{
    def: &'t MapType,
    defs: &'t Types,
    remaining_count: u64,
    msg: &'t mut Message<Cursor<&'de [u8]>>,
}

impl<'t, 'de> MapMapAccess<'t, 'de> {
    fn new(
        def: &'t MapType,
        defs: &'t Types,
        msg: &'t mut Message<Cursor<&'de [u8]>>,
        len: u64,
    ) -> Result<MapMapAccess<'t, 'de>, Error> {
        Ok(MapMapAccess {
            def,
            defs,
            remaining_count: len,
            msg,
        })
    }
}

impl<'f, 'de> MapAccess<'de> for MapMapAccess<'f, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.remaining_count == 0 {
            return Ok(None);
        }
        self.remaining_count -= 1;
        let de = FieldValueDeserializer::new(self.def.key, self.defs, &mut self.msg);
        seed.deserialize(de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let de = FieldValueDeserializer::new(self.def.elem, self.defs, &mut self.msg);
        seed.deserialize(de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining_count as usize)
    }
}

pub(crate) struct MapValueDeserializer<'t, 'de>
where
    'de: 't,
{
    def: &'t MapType,
    defs: &'t Types,
    msg: &'t mut Message<Cursor<&'de [u8]>>,
}

impl<'t, 'de> MapValueDeserializer<'t, 'de> {
    #[inline]
    pub(crate) fn new(
        def: &'t MapType,
        defs: &'t Types,
        msg: &'t mut Message<Cursor<&'de [u8]>>,
    ) -> MapValueDeserializer<'t, 'de> {
        MapValueDeserializer { def, defs, msg }
    }

    #[allow(dead_code)]
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        // When using a map as a struct, we should not read the length again if it was already read in `ValueDeserializer`.
        // However, `MapMapAccess::new` currently reads `read_uint`.
        // We need to pass the length down or read it here if not passed.
        // But `ValueDeserializer` reads the length to check for singleton.
        
        // Wait, `ValueDeserializer` calls `new` which doesn't take length.
        // `MapMapAccess::new` reads it.
        // If `ValueDeserializer` read it, `MapMapAccess` will read the *next* thing which is wrong.
        
        // Let's modify `MapMapAccess::new` to take the length optionally?
        // Or simply `deserialize_any` reads it.
        
        // If we are called from `FieldValueDeserializer`, it is because `is_map_interface` is true.
        // But `FieldValueDeserializer` doesn't know the length if it didn't read it.
        // `ValueDeserializer` read the length and checked it.
        
        // We should probably just read the length here.
        // The issue in `ValueDeserializer` was that it read the length and then discarded it?
        // No, I modified `ValueDeserializer` to read `len`.
        
        // If `deserialize_struct` is called on `MapValueDeserializer`, it should behave like `deserialize_any` but maybe strict about fields?
        // A Map doesn't have "fields" in the Gob sense, it has keys.
        // So `visit_map` is correct.
        
        let len = self.msg.read_uint()?;
        visitor.visit_map(MapMapAccess::new(self.def, self.defs, self.msg, len)?)
    }
}

impl<'t, 'de> Deserializer<'de> for MapValueDeserializer<'t, 'de> {
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len = self.msg.read_uint()?;
        visitor.visit_map(MapMapAccess::new(self.def, self.defs, self.msg, len)?)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
