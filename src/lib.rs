//! Rust implementation of the Blaze packet system
pub mod codec;
pub mod error;
pub mod macros;
pub mod packet;
pub mod reader;
pub mod tag;
pub mod types;
pub mod writer;

#[cfg(feature = "actix")]
pub mod actix;

#[cfg(feature = "serde")]
pub mod serialize;
