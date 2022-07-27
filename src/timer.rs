use crate::memory::Memory;
use crate::utils::{Byte, Word};

pub const TIMER_START: Word = 0xFF04;
pub const TIMER_END: Word = 0xFF07;

pub(crate) struct Timer {}

impl Timer {
    pub fn new() -> Self {
        Timer {}
    }

    pub fn tick(&self) {}
}

impl Memory for Timer {
    fn read(&self, _address: Word) -> Byte {
        log::error!("Timer not implemented");
        0xFF
    }

    fn write(&mut self, _address: Word, _data: Byte) {
        log::error!("Timer not implemented");
    }
}
