use std::io::{Bytes, Chain, IoSliceMut, Read, Take, Write};
use byteorder::ReadBytesExt;
use crate::error::TdfError;

pub type TdfResult<T> = Result<T, TdfError>;

pub trait Writable: Send + Sync {
    fn write<W: Write>(&self, out: &mut W) -> TdfResult<()>;
}

pub trait Readable: Send + Sync {
    fn read<R: Read>(input: &mut R) -> TdfResult<Self> where Self: Sized;
}

pub trait TypedReadable: Send + Sync {
    fn read<R: Read>(rtype: u8, input: &mut R) -> TdfResult<Self> where Self: Sized;
}

pub trait ReadWrite: Writable + Readable + Sized {}


pub struct BytePeek<R: Read> {
    inner: R,
    peeked: Option<u8>,
    unpeek: bool,
}

impl<R: Read> BytePeek<R> {
    pub fn new(value: R) -> Self {
        Self {
            inner: value,
            peeked: None,
            unpeek: false,
        }
    }

    pub fn peek(&mut self) -> std::io::Result<u8> {
        let value = self.inner.read_u8()?;
        self.peeked = Some(value.clone());
        Ok(value)
    }

    pub fn unpeek(&mut self) {
        self.unpeek = true
    }
}

impl<R: Read> Read for BytePeek<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.unpeek {
            return self.inner.read(buf);
        }
        if let Some(peeked) = self.peeked {
            let len = buf.len();
            if len > 0 {
                let mut read_count = 1;
                buf[0] = peeked;
                self.peeked = None;
                if len > 1 {
                    let mut inner_bytes = Vec::with_capacity(len);
                    let inner_read_count = self.inner.read(&mut inner_bytes)?;
                    for i in 1..len {
                        if i > inner_read_count {
                            break;
                        }
                        let value = inner_bytes[i - 1];
                        buf[i] = value;
                    }
                }
                Ok(read_count)
            } else {
                Ok(0)
            }
        } else {
            self.inner.read(buf)
        }
    }
}