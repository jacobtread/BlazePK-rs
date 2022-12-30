//! Rust implementation of the Blaze packet system
pub mod codec;
pub mod error;
mod macros;
pub mod packet;
pub mod reader;
pub mod router;
pub mod tag;
pub mod types;
pub mod writer;

#[cfg(feature = "serde")]
pub mod serialize;
