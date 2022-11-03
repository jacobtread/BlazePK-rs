use std::collections::HashMap;

use crate::{encode_str, Codec, MapKey, Tag, ValueType, VarInt, EMPTY_OPTIONAL};

// Writing Tags

#[inline]
pub fn tag_bool(output: &mut Vec<u8>, tag: &str, value: bool) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    value.encode(output);
}

#[inline]
pub fn tag_zero(output: &mut Vec<u8>, tag: &str) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    output.push(0);
}

#[inline]
pub fn tag_u8(output: &mut Vec<u8>, tag: &str, value: u8) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    value.encode(output);
}

#[inline]
pub fn tag_u16(output: &mut Vec<u8>, tag: &str, value: u16) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    value.encode(output);
}

#[inline]
pub fn tag_u32(output: &mut Vec<u8>, tag: &str, value: u32) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    value.encode(output);
}

#[inline]
pub fn tag_usize(output: &mut Vec<u8>, tag: &str, value: usize) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    value.encode(output);
}

#[inline]
pub fn tag_u64(output: &mut Vec<u8>, tag: &str, value: u64) {
    Tag::encode_from(tag, &ValueType::VarInt, output);
    value.encode(output);
}

#[inline]
pub fn tag_empty_str(output: &mut Vec<u8>, tag: &str) {
    Tag::encode_from(tag, &ValueType::String, output);
    encode_empty_str(output);
}

#[inline]
pub fn tag_empty_blob(output: &mut Vec<u8>, tag: &str) {
    Tag::encode_from(tag, &ValueType::Blob, output);
    output.push(0);
}

#[inline]
pub fn tag_str(output: &mut Vec<u8>, tag: &str, value: &str) {
    Tag::encode_from(tag, &ValueType::String, output);
    encode_str(value, output);
}

#[inline]
pub fn encode_empty_str(output: &mut Vec<u8>) {
    output.push(1);
    output.push(0);
}

#[inline]
pub fn tag_group_start(output: &mut Vec<u8>, tag: &str) {
    Tag::encode_from(tag, &ValueType::Group, output);
}

#[inline]
pub fn tag_start(output: &mut Vec<u8>, tag: &str, ty: ValueType) {
    Tag::encode_from(tag, &ty, output);
}

#[inline]
pub fn tag_value<T: Codec>(output: &mut Vec<u8>, tag: &str, value: &T) {
    Tag::encode_from(tag, &T::value_type(), output);
    T::encode(value, output);
}

#[inline]
pub fn tag_list_start(output: &mut Vec<u8>, tag: &str, ty: ValueType, len: usize) {
    Tag::encode_from(tag, &ValueType::List, output);
    ty.encode(output);
    len.encode(output);
}

#[inline]
pub fn tag_optional_start(output: &mut Vec<u8>, tag: &str, ty: u8) {
    Tag::encode_from(tag, &ValueType::Optional, output);
    output.push(ty);
}

#[inline]
pub fn tag_optional_none(output: &mut Vec<u8>, tag: &str) {
    Tag::encode_from(tag, &ValueType::Optional, output);
    output.push(EMPTY_OPTIONAL);
}

#[inline]
pub fn tag_list<T: Codec>(output: &mut Vec<u8>, tag: &str, value: Vec<T>) {
    Tag::encode_from(tag, &ValueType::List, output);
    value.encode(output);
}

#[inline]
pub fn tag_list_empty(output: &mut Vec<u8>, tag: &str, ty: ValueType) {
    Tag::encode_from(tag, &ValueType::List, output);
    ty.encode(output);
    output.push(0);
}

#[inline]
pub fn tag_var_int_list_empty(output: &mut Vec<u8>, tag: &str) {
    Tag::encode_from(tag, &ValueType::VarIntList, output);
    output.push(0);
}

#[inline]
pub fn tag_var_int_list<T: VarInt>(output: &mut Vec<u8>, tag: &str, values: Vec<T>) {
    Tag::encode_from(tag, &ValueType::VarIntList, output);
    values.len().encode(output);
    for value in values {
        value.encode(output);
    }
}

pub fn tag_map_start(
    output: &mut Vec<u8>,
    tag: &str,
    key: ValueType,
    value: ValueType,
    len: usize,
) {
    Tag::encode_from(tag, &ValueType::Map, output);
    key.encode(output);
    value.encode(output);
    len.encode(output);
}

#[inline]
pub fn map_value(output: &mut Vec<u8>, key: impl MapKey, value: impl Codec) {
    key.encode(output);
    value.encode(output);
}

pub fn tag_map<K: MapKey, V: Codec>(output: &mut Vec<u8>, tag: &str, value: &HashMap<K, V>) {
    Tag::encode_from(tag, &ValueType::Map, output);
    K::value_type().encode(output);
    V::value_type().encode(output);
    value.len().encode(output);
    for (key, value) in value {
        key.encode(output);
        value.encode(output);
    }
}

#[inline]
pub fn tag_group_end(output: &mut Vec<u8>) {
    output.push(0)
}

#[inline]
pub fn tag_triple<A: VarInt, B: VarInt, C: VarInt>(
    output: &mut Vec<u8>,
    tag: &str,
    value: &(A, B, C),
) {
    Tag::encode_from(tag, &ValueType::Triple, output);
    value.encode(output);
}

#[inline]
pub fn tag_pair<A: VarInt, B: VarInt>(output: &mut Vec<u8>, tag: &str, value: &(A, B)) {
    Tag::encode_from(tag, &ValueType::Pair, output);
    value.encode(output);
}
