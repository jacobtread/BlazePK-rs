use crate::error::{DecodeError, DecodeResult};

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

impl TdfReader<'_> {
    pub fn new(buffer: &[u8]) -> Self {
        Self { buffer, cursor: 0 }
    }

    /// Takes a single byte from the underlying buffer moving
    /// the cursor over by one. Will return UnexpectedEof error
    /// if there are no bytes left
    pub fn take_byte(&mut self) -> DecodeResult<u8> {
        if self.cursor + 1 >= self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: 1,
                remaining: 0,
            });
        }
        let byte = self.buffer[self.cursor];
        self.cursor += 1;
        Ok(byte)
    }

    /// Attempts to take four bytes from the underlying buffer moving
    /// the cursor over 4 bytes. This is used when decoding tags and
    /// taking floats as they both require 4 bytes. Will return an
    /// UnexpectedEof error if there is not 4 bytes after the cursor
    pub fn take_bytes_4(&mut self) -> DecodeResult<[u8; 4]> {
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
    pub fn take_slice(&mut self, length: usize) -> DecodeResult<&[u8]> {}

    /// Takes a float value from the buffer which moves the
    /// cursor over by 4 bytes
    pub fn take_f32(&mut self) -> DecodeResult<f32> {
        let bytes: [u8; 4] = self.take_bytes_4()?;
        Ok(f32::from_be_bytes(bytes))
    }

    /// Returns the remaining length left after the cursor
    pub fn len(&self) -> usize {
        self.buffer.len() - self.cursor
    }
}
