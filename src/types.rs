use std::io::{Read, Write};
use byteorder::{ReadBytesExt, WriteBytesExt};
use linked_hash_map::LinkedHashMap;
use crate::error::{EmptyTdfResult, TdfResult};
use crate::io::{Readable, TdfRead, Writable};
use crate::tdf::{Tdf, TdfValue, TdfValueType};

/// Type for variable length integer. Value is encoded to different
/// lengths based on how large it is rather than always taking
/// the same size. This value is represented as a u64
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VarInt(pub u64);

// Type for storing two variable length integers.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VarIntPair(pub u64, pub u64);

// Type for storing three variable length integers
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VarIntTriple(pub u64, pub u64, pub u64);

/// Type for list of variable length integer
pub type VarIntList = Vec<u64>;

/// Represents a group of tdf values that can
/// possibly start with a 2
#[derive(Clone)]
pub struct TdfGroup {
    pub start2: bool,
    pub values: Vec<Tdf>,
}

impl PartialEq for TdfGroup {
    fn eq(&self, _: &Self) -> bool {
        return false
    }
}

// Represents a list of
#[derive(Clone)]
pub struct TdfList {
    pub value_type: TdfValueType,
    pub values: Vec<TdfValue>,
}

impl PartialEq for TdfList {
    fn eq(&self, _: &Self) -> bool {
        return false
    }
}


/// Represents a mapping of tdf value keys to tdf value
/// values
#[derive(Clone, PartialEq)]
pub struct TdfMap {
    pub key_type: TdfValueType,
    pub value_type: TdfValueType,
    pub map: LinkedHashMap<TdfValue, TdfValue>,
}

/// Represents a value that may be present
/// or not depending on value_type
#[derive(Clone)]
pub struct TdfOptional {
    pub value_type: u8,
    pub value: Option<Box<Tdf>>,
}


impl PartialEq for TdfOptional {
    fn eq(&self, _: &Self) -> bool {
        return false
    }
}

/// Implement from VarInt for u64 to convert VarInt to a u64 which
/// is just the value stored inside it
impl From<VarInt> for u64 {
    fn from(value: VarInt) -> Self {
        value.0
    }
}

/// Macro for defining the Into<VarInt> trait for number types
/// used below to create lots of definitions
macro_rules! into_var_int {
    (
        $($ty:ty),*
    ) => {
        $(
            impl Into<VarInt> for $ty {
                fn into(self) -> VarInt {
                    VarInt(self as u64)
                }
            }
        )*
    };
}

into_var_int!(u8, u16, u32, u64, i8, i16, i32, i64, usize);

/// Function for reading variable length integers
pub fn read_var_int<R: Read>(input: &mut R) -> TdfResult<u64> {
    let mut result = {
        let first_byte = input.read_u8()?;
        let value = (first_byte & 63) as u64;
        if first_byte < 128 {
            return Ok(value);
        }
        value
    };
    let mut shift = 6;
    let mut byte: u8;
    loop {
        byte = input.read_u8()?;
        result |= ((byte & 127) as u64) << shift;
        shift += 7;
        if byte < 128 {
            break;
        }
    }
    return Ok(result);
}

pub fn read_byte_array<R: Read>(input: &mut R) -> TdfResult<Vec<u8>> {
    let length = read_var_int(input)? as usize;
    let mut bytes = Vec::with_capacity(length);
    input.read_exact(&mut bytes)?;
    return Ok(bytes);
}

pub fn write_byte_array<W: Write>(value: &[u8], out: &mut W) -> EmptyTdfResult {
    write_var_int(value.len() as u64, out)?;
    out.write(value)?;
    Ok(())
}

/// Function for writing variable length integers
pub fn write_var_int<W: Write>(value: u64, out: &mut W) -> EmptyTdfResult {
    if value < 64 {
        out.write_u8(value as u8)?;
    } else {
        let mut cur_byte = ((value & 63) as u8) | 128;
        out.write_u8(cur_byte as u8)?;
        let mut cur_shift = value >> 6;
        while cur_shift >= 128 {
            cur_byte = ((cur_shift & 127) | 128) as u8;
            cur_shift >>= 7;
            out.write_u8(cur_byte)?;
        }
        out.write_u8(cur_shift as u8)?;
    }
    Ok(())
}

/// Implement reading logic for VarInt
impl Readable for VarInt {
    fn read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self> where Self: Sized {
        let value = read_var_int(input)?;
        Ok(VarInt(value))
    }
}

/// Implement writing logic for VarInt
impl Writable for VarInt {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        write_var_int(self.0, out)
    }
}

/// Implement reading logic for VarInt pair
impl Readable for VarIntPair {
    fn read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self> where Self: Sized {
        let a = read_var_int(input)?;
        let b = read_var_int(input)?;
        Ok(VarIntPair(a, b))
    }
}

/// Implement writing logic for VarInt pair
impl Writable for VarIntPair {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        write_var_int(self.0, out)?;
        write_var_int(self.1, out)
    }
}

/// Implement reading logic for VarInt triple
impl Readable for VarIntTriple {
    fn read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self> where Self: Sized {
        let a = read_var_int(input)?;
        let b = read_var_int(input)?;
        let c = read_var_int(input)?;
        Ok(VarIntTriple(a, b, c))
    }
}

/// Implement writing logic for VarInt triple
impl Writable for VarIntTriple {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        write_var_int(self.0, out)?;
        write_var_int(self.1, out)?;
        write_var_int(self.2, out)
    }
}
