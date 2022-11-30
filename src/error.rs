use crate::tag::TdfType;

/// Error type for errors that can occur while decoding a value
/// using the tdf decode
pub enum DecodeError {
    /// The tag that was expected could not be found
    MissingTag {
        /// The tag that was being searched for
        tag: String,
        /// The type of the tag being searched for
        ty: TdfType,
    },
    /// The found tag was not of the correct type
    InvalidTagType {
        /// The tag which the type was invalid for
        tag: String,
        /// The expected tdf type
        expected: TdfType,
        /// The actual tdf type
        actual: TdfType,
    },
    /// Encountered an unexpected type when decoding a
    /// map or list
    InvalidType {
        /// The expected tdf type
        expected: TdfType,
        /// The actual tdf type
        actual: TdfType,
    },
    /// Reached the end of the available bytes before
    /// a value could be obtained
    UnexpectedEof {
        /// The current reader cusor position
        cursor: usize,
        /// The number of bytes attempted to read
        wanted: usize,
        /// The remaining bytes in the reader slice
        remaining: usize,
    },
}

/// Type alias
pub type DecodeResult<T> = Result<T, DecodeError>;
