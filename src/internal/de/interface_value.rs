use std::io::Cursor;
use bytes::Buf;
use serde::de::{DeserializeSeed, Deserializer, IntoDeserializer, SeqAccess, Visitor};
use crate::error::Error;
use crate::internal::gob::Message;

struct InterfaceSeqAccess<'t, 'de>
where
    'de: 't,
{
    remaining_count: u64,
    msg: &'t mut Message<Cursor<&'de [u8]>>,
}

impl<'t, 'de> InterfaceSeqAccess<'t, 'de> {
    #[inline]
    fn new(msg: &'t mut Message<Cursor<&'de [u8]>>) -> InterfaceSeqAccess<'t, 'de> {
        InterfaceSeqAccess {
            remaining_count: 2,
            msg,
        }
    }
}

impl<'t, 'de> SeqAccess<'de> for InterfaceSeqAccess<'t, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.remaining_count == 0 {
            return Ok(None);
        }
        self.remaining_count -= 1;
        
        let len = self.msg.read_bytes_len()?;
        let pos = self.msg.get_ref().position() as usize;
        self.msg.get_mut().advance(len);
        let bytes = &self.msg.get_ref().get_ref()[pos..pos + len];
        println!("bytes: {:?}", bytes);

        let float = self.msg.read_float()?;
        seed.deserialize(float.into_deserializer()).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining_count as usize)
    }
}

pub(crate) struct InterfaceValueDeserializer<'t, 'de>
where
    'de: 't,
{
    msg: &'t mut Message<Cursor<&'de [u8]>>,
}

impl<'t, 'de> InterfaceValueDeserializer<'t, 'de> {
    #[inline]
    pub(crate) fn new(
        msg: &'t mut Message<Cursor<&'de [u8]>>,
    ) -> InterfaceValueDeserializer<'t, 'de> {
        InterfaceValueDeserializer { msg }
    }
}

impl<'t, 'de> Deserializer<'de> for InterfaceValueDeserializer<'t, 'de> {
    type Error = Error;

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(InterfaceSeqAccess::new(self.msg))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
