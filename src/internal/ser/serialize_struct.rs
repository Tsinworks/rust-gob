use std::borrow::Borrow;

use owning_ref::OwningRef;
use serde::ser::{self, Serialize};
use serde_gob::types::{StructField, Type};

use crate::error::Error;
use crate::internal::types::TypeId;
use crate::schema::{Schema, SchemaType};

use super::{FieldValueSerializer, SerializationCtx, SerializationOk};

enum StructMode {
    Struct {
        fields: OwningRef<SchemaType, [StructField<TypeId>]>,
        current_field_idx: usize,
        last_serialized_field_idx: i64,
    },
    Map {
        len: usize,
        key_type: TypeId,
        elem_type: TypeId,
        needs_init: bool,
    },
}

pub(crate) struct SerializeStructValue<S> {
    ctx: SerializationCtx<S>,
    mode: StructMode,
}

impl<S: Borrow<Schema>> SerializeStructValue<S> {
    pub(crate) fn new(
        ctx: SerializationCtx<S>,
        type_id: TypeId,
        len: usize,
    ) -> Result<Self, Error> {
        let schema_type = if let Some(schema_type) = ctx.schema.borrow().lookup(type_id) {
            schema_type
        } else {
            return Err(ser::Error::custom("type not found"));
        };

        match *schema_type {
            Type::Struct(_) => {
                let fields = OwningRef::new(schema_type).map(|typ| {
                    if let Type::Struct(ref struct_type) = *typ {
                        struct_type.fields()
                    } else {
                        unreachable!()
                    }
                });
                Ok(SerializeStructValue {
                    ctx,
                    mode: StructMode::Struct {
                        fields,
                        current_field_idx: 0,
                        last_serialized_field_idx: -1,
                    },
                })
            }
            Type::Map(ref map_type) => Ok(SerializeStructValue {
                ctx,
                mode: StructMode::Map {
                    len,
                    key_type: *map_type.key_type(),
                    elem_type: *map_type.value_type(),
                    needs_init: true,
                },
            }),
            _ => Err(ser::Error::custom("schema mismatch, not a struct or map")),
        }
    }

    pub(crate) fn from_parts(
        ctx: SerializationCtx<S>,
        fields: OwningRef<SchemaType, [StructField<TypeId>]>,
    ) -> Self {
        SerializeStructValue {
            ctx,
            mode: StructMode::Struct {
                fields,
                current_field_idx: 0,
                last_serialized_field_idx: -1,
            },
        }
    }
}

impl<S: Borrow<Schema>> ser::SerializeStruct for SerializeStructValue<S> {
    type Ok = SerializationOk<S>;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        match self.mode {
            StructMode::Struct {
                ref fields,
                ref mut current_field_idx,
                ref mut last_serialized_field_idx,
            } => {
                let pre_pos = self.ctx.value.get_ref().len();
                let field_delta = *current_field_idx as i64 - *last_serialized_field_idx;
                self.ctx.value.write_uint(field_delta as u64);

                let type_id = *fields[*current_field_idx].field_type();
                let is_empty = self.ctx.with_borrow(|ctx| {
                    let de = FieldValueSerializer { ctx, type_id };
                    value.serialize(de)
                })?;

                if !is_empty {
                    *last_serialized_field_idx = *current_field_idx as i64;
                } else {
                    // reset the buffer to the previous position
                    self.ctx.value.get_mut().truncate(pre_pos);
                }

                *current_field_idx += 1;
                Ok(())
            }
            StructMode::Map {
                ref mut needs_init,
                key_type,
                elem_type,
                len,
            } => {
                if *needs_init {
                    self.ctx.value.write_uint(0); // singleton marker
                    self.ctx.value.write_uint(len as u64);
                    *needs_init = false;
                }
                let type_id = key_type;
                self.ctx.with_borrow(|ctx| {
                    let de = FieldValueSerializer { ctx, type_id };
                    key.serialize(de)
                })?;

                let type_id = elem_type;
                self.ctx.with_borrow(|ctx| {
                    let de = FieldValueSerializer { ctx, type_id };
                    value.serialize(de)
                })?;
                Ok(())
            }
        }
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        match self.mode {
            StructMode::Struct {
                ref mut current_field_idx,
                ..
            } => {
                *current_field_idx += 1;
                Ok(())
            }
            StructMode::Map { .. } => {
                // Should we decrement len? Or just ignore?
                // If we skip a field in map mode, we end up with fewer items than declared.
                // This might be invalid Gob.
                // But SerializeStruct::skip_field is called for Option::None usually?
                // No, skip_field is rarely called by derived Serialize.
                // We'll ignore it for Map for now.
                Ok(())
            }
        }
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        match self.mode {
            StructMode::Struct { .. } => {
                self.ctx.value.write_uint(0);
            }
            StructMode::Map { len, .. } => {
                if len == 0 {
                    self.ctx.value.write_uint(0);
                }
            }
        }

        Ok(SerializationOk {
            ctx: self.ctx,
            is_empty: false,
        })
    }
}
