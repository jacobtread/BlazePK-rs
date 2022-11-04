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
pub struct VarIntList<T: VarInt>(pub Vec<T>);

impl<T: VarInt> VarIntList<T> {
    /// Creates a new VarIntList
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates a new VarIntList with no capacity
    pub fn empty() -> Self {
        Self(Vec::with_capacity(0))
    }

    pub fn only(value: T) -> Self {
        let mut values = Vec::with_capacity(1);
        values.push(value);
        Self(values)
    }

    /// Creates a new VarIntList with the provided
    /// capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    /// Inserts a new value into the underlying list
    pub fn insert(&mut self, value: impl Into<T>) {
        self.0.push(value.into())
    }

    /// Removes the value at the provided index and returns
    /// the value stored at it if there is one
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.0.len() {
            Some(self.0.remove(index))
        } else {
            None
        }
    }

    /// Retrieves the value at the provided index returning
    /// a borrow if one is there
    pub fn get(&mut self, index: usize) -> Option<&T> {
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

pub trait VarInt: PartialEq + Eq + Debug + Codec {}

/// Trait implemented by types that can be map keys
pub trait MapKey: PartialEq + Eq + Debug + Codec {}

impl MapKey for &'static str {}

impl MapKey for String {}

impl<T: VarInt> MapKey for T {}

macro_rules! impl_var_int {
    ($($ty:ty),*) => {
        $(
        impl VarInt for $ty {}
        )*
    };
}

impl_var_int!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);

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

        self.len().encode(output);

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

        let length = usize::decode(reader)?;
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

impl<K: MapKey + PartialOrd, V: Codec> TdfMap<K, V> {
    /// Orders this map based on its keys by ordering keys that
    /// are greater further up in the map
    pub fn order(&mut self) {
        let keys = &mut self.keys;
        let values = &mut self.values;
        let mut did_run = true;
        while did_run {
            did_run = false;
            for i in 0..(keys.len() - 1) {
                if keys[i] > keys[i + 1] {
                    keys.swap(i, i + 1);
                    values.swap(i, i + 1);
                    did_run = true
                }
            }
        }
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
    pub fn only(key: impl Into<K>, value: impl Into<V>) -> TdfMap<K, V> {
        let mut keys = Vec::with_capacity(1);
        let mut values = Vec::with_capacity(1);

        keys.push(key.into());
        values.push(value.into());

        Self { keys, values }
    }

    pub fn pop_front(&mut self) -> Option<(K, V)> {
        let key = self.keys.pop()?;
        let value = self.values.pop()?;
        Some((key, value))
    }

    pub fn extend(&mut self, mut other: TdfMap<K, V>) {
        while let Some((key, value)) = other.pop_front() {
            if !self.keys.contains(&key) {
                self.insert(key, value);
            }
        }
    }

    /// Creates a new empty TdfMap
    pub fn new() -> TdfMap<K, V> {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Creates a new TdfMap where both Vec have
    /// a zero capacity
    pub fn empty() -> TdfMap<K, V> {
        Self {
            keys: Vec::with_capacity(0),
            values: Vec::with_capacity(0),
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
    pub fn take<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
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
    pub fn at_index(&self, index: usize) -> Option<(&K, &V)> {
        let key = self.keys.get(index)?;
        let value = self.values.get(index)?;
        Some((key, value))
    }

    pub fn iter<'a>(&'a self) -> TdfMapIter<'a, K, V> {
        TdfMapIter {
            map: self,
            index: 0,
        }
    }
}

impl<K: MapKey, V: Codec> Iterator for TdfMap<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.keys.pop()?;
        let value = self.values.pop()?;
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

impl Codec for bool {
    fn encode(&self, output: &mut Vec<u8>) {
        (if *self { 1u8 } else { 0u8 }).encode(output)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let byte = u8::decode(reader)?;
        Ok(byte == 1)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

macro_rules! impl_encode_var {
    ($value:ident, $output:ident) => {
        if $value < 64 {
            $output.push($value as u8);
            return;
        }
        let mut byte = (($value & 63) as u8) | 128;
        $output.push(byte);
        let mut cur_shift = $value >> 6;
        while cur_shift >= 128 {
            byte = ((cur_shift & 127) | 128) as u8;
            cur_shift >>= 7;
            $output.push(byte);
        }
        $output.push(cur_shift as u8)
    };
}

macro_rules! impl_decode_var {
    ($ty:ty, $reader:ident) => {{
        let first = $reader.take_one()?;
        let mut result = (first & 63) as $ty;
        if first < 128 {
            return Ok(result);
        }
        let mut shift: u8 = 6;
        let mut byte: u8;
        loop {
            byte = $reader.take_one()?;
            result |= ((byte & 127) as $ty) << shift;
            if byte < 128 {
                break;
            }
            shift += 7;
        }
        Ok(result)
    }};
}

impl Codec for u8 {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        if value < 64 {
            output.push(*self);
            return;
        }
        output.push((value & 63) | 128);
        output.push(value >> 6)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let first = reader.take_one()?;
        let mut result = first & 63;
        if first < 128 {
            return Ok(result);
        }
        let byte = reader.take_one()?;
        result |= (byte & 127) << 6;
        if byte >= 128 {
            reader.consume_while(|value| value >= 128);
        }
        Ok(result)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for i8 {
    fn encode(&self, output: &mut Vec<u8>) {
        u8::encode(&(*self as u8), output)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        Ok(u8::decode(reader)? as i8)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for u16 {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        if value < 64 {
            output.push(value as u8);
            return;
        }
        let mut byte = ((value & 63) as u8) | 128;
        let mut shift = value >> 6;
        output.push(byte);
        byte = ((shift & 127) | 128) as u8;
        shift >>= 7;
        output.push(byte);
        output.push(shift as u8);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(u16, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for i16 {
    fn encode(&self, output: &mut Vec<u8>) {
        u16::encode(&(*self as u16), output)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(i16, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for u32 {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        impl_encode_var!(value, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(u32, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for i32 {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        impl_encode_var!(value, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(i32, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for u64 {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        impl_encode_var!(value, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(u64, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for i64 {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        impl_encode_var!(value, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(i64, reader)
    }
}

impl Codec for usize {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        impl_encode_var!(value, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(usize, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

impl Codec for isize {
    fn encode(&self, output: &mut Vec<u8>) {
        let value = *self;
        impl_encode_var!(value, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        impl_decode_var!(isize, reader)
    }

    fn value_type() -> ValueType {
        ValueType::VarInt
    }
}

pub fn encode_str(value: &str, output: &mut Vec<u8>) {
    let mut bytes = value.as_bytes().to_vec();
    match bytes.last() {
        // Ignore if already null terminated
        Some(0) => {}
        // Null terminate
        _ => bytes.push(0),
    }

    bytes.len().encode(output);
    output.extend_from_slice(&bytes);
}

impl Codec for &'_ str {
    fn encode(&self, output: &mut Vec<u8>) {
        encode_str(self, output);
    }

    fn decode(_reader: &mut Reader) -> CodecResult<Self> {
        // Static string cannot be decoded only encoded
        Err(CodecError::InvalidAction(
            "Attempted to decode string with static lifetime",
        ))
    }

    fn value_type() -> ValueType {
        ValueType::String
    }
}

impl Codec for String {
    fn encode(&self, output: &mut Vec<u8>) {
        encode_str(self, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let length = usize::decode(reader)?;
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

#[derive(Debug, Clone)]
pub struct Blob(pub Vec<u8>);

impl Blob {
    pub fn empty() -> Self {
        Self(Vec::with_capacity(0))
    }
}

impl Codec for Blob {
    fn encode(&self, output: &mut Vec<u8>) {
        self.0.len().encode(output);
        output.extend_from_slice(&self.0)
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let length = usize::decode(reader)?;
        let bytes = reader.take(length)?;
        Ok(Blob(bytes.to_vec()))
    }

    fn value_type() -> ValueType {
        ValueType::Blob
    }
}

impl<T: Codec> Codec for Vec<T> {
    fn encode(&self, output: &mut Vec<u8>) {
        T::value_type().encode(output);
        self.len().encode(output);
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
        let length = usize::decode(reader)?;
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

impl<T: VarInt> Codec for VarIntList<T> {
    fn encode(&self, output: &mut Vec<u8>) {
        self.0.len().encode(output);
        for value in &self.0 {
            value.encode(output);
        }
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let length = usize::decode(reader)?;
        let mut out = Vec::with_capacity(length);
        for _ in 0..length {
            out.push(T::decode(reader)?)
        }
        Ok(VarIntList(out))
    }

    fn value_type() -> ValueType {
        ValueType::VarIntList
    }
}

impl<A: VarInt, B: VarInt> Codec for (A, B) {
    fn encode(&self, output: &mut Vec<u8>) {
        self.0.encode(output);
        self.1.encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let a = A::decode(reader)?;
        let b = B::decode(reader)?;
        Ok((a, b))
    }

    fn value_type() -> ValueType {
        ValueType::Pair
    }
}

impl<A: VarInt, B: VarInt, C: VarInt> Codec for (A, B, C) {
    fn encode(&self, output: &mut Vec<u8>) {
        self.0.encode(output);
        self.1.encode(output);
        self.2.encode(output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let a = A::decode(reader)?;
        let b = B::decode(reader)?;
        let c = C::decode(reader)?;
        Ok((a, b, c))
    }

    fn value_type() -> ValueType {
        ValueType::Triple
    }
}

#[cfg(test)]
mod test {
    use crate::types::TdfMap;
    use crate::{Codec, Reader};

    #[test]
    fn test_map_ord() {
        let mut map = TdfMap::<String, String>::new();

        map.insert("key1", "ABC");
        map.insert("key2", "ABC");
        map.insert("key4", "ABC");
        map.insert("key24", "ABC");
        map.insert("key11", "ABC");
        map.insert("key17", "ABC");

        map.order();

        println!("{map:?}")
    }

    #[test]
    fn test() {
        let mut map = TdfMap::<String, String>::new();
        map.insert("Test", "Abc");

        let value = map.get("Test");

        assert_eq!(value.unwrap(), "Abc");

        println!("{value:?}")
    }

    #[test]
    fn test_u8() {
        for value in u8::MIN..u8::MAX {
            let mut out = Vec::with_capacity(4);
            value.encode(&mut out);
            let mut reader = Reader::new(&out);
            let v2 = u8::decode(&mut reader).unwrap();
            assert_eq!(value, v2)
        }
    }

    #[test]
    fn test_u16() {
        for value in u16::MIN..u16::MAX {
            let mut out = Vec::new();
            value.encode(&mut out);
            let mut reader = Reader::new(&out);
            let v2 = u16::decode(&mut reader).unwrap();
            assert_eq!(value, v2)
        }
    }
}
