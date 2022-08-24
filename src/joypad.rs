use crate::{
    memory::Memory,
    utils::{Byte, Word},
};

pub const JOYP_ADDRESS: Word = 0xFF00;

pub struct Joypad {}

impl Joypad {
    pub fn new() -> Self {
        Joypad {}
    }
}

impl Memory for Joypad {
    fn read(&self, address: Word) -> Byte {
        if address == JOYP_ADDRESS {
            // TODO: Complete Joypad::read
            return 0xFF;
        }

        panic!("Invalid address {:#06X} for Joypad::Read", address);
    }

    fn write(&mut self, address: Word, _data: Byte) {
        if address == JOYP_ADDRESS {
            // TODO: Complete Joypad::write
            return;
        }

        panic!("Invalid address {:#06X} for Joypad::Write", address);
    }
}
