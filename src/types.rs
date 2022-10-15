use crate::codec::{Codec, CodecError, CodecResult, Reader};
use crate::tag::{TaggedValue, ValueType};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::slice::Iter;

pub trait TdfGroup: Codec + Debug {
    fn start_two() -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub struct VarInt(pub u64);

/// Trait for converting var int to another type
/// and back again
trait AsVarInt: PartialEq + Eq + Debug {
    /// Function for converting self to VarInt
    fn to_var_int(self) -> VarInt;

    /// Function for converting VarInt to self
    fn from_var_int(value: VarInt) -> Self;
}

/// Macro for automatically generating From traits for VarInt
/// for the all the number types
macro_rules! into_var_int {
    ($($ty:ty),*) => {
        $(
            impl From<$ty> for VarInt {
                fn from(value: $ty) -> VarInt {
                    VarInt(value as u64)
                }
            }

            impl Into<$ty> for VarInt {
                fn into(self) -> $ty {
                    self.0 as $ty
                }
            }
        )*
    };
}

/// Macro for automatically generating the AsVarInt
/// trait for conversion between types
macro_rules! as_var_int {
    ($($ty:ty),*) => {
        $(
            impl AsVarInt for $ty {
                #[inline]
                fn to_var_int(self) -> VarInt {
                    VarInt::from(self)
                }

                #[inline]
                fn from_var_int(value: VarInt) -> $ty {
                    value.into()
                }
            }
        )*
    };
}

into_var_int!(i8, i16, i32, i64, u8, u16, u32, u64);
as_var_int!(i8, i16, i32, i64, u8, u16, u32, u64);

#[derive(Debug, PartialEq, Eq)]
pub struct VarIntList(pub Vec<VarInt>);

impl VarIntList {
    /// Creates a new VarIntList
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates a new VarIntList with the provided
    /// capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    /// Inserts a new value into the underlying list
    pub fn insert(&mut self, value: impl Into<VarInt>) {
        self.0.push(value.into())
    }

    /// Removes the value at the provided index and returns
    /// the value stored at it if there is one
    pub fn remove(&mut self, index: usize) -> Option<VarInt> {
        if index < self.0.len() {
            Some(self.0.remove(index))
        } else {
            None
        }
    }

    /// Retrieves the value at the provided index returning
    /// a borrow if one is there
    pub fn get(&mut self, index: usize) -> Option<&VarInt> {
        self.0.get(index)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TdfOptional<T: Codec> {
    Some(u8, TaggedValue<T>),
    None,
}

impl<T: Codec> TdfOptional<T> {
    /// Returns true if there is a value
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_, _))
    }

    /// Returns true if there is no value
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Function for choosing a some value with a
    /// default type
    #[inline]
    pub fn default_some(tag: &str, value: T) -> TdfOptional<T> {
        TdfOptional::Some(0, (tag.to_string(), value))
    }
}

pub const EMPTY_OPTIONAL: u8 = 0x7F;

/// Trait implemented by types that can be map keys
pub trait MapKey: PartialEq + Eq + Debug + Codec {}

impl MapKey for &'static str {}
impl MapKey for String {}
impl MapKey for VarInt {}

#[derive(Debug, Clone)]
pub struct TdfMapBuilder<K: MapKey, V: Codec> {
    /// The keys stored in this builder
    keys: Vec<K>,
    /// The values stored in this builder
    values: Vec<V>,
}

impl<K: MapKey, V: Codec> TdfMapBuilder<K, V> {
    pub fn add(mut self, key: impl Into<K>, value: impl Into<V>) -> Self {
        self.keys.push(key.into());
        self.values.push(value.into());
        self
    }

    pub fn build(self) -> TdfMap<K, V> {
        TdfMap::from_existing(self.keys, self.values)
    }
}

/// Structure for Tdf maps these are maps that are created
/// from two Vec so they retain insertion order but are slow
/// for lookups. This implementation guarantees the lengths
/// of both lists are the same
#[derive(Debug, Clone)]
pub struct TdfMap<K: MapKey, V: Codec> {
    /// The keys stored in this map
    keys: Vec<K>,
    /// The values stored in this map
    values: Vec<V>,
}

impl<K: MapKey, V: Codec> Codec for TdfMap<K, V> {
    fn encode(&self, output: &mut Vec<u8>) {
        let key_type = K::value_type();
        let value_type = V::value_type();

        key_type.encode(output);
        value_type.encode(output);

        let size = self.len();
        VarInt(size as u64).encode(output);

        for (key, value) in self {
            key.encode(output);
            value.encode(output);
        }
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let expected_key = K::value_type();
        let expected_value = V::value_type();

        let key_type = ValueType::decode(reader)?;
        let value_type = ValueType::decode(reader)?;

        if expected_key != key_type {
            return Err(CodecError::UnexpectedType(expected_key, key_type));
        }

        if expected_value != value_type {
            return Err(CodecError::UnexpectedType(expected_value, value_type));
        }

        let length = VarInt::decode(reader)?.0 as usize;
        let mut map = TdfMap::with_capacity(length);

        for _ in 0..length {
            let key = K::decode(reader)?;
            let value = V::decode(reader)?;
            map.insert(key, value);
        }

        Ok(map)
    }

    fn value_type() -> ValueType {
        ValueType::Map
    }
}

/// Implementation for converting a HashMap to a TdfMap by taking
/// all its keys and values and building lists for the TdfMap
impl<K: MapKey, V: Codec> From<HashMap<K, V>> for TdfMap<K, V> {
    fn from(map: HashMap<K, V>) -> Self {
        let mut keys = Vec::with_capacity(map.len());
        let mut values = Vec::with_capacity(map.len());

        for (key, value) in map {
            keys.push(key);
            values.push(value)
        }

        Self { keys, values }
    }
}

impl<K: MapKey, V: Codec> TdfMap<K, V> {
    /// Creates a new empty TdfMap
    pub fn new() -> TdfMap<K, V> {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn build() -> TdfMapBuilder<K, V> {
        TdfMapBuilder {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Creates a new empty TdfMap sized to account
    /// for the provided capacity of contents
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    pub fn from_existing(keys: Vec<K>, values: Vec<V>) -> Self {
        Self { keys, values }
    }

    /// Insert a new entry into the map
    pub fn insert(&mut self, key: impl Into<K>, value: impl Into<V>) {
        self.keys.push(key.into());
        self.values.push(value.into())
    }

    /// Inserts multiple entries from an iterable value
    /// (i.e. Vec / slice of key value tuples)
    pub fn insert_multiple(&mut self, entries: impl IntoIterator<Item = (K, V)>) {
        for (key, value) in entries {
            self.keys.push(key);
            self.values.push(value);
        }
    }

    /// Returns the index of the provided key or None if
    /// the key was not present
    fn index_of_key<Q: ?Sized>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        for i in 0..self.keys.len() {
            let key_at = self.keys[i].borrow();
            if key_at.eq(key) {
                return Some(i);
            }
        }
        None
    }

    /// Removes a value by its key and returns the entry
    /// that was present at that position.
    pub fn remove(&mut self, key: &K) -> Option<(K, V)> {
        let index = self.index_of_key(key)?;
        let key = self.keys.remove(index);
        let value = self.values.remove(index);
        Some((key, value))
    }

    /// Returns the value stored at the provided key if
    /// its present or None.
    #[inline]
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        let index = self.index_of_key(key)?;
        let value = self.values.get(index)?;
        Some(value)
    }

    /// Takes the value stored at the provided key out of
    /// the map taking ownership this also removes the key.
    pub fn take(&mut self, key: &K) -> Option<V> {
        let index = self.index_of_key(key)?;
        let value = self.values.remove(index);
        self.keys.remove(index);
        Some(value)
    }

    /// Iterator access for the map keys
    pub fn keys(&self) -> Iter<'_, K> {
        self.keys.iter()
    }

    /// Iterator access for the map values
    pub fn values(&self) -> Iter<'_, V> {
        self.values.iter()
    }

    /// Returns the length of this map
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns the key value pair stored at the
    /// provided index if one exists
    fn at_index(&self, index: usize) -> Option<(&K, &V)> {
        let key = self.keys.get(index)?;
        let value = self.values.get(index)?;
        Some((key, value))
    }
}

/// Iterator implementation for the TdfMap
/// for iterating over the entries in the
/// Map
pub struct TdfMapIter<'a, K: MapKey, V: Codec> {
    map: &'a TdfMap<K, V>,
    index: usize,
}

impl<'a, K: MapKey, V: Codec> Iterator for TdfMapIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.map.at_index(self.index);
        self.index += 1;
        value
    }
}

impl<'a, K: MapKey, V: Codec> IntoIterator for &'a TdfMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = TdfMapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        TdfMapIter {
            map: self,
            index: 0,
        }
    }
}

impl Codec for f32 {
    fn encode(&self, output: &mut Vec<u8>) {
        let bytes: [u8; 4] = self.to_be_bytes();
        output.extend_from_slice(&bytes);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let bytes = reader.take(4)?;
        Ok(f32::from_be_bytes(
            bytes.try_into().map_err(|_| CodecError::UnknownError)?,
        ))
    }
}

impl<T: AsVarInt + Copy> Codec for T {
    fn encode(&self, output: &mut Vec<u8>) {
        (*self).to_var_int().encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let value = VarInt::decode(reader)?;
        Ok(T::from_var_int(value))
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for bool {
    fn encode(&self, output: &mut Vec<u8>) {
        output.push(if *self { 1 } else { 0 })
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let byte = reader.take_one()?;
        Ok(byte == 1)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for VarInt {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = self.0;
        if value < 64 {
            output.push(value as u8);
        } else {
            let mut cur_byte = ((value & 63) as u8) | 128;
            output.push(cur_byte);
            let mut cur_shift = value >> 6;
            while cur_shift >= 128 {
                cur_byte = ((cur_shift & 127) | 128) as u8;
                cur_shift >>= 7;
                output.push(cur_byte);
            }
            output.push(cur_shift as u8)
        }
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let first = reader.take_one()?;
        let mut result = (first & 63) as u64;
        if first < 128 {
            return Ok(VarInt(result));
        }
        let mut shift = 6;
        let mut byte: u8;
        loop {
            byte = reader.take_one()?;
            result |= ((byte & 127) as u64) << shift;
            shift += 7;
            if byte < 128 {
                break;
            }
        }
        Ok(VarInt(result))
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for &'static str {
    fn encode(&self, output: &mut Vec<u8>) {
        let mut bytes = self.as_bytes().to_vec();
        match bytes.last() {
            // Ignore if already null terminated
            Some(0) => {}
            // Null terminate
            _ => bytes.push(0),
        }

        VarInt::encode(&VarInt(bytes.len() as u64), output);
        output.extend_from_slice(&bytes);
    }

    fn decode(_reader: &mut Reader) -> CodecResult<Self> {
        // Static string cannot be decoded only encoded
        Err(CodecError::UnknownError)
    }
}

impl Codec for String {
    fn encode(&self, output: &mut Vec<u8>) {
        let mut bytes = self.as_bytes().to_vec();
        match bytes.last() {
            // Ignore if already null terminated
            Some(0) => {}
            // Null terminate
            _ => bytes.push(0),
        }

        VarInt::encode(&VarInt(bytes.len() as u64), output);
        output.extend_from_slice(&bytes);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let length = VarInt::decode(reader)?.0 as usize;
        let bytes = reader.take(length)?;
        let text = String::from_utf8_lossy(bytes);
        let mut text = text.to_string();
        // Pop the null terminator from the end of the string
        text.pop();
        Ok(text)
    }

    fn value_type() -> ValueType {
        ValueType::String
    }
}

impl Codec for Vec<u8> {
    fn encode(&self, output: &mut Vec<u8>) {
        VarInt(self.len() as u64).encode(output);
        output.extend_from_slice(self)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let length = VarInt::decode(reader)?.0 as usize;
        let bytes = reader.take(length)?;
        Ok(bytes.to_vec())
    }

    fn value_type() -> ValueType {
        ValueType::Blob
    }
}

/// Trait for a Codec value which can be apart of a Tdf list
pub trait Listable: Codec {}

impl Listable for bool {}
impl Listable for VarInt {}
impl Listable for String {}
impl Listable for (VarInt, VarInt, VarInt) {}

impl<T: Listable> Codec for Vec<T> {
    fn encode(&self, output: &mut Vec<u8>) {
        T::value_type().encode(output);
        VarInt(self.len() as u64).encode(output);
        for value in self {
            value.encode(output);
        }
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let value_type = ValueType::decode(reader)?;
        let expected_type = T::value_type();
        if value_type != expected_type {
            return Err(CodecError::UnexpectedType(value_type, expected_type));
        }
        let length = VarInt::decode(reader)?.0 as usize;
        let mut out = Vec::with_capacity(length);
        for _ in 0..length {
            out.push(T::decode(reader)?);
        }
        Ok(out)
    }

    fn value_type() -> ValueType {
        ValueType::List
    }
}

impl<T: Codec> Codec for TdfOptional<T> {
    fn encode(&self, output: &mut Vec<u8>) {
        match self {
            TdfOptional::Some(ty, value) => {
                output.push(*ty);
                value.encode(output);
            }
            TdfOptional::None => output.push(EMPTY_OPTIONAL),
        }
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let ty = reader.take_one()?;
        Ok(if ty != 0x7F {
            let value = TaggedValue::<T>::decode(reader)?;
            TdfOptional::Some(ty, value)
        } else {
            TdfOptional::None
        })
    }

    fn value_type() -> ValueType {
        ValueType::Optional
    }
}

impl Codec for VarIntList {
    fn encode(&self, output: &mut Vec<u8>) {
        VarInt(self.0.len() as u64).encode(output);
        for value in &self.0 {
            value.encode(output);
        }
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let length = VarInt::decode(reader)?.0 as usize;
        let mut out = Vec::with_capacity(length);
        for _ in 0..length {
            out.push(VarInt::decode(reader)?)
        }
        Ok(VarIntList(out))
    }

    fn value_type() -> ValueType {
        ValueType::VarIntList
    }
}

impl Codec for (VarInt, VarInt) {
    fn encode(&self, output: &mut Vec<u8>) {
        self.0.encode(output);
        self.1.encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let a = VarInt::decode(reader)?;
        let b = VarInt::decode(reader)?;
        Ok((a, b))
    }

    fn value_type() -> ValueType {
        ValueType::Pair
    }
}

impl Codec for (VarInt, VarInt, VarInt) {
    fn encode(&self, output: &mut Vec<u8>) {
        self.0.encode(output);
        self.1.encode(output);
        self.2.encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let a = VarInt::decode(reader)?;
        let b = VarInt::decode(reader)?;
        let c = VarInt::decode(reader)?;
        Ok((a, b, c))
    }

    fn value_type() -> ValueType {
        ValueType::Triple
    }
}

#[cfg(test)]
mod test {
    use crate::types::TdfMap;

    #[test]
    fn test() {
        let mut map = TdfMap::<String, String>::new();
        map.insert("Test", "Abc");

        let value = map.get("Test");

        assert_eq!(value.unwrap(), "Abc");

        println!("{value:?}")
    }
}
