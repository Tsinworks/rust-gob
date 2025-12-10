use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;

use serde::ser::Serialize;

#[cfg(feature = "bytes")]
use serde_bytes::{ByteBuf, Bytes};

use types::*;
use Schema;

pub trait GobSerialize: Serialize {
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error>;
}

// # Implementatiions

// ## Primitive Types

macro_rules! primitive_impl {
    ($t:ty, $id:tt) => {
        impl GobSerialize for $t {
            #[inline]
            fn schema_register<S: Schema>(_: &mut S) -> Result<S::TypeId, S::Error> {
                Ok(TypeId::$id)
            }
        }
    };
}

primitive_impl!(bool, BOOL);
primitive_impl!(i8, I8);
primitive_impl!(i16, I16);
primitive_impl!(i32, I32);
primitive_impl!(i64, I64);
primitive_impl!(isize, I64);
primitive_impl!(u8, U8);
primitive_impl!(u16, U16);
primitive_impl!(u32, U32);
primitive_impl!(u64, U64);
primitive_impl!(usize, U64);
primitive_impl!(f32, F32);
primitive_impl!(f64, F64);
primitive_impl!(char, CHAR);

// ## Strings

impl GobSerialize for str {
    #[inline]
    fn schema_register<S: Schema>(_: &mut S) -> Result<S::TypeId, S::Error> {
        Ok(TypeId::STR)
    }
}

impl GobSerialize for String {
    #[inline]
    fn schema_register<S: Schema>(_: &mut S) -> Result<S::TypeId, S::Error> {
        Ok(TypeId::STR)
    }
}

// ## Bytes

#[cfg(feature = "bytes")]
impl<'a> GobSerialize for Bytes<'a> {
    #[inline]
    fn schema_register<S: Schema>(_: &mut S) -> Result<S::TypeId, S::Error> {
        Ok(TypeId::BYTES)
    }
}

#[cfg(feature = "bytes")]
impl GobSerialize for ByteBuf {
    #[inline]
    fn schema_register<S: Schema>(_: &mut S) -> Result<S::TypeId, S::Error> {
        Ok(TypeId::BYTES)
    }
}

// ## Option

impl<T: GobSerialize> GobSerialize for Option<T> {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        let id = T::schema_register(schema)?;
        schema.register_type(Type::Option(OptionType { value: id }))
    }
}

// ## PhantomData

impl<T> GobSerialize for PhantomData<T> {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        schema.register_type(Type::UnitStruct(UnitStructType {
            _phan: PhantomData,
            name: Cow::Borrowed("PhantomData"),
        }))
    }
}

// ## Arrays

macro_rules! array_impls {
    {$($len:tt)+} => {
        $(
            impl<T: GobSerialize> GobSerialize for [T; $len] {
                #[inline]
                fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
                    let id = T::schema_register(schema)?;
                    schema.register_type(Type::Seq(SeqType { len: Some($len), element: id }))
                }
            }
        )+
    }
}

array_impls! {
    00 01 02 03 04 05 06 07 08 09
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}

// ## Slices

impl<T: GobSerialize> GobSerialize for [T] {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        let id = T::schema_register(schema)?;
        schema.register_type(Type::Seq(SeqType {
            len: None,
            element: id,
        }))
    }
}

// ## Sequence Collections

macro_rules! seq_impl {
    ($ty:ident <
        T $(: $tbound1:ident $(+ $tbound2:ident)*)* $(, $typaram:ident : $bound:ident)*
    >) => {
        impl<T $(, $typaram)*> GobSerialize for $ty<T $(, $typaram)*>
        where
            T: GobSerialize $(+ $tbound1 $(+ $tbound2)*)*,
            $($typaram: $bound,)*
        {
            #[inline]
            fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
                let id = T::schema_register(schema)?;
                schema.register_type(Type::Seq(SeqType { len: None, element: id }))
            }
        }
    }
}

seq_impl!(BinaryHeap<T: Ord>);
seq_impl!(BTreeSet<T: Ord>);
seq_impl!(HashSet<T: Eq + Hash, H: BuildHasher>);
seq_impl!(LinkedList<T>);
seq_impl!(Vec<T>);
seq_impl!(VecDeque<T>);

// ## Range

impl<Idx: GobSerialize> GobSerialize for ::std::ops::Range<Idx> {
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        let id = Idx::schema_register(schema)?;
        schema.register_type(Type::Struct(StructType {
            name: Cow::Borrowed("Range"),
            fields: Cow::Owned(vec![
                StructField {
                    name: Cow::Borrowed("start"),
                    id: id.clone(),
                },
                StructField {
                    name: Cow::Borrowed("end"),
                    id,
                },
            ]),
        }))
    }
}

// ## Unit

impl GobSerialize for () {
    #[inline]
    fn schema_register<S: Schema>(_: &mut S) -> Result<S::TypeId, S::Error> {
        Ok(TypeId::UNIT)
    }
}

// ## Tuples

macro_rules! tuple_impls {
    ($($len:expr => ($($n:tt $name:ident)+))+) => {
        $(
            impl<$($name),+> GobSerialize for ($($name,)+)
            where
                $($name: GobSerialize,)+
            {
                #[inline]
                fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
                    let elements = vec![
                        $(
                            $name::schema_register(schema)?,
                        )+
                    ];
                    schema.register_type(Type::Tuple(TupleType { elements: Cow::Owned(elements) }))
                }
            }
        )+
    }
}

tuple_impls! {
    1 => (0 T0)
    2 => (0 T0 1 T1)
    3 => (0 T0 1 T1 2 T2)
    4 => (0 T0 1 T1 2 T2 3 T3)
    5 => (0 T0 1 T1 2 T2 3 T3 4 T4)
    6 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5)
    7 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6)
    8 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7)
    9 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8)
    10 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9)
    11 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10)
    12 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11)
    13 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12)
    14 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13)
    15 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14)
    16 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14
        15 T15)
}

// ## Map Collections

macro_rules! map_impl {
    ($ty:ident <
        K $(: $kbound1:ident $(+ $kbound2:ident)*)*,
        V $(, $typaram:ident : $bound:ident)*
    >) => {
        impl<K, V $(, $typaram)*> GobSerialize for $ty<K, V $(, $typaram)*>
        where
            K: GobSerialize $(+ $kbound1 $(+ $kbound2)*)*,
            V: GobSerialize,
            $($typaram: $bound,)*
        {
            #[inline]
            fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
                let k = K::schema_register(schema)?;
                let v = V::schema_register(schema)?;
                schema.register_type(Type::Map(MapType { key: k, value: v }))
            }
        }
    }
}

map_impl!(BTreeMap<K: Ord, V>);
map_impl!(HashMap<K: Eq + Hash, V, H: BuildHasher>);

// ## References

impl<'a, T: GobSerialize + ?Sized> GobSerialize for &'a T {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        T::schema_register(schema)
    }
}

impl<'a, T: GobSerialize + ?Sized> GobSerialize for &'a mut T {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        T::schema_register(schema)
    }
}

impl<T: GobSerialize + ?Sized> GobSerialize for Box<T> {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        T::schema_register(schema)
    }
}

impl<'a, T: GobSerialize + ToOwned + ?Sized> GobSerialize for Cow<'a, T> {
    #[inline]
    fn schema_register<S: Schema>(schema: &mut S) -> Result<S::TypeId, S::Error> {
        T::schema_register(schema)
    }
}