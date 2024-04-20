use crate::memory::Memory;

pub(crate) const INTERRUPT_FLAG_ADDRESS: u16 = 0xFF0F;
pub(crate) const INTERRUPT_ENABLE_ADDRESS: u16 = 0xFFFF;

#[derive(Debug, Copy, Clone)]
pub(crate) enum InterruptType {
    Vblank = 1,
    LcdStat = 1 << 1,
    Timer = 1 << 2,
    Serial = 1 << 3,
    Joypad = 1 << 4,
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

    pub fn vector(&self) -> u16 {
        match self {
            InterruptType::Vblank => InterruptVector::Vblank as u16,
            InterruptType::LcdStat => InterruptVector::LcdStat as u16,
            InterruptType::Timer => InterruptVector::Timer as u16,
            InterruptType::Serial => InterruptVector::Serial as u16,
            InterruptType::Joypad => InterruptVector::Joypad as u16,
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
    interrupt_enable: u8,
    interrupt_flag: u8,
}

impl InterruptHandler {
    pub fn is_interrupt_enabled(&self, interrupt: InterruptType) -> bool {
        self.interrupt_enable & interrupt as u8 != 0
    }

    pub fn disable_interrupt(&mut self, interrupt: InterruptType) {
        self.interrupt_enable &= !(interrupt as u8);
    }

    pub fn request_interrupt(&mut self, interrupt: InterruptType) {
        self.interrupt_flag |= interrupt as u8;
    }

    pub fn reset_interrupt_request(&mut self, interrupt: InterruptType) {
        self.interrupt_flag &= !(interrupt as u8);
    }
}

impl Memory for InterruptHandler {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            INTERRUPT_FLAG_ADDRESS => self.interrupt_flag & 0x1F,
            INTERRUPT_ENABLE_ADDRESS => self.interrupt_enable & 0x1F,
            _ => panic!("Invalid address for Interrupts {:#06X}", address),
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            INTERRUPT_FLAG_ADDRESS => self.interrupt_flag = data & 0x1F,
            INTERRUPT_ENABLE_ADDRESS => self.interrupt_enable = data & 0x1F,
            _ => panic!("Invalid address for Interrupts {:#06X}", address),
        }
    }
}
