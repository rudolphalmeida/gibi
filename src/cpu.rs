use paste::paste;
use std::{cell::RefCell, rc::Rc};

use crate::{
    mmu::Mmu,
    utils::{compose_word, decompose_word, Byte, Cycles, Word},
};

pub(crate) struct Cpu {
    mmu: Rc<RefCell<Mmu>>,
    regs: Registers,
}

impl Cpu {
    pub fn new(mmu: Rc<RefCell<Mmu>>) -> Self {
        let regs = Default::default();
        log::debug!("Initialized CPU for DMG");
        Cpu { mmu, regs }
    }

    pub fn execute_opcode(&mut self) -> Cycles {
        todo!()
    }
}

pub enum FlagRegisterMask {
    Zero = (1 << 7),
    Subtraction = (1 << 6),
    HalfCarry = (1 << 5),
    Carry = (1 << 4),
}

pub(crate) struct Registers {
    a: Byte,
    f: Byte,
    b: Byte,
    c: Byte,
    d: Byte,
    e: Byte,
    h: Byte,
    l: Byte,

    sp: Word,
    pc: Word,
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            a: 0x01,
            // TODO: The carry and half-carry flags are reset if the header checksum was 0x00 and
            //       both are set if it was not. Emulate this
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }
}

macro_rules! register_pair {
    ($upper: ident, $lower: ident) => {
        paste! {
            pub fn [<get_ $upper>](&self) -> Byte {
                self.$upper
            }

            pub fn [<set_ $upper>](&mut self, value: Byte) {
                self.$upper = value;
            }

            pub fn [<get_ $lower>](&self) -> Byte {
                self.$lower
            }

            pub fn [<set_ $lower>](&mut self, value: Byte) {
                self.$lower = value;
            }

            pub fn [<get_ $upper $lower >](&self) -> Word {
                compose_word(self.[<get_ $upper>](), self.[<get_ $lower>]())
            }

            pub fn [<set_ $upper $lower>](&mut self, value: Word) {
                let (msb, lsb) = decompose_word(value);

                self.[<set_ $upper>](msb);
                self.[<set_ $lower>](lsb);
            }
        }
    };
}

impl Registers {
    register_pair!(b, c);
    register_pair!(d, e);
    register_pair!(h, l);

    pub fn get_a(&self) -> Byte {
        self.a
    }

    pub fn set_a(&mut self, value: Byte) {
        self.a = value;
    }

    pub fn get_f(&self) -> Byte {
        self.f & 0xF0
    }

    pub fn set_f(&mut self, value: Byte) {
        self.f = value & 0xF0;
    }

    pub fn get_af(&self) -> Word {
        compose_word(self.get_a(), self.get_f())
    }

    pub fn set_af(&mut self, value: Word) {
        let (msb, lsb) = decompose_word(value);

        self.set_a(msb);
        self.set_f(lsb);
    }

    pub fn is_set_flag(&self, flag: FlagRegisterMask) -> bool {
        (self.f & (flag as Byte)) != 0
    }

    pub fn set_flag(&mut self, flag: FlagRegisterMask) {
        self.f = self.f | (flag as Byte);
    }

    pub fn reset_flag(&mut self, flag: FlagRegisterMask) {
        self.f = self.f & !(flag as Byte);
    }
}
