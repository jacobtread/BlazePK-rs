use std::borrow::Cow;

use crate::{
    codec::{Decodable, ValueType},
    error::{DecodeError, DecodeResult},
    tag::TdfType,
    types::TdfMap,
};

/// Buffered readable implementation. Allows reading through the
/// underlying slice using a cursor and with a position that can
/// be saved usin the marker.
pub struct TdfReader<'a> {
    /// The underlying buffer to read from
    pub buffer: &'a [u8],
    /// The cursor position on the buffer. The cursor should not be set
    /// to any arbitry values should only be set to previously know values
    pub cursor: usize,
}

macro_rules! impl_decode_var {
    ($ty:ty, $reader:ident) => {{
        let first: u8 = $reader.read_byte()?;
        let mut result: $ty = (first & 63) as $ty;
        if first < 128 {
            return Ok(result);
        }
        let mut shift: u8 = 6;
        let mut byte: u8;
        loop {
            byte = $reader.read_byte()?;
            result |= ((byte & 127) as $ty) << shift;
            if byte < 128 {
                break;
            }
            shift += 7;
        }
        Ok(result)
    }};
}

impl TdfReader<'_> {
    /// Creates a new reader over the provided slice of bytes with
    /// the default cursor position at zero
    pub fn new(buffer: &[u8]) -> Self {
        Self { buffer, cursor: 0 }
    }

    /// Takes a single byte from the underlying buffer moving
    /// the cursor over by one. Will return UnexpectedEof error
    /// if there are no bytes left
    pub fn read_byte(&mut self) -> DecodeResult<u8> {
        if self.cursor + 1 >= self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: 1,
                remaining: 0,
            });
        }
        let byte: u8 = self.buffer[self.cursor];
        self.cursor += 1;
        Ok(byte)
    }

    /// Attempts to take four bytes from the underlying buffer moving
    /// the cursor over 4 bytes. This is used when decoding tags and
    /// taking floats as they both require 4 bytes. Will return an
    /// UnexpectedEof error if there is not 4 bytes after the cursor
    fn read_byte_4(&mut self) -> DecodeResult<[u8; 4]> {
        // Ensure we have the required number of bytes
        if self.cursor + 4 >= self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: 4,
                remaining: self.len(),
            });
        }
        // Alocate and copy the bytes from the buffer
        let bytes: [u8; 4] = [0u8; 4];
        bytes.copy_from_slice(&self.buffer[self.cursor..self.cursor + 4]);
        // Move the cursor
        self.cursor += 4;
        Ok(bytes)
    }

    /// Takes a slice of the provided length from the portion of the
    /// buffer that is after the cursor position
    ///
    /// `length` The length of the slice to take
    pub fn read_slice(&mut self, length: usize) -> DecodeResult<&[u8]> {
        // Ensure we have the required number of bytes
        if self.cursor + length >= self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: length,
                remaining: self.len(),
            });
        }
        let slice: &[u8] = &self.buffer[self.cursor..self.cursor + length];
        self.cursor += length;
        Ok(slice)
    }

    /// Takes a float value from the buffer which moves the
    /// cursor over by 4 bytes
    pub fn read_f32(&mut self) -> DecodeResult<f32> {
        let bytes: [u8; 4] = self.read_byte_4()?;
        Ok(f32::from_be_bytes(bytes))
    }

    /// Returns the remaining length left after the cursor
    pub fn len(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    /// Decodes a u8 value using the VarInt encoding
    pub fn read_u8(&mut self) -> DecodeResult<u8> {
        let first = self.read_byte()?;
        let mut result = first & 63;
        // Values less than 128 are already complete and don't need more reading
        if first < 128 {
            return Ok(result);
        }

        let byte = self.read_byte()?;
        result |= (byte & 127) << 6;

        // Consume remaining unused VarInt data. We only wanted a u8
        if byte >= 128 {
            while self.cursor < self.buffer.len() {
                let byte = self.buffer[self.cursor];
                if byte < 128 {
                    break;
                }
                self.cursor += 1;
            }
        }
        Ok(result)
    }

    /// Decodes a u16 value using hte VarInt encoding. This uses
    /// the impl_decode_var macro so its implementation is the
    /// same as others
    pub fn read_u16(&mut self) -> DecodeResult<u16> {
        impl_decode_var!(u16, self)
    }

    /// Decodes a u32 value using hte VarInt encoding. This uses
    /// the impl_decode_var macro so its implementation is the
    /// same as others
    pub fn read_u32(&mut self) -> DecodeResult<u32> {
        impl_decode_var!(u32, self)
    }

    /// Decodes a u64 value using hte VarInt encoding. This uses
    /// the impl_decode_var macro so its implementation is the
    /// same as others
    pub fn read_u64(&mut self) -> DecodeResult<u64> {
        impl_decode_var!(u64, self)
    }

    /// Decodes a u64 value using hte VarInt encoding. This uses
    /// the impl_decode_var macro so its implementation is the
    /// same as others
    pub fn read_usize(&mut self) -> DecodeResult<usize> {
        impl_decode_var!(usize, self)
    }

    /// Reads a string from the underlying buffer
    pub fn read_string(&mut self) -> DecodeResult<String> {
        let length: usize = self.read_usize()?;
        let bytes: &[u8] = self.read_slice(length)?;
        let text: Cow<str> = String::from_utf8_lossy(bytes);
        let mut text: String = text.to_string();
        // Remove null terminator
        text.pop();
        Ok(text)
    }

    /// Reads a boolean value this is encoded using the
    /// var int encoding
    pub fn read_bool(&mut self) -> DecodeResult<bool> {
        Ok(match self.read_u8()? {
            1 => true,
            _ => false,
        })
    }

    /// Reads a generic decodable type
    #[inline]
    pub fn read<C: Decodable>(&mut self) -> Decodable<C> {
        C::decode(self)
    }

    /// Reads a map from the underlying buffer
    pub fn read_map<K: Decodable + ValueType, V: Decodable + ValueType>(
        &mut self,
    ) -> DecodeResult<TdfMap<K, V>> {
        let length = self.read_map_header(K::value_type(), V::value_type())?;
        self.read_map_body(length)
    }

    /// Reads a map header from the underlying buffer ensuring that the key
    /// and value types match the provided key and value types. Returns
    /// the length of the following content
    ///
    /// `exp_key_type`   The type of key to expect
    /// `exp_value_type` The type of value to expect
    pub fn read_map_header(
        &mut self,
        exp_key_type: TdfType,
        exp_value_type: TdfType,
    ) -> DecodeResult<usize> {
        let key_type: TdfType = self.read()?;
        if key_type != exp_key_type {
            return Err(DecodeError::InvalidType {
                expected: exp_key_type,
                actual: key_type,
            });
        }
        let value_type: TdfType = self.read();
        if value_type != exp_value_type {
            return Err(DecodeError::InvalidType {
                expected: exp_value_type,
                actual: value_type,
            });
        }
        self.read_usize()
    }

    /// Reads the contents of the map for the provided key value types
    /// and for the provided length
    ///
    /// `length` The length of the map (The number of entries)
    pub fn read_map_body<K: Decodable, V: Decodable>(
        &mut self,
        length: usize,
    ) -> DecodeResult<TdfMap<K, V>> {
        let mut map = TdfMap::with_capacity(length);
        for _ in 0..length {
            let key: K = self.read()?;
            let value: V = self.read()?;
            map.insert(key, value);
        }
        Ok(map)
    }
}
