use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use byteorder::{BE, ReadBytesExt, WriteBytesExt};
use linked_hash_map::LinkedHashMap;
use crate::error::{EmptyTdfResult, TdfError, TdfResult};
use crate::io::{Readable, TdfRead, TypedReadable, Writable};
use crate::types::{read_byte_array, read_var_int, TdfGroup, TdfList, TdfMap, TdfOptional, VarIntList, VarIntPair, VarIntTriple, write_byte_array, write_var_int};

#[derive(Clone)]
pub struct Tdf {
    pub name: String,
    pub value: TdfValue,
}

impl Tdf {
    /// Function for creating new Tdf value
    pub fn new(name: &str, value: TdfValue) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }

    /// Convert string label into u32 encoded tag
    pub fn label_to_tag(label: &String) -> u32 {
        // Array of output bytes for tag
        let mut output: [u8; 3] = [0, 0, 0];
        // Array of input bytes from label string
        let mut input: [u8; 4] = [0, 0, 0, 0];
        // Takes bytes from label
        {
            let mut bytes = label.bytes();
            for i in 0..4 {
                if let Some(byte) = bytes.next() {
                    input[i] = byte;
                } else {
                    break;
                }
            };
        }

        output[0] |= (input[0] & 0x40) << 1;
        output[0] |= (input[0] & 0x10) << 2;
        output[0] |= (input[0] & 0x0F) << 2;
        output[0] |= (input[1] & 0x40) >> 5;
        output[0] |= (input[1] & 0x10) >> 4;

        output[1] |= (input[1] & 0x0F) << 4;
        output[1] |= (input[2] & 0x40) >> 3;
        output[1] |= (input[2] & 0x10) >> 2;
        output[1] |= (input[2] & 0x0C) >> 2;

        output[2] |= (input[2] & 0x03) << 6;
        output[2] |= (input[3] & 0x40) >> 1;
        output[2] |= input[3] & 0x1F;

        let mut tag: u32 = 0;
        tag |= ((output[0] as u32) << 24) as u32;
        tag |= ((output[1] as u32) << 16) as u32;
        tag |= ((output[2] as u32) << 8) as u32;
        tag
    }

    /// Converts u32 encoded tag back into string
    pub fn tag_to_label(input: &[u8; 3]) -> String {
        let mut output: [u8; 4] = [0, 0, 0, 0];

        output[0] |= (input[0] & 0x80) >> 1;
        output[0] |= (input[0] & 0x40) >> 2;
        output[0] |= (input[0] & 0x30) >> 2;
        output[0] |= (input[0] & 0x0C) >> 2;

        output[1] |= (input[0] & 0x02) << 5;
        output[1] |= (input[0] & 0x01) << 4;
        output[1] |= (input[1] & 0xF0) >> 4;

        output[2] |= (input[1] & 0x08) << 3;
        output[2] |= (input[1] & 0x04) << 2;
        output[2] |= (input[1] & 0x03) << 2;
        output[2] |= (input[2] & 0xC0) >> 6;

        output[3] |= (input[2] & 0x20) << 1;
        output[3] |= input[2] & 0x1F;

        let mut out = String::with_capacity(4);

        for i in 0..4 {
            let value = output[i];
            if value == 0 {
                out.push(' ')
            } else {
                out.push(char::from(value))
            }
        }

        out
    }
}

impl Readable for Tdf {
    fn read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self> where Self: Sized {
        let mut tag: [u8; 3] = [0, 0, 0];
        input.read(&mut tag)?;
        let rtype = TdfValueType::try_read(input)?;
        let name = Tdf::tag_to_label(&tag);
        let value = TdfValue::read(&rtype, input)?;
        Ok(Tdf {
            name,
            value,
        })
    }
}

impl Writable for Tdf {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        let name = &self.name;
        let value = &self.value;
        let tag = Tdf::label_to_tag(name);
        let tdf_type = TdfValueType::from(value);
        out.write_u8((tag >> 24) as u8)?;
        out.write_u8((tag >> 16) as u8)?;
        out.write_u8((tag >> 8) as u8)?;
        tdf_type.write(out)?;
        value.write(out)
    }
}

/// Enum for the different types of Tdf values
#[repr(u8)]
#[derive(Clone)]
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
    Triple = 0x9,
    Float = 0xA,
}

impl PartialEq for TdfValueType {
    fn eq(&self, other: &Self) -> bool {
        let a: u8 = self.into();
        let b: u8 = other.into();
        return a == b;
    }
}

impl Writable for TdfValueType {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        out.write_u8(self.into())?;
        Ok(())
    }
}

impl Into<u8> for &TdfValueType {
    fn into(self) -> u8 {
        match self {
            TdfValueType::VarInt => 0x0,
            TdfValueType::String => 0x1,
            TdfValueType::Blob => 0x2,
            TdfValueType::Group => 0x3,
            TdfValueType::List => 0x4,
            TdfValueType::Map => 0x5,
            TdfValueType::Optional => 0x6,
            TdfValueType::VarIntList => 0x7,
            TdfValueType::Pair => 0x8,
            TdfValueType::Triple => 0x9,
            TdfValueType::Float => 0xA,
        }
    }
}

impl TryFrom<u8> for TdfValueType {
    type Error = TdfError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0x0 => TdfValueType::VarInt,
            0x1 => TdfValueType::String,
            0x2 => TdfValueType::Blob,
            0x3 => TdfValueType::Group,
            0x4 => TdfValueType::List,
            0x5 => TdfValueType::Map,
            0x6 => TdfValueType::Optional,
            0x7 => TdfValueType::VarIntList,
            0x8 => TdfValueType::Pair,
            0x9 => TdfValueType::Triple,
            0xA => TdfValueType::Float,
            value => Err(TdfError::UnknownType(value))?
        })
    }
}

impl TdfValueType {
    fn try_read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self> {
        let value = input.read_u8()?;
        return TdfValueType::try_from(value);
    }
}

#[derive(Clone,PartialEq)]
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

impl TdfValue {
    fn tag(self, tag: &str) -> Tdf {
        Tdf::new(tag.to_string(), self)
    }
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

impl Eq for TdfValue {}

impl From<&TdfValue> for TdfValueType {
    fn from(value: &TdfValue) -> Self {
        match value {
            TdfValue::VarInt(_) => TdfValueType::VarInt,
            TdfValue::String(_) => TdfValueType::String,
            TdfValue::Blob(_) => TdfValueType::Blob,
            TdfValue::Group(_) => TdfValueType::Group,
            TdfValue::List(_) => TdfValueType::List,
            TdfValue::Map(_) => TdfValueType::Map,
            TdfValue::Optional(_) => TdfValueType::Optional,
            TdfValue::VarIntList(_) => TdfValueType::VarIntList,
            TdfValue::Pair(_) => TdfValueType::Pair,
            TdfValue::Triple(_) => TdfValueType::Triple,
            TdfValue::Float(_) => TdfValueType::Float
        }
    }
}

impl Writable for TdfValue {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        match self {
            TdfValue::VarInt(value) => {
                write_var_int(*value, out)?;
            }
            TdfValue::String(value) => {
                let bytes = value.clone().into_bytes();
                write_byte_array(&bytes, out)?;
            }
            TdfValue::Blob(value) => {
                write_byte_array(value, out)?;
            }
            TdfValue::Group(value) => {
                if value.start2 {
                    out.write_u8(2)?;
                }

                for value in &value.values {
                    value.write(out)?;
                }

                out.write_u8(0)?
            }
            TdfValue::List(value) => {
                value.value_type.write(out)?;
                for value in &value.values {
                    value.write(out)?;
                }
            }
            TdfValue::Map(value) => {
                value.key_type.write(out)?;
                value.value_type.write(out)?;

                let map = &value.map;
                write_var_int(map.len() as u64, out)?;

                for entry in map {
                    entry.0.write(out)?;
                    entry.1.write(out)?;
                }
            }
            TdfValue::Optional(value) => {
                out.write_u8(value.value_type)?;
                if let Some(value) = &value.value {
                    value.write(out)?;
                }
            }
            TdfValue::VarIntList(value) => {
                let length = value.len();
                write_var_int(length as u64, out)?;
                for item in value {
                    write_var_int(*item, out)?;
                }
            }
            TdfValue::Pair(value) => {
                value.write(out)?;
            }
            TdfValue::Triple(value) => {
                value.write(out)?;
            }
            TdfValue::Float(value) => {
                out.write_f32::<BE>(*value)?;
            }
        }
        Ok(())
    }
}

impl TypedReadable for TdfValue {
    fn read<R: Read>(rtype: &TdfValueType, input: &mut TdfRead<R>) -> TdfResult<Self> where Self: Sized {
        match rtype {
            TdfValueType::VarInt => {
                let value = read_var_int(input)?;
                Ok(TdfValue::VarInt(value))
            }
            TdfValueType::String => {
                let bytes = read_byte_array(input)?;
                let text = String::from_utf8_lossy(&bytes)
                    .to_string();
                Ok(TdfValue::String(text))
            }
            TdfValueType::Blob => {
                let bytes = read_byte_array(input)?;
                Ok(TdfValue::Blob(bytes))
            }
            TdfValueType::Group => {
                let mut values = Vec::new();
                let mut start2 = false;
                let mut peeked: u8;
                loop {
                    peeked = input.peek()?;
                    if peeked == 0 {
                        break;
                    } else if peeked == 2 {
                        start2 = true;
                    } else {
                        input.revert_peek();
                        let value = Tdf::read(input)?;
                        values.push(value);
                    }
                }
                Ok(TdfValue::Group(TdfGroup { start2, values }))
            }
            TdfValueType::List => {
                let value_type = TdfValueType::try_read(input)?;
                let length = read_var_int(input)? as usize;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    let value = TdfValue::read(&value_type, input)?;
                    values.push(value);
                }
                Ok(TdfValue::List(TdfList { value_type, values }))
            }
            TdfValueType::Map => {
                let key_type = TdfValueType::try_read(input)?;
                let value_type = TdfValueType::try_read(input)?;
                let length = read_var_int(input)? as usize;
                let mut map = LinkedHashMap::with_capacity(length);
                for _ in 0..length {
                    let key = TdfValue::read(&key_type, input)?;
                    let value = TdfValue::read(&value_type, input)?;
                    map.insert(key, value);
                }
                Ok(TdfValue::Map(TdfMap { key_type, value_type, map }))
            }
            TdfValueType::Optional => {
                let value_type = input.read_u8()?;
                let value = if value_type != 0x7F {
                    Some(Box::new(Tdf::read(input)?))
                } else {
                    None
                };
                Ok(TdfValue::Optional(TdfOptional { value_type, value }))
            }
            TdfValueType::VarIntList => {
                let length = read_var_int(input)? as usize;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(read_var_int(input)?);
                }
                Ok(TdfValue::VarIntList(values))
            }
            TdfValueType::Pair => {
                let value = VarIntPair::read(input)?;
                Ok(TdfValue::Pair(value))
            }
            TdfValueType::Triple => {
                let value = VarIntTriple::read(input)?;
                Ok(TdfValue::Triple(value))
            }
            TdfValueType::Float => {
                let value = input.read_f32::<BE>()?;
                Ok(TdfValue::Float(value))
            }
        }
    }
}

impl TdfLookup for Vec<Tdf> {
    fn get_by_tag(&self, tag: &String) -> Option<Tdf> {
        for tdf in self {
            if tdf.name.eq(tag) {
                return Some(tdf.clone());
            }
        }
        None
    }
}

trait TdfLookup {
    fn get_by_tag(&self, tag: &String) -> Option<Tdf>;
}