use crate::{
    codec::{decode_u16_be, encode_u16_be, Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
};
use bytes::Bytes;
use std::io;
#[cfg(feature = "sync")]
use std::io::{Read, Write};
use std::{fmt::Debug, hash::Hash};
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Trait for implementing packet target details
pub trait PacketComponent: Debug + Eq + PartialEq {
    // Converts the component command value into its u16 value
    fn command(&self) -> u16;

    /// Finds a component with the matching value based on whether
    /// the packet is a notify packet or not
    ///
    /// `value`  The component value
    /// `notify` Whether the packet was a notify packet
    fn from_value(value: u16, notify: bool) -> Self;
}

/// Trait implemented by packet components for converting them into
/// values and finding values from components
pub trait PacketComponents: Debug + Eq + PartialEq + Sized + Hash {
    /// Converts the packet component into the ID of the
    /// component, and command
    fn values(&self) -> (u16, u16);

    /// Decodes the packet component using the provided component id,
    /// command id, and whether the packet is a notify packet
    ///
    /// `component` The packet component
    /// `command`   The packet command
    /// `notify`    Whether the packet is a notify packet
    fn from_values(component: u16, command: u16, notify: bool) -> Self;

    /// Decodes the packet component using the details stored in the provided
    /// packet header
    ///
    /// `header` The packet header to decode from
    fn from_header(header: &PacketHeader) -> Self {
        Self::from_values(
            header.component,
            header.command,
            matches!(&header.ty, PacketType::Notify),
        )
    }
}

/// The different types of packets
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    ///
    /// `value` The value to get the type for
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    /// Creates a notify header for the provided component and command
    ///
    /// `component` The component to use
    /// `command`   The command to use
    pub const fn notify(component: u16, command: u16) -> Self {
        Self {
            component,
            command,
            error: 0,
            ty: PacketType::Notify,
            id: 0,
        }
    }

    /// Creates a request header for the provided id, component
    /// and command
    ///
    /// `id`        The packet ID
    /// `component` The component to use
    /// `command`   The command to use
    pub const fn request(id: u16, component: u16, command: u16) -> Self {
        Self {
            component,
            command,
            error: 0,
            ty: PacketType::Request,
            id,
        }
    }

    /// Creates a response to the provided packet header by
    /// changing the type of the header
    #[inline]
    pub const fn response(&self) -> Self {
        self.with_type(PacketType::Response)
    }

    /// Copies the header contents changing its Packet Type
    ///
    /// `ty` The new packet type
    pub const fn with_type(&self, ty: PacketType) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error: self.error,
            ty,
            id: self.id,
        }
    }

    /// Copies the header contents changing its Packet Type
    pub const fn with_error(&self, error: u16) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error,
            ty: PacketType::Error,
            id: self.id,
        }
    }

    /// Checks if the component and command of this packet header matches
    /// that of the other packet header
    ///
    /// `other` The packet header to compare to
    pub fn path_matches(&self, other: &PacketHeader) -> bool {
        self.component.eq(&other.component) && self.command.eq(&other.command)
    }

    /// Encodes the contents of this header to bytes appending those bytes
    /// to the provided output source
    ///
    /// `output` The output bytes to append to
    /// `length` The length of the packet contents
    pub fn write_bytes(&self, output: &mut Vec<u8>, length: usize) {
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

    /// Wraps the write_bytes function directly with a header buffer
    /// to encoded directly into bytes without appending to an existing
    /// one
    ///
    /// `length` The length of the packet contents
    pub fn encode_bytes(&self, length: usize) -> Vec<u8> {
        let mut header = Vec::with_capacity(12);
        self.write_bytes(&mut header, length);
        header
    }

    /// Syncronously reads a packet header from the provided input. Returning
    /// the Packet header as well as the length of the packet.
    ///
    /// `input` The input source to read from
    #[cfg(feature = "sync")]
    pub fn read<R: Read>(input: &mut R) -> io::Result<(PacketHeader, usize)>
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

    /// Asyncronously reads a packet header from the provided AsyncRead input.
    /// Returning the Packet header as well as the length of the packet.
    ///
    /// `input` The input to read from
    #[cfg(feature = "async")]
    pub async fn read_async<R: AsyncRead + Unpin>(
        input: &mut R,
    ) -> io::Result<(PacketHeader, usize)>
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

/// Structure for Blaze packets contains the contents of the packet
/// and the header for identification.
///
/// Packets can be cloned with little memory usage increase because
/// the content is stored as Bytes.
#[derive(Debug, Clone)]
pub struct Packet {
    /// The packet header
    pub header: PacketHeader,
    /// The packet encoded byte contents
    pub contents: Bytes,
}

impl Packet {
    /// Creates a packet from its raw components
    ///
    /// `header`   The packet header
    /// `contents` The encoded packet contents
    pub fn raw(header: PacketHeader, contents: Vec<u8>) -> Self {
        Self {
            header,
            contents: Bytes::from(contents),
        }
    }

    /// Creates a packet from its raw components
    /// where the contents are empty
    ///
    /// `header` The packet header
    pub const fn raw_empty(header: PacketHeader) -> Self {
        Self {
            header,
            contents: Bytes::new(),
        }
    }

    /// Creates a packet responding to the provided packet.
    /// Clones the header of the request packet and changes
    /// the type to repsonse
    ///
    /// `packet`   The packet to respond to
    /// `contents` The contents to encode for the packet
    pub fn response<C: Encodable>(packet: &Packet, contents: C) -> Self {
        Self {
            header: packet.header.response(),
            contents: Bytes::from(contents.encode_bytes()),
        }
    }

    /// Creates a packet responding to the current packet.
    /// Clones the header of the request packet and changes
    /// the type to repsonse
    ///
    /// `packet`   The packet to respond to
    /// `contents` The contents to encode for the packet
    #[inline]
    pub fn respond<C: Encodable>(&self, contents: C) -> Self {
        Self::response(self, contents)
    }

    /// Creates a response packet responding to the provided packet
    /// but with raw contents that have already been encoded.
    ///
    /// `packet`   The packet to respond to
    /// `contents` The raw encoded packet contents
    pub fn response_raw(packet: &Packet, contents: Vec<u8>) -> Self {
        Self {
            header: packet.header.response(),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a response packet responding to the provided packet
    /// but with empty contents.
    ///
    /// `packet` The packet to respond to
    pub const fn response_empty(packet: &Packet) -> Self {
        Self {
            header: packet.header.response(),
            contents: Bytes::new(),
        }
    }

    /// Creates a response packet responding to the provided packet
    /// but with empty contents.
    ///
    /// `packet`   The packet to respond to
    /// `contents` The contents to encode for the packet
    #[inline]
    pub const fn respond_empty(&self) -> Self {
        Self::response_empty(self)
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The response contents
    pub fn error<C: Encodable>(packet: &Packet, error: u16, contents: C) -> Self {
        Self {
            header: packet.header.with_error(error),
            contents: Bytes::from(contents.encode_bytes()),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The response contents
    #[inline]
    pub fn respond_error<C: Encodable>(&self, error: u16, contents: C) -> Self {
        Self::error(self, error, contents)
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and raw encoded contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The raw encoded contents
    pub fn error_raw(packet: &Packet, error: u16, contents: Vec<u8>) -> Self {
        Self {
            header: packet.header.with_error(error),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error with empty contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    #[inline]
    pub const fn error_empty(packet: &Packet, error: u16) -> Packet {
        Self {
            header: packet.header.with_error(error),
            contents: Bytes::new(),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error with empty contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    #[inline]
    pub const fn respond_error_empty(&self, error: u16) -> Packet {
        Self::error_empty(self, error)
    }

    /// Creates a notify packet for the provided component with the
    /// provided contents.
    ///
    /// `component` The packet component to use for the header
    /// `contents`  The contents of the packet to encode
    pub fn notify<C: Encodable, T: PacketComponents>(component: T, contents: C) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::notify(component, command),
            contents: Bytes::from(contents.encode_bytes()),
        }
    }

    /// Creates a notify packet for the provided component with the
    /// provided raw encoded contents.
    ///
    /// `component` The packet component
    /// `contents`  The encoded packet contents
    pub fn notify_raw<T: PacketComponents>(component: T, contents: Vec<u8>) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::notify(component, command),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a notify packet for the provided component with
    /// empty contents
    ///
    /// `component` The packet component
    #[inline]
    pub fn notify_empty<T: PacketComponents>(component: T) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::notify(component, command),
            contents: Bytes::new(),
        }
    }

    /// Creates a new request packet from the provided id, component, and contents
    ///
    /// `id`        The packet id
    /// `component` The packet component
    /// `contents`  The packet contents
    pub fn request<C: Encodable, T: PacketComponents>(
        id: u16,
        component: T,
        contents: C,
    ) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::request(id, component, command),
            contents: Bytes::from(contents.encode_bytes()),
        }
    }

    /// Creates a new request packet from the provided id, component
    /// with raw encoded contents
    ///
    /// `id`        The packet id
    /// `component` The packet component
    /// `contents`  The raw encoded contents
    pub fn request_raw<T: PacketComponents>(id: u16, component: T, contents: Vec<u8>) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::request(id, component, command),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a new request packet from the provided id, component
    /// with empty contents
    ///
    /// `id`        The packet id
    /// `component` The packet component
    /// `contents`  The packet contents
    pub fn request_empty<T: PacketComponents>(id: u16, component: T) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::request(id, component, command),
            contents: Bytes::new(),
        }
    }

    /// Attempts to decode the contents bytes of this packet into the
    /// provided Codec type value.
    pub fn decode<C: Decodable>(&self) -> DecodeResult<C> {
        let mut reader = TdfReader::new(&self.contents);
        C::decode(&mut reader)
    }

    /// Syncronously reads a packet from the provided readable input
    /// returning the packet that was read
    ///
    /// `input` The input source to read from
    #[cfg(feature = "sync")]
    pub fn read<R: Read>(input: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read(input)?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents)?;
        Ok(Self {
            header,
            contents: Bytes::from(contents),
        })
    }

    /// Asyncronously reads a packet from the provided input returning
    /// the packet that was read.
    ///
    /// `input` The input source to read from
    #[cfg(feature = "async")]
    pub async fn read_async<R: AsyncRead + Unpin>(input: &mut R) -> io::Result<Self>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_async(input).await?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents).await?;
        Ok(Self {
            header,
            contents: Bytes::from(contents),
        })
    }

    /// Syncronously reads a packet from the provided readable input
    /// returning the packet that was read along and also decodes
    /// the component using the provided `T` and returns that aswell
    ///
    /// `input` The input source to read from
    #[cfg(feature = "sync")]
    pub fn read_typed<T: PacketComponents, R: Read>(input: &mut R) -> io::Result<(T, Self)>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read(input)?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents)?;
        let component = T::from_header(&header);
        Ok((
            component,
            Self {
                header,
                contents: Bytes::from(contents),
            },
        ))
    }

    /// Reads a packet from the provided input without parsing
    /// the contents of the packet
    ///
    /// `input` The input source to read from
    #[cfg(feature = "async")]
    pub async fn read_async_typed<T: PacketComponents, R: AsyncRead + Unpin>(
        input: &mut R,
    ) -> io::Result<(T, Self)>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_async(input).await?;
        let mut contents = vec![0u8; length];
        input.read_exact(&mut contents).await?;
        let component = T::from_header(&header);
        Ok((
            component,
            Self {
                header,
                contents: Bytes::from(contents),
            },
        ))
    }

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    ///
    /// `output` The output source to write to
    #[cfg(feature = "sync")]
    pub fn write<W: Write>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let contents = &self.contents;
        let header = self.header.encode_bytes(contents.len());
        output.write_all(&header)?;
        output.write_all(contents)?;
        Ok(())
    }

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    ///
    /// `output` The output source to write to
    #[cfg(feature = "async")]
    pub async fn write_async<W: AsyncWrite + Unpin>(&self, output: &mut W) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = &self.contents;
        let header = self.header.encode_bytes(content.len());
        output.write_all(&header).await?;
        output.write_all(content).await?;
        Ok(())
    }

    /// Appends the header and contents of this packet to the provided output
    /// Vec of bytes.
    ///
    /// `output` The output vec to append the bytes to
    pub fn write_bytes(&self, output: &mut Vec<u8>) {
        let content = &self.contents;
        let length = content.len();
        self.header.write_bytes(output, length);
        output.extend_from_slice(content);
    }

    /// Encodes this packet header and contents into a Vec. Vec may be
    /// over allocated by 2 bytes to prevent reallocation for longer
    /// packets.
    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(14 + self.contents.len());
        self.write_bytes(&mut output);
        output
    }
}

/// Trait for a type that can be converted into a packet
/// response using the provided req packet
pub trait IntoResponse {
    /// Into packet conversion
    fn into_response(self, req: Packet) -> Packet;
}

impl<E> IntoResponse for E
where
    E: Encodable,
{
    fn into_response(self, req: Packet) -> Packet {
        req.respond(self)
    }
}

impl<S, E> IntoResponse for Result<S, E>
where
    S: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self, req: Packet) -> Packet {
        match self {
            Ok(value) => value.into_response(req),
            Err(value) => value.into_response(req),
        }
    }
}

impl<S> IntoResponse for Option<S>
where
    S: IntoResponse,
{
    fn into_response(self, req: Packet) -> Packet {
        match self {
            Some(value) => value.into_response(req),
            None => req.respond_empty(),
        }
    }
}
