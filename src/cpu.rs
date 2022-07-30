use std::{cell::RefCell, rc::Rc};

use paste::paste;

use crate::interrupts::{
    InterruptHandler, InterruptType, InterruptVector, INTERRUPT_ENABLE_ADDRESS,
    INTERRUPT_FLAG_ADDRESS,
};
use crate::{
    memory::Memory,
    mmu::Mmu,
    utils::{compose_word, decompose_word, Byte, Word},
};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum CpuState {
    Halted,
    Executing,
    Stopped,
}

pub(crate) struct Cpu {
    mmu: Rc<RefCell<Mmu>>,
    regs: Registers,

    state: CpuState,

    ime: bool,
    interrupts: Rc<RefCell<InterruptHandler>>,
}

impl Cpu {
    pub fn new(mmu: Rc<RefCell<Mmu>>, interrupts: Rc<RefCell<InterruptHandler>>) -> Self {
        let regs = Default::default();
        let ime = true;
        let state = CpuState::Executing;

        log::debug!("Initialized CPU for DMG");
        Cpu {
            mmu,
            regs,
            state,
            ime,
            interrupts,
        }
    }

    fn fetch(&mut self) -> Byte {
        let byte = self.mmu.borrow().read(self.regs.pc);
        self.regs.pc += 1;
        byte
    }

    pub fn execute(&mut self) {
        // Each HALT execute is equal to 1-m cycle
        if self.state == CpuState::Halted {
            self.mmu.borrow().tick();
            return;
        }

        self.handle_interrupts();
        self.execute_opcode();
    }

    /// CPU Interrupt Handler. Should take 5 m-cycles
    fn handle_interrupts(&mut self) {
        if self.state != CpuState::Halted && !self.ime {
            return;
        }

        let intf = self.mmu.borrow().read(INTERRUPT_FLAG_ADDRESS);
        let inte = self.mmu.borrow().read(INTERRUPT_ENABLE_ADDRESS);

        let ii = intf & inte;
        if ii == 0x00 {
            // No interrupts pending
            return;
        }

        // When there are pending interrupts, the CPU starts executing again and jumps to the interrupt
        // with the highest priority
        self.state = CpuState::Executing;

        // However, if there are pending interrupts, but *all* interrupts are disabled, the CPU still
        // needs to be executing, however we don't service any interrupt.
        if !self.ime {
            return;
        }
        self.ime = false;
        let highest_priority_interrupt = ii.trailing_zeros();
        let interrupt = InterruptType::from_index(highest_priority_interrupt);
        self.interrupts
            .borrow_mut()
            .reset_interrupt_request(interrupt);

        // Push PC to stack
        let (upper, lower) = decompose_word(self.regs.pc);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        self.mmu.borrow_mut().write(self.regs.sp, upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        self.mmu.borrow_mut().write(self.regs.sp, lower);

        // Jump to interrupt handler
        self.regs.pc = match interrupt {
            InterruptType::Vblank => InterruptVector::Vblank as Word,
            InterruptType::LcdStat => InterruptVector::LcdStat as Word,
            InterruptType::Timer => InterruptVector::Timer as Word,
            InterruptType::Serial => InterruptVector::Serial as Word,
            InterruptType::Joypad => InterruptVector::Joypad as Word,
        };
        self.mmu.borrow().tick(); // The PC set takes another m-cycle
    }

    fn execute_opcode(&mut self) {
        self.print_debug_log();

        let opcode_byte = self.fetch();

        match opcode_byte {
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_r16_u16(opcode_byte),
            0x80..=0xBF => self.alu_a_r8(opcode_byte),
            0xC6 | 0xD6 | 0xE6 | 0xF6 | 0xCE | 0xDE | 0xEE | 0xFE => self.alu_a_u8(opcode_byte),
            0x02 | 0x12 | 0x22 | 0x32 => self.ld_r16_a(opcode_byte),
            0xCB => self.cb_prefixed_opcodes(opcode_byte),
            0x20 | 0x30 | 0x28 | 0x38 => self.jr_cc_i8(opcode_byte),
            0x18 => self.jr_i8(opcode_byte),
            0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => self.ld_r8_u8(opcode_byte),
            0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C => self.inc_r8(opcode_byte),
            0x05 | 0x15 | 0x25 | 0x35 | 0x0D | 0x1D | 0x2D | 0x3D => self.dec_r8(opcode_byte),
            0xE2 => self.ld_ff00_c_a(opcode_byte),
            0x76 => self.halt(opcode_byte),
            0x40..=0x7F => self.ld_r8_r8(opcode_byte),
            0xE0 => self.ld_ff00_u8_a(opcode_byte),
            0xF0 => self.ld_a_ff00_u8(opcode_byte),
            0x0A | 0x1A | 0x2A | 0x3A => self.ld_a_r16(opcode_byte),
            0xCD => self.call_u16(opcode_byte),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_r16(opcode_byte),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_r16(opcode_byte),
            0x07 | 0x17 | 0x27 | 0x37 | 0x0F | 0x1F | 0x2F | 0x3F => self.flag_ops(opcode_byte),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_r16(opcode_byte),
            0xC9 => self.ret(opcode_byte),
            0xEA => self.ld_u16_a(opcode_byte),
            0xFA => self.ld_a_u16(opcode_byte),
            _ => panic!(
                "Unimplemented or illegal opcode {:#04X} at PC: {:#06X}",
                opcode_byte,
                self.regs.pc - 1
            ),
        };
    }

    fn print_debug_log(&self) {
        let pc = self.regs.pc;
        let byte_0 = self.mmu.borrow().raw_read(pc);
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

    fn alu_a_u8(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = self.fetch();

        match b543 {
            0 => self.add_a(operand),
            1 => self.adc_a(operand),
            2 => self.sub_a(operand),
            3 => self.sbc_a(operand),
            4 => self.and_a(operand),
            5 => self.xor_a(operand),
            6 => self.or_a(operand),
            7 => self.cp_a(operand),
            _ => panic!("Invalid bits {:b} for ALU A, u8 operation", b543),
        };
    }

    fn add_a(&mut self, operand: Byte) {
        let (result, carry) = self.regs.a.overflowing_add(operand);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = (self.regs.a & 0x0F) + (operand & 0x0F) > 0x0F;
        self.regs.f.carry = carry;

        self.regs.a = result;
    }

    fn adc_a(&mut self, _operand: Byte) {
        todo!()
    }

    fn sub_a(&mut self, operand: Byte) {
        let (result, borrow) = self.regs.a.overflowing_sub(operand);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = (self.regs.a & 0x0F) < (operand & 0x0F);
        self.regs.f.carry = borrow;

        self.regs.a = result;
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

    fn cp_a(&mut self, operand: Byte) {
        let (result, borrow) = self.regs.a.overflowing_sub(operand);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = (self.regs.a & 0x0F) < (operand & 0x0F);
        self.regs.f.carry = borrow;
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

    fn jr_i8(&mut self, _: Byte) {
        let offset = i16::from(self.fetch() as i8) as u16;
        self.regs.pc = self.regs.pc.wrapping_add(offset);
        self.mmu.borrow().tick();
    }

    fn ld_r8_u8(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = self.fetch();
        ByteRegister::for_r8(b543, self).set(operand);
    }

    fn cb_prefixed_opcodes(&mut self, _: Byte) {
        let prefixed_opcode = self.fetch();

        match prefixed_opcode {
            0x00..=0x3F => self.rotate_and_swap_r8(prefixed_opcode),
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

    fn dec_r8(&mut self, opcode: u8) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = ByteRegister::for_r8(b543, self).get();

        let result = operand.wrapping_sub(1);
        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = operand & 0x0F == 0x00;

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

    fn ld_a_ff00_u8(&mut self, _: Byte) {
        let offset = Word::from(self.fetch());
        self.regs.a = self.mmu.borrow().read(0xFF00 + offset);
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

    fn push_r16(&mut self, opcode: Byte) {
        let b54 = (opcode & 0x30) >> 4;
        let register_value = WordRegister::for_group3(b54, self).get();
        let (upper, lower) = decompose_word(register_value);

        // Pre-decrement of SP takes a cycle
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        self.mmu.borrow().tick();

        self.mmu.borrow_mut().write(self.regs.sp, upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        self.mmu.borrow_mut().write(self.regs.sp, lower);
    }

    fn pop_r16(&mut self, opcode: Byte) {
        let b54 = (opcode & 0x30) >> 4;

        let lower = self.mmu.borrow().read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let upper = self.mmu.borrow().read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);

        WordRegister::for_group3(b54, self).set(compose_word(upper, lower));
    }

    fn rotate_and_swap_r8(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let operand = ByteRegister::for_r8(b321, self).get();
        let result = match b543 {
            0 => self.rlc(operand),
            1 => self.rrc(operand),
            2 => self.rl(operand),
            3 => self.rr(operand),
            4 => self.sla(operand),
            5 => self.sra(operand),
            6 => self.swap(operand),
            7 => self.srl(operand),
            _ => panic!("Invalid bits {:b} for SWAP/ROTATE/SHIFT r8 operation", b543),
        };

        ByteRegister::for_r8(b321, self).set(result);
    }

    fn rlc(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn rrc(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn rl(&mut self, mut operand: Byte) -> Byte {
        let carry = if self.regs.f.carry { 1 } else { 0 };
        self.regs.f.carry = (operand & 0x80) != 0;
        operand <<= 1;
        operand |= carry;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn rr(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn sla(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn sra(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn swap(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn srl(&self, _operand: u8) -> u8 {
        todo!()
    }

    fn flag_ops(&mut self, opcode: Byte) {
        let b543 = (opcode & 0x38) >> 3;

        match b543 {
            0 => self.regs.a = self.rlca(),
            1 => self.regs.a = self.rrca(),
            2 => self.regs.a = self.rla(),
            3 => self.regs.a = self.rra(),
            4 => self.regs.a = self.daa(),
            5 => self.cpl(),
            6 => self.scf(),
            7 => self.ccf(),
            _ => panic!("Invalid bits {:b} for FLAG operations", b543),
        }
    }

    fn rlca(&self) -> u8 {
        todo!()
    }

    fn rrca(&self) -> u8 {
        todo!()
    }

    fn rla(&mut self) -> Byte {
        let result = self.rl(self.regs.a);
        self.regs.f.zero = false; // RLA unsets zero flag always
        result
    }

    fn rra(&self) -> u8 {
        todo!()
    }

    fn daa(&self) -> u8 {
        todo!()
    }

    fn cpl(&self) {
        todo!()
    }

    fn scf(&self) {
        todo!()
    }

    fn ccf(&self) {
        todo!()
    }

    fn inc_r16(&mut self, opcode: Byte) {
        let b54 = (opcode & 0x30) >> 4;
        WordRegister::for_group1(b54, self).inc();
    }

    fn ret(&mut self, _: Byte) {
        let lower = self.mmu.borrow().read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);

        let upper = self.mmu.borrow().read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);

        self.regs.pc = compose_word(upper, lower);
        // Final m-cycle
        self.mmu.borrow().tick();
    }

    fn ld_u16_a(&mut self, _: u8) {
        let lower = self.fetch();
        let upper = self.fetch();

        let address = compose_word(upper, lower);
        self.mmu.borrow_mut().write(address, self.regs.a);
    }

    fn ld_a_u16(&mut self, _: Byte) {
        let lower = self.fetch();
        let upper = self.fetch();

        let address = compose_word(upper, lower);
        self.regs.a = self.mmu.borrow().read(address);
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

    pub fn for_group3(bits: Byte, cpu: &'a mut Cpu) -> Self {
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
            3 => WordRegister::AccumAndFlag {
                a: &mut cpu.regs.a,
                f: &mut cpu.regs.f,
            },
            _ => panic!("Invalid decode bits for Group 3 R16 registers {:b}", bits),
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

    fn inc(&mut self) {
        match self {
            WordRegister::Pair { lower, upper } => {
                let (inc_lower, carry) = lower.overflowing_add(1);
                **lower = inc_lower;

                if carry {
                    **upper = upper.wrapping_add(1);
                }
            }
            WordRegister::Single(reg) => {
                **reg = reg.wrapping_add(1);
            }
            WordRegister::AccumAndFlag { .. } => panic!("INC not required for AF register"),
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
