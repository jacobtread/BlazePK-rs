/// Writer implementation for writing values to an underlying buffer
#[derive(Default)]
pub struct TdfWriter {
    /// The buffer that will be written to
    pub buffer: Vec<u8>,
}

impl TdfWriter {
    //
    pub fn write_byte(&mut self, value: u8) {
        self.buffer.push(value)
    }
}
