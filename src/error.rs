use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug)]
pub enum TdfError {
    IOError(io::Error),
    InvalidMapSize,
    UnknownType(u8)
}

impl From<io::Error> for TdfError {
    fn from(err: io::Error) -> Self {
        TdfError::IOError(err)
    }
}

impl Display for TdfError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TdfError::IOError(_) => { f.write_str("io error")?; }
            TdfError::InvalidMapSize => { f.write_str("map key and value lengths didn't match")?; }
            TdfError::UnknownType(ty) => {
                f.write_str("Unknown tdf type value")?;
                f.write_str(ty.to_string().as_str())?;
            }
        }
        Ok(())
    }
}