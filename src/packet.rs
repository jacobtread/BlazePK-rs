use crate::codec::{Codec, CodecResult, ReadBytesExt};
use std::fmt::Debug;
use std::io;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU16, Ordering};

/// Trait implemented by Codec values that can be used
/// as packet contents
pub trait PacketContent: Codec + Debug {}

/// The different types of packets
#[derive(Debug)]
pub enum PacketType {
    /// ID counted request packets
    Request,
    /// Packets responding to requests
    Response,
    /// Unique packets coming from the server
    Notify,
    /// Error packets
    Error,
    /// Packet type that is unknown
    Unknown(u16),
}

impl PacketType {
    /// Returns the u16 representation of the packet type
    pub fn value(&self) -> u16 {
        match self {
            PacketType::Request => 0x0000,
            PacketType::Response => 0x1000,
            PacketType::Notify => 0x2000,
            PacketType::Error => 0x3000,
            PacketType::Unknown(value) => *value,
        }
    }

    /// Gets the packet type this value is represented by
    pub fn from_value(value: u16) -> PacketType {
        match value {
            0x0000 => PacketType::Request,
            0x1000 => PacketType::Response,
            0x2000 => PacketType::Notify,
            0x3000 => PacketType::Error,
            value => PacketType::Unknown(value),
        }
    }
}

/// Structure for a packet created by ourselves where
/// the data contents are already known and not encoded
#[derive(Debug)]
pub struct Packet<C: PacketContent> {
    /// The component of this packet
    pub component: u16,
    /// The command of this packet
    pub command: u16,
    /// A possible error this packet contains (zero is none)
    pub error: u16,
    /// The type of this packet
    pub ty: PacketType,
    /// The unique ID of this packet (Notify packets this is just zero)
    pub id: u16,
    /// The contents of the packet
    pub contents: C,
}

impl<C: PacketContent> Packet<C> {
    /// Creates a new response packet for responding to the provided
    /// decodable packet. With the `contents`
    pub fn response(&self, packet: &OpaquePacket, contents: C) -> Packet<C> {
        Packet {
            component: packet.component,
            command: packet.command,
            error: 0,
            ty: PacketType::Response,
            id: packet.id,
            contents,
        }
    }

    /// Creates a new error response packet for responding to the
    /// provided packet with an error number with `contents`
    pub fn error(&self, packet: &OpaquePacket, error: impl Into<u16>, contents: C) -> Packet<C> {
        Packet {
            component: packet.component,
            command: packet.command,
            error: error.into(),
            ty: PacketType::Error,
            id: packet.id,
            contents,
        }
    }

    /// Creates a new notify packet with the provided component and command
    /// and `contents`
    pub fn notify(
        &self,
        component: impl Into<u16>,
        command: impl Into<u16>,
        contents: C,
    ) -> Packet<C> {
        Packet {
            component: component.into(),
            command: command.into(),
            error: 0,
            ty: PacketType::Notify,
            id: 0,
            contents,
        }
    }

    /// Creates a new request packet retrieving its ID from the provided
    /// request counter.
    pub fn request<R: RequestCounter>(
        &self,
        counter: &mut R,
        component: impl Into<u16>,
        command: impl Into<u16>,
        contents: C,
    ) -> Packet<C> {
        Packet {
            component: component.into(),
            command: command.into(),
            error: 0,
            ty: PacketType::Notify,
            id: counter.next(),
            contents,
        }
    }

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    pub fn write<W: Write>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = self.contents.encode_bytes();
        let mut header = Vec::with_capacity(12);
        let length = content.len();
        let is_extended = length > 0xFFFF;

        (length as u16).encode(&mut header);
        self.component.encode(&mut header);
        self.command.encode(&mut header);
        self.error.encode(&mut header);

        header.push((self.ty.value() >> 8) as u8);
        header.push(if is_extended { 0x10 } else { 0x00 });

        self.id.encode(&mut header);

        if is_extended {
            header.push(((length & 0xFF000000) >> 24) as u8);
            header.push(((length & 0x00FF0000) >> 16) as u8);
        }

        output.write_all(&header)?;
        output.write_all(&content)?;
        Ok(())
    }
}

/// Structure for packets that have been read where the contents
/// are not know and are encoded as a vector of bytes.
#[derive(Debug)]
pub struct OpaquePacket {
    /// The component of this packet
    pub component: u16,
    /// The command of this packet
    pub command: u16,
    /// A possible error this packet contains (zero is none)
    pub error: u16,
    /// The type of this packet
    pub ty: PacketType,
    /// The unique ID of this packet (Notify packets this is just zero)
    pub id: u16,
    /// The raw encoded byte contents of the packet
    pub contents: Vec<u8>,
}

impl OpaquePacket {
    /// Reads the contents of this encoded packet and tries to decode
    /// the `R` from it.
    pub fn content<R: PacketContent>(&self) -> CodecResult<R> {
        R::decode_from(&self.contents)
    }

    /// Reads an OpaquePacket from the provided input
    pub fn read<R: Read>(input: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let length = input.read_u16()?;
        let component = input.read_u16()?;
        let command = input.read_u16()?;
        let error = input.read_u16()?;
        let q_type = input.read_u16()?;
        let id = input.read_u16()?;
        let ext_length = if (q_type & 0x10) != 0 {
            input.read_u16()?
        } else {
            0
        };

        let content_length = length as usize + ((ext_length as usize) << 16);
        let mut contents = vec![0u8; content_length];
        input.read_exact(&mut contents)?;

        let ty = PacketType::from_value(q_type);

        Ok(OpaquePacket {
            component,
            command,
            error,
            ty,
            id,
            contents,
        })
    }
}

/// Structure for counting requests to generate the packet
/// ID's for requests
pub trait RequestCounter {
    /// Called to obtain the next packet ID
    fn next(&mut self) -> u16;
}

/// Simple counter which is just backed by a u16
/// value that is incremented on each request
pub struct SimpleCounter {
    value: u16,
}

impl SimpleCounter {
    /// Creates a new simple counter
    pub fn new() -> SimpleCounter {
        SimpleCounter { value: 0 }
    }
}

impl RequestCounter for SimpleCounter {
    fn next(&mut self) -> u16 {
        self.value += 1;
        self.value
    }
}

/// Atomic backed counter implementation
pub struct AtomicCounter {
    value: AtomicU16,
}

impl AtomicCounter {
    /// Creates a new atomic counter
    pub fn new() -> AtomicCounter {
        AtomicCounter {
            value: AtomicU16::new(0),
        }
    }
}

impl RequestCounter for AtomicCounter {
    fn next(&mut self) -> u16 {
        self.value.fetch_add(1, Ordering::AcqRel)
    }
}
