use std::io::{Read, Write};
use byteorder::{BE, ReadBytesExt, WriteBytesExt};
use crate::error::TdfError;
use crate::io::{BytePeek, Readable, TdfResult, TypedReadable, Writable};

#[derive(Clone)]
pub struct Tdf(String, TdfType);

impl Tdf {
    const VAR_INT_TYPE: u8 = 0x0;
    const STRING_TYPE: u8 = 0x1;
    const BLOB_TYPE: u8 = 0x2;
    const GROUP_TYPE: u8 = 0x3;
    const LIST_TYPE: u8 = 0x4;
    const MAP_TYPE: u8 = 0x5;
    const OPTIONAL_TYPE: u8 = 0x6;
    const INT_LIST_TYPE: u8 = 0x7;
    const PAIR_TYPE: u8 = 0x8;
    const TRIPLE_TYPE: u8 = 0x9;
    const FLOAT_TYPE: u8 = 0xA;

    const OPTIONAL_NO_VALUE: u8 = 0x7F;

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
        output[2] |= (input[3] & 0x1F);

        let mut tag: u32 = 0;
        tag |= (output[0] << 24) as u32;
        tag |= (output[1] << 16) as u32;
        tag |= (output[2] << 8) as u32;
        tag
    }

    /// Converts u32 encoded tag back into string
    pub fn tag_to_label(input: &[u8; 3]) -> String {
        let mut output: [u8; 4] = [0, 0, 0, 0];

        output[0] |= ((input[0] & 0x80) >> 1);
        output[0] |= ((input[0] & 0x40) >> 2);
        output[0] |= ((input[0] & 0x30) >> 2);
        output[0] |= ((input[0] & 0x0C) >> 2);

        output[1] |= ((input[0] & 0x02) << 5);
        output[1] |= ((input[0] & 0x01) << 4);
        output[1] |= ((input[1] & 0xF0) >> 4);

        output[2] |= ((input[1] & 0x08) << 3);
        output[2] |= ((input[1] & 0x04) << 2);
        output[2] |= ((input[1] & 0x03) << 2);
        output[2] |= ((input[2] & 0xC0) >> 6);

        output[3] |= ((input[2] & 0x20) << 1);
        output[3] |= (input[2] & 0x1F);

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

impl Writable for Tdf {
    fn write<W: Write>(&self, out: &mut W) -> TdfResult<()> {
        let value = &self.1;
        let tag = Tdf::label_to_tag(&self.0);
        let tdf_type = u8::from(value);
        out.write_u8((tag >> 24) as u8)?;
        out.write_u8((tag >> 16) as u8)?;
        out.write_u8((tag >> 8) as u8)?;
        out.write_u8(tdf_type)?;
        value.write(out)
    }
}

impl Readable for Tdf {
    fn read<R: Read>(input: &mut R) -> TdfResult<Self> where Self: Sized {
        let mut tag: [u8; 3] = [0, 0, 0];
        input.read(&mut tag)?;
        let rtype = input.read_u8()?;
        let label = Tdf::tag_to_label(&tag);
        let value = TdfType::read(rtype, input)?;
        Ok(Self(label, value))
    }
}

#[derive(Clone)]
pub struct VarInt(pub u64);

impl From<VarInt> for u64 {
    fn from(value: VarInt) -> Self {
        value.0
    }
}

impl Readable for VarInt {
    fn read<R: Read>(input: &mut R) -> TdfResult<Self> where Self: Sized {
        let mut result = {
            let first_byte = input.read_u8()?;
            let value = (first_byte & 63) as u64;
            if first_byte < 128 {
                return Ok(VarInt(value));
            }
            value
        };
        let mut shift = 6;
        let mut byte: u8;
        loop {
            byte = input.read_u8()?;
            result |= (((byte & 127) as u64) << shift);
            shift += 7;
            if byte < 128 {
                break;
            }
        }
        Ok(VarInt(result))
    }
}

impl Writable for VarInt {
    fn write<W: Write>(&self, out: &mut W) -> TdfResult<()> {
        let value = self.0;
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
}

#[derive(Clone)]
pub enum TdfType {
    VarInt { value: VarInt },
    String { value: String },
    Blob { value: Vec<u8> },
    Group { start2: bool, values: Vec<Tdf> },
    List { l_type: u8, values: Vec<TdfType> },
    Map { key_type: u8, value_type: u8, keys: Vec<TdfType>, values: Vec<TdfType> },
    Optional { value_type: u8, value: Option<Box<Tdf>> },
    VarIntList { values: Vec<VarInt> },
    Pair { a: VarInt, b: VarInt },
    Triple { a: VarInt, b: VarInt, c: VarInt },
    Float { value: f32 },
}

impl From<&TdfType> for u8 {
    fn from(value: &TdfType) -> Self {
        match value {
            TdfType::VarInt { .. } => Tdf::VAR_INT_TYPE,
            TdfType::String { .. } => Tdf::STRING_TYPE,
            TdfType::Blob { .. } => Tdf::BLOB_TYPE,
            TdfType::Group { .. } => Tdf::GROUP_TYPE,
            TdfType::List { .. } => Tdf::LIST_TYPE,
            TdfType::Map { .. } => Tdf::MAP_TYPE,
            TdfType::Optional { .. } => Tdf::OPTIONAL_TYPE,
            TdfType::VarIntList { .. } => Tdf::INT_LIST_TYPE,
            TdfType::Pair { .. } => Tdf::PAIR_TYPE,
            TdfType::Triple { .. } => Tdf::TRIPLE_TYPE,
            TdfType::Float { .. } => Tdf::FLOAT_TYPE,
        }
    }
}

impl TypedReadable for TdfType {
    fn read<R: Read>(rtype: u8, input: &mut R) -> TdfResult<Self> where Self: Sized {
        match rtype {
            Tdf::VAR_INT_TYPE => {
                let value = VarInt::read(input)?;
                Ok(TdfType::VarInt { value })
            }
            Tdf::STRING_TYPE => {
                let length = VarInt::read(input)?.0 as usize;
                let mut bytes = Vec::with_capacity(length);
                input.read_exact(&mut bytes)?;
                let value = String::from_utf8_lossy(&bytes)
                    .to_string();
                Ok(TdfType::String { value })
            }
            Tdf::BLOB_TYPE => {
                let length = VarInt::read(input)?.0 as usize;
                let mut bytes = Vec::with_capacity(length);
                input.read_exact(&mut bytes)?;
                Ok(TdfType::Blob { value: bytes })
            }
            Tdf::GROUP_TYPE => {
                let mut values = Vec::new();
                let mut read = BytePeek::new(input);
                let mut start2 = false;
                let mut peeked: u8;
                loop {
                    peeked = read.peek()?;
                    if peeked == 0 {
                        break;
                    } else if peeked == 2 {
                        start2 = true
                    } else {
                        read.unpeek();
                        let value = Tdf::read(&mut read)?;
                        values.push(value)
                    }
                }
                Ok(TdfType::Group { start2, values })
            }
            Tdf::LIST_TYPE => {
                let sub_type = input.read_u8()?;
                let length = VarInt::read(input)?.0 as usize;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    let value = TdfType::read(sub_type, input)?;
                    values.push(value)
                }
                Ok(TdfType::List { l_type: sub_type, values })
            }
            Tdf::MAP_TYPE => {
                let key_type = input.read_u8()?;
                let value_type = input.read_u8()?;
                let length = VarInt::read(input)?.0 as usize;

                let mut keys = Vec::with_capacity(length);
                let mut values = Vec::with_capacity(length);

                for _ in 0..length {
                    let key = TdfType::read(key_type, input)?;
                    keys.push(key);

                    let value = TdfType::read(value_type, input)?;
                    values.push(value);
                }

                Ok(TdfType::Map { key_type, value_type, keys, values })
            }
            Tdf::OPTIONAL_TYPE => {
                let value_type = input.read_u8()?;
                let value = if value_type != Tdf::OPTIONAL_NO_VALUE {
                    Some(Box::new(Tdf::read(input)?))
                } else {
                    None
                };
                Ok(TdfType::Optional { value_type, value })
            }
            Tdf::INT_LIST_TYPE => {
                let length = VarInt::read(input)?.0 as usize;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(VarInt::read(input)?);
                }
                Ok(TdfType::VarIntList { values })
            }
            Tdf::PAIR_TYPE => {
                let a = VarInt::read(input)?;
                let b = VarInt::read(input)?;
                Ok(TdfType::Pair { a, b })
            }
            Tdf::TRIPLE_TYPE => {
                let a = VarInt::read(input)?;
                let b = VarInt::read(input)?;
                let c = VarInt::read(input)?;
                Ok(TdfType::Triple { a, b, c })
            }
            Tdf::FLOAT_TYPE => {
                let value = input.read_f32::<BE>()?;
                Ok(TdfType::Float { value })
            }
            rtype => Err(TdfError::UnknownType(rtype))
        }
    }
}

impl Writable for TdfType {
    fn write<W: Write>(&self, out: &mut W) -> TdfResult<()> {
        match self {
            TdfType::VarInt { value } => {
                value.write(out)?
            }
            TdfType::String { value } => {
                let bytes = value.clone().into_bytes();
                VarInt(bytes.len() as u64).write(out)?;
                out.write(bytes.as_ref())?;
            }
            TdfType::Blob { value } => {
                VarInt(value.len() as u64).write(out)?;
                out.write(value.as_ref())?;
            }
            TdfType::Group { start2, values } => {
                if *start2 {
                    out.write_u8(2)?;
                }
                for value in values {
                    value.write(out)?;
                }
                out.write_u8(0)?;
            }
            TdfType::List { l_type, values } => {
                out.write_u8(*l_type)?;
                for value in values {
                    value.write(out)?;
                }
            }
            TdfType::Map { key_type, value_type, keys, values } => {
                out.write_u8(*key_type)?;
                out.write_u8(*value_type)?;

                let length = keys.len();

                VarInt(length as u64).write(out)?;

                for i in 0..length {
                    let key = keys.get(i)
                        .ok_or(TdfError::InvalidMapSize)?;
                    let value = values.get(i)
                        .ok_or(TdfError::InvalidMapSize)?;
                    key.write(out)?;
                    value.write(out)?;
                }
            }
            TdfType::Optional { value_type, value } => {
                out.write_u8(*value_type)?;
                if let Some(value) = value {
                    value.write(out)?;
                }
            }
            TdfType::VarIntList { values } => {
                let length = values.len();
                VarInt(length as u64).write(out)?;
                for value in values {
                    value.write(out)?;
                }
            }
            TdfType::Pair { a, b } => {
                a.write(out)?;
                b.write(out)?;
            }
            TdfType::Triple { a, b, c } => {
                a.write(out)?;
                b.write(out)?;
                c.write(out)?;
            }
            TdfType::Float { value } => {
                out.write_f32::<BE>(*value)?;
            }
        }
        Ok(())
    }
}