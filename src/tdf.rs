use std::hash::{Hash, Hasher};
use crate::error::{TdfError, TdfResult};
use crate::types::{TdfGroup, TdfList, TdfMap, TdfOptional, VarIntList, VarIntPair, VarIntTriple};

pub struct Tdf {
    name: String,
    value: TdfValue,
}

/// Enum for the different types of Tdf values
#[repr(u8)]
pub enum TdfValueType {
    VarInt = 0x0,
    String = 0x1,
    Blob = 0x2,
    Group = 0x3,
    List = 0x4,
    Map = 0x5,
    Optional = 0x6,
    VarIntList = 0x7,
    Pair = 0x8,
    Tuple = 0x9,
    Float = 0xA,
}

impl TdfValueType {
    fn try_get(value: u8) -> TdfResult<Self> {
        TdfValueType::try_from(value)
            .map_err(|_| TdfError::UnknownType(value))
    }
}

#[derive(Clone)]
pub enum TdfValue {
    VarInt(u64),
    String(String),
    Blob(Vec<u8>),
    Group(TdfGroup),
    List(TdfList),
    Map(TdfMap),
    Optional(TdfOptional),
    VarIntList(VarIntList),
    Pair(VarIntPair),
    Triple(VarIntTriple),
    Float(f32),
}

impl Hash for TdfValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            TdfValue::VarInt(value) => value.hash(state),
            TdfValue::String(value) => value.hash(state),
            _ => state.write_u8(0)
        }
    }
}