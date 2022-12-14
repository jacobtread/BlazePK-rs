use crate::error::{DecodeError, DecodeResult};
use std::fmt::Debug;

/// Tag for a Tdf value. This contains the String tag for naming
/// the field and then the type of the field
#[derive(Debug, Eq, PartialEq)]
pub struct Tag(pub String, pub TdfType);

/// Types from the Blaze packet system which are used to describe
/// what data needs to be decoded.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TdfType {
    /// Variable length integer value
    VarInt,
    /// Strings
    String,
    /// List of bytes
    Blob,
    /// Group of tags
    Group,
    /// List of any of the previously mentioned
    List,
    /// Map of TdfType to TdfType
    Map,
    /// Union of value where with unset type
    Union,
    /// List of variable length integers
    VarIntList,
    /// Pair of two var int values
    Pair,
    /// Three var int values
    Triple,
    /// f32 value
    Float,
}

impl TdfType {
    /// Returns the mapped byte value for the type
    pub fn value(&self) -> u8 {
        match self {
            TdfType::VarInt => 0x0,
            TdfType::String => 0x1,
            TdfType::Blob => 0x2,
            TdfType::Group => 0x3,
            TdfType::List => 0x4,
            TdfType::Map => 0x5,
            TdfType::Union => 0x6,
            TdfType::VarIntList => 0x7,
            TdfType::Pair => 0x8,
            TdfType::Triple => 0x9,
            TdfType::Float => 0xA,
        }
    }

    /// Converts the byte value to its actual type returning
    /// an error if the type was unknown
    ///
    /// `value` The value to convert
    pub fn from_value(value: u8) -> DecodeResult<TdfType> {
        Ok(match value {
            0x0 => TdfType::VarInt,
            0x1 => TdfType::String,
            0x2 => TdfType::Blob,
            0x3 => TdfType::Group,
            0x4 => TdfType::List,
            0x5 => TdfType::Map,
            0x6 => TdfType::Union,
            0x7 => TdfType::VarIntList,
            0x8 => TdfType::Pair,
            0x9 => TdfType::Triple,
            0xA => TdfType::Float,
            ty => return Err(DecodeError::UnknownType { ty }),
        })
    }
}
