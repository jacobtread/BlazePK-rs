use crate::codec::{decode_u16_be, encode_u16_be, Codec, CodecError, CodecResult, Reader};
use std::fmt::Debug;
use std::io;
use std::io::{Read, Write};

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
    /// Checks if the component and command of this packet header matches
    /// that of the other packet header
    pub fn path_matches(&self, other: &PacketHeader) -> bool {
        self.component.eq(&other.component) && self.command.eq(&other.command)
    }

    /// Encodes the header writing its bytes to the provided output
    /// Vec
    pub fn write_bytes(&self, length: usize, output: &mut Vec<u8>) {
        let is_extended = length > 0xFFFF;
        encode_u16_be(&(length as u16), output);
        encode_u16_be(&self.component, output);
        encode_u16_be(&self.command, output);
        encode_u16_be(&self.error, output);
        output.push((self.ty.value() >> 8) as u8);
        output.push(if is_extended { 0x10 } else { 0x00 });
        encode_u16_be(&self.id, output);
        if is_extended {
            output.push(((length & 0xFF000000) >> 24) as u8);
            output.push(((length & 0x00FF0000) >> 16) as u8);
        }
    }

    /// Encodes a packet header with the provided length value
    pub fn encode_bytes(&self, length: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(12);
        self.write_bytes(length, &mut header);
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
        let mut length = decode_u16_be(&header[0..2])? as usize;
        let component = decode_u16_be(&header[2..4])?;
        let command = decode_u16_be(&header[4..6])?;
        let error = decode_u16_be(&header[6..8])?;
        let q_type = decode_u16_be(&header[8..10])?;
        let id = decode_u16_be(&header[10..12])?;
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
    pub async fn read_async<R: AsyncRead + Unpin>(
        input: &mut R,
    ) -> PacketResult<(PacketHeader, usize)>
    where
        Self: Sized,
    {
        let mut header = [0u8; 12];
        input.read_exact(&mut header).await?;
        let mut length = decode_u16_be(&header[0..2])? as usize;
        let component = decode_u16_be(&header[2..4])?;
        let command = decode_u16_be(&header[4..6])?;
        let error = decode_u16_be(&header[6..8])?;
        let q_type = decode_u16_be(&header[8..10])?;
        let id = decode_u16_be(&header[10..12])?;
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
pub struct Packet<C: Codec>(pub PacketHeader, pub C);

/// Structure for storing functions related to creation of packets
pub struct Packets {}

impl Packets {
    /// Creates a new response packet for responding to the provided
    /// decodable packet. With the `contents`
    pub fn response<C: Codec>(packet: &OpaquePacket, contents: &C) -> OpaquePacket {
        let mut header = packet.0.clone();
        header.ty = PacketType::Response;
        OpaquePacket(header, contents.encode_bytes())
    }

    pub fn response_raw(packet: &OpaquePacket, contents: Vec<u8>) -> OpaquePacket {
        let mut header = packet.0.clone();
        header.ty = PacketType::Response;
        OpaquePacket(header, contents)
    }

    /// Shortcut function for creating a response packet with no content
    #[inline]
    pub fn response_empty(packet: &OpaquePacket) -> OpaquePacket {
        let mut header = packet.0.clone();
        header.ty = PacketType::Response;
        OpaquePacket(header, Vec::with_capacity(0))
    }

    /// Creates a new error response packet for responding to the
    /// provided packet with an error number with `contents`
    pub fn error<C: Codec>(
        packet: &OpaquePacket,
        error: impl Into<u16>,
        contents: &C,
    ) -> OpaquePacket {
        let mut header = packet.0.clone();
        header.error = error.into();
        header.ty = PacketType::Error;
        OpaquePacket(header, contents.encode_bytes())
    }

    /// Shortcut function for creating an error packet with no content
    #[inline]
    pub fn error_empty(packet: &OpaquePacket, error: impl Into<u16>) -> OpaquePacket {
        let mut header = packet.0.clone();
        header.error = error.into();
        header.ty = PacketType::Error;
        OpaquePacket(header, Vec::with_capacity(0))
    }

    /// Creates a new notify packet with the provided component and command
    /// and `contents`
    pub fn notify<C: Codec, T: PacketComponents>(component: T, contents: &C) -> OpaquePacket {
        let (component, command) = component.values();
        OpaquePacket(
            PacketHeader {
                component,
                command,
                error: 0,
                ty: PacketType::Notify,
                id: 0,
            },
            contents.encode_bytes(),
        )
    }

    pub fn notify_raw<T: PacketComponents>(component: T, contents: Vec<u8>) -> OpaquePacket {
        let (component, command) = component.values();
        OpaquePacket(
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
    pub fn notify_empty<T: PacketComponents>(component: T) -> OpaquePacket {
        let (component, command) = component.values();
        let header = PacketHeader {
            component,
            command,
            error: 0,
            ty: PacketType::Notify,
            id: 0,
        };
        OpaquePacket(header, Vec::with_capacity(0))
    }

    /// Creates a new request packet retrieving its ID from the provided
    /// request counter.
    pub fn request<C: Codec, T: PacketComponents>(
        id: u16,
        component: T,
        contents: &C,
    ) -> OpaquePacket {
        let (component, command) = component.values();
        OpaquePacket(
            PacketHeader {
                component,
                command,
                error: 0,
                ty: PacketType::Request,
                id,
            },
            contents.encode_bytes(),
        )
    }

    /// Creates a new request packet retrieving its ID from the provided
    /// request counter.
    pub fn request_empty<T: PacketComponents>(id: u16, component: T) -> OpaquePacket {
        let (component, command) = component.values();
        OpaquePacket(
            PacketHeader {
                component,
                command,
                error: 0,
                ty: PacketType::Request,
                id,
            },
            Vec::with_capacity(0),
        )
    }
}

impl<C: Codec> Packet<C> {
    /// Converts this packet into an opaque packet
    pub fn opaque(self) -> OpaquePacket {
        let contents = self.1.encode_bytes();
        OpaquePacket(self.0, contents)
    }

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
    pub async fn read_async<R: AsyncRead + Unpin>(input: &mut R) -> PacketResult<Packet<C>>
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
    pub async fn read_typed_async<T: PacketComponents, R: AsyncRead + Unpin>(
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
    pub async fn write_async<W: AsyncWrite + Unpin>(&self, output: &mut W) -> io::Result<()>
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

impl<C: Codec> TryInto<Packet<C>> for OpaquePacket {
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
    pub fn contents<R: Codec>(&self) -> CodecResult<R> {
        let mut reader = Reader::new(&self.1);
        R::decode(&mut reader)
    }

    /// Debug decoding decodes self printing all the hit nodes
    pub fn debug_decode(&self) -> CodecResult<String> {
        let mut reader = Reader::new(&self.1);
        let mut out = String::new();
        out.push_str(&format!(
            "packet({:?}, {:?}) {{\n",
            self.0.component, self.0.command
        ));
        Tag::stringify(&mut reader, &mut out, 1)?;
        out.push('}');
        Ok(out)
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
            header.component,
            header.command,
            matches!(&header.ty, PacketType::Notify),
        );
        Ok((component, Self(header, contents)))
    }

    /// Reads a packet from the provided input without parsing
    /// the contents of the packet
    #[cfg(feature = "async")]
    pub async fn read_async<R: AsyncRead + Unpin>(input: &mut R) -> PacketResult<Self>
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
    pub async fn read_async_typed<T: PacketComponents, R: AsyncRead + Unpin>(
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

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    pub fn write<W: Write>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = &self.1;
        let header = self.0.encode_bytes(content.len());
        output.write_all(&header)?;
        output.write_all(content)?;
        Ok(())
    }

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    #[cfg(feature = "async")]
    pub async fn write_async<W: AsyncWrite + Unpin>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = &self.1;
        let header = self.0.encode_bytes(content.len());
        output.write_all(&header).await?;
        output.write_all(content).await?;
        Ok(())
    }

    /// Appends the header and contents of this packet to the provided output
    /// Vec of bytes.
    pub fn write_bytes(&self, output: &mut Vec<u8>) {
        let content = &self.1;
        let length = content.len();
        self.0.write_bytes(length, output);
        output.extend_from_slice(content);
    }

    /// Encodes this packet header and contents into a Vec. Vec may be
    /// over allocated by 2 bytes to prevent reallocation for longer
    /// packets.
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(14 + self.1.len());
        self.write_bytes(&mut output);
        output
    }
}

#[cfg(test)]
mod test {
    use crate::packet::{OpaquePacket, Packet, Packets};
    use crate::{define_components, packet};
    use std::io::Cursor;

    packet! {
        struct Test {
            TEST test: String,
            ALT alt: u32,
            AA aa: u32,
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
            test: String::from("Test"),
            alt: 0,
            aa: 32,
        };
        println!("{:?}", contents);
        let packet = Packets::notify(
            Components::Authentication(Authentication::Second),
            &contents,
        );
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
