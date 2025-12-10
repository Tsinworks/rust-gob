use serde::ser::{self, Serialize};

use crate::error::Error;
use crate::internal::ser::{SerializationCtx, SerializeStructValue};
use crate::internal::types::TypeId;
use crate::internal::utils::Bow;
use crate::schema::Schema;

use super::output::Output;

pub struct SerializeStruct<'t, O> {
    inner: SerializeStructValue<Bow<'t, Schema>>,
    out: O,
}

impl<'t, O: Output> SerializeStruct<'t, O> {
    pub(crate) fn new(
        type_id: TypeId,
        ctx: SerializationCtx<Bow<'t, Schema>>,
        out: O,
        len: usize,
    ) -> Result<Self, Error> {
        Ok(SerializeStruct {
            inner: SerializeStructValue::new(ctx, type_id, len)?,
            out,
        })
    }
}

impl<'t, O: Output> ser::SerializeStruct for SerializeStruct<'t, O> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.inner.serialize_field(key, value)
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Error> {
        self.inner.skip_field(key)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut ok = self.inner.end()?;
        ok.ctx.flush(self.out)
    }
}
