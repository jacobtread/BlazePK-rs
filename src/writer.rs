use crate::{
    codec::{Encodable, ValueType},
    tag::TdfType,
    types::{VarInt, UNION_UNSET},
};

/// Writer implementation for writing values to an underlying buffer
#[derive(Default)]
pub struct TdfWriter {
    /// The buffer that will be written to
    pub buffer: Vec<u8>,
}

macro_rules! impl_encode_var {
    ($value:ident, $output:ident) => {
        if $value < 64 {
            $output.write_byte($value as u8);
            return;
        }
        let mut byte: u8 = (($value & 63) as u8) | 128;
        $output.write_byte(byte);
        let mut cur_shift = $value >> 6;
        while cur_shift >= 128 {
            byte = ((cur_shift & 127) | 128) as u8;
            cur_shift >>= 7;
            $output.write_byte(byte);
        }
        $output.write_byte(cur_shift as u8)
    };
}

impl TdfWriter {
    /// Writes a single byte to the underlying buffer. This just
    /// appends the byte to the buffer.
    ///
    /// `value` The value to write
    #[inline]
    pub fn write_byte(&mut self, value: u8) {
        self.buffer.push(value)
    }

    /// Extends the underlying buffer with the provided slice
    /// value.
    ///
    /// `value` The slice value to write
    #[inline]
    pub fn write_slice(&mut self, value: &[u8]) {
        self.buffer.extend_from_slice(value);
    }

    #[inline]
    pub fn write_type(&mut self, ty: TdfType) {
        self.write_byte(ty.value());
    }

    /// Writes a tag vvalue to the underlying buffer
    ///
    /// `tag`        The tag bytes to write
    /// `value_type` The value type for the tag
    pub fn tag(&mut self, tag: &[u8], value_type: TdfType) {
        let mut output: [u8; 4] = [0, 0, 0, value_type.value()];
        let length: usize = tag.len();
        if length > 0 {
            output[0] |= (tag[0] & 0x40) << 1;
            output[0] |= (tag[0] & 0x10) << 2;
            output[0] |= (tag[0] & 0x0F) << 2;
        }
        if length > 1 {
            output[0] |= (tag[1] & 0x40) >> 5;
            output[0] |= (tag[1] & 0x10) >> 4;
            output[1] |= (tag[1] & 0x0F) << 4;
        }
        if length > 2 {
            output[1] |= (tag[2] & 0x40) >> 3;
            output[1] |= (tag[2] & 0x10) >> 2;
            output[1] |= (tag[2] & 0x0C) >> 2;
            output[2] |= (tag[2] & 0x03) << 6;
        }
        if length > 3 {
            output[2] |= (tag[3] & 0x40) >> 1;
            output[2] |= tag[3] & 0x1F;
        }
        self.buffer.extend_from_slice(&output);
    }

    #[inline]
    pub fn tag_bool(&mut self, tag: &[u8], value: bool) {
        self.tag(tag, TdfType::VarInt);
        self.write_bool(value);
    }

    #[inline]
    pub fn tag_zero(&mut self, tag: &[u8]) {
        self.tag(tag, TdfType::VarInt);
        self.write_byte(0);
    }

    #[inline]
    pub fn tag_u8(&mut self, tag: &[u8], value: u8) {
        self.tag(tag, TdfType::VarInt);
        self.write_u8(value);
    }

    #[inline]
    pub fn tag_u16(&mut self, tag: &[u8], value: u16) {
        self.tag(tag, TdfType::VarInt);
        self.write_u16(value);
    }

    #[inline]
    pub fn tag_u32(&mut self, tag: &[u8], value: u32) {
        self.tag(tag, TdfType::VarInt);
        self.write_u32(value);
    }

    #[inline]
    pub fn tag_u64(&mut self, tag: &[u8], value: u64) {
        self.tag(tag, TdfType::VarInt);
        self.write_u64(value);
    }

    #[inline]
    pub fn tag_usize(&mut self, tag: &[u8], value: usize) {
        self.tag(tag, TdfType::VarInt);
        self.write_usize(value);
    }

    #[inline]
    pub fn tag_str_empty(&mut self, tag: &[u8]) {
        self.tag(tag, TdfType::String);
        self.write_empty_str();
    }

    #[inline]
    pub fn tag_empty_blob(&mut self, tag: &[u8]) {
        self.tag(tag, TdfType::Blob);
        self.buffer.push(0);
    }

    #[inline]
    pub fn tag_str(&mut self, tag: &[u8], value: &str) {
        self.tag(tag, TdfType::String);
        self.write_str(value)
    }

    #[inline]
    pub fn tag_group(&mut self, tag: &[u8]) {
        self.tag(tag, TdfType::Group);
    }

    #[inline]
    pub fn tag_group_end(&mut self) {
        self.buffer.push(0)
    }

    pub fn tag_list_start(&mut self, tag: &[u8], ty: TdfType, length: usize) {
        self.tag(tag, TdfType::List);
        self.write_type(ty);
        self.write_usize(length);
    }

    #[inline]
    pub fn tag_union_start(&mut self, tag: &[u8], key: u8) {
        self.tag(tag, TdfType::Union);
        self.buffer.push(key);
    }

    pub fn tag_union_value<C: Encodable + ValueType>(
        &mut self,
        tag: &[u8],
        key: u8,
        value_tag: &[u8],
        value: C,
    ) {
        self.tag_union_start(tag, key);
        self.tag(value_tag, C::value_type());
        value.encode(self);
    }

    #[inline]
    pub fn tag_union_unset(&mut self, tag: &[u8]) {
        self.tag_union_start(tag, UNION_UNSET);
    }

    #[inline]
    pub fn tag_value<C: Encodable + ValueType>(&mut self, tag: &[u8], value: &C) {
        self.tag(tag, C::value_type());
        value.encode(self);
    }

    #[inline]
    pub fn tag_list_empty(&mut self, tag: &[u8], ty: TdfType) {
        self.tag(tag, TdfType::List);
        self.write_type(ty);
        self.buffer.push(0);
    }

    #[inline]
    pub fn tag_var_int_list_empty(&mut self, tag: &[u8]) {
        self.tag(tag, TdfType::VarIntList);
        self.buffer.push(0);
    }

    pub fn tag_map_start(&mut self, tag: &[u8], key: TdfType, value: TdfType, length: usize) {
        self.tag(tag, TdfType::Map);
        self.write_type(key);
        self.write_type(value);
        self.write_usize(length);
    }

    #[inline]
    pub fn tag_pair<A, B>(&mut self, tag: &[u8], value: (A, B))
    where
        A: VarInt,
        B: VarInt,
    {
        self.tag(tag, TdfType::Pair);
        value.encode(self);
    }

    #[inline]
    pub fn tag_triple<A, B, C>(&mut self, tag: &[u8], value: (A, B, C))
    where
        A: VarInt,
        B: VarInt,
        C: VarInt,
    {
        self.tag(tag, TdfType::Triple);
        value.encode(self);
    }

    #[inline]
    pub fn write_empty_str(&mut self) {
        self.buffer.extend_from_slice(&[1, 0])
    }

    /// Writes 32 bit float value to the underlying buffer in
    /// big-endian byte order.
    ///
    /// `value` The float value to write
    pub fn write_f32(&mut self, value: f32) {
        let bytes: [u8; 4] = value.to_be_bytes();
        self.buffer.extend_from_slice(&bytes);
    }

    /// Writes a u8 value using the VarInt encoding
    ///
    /// `value` The value to write
    pub fn write_u8(&mut self, value: u8) {
        // Values < 64 are directly appended to buffer
        if value < 64 {
            self.buffer.push(value);
            return;
        }
        self.buffer.push((value & 63) | 128);
        self.buffer.push(value >> 6);
    }

    /// Writes a u16 value using the VarInt encoding
    ///
    /// `value` The value to write
    pub fn write_u16(&mut self, value: u16) {
        if value < 64 {
            self.buffer.push(value as u8);
            return;
        }
        let mut byte: u8 = ((value & 63) as u8) | 128;
        let mut shift: u16 = value >> 6;
        self.buffer.push(byte);
        byte = ((shift & 127) | 128) as u8;
        shift >>= 7;
        self.buffer.push(byte);
        self.buffer.push(shift as u8);
    }

    /// Writes a u32 value using the VarInt encoding
    ///
    /// `value` The value to write
    pub fn write_u32(&mut self, value: u32) {
        impl_encode_var!(value, self);
    }

    /// Writes a u64 value using the VarInt encoding
    ///
    /// `value` The value to write
    pub fn write_u64(&mut self, value: u64) {
        impl_encode_var!(value, self);
    }

    /// Writes a usize value using the VarInt encoding
    ///
    /// `value` The value to write
    pub fn write_usize(&mut self, value: usize) {
        impl_encode_var!(value, self);
    }

    /// Writes a string to the underlying buffer. The bytes
    /// are encoded an a null terminator is appended to the
    /// end then the size and bytes are written to the buffer
    ///
    /// `value` The string value to write
    pub fn write_str(&mut self, value: &str) {
        let mut bytes = value.as_bytes().to_vec();
        match bytes.last() {
            // Ignore if already null terminated
            Some(0) => {}
            // Null terminate
            _ => bytes.push(0),
        }

        self.write_usize(bytes.len());
        self.write_slice(&bytes);
    }

    /// Writes a boolean value which uses the VarInt encoding
    /// except because the values are < 64 they are just directly
    /// appended as bytes
    pub fn write_bool(&mut self, value: bool) {
        match value {
            false => self.buffer.push(0),
            true => self.buffer.push(1),
        }
    }

    /// Writes the header for a map in order to begin writing map values
    ///
    /// `key_type`   The type of the map keys
    /// `value_type` The type of the map values
    /// `length`     The total number of items that will be written
    pub fn write_map_header(&mut self, key_type: TdfType, value_type: TdfType, length: usize) {
        self.write_type(key_type);
        self.write_type(value_type);
        self.write_usize(length);
    }
}

impl Into<Vec<u8>> for TdfWriter {
    fn into(self) -> Vec<u8> {
        self.buffer
    }
}