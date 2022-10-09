use std::fmt::Debug;
use std::io;
use std::io::Read;
use std::thread::current;

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
    pub fn take(&mut self, count: usize) -> Option<&[u8]> {
        if self.remaining() < count {
            return None;
        }

        let current = self.cursor;
        self.cursor += count;
        Some(&self.buffer[current..current + count])
    }

    /// Takes a single bytes from the reader increasing
    /// the cursor by one.
    pub fn take_one(&mut self) -> Option<u8> {
        if self.remaining() < 1 {
            return None;
        }
        let byte = self.buffer[self.cursor];
        self.cursor += 1;
        Some(byte)
    }

    /// Attempts to take a slice with the provided
    /// number of bytes and create a reader from it
    pub fn slice(&mut self, count: usize) -> Option<Reader> {
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
pub trait Codec: Debug + Sized {
    /// Function for implementing encoding of Self to the
    /// provided vec of bytes
    fn encode(&self, output: &mut Vec<u8>);

    /// Function for implementing decoding of Self from
    /// the provided Reader. Will return None if self
    /// cannot be decoded
    fn decode(reader: &mut Reader) -> Option<Self>;

    /// Shortcut function for encoding self directly to
    /// a Vec of bytes
    fn encode_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        self.encode(&mut output);
        output
    }

    /// Shortcut function for decoding self directly
    /// from a slice of bytes.
    fn decode_from(input: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(input);
        Self::decode(&mut reader)
    }
}

impl Codec for u16 {
    fn encode(&self, output: &mut Vec<u8>) {
        let bytes: [u8; 2] = self.to_be_bytes();
        output.extend_from_slice(&bytes);
    }

    fn decode(reader: &mut Reader) -> Option<Self> {
        let bytes = reader.take(2)?;
        Some(u16::from_be_bytes(bytes.try_into().ok()?))
    }
}

pub trait ReadBytesExt: Read {
    #[inline]
    fn read_u16(&mut self) -> io::Result<u16> {
        let mut buffer = [0; 2];
        self.read_exact(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }
}

impl<W: Read> ReadBytesExt for W {}
