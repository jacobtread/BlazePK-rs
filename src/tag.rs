use crate::codec::{Codec, CodecError, CodecResult, Reader};
use crate::types::{VarInt, VarIntList, EMPTY_OPTIONAL};
use std::fmt::Debug;

/// Tag for a Tdf value. This contains the String tag for naming
/// the field and then the type of the field
#[derive(Debug, Eq, PartialEq)]
pub struct Tag(pub String, pub ValueType);

/// Encoding structure for Codec values tagged with a string tag
/// these are encoded as the tag then the value
pub type TaggedValue<T> = (String, T);

impl<T: Codec> Codec for TaggedValue<T> {
    fn encode(&self, output: &mut Vec<u8>) {
        Tag::encode_from(&self.0, &T::value_type(), output);
        T::encode(&self.1, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let tag = Tag::decode(reader)?;

        let expected_type = T::value_type();
        let actual_type = tag.1;

        if actual_type != expected_type {
            return Err(CodecError::UnexpectedType(expected_type, actual_type));
        }

        let value = T::decode(reader)?;

        Ok((tag.0, value))
    }
}

impl Tag {
    /// Reads through the provided reader until a tag with
    /// the provided tag and value type is met returning null
    /// if no more tags were found or the tag was found but was
    /// not of the right type
    pub fn expect_tag(
        tag: &'static str,
        value_type: &ValueType,
        reader: &mut Reader,
    ) -> CodecResult<Tag> {
        loop {
            match Tag::decode(reader) {
                Ok(read_tag) => {
                    if read_tag.0.eq(tag) {
                        if read_tag.1.ne(value_type) {
                            return Err(CodecError::UnexpectedFieldType(
                                tag,
                                value_type.clone(),
                                read_tag.1.clone(),
                            ));
                        }

                        return Ok(read_tag);
                    } else {
                        Self::discard_type(&read_tag.1, reader)?;
                    }
                }
                Err(CodecError::NotEnoughBytes(_, _, _)) => {
                    return Err(CodecError::MissingField(tag));
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Tries to take a byte from the front checking if its
    /// a two and stepping back if the value isn't two
    pub fn take_two(reader: &mut Reader) -> CodecResult<()> {
        let byte = reader.take_one()?;
        if byte != 2 {
            reader.step_back();
        }
        Ok(())
    }

    /// Discards everything printing out everything it hits
    pub fn debug_discard(reader: &mut Reader) -> CodecResult<()> {
        while reader.remaining() > 0 {
            let tag = Tag::decode(reader)?;
            println!("{tag:?}");
            Self::debug_discard_type(&tag.1, reader)?;
        }
        Ok(())
    }

    /// Discards a type printing out everything it hits
    fn debug_discard_type(ty: &ValueType, reader: &mut Reader) -> CodecResult<()> {
        match ty {
            ValueType::VarInt => {
                let value = VarInt::decode(reader)?;
                println!("VarInt: {value:?}");
            }
            ValueType::String => {
                let value = String::decode(reader)?;
                println!("String: {value:?}");
            }
            ValueType::Blob => {
                let value = <Vec<u8>>::decode(reader)?;
                println!("Blob: {value:?}");
            }
            ValueType::Group => {
                println!("Start group");
                while let Ok(next_byte) = reader.take_one() {
                    if next_byte == 0 {
                        break;
                    }
                    if next_byte != 2 {
                        reader.step_back();
                    }
                    let tag = Tag::decode(reader)?;
                    println!("{tag:?}");
                    Self::debug_discard_type(&tag.1, reader)?;
                }
                println!("End group");
            }
            ValueType::List => {
                let new_ty = ValueType::decode(reader)?;
                println!("List Type: {new_ty:?}");
                let length = VarInt::decode(reader)?.0 as usize;
                println!("STart list");
                for _ in 0..length {
                    Self::debug_discard_type(&new_ty, reader)?;
                }
                println!("End list")
            }
            ValueType::Map => {
                let key_ty = ValueType::decode(reader)?;
                println!("Map Key Type: {key_ty:?}");
                let value_ty = ValueType::decode(reader)?;
                println!("Map Value Type: {value_ty:?}");
                let length = VarInt::decode(reader)?.0 as usize;
                for _ in 0..length {
                    Self::debug_discard_type(&key_ty, reader)?;
                    Self::debug_discard_type(&value_ty, reader)?;
                }
            }
            ValueType::Optional => {
                let ty = reader.take_one()?;
                println!("Optional Type {ty}");
                if ty != EMPTY_OPTIONAL {
                    let new_ty = ValueType::decode(reader)?;
                    println!("Optional Value {new_ty:?}");
                    Self::debug_discard_type(&new_ty, reader)?;
                }
            }
            ValueType::VarIntList => {
                let list = VarIntList::decode(reader)?;
                println!("VarIntList {list:?}");
            }
            ValueType::Pair => {
                let pair = <(VarInt, VarInt)>::decode(reader)?;
                println!("Pair {pair:?}");
            }
            ValueType::Triple => {
                let value = <(VarInt, VarInt, VarInt)>::decode(reader)?;
                println!("Triple {value:?}")
            }
            ValueType::Float => {
                let value = f32::decode(reader)?;
                println!("Float {value:?}")
            }
            ValueType::Unknown(_) => {}
        }
        Ok(())
    }

    /// Discards the next tag and all of its contents
    pub fn discard_tag(reader: &mut Reader) -> CodecResult<()> {
        let tag = Tag::decode(reader)?;
        Self::discard_type(&tag.1, reader)
    }

    /// Discards the provided type of value
    pub fn discard_type(ty: &ValueType, reader: &mut Reader) -> CodecResult<()> {
        match ty {
            ValueType::VarInt => VarInt::skip(reader)?,
            ValueType::String => String::skip(reader)?,
            ValueType::Blob => <Vec<u8>>::skip(reader)?,
            ValueType::Group => Self::discard_group(reader)?,
            ValueType::List => {
                let new_ty = ValueType::decode(reader)?;
                let length = VarInt::decode(reader)?.0 as usize;
                for _ in 0..length {
                    Self::discard_type(&new_ty, reader)?;
                }
            }
            ValueType::Map => {
                let key_ty = ValueType::decode(reader)?;
                let value_ty = ValueType::decode(reader)?;
                let length = VarInt::decode(reader)?.0 as usize;
                for _ in 0..length {
                    Self::discard_type(&key_ty, reader)?;
                    Self::discard_type(&value_ty, reader)?;
                }
            }
            ValueType::Optional => {
                let ty = reader.take_one()?;
                if ty != EMPTY_OPTIONAL {
                    Self::discard_tag(reader)?;
                }
            }
            ValueType::VarIntList => VarIntList::skip(reader)?,
            ValueType::Pair => <(VarInt, VarInt)>::skip(reader)?,
            ValueType::Triple => <(VarInt, VarInt, VarInt)>::skip(reader)?,
            ValueType::Float => f32::skip(reader)?,
            ValueType::Unknown(_) => {}
        };
        Ok(())
    }

    /// Discards any remaining tags in the group to exhaust the
    /// remaining bytes until the group ending byte
    pub fn discard_group(reader: &mut Reader) -> CodecResult<()> {
        while let Ok(next_byte) = reader.take_one() {
            if next_byte == 0 {
                break;
            }
            reader.step_back();
            Self::discard_tag(reader)?;
        }
        Ok(())
    }

    /// Encodes a tag directly using the provided values
    pub fn encode_from(tag: &str, value_type: &ValueType, output: &mut Vec<u8>) {
        Self::encode_tag(tag, output);
        value_type.encode(output);
    }

    /// Encodes the provided tag into its byte form and
    /// appends it to the provided output vec
    pub fn encode_tag(tag: &str, output: &mut Vec<u8>) {
        let mut out: [u8; 3] = [0, 0, 0];
        let mut input: [u8; 4] = [0, 0, 0, 0];

        let mut bytes = tag.bytes();
        for i in 0..4 {
            input[i] = match bytes.next() {
                None => 0,
                Some(value) => value,
            }
        }

        out[0] |= (input[0] & 0x40) << 1;
        out[0] |= (input[0] & 0x10) << 2;
        out[0] |= (input[0] & 0x0F) << 2;
        out[0] |= (input[1] & 0x40) >> 5;
        out[0] |= (input[1] & 0x10) >> 4;

        out[1] |= (input[1] & 0x0F) << 4;
        out[1] |= (input[2] & 0x40) >> 3;
        out[1] |= (input[2] & 0x10) >> 2;
        out[1] |= (input[2] & 0x0C) >> 2;

        out[2] |= (input[2] & 0x03) << 6;
        out[2] |= (input[3] & 0x40) >> 1;
        out[2] |= input[3] & 0x1F;

        output.extend_from_slice(&out);
    }
}

impl Codec for Tag {
    fn encode(&self, output: &mut Vec<u8>) {
        Tag::encode_from(&self.0, &self.1, output);
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let tag: &[u8; 4] = reader
            .take(4)?
            .try_into()
            .map_err(|_| CodecError::UnknownError)?;

        let value_type = ValueType::from_value(tag[3]);
        let mut output: [u8; 4] = [0, 0, 0, 0];

        output[0] |= (tag[0] & 0x80) >> 1;
        output[0] |= (tag[0] & 0x40) >> 2;
        output[0] |= (tag[0] & 0x30) >> 2;
        output[0] |= (tag[0] & 0x0C) >> 2;

        output[1] |= (tag[0] & 0x02) << 5;
        output[1] |= (tag[0] & 0x01) << 4;
        output[1] |= (tag[1] & 0xF0) >> 4;

        output[2] |= (tag[1] & 0x08) << 3;
        output[2] |= (tag[1] & 0x04) << 2;
        output[2] |= (tag[1] & 0x03) << 2;
        output[2] |= (tag[2] & 0xC0) >> 6;

        output[3] |= (tag[2] & 0x20) << 1;
        output[3] |= tag[2] & 0x1F;

        let mut out = String::new();
        for value in output {
            match value {
                0 => {}
                value => out.push(char::from(value)),
            }
        }

        Ok(Tag(out, value_type))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ValueType {
    VarInt,
    String,
    Blob,
    Group,
    List,
    Map,
    Optional,
    VarIntList,
    Pair,
    Triple,
    Float,
    Unknown(u8),
}

impl Codec for ValueType {
    fn encode(&self, output: &mut Vec<u8>) {
        output.push(self.value());
    }

    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        reader.take_one().map(ValueType::from_value)
    }
}

impl ValueType {
    pub fn value(&self) -> u8 {
        match self {
            ValueType::VarInt => 0x0,
            ValueType::String => 0x1,
            ValueType::Blob => 0x2,
            ValueType::Group => 0x3,
            ValueType::List => 0x4,
            ValueType::Map => 0x5,
            ValueType::Optional => 0x6,
            ValueType::VarIntList => 0x7,
            ValueType::Pair => 0x8,
            ValueType::Triple => 0x9,
            ValueType::Float => 0xA,
            ValueType::Unknown(value) => *value,
        }
    }

    pub fn from_value(value: u8) -> ValueType {
        match value {
            0x0 => ValueType::VarInt,
            0x1 => ValueType::String,
            0x2 => ValueType::Blob,
            0x3 => ValueType::Group,
            0x4 => ValueType::List,
            0x5 => ValueType::Map,
            0x6 => ValueType::Optional,
            0x7 => ValueType::VarIntList,
            0x8 => ValueType::Pair,
            0x9 => ValueType::Triple,
            0xA => ValueType::Float,
            value => ValueType::Unknown(value),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::codec::{Codec, Reader};
    use crate::tag::{Tag, ValueType};

    #[test]
    fn test_read_write() {
        let mut out = Vec::new();
        let tag_in = Tag(String::from("TEST"), ValueType::String);
        tag_in.encode(&mut out);
        let mut reader = Reader::new(&out);
        let tag = Tag::decode(&mut reader).unwrap();
        assert_eq!(tag_in, tag)
    }

    #[test]
    fn test_tag() {
        let tag_out = Tag(String::from("PORT"), ValueType::VarInt);
        let mut out = Vec::new();
        tag_out.encode(&mut out);
        println!("{out:?}")
    }

    #[test]
    fn parse_tag() {
        let tag = [226, 75, 179, 0];
        let mut reader = Reader::new(&tag);
        let tag = Tag::decode(&mut reader);

        print!("{tag:?}")
    }
}
