use paste::paste;
use std::{cell::RefCell, rc::Rc};

use crate::{
    mmu::Mmu,
    opcodes::OPCODE_METADATA,
    utils::{compose_word, decompose_word, Byte, Cycles, Word, HEX_LOOKUP},
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

    pub fn execute(&mut self) -> Cycles {
        // TODO: Check for interrupts and halts here
        self.execute_opcode()
    }

    fn execute_opcode(&mut self) -> Cycles {
        self.print_debug_log();

        let opcode_byte = self.fetch();
        let opcode_metadata = &OPCODE_METADATA.unprefixed[&*HEX_LOOKUP[&opcode_byte]];

        log::debug!("Opcode name: {}", opcode_metadata.mnemonic);

        let branch_taken_cycles = match opcode_byte {
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_r16_u16(opcode_byte),
            _ => panic!("Unimplemented or illegal opcode {:#04X}", opcode_byte),
        };

        opcode_metadata.cycles[0] + branch_taken_cycles
    }

    fn print_debug_log(&self) {
        let pc = self.regs.pc;
        let byte_0 = self.mmu.borrow_mut().read(pc + 0);
        let byte_1 = self.mmu.borrow_mut().read(pc + 1);
        let byte_2 = self.mmu.borrow_mut().read(pc + 2);
        let byte_3 = self.mmu.borrow_mut().read(pc + 3);
        println!("A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})", 
        self.regs.a, self.regs.f, self.regs.b, self.regs.c, self.regs.d, self.regs.e, self.regs.h, self.regs.l, self.regs.sp, self.regs.pc, byte_0, byte_1, byte_2, byte_3);
    }

    fn fetch(&mut self) -> Byte {
        let byte = self.mmu.borrow_mut().read(self.regs.pc);
        self.regs.pc += 1;
        byte
    }

    // Opcode Implementations
    fn ld_r16_u16(&mut self, opcode: Byte) -> Cycles {
        let lower = self.fetch();
        let upper = self.fetch();

        let b54 = (opcode & 0x30) >> 4;
        let value = compose_word(upper, lower);
        WordRegister::for_group1(b54, self).set(value);

        0x00
    }
}

pub enum FlagRegisterMask {
    Zero = (1 << 7),
    Subtraction = (1 << 6),
    HalfCarry = (1 << 5),
    Carry = (1 << 4),
}

#[derive(Default)]
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

// Register decoding for opcodes
enum WordRegister<'a> {
    Pair {
        lower: &'a mut Byte,
        upper: &'a mut Byte,
    },
    Single(&'a mut Word),
}

impl<'a> WordRegister<'a> {
    pub fn for_group1(bits: Byte, cpu: &'a mut Cpu) -> Self {
        match bits {
            0 => WordRegister::Pair {
                lower: &mut cpu.regs.b,
                upper: &mut cpu.regs.c,
            },
            1 => WordRegister::Pair {
                lower: &mut cpu.regs.d,
                upper: &mut cpu.regs.e,
            },
            2 => WordRegister::Pair {
                lower: &mut cpu.regs.h,
                upper: &mut cpu.regs.l,
            },
            3 => WordRegister::Single(&mut cpu.regs.sp),
            _ => panic!("Invalid decode bits for Group 1 R16 registers {:b}", bits),
        }
    }

    pub fn get(&self) -> Word {
        match self {
            WordRegister::Pair { lower, upper } => compose_word(**upper, **lower),
            WordRegister::Single(reg) => **reg,
        }
    }

    pub fn set(&mut self, value: u16) {
        match self {
            WordRegister::Pair { lower, upper } => {
                let (msb, lsb) = decompose_word(value);
                **lower = lsb;
                **upper = msb;
            }
            WordRegister::Single(reg) => **reg = value,
        }
    }
}
