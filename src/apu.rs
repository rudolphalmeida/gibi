use crate::interrupts::InterruptHandler;
use crate::memory::Memory;
use crate::SystemState;

pub const SOUND_START: u16 = 0xFF10;
pub const SOUND_END: u16 = 0xFF26;
pub const WAVE_START: u16 = 0xFF30;
pub const WAVE_END: u16 = 0xFF3F;

pub(crate) struct Apu {}

impl Apu {
    pub fn new() -> Self {
        Apu {}
    }

    pub fn tick(&mut self, _system_state: &mut SystemState, _interrupts: &mut InterruptHandler) {
        // The APU is not affected by CGB double speed mode
    }
}

impl Memory for Apu {
    fn read(&mut self, _address: u16) -> u8 {
        0xFF
    }

    fn write(&mut self, _address: u16, _data: u8) {}
}
