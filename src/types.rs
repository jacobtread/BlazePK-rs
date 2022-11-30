use crate::codec::{Decodable, Encodable, ValueType};

use crate::error::{DecodeError, DecodeResult};
use crate::reader::TdfReader;
use crate::tag::TdfType;
use crate::value_type;
use crate::writer::TdfWriter;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::slice::Iter;

#[derive(Debug, PartialEq, Eq)]
pub struct VarIntList<T>(pub Vec<T>);

impl<T> VarIntList<T> {
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

impl<C> Encodable for VarIntList<C>
where
    C: VarInt,
{
    fn encode(&self, output: &mut TdfWriter) {
        output.write_usize(self.0.len());
        for value in &self.0 {
            value.encode(output);
        }
    }
}

impl<C> Decodable for VarIntList<C>
where
    C: VarInt,
{
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let length = reader.read_usize()?;
        let mut out = Vec::with_capacity(length);
        for _ in 0..length {
            out.push(C::decode(reader)?);
        }
        Ok(VarIntList(out))
    }
}

impl<C> ValueType for VarIntList<C> {
    fn value_type() -> TdfType {
        TdfType::VarIntList
    }
}

/// Type that can be unset or contain a pair of key
/// values
#[derive(Debug, PartialEq, Eq)]
pub enum Union<C> {
    Set { key: u8, tag: String, value: C },
    Unset,
}

impl<C> Union<C> {
    /// Creates a new union with a unset value
    pub fn unset() -> Self {
        Self::Unset
    }

    /// Creates a new set union value with the provided
    /// key tag and value
    pub fn set(key: u8, tag: &str, value: C) -> Self {
        Self::Set {
            key,
            tag: tag.to_owned(),
            value,
        }
    }

    /// Checks if the union is of set type
    pub fn is_set(&self) -> bool {
        matches!(self, Self::Set { .. })
    }

    /// Checks if the union is of unset type
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }

    pub fn unwrap(self) -> C {
        match self {
            Self::Unset => panic!("Attempted to unwrap union with no value"),
            Self::Set { value, .. } => value,
        }
    }
}

impl<C> Into<Option<C>> for Union<C> {
    fn into(self) -> Option<C> {
        match self {
            Self::Set { value, .. } => Some(value),
            Self::Unset => None,
        }
    }
}

impl<C> ValueType for Union<C> {
    fn value_type() -> TdfType {
        TdfType::Union
    }
}

impl<C> Encodable for Union<C>
where
    C: Encodable + ValueType,
{
    fn encode(&self, output: &mut TdfWriter) {
        match self {
            Union::Set { key, tag, value } => {
                output.write_byte(*key);
                output.tag(tag.as_bytes(), C::value_type());
                value.encode(output);
            }
            Union::Unset => output.write_byte(UNION_UNSET),
        }
    }
}

impl<C> Decodable for Union<C>
where
    C: Decodable + ValueType,
{
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let key = reader.read_byte()?;
        if key == UNION_UNSET {
            return Ok(Union::Unset);
        }
        let tag = reader.read_tag()?;
        let expected_type = C::value_type();
        let actual_type = tag.1;
        if actual_type != expected_type {
            return Err(DecodeError::InvalidType {
                expected: expected_type,
                actual: actual_type,
            });
        }
        let value = C::decode(reader)?;

        Ok(Union::Set {
            key,
            tag: tag.0,
            value,
        })
    }
}

pub const UNION_UNSET: u8 = 0x7F;

pub trait VarInt: PartialEq + Eq + Debug + Encodable + Decodable {}

/// Trait that must be implemented on a type for it to
/// be considered a map key
pub trait MapKey: PartialEq + Eq + Debug {}

impl MapKey for &'_ str {}
impl MapKey for String {}
impl<T: VarInt> MapKey for T {}

macro_rules! impl_var_int {
    ($($ty:ty),*) => { $(impl VarInt for $ty {})* };
}

impl_var_int!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize);

/// Structure for maps used in the protocol. These maps have a special
/// order that is usually required and they retain the order of insertion
/// because it uses two vecs as the underlying structure
pub struct TdfMap<K, V> {
    /// The keys stored in this map
    keys: Vec<K>,
    /// The values stored in this map
    values: Vec<V>,
}

impl<K, V> Default for TdfMap<K, V> {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }
}

impl<K, V> Debug for TdfMap<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TdfMap {")?;
        for (key, value) in self.iter() {
            write!(f, "  \"{key:?}\": \"{value:?}\"\n")?;
        }
        f.write_str("}")
    }
}

impl<K, V> TdfMap<K, V> {
    /// Constructor implemention just uses the underlying default
    /// implemenation
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Function for creating a new TdfMap where the underlying
    /// lists have an initial capacity
    ///
    /// `capacity` The capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Returns the length of the underlying lists
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Creates a new iterator over the underlying items
    /// in the map
    pub fn iter<'a>(&'a self) -> TdfMapIter<'a, K, V> {
        TdfMapIter {
            map: self,
            index: 0,
        }
    }

    /// Returns the key and value stored at the provided index
    /// will return None if there is nothing at the provided index
    pub fn index<'a>(&'a self, index: usize) -> Option<(&'a K, &'a V)> {
        let key = self.keys.get(index)?;
        let value = self.values.get(index)?;
        Some((key, value))
    }

    /// Inserts a new key value pair into the underlying structure
    ///
    /// `key`   The entry key
    /// `value` The entry value
    pub fn insert<A: Into<K>, B: Into<V>>(&mut self, key: A, value: B) {
        self.keys.push(key.into());
        self.values.push(value.into())
    }

    /// Removes the last key and value returning them or None
    /// if there are no entries
    pub fn pop(&mut self) -> Option<(K, V)> {
        let key = self.keys.pop()?;
        let value = self.values.pop()?;
        Some((key, value))
    }

    /// Iterator access for the map keys
    pub fn keys(&self) -> Iter<'_, K> {
        self.keys.iter()
    }

    /// Iterator access for the map values
    pub fn values(&self) -> Iter<'_, V> {
        self.values.iter()
    }
}

impl<K, V> TdfMap<K, V>
where
    K: PartialEq + Eq,
{
    /// Extends this map with the contents of another map. Any keys that already
    /// exist in the map will be replaced with the keys from the other map
    /// and any keys not present will be inserted
    ///
    /// `other` The map to extend with
    pub fn extend(&mut self, other: TdfMap<K, V>) {
        for (key, value) in other.into_iter() {
            let key_index: Option<usize> = self.keys.iter().position(|value| key.eq(value));
            if let Some(index) = key_index {
                self.values[index] = value;
            } else {
                self.insert(key, value);
            }
        }
    }

    /// Returns the index of the provided key or None if
    /// the key was not present
    ///
    /// `key` The key to find the index of
    fn index_of_key<Q: ?Sized>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        for index in 0..self.keys.len() {
            let key_at = self.keys[index].borrow();
            if key_at.eq(key) {
                return Some(index);
            }
        }
        None
    }

    /// Removes a value by its key and returns the entry
    /// that was present at that position.
    ///
    /// `key` The key to remove
    pub fn remove(&mut self, key: &K) -> Option<(K, V)> {
        let index = self.index_of_key(key)?;
        let key = self.keys.remove(index);
        let value = self.values.remove(index);
        Some((key, value))
    }

    /// Returns the value stored at the provided key if
    /// its present or None.
    ///
    /// `key` The key to retrieve the value for
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

    /// Returns a mutable borrow to the value stored at the
    /// provided key if its present or None.
    ///
    /// `key` The key to retrieve the value for
    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        let index = self.index_of_key(key)?;
        let value = self.values.get_mut(index)?;
        Some(value)
    }

    /// Takes the value stored at the provided key out of
    /// the map taking ownership this also removes the key.
    pub fn get_owned<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        let index = self.index_of_key(key)?;
        let value = self.values.remove(index);
        self.keys.remove(index);
        Some(value)
    }
}

/// Iterator implementation for iterating over TdfMap
pub struct TdfMapIter<'a, K, V> {
    /// The map iterate over
    map: &'a TdfMap<K, V>,
    index: usize,
}

impl<'a, K, V> Iterator for TdfMapIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.map.index(self.index);
        self.index += 1;
        entry
    }
}

pub struct OwnedTdfMapIter<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K, V> Iterator for OwnedTdfMapIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let key = self.keys.pop()?;
        let value = self.values.pop()?;
        Some((key, value))
    }
}

impl<K, V> Encodable for TdfMap<K, V>
where
    K: Encodable + ValueType,
    V: Encodable + ValueType,
{
    fn encode(&self, output: &mut TdfWriter) {
        output.write_map_header(K::value_type(), V::value_type(), self.len());

        for (key, value) in self.iter() {
            key.encode(output);
            value.encode(output);
        }
    }
}

impl<K, V> Decodable for TdfMap<K, V>
where
    K: Decodable + ValueType,
    V: Decodable + ValueType,
{
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_map()
    }
}

impl<K, V> ValueType for TdfMap<K, V> {
    fn value_type() -> TdfType {
        TdfType::Map
    }
}

impl<K, V> TdfMap<K, V>
where
    K: PartialOrd,
{
    /// Orders this map based on its keys by ordering keys that
    /// are greater further up in the map
    pub fn order(&mut self) {
        let keys = &mut self.keys;
        let values = &mut self.values;
        let length = keys.len();
        // If empty or 1 item no need to order
        if length <= 1 {
            return;
        }
        let mut did_run = true;
        while did_run {
            did_run = false;
            for i in 0..(length - 1) {
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
impl<K, V> From<HashMap<K, V>> for TdfMap<K, V> {
    fn from(map: HashMap<K, V>) -> Self {
        let mut keys: Vec<K> = Vec::with_capacity(map.len());
        let mut values: Vec<V> = Vec::with_capacity(map.len());

        for (key, value) in map.into_iter() {
            keys.push(key);
            values.push(value)
        }

        Self { keys, values }
    }
}

impl<K, V> IntoIterator for TdfMap<K, V> {
    type Item = (K, V);
    type IntoIter = OwnedTdfMapIter<K, V>;

    fn into_iter(mut self) -> Self::IntoIter {
        self.keys.reverse();
        self.values.reverse();
        OwnedTdfMapIter {
            keys: self.keys,
            values: self.values,
        }
    }
}

impl Encodable for f32 {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_f32(*self)
    }
}

impl Decodable for f32 {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_f32()
    }
}

value_type!(f32, TdfType::Float);

impl Encodable for bool {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_bool(*self)
    }
}

impl Decodable for bool {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_bool()
    }
}

value_type!(bool, TdfType::VarInt);

/// Macro for forwarding the encode and decodes of a type to
/// another types encoder and decoder
///
/// `$a` The type to forward
/// `$b` The type to forward to
macro_rules! forward_codec {
    ($a:ident, $b:ident) => {
        impl Decodable for $a {
            #[inline]
            fn decode(reader: &mut $crate::reader::TdfReader) -> $crate::error::DecodeResult<Self> {
                Ok($b::decode(reader)? as $a)
            }
        }

        impl Encodable for $a {
            #[inline]
            fn encode(&self, output: &mut TdfWriter) {
                $b::encode(&(*self as $b), output)
            }
        }

        impl $crate::codec::ValueType for $a {
            #[inline]
            fn value_type() -> TdfType {
                $b::value_type()
            }
        }
    };
}

// Encoding for u8 values

impl Encodable for u8 {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u8(*self)
    }
}

impl Decodable for u8 {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_u8()
    }
}

impl Encodable for u16 {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u16(*self)
    }
}

impl Decodable for u16 {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_u16()
    }
}

impl Encodable for u32 {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u32(*self)
    }
}

impl Decodable for u32 {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_u32()
    }
}

impl Encodable for u64 {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_u64(*self)
    }
}

impl Decodable for u64 {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_u64()
    }
}

impl Encodable for usize {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_usize(*self)
    }
}

impl Decodable for usize {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_usize()
    }
}

value_type!(u8, TdfType::VarInt);
value_type!(u16, TdfType::VarInt);
value_type!(u32, TdfType::VarInt);
value_type!(u64, TdfType::VarInt);
value_type!(usize, TdfType::VarInt);

forward_codec!(i8, u8);
forward_codec!(i16, u16);
forward_codec!(i32, u32);
forward_codec!(i64, u64);
forward_codec!(isize, usize);

impl Encodable for &'_ str {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_str(self)
    }
}

value_type!(&'_ str, TdfType::String);

impl Encodable for String {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_str(self);
    }
}

impl Decodable for String {
    #[inline]
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        reader.read_string()
    }
}

value_type!(String, TdfType::String);

/// Blob structure wrapping a vec of bytes. This implementation is
/// to differenciate between a list of VarInts and a Blob of straight
/// bytes
#[derive(Default, Debug, Clone)]
pub struct Blob(pub Vec<u8>);

impl Encodable for Blob {
    fn encode(&self, output: &mut TdfWriter) {
        output.write_usize(self.0.len());
        output.write_slice(&self.0);
    }
}

impl Decodable for Blob {
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let length = reader.read_usize()?;
        let bytes = reader.read_slice(length)?;
        Ok(Blob(bytes.to_vec()))
    }
}

value_type!(Blob, TdfType::Blob);

/// Vec List encoding for encodable items items are required
/// to have the ValueType trait in order to write the list header
impl<C> Encodable for Vec<C>
where
    C: Encodable + ValueType,
{
    fn encode(&self, output: &mut TdfWriter) {
        output.write_type(C::value_type());
        output.write_usize(self.len());
        for value in self {
            value.encode(output);
        }
    }
}

impl<C> Decodable for Vec<C>
where
    C: Decodable + ValueType,
{
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let value_type: TdfType = reader.read_type()?;
        let expected_type = C::value_type();
        if value_type != expected_type {
            return Err(DecodeError::InvalidType {
                expected: expected_type,
                actual: value_type,
            });
        }

        let length = reader.read_usize()?;
        let mut values = Vec::with_capacity(length);
        for _ in 0..length {
            values.push(C::decode(reader)?);
        }
        Ok(values)
    }
}

impl<C> ValueType for Vec<C> {
    fn value_type() -> TdfType {
        TdfType::List
    }
}

/// Pair type alias. (Note Pairs should only ever be used with VarInts)
type Pair<A, B> = (A, B);

impl<A, B> Encodable for Pair<A, B>
where
    A: VarInt,
    B: VarInt,
{
    fn encode(&self, output: &mut TdfWriter) {
        self.0.encode(output);
        self.1.encode(output);
    }
}

impl<A, B> Decodable for Pair<A, B>
where
    A: VarInt,
    B: VarInt,
{
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let a = A::decode(reader)?;
        let b = B::decode(reader)?;
        Ok((a, b))
    }
}

impl<A, B> ValueType for Pair<A, B> {
    fn value_type() -> TdfType {
        TdfType::Pair
    }
}

/// Triple type alias. (Note Triples should only ever be used with VarInts)
type Triple<A, B, C> = (A, B, C);

impl<A, B, C> Encodable for Triple<A, B, C>
where
    A: VarInt,
    B: VarInt,
    C: VarInt,
{
    fn encode(&self, output: &mut TdfWriter) {
        self.0.encode(output);
        self.1.encode(output);
        self.2.encode(output);
    }
}
impl<A, B, C> Decodable for Triple<A, B, C>
where
    A: VarInt,
    B: VarInt,
    C: VarInt,
{
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self> {
        let a = A::decode(reader)?;
        let b = B::decode(reader)?;
        let c = C::decode(reader)?;
        Ok((a, b, c))
    }
}

impl<A, B, C> ValueType for Triple<A, B, C> {
    fn value_type() -> TdfType {
        TdfType::Triple
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Decodable, Encodable};
    use crate::reader::TdfReader;
    use crate::types::TdfMap;
    use crate::writer::TdfWriter;

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
    fn test_map_extend() {
        let mut mapa = TdfMap::<String, String>::new();

        mapa.insert("key1", "ABC");
        mapa.insert("key2", "ABC");
        mapa.insert("key4", "ABC");
        mapa.insert("key24", "ABC");
        mapa.insert("key11", "ABC");
        mapa.insert("key17", "ABC");

        let mut mapb = TdfMap::<String, String>::new();

        mapb.insert("key1", "DDD");
        mapb.insert("key2", "ABC");
        mapb.insert("key4", "DDD");
        mapb.insert("abc", "ABC");

        mapa.extend(mapb);
        println!("{mapa:?}")
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
            let mut out = TdfWriter { buffer: Vec::new() };
            value.encode(&mut out);
            let mut reader = TdfReader::new(&out.buffer);
            let v2 = u8::decode(&mut reader).unwrap();
            assert_eq!(value, v2)
        }
    }

    #[test]
    fn test_u16() {
        for value in u16::MIN..u16::MAX {
            let mut out = TdfWriter { buffer: Vec::new() };
            value.encode(&mut out);
            let mut reader = TdfReader::new(&out.buffer);
            let v2 = u16::decode(&mut reader).unwrap();
            assert_eq!(value, v2)
        }
    }
}
