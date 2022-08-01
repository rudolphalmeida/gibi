use crate::memory::Memory;
use crate::utils::{Byte, Word};

pub(crate) const INTERRUPT_FLAG_ADDRESS: Word = 0xFF0F;
pub(crate) const INTERRUPT_ENABLE_ADDRESS: Word = 0xFFFF;

#[derive(Debug, Copy, Clone)]
pub(crate) enum InterruptType {
    Vblank = 1,
    LcdStat = (1 << 1),
    Timer = (1 << 2),
    Serial = (1 << 3),
    Joypad = (1 << 4),
}

impl InterruptType {
    pub fn from_index(interrupt_index: u32) -> Self {
        match interrupt_index {
            0 => InterruptType::Vblank,
            1 => InterruptType::LcdStat,
            2 => InterruptType::Timer,
            3 => InterruptType::Serial,
            4 => InterruptType::Joypad,
            _ => panic!("Impossible interrupt index {}", interrupt_index),
        }
    }
}

pub(crate) enum InterruptVector {
    Vblank = 0x40,
    LcdStat = 0x48,
    Timer = 0x50,
    Serial = 0x58,
    Joypad = 0x60,
}

#[derive(Debug, Copy, Clone, Default)]
pub(crate) struct InterruptHandler {
    interrupt_enable: Byte,
    interrupt_flag: Byte,
}

impl InterruptHandler {
    pub fn is_interrupt_enabled(&self, interrupt: InterruptType) -> bool {
        self.interrupt_enable & interrupt as Byte != 0
    }

    pub fn disable_interrupt(&mut self, interrupt: InterruptType) {
        self.interrupt_enable &= !(interrupt as Byte);
    }

    pub fn request_interrupt(&mut self, interrupt: InterruptType) {
        self.interrupt_flag |= interrupt as Byte;
    }

    pub fn reset_interrupt_request(&mut self, interrupt: InterruptType) {
        self.interrupt_flag &= !(interrupt as Byte);
    }
}

impl Memory for InterruptHandler {
    fn read(&self, address: Word) -> Byte {
        match address {
            INTERRUPT_FLAG_ADDRESS => self.interrupt_flag & 0x1F,
            INTERRUPT_ENABLE_ADDRESS => self.interrupt_enable & 0x1F,
            _ => panic!("Invalid address for Interrupts {:#06X}", address),
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        match address {
            INTERRUPT_FLAG_ADDRESS => self.interrupt_flag = data & 0x1F,
            INTERRUPT_ENABLE_ADDRESS => self.interrupt_enable = data & 0x1F,
            _ => panic!("Invalid address for Interrupts {:#06X}", address),
        }
    }
}
