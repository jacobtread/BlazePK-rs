use std::io::{Cursor, Read, Write};
use byteorder::{BE, ReadBytesExt, WriteBytesExt};
use crate::error::{EmptyTdfResult, TdfResult};
use crate::io::{Readable, TdfRead, Writable};
use crate::tdf::Tdf;


pub enum PacketDirection {
    Request,
    Response,
    Notify,
    Error,
    Unknown(u16),
}

impl Into<u16> for &PacketDirection {
    fn into(self) -> u16 {
        match self {
            PacketDirection::Request => 0x0000,
            PacketDirection::Response => 0x1000,
            PacketDirection::Notify => 0x2000,
            PacketDirection::Error => 0x3000,
            PacketDirection::Unknown(value) => *value
        }
    }
}

impl From<u16> for PacketDirection {
    fn from(value: u16) -> Self {
        match value {
            0x0000 => PacketDirection::Request,
            0x1000 => PacketDirection::Response,
            0x2000 => PacketDirection::Notify,
            0x3000 => PacketDirection::Error,
            value => PacketDirection::Unknown(value)
        }
    }
}

pub struct Packet {
    pub component: u16,
    pub command: u16,
    pub error: u16,
    pub dir: PacketDirection,
    pub id: u16,
    pub contents: Vec<Tdf>,
}

impl Packet {
    pub fn push(&mut self, value: Tdf) {
        self.contents.push(value);
    }

    pub fn set_contents(&mut self, value: Vec<Tdf>) {
        self.contents = value;
    }

    pub fn response(packet: &DecodedPacket, contents: Vec<Tdf>) -> Self {
        Self {
            component: packet.component,
            command: packet.command,
            error: 0,
            dir: PacketDirection::Response,
            id: packet.id,
            contents,
        }
    }

    pub fn error(packet: &DecodedPacket, error: u16, contents: Vec<Tdf>) -> Self {
        Self {
            component: packet.component,
            command: packet.command,
            error,
            dir: PacketDirection::Response,
            id: packet.id,
            contents,
        }
    }

    pub fn notify(component: u16, command: u16, contents: Vec<Tdf>) -> Self {
        Self {
            component,
            command,
            error: 0,
            dir: PacketDirection::Notify,
            id: 0,
            contents,
        }
    }
}

pub struct DecodedPacket {
    pub component: u16,
    pub command: u16,
    pub error: u16,
    pub qtype: u16,
    pub id: u16,
    pub contents: Vec<u8>,
}

impl DecodedPacket {
    pub fn decode<T>(&self) {}
}


impl Readable for DecodedPacket {
    fn read<R: Read>(input: &mut TdfRead<R>) -> TdfResult<Self> where Self: Sized {
        let length = input.read_u16::<BE>()?;
        let component = input.read_u16::<BE>()?;
        let command = input.read_u16::<BE>()?;
        let error = input.read_u16::<BE>()?;
        let qtype = input.read_u16::<BE>()?;
        let id = input.read_u16::<BE>()?;
        let ext_length = if (qtype & 0x10) != 0 {
            input.read_u16::<BE>()?
        } else {
            0
        };
        let content_length: u32 = length as u32 + ((ext_length as u32) << 16);
        let mut bytes = Vec::with_capacity(content_length as usize);
        input.read_exact(&mut bytes)?;

        Ok(DecodedPacket {
            component,
            command,
            error,
            qtype,
            id,
            contents: bytes,
        })
    }
}

impl Writable for Packet {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        let mut bytes = Vec::new();
        let contents_buffer = &mut Cursor::new(&mut bytes);
        for tdf in &self.contents {
            tdf.write(contents_buffer)?;
        }

        let content_size = bytes.len();
        let is_extended = content_size > 0xFFFF;
        out.write_u16::<BE>(content_size as u16)?;
        out.write_u16::<BE>(self.component)?;
        out.write_u16::<BE>(self.command)?;
        out.write_u16::<BE>(self.error)?;
        let dir_value: u16 = (&self.dir).into();
        out.write_u8((dir_value << 8) as u8)?;
        if is_extended {
            out.write_u8(0x10)?;
        } else {
            out.write_u8(0x00)?;
        }
        out.write_u16::<BE>(self.id)?;
        if is_extended {
            out.write_u8(((content_size & 0xFF000000) >> 24) as u8)?;
            out.write_u8(((content_size & 0x00FF0000) >> 16) as u8)?;
        }

        out.write(&bytes)?;
        Ok(())
    }
}

impl Writable for DecodedPacket {
    fn write<W: Write>(&self, out: &mut W) -> EmptyTdfResult {
        let bytes = &self.contents;
        let content_size = bytes.len();
        let is_extended = content_size > 0xFFFF;
        out.write_u16::<BE>(content_size as u16)?;
        out.write_u16::<BE>(self.component)?;
        out.write_u16::<BE>(self.command)?;
        out.write_u16::<BE>(self.error)?;
        out.write_u8((self.qtype << 8) as u8)?;
        if is_extended {
            out.write_u8(0x10)?;
        } else {
            out.write_u8(0x00)?;
        }
        out.write_u16::<BE>(self.id)?;
        if is_extended {
            out.write_u8(((content_size & 0xFF000000) >> 24) as u8)?;
            out.write_u8(((content_size & 0x00FF0000) >> 16) as u8)?;
        }
        out.write(bytes)?;
        Ok(())
    }
}