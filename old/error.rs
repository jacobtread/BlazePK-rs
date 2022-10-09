use std::fmt::{Display, Formatter};
use std::io;

pub type TdfResult<T> = Result<T, TdfError>;
pub type EmptyTdfResult = TdfResult<()>;

/// Enum for errors that occur while reading or writing Tdf's
#[derive(Debug)]
pub enum TdfError {
    // IO Errors
    IOError(io::Error),
    // Map key and values lengths don't match (can't encode)
    InvalidMapSize,
    // Unknown tdf type value
    UnknownType(u8),
    DeserializeError,
}

/// Implement display for all the tdf error values
impl Display for TdfError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TdfError::IOError(err) =>
                f.write_str(&format!("IO Error: {}", err)),
            TdfError::InvalidMapSize =>
                f.write_str("Map key and value lengths don't match"),
            TdfError::UnknownType(ty) =>
                f.write_str(&format!("Unknown Tdf type {}", ty)),
            TdfError::DeserializeError =>
                f.write_str("Failed to deserialize")
        }
    }
}

/// Implement from io::Error for converting to TdfError when using ?
/// operator to simplify code
impl From<io::Error> for TdfError {
    fn from(err: io::Error) -> Self {
        TdfError::IOError(err)
    }
}

