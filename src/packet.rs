use crate::{
    codec::{Decodable, Encodable},
    error::DecodeResult,
    reader::TdfReader,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{fmt::Debug, hash::Hash, sync::Arc};
use std::{io, ops::Deref};
use tokio_util::codec::{Decoder, Encoder};

pub trait PacketComponents: Debug + Hash + Eq + Sized {
    /// Converts the packet component into the ID of the
    /// component, and command
    fn values(&self) -> (u16, u16);

    /// Decodes the packet component using the provided component id,
    /// command id, and whether the packet is a notify packet
    ///
    /// `component` The packet component
    /// `command`   The packet command
    /// `notify`    Whether the packet is a notify packet
    fn from_values(component: u16, command: u16, notify: bool) -> Option<Self>;

    /// Decodes the packet component using the details stored in the provided
    /// packet header
    ///
    /// `header` The packet header to decode from
    fn from_header(header: &PacketHeader) -> Option<Self> {
        Self::from_values(
            header.component,
            header.command,
            matches!(&header.ty, PacketType::Notify),
        )
    }
}

/// Trait for implementing packet target details
pub trait PacketComponent: Debug + Hash + Eq + Sized {
    // Converts the component command value into its u16 value
    fn command(&self) -> u16;

    /// Finds a component with the matching value based on whether
    /// the packet is a notify packet or not
    ///
    /// `value`  The component value
    /// `notify` Whether the packet was a notify packet
    fn from_value(value: u16, notify: bool) -> Option<Self>;
}

/// The different types of packets
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// ID counted request packets (0x00)
    Request = 0x0,
    /// Packets responding to requests (0x10)
    Response = 0x1,
    /// Unique packets coming from the server (0x20)
    Notify = 0x2,
    /// Error packets (0x30)
    Error = 0x3,
}

impl PacketType {
    /// Gets the packet type this value is represented by
    ///
    /// `value` The value to get the type for
    pub fn from_value(value: u8) -> PacketType {
        match value {
            0x0 => PacketType::Request,
            0x1 => PacketType::Response,
            0x2 => PacketType::Notify,
            0x3 => PacketType::Error,
            // Default type fallback to request
            _ => PacketType::Request,
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

    /// Encodes the contents of this header appending to the
    /// output source
    ///
    /// `dst`    The dst to append the bytes to
    /// `length` The length of the content after the header
    pub fn write(&self, dst: &mut BytesMut, length: usize) {
        let is_extended = length > 0xFFFF;
        dst.put_u16(length as u16);
        dst.put_u16(self.component);
        dst.put_u16(self.command);
        dst.put_u16(self.error);
        dst.put_u8(self.ty as u8);
        dst.put_u8(if is_extended { 0x10 } else { 0x00 });
        dst.put_u16(self.id);
        if is_extended {
            dst.put_u8(((length & 0xFF000000) >> 24) as u8);
            dst.put_u8(((length & 0x00FF0000) >> 16) as u8);
        }
    }

    /// Attempts to read the packet header from the provided
    /// source bytes returning None if there aren't enough bytes
    ///
    /// `src` The bytes to read from
    pub fn read(src: &mut BytesMut) -> Option<(PacketHeader, usize)> {
        if src.len() < 12 {
            return None;
        }
        let mut length = src.get_u16() as usize;
        let component = src.get_u16();
        let command = src.get_u16();
        let error = src.get_u16();
        let ty = src.get_u8();
        // If we encounter 0x10 here then the packet contains extended length
        // bytes so its longer than a u16::MAX length
        let is_extended = src.get_u8() == 0x10;
        let id = src.get_u16();

        if is_extended {
            // We need another two bytes for the extended length
            if src.len() < 2 {
                return None;
            }
            length += src.get_u16() as usize;
        }

        let ty = PacketType::from_value(ty);
        let header = PacketHeader {
            component,
            command,
            error,
            ty,
            id,
        };
        Some((header, length))
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

    pub fn read(src: &mut BytesMut) -> Option<Self> {
        let (header, length) = PacketHeader::read(src)?;
        if src.len() < length {
            return None;
        }
        let contents = src.split_to(length);
        Some(Self {
            header,
            contents: contents.freeze(),
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        let contents = &self.contents;
        self.header.write(dst, contents.len());
        dst.extend_from_slice(contents);
    }
}

pub struct PacketCodec;

/// Decoder implementation
impl Decoder for PacketCodec {
    type Error = io::Error;
    type Item = Packet;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(Packet::read(src))
    }
}

/// Encoder implementation for owned packets
impl Encoder<Packet> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Encoder implementation for borrowed packets
impl Encoder<&Packet> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: &Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Encoder implementation for arc reference packets
impl Encoder<Arc<Packet>> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Arc<Packet>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Structure wrapping a from request type to include a packet
/// header to allow the response type to be created
pub struct Request<T: FromRequest> {
    // The decoded request type
    pub req: T,
    // The packet header from the request
    pub header: PacketHeader,
}

/// Deref implementation so that the request fields can be
/// directly accessed
impl<T: FromRequest> Deref for Request<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.req
    }
}

impl<T: FromRequest> Request<T> {
    /// Creates a response from the provided response type value
    /// returning a Response structure which can be used as a Route
    /// repsonse
    ///
    /// `res` The into response type implementation
    pub fn response<E>(&self, res: E) -> Response
    where
        E: Encodable,
    {
        Response(Packet {
            header: self.header.response(),
            contents: Bytes::from(res.encode_bytes()),
        })
    }
}

/// Type for route responses that have already been turned into
/// packets usually for lifetime reasons
pub struct Response(Packet);

impl IntoResponse for Response {
    /// Simply provide the already compute response
    fn into_response(self, _req: &Packet) -> Packet {
        self.0
    }
}

impl<T: FromRequest> FromRequest for Request<T> {
    fn from_request(req: &Packet) -> DecodeResult<Self> {
        let inner = T::from_request(req)?;
        let header = req.header;
        Ok(Self { req: inner, header })
    }
}

/// Trait implementing by structures which can be created from a request
/// packet and is used for the arguments on routing functions
pub trait FromRequest: Sized + Send + 'static {
    /// Takes the value from the request returning a decode result of
    /// whether the value could be created
    ///
    /// `req` The request packet
    fn from_request(req: &Packet) -> DecodeResult<Self>;
}

impl<D> FromRequest for D
where
    D: Decodable + Send + 'static,
{
    fn from_request(req: &Packet) -> DecodeResult<Self> {
        req.decode()
    }
}

/// Trait for a type that can be converted into a packet
/// response using the header from the request packet
pub trait IntoResponse: 'static {
    /// Into packet conversion
    fn into_response(self, req: &Packet) -> Packet;
}

/// Empty response implementation for unit types to allow
/// functions to have no return type
impl IntoResponse for () {
    fn into_response(self, req: &Packet) -> Packet {
        req.respond_empty()
    }
}

/// Into response imeplementation for encodable responses
/// which just calls res.respond
impl<E> IntoResponse for E
where
    E: Encodable + 'static,
{
    fn into_response(self, req: &Packet) -> Packet {
        req.respond(self)
    }
}

/// Into response implementation on result turning whichever
/// portion of the result into a response
impl<S, E> IntoResponse for Result<S, E>
where
    S: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self, req: &Packet) -> Packet {
        match self {
            Ok(value) => value.into_response(req),
            Err(value) => value.into_response(req),
        }
    }
}

/// Into response implementation for option type turning
/// None responses into an empty response
impl<S> IntoResponse for Option<S>
where
    S: IntoResponse,
{
    fn into_response(self, req: &Packet) -> Packet {
        match self {
            Some(value) => value.into_response(req),
            None => req.respond_empty(),
        }
    }
}

/// Wrapper over a packet structure to provde debug logging
/// with names resolved for the component
pub struct PacketDebug<'a, C> {
    /// Reference to the packet itself
    pub packet: &'a Packet,
    /// The component derived from the packet header
    pub component: Option<&'a C>,
    /// Decide whether to display the contents of the packet
    pub minified: bool,
}

impl<'a, C> Debug for PacketDebug<'a, C>
where
    C: PacketComponents,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Append basic header information
        let header = &self.packet.header;
        if let Some(component) = self.component {
            writeln!(f, "Component: {:?}", component)?;
        } else {
            writeln!(f, "Component: {:#06x}", header.component)?;
            writeln!(f, "Command: {:#06x}", header.command)?;
        }

        writeln!(f, "Type: {:?}", header.ty)?;

        if !matches!(&header.ty, PacketType::Notify) {
            writeln!(f, "ID: {}", &header.id)?;
        }

        if let PacketType::Error = &header.ty {
            writeln!(f, "Error: {:#06x}", &header.error)?;
        }

        // Skip remaining if the message shouldn't contain its content
        if self.minified {
            return Ok(());
        }

        let mut reader = TdfReader::new(&self.packet.contents);
        let mut out = String::new();

        out.push_str("{\n");

        // Stringify the content or append error instead
        if let Err(err) = reader.stringify(&mut out) {
            writeln!(f, "Content: Content was malformed")?;
            writeln!(f, "Error: {:?}", err)?;
            writeln!(f, "Partial Content: {}", out)?;
            writeln!(f, "Raw: {:?}", &self.packet.contents)?;
            return Ok(());
        }

        if out.len() == 2 {
            // Remove new line if nothing else was appended
            out.pop();
        }

        out.push('}');

        write!(f, "Content: {}", out)
    }
}
