use crate::codec::{decode_u16_be, encode_u16_be, Codec, CodecResult, Reader};
#[cfg(feature = "blaze-ssl")]
use blaze_ssl_async::stream::BlazeStream;
use bytes::Bytes;
use std::fmt::Debug;
use std::io;
#[cfg(feature = "sync")]
use std::io::{Read, Write};
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Trait for implementing packet target details
pub trait PacketComponent: Debug + Eq + PartialEq {
    fn command(&self) -> u16;

    fn from_value(value: u16, notify: bool) -> Self;
}

/// Trait implemented by packet components for converting them into
/// values and finding values from components
pub trait PacketComponents: Debug + Eq + PartialEq + Sized {
    /// Converts the packet component into the ID of the
    /// component, and command
    fn values(&self) -> (u16, u16);

    /// Decodes the packet component using the provided component id,
    /// command id, and whether the packet is a notify packet
    fn from_values(component: u16, command: u16, notify: bool) -> Self;

    /// Decodes the packet component using the details stored in the provided
    /// packet header
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
    pub fn notify(component: u16, command: u16) -> Self {
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
    /// `component` The component to use
    /// `command`   The command to use
    pub fn request(id: u16, component: u16, command: u16) -> Self {
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
    pub fn response(&self) -> Self {
        self.with_type(PacketType::Response)
    }

    /// Copies the header contents changing its Packet Type
    ///
    /// `ty` The new packet type
    pub fn with_type(&self, ty: PacketType) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error: self.error,
            ty,
            id: self.id,
        }
    }

    /// Copies the header contents changing its Packet Type
    pub fn with_error(&self, error: u16) -> Self {
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

    /// Asyncronously reads a packet header from the provided BlazeStream from
    /// the blaze-ssl system. Returning the Packet header as well as the length
    /// of the packet.
    ///
    /// `input` The input blaze stream to read from
    #[cfg(feature = "blaze-ssl")]
    pub async fn read_blaze<R: AsyncRead + AsyncWrite + Unpin>(
        input: &mut BlazeStream<R>,
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
/// and the header for identification
#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
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
    pub fn raw_empty(header: PacketHeader) -> Self {
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
    pub fn response<C: Codec>(packet: &Packet, contents: C) -> Self {
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
    pub fn respond<C: Codec>(&self, contents: C) -> Self {
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
    pub fn response_empty(packet: &Packet) -> Self {
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
    pub fn respond_empty(&self) -> Self {
        Self::response_empty(self)
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The response contents
    pub fn error<C: Codec>(packet: &Packet, error: u16, contents: C) -> Self {
        Self {
            header: packet.header.with_error(error.into()),
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
    pub fn respond_error<C: Codec>(&self, error: u16, contents: C) -> Self {
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
            header: packet.header.with_error(error.into()),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error with empty contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    #[inline]
    pub fn error_empty(packet: &Packet, error: u16) -> Packet {
        Self {
            header: packet.header.with_error(error.into()),
            contents: Bytes::new(),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error with empty contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    #[inline]
    pub fn respond_error_empty(&self, error: u16) -> Packet {
        Self::error_empty(self, error)
    }

    /// Creates a notify packet for the provided component with the
    /// provided contents.
    ///
    /// `component` The packet component to use for the header
    /// `contents`  The contents of the packet to encode
    pub fn notify<C: Codec, T: PacketComponents>(component: T, contents: C) -> Packet {
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
    pub fn request<C: Codec, T: PacketComponents>(id: u16, component: T, contents: C) -> Packet {
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
    pub fn decode<C: Codec>(&self) -> CodecResult<C> {
        let mut reader = Reader::new(&self.contents);
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

    /// Asyncronously reads a packet from the provided blaze ssl input stream
    /// returning the packet that was read.
    ///
    /// `input` The input source to read from
    #[cfg(feature = "blaze-ssl")]
    pub async fn read_blaze<R: AsyncRead + AsyncWrite + Unpin>(
        input: &mut BlazeStream<R>,
    ) -> io::Result<Self>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_blaze(input).await?;
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

    /// Reads a packet from the provided input without parsing
    /// the contents of the packet
    #[cfg(feature = "blaze-ssl")]
    pub async fn read_blaze_typed<T: PacketComponents, R: AsyncRead + AsyncWrite + Unpin>(
        input: &mut BlazeStream<R>,
    ) -> io::Result<(T, Self)>
    where
        Self: Sized,
    {
        let (header, length) = PacketHeader::read_blaze(input).await?;
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

    /// Handles writing the header and contents of this packet to
    /// the Writable object
    #[cfg(feature = "blaze-ssl")]
    pub fn write_blaze<W: AsyncRead + AsyncWrite + Unpin>(
        &self,
        output: &mut BlazeStream<W>,
    ) -> io::Result<()>
    where
        Self: Sized,
    {
        let content = &self.contents;
        let header = self.header.encode_bytes(content.len());
        output.write(&header)?;
        output.write(content)?;
        Ok(())
    }

    /// Appends the header and contents of this packet to the provided output
    /// Vec of bytes.
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

#[cfg(test)]
mod test {
    use crate::packet::Packet;
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
        let packet = Packet::notify(Components::Authentication(Authentication::Second), contents);
        println!("{packet:?}");

        let mut out = Cursor::new(Vec::new());
        packet.write(&mut out).unwrap();

        let bytes = out.get_ref();
        println!("{bytes:?}");
        let mut bytes_in = Cursor::new(bytes);

        let packet_in = Packet::read(&mut bytes_in).unwrap();
        println!("{packet_in:?}");
        let packet_in_dec: Test = packet_in.decode().unwrap();
        println!("{packet_in_dec:?}");
    }
}
