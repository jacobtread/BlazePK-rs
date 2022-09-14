use std::io::{Cursor, Read, Write};
use byteorder::ReadBytesExt;
use crate::error::{EmptyTdfResult, TdfResult};
use crate::tdf::TdfValueType;
/// Trait for something that can be written to the provided output
pub trait Writable: Send + Sync {
    /// Function which handles writing self to the out
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult;
}

/// Trait for something that can be read from the input
pub trait Readable: Send + Sync {
    /// Function for reading self from the input
    fn read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self>
        where Self: Sized;
}

/// Trait for something that can be read from the input using its
/// provided type value
pub trait TypedReadable: Send + Sync {
    /// Function for reading self from the input
    fn read<R: Read>(rtype: &TdfValueType, input: &mut TdfRead<R>) -> TdfResult<Self>
        where Self: Sized;
}

/// Struct which wraps buffer providing functions to read ahead
/// and look at the next byte without consuming it.
pub struct TdfRead<R> {
    inner: R,
    peeked: Option<u8>,
    reverted: bool,
}

impl<V> TdfRead<Cursor<V>>  {
    pub fn position(&mut self) -> u64 {
        self.inner.position()
    }
}

impl<R: Read> TdfRead<R> {
    pub fn new(value: R) -> Self {
        Self {
            inner: value,
            peeked: None,
            reverted: false,
        }
    }

    /// Function for obtaining the next byte and setting the peek state
    pub fn peek(&mut self) -> std::io::Result<u8> {
        self.reverted = false;
        let value = self.inner.read_u8()?;
        self.peeked = Some(value.clone());
        Ok(value)
    }

    /// Function for telling the next read operation to read the peeked value
    pub fn revert_peek(&mut self) {
        self.reverted = true;
    }

    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for TdfRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.reverted {
            if let Some(peeked) = self.peeked.take() {
                let buf_len = buf.len();
                // Ignore buffers that have no length
                if buf_len < 1 {
                    return Ok(0);
                }
                let mut read_count = 1;
                // Set the first item in the buffer to the peeked value
                buf[0] = peeked;

                // If the buffer still has more capacity
                if buf_len > 1 {
                    // Read the remaining length from the inner buffer
                    let mut byte_buff = vec![0u8; (buf_len - 1)];
                    let inner_read_count = self.inner.read(&mut byte_buff)?;
                    for i in 0..inner_read_count {
                        let value = byte_buff[i];
                        buf[i + 1] = value
                    }
                    // Increase the read count by the amount read
                    read_count += inner_read_count;
                }
                return Ok(read_count)
            }
        }

        // Finally just read from the inner
        return self.inner.read(buf);
    }
}