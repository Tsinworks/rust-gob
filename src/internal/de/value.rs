use std::io::Cursor;

use serde;
use serde::de::{Deserializer, IgnoredAny, Visitor, IntoDeserializer};
use serde::de::value::MapDeserializer;
use bytes::Buf;

use crate::error::Error;
use crate::internal::gob::Message;
use crate::internal::types::{TypeId, Types, WireType};

use super::field_value::FieldValueDeserializer;
use super::struct_value::StructValueDeserializer;
//use super::map_value::MapValueDeserializer;

// Minimal value container to feed serde's MapDeserializer
#[derive(Debug)]
enum SimpleValue {
    Str(String),
    I64(i64),
    U64(u64),
    Bool(bool),
    F64(f64),
}

impl<'de> IntoDeserializer<'de, Error> for SimpleValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> serde::Deserializer<'de> for SimpleValue {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            SimpleValue::Str(s) => visitor.visit_string(s),
            SimpleValue::I64(v) => visitor.visit_i64(v),
            SimpleValue::U64(v) => visitor.visit_u64(v),
            SimpleValue::Bool(v) => visitor.visit_bool(v),
            SimpleValue::F64(v) => visitor.visit_f64(v),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any enum
    }
}

pub(crate) struct ValueDeserializer<'t, 'de>
where
    'de: 't,
{
    type_id: TypeId,
    defs: &'t Types,
    msg: &'t mut Message<Cursor<&'de [u8]>>,
}

impl<'t, 'de> ValueDeserializer<'t, 'de> {
    pub fn new(
        type_id: TypeId,
        defs: &'t Types,
        msg: &'t mut Message<Cursor<&'de [u8]>>,
    ) -> ValueDeserializer<'t, 'de> {
        ValueDeserializer { type_id, defs, msg }
    }
}

impl<'t, 'de> Deserializer<'de> for ValueDeserializer<'t, 'de> {
    type Error = Error;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let Some(&WireType::Struct(ref struct_type)) = self.defs.lookup(self.type_id) {
            let de = StructValueDeserializer::new(struct_type, &self.defs, &mut self.msg);
            return de.deserialize_any(visitor);
        }

        if self.msg.read_uint()? != 0 {
            return Err(serde::de::Error::custom(format!(
                "neither a singleton nor a struct value"
            )));
        }

        let de = FieldValueDeserializer::new(self.type_id, &self.defs, &mut self.msg);
        return de.deserialize_any(visitor);
    }

    fn deserialize_enum<V>(
        mut self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let Some(&WireType::Struct(ref struct_type)) = self.defs.lookup(self.type_id) {
            let de = StructValueDeserializer::new(struct_type, &self.defs, &mut self.msg);
            return de.deserialize_enum(name, variants, visitor);
        }

        if self.msg.read_uint()? != 0 {
            return Err(serde::de::Error::custom(format!(
                "neither a singleton nor a struct value"
            )));
        }

        let de = FieldValueDeserializer::new(self.type_id, &self.defs, &mut self.msg);
        return de.deserialize_enum(name, variants, visitor);
    }

    fn deserialize_struct<V>(
        mut self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let mut is_map_interface = false;
        if let Some(&WireType::Struct(ref struct_type)) = self.defs.lookup(self.type_id) {
            let de = StructValueDeserializer::new(struct_type, &self.defs, &mut self.msg);
            return de.deserialize_struct(name, fields, visitor);
        } else if let Some(&WireType::Map(ref map_type)) = self.defs.lookup(self.type_id) {
            if map_type.elem.0 == TypeId::INTERFACE.0 && map_type.key.0 == TypeId::INTERFACE.0 {
                // deserialize as map[interface{}]interface{}
                is_map_interface = true;
            }
        }

        if is_map_interface {
            // Map[interface{}]interface{}: decode entries eagerly into an in-memory map
            // and feed it to the visitor. This avoids the streaming MapAccess path
            // that failed to mark struct fields as visited.
            
            // First read singleton marker (expected to be 0 for map values)
            let singleton = self.msg.read_uint()?;
            if singleton != 0 {
                return Err(serde::de::Error::custom(
                    "expected singleton=0 for map[interface{}]interface{} value"
                ));
            }
            
            let len = self.msg.read_uint()? as usize;
            let mut entries = Vec::with_capacity(len);

            for _ in 0..len {
                // key: interface value; expect string
                let key_ty_len = self.msg.read_bytes_len()?;
                let key_ty_pos = self.msg.get_ref().position() as usize;
                self.msg.get_mut().advance(key_ty_len);
                let key_ty_bytes = &self.msg.get_ref().get_ref()[key_ty_pos..key_ty_pos + key_ty_len];
                let key_ty = ::std::str::from_utf8(key_ty_bytes)
                    .map_err(|err| <Error as serde::de::Error>::custom(err))?;
                
                let _key_ty_id = self.msg.read_int()?;
                
                // Read byte count and singleton.
                // NOTE: Rust gob serializer writes these. Standard Go gob usually includes byte count
                // but might not singleton for interface. However, our internal logic expects them.
                // Based on successful parsing of 'uid' then 'int64', these fields ARE present.
                let _byte_count = self.msg.read_uint()?;
                let _singleton = self.msg.read_uint()?;

                // Read key value based on key_ty
                let key: String = match key_ty {
                    "string" => {
                        let k_len = self.msg.read_bytes_len()?;
                        let k_pos = self.msg.get_ref().position() as usize;
                        self.msg.get_mut().advance(k_len);
                        let k_bytes = &self.msg.get_ref().get_ref()[k_pos..k_pos + k_len];
                        ::std::str::from_utf8(k_bytes)
                            .map_err(|err| <Error as serde::de::Error>::custom(err))?
                            .to_string()
                    }
                    other => {
                        return Err(serde::de::Error::custom(format!(
                            "unsupported map key type in interface map: {}",
                            other
                        )))
                    }
                };

                // value: interface value
                let val_ty_len = self.msg.read_bytes_len()?;
                let val_ty_pos = self.msg.get_ref().position() as usize;
                self.msg.get_mut().advance(val_ty_len);
                let val_ty_bytes = &self.msg.get_ref().get_ref()[val_ty_pos..val_ty_pos + val_ty_len];
                let val_ty = ::std::str::from_utf8(val_ty_bytes)
                    .map_err(|err| <Error as serde::de::Error>::custom(err))?;
                
                let _val_ty_id = self.msg.read_int()?;
                let _val_byte_count = self.msg.read_uint()?;
                let _val_singleton = self.msg.read_uint()?;

                let value = match val_ty {
                    "string" => {
                        let v_len = self.msg.read_bytes_len()?;
                        let v_pos = self.msg.get_ref().position() as usize;
                        self.msg.get_mut().advance(v_len);
                        let v_bytes = &self.msg.get_ref().get_ref()[v_pos..v_pos + v_len];
                        let s = ::std::str::from_utf8(v_bytes)
                            .map_err(|err| <Error as serde::de::Error>::custom(err))?
                            .to_string();
                        SimpleValue::Str(s)
                    }
                    "int64" => SimpleValue::I64(self.msg.read_int()?),
                    "uint64" => SimpleValue::U64(self.msg.read_uint()?),
                    "bool" => SimpleValue::Bool(self.msg.read_bool()?),
                    "float64" => SimpleValue::F64(self.msg.read_float()?),
                    other => {
                        return Err(serde::de::Error::custom(format!(
                            "unsupported map value type in interface map: {}",
                            other
                        )))
                    }
                };

                entries.push((key, value));
            }

            let map_de = MapDeserializer::new(entries.into_iter());
            return visitor.visit_map(map_de);
        }

        let len = self.msg.read_uint()?;
        if len != 0 {
            return Err(serde::de::Error::custom(format!(
                "neither a singleton nor a struct value"
            )));
        }

        let de = FieldValueDeserializer::new(self.type_id, &self.defs, &mut self.msg);
        return de.deserialize_struct(name, fields, visitor);
    }

    #[inline]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_ignored_any(IgnoredAny)?;
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
        byte_buf option unit_struct newtype_struct seq tuple
        tuple_struct map identifier ignored_any
    }
}
