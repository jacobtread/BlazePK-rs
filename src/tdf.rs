use crate::codec::{Codec, Reader};
use std::borrow::Cow;

pub struct Tdf(String, Value);

impl Codec for Tdf {
    fn encode(&self, output: &mut Vec<u8>) {
        todo!()
    }

    fn decode(reader: &mut Reader) -> Option<Self> {
        let tag = reader.take(3).and_then(Self::read_tag)?;
        let tdf_type = reader.take_one()?;

        todo!()
    }
}

impl Tdf {
    fn read_tag(input: &[u8]) -> Option<String> {
        let input: &[u8; 3] = input.try_into().ok()?;
        let mut buffer: [u8; 4] = [0, 0, 0, 0];

        buffer[0] |= (input[0] & 0x80) >> 1;
        buffer[0] |= (input[0] & 0x40) >> 2;
        buffer[0] |= (input[0] & 0x30) >> 2;
        buffer[0] |= (input[0] & 0x0C) >> 2;

        buffer[1] |= (input[0] & 0x02) << 5;
        buffer[1] |= (input[0] & 0x01) << 4;
        buffer[1] |= (input[1] & 0xF0) >> 4;

        buffer[2] |= (input[1] & 0x08) << 3;
        buffer[2] |= (input[1] & 0x04) << 2;
        buffer[2] |= (input[1] & 0x03) << 2;
        buffer[2] |= (input[2] & 0xC0) >> 6;

        buffer[3] |= (input[2] & 0x20) << 1;
        buffer[3] |= input[2] & 0x1F;

        let mut output = String::with_capacity(4);
        for byte in buffer {
            if byte == 0 {
                output.push(' ');
            } else {
                output.push(char::from(byte));
            }
        }
        output
    }
}

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
}

pub enum Value {
    VarInt(u64),
    String(String),
    Blob(Vec<u8>),
    Group { start2: bool, values: Vec<Tdf> },
    List { value_type: u8 },
}

impl Value {}
