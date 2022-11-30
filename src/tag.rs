use crate::codec::{Codec, CodecError, CodecResult, Decodable, Encodable, Reader};
use crate::reader::TdfReader;
use crate::types::{Blob, VarIntList, UNION_UNSET};
use crate::writer::TdfWriter;
use std::fmt::Debug;

/// Tag for a Tdf value. This contains the String tag for naming
/// the field and then the type of the field
#[derive(Debug, Eq, PartialEq)]
pub struct Tag(pub String, pub TdfType);

impl Tag {
    /// Decodes tags from the reader until the tag with the provided tag name
    /// is found. If the tag type doesn't match the `expected_type` then an
    /// error will be returned.
    ///
    /// `reader`        The reader to read from
    /// `tag`       	The tag name to read until
    /// `expected_type` The expected type of the tag
    pub fn decode_until(reader: &mut Reader, tag: &str, expected_type: TdfType) -> CodecResult<()> {
        loop {
            let decoded = match Self::decode(reader) {
                Ok(tag) => tag,
                Err(CodecError::NotEnoughBytes(_, _, _)) => {
                    return Err(CodecError::MissingField(tag.to_string()))
                }
                Err(err) => return Err(err),
            };
            if decoded.0.ne(tag) {
                Self::discard_type(&decoded.1, reader)?;
                continue;
            }
            if decoded.1.ne(&expected_type) {
                return Err(CodecError::UnexpectedFieldType(
                    tag.to_string(),
                    expected_type.clone(),
                    decoded.1.clone(),
                ));
            }
            return Ok(());
        }
    }

    /// Attempting version of decode_until that returns true if the value was decoded up to
    /// otherwise returns false. Marks the reader position before reading and resets the position
    /// if the tag was not found
    ///
    /// `reader`        The reader to read from
    /// `tag`       	The tag name to read until
    /// `expected_type` The expected type of the tag
    pub fn try_decode_until(reader: &mut Reader, tag: &str, expected_type: TdfType) -> bool {
        reader.mark();
        while let Ok(decoded) = Self::decode(reader) {
            if decoded.0.ne(tag) {
                if Self::discard_type(&decoded.1, reader).is_err() {
                    break;
                } else {
                    continue;
                }
            }
            // If the types don't match then break
            if decoded.1.ne(&expected_type) {
                break;
            }
            return true;
        }
        reader.reset_marker();
        false
    }

    /// Expects to be able to decode the value of tag somewhere throughout the
    /// reader using the data type of T
    ///
    /// `reader` The reader to read from
    /// `tag`    The tag to expect
    pub fn expect<T: Codec>(reader: &mut Reader, tag: &str) -> CodecResult<T> {
        let expected_type = T::value_type();
        let _ = Self::decode_until(reader, tag, expected_type)?;
        T::decode(reader)
    }

    pub fn try_expect<T: Codec>(reader: &mut Reader, tag: &str) -> CodecResult<Option<T>> {
        reader.mark();
        match Self::expect(reader, tag) {
            Err(CodecError::MissingField(_)) => {
                reader.reset_marker();
                Ok(None)
            }
            Ok(value) => Ok(Some(value)),
            Err(err) => Err(err),
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

    pub fn stringify(reader: &mut Reader, out: &mut String, indent: usize) -> CodecResult<()> {
        while reader.remaining() > 0 {
            match Self::create_string_tag(reader, out, indent) {
                Ok(_) => {}
                Err(err) => {
                    out.push_str(&format!(
                        "... remaining {}, cause: {:?}",
                        reader.remaining(),
                        err
                    ));
                    break;
                }
            };
        }
        Ok(())
    }

    pub fn create_string_tag(
        reader: &mut Reader,
        out: &mut String,
        indent: usize,
    ) -> CodecResult<()> {
        let tag = Tag::decode(reader)?;
        out.push_str(&"  ".repeat(indent));
        out.push_str(&format!("\"{}\": ", &tag.0));
        return match Self::create_string_type(reader, out, indent, &tag.1) {
            Ok(_) => {
                out.push_str(",\n");
                Ok(())
            }
            Err(err) => {
                out.push_str("...");
                Err(err)
            }
        };
    }

    pub fn create_string_type(
        reader: &mut Reader,
        out: &mut String,
        indent: usize,
        ty: &TdfType,
    ) -> CodecResult<()> {
        match ty {
            TdfType::VarInt => {
                let value = u64::decode(reader)?;
                out.push_str(&value.to_string());
            }
            TdfType::String => {
                let value = String::decode(reader)?;
                out.push('"');
                out.push_str(&value);
                out.push('"');
            }
            TdfType::Blob => {
                let value = Blob::decode(reader)?;
                out.push_str("Blob[");
                for b in value.0 {
                    out.push_str(&format!("0x{:X}", b));
                }
                out.push(']');
            }
            TdfType::Group => {
                out.push_str("{\n");
                let mut is_two = false;
                loop {
                    let next_byte = reader.take_one()?;
                    if next_byte == 0 {
                        break;
                    }
                    if next_byte != 2 {
                        reader.step_back();
                    } else {
                        is_two = true;
                    }
                    Self::create_string_tag(reader, out, indent + 1)?;
                }
                out.push_str(&"  ".repeat(indent));
                out.push_str("}");
                if is_two {
                    out.push_str(" (2)");
                }
            }
            TdfType::List => {
                let value_type = TdfType::decode(reader)?;
                let length = usize::decode(reader)?;

                let nl = match value_type {
                    TdfType::Map | TdfType::Group => true,
                    _ => false,
                };

                out.push_str(&format!("List<{:?}> ", value_type));
                out.push('[');
                if nl {
                    out.push('\n')
                }

                for i in 0..length {
                    if nl {
                        out.push_str(&"  ".repeat(indent + 1));
                    }
                    Self::create_string_type(reader, out, indent + 1, &value_type)?;

                    if i < length - 1 {
                        out.push_str(", ");
                    }

                    if nl {
                        out.push('\n')
                    }
                }

                if nl {
                    out.push_str(&"  ".repeat(indent));
                }
                out.push(']');
            }
            TdfType::Map => {
                let key_type = TdfType::decode(reader)?;
                let value_type = TdfType::decode(reader)?;
                let length = usize::decode(reader)?;
                out.push_str(&format!("Map<{:?}, {:?}> ", key_type, value_type));
                out.push_str("{\n");

                for _ in 0..length {
                    out.push_str(&"  ".repeat(indent + 1));

                    Self::create_string_type(reader, out, indent + 1, &key_type)?;

                    out.push_str(": ");

                    Self::create_string_type(reader, out, indent + 1, &value_type)?;

                    out.push('\n')
                }

                out.push_str(&"  ".repeat(indent));
                out.push('}');
            }
            TdfType::Union => {
                let ty = reader.take_one()?;
                if ty != UNION_UNSET {
                    out.push_str("Union(");
                    let tag = Tag::decode(reader)?;
                    out.push_str(&format!("\"{}\", {:?}: ", &tag.0, ty));
                    Self::create_string_type(reader, out, indent + 1, &tag.1)?;
                    out.push_str(")")
                } else {
                    out.push_str("Union(Unset)");
                }
            }
            TdfType::VarIntList => {
                let value = VarIntList::<usize>::decode(reader)?.0;
                out.push_str("VarList[");
                let length = value.len();
                for i in 0..length {
                    let b = value[i];
                    out.push_str(&format!("0x{:X}", b));
                    if i < length - 1 {
                        out.push_str(", ");
                    }
                }
                out.push(']');
            }
            TdfType::Pair => {
                let pair = <(usize, usize)>::decode(reader)?;
                out.push_str(&format!("({}, {})", &pair.0, &pair.1));
            }
            TdfType::Triple => {
                let value = <(usize, usize, usize)>::decode(reader)?;
                out.push_str(&format!("({}, {}, {})", &value.0, &value.1, &value.2));
            }
            TdfType::Float => {
                let value = f32::decode(reader)?;
                out.push_str(&value.to_string());
            }
            TdfType::Unknown(_) => return Err(CodecError::Other("Unknown tag type")),
        }
        Ok(())
    }

    /// Discards the next tag and all of its contents
    pub fn discard_tag(reader: &mut Reader) -> CodecResult<()> {
        let tag = Tag::decode(reader)?;
        Self::discard_type(&tag.1, reader)
    }

    /// Discards the provided type of value
    pub fn discard_type(ty: &TdfType, reader: &mut Reader) -> CodecResult<()> {
        match ty {
            TdfType::VarInt => usize::skip(reader)?,
            TdfType::String => String::skip(reader)?,
            TdfType::Blob => <Vec<u8>>::skip(reader)?,
            TdfType::Group => Self::discard_group(reader)?,
            TdfType::List => {
                let new_ty = TdfType::decode(reader)?;
                let length = usize::decode(reader)?;
                for _ in 0..length {
                    Self::discard_type(&new_ty, reader)?;
                }
            }
            TdfType::Map => {
                let key_ty = TdfType::decode(reader)?;
                let value_ty = TdfType::decode(reader)?;
                let length = usize::decode(reader)?;
                for _ in 0..length {
                    Self::discard_type(&key_ty, reader)?;
                    Self::discard_type(&value_ty, reader)?;
                }
            }
            TdfType::Union => {
                let ty = reader.take_one()?;
                if ty != UNION_UNSET {
                    Self::discard_tag(reader)?;
                }
            }
            TdfType::VarIntList => VarIntList::<usize>::skip(reader)?,
            TdfType::Pair => <(usize, usize)>::skip(reader)?,
            TdfType::Triple => <(usize, usize, usize)>::skip(reader)?,
            TdfType::Float => f32::skip(reader)?,
            TdfType::Unknown(_) => {}
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
    pub fn encode_from(tag: &str, value_type: &TdfType, output: &mut Vec<u8>) {
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

impl Encodable for Tag {
    fn encode(&self, output: &mut Vec<u8>) {
        Tag::encode_from(&self.0, &self.1, output);
    }
}

impl Decodable for Tag {
    fn decode(reader: &mut Reader) -> CodecResult<Self> {
        let tag: &[u8; 4] = reader.take(4)?.into();

        let value_type = TdfType::from_value(tag[3]);
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
pub enum TdfType {
    VarInt,
    String,
    Blob,
    Group,
    List,
    Map,
    Union,
    VarIntList,
    Pair,
    Triple,
    Float,
    Unknown(u8),
}

impl Encodable for TdfType {
    #[inline]
    fn encode(&self, output: &mut TdfWriter) {
        output.write_byte(self.value())
    }
}

impl Decodable for TdfType {
    fn decode(reader: &mut TdfReader) -> CodecResult<Self> {
        reader.read_byte().map(TdfType::from_value)
    }
}

impl TdfType {
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
            TdfType::Unknown(value) => *value,
        }
    }

    pub fn from_value(value: u8) -> TdfType {
        match value {
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
            value => TdfType::Unknown(value),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::codec::Reader;
    use crate::tag::{Tag, TdfType};

    #[test]
    fn test_read_write() {
        let mut out = Vec::new();
        let tag_in = Tag(String::from("TEST"), TdfType::String);
        tag_in.encode(&mut out);
        let mut reader = Reader::new(&out);
        let tag = Tag::decode(&mut reader).unwrap();
        assert_eq!(tag_in, tag)
    }

    #[test]
    fn test_tag() {
        let tag_out = Tag(String::from("PORT"), TdfType::VarInt);
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
