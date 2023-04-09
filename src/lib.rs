//! Rust library for working with the Blaze packet system this is the networking solution used by games such as
//! Mass Effect 3, Battlefield 3, another Other EA games.

pub mod codec;
pub mod error;
pub mod packet;
pub mod reader;
pub mod router;
pub mod tag;
pub mod types;
pub mod writer;

/// Serde serialization
#[cfg(feature = "serde")]
pub mod serialize;

/// Re-exports for derive macros
pub use blaze_pk_derive::{PacketComponent, PacketComponents};
