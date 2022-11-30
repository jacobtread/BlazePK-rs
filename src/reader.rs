use crate::codec::Reader;

/// Buffered readable implementation. Allows reading through the
/// underlying slice using a cursor and with a position that can
/// be saved usin the marker.
pub struct TdfReader<'a> {
    /// The underlying buffer to read from
    buffer: &'a [u8],
    /// The cursor position on the buffer
    cursor: usize,
    /// The last marked position to return to
    marker: usize,
}

impl TdfReader<'_> {
    pub fn new(buffer: &[u8]) -> Self {
        Self {
            buffer,
            cursor: 0,
            marker: 0,
        }
    }

    pub fn slice(&mut self, length: usize) -> CodecRes
}
