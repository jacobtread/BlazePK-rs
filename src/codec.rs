use crate::{error::DecodeResult, tag::TdfType};
use std::{
    fmt::{Debug, Formatter},
    io::{self, Read},
};

/// Structure for reading over a vec
/// of bytes using a cursor.
pub struct Reader<'a> {
    buffer: &'a [u8],
    cursor: usize,
    marker: usize,
}

impl<'a> Reader<'a> {
    /// Creates a new reader for the provided buffer
    pub fn new(buffer: &[u8]) -> Reader {
        Reader {
            buffer,
            cursor: 0,
            marker: 0,
        }
    }

    /// Attempts to take a slice of the buffer after
    /// the cursor with the provided `count` number
    /// of bytes. Returns None if theres not enough
    /// bytes after the cursor
    pub fn take(&mut self, count: usize) -> CodecResult<&[u8]> {
        if self.remaining() < count {
            return Err(CodecError::NotEnoughBytes(
                self.cursor,
                count,
                self.remaining(),
            ));
        }

        let current = self.cursor;
        self.cursor += count;
        Ok(&self.buffer[current..current + count])
    }

    /// Takes a single bytes from the reader increasing
    /// the cursor by one.
    pub fn take_one(&mut self) -> CodecResult<u8> {
        if self.remaining() < 1 {
            return Err(CodecError::NotEnoughBytes(self.cursor, 1, 0));
        }
        let byte = self.buffer[self.cursor];
        self.cursor += 1;
        Ok(byte)
    }

    /// Consumes every byte until the condition function fails
    pub fn consume_while<F: Fn(u8) -> bool>(&mut self, test: F) {
        while self.cursor < self.buffer.len() {
            let byte = self.buffer[self.cursor];
            if !test(byte) {
                break;
            }
            self.cursor += 1;
        }
    }

    /// Step back a cursor position
    pub fn step_back(&mut self) {
        self.cursor -= 1;
    }

    /// Returns the number of bytes remaining after
    /// the cursor
    pub fn remaining(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    pub fn mark(&mut self) {
        self.marker = self.cursor;
    }

    pub fn reset_marker(&mut self) {
        self.cursor = self.marker;
    }
}

pub trait Decodable: Sized {
    /// Function for implementing decoding of Self from
    /// the provided Reader. Will return None if self
    /// cannot be decoded
    ///
    /// `reader` The reader to decode from
    fn decode(reader: &mut Reader) -> DecodeResult<Self>;

    /// Function to provide functionality for skipping this
    /// data type (e.g. read the bytes without using them)
    ///
    /// Default implementation reads discarding the value.
    /// Other implementations should implement a more
    /// performant version
    ///
    /// `reader` The reader to skip with
    fn skip(reader: &mut Reader) -> DecodeResult<()> {
        let _ = Self::decode(reader)?;
    }
}

pub trait Encodable: Sized {
    /// Function for implementing encoding of Self to the
    /// provided vec of bytes
    ///
    /// `output` The output to decode to
    fn encode(&self, output: &mut Vec<u8>);

    /// Shortcut function for encoding self directly to
    /// a Vec of bytes
    fn encode_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        self.encode(&mut output);
        output
    }
}

pub trait ValueType {
    /// The type of tdf value this is
    fn value_type() -> TdfType;
}

/// Attempts to decode a u16 value from the provided slice
pub fn decode_u16_be(value: &[u8]) -> io::Result<u16> {
    Ok(u16::from_be_bytes(value.try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to fit u16 bytes into u16",
        )
    })?))
}

/// Encodes the provided u16 value to bytes and extends
/// the output slice with the bytes
pub fn encode_u16_be(value: &u16, output: &mut Vec<u8>) {
    let bytes = value.to_be_bytes();
    output.extend_from_slice(&bytes);
}

#[cfg(test)]
mod test {
    use crate::codec::Reader;

    #[test]
    pub fn test_reader_take_one() {
        let bytes = [0, 15, 23, 5, 10, 0];
        let mut reader = Reader::new(&bytes);

        for i in 0..bytes.len() {
            let byte = bytes[i];
            let got = reader.take_one().unwrap();
            assert_eq!(byte, got)
        }
    }
}
