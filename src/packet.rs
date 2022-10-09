use crate::codec::{ReadBytesExt, Reader};
use std::io;
use std::io::Read;

pub struct DecodedPacket {
    pub component: u16,
    pub command: u16,
    pub error: u16,
    pub qtype: u16,
    pub id: u16,
    pub contents: Vec<u8>,
}

impl DecodedPacket {
    pub fn read<R: Read>(input: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let length = input.read_u16()?;
        let component = input.read_u16()?;
        let command = input.read_u16()?;
        let error = input.read_u16()?;
        let qtype = input.read_u16()?;
        let id = input.read_u16()?;
        let ext_length = if (qtype & 0x10) != 0 {
            input.read_u16()?
        } else {
            0
        };

        let content_length = length as usize + ((ext_length as usize) << 16);
        let mut content_bytes = vec![0u8; content_length];
        input.read_exact(&mut content_bytes)?;
        Ok(DecodedPacket {
            component,
            command,
            error,
            qtype,
            id,
            contents: content_bytes,
        })
    }
}
