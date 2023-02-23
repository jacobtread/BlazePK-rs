//! Rust implementation of the Blaze packet system
pub mod codec;
pub mod error;
pub mod packet;
pub mod reader;
pub mod router;
pub mod tag;
pub mod types;
pub mod writer;

// Serde serialization
#[cfg(feature = "serde")]
pub mod serialize;

/// Re-exports for derive macros
pub use blaze_pk_derive::{PacketComponent, PacketComponents};
