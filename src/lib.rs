//! Rust implementation of the Blaze packet system
//!
//! See README for usage
mod codec;
pub mod macros;
mod packet;
mod tag;
mod types;

pub use codec::{Codec, CodecError, CodecResult, Reader};
pub use packet::{
    AtomicCounter, OpaquePacket, Packet, PacketComponent, PacketComponents, PacketContent,
    PacketError, PacketResult, PacketType, Packets, RequestCounter, SimpleCounter,
};
pub use tag::{Tag, ValueType};
pub use types::{Blob, Listable, TdfMap, TdfOptional, VarInt, VarIntList, EMPTY_OPTIONAL};
