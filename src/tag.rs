//! Implementation for [`Tag`]s and [`TdfType`]s

use crate::error::DecodeError;
use std::fmt::Debug;

/// Tag for a Tdf value. This contains the String tag for naming
/// the field and then the type of the field
#[derive(Debug, Eq, PartialEq)]
pub struct Tag(pub String, pub TdfType);

/// Types from the Blaze packet system which are used to describe
/// what data needs to be decoded.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum TdfType {
    /// Variable length integer value
    VarInt = 0x0,
    /// Strings
    String = 0x1,
    /// List of bytes
    Blob = 0x2,
    /// Group of tags
    Group = 0x3,
    /// List of any of the previously mentioned
    List = 0x4,
    /// Map of TdfType to TdfType
    Map = 0x5,
    /// Union of value where with unset type
    Union = 0x6,
    /// List of variable length integers
    VarIntList = 0x7,
    /// Pair of two var int values
    Pair = 0x8,
    /// Three var int values
    Triple = 0x9,
    /// f32 value
    Float = 0xA,
}

/// Convert bytes back to tdf types
impl TryFrom<u8> for TdfType {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
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
