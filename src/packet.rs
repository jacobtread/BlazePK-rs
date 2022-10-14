use crate::codec::{decode_u16, encode_u16, Codec, CodecError, CodecResult, Reader};
use std::fmt::Debug;
use std::io;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU16, Ordering};

use crate::Tag;
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Enum for errors that could occur when dealing with packets
/// (encoding and decoding)
#[derive(Debug)]
pub enum PacketError {
    CodecError(CodecError),
    IO(io::Error),
}

impl From<CodecError> for PacketError {
    fn from(err: CodecError) -> Self {
        PacketError::CodecError(err)
    }
}

impl From<io::Error> for PacketError {
    fn from(err: io::Error) -> Self {
        PacketError::IO(err)
    }
}

/// Result type for returning a value or Packet Error
pub type PacketResult<T> = Result<T, PacketError>;

/// Empty content type for packets
#[derive(Debug)]
pub struct EmptyContent {}

impl Codec for EmptyContent {
    fn encode(&self, _: &mut Vec<u8>) {}

    fn decode(_: &mut Reader) -> CodecResult<Self> {
        Ok(EmptyContent {})
    }
}

impl PacketContent for EmptyContent {}

/// Trait implemented by Codec values that can be used
/// as packet contents
pub trait PacketContent: Codec + Debug {}

/// Trait for implementing packet target details
pub trait PacketComponent: Debug + Eq + PartialEq {
    fn command(&self) -> u16;

    fn from_value(value: u16, notify: bool) -> Self;
}

pub trait PacketComponents: Debug + Eq + PartialEq {
    fn values(&self) -> (u16, u16);

    fn from_values(component: u16, command: u16, notify: bool) -> Self;
}

/// The different types of packets
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Structure of packet header which comes before the
/// packet content and describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
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
}

impl PacketHeader {
    /// Encodes a packet header with the provided length value
    pub fn encode_bytes(&self, length: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(12);
        let is_extended = length > 0xFFFF;
        encode_u16(&(length as u16), &mut header);
        encode_u16(&self.component, &mut header);
        encode_u16(&self.command, &mut header);
        encode_u16(&self.error, &mut header);
        header.push((self.ty.value() >> 8) as u8);
        header.push(if is_extended { 0x10 } else { 0x00 });
        encode_u16(&self.id, &mut header);
        if is_extended {
            header.push(((length & 0xFF000000) >> 24) as u8);
            header.push(((length & 0x00FF0000) >> 16) as u8);
        }
        header
    }

    /// Reads a packet header from the provided input as well as
    /// the length of the content
    pub fn read<R: Read>(input: &mut R) -> PacketResult<(PacketHeader, usize)>
    where
        Self: Sized,
    {
        let mut header = [0u8; 12];
        input.read_exact(&mut header)?;
        let mut length = decode_u16(&header[0..2])? as usize;
        let component = decode_u16(&header[2..4])?;
        let command = decode_u16(&header[4..6])?;
        let error = decode_u16(&header[6..8])?;
        let q_type = decode_u16(&header[8..10])?;
        let id = decode_u16(&header[10..12])?;
        if q_type & 0x10 != 0 {
            let mut buffer = [0; 2];
            input.read_exact(&mut buffer)?;
            let ext_length = u16::from_be_bytes(buffer);
            length += (ext_length as usize) << 16;
        }
        let ty = PacketType::from_value(q_type);
        let header = PacketHeader {
            component,
            command,
            error,
            ty,
            id,
        };
        Ok((header, length))
    }

    #[cfg(feature = "async")]
    pub async fn read_async<R: AsyncRead>(input: &mut R) -> PacketResult<(PacketHeader, usize)>
    where
        Self: Sized,
    {
        let mut header = [0u8; 12];
        input.read_exact(&mut header).await?;
        let mut length = decode_u16(&header[0..2])? as usize;
        let component = decode_u16(&header[2..4])?;
        let command = decode_u16(&header[4..6])?;
        let error = decode_u16(&header[6..8])?;
        let q_type = decode_u16(&header[8..10])?;
        let id = decode_u16(&header[10..12])?;
        if q_type & 0x10 != 0 {
            let mut buffer = [0; 2];
            input.read_exact(&mut buffer).await?;
            let ext_length = u16::from_be_bytes(buffer);
            length += (ext_length as usize) << 16;
        }
        let ty = PacketType::from_value(q_type);
        let header = PacketHeader {
            component,
            command,
            error,
            ty,
            id,
        };
        Ok((header, length))
    }
}

/// Structure for a packet created by ourselves where
/// the data contents are already known and not encoded
#[derive(Debug)]
pub struct Packet<C: PacketContent>(pub PacketHeader, pub C);

/// Structure for storing functions related to creation of packets
pub struct Packets {}

impl Packets {
    /// Creates a new response packet for responding to the provided
    /// decodable packet. With the `contents`
    pub fn response<C: PacketContent>(packet: &OpaquePacket, contents: C) -> Packet<C> {
        let mut header = packet.0.clone();
        header.ty = PacketType::Response;
        Packet(header, contents)
    }

    /// Shortcut function for creating a response packet with no content
    #[inline]
    pub fn response_empty(packet: &OpaquePacket) -> Packet<EmptyContent> {
        Self::response(packet, EmptyContent {})
    }

    /// Creates a new error response packet for responding to the
    /// provided packet with an error number with `contents`
    pub fn error<C: PacketContent>(
        packet: &OpaquePacket,
        error: impl Into<u16>,
        contents: C,
    ) -> Packet<C> {
        let mut header = packet.0.clone();
        header.error = error.into();
        header.ty = PacketType::Error;
        Packet(header, contents)
    }

    /// Shortcut function for creating an error packet with no content
    #[inline]
    pub fn error_empty(packet: &OpaquePacket, error: impl Into<u16>) -> Packet<EmptyContent> {
        Self::error(packet, error, EmptyContent {})
    }

    /// Creates a new notify packet with the provided component and command
    /// and `contents`
    pub fn notify<C: PacketContent, T: PacketComponents>(component: T, contents: C) -> Packet<C> {
        let (component, command) = component.values();
        Packet(
            PacketHeader {
                component,
                command,
                error: 0,
                ty: PacketType::Notify,
                id: 0,
            },
            contents,
        )
    }
    /// Shortcut function for creating a notify packet with no content
    #[inline]
    pub fn notify_empty<T: PacketComponents>(component: T) -> Packet<EmptyContent> {
        Self::notify(component, EmptyContent {})
    }

    /// Creates a new request packet retrieving its ID from the provided
    /// request counter.
    pub fn request<R: RequestCounter, C: PacketContent, T: PacketComponents>(
        counter: &mut R,
        component: T,
        contents: C,
    ) -> Packet<C> {
        let (component, command) = component.values();
        Packet(
            PacketHeader {
                component,
                command,
                error: 0,
                ty: PacketType::Request,
                id: counter.next(),
            },
            contents,
        )
    }
}

impl<C: PacketContent> Packet<C> {
    /// Reads a packet from the provided input and parses the
    /// contents
    pub fn read<R: Read>(input: &mut R) -> PacketResult<Packet<C>>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read(input)?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents)?;
        let mut reader = Reader::new(&contents);
        let contents = C::decode(&mut reader)?;
        Ok(Packet(header, contents))
    }

    /// Reads a packet from the provided input and parses the
    /// contents
    pub fn read_typed<T: PacketComponents, R: Read>(input: &mut R) -> PacketResult<(T, C)>
    where
        Self: Sized,
    {
        let packet = Self::read(input)?;
        let header = packet.0;

        let t = T::from_values(
            header.component,
            header.component,
            matches!(header.ty, PacketType::Notify),
        );
        Ok((t, packet.1))
    }

    /// Reads a packet from the provided input and parses the
    /// contents
    #[cfg(feature = "async")]
    pub async fn read_async<R: AsyncRead>(input: &mut R) -> PacketResult<Packet<C>>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_async(input).await?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents).await?;
        let mut reader = Reader::new(&contents);
        let contents = C::decode(&mut reader)?;
        Ok(Packet(header, contents))
    }

    #[cfg(feature = "async")]
    pub async fn read_typed_async<T: PacketComponents, R: AsyncRead>(
        input: &mut R,
    ) -> PacketResult<(T, C)>
    where
        Self: Sized,
    {
        let packet = Self::read_async(input).await?;
        let header = packet.0;

        let t = T::from_values(
            header.component,
            header.component,
            matches!(header.ty, PacketType::Notify),
        );
        Ok((t, packet.1))
    }

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    pub fn write<W: Write>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = self.1.encode_bytes();
        let header = self.0.encode_bytes(content.len());
        output.write_all(&header)?;
        output.write_all(&content)?;
        Ok(())
    }

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    #[cfg(feature = "async")]
    pub async fn write_async<W: AsyncWrite>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = self.1.encode_bytes();
        let header = self.0.encode_bytes(content.len());
        output.write_all(&header).await?;
        output.write_all(&content).await?;
        Ok(())
    }
}

impl<C: PacketContent> TryInto<Packet<C>> for OpaquePacket {
    type Error = CodecError;

    fn try_into(self) -> Result<Packet<C>, Self::Error> {
        let contents = self.contents::<C>()?;
        Ok(Packet(self.0, contents))
    }
}

/// Structure for packets that have been read where the contents
/// are not know and are encoded as a vector of bytes.
#[derive(Debug)]
pub struct OpaquePacket(pub PacketHeader, pub Vec<u8>);

impl OpaquePacket {
    /// Reads the contents of this encoded packet and tries to decode
    /// the `R` from it.
    pub fn contents<R: PacketContent>(&self) -> CodecResult<R> {
        let mut reader = Reader::new(&self.1);
        R::decode(&mut reader)
    }

    /// Debug decoding decodes self printing all the hit nodes
    pub fn debug_decode(&self) -> CodecResult<()> {
        let mut reader = Reader::new(&self.1);
        Tag::debug_discard(&mut reader)?;
        Ok(())
    }

    /// Reads a packet from the provided input without parsing
    /// the contents of the packet
    pub fn read<R: Read>(input: &mut R) -> PacketResult<Self>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read(input)?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents)?;
        Ok(Self(header, contents))
    }

    pub fn read_typed<R: Read, T: PacketComponents>(input: &mut R) -> PacketResult<(T, Self)>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read(input)?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents)?;
        let component = T::from_values(
            *&header.component,
            *&header.command,
            matches!(&header.ty, PacketType::Notify),
        );
        Ok((component, Self(header, contents)))
    }

    /// Reads a packet from the provided input without parsing
    /// the contents of the packet
    #[cfg(feature = "async")]
    pub async fn read_async<R: AsyncRead>(input: &mut R) -> PacketResult<Self>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_async(input).await?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents).await?;
        Ok(Self(header, contents))
    }

    /// Reads a packet from the provided input without parsing
    /// the contents of the packet
    #[cfg(feature = "async")]
    pub async fn read_async_typed<R: AsyncRead, T: PacketComponents>(
        input: &mut R,
    ) -> PacketResult<(T, Self)>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_async(input).await?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents).await?;
        let component = T::from_values(
            *&header.component,
            *&header.command,
            matches!(&header.ty, PacketType::Notify),
        );
        Ok((component, Self(header, contents)))
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

#[cfg(test)]
mod test {
    use crate::packet::{OpaquePacket, Packet, Packets};
    use crate::types::VarInt;
    use crate::{define_components, packet};
    use std::io::Cursor;

    packet! {
        struct Test {
            TEST: String,
            ALT: VarInt,
            AA: u32,
        }
    }

    define_components! {
        Authentication (0x0) {
            First (0x1)
            Second (0x2)
            Third (0x3)
        }

        Other (0x1) {
            First (0x1)
            Second (0x2)
            Third (0x3)
        }
    }

    #[test]
    fn test() {
        let contents = Test {
            TEST: String::from("Test"),
            ALT: VarInt(0),
            AA: 32,
        };
        println!("{:?}", contents);
        let packet = Packets::notify(components::Authentication::Second, contents);
        println!("{packet:?}");

        let mut out = Cursor::new(Vec::new());
        packet.write(&mut out).unwrap();

        let bytes = out.get_ref();
        println!("{bytes:?}");
        let mut bytes_in = Cursor::new(bytes);

        let packet_in = OpaquePacket::read(&mut bytes_in).unwrap();
        println!("{packet_in:?}");
        let packet_in_dec: Packet<Test> = packet_in.try_into().unwrap();
        println!("{packet_in_dec:?}");

        assert_eq!(packet.0, packet_in_dec.0)
    }
}
