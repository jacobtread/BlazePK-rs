extern crate core;

use crate::error::{TdfResult,TdfError};
use crate::tdf::{Tdf, TdfValue};
use blaze_pk_derive::TdfStruct;

pub mod tdf;
pub mod types;
pub mod io;
pub mod error;
pub mod packet;

// Struct for use with proc macro to generate struct serialization
pub trait TdfStruct {
    /// Function for serializing self as vec of Tdf's
    fn serialize(&self) -> TdfResult<Vec<Tdf>>;

    /// Function for deserializing vec of Tdf's into self
    fn deserialize(contents: Vec<Tdf>) -> TdfResult<Self> where Self: Sized;
}

#[derive(TdfStruct)]
struct TdfTest {
    #[tag("TEST")] name: String,
    #[tag("TEST")] v: u8,
    #[tag("TEST")] a: u16,
    #[tag("TEST")] b: bool,
    #[tag("ALT")] c: Other,
}

#[derive(TdfStruct)]
struct Other {
    #[tag("TEST")]
    d: Vec<u8>,
    #[tag("TEST")]
    f: Vec<u64>,
}

pub fn test_compose() {

    let contents = vec![
        Tdf::new("TEST", TdfValue::VarInt(1)),

    ]

}