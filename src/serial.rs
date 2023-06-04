use crate::memory::Memory;

pub const SERIAL_START: u16 = 0xFF01;
pub const SERIAL_END: u16 = 0xFF02;

pub(crate) struct Serial {}

impl Serial {
    pub fn new() -> Self {
        Self {}
    }
}

impl Memory for Serial {
    fn read(&self, _address: u16) -> u8 {
        0xFF
    }

    fn write(&mut self, _address: u16, _data: u8) {}
}
