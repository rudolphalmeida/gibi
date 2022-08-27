use std::cell::RefCell;
use std::rc::Rc;

use crate::interrupts::{InterruptHandler, InterruptType};
use crate::utils::Cycles;
use crate::{
    memory::Memory,
    utils::{Byte, Word},
};

pub(crate) const JOYP_ADDRESS: Word = 0xFF00;
pub(crate) const JOYPAD_POLL_CYCLES: Cycles = 65536; // 64Hz

pub enum JoypadKeys {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Select = 6,
    Start = 7,
}

pub(crate) struct Joypad {
    keys: Byte,
    joyp: Byte,

    cycles: Cycles,

    interrupts: Rc<RefCell<InterruptHandler>>,
}

impl Joypad {
    pub fn new(interrupts: Rc<RefCell<InterruptHandler>>) -> Self {
        Joypad {
            keys: 0xFF,
            joyp: 0xFF,
            cycles: 0,
            interrupts,
        }
    }

    pub(crate) fn keydown(&mut self, key: JoypadKeys) {
        self.keys &= !(key as Byte);
    }

    pub(crate) fn keyup(&mut self, key: JoypadKeys) {
        self.keys |= key as Byte;
    }

    pub(crate) fn tick(&mut self) {
        self.cycles += 4;

        // The Joypad polls for input every 64Hz
        if self.cycles >= JOYPAD_POLL_CYCLES {
            self.cycles %= JOYPAD_POLL_CYCLES;
            self.update();
        }
    }

    fn update(&mut self) {
        let mut current = self.joyp & 0xF0;

        match current & 0x30 {
            0x10 => current |= (self.keys >> 4) & 0x0F,
            0x20 => current |= self.keys & 0x0F,
            0x30 => current |= 0x0F,
            _ => panic!("Should not be possible"),
        }

        if (self.joyp & !current & 0x0F) != 0 {
            self.interrupts
                .borrow_mut()
                .request_interrupt(InterruptType::Joypad);
        }

        self.joyp = current;
    }
}

impl Memory for Joypad {
    fn read(&self, address: Word) -> Byte {
        if address == JOYP_ADDRESS {
            return self.joyp;
        }

        panic!("Invalid address {:#06X} for Joypad::Read", address);
    }

    fn write(&mut self, address: Word, data: Byte) {
        if address == JOYP_ADDRESS {
            self.joyp = (self.joyp & 0xCF) | (data & 0x30);
            self.update();
            return;
        }

        panic!("Invalid address {:#06X} for Joypad::Write", address);
    }
}
