use paste::paste;
use std::{cell::RefCell, rc::Rc};

use crate::{
    memory::Memory,
    mmu::Mmu,
    opcodes::OPCODE_METADATA,
    utils::{compose_word, decompose_word, Byte, Word, HEX_LOOKUP},
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
            0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => self.ld_r8_u8(opcode_byte),
            0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C => self.inc_r8(opcode_byte),
            0xE2 => self.ld_ff00_c_a(opcode_byte),
            0x76 => self.halt(opcode_byte),
            0x40..=0x7F => self.ld_r8_r8(opcode_byte),
            0xE0 => self.ld_ff00_u8_a(opcode_byte),
            0x0A | 0x1A | 0x2A | 0x3A => self.ld_a_r16(opcode_byte),
            0xCD => self.call_u16(opcode_byte),
            _ => panic!(
                "Unimplemented or illegal opcode {:#04X} at PC: {:#06X}",
                opcode_byte, self.regs.pc
            ),
        };
    }

    fn print_debug_log(&self) {
        let pc = self.regs.pc;
        let byte_0 = self.mmu.borrow().raw_read(pc + 0);
        let byte_1 = self.mmu.borrow().raw_read(pc + 1);
        let byte_2 = self.mmu.borrow().raw_read(pc + 2);
        let byte_3 = self.mmu.borrow().raw_read(pc + 3);
        println!("A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})", 
        self.regs.a, Byte::from(self.regs.f), self.regs.b, self.regs.c, self.regs.d, self.regs.e, self.regs.h, self.regs.l, self.regs.sp, self.regs.pc, byte_0, byte_1, byte_2, byte_3);
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

        self.regs.f.zero = self.regs.a == 0x00;
        self.regs.f.carry = false;
        self.regs.f.half_carry = false;
        self.regs.f.negative = false;
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

    fn ld_a_r16(&mut self, opcode: Byte) {
        let b54 = (opcode & 0x30) >> 4;
        let address = WordRegister::for_group2(b54, self).get();
        self.regs.a = self.mmu.borrow().read(address);

        // Increment or decrement HL if the opcode requires it
        if b54 == 2 {
            self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
        } else if b54 == 3 {
            self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
        }
    }

    fn check_condition(&self, bits: Byte) -> bool {
        match bits {
            0 => !self.regs.f.zero,
            1 => self.regs.f.zero,
            2 => !self.regs.f.carry,
            3 => self.regs.f.carry,
            _ => panic!("Invalid decode bits for condition check {:b}", bits),
        }
    }

    fn jr_cc_i8(&mut self, opcode: Byte) {
        let b43 = (opcode & 0x18) >> 3;
        let offset = i16::from(self.fetch() as i8) as u16;

        if self.check_condition(b43) {
            self.regs.pc = self.regs.pc.wrapping_add(offset);
            // TODO: This might be an opcode preload or dummy read. Fix it
            self.mmu.borrow().tick();
        }
    }

    fn ld_r8_u8(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = self.fetch();
        ByteRegister::for_r8(b543, self).set(operand);
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
        self.regs.f.zero = operand & (1 << b543) == 0;
        self.regs.f.negative = false;
        self.regs.f.half_carry = true;
    }

    fn ld_ff00_c_a(&mut self, _: Byte) {
        self.mmu
            .borrow_mut()
            .write(0xFF00 + Word::from(self.regs.c), self.regs.a);
    }

    fn inc_r8(&mut self, opcode: u8) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = ByteRegister::for_r8(b543, self).get();

        let result = operand.wrapping_add(1);
        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = ((operand & 0x0F) + 0x1) > 0x0F;

        ByteRegister::for_r8(b543, self).set(result);
    }

    fn halt(&mut self, _: Byte) {
        todo!()
    }

    fn ld_r8_r8(&mut self, opcode: u8) {
        let b543 = (opcode & 0x38) >> 3; // Destination
        let b210 = opcode & 0x07; // Source

        let source = ByteRegister::for_r8(b210, self).get();
        ByteRegister::for_r8(b543, self).set(source);
    }

    fn ld_ff00_u8_a(&mut self, _: Byte) {
        let offset = Word::from(self.fetch());
        self.mmu.borrow_mut().write(0xFF00 + offset, self.regs.a);
    }

    fn call_u16(&mut self, _: u8) {
        let lsb = self.fetch();
        let msb = self.fetch();
        let jump_address = compose_word(msb, lsb);

        // Pre-decrement SP
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        self.mmu.borrow().tick();

        // Write return location to stack
        let (pc_upper, pc_lower) = decompose_word(self.regs.pc);
        self.mmu.borrow_mut().write(self.regs.sp, pc_upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        self.mmu.borrow_mut().write(self.regs.sp, pc_lower);

        self.regs.pc = jump_address;
    }
}

enum FlagRegisterMask {
    Zero = (1 << 7),
    Negative = (1 << 6),
    HalfCarry = (1 << 5),
    Carry = (1 << 4),
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FlagRegister {
    zero: bool,
    negative: bool,
    half_carry: bool,
    carry: bool,
}

impl From<u8> for FlagRegister {
    fn from(value: u8) -> Self {
        let zero = (value & FlagRegisterMask::Zero as u8) != 0;
        let negative = (value & FlagRegisterMask::Negative as u8) != 0;
        let half_carry = (value & FlagRegisterMask::HalfCarry as u8) != 0;
        let carry = (value & FlagRegisterMask::Carry as u8) != 0;

        FlagRegister {
            zero,
            negative,
            half_carry,
            carry,
        }
    }
}

impl From<FlagRegister> for u8 {
    fn from(register: FlagRegister) -> Self {
        let mut value = 0x00;
        if register.zero {
            value |= FlagRegisterMask::Zero as u8;
        }
        if register.negative {
            value |= FlagRegisterMask::Negative as u8;
        }
        if register.half_carry {
            value |= FlagRegisterMask::HalfCarry as u8;
        }
        if register.carry {
            value |= FlagRegisterMask::Carry as u8;
        }

        value
    }
}

#[derive(Default)]
pub(crate) struct Registers {
    a: Byte,
    f: FlagRegister,
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
            pub fn [<get_ $upper $lower >](&self) -> Word {
                compose_word(self.$upper, self.$lower.into())
            }

            pub fn [<set_ $upper $lower>](&mut self, value: Word) {
                let (msb, lsb) = decompose_word(value);

                self.$upper = msb;
                self.$lower = lsb.into();
            }
        }
    };
}

impl Registers {
    register_pair!(b, c);
    register_pair!(d, e);
    register_pair!(h, l);
    register_pair!(a, f);
}

// Register decoding for opcodes
enum WordRegister<'a> {
    Pair {
        lower: &'a mut Byte,
        upper: &'a mut Byte,
    },
    Single(&'a mut Word),
    /// For the AF register pair
    AccumAndFlag {
        a: &'a mut Byte,
        f: &'a mut FlagRegister,
    },
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
            WordRegister::AccumAndFlag { a, f } => compose_word(**a, Byte::from(**f)),
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
            WordRegister::AccumAndFlag { a, f } => {
                let (msb, lsb) = decompose_word(value);
                **a = msb;
                **f = lsb.into();
            }
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
