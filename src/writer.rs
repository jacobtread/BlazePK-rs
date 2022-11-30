use crate::{codec::Encodable, tag::TdfType};

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
        key_type.encode(self);
        value_type.encode(self);
        self.write_usize(length);
    }
}

impl Into<Vec<u8>> for TdfWriter {
    fn into(self) -> Vec<u8> {
        self.buffer
    }
}
