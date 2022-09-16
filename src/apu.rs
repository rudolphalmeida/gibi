use crate::memory::Memory;
use crate::utils::{Byte, Word};

pub const SOUND_START: Word = 0xFF10;
pub const SOUND_END: Word = 0xFF26;
pub const WAVE_START: Word = 0xFF30;
pub const WAVE_END: Word = 0xFF3F;

pub(crate) struct Apu {}

impl Apu {
    pub fn new() -> Self {
        Apu {}
    }

    pub fn tick(&mut self) {}
}

impl Memory for Apu {
    fn read(&self, _address: Word) -> Byte {
        0xFF
    }

    fn write(&mut self, _address: Word, _data: Byte) {}
}
