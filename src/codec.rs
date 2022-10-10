use crate::tdf::ValueType;
use derive_more::Display;
use std::fmt::Debug;
use std::io;

/// Structure for reading over a vec
/// of bytes using a cursor.
pub struct Reader<'a> {
    buffer: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    /// Creates a new reader for the provided buffer
    pub fn new(buffer: &[u8]) -> Reader {
        Reader { buffer, cursor: 0 }
    }

    /// Takes a slice of all bytes remaining after
    /// the cursor moving the cursor to the end
    /// of the buffer.
    pub fn take_all(&mut self) -> &[u8] {
        let rest = &self.buffer[self.cursor..];
        self.cursor = self.buffer.len();
        rest
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

    /// Step back a cursor position
    pub fn step_back(&mut self) {
        self.cursor -= 1;
    }

    /// Attempts to take a slice with the provided
    /// number of bytes and create a reader from it
    pub fn slice(&mut self, count: usize) -> CodecResult<Reader> {
        self.take(count).map(Reader::new)
    }

    /// Returns whether there are any bytes remaining
    /// after the cursor
    pub fn has_remaining(&self) -> bool {
        self.cursor < self.buffer.len()
    }

    /// Returns the number of bytes remaining after
    /// the cursor
    pub fn remaining(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    /// Returns the cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

/// Trait for implementing things that can be decoded from
/// a Reader and encoded to a byte Vec
pub trait Codec: Sized {
    /// Function for implementing encoding of Self to the
    /// provided vec of bytes
    fn encode(&self, output: &mut Vec<u8>);

    /// Function for implementing decoding of Self from
    /// the provided Reader. Will return None if self
    /// cannot be decoded
    fn decode(reader: &mut Reader) -> CodecResult<Self>;

    /// Function to provide functionality for skipping this
    /// data type (e.g. read the bytes without using them)
    fn skip(reader: &mut Reader) -> CodecResult<()> {
        Self::decode(reader)?;
        Ok(())
    }

    /// Optional additional specifier for Tdf types that
    /// tells which type this is
    #[inline]
    fn value_type() -> ValueType {
        ValueType::Unknown(0)
    }

    /// Shortcut function for encoding self directly to
    /// a Vec of bytes
    fn encode_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        self.encode(&mut output);
        output
    }

    /// Shortcut function for decoding self directly
    /// from a slice of bytes.
    fn decode_from(input: &[u8]) -> CodecResult<Self> {
        let mut reader = Reader::new(input);
        Self::decode(&mut reader)
    }
}

/// Errors for when decoding packet structures
#[derive(Debug, Display)]
pub enum CodecError {
    #[display(fmt = "Missing field {}", _0)]
    MissingField(&'static str),
    #[display(fmt = "Unable to decode field {}", _0)]
    DecodeFail(&'static str, Box<CodecError>),
    #[display(fmt = "Unexpected type; expected {} but got {}", _0, _1)]
    UnexpectedType(ValueType, ValueType),
    #[display(
        fmt = "Unexpected type for field {} expected {} but got {}",
        _0,
        _1,
        _2
    )]
    UnexpectedFieldType(&'static str, ValueType, ValueType),
    #[display(
        fmt = "Didn't have enough bytes (cursor: {}, wanted: {}, remaining: {})",
        _0,
        _1,
        _2
    )]
    NotEnoughBytes(usize, usize, usize),
    #[display(fmt = "Unknown error occurred when trying to fit bytes")]
    UnknownError,
    #[display(fmt = "Attempted to decode packet contents twice")]
    DecodedTwice,
}

pub type CodecResult<T> = Result<T, CodecError>;

impl Codec for u16 {
    fn encode(&self, output: &mut Vec<u8>) {
        let bytes: [u8; 2] = self.to_be_bytes();
        output.extend_from_slice(&bytes);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let bytes = reader.take(2)?;
        Ok(u16::from_be_bytes(
            bytes.try_into().map_err(|_| CodecError::UnknownError)?,
        ))
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

pub trait ReadBytesExt: io::Read {
    #[inline]
    fn read_u16(&mut self) -> io::Result<u16> {
        let mut buffer = [0; 2];
        self.read_exact(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }
}

impl<R: io::Read> ReadBytesExt for R {}
