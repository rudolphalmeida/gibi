use crate::memory::Memory;
use crate::utils::{Byte, Word};

pub const SERIAL_START: Word = 0xFF01;
pub const SERIAL_END: Word = 0xFF02;

pub(crate) struct Serial {}

impl Serial {
    pub fn new() -> Self {
        Self {}
    }
}

impl Memory for Serial {
    fn read(&self, _address: Word) -> Byte {
        log::debug!("Serial not implemented");
        0xFF
    }

    fn write(&mut self, _address: Word, _data: Byte) {
        log::debug!("Serial not implemented");
    }
}
