use std::io::{Cursor, Read, Write};
use byteorder::{BE, ReadBytesExt, WriteBytesExt};
use crate::io::{Readable, TdfResult, Writable};
use crate::tdf::Tdf;

pub struct Packet {
    component: u16,
    command: u16,
    error: u16,
    qtype: u16,
    id: u16,
    contents: Vec<Tdf>,
}

impl Readable for Packet {
    fn read<R: Read>(input: &mut R) -> TdfResult<Self> where Self: Sized {
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
        let content_length: u32 = (length as u32 + ((ext_length as u32) << 16));
        let bytes = &mut Vec::with_capacity(content_length as usize);
        input.read_exact(bytes)?;
        let mut reader = Cursor::new(bytes);

        let mut contents = Vec::new();
        loop {
            let value = Tdf::read(&mut reader);
            if let Ok(value) = value {
                contents.push(value)
            } else {
                break;
            }
        }

        Ok(Packet {
            component,
            command,
            error,
            qtype,
            id,
            contents,
        })
    }
}

impl Writable for Packet {
    fn write<W: Write>(&self, out: &mut W) -> TdfResult<()> {
        let mut bytes = Vec::new();
        let contents_out = &mut Cursor::new(&mut bytes);
        for tdf in &self.contents {
            tdf.write(contents_out)?;
        }

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

        out.write(&bytes)?;
        Ok(())
    }
}

impl Packet {
    const REQUEST: u16 = 0x0000;
    const RESPONSE: u16 = 0x1000;
    const NOTIFY: u16 = 0x2000;
    const ERROR: u16 = 0x3000;


    pub fn response(packet: &Packet, contents: Vec<Tdf>) -> Self {
        Self {
            component: packet.component,
            command: packet.command,
            error: 0,
            qtype: Packet::RESPONSE,
            id: packet.id,
            contents,
        }
    }

    pub fn error(packet: &Packet, error: u16, contents: Vec<Tdf>) -> Self {
        Self {
            component: packet.component,
            command: packet.command,
            error,
            qtype: Packet::ERROR,
            id: packet.id,
            contents,
        }
    }

    pub fn notify(component: u16, command: u16, contents: Vec<Tdf>) -> Self {
        Self {
            component,
            command,
            error: 0,
            qtype: Packet::NOTIFY,
            id: 0,
            contents,
        }
    }

    pub fn push(&mut self, value: Tdf) {
        self.contents.push(value);
    }
}

