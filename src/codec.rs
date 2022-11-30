use crate::{error::DecodeResult, reader::TdfReader, tag::TdfType, writer::TdfWriter};
use std::io;

pub trait Decodable: Sized {
    /// Function for implementing decoding of Self from
    /// the provided Reader. Will return None if self
    /// cannot be decoded
    ///
    /// `reader` The reader to decode from
    fn decode(reader: &mut TdfReader) -> DecodeResult<Self>;
}

pub trait Encodable: Sized {
    /// Function for implementing encoding of Self to the
    /// provided vec of bytes
    ///
    /// `output` The output to decode to
    fn encode(&self, writer: &mut TdfWriter);

    /// Shortcut function for encoding self directly to
    /// a Vec of bytes
    fn encode_bytes(&self) -> Vec<u8> {
        let mut output = TdfWriter::default();
        self.encode(&mut output);
        output.into()
    }
}

pub trait ValueType {
    /// The type of tdf value this is
    fn value_type() -> TdfType;
}

#[macro_export]
macro_rules! value_type {
    ($for:ty, $type:expr) => {
        impl $crate::codec::ValueType for $for {
            fn value_type() -> $crate::tag::TdfType {
                $type
            }
        }
    };
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
