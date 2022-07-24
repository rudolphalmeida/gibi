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

    fn fetch(&mut self) -> Byte {
        let byte = self.mmu.borrow().read(self.regs.pc);
        self.regs.pc += 1;
        byte
    }

    pub fn execute(&mut self) {
        // TODO: Check for interrupts and halts here
        self.execute_opcode();
    }

    fn execute_opcode(&mut self) {
        self.print_debug_log();

        let opcode_byte = self.fetch();
        let opcode_metadata = &OPCODE_METADATA.unprefixed[&*HEX_LOOKUP[&opcode_byte]];

        log::debug!("Opcode name: {}", opcode_metadata.mnemonic);

        match opcode_byte {
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_r16_u16(opcode_byte),
            0x80..=0xBF => self.alu_a_r8(opcode_byte),
            0x02 | 0x12 | 0x22 | 0x32 => self.ld_r16_a(opcode_byte),
            0xCB => self.cb_prefixed_opcodes(opcode_byte),
            0x20 | 0x30 | 0x28 | 0x38 => self.jr_cc_i8(opcode_byte),
            _ => panic!("Unimplemented or illegal opcode {:#04X}", opcode_byte),
        };
    }

    fn print_debug_log(&self) {
        let pc = self.regs.pc;
        let byte_0 = self.mmu.borrow().raw_read(pc + 0);
        let byte_1 = self.mmu.borrow().raw_read(pc + 1);
        let byte_2 = self.mmu.borrow().raw_read(pc + 2);
        let byte_3 = self.mmu.borrow().raw_read(pc + 3);
        println!("A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})", 
        self.regs.a, self.regs.f, self.regs.b, self.regs.c, self.regs.d, self.regs.e, self.regs.h, self.regs.l, self.regs.sp, self.regs.pc, byte_0, byte_1, byte_2, byte_3);
    }

    // Opcode Implementations
    fn ld_r16_u16(&mut self, opcode: Byte) {
        let lower = self.fetch();
        let upper = self.fetch();

        let b54 = (opcode & 0x30) >> 4;
        let value = compose_word(upper, lower);
        WordRegister::for_group1(b54, self).set(value);
    }

    fn alu_a_r8(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let operand = ByteRegister::for_r8(b321, self).get();

        match b543 {
            0 => self.add_a(operand),
            1 => self.adc_a(operand),
            2 => self.sub_a(operand),
            3 => self.sbc_a(operand),
            4 => self.and_a(operand),
            5 => self.xor_a(operand),
            6 => self.or_a(operand),
            7 => self.cp_a(operand),
            _ => panic!("Invalid bits {:b} for ALU A, r8 operation", b543),
        };
    }

    fn add_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn adc_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn sub_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn sbc_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn and_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn xor_a(&mut self, operand: Byte) {
        self.regs.a ^= operand;

        self.regs
            .update_flag(FlagRegisterMask::Zero, self.regs.a == 0x00);
        self.regs.update_flag(FlagRegisterMask::Carry, false);
        self.regs.update_flag(FlagRegisterMask::HalfCarry, false);
        self.regs.update_flag(FlagRegisterMask::Subtraction, false);
    }

    fn or_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn cp_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn ld_r16_a(&mut self, opcode: Byte) {
        let b54 = (opcode & 0x30) >> 4;
        let address = WordRegister::for_group2(b54, self).get();
        self.mmu.borrow_mut().write(address, self.regs.a);

        // Increment or decrement HL if the opcode requires it
        if b54 == 2 {
            self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
        } else if b54 == 3 {
            self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
        }
    }

    fn check_condition(&self, bits: Byte) -> bool {
        match bits {
            0 => !self.regs.is_set_flag(FlagRegisterMask::Zero),
            1 => self.regs.is_set_flag(FlagRegisterMask::Zero),
            2 => !self.regs.is_set_flag(FlagRegisterMask::Carry),
            3 => self.regs.is_set_flag(FlagRegisterMask::Carry),
            _ => panic!("Invalid decode bits for condition check {:b}", bits),
        }
    }

    fn jr_cc_i8(&mut self, opcode: Byte) {
        let b43 = (opcode & 0x18) >> 3;
        let offset = i16::from(self.fetch() as i8) as u16;

        if self.check_condition(b43) {
            self.regs.pc = self.regs.pc.wrapping_add(offset);
            // TODO: This might be an opcode preload or dummy read. Fix it
            self.mmu.borrow().read(self.regs.pc); // Dummy tick for this cycle
        }
    }

    fn cb_prefixed_opcodes(&mut self, _: Byte) {
        let prefixed_opcode = self.fetch();

        match prefixed_opcode {
            0x40..=0x7F => self.bit_n_r8(prefixed_opcode),
            _ => panic!(
                "Unimplemented or illegal prefixed opcode {:#04X}",
                prefixed_opcode
            ),
        };
    }

    fn bit_n_r8(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let operand = ByteRegister::for_r8(b321, self).get();
        self.regs
            .update_flag(FlagRegisterMask::Zero, operand & (1 << b543) == 0);
        self.regs.update_flag(FlagRegisterMask::Subtraction, false);
        self.regs.update_flag(FlagRegisterMask::HalfCarry, true);
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

    pub fn update_flag(&mut self, flag: FlagRegisterMask, value: bool) {
        if value {
            self.f = self.f | (flag as Byte);
        } else {
            self.f = self.f & !(flag as Byte);
        }
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
                upper: &mut cpu.regs.b,
                lower: &mut cpu.regs.c,
            },
            1 => WordRegister::Pair {
                upper: &mut cpu.regs.d,
                lower: &mut cpu.regs.e,
            },
            2 => WordRegister::Pair {
                upper: &mut cpu.regs.h,
                lower: &mut cpu.regs.l,
            },
            3 => WordRegister::Single(&mut cpu.regs.sp),
            _ => panic!("Invalid decode bits for Group 1 R16 registers {:b}", bits),
        }
    }

    pub fn for_group2(bits: Byte, cpu: &'a mut Cpu) -> Self {
        match bits {
            0 => WordRegister::Pair {
                upper: &mut cpu.regs.b,
                lower: &mut cpu.regs.c,
            },
            1 => WordRegister::Pair {
                upper: &mut cpu.regs.d,
                lower: &mut cpu.regs.e,
            },
            2 | 3 => WordRegister::Pair {
                upper: &mut cpu.regs.h,
                lower: &mut cpu.regs.l,
            },
            _ => panic!("Invalid decode bits for Group 2 R16 registers {:b}", bits),
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

enum ByteRegister<'a> {
    Register(&'a mut Byte),
    MemoryReference(Word, Rc<RefCell<Mmu>>),
}

impl<'a> ByteRegister<'a> {
    pub fn for_r8(bits: Byte, cpu: &'a mut Cpu) -> Self {
        match bits {
            0 => ByteRegister::Register(&mut cpu.regs.b),
            1 => ByteRegister::Register(&mut cpu.regs.c),
            2 => ByteRegister::Register(&mut cpu.regs.d),
            3 => ByteRegister::Register(&mut cpu.regs.e),
            4 => ByteRegister::Register(&mut cpu.regs.h),
            5 => ByteRegister::Register(&mut cpu.regs.l),
            6 => ByteRegister::MemoryReference(cpu.regs.get_hl(), Rc::clone(&cpu.mmu)),
            7 => ByteRegister::Register(&mut cpu.regs.a),
            _ => panic!("Invalid decode bits for R8 register {:b}", bits),
        }
    }

    pub fn get(&self) -> Byte {
        match self {
            ByteRegister::Register(ptr) => **ptr,
            ByteRegister::MemoryReference(address, mmu) => mmu.borrow().read(*address),
        }
    }

    pub fn set(&mut self, value: Byte) {
        match self {
            ByteRegister::Register(ptr) => **ptr = value,
            ByteRegister::MemoryReference(address, mmu) => mmu.borrow_mut().write(*address, value),
        }
    }
}
