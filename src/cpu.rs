use crate::debug::{CpuDebug, ExecutedOpcode};
use circular_buffer::CircularBuffer;
use paste::paste;

use crate::interrupts::{InterruptType, INTERRUPT_ENABLE_ADDRESS, INTERRUPT_FLAG_ADDRESS};
use crate::memory::SystemBus;
use crate::ExecutionState;

pub(crate) struct Cpu {
    regs: Registers,
    ime: bool,
    previous_execution_state: Option<ExecutionState>,
    opcodes: CircularBuffer<10, ExecutedOpcode>,
}

impl Cpu {
    pub fn new() -> Self {
        let regs = Default::default();
        let ime = true;
        let opcodes = CircularBuffer::new();

        log::debug!("Initialized CPU for CGB");
        Cpu {
            regs,
            ime,
            previous_execution_state: None,
            opcodes,
        }
    }

    pub fn debug(&self) -> CpuDebug {
        CpuDebug {
            registers: self.regs,
            opcodes: self.opcodes.to_vec(),
        }
    }

    fn fetch<BusType: SystemBus>(&mut self, mmu: &mut BusType) -> u8 {
        let byte = mmu.read(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        byte
    }

    pub fn execute<BusType: SystemBus>(&mut self, mmu: &mut BusType) {
        if self.check_for_pending_interrupts(mmu) {
            self.handle_interrupts(mmu);
        }

        let execution_state = mmu.system_state().execution_state;
        match execution_state {
              ExecutionState::Halted                   // CPU does not execute when halted
            | ExecutionState::PreparingSpeedSwitch     // switching speed
            => mmu.tick(),
            ExecutionState::ExecutingProgram => self.execute_opcode(mmu),
        }
    }

    fn check_for_pending_interrupts<BusType: SystemBus>(&self, mmu: &mut BusType) -> bool {
        let intf = mmu.unticked_read(INTERRUPT_FLAG_ADDRESS);
        let inte = mmu.unticked_read(INTERRUPT_ENABLE_ADDRESS);

        let ii = intf & inte;
        ii != 0x00
    }

    /// CPU Interrupt Handler. Should take 5 m-cycles
    fn handle_interrupts<BusType: SystemBus>(&mut self, mmu: &mut BusType) {
        // Cycle 1
        let intf = mmu.read(INTERRUPT_FLAG_ADDRESS);
        // Cycle 2
        let inte = mmu.read(INTERRUPT_ENABLE_ADDRESS);

        let ii = intf & inte;
        if ii == 0x00 {
            return;
        }

        // When there are pending interrupts, the CPU starts executing again and jumps to the interrupt
        // with the highest priority
        if let Some(previous_execution_state) = self.previous_execution_state {
            mmu.system_state().execution_state = previous_execution_state;
        }

        // However, if there are pending interrupts, but *all* interrupts are disabled, the CPU still
        // needs to be executing, however we don't service any interrupt.
        if !self.ime {
            return;
        }
        self.ime = false;
        let highest_priority_interrupt = ii.trailing_zeros();
        let interrupt = InterruptType::from_index(highest_priority_interrupt);
        mmu.interrupt_handler().reset_interrupt_request(interrupt);

        // Push PC to stack
        let [upper, lower] = self.regs.pc.to_be_bytes();
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        // Cycle 3
        mmu.write(self.regs.sp, upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        // Cycle 4
        mmu.write(self.regs.sp, lower);

        // Jump to interrupt handler
        self.regs.pc = interrupt.vector();
        mmu.tick(); // The PC set takes another m-cycle - Cycle 5
    }

    fn execute_opcode<BusType: SystemBus>(&mut self, mmu: &mut BusType) {
        let mut opcode = ExecutedOpcode {
            pc: self.regs.pc,
            ..ExecutedOpcode::default()
        };

        let opcode_byte = self.fetch(mmu);
        opcode.opcode = opcode_byte;
        opcode.arg1 = mmu.unticked_read(self.regs.pc);
        opcode.arg2 = mmu.unticked_read(self.regs.pc + 1);

        self.opcodes.push_back(opcode);

        match opcode_byte {
            0x00 => {} // NOP
            0x10 => self.stop(mmu),
            0x01 | 0x11 | 0x21 | 0x31 => self.ld_r16_u16(opcode_byte, mmu),
            0x80..=0xBF => self.alu_a_r8(opcode_byte, mmu),
            0xC6 | 0xD6 | 0xE6 | 0xF6 | 0xCE | 0xDE | 0xEE | 0xFE => {
                self.alu_a_u8(opcode_byte, mmu)
            }
            0x02 | 0x12 | 0x22 | 0x32 => self.ld_r16_a(opcode_byte, mmu),
            0xCB => self.cb_prefixed_opcodes(opcode_byte, mmu),
            0x20 | 0x30 | 0x28 | 0x38 => self.jr_cc_i8(opcode_byte, mmu),
            0x18 => self.jr_i8(opcode_byte, mmu),
            0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => {
                self.ld_r8_u8(opcode_byte, mmu)
            }
            0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C => self.inc_r8(opcode_byte, mmu),
            0x05 | 0x15 | 0x25 | 0x35 | 0x0D | 0x1D | 0x2D | 0x3D => self.dec_r8(opcode_byte, mmu),
            0x76 => self.halt(opcode_byte, mmu),
            0x40..=0x7F => self.ld_r8_r8(opcode_byte, mmu),
            0xE0 => self.ld_ff00_u8_a(opcode_byte, mmu),
            0xE2 => self.ld_ff00_c_a(opcode_byte, mmu),
            0xF0 => self.ld_a_ff00_u8(opcode_byte, mmu),
            0xF2 => self.ld_a_ff00_c(opcode_byte, mmu),
            0x0A | 0x1A | 0x2A | 0x3A => self.ld_a_r16(opcode_byte, mmu),
            0xCD => self.call_u16(opcode_byte, mmu),
            0xC4 | 0xD4 | 0xCC | 0xDC => self.call_cc_u16(opcode_byte, mmu),
            0xC5 | 0xD5 | 0xE5 | 0xF5 => self.push_r16(opcode_byte, mmu),
            0xC1 | 0xD1 | 0xE1 | 0xF1 => self.pop_r16(opcode_byte, mmu),
            0x07 | 0x17 | 0x27 | 0x37 | 0x0F | 0x1F | 0x2F | 0x3F => self.flag_ops(opcode_byte),
            0x03 | 0x13 | 0x23 | 0x33 => self.inc_r16(opcode_byte),
            0x0B | 0x1B | 0x2B | 0x3B => self.dec_r16(opcode_byte),
            0x09 | 0x19 | 0x29 | 0x39 => self.add_hl_r16(opcode_byte, mmu),
            0xC9 => self.ret(opcode_byte, mmu),
            0xD9 => self.reti(opcode_byte, mmu),
            0xC0 | 0xD0 | 0xC8 | 0xD8 => self.ret_cc(opcode_byte, mmu),
            0xEA => self.ld_u16_a(opcode_byte, mmu),
            0xFA => self.ld_a_u16(opcode_byte, mmu),
            0xC3 => self.jp_u16(opcode_byte, mmu),
            0xC2 | 0xD2 | 0xCA | 0xDA => self.jp_cc_u16(opcode_byte, mmu),
            0xF3 => self.di(opcode_byte),
            0xE9 => self.jp_hl(opcode_byte),
            0xE8 => self.add_sp_i8(opcode_byte, mmu),
            0xF8 => self.ld_hl_sp_i8(opcode_byte, mmu),
            0xF9 => self.ld_sp_hl(opcode_byte, mmu),
            0xFB => self.ei(opcode_byte, mmu),
            0x08 => self.ld_u16_sp(opcode_byte, mmu),
            0xC7 | 0xD7 | 0xE7 | 0xF7 | 0xCF | 0xDF | 0xEF | 0xFF => self.rst(opcode_byte, mmu),
            _ => panic!(
                "Unimplemented or illegal opcode {:#04X} at PC: {:#06X}",
                opcode_byte,
                self.regs.pc - 1
            ),
        };
    }

    // Opcode Implementations
    fn stop<BusType: SystemBus>(&mut self, mmu: &mut BusType) {
        if mmu.system_state().key1 & 0b1 == 0b1 {
            // If a speed switch has been requested
            mmu.system_state().execution_state = ExecutionState::PreparingSpeedSwitch;
        }
        self.fetch(mmu);
    }

    fn ld_r16_u16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let lower = self.fetch(mmu);
        let upper = self.fetch(mmu);

        let b54 = (opcode & 0x30) >> 4;
        let value = u16::from_be_bytes([upper, lower]);
        WordRegister::for_group1(b54, self).set(value);
    }

    fn alu_a_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let operand = ByteRegister::for_r8(b321, self).get(mmu);

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

    fn alu_a_u8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = self.fetch(mmu);

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

    fn add_a(&mut self, operand: u8) {
        let (result, carry) = self.regs.a.overflowing_add(operand);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = (self.regs.a & 0x0F) + (operand & 0x0F) > 0x0F;
        self.regs.f.carry = carry;

        self.regs.a = result;
    }

    fn adc_a(&mut self, operand: u8) {
        let carry = u8::from(self.regs.f.carry);
        let result = self.regs.a.wrapping_add(operand).wrapping_add(carry);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = ((self.regs.a & 0xF) + (operand & 0xF) + carry) > 0xF;
        self.regs.f.carry = ((self.regs.a as u16) + (operand as u16) + (carry as u16)) > 0xFF;

        self.regs.a = result;
    }

    fn sub_a(&mut self, operand: u8) {
        let (result, borrow) = self.regs.a.overflowing_sub(operand);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = (self.regs.a & 0x0F) < (operand & 0x0F);
        self.regs.f.carry = borrow;

        self.regs.a = result;
    }

    fn sbc_a(&mut self, operand: u8) {
        let carry = u8::from(self.regs.f.carry);
        let result = self.regs.a.wrapping_sub(operand).wrapping_sub(carry);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = (self.regs.a & 0xF)
            .wrapping_sub(operand & 0xF)
            .wrapping_sub(carry)
            & (0xF + 1)
            != 0;
        self.regs.f.carry = (self.regs.a as u16) < (operand as u16) + (carry as u16);

        self.regs.a = result;
    }

    fn and_a(&mut self, operand: u8) {
        self.regs.a &= operand;

        self.regs.f.zero = self.regs.a == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = true;
        self.regs.f.carry = false;
    }

    fn xor_a(&mut self, operand: u8) {
        self.regs.a ^= operand;

        self.regs.f.zero = self.regs.a == 0x00;
        self.regs.f.carry = false;
        self.regs.f.half_carry = false;
        self.regs.f.negative = false;
    }

    fn or_a(&mut self, operand: u8) {
        self.regs.a |= operand;

        self.regs.f.zero = self.regs.a == 0x00;
        self.regs.f.carry = false;
        self.regs.f.half_carry = false;
        self.regs.f.negative = false;
    }

    fn cp_a(&mut self, operand: u8) {
        let (result, borrow) = self.regs.a.overflowing_sub(operand);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = (self.regs.a & 0x0F) < (operand & 0x0F);
        self.regs.f.carry = borrow;
    }

    fn ld_r16_a<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b54 = (opcode & 0x30) >> 4;
        let address = WordRegister::for_group2(b54, self).get();
        mmu.write(address, self.regs.a);

        // Increment or decrement HL if the opcode requires it
        if b54 == 2 {
            self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
        } else if b54 == 3 {
            self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
        }
    }

    fn ld_a_r16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b54 = (opcode & 0x30) >> 4;
        let address = WordRegister::for_group2(b54, self).get();
        self.regs.a = mmu.read(address);

        // Increment or decrement HL if the opcode requires it
        if b54 == 2 {
            self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
        } else if b54 == 3 {
            self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
        }
    }

    fn check_condition(&self, bits: u8) -> bool {
        match bits {
            0 => !self.regs.f.zero,
            1 => self.regs.f.zero,
            2 => !self.regs.f.carry,
            3 => self.regs.f.carry,
            _ => panic!("Invalid decode bits for condition check {:b}", bits),
        }
    }

    fn jr_cc_i8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b43 = (opcode & 0x18) >> 3;
        let offset = self.fetch(mmu) as i8 as i16 as u16;

        if self.check_condition(b43) {
            self.regs.pc = self.regs.pc.wrapping_add(offset);
            mmu.tick();
        }
    }

    fn jr_i8<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let offset = self.fetch(mmu) as i8 as i16 as u16;
        self.regs.pc = self.regs.pc.wrapping_add(offset);
        mmu.tick();
    }

    fn ld_r8_u8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = self.fetch(mmu);
        ByteRegister::for_r8(b543, self).set(operand, mmu);
    }

    fn cb_prefixed_opcodes<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let prefixed_opcode = self.fetch(mmu);

        match prefixed_opcode {
            0x00..=0x3F => self.rotate_and_swap_r8(prefixed_opcode, mmu),
            0x40..=0x7F => self.bit_n_r8(prefixed_opcode, mmu),
            0x80..=0xBF => self.res_n_r8(prefixed_opcode, mmu),
            0xC0..=0xFF => self.set_n_r8(prefixed_opcode, mmu),
        };
    }

    fn bit_n_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let operand = ByteRegister::for_r8(b321, self).get(mmu);
        self.regs.f.zero = operand & (1 << b543) == 0;
        self.regs.f.negative = false;
        self.regs.f.half_carry = true;
    }

    fn res_n_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let mut register = ByteRegister::for_r8(b321, self).get(mmu);
        register &= !(0x1 << b543);

        ByteRegister::for_r8(b321, self).set(register, mmu);
    }

    fn set_n_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let mut register = ByteRegister::for_r8(b321, self).get(mmu);
        register |= 0x1 << b543;

        ByteRegister::for_r8(b321, self).set(register, mmu);
    }

    fn inc_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = ByteRegister::for_r8(b543, self).get(mmu);

        let result = operand.wrapping_add(1);
        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = ((operand & 0x0F) + 0x1) > 0x0F;

        ByteRegister::for_r8(b543, self).set(result, mmu);
    }

    fn dec_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let operand = ByteRegister::for_r8(b543, self).get(mmu);

        let result = operand.wrapping_sub(1);
        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = true;
        self.regs.f.half_carry = operand & 0x0F == 0x00;

        ByteRegister::for_r8(b543, self).set(result, mmu);
    }

    fn halt<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        self.previous_execution_state = Some(mmu.system_state().execution_state);
        mmu.system_state().execution_state = ExecutionState::Halted;
    }

    fn ld_r8_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3; // Destination
        let b210 = opcode & 0x07; // Source

        let source = ByteRegister::for_r8(b210, self).get(mmu);
        ByteRegister::for_r8(b543, self).set(source, mmu);
    }

    fn ld_ff00_u8_a<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let offset = u16::from(self.fetch(mmu));
        mmu.write(0xFF00 + offset, self.regs.a);
    }

    fn ld_ff00_c_a<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        mmu.write(0xFF00 + u16::from(self.regs.c), self.regs.a);
    }

    fn ld_a_ff00_u8<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let offset = u16::from(self.fetch(mmu));
        self.regs.a = mmu.read(0xFF00 + offset);
    }

    fn ld_a_ff00_c<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        self.regs.a = mmu.read(0xFF00 + self.regs.c as u16);
    }

    fn call_u16<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let lsb = self.fetch(mmu);
        let msb = self.fetch(mmu);
        let jump_address = u16::from_be_bytes([msb, lsb]);

        // Pre-decrement SP
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.tick();

        // Write return location to stack
        let [pc_upper, pc_lower] = self.regs.pc.to_be_bytes();
        mmu.write(self.regs.sp, pc_upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.write(self.regs.sp, pc_lower);

        self.regs.pc = jump_address;
    }

    fn call_cc_u16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b43 = (opcode & 0x18) >> 3;

        if self.check_condition(b43) {
            self.call_u16(opcode, mmu);
        } else {
            // Fetch and discard the subroutine address
            self.fetch(mmu);
            self.fetch(mmu);
        }
    }

    fn push_r16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b54 = (opcode & 0x30) >> 4;
        let register_value = WordRegister::for_group3(b54, self).get();
        let [upper, lower] = register_value.to_be_bytes();

        // Pre-decrement of SP takes a cycle
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.tick();

        mmu.write(self.regs.sp, upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.write(self.regs.sp, lower);
    }

    fn pop_r16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b54 = (opcode & 0x30) >> 4;

        let lower = mmu.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);
        let upper = mmu.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);

        WordRegister::for_group3(b54, self).set(u16::from_be_bytes([upper, lower]));
    }

    fn rotate_and_swap_r8<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b543 = (opcode & 0x38) >> 3;
        let b321 = opcode & 0x07;

        let operand = ByteRegister::for_r8(b321, self).get(mmu);
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

        ByteRegister::for_r8(b321, self).set(result, mmu);
    }

    fn rlc(&mut self, mut operand: u8) -> u8 {
        let bit7 = u8::from(operand & 0x80 != 0);
        self.regs.f.carry = bit7 != 0;

        operand <<= 1;
        operand |= bit7;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn rrc(&mut self, mut operand: u8) -> u8 {
        let bit0 = u8::from(operand & 0x1 != 0);
        self.regs.f.carry = bit0 != 0;

        operand >>= 1;
        operand |= bit0 << 7;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn rl(&mut self, mut operand: u8) -> u8 {
        let carry = u8::from(self.regs.f.carry);
        self.regs.f.carry = (operand & 0x80) != 0;
        operand <<= 1;
        operand |= carry;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn rr(&mut self, mut operand: u8) -> u8 {
        let carry = u8::from(self.regs.f.carry);
        self.regs.f.carry = (operand & 0b1) != 0;
        operand >>= 1;
        operand |= carry << 7;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn sla(&mut self, mut operand: u8) -> u8 {
        self.regs.f.carry = operand & 0x80 != 0;
        operand <<= 1;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn sra(&mut self, mut operand: u8) -> u8 {
        self.regs.f.carry = operand & 0x1 != 0;
        operand = ((operand as i8) >> 1) as u8;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn swap(&mut self, operand: u8) -> u8 {
        let result = (operand >> 4) | (operand << 4);

        self.regs.f.zero = result == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = false;

        result
    }

    fn srl(&mut self, mut operand: u8) -> u8 {
        self.regs.f.carry = (operand & 0b1) != 0;
        operand >>= 1;

        self.regs.f.zero = operand == 0x00;
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;

        operand
    }

    fn flag_ops(&mut self, opcode: u8) {
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

    fn rlca(&mut self) -> u8 {
        let result = self.rlc(self.regs.a);
        self.regs.f.zero = false; // RLCA unsets zero flag always
        result
    }

    fn rrca(&mut self) -> u8 {
        let result = self.rrc(self.regs.a);
        self.regs.f.zero = false; // RRCA unsets zero flag always
        result
    }

    fn rla(&mut self) -> u8 {
        let result = self.rl(self.regs.a);
        self.regs.f.zero = false; // RLA unsets zero flag always
        result
    }

    fn rra(&mut self) -> u8 {
        let result = self.rr(self.regs.a);
        self.regs.f.zero = false; // RRA unsets zero flag always
        result
    }

    fn daa(&mut self) -> u8 {
        let mut correction = 0x00;

        if self.regs.f.half_carry || (!self.regs.f.negative && (self.regs.a & 0xF) > 9) {
            correction |= 0x6;
        }

        if self.regs.f.carry || (!self.regs.f.negative && (self.regs.a > 0x99)) {
            correction |= 0x60;
            self.regs.f.carry = true;
        }

        if self.regs.f.negative {
            self.regs.a = self.regs.a.wrapping_sub(correction);
        } else {
            self.regs.a = self.regs.a.wrapping_add(correction);
        }

        self.regs.f.zero = self.regs.a == 0x00;
        self.regs.f.half_carry = false;

        self.regs.a
    }

    fn cpl(&mut self) {
        self.regs.a = !self.regs.a;

        self.regs.f.negative = true;
        self.regs.f.half_carry = true;
    }

    fn scf(&mut self) {
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = true;
    }

    fn ccf(&mut self) {
        self.regs.f.negative = false;
        self.regs.f.half_carry = false;
        self.regs.f.carry = !self.regs.f.carry;
    }

    fn inc_r16(&mut self, opcode: u8) {
        let b54 = (opcode & 0x30) >> 4;
        WordRegister::for_group1(b54, self).inc();
    }

    fn dec_r16(&mut self, opcode: u8) {
        let b54 = (opcode & 0x30) >> 4;
        WordRegister::for_group1(b54, self).dec();
    }

    fn ret<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let lower = mmu.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);

        let upper = mmu.read(self.regs.sp);
        self.regs.sp = self.regs.sp.wrapping_add(1);

        self.regs.pc = u16::from_be_bytes([upper, lower]);
        // Final m-cycle
        mmu.tick();
    }

    fn reti<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        self.ret(opcode, mmu);
        self.ime = true;
    }

    fn ret_cc<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b43 = (opcode & 0x18) >> 3;

        mmu.tick(); // Internal branch decision

        if self.check_condition(b43) {
            self.ret(opcode, mmu);
        }
    }

    fn ld_u16_a<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let lower = self.fetch(mmu);
        let upper = self.fetch(mmu);

        let address = u16::from_be_bytes([upper, lower]);
        mmu.write(address, self.regs.a);
    }

    fn ld_a_u16<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let lower = self.fetch(mmu);
        let upper = self.fetch(mmu);

        let address = u16::from_be_bytes([upper, lower]);
        self.regs.a = mmu.read(address);
    }

    fn jp_u16<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let lower = self.fetch(mmu);
        let upper = self.fetch(mmu);

        self.regs.pc = u16::from_be_bytes([upper, lower]);
        mmu.tick();
    }

    fn jp_cc_u16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b43 = (opcode & 0x18) >> 3;

        if self.check_condition(b43) {
            self.jp_u16(opcode, mmu);
        } else {
            // Fetch and discard the jump address
            self.fetch(mmu);
            self.fetch(mmu);
        }
    }

    fn di(&mut self, _: u8) {
        self.ime = false;
    }

    fn add_hl_r16<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let b54 = (opcode & 0x30) >> 4;
        let operand = WordRegister::for_group1(b54, self).get();

        let (result, carry) = self.regs.get_hl().overflowing_add(operand);
        self.regs.f.negative = false;
        self.regs.f.carry = carry;
        self.regs.f.half_carry = (self.regs.get_hl() & 0xFFF) + (operand & 0xFFF) > 0xFFF;

        self.regs.set_hl(result);
        mmu.tick(); // Second cycle
    }

    fn jp_hl(&mut self, _: u8) {
        self.regs.pc = self.regs.get_hl();
    }

    fn add_sp_i8<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let operand = self.fetch(mmu) as i8 as i16 as u16;
        let result = self.regs.sp.wrapping_add(operand);

        self.regs.f.zero = false;
        self.regs.f.negative = false;
        self.regs.f.half_carry = (self.regs.sp & 0xF) + (operand & 0xF) > 0xF;
        self.regs.f.carry = (self.regs.sp & 0xFF) + (operand & 0xFF) > 0xFF;

        self.regs.sp = result;
    }

    fn ld_hl_sp_i8<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let operand = self.fetch(mmu) as i8 as i16 as u16;
        let result = self.regs.sp.wrapping_add(operand);

        self.regs.f.zero = false;
        self.regs.f.negative = false;
        self.regs.f.half_carry = (self.regs.sp & 0xF) + (operand & 0xF) > 0xF;
        self.regs.f.carry = (self.regs.sp & 0xFF) + (operand & 0xFF) > 0xFF;

        self.regs.set_hl(result);
    }

    fn ld_sp_hl<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        self.regs.sp = self.regs.get_hl();
        mmu.tick();
    }

    fn ei<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        // The effect of EI is delayed by one m-cycle
        // TODO: Check behaviour if EI is followed by a HALT

        // Don't execute another opcode if running CPU opcode tests
        #[cfg(not(test))]
        {
            self.execute_opcode(mmu);
        }
        self.ime = true;
    }

    fn ld_u16_sp<BusType: SystemBus>(&mut self, _: u8, mmu: &mut BusType) {
        let lower = self.fetch(mmu);
        let upper = self.fetch(mmu);
        let address = u16::from_be_bytes([upper, lower]);
        let [sp_upper, sp_lower] = self.regs.sp.to_be_bytes();

        mmu.write(address, sp_lower);
        mmu.write(address + 1, sp_upper);
    }

    fn rst<BusType: SystemBus>(&mut self, opcode: u8, mmu: &mut BusType) {
        let target = (opcode & 0x38) as u16;

        // Pre-decrement SP
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.tick();

        // Write return location to stack
        let [pc_upper, pc_lower] = self.regs.pc.to_be_bytes();
        mmu.write(self.regs.sp, pc_upper);
        self.regs.sp = self.regs.sp.wrapping_sub(1);
        mmu.write(self.regs.sp, pc_lower);

        self.regs.pc = target;
    }
}

enum FlagRegisterMask {
    Zero = 1 << 7,
    Negative = 1 << 6,
    HalfCarry = 1 << 5,
    Carry = 1 << 4,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct FlagRegister {
    pub zero: bool,
    pub negative: bool,
    pub half_carry: bool,
    pub carry: bool,
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

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registers {
    a: u8,
    pub f: FlagRegister,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,

    pub sp: u16,
    pub pc: u16,
}

macro_rules! register_pair {
    ($upper: ident, $lower: ident) => {
        paste! {
            pub fn [<get_ $upper $lower >](&self) -> u16 {
                // The `.into` is only required for `f`. For all else
                // it should be a NOP
                u16::from_be_bytes([self.$upper, self.$lower.into()])
            }

            pub fn [<set_ $upper $lower>](&mut self, value: u16) {
                let [msb, lsb] = value.to_be_bytes();

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
        lower: &'a mut u8,
        upper: &'a mut u8,
    },
    Single(&'a mut u16),
    /// For the AF register pair
    AccumAndFlag {
        a: &'a mut u8,
        f: &'a mut FlagRegister,
    },
}

impl<'a> WordRegister<'a> {
    pub fn for_group1(bits: u8, cpu: &'a mut Cpu) -> Self {
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

    pub fn for_group2(bits: u8, cpu: &'a mut Cpu) -> Self {
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

    pub fn for_group3(bits: u8, cpu: &'a mut Cpu) -> Self {
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

    pub fn get(&self) -> u16 {
        match self {
            WordRegister::Pair { lower, upper } => u16::from_be_bytes([**upper, **lower]),
            WordRegister::Single(reg) => **reg,
            WordRegister::AccumAndFlag { a, f } => u16::from_be_bytes([**a, u8::from(**f)]),
        }
    }

    pub fn set(&mut self, value: u16) {
        match self {
            WordRegister::Pair { lower, upper } => {
                let [msb, lsb] = value.to_be_bytes();
                **lower = lsb;
                **upper = msb;
            }
            WordRegister::Single(reg) => **reg = value,
            WordRegister::AccumAndFlag { a, f } => {
                let [msb, lsb] = value.to_be_bytes();
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

    fn dec(&mut self) {
        match self {
            WordRegister::Pair { lower, upper } => {
                let (dec_lower, borrow) = lower.overflowing_sub(1);
                **lower = dec_lower;

                if borrow {
                    **upper = upper.wrapping_sub(1);
                }
            }
            WordRegister::Single(reg) => {
                **reg = reg.wrapping_sub(1);
            }
            WordRegister::AccumAndFlag { .. } => panic!("DEC not required for AF register"),
        }
    }
}

enum ByteRegister<'a> {
    Register(&'a mut u8),
    MemoryReference(u16),
}

impl<'a> ByteRegister<'a> {
    pub fn for_r8(bits: u8, cpu: &'a mut Cpu) -> Self {
        match bits {
            0 => ByteRegister::Register(&mut cpu.regs.b),
            1 => ByteRegister::Register(&mut cpu.regs.c),
            2 => ByteRegister::Register(&mut cpu.regs.d),
            3 => ByteRegister::Register(&mut cpu.regs.e),
            4 => ByteRegister::Register(&mut cpu.regs.h),
            5 => ByteRegister::Register(&mut cpu.regs.l),
            6 => ByteRegister::MemoryReference(cpu.regs.get_hl()),
            7 => ByteRegister::Register(&mut cpu.regs.a),
            _ => panic!("Invalid decode bits for R8 register {:b}", bits),
        }
    }

    pub fn get<BusType: SystemBus>(&self, mmu: &mut BusType) -> u8 {
        match self {
            ByteRegister::Register(ptr) => **ptr,
            ByteRegister::MemoryReference(address) => mmu.read(*address),
        }
    }

    pub fn set<BusType: SystemBus>(&mut self, value: u8, mmu: &mut BusType) {
        match self {
            ByteRegister::Register(ptr) => **ptr = value,
            ByteRegister::MemoryReference(address) => mmu.write(*address, value),
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
pub mod tests {
    use std::fmt::Display;
    use std::fs::File;
    use std::io;
    use std::io::Read;
    use std::path::Path;
    use std::str::FromStr;

    use crate::interrupts::InterruptHandler;
    use num_traits;
    use serde::{Deserialize, Deserializer};

    use crate::memory::Memory;
    use crate::SystemState;

    use super::*;

    pub fn deserialize_number_from_string<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr + serde::Deserialize<'de> + num_traits::Unsigned,
        <T as FromStr>::Err: Display,
        <T as num_traits::Num>::FromStrRadixErr: Display,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrInt<T> {
            String(String),
            Number(T),
        }

        match StringOrInt::<T>::deserialize(deserializer)? {
            StringOrInt::String(s) => {
                T::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
            }
            StringOrInt::Number(i) => Ok(i),
        }
    }

    #[derive(Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
    struct CpuState {
        #[serde(deserialize_with = "deserialize_number_from_string")]
        a: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        b: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        c: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        d: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        e: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        f: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        h: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        l: u8,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        pc: u16,
        #[serde(deserialize_with = "deserialize_number_from_string")]
        sp: u16,
    }

    impl From<CpuState> for Registers {
        fn from(value: CpuState) -> Self {
            Self {
                a: value.a,
                f: FlagRegister::from(value.f),
                b: value.b,
                c: value.c,
                d: value.d,
                e: value.e,
                h: value.h,
                l: value.l,
                sp: value.sp,
                pc: value.pc,
            }
        }
    }

    #[derive(Deserialize, Debug)]
    struct RamState(
        #[serde(deserialize_with = "deserialize_number_from_string")] u16,
        #[serde(deserialize_with = "deserialize_number_from_string")] u8,
    );

    #[derive(Deserialize, Debug)]
    struct MemoryState {
        cpu: CpuState,
        ram: Vec<RamState>,
    }

    #[derive(Deserialize, Debug, Clone)]
    struct CycleState(
        #[serde(deserialize_with = "deserialize_number_from_string")] u16,
        #[serde(deserialize_with = "deserialize_number_from_string")] u8,
        String,
    );

    #[derive(Deserialize, Debug)]
    struct OpcodeTestCase {
        name: String,
        initial: MemoryState,
        r#final: MemoryState,
        cycles: Vec<Option<CycleState>>,
    }

    fn read_test_data(opcode: u8) -> io::Result<Vec<OpcodeTestCase>> {
        let path_str = format!("./sm83-test-data/cpu_tests/v1/{:02x}.json", opcode);
        let path = Path::new(&path_str);
        let mut test_data = String::new();
        File::open(path)?.read_to_string(&mut test_data)?;
        let deserialized_test_data = serde_json::from_str(&test_data).unwrap();
        Ok(deserialized_test_data)
    }

    struct FlatMmu<'a> {
        memory: [u8; 0x10000],
        ticked_cycle_count: u64,
        expected_cycles: &'a [Option<CycleState>],
        system_state: SystemState,
        interrupts: InterruptHandler,
    }

    impl<'a> FlatMmu<'a> {
        pub fn new(ram_states: &[RamState], expected_cycles: &'a [Option<CycleState>]) -> Self {
            let mut memory = [0x00; 0x10000];
            let system_state = SystemState::default();
            let interrupts = InterruptHandler::default();

            for ram_state in ram_states {
                memory[ram_state.0 as usize] = ram_state.1;
            }

            Self {
                memory,
                ticked_cycle_count: 0,
                expected_cycles,
                system_state,
                interrupts,
            }
        }
    }

    impl Memory for FlatMmu<'_> {
        fn read(&mut self, address: u16) -> u8 {
            let expected_cycle = self.expected_cycles[self.ticked_cycle_count as usize]
                .clone()
                .unwrap();
            assert_eq!(expected_cycle.0, address);
            assert_eq!(&expected_cycle.2, "read");

            self.tick();
            let data = self.unticked_read(address);
            assert_eq!(expected_cycle.1, data);

            data
        }

        fn write(&mut self, address: u16, data: u8) {
            let expected_cycle = self.expected_cycles[self.ticked_cycle_count as usize]
                .clone()
                .unwrap();
            assert_eq!(expected_cycle.0, address);
            assert_eq!(expected_cycle.1, data);
            assert_eq!(&expected_cycle.2, "write");

            self.tick();
            self.unticked_write(address, data);
        }
    }

    impl SystemBus for FlatMmu<'_> {
        fn unticked_read(&mut self, address: u16) -> u8 {
            self.memory[address as usize]
        }

        fn unticked_write(&mut self, address: u16, data: u8) {
            self.memory[address as usize] = data
        }

        fn tick(&mut self) {
            self.ticked_cycle_count += 1;
        }

        fn system_state(&mut self) -> &mut SystemState {
            &mut self.system_state
        }

        fn interrupt_handler(&mut self) -> &mut InterruptHandler {
            &mut self.interrupts
        }
    }

    fn run_test_case(test_case: &OpcodeTestCase) {
        let mut mmu = FlatMmu::new(&test_case.initial.ram, &test_case.cycles);
        let mut cpu = Cpu::new();
        cpu.regs = Registers::from(test_case.initial.cpu);
        cpu.execute_opcode(&mut mmu);

        assert_eq!(cpu.regs, test_case.r#final.cpu.into());
        for RamState(address, data) in &test_case.r#final.ram {
            assert_eq!(mmu.memory[*address as usize], *data);
        }
    }

    macro_rules! test_opcode {
        ($opcode: literal) => {
            paste! {
                #[test]
                fn [<test_ $opcode>]() -> io::Result<()> {
                    let test_data = read_test_data($opcode)?;
                    for test_case in test_data {
                        run_test_case(&test_case);
                    }
                    Ok(())
                }
            }
        };
    }

    test_opcode!(0x00);
    test_opcode!(0x01);
    test_opcode!(0x02);
    test_opcode!(0x03);
    test_opcode!(0x04);
    test_opcode!(0x05);
    test_opcode!(0x06);
    test_opcode!(0x07);
    test_opcode!(0x08);
    test_opcode!(0x09);
    test_opcode!(0x0A);
    test_opcode!(0x0B);
    test_opcode!(0x0C);
    test_opcode!(0x0D);
    test_opcode!(0x0E);
    test_opcode!(0x0F);
    test_opcode!(0x10);
    test_opcode!(0x11);
    test_opcode!(0x12);
    test_opcode!(0x13);
    test_opcode!(0x14);
    test_opcode!(0x15);
    test_opcode!(0x16);
    test_opcode!(0x17);
    test_opcode!(0x18);
    test_opcode!(0x19);
    test_opcode!(0x1A);
    test_opcode!(0x1B);
    test_opcode!(0x1C);
    test_opcode!(0x1D);
    test_opcode!(0x1E);
    test_opcode!(0x1F);
    test_opcode!(0x20);
    test_opcode!(0x21);
    test_opcode!(0x22);
    test_opcode!(0x23);
    test_opcode!(0x24);
    test_opcode!(0x25);
    test_opcode!(0x26);
    test_opcode!(0x27);
    test_opcode!(0x28);
    test_opcode!(0x29);
    test_opcode!(0x2A);
    test_opcode!(0x2B);
    test_opcode!(0x2C);
    test_opcode!(0x2D);
    test_opcode!(0x2E);
    test_opcode!(0x2F);
    test_opcode!(0x30);
    test_opcode!(0x31);
    test_opcode!(0x32);
    test_opcode!(0x33);
    test_opcode!(0x34);
    test_opcode!(0x35);
    test_opcode!(0x36);
    test_opcode!(0x37);
    test_opcode!(0x38);
    test_opcode!(0x39);
    test_opcode!(0x3A);
    test_opcode!(0x3B);
    test_opcode!(0x3C);
    test_opcode!(0x3D);
    test_opcode!(0x3E);
    test_opcode!(0x3F);
    test_opcode!(0x40);
    test_opcode!(0x41);
    test_opcode!(0x42);
    test_opcode!(0x43);
    test_opcode!(0x44);
    test_opcode!(0x45);
    test_opcode!(0x46);
    test_opcode!(0x47);
    test_opcode!(0x48);
    test_opcode!(0x49);
    test_opcode!(0x4A);
    test_opcode!(0x4B);
    test_opcode!(0x4C);
    test_opcode!(0x4D);
    test_opcode!(0x4E);
    test_opcode!(0x4F);
    test_opcode!(0x50);
    test_opcode!(0x51);
    test_opcode!(0x52);
    test_opcode!(0x53);
    test_opcode!(0x54);
    test_opcode!(0x55);
    test_opcode!(0x56);
    test_opcode!(0x57);
    test_opcode!(0x58);
    test_opcode!(0x59);
    test_opcode!(0x5A);
    test_opcode!(0x5B);
    test_opcode!(0x5C);
    test_opcode!(0x5D);
    test_opcode!(0x5E);
    test_opcode!(0x5F);
    test_opcode!(0x60);
    test_opcode!(0x61);
    test_opcode!(0x62);
    test_opcode!(0x63);
    test_opcode!(0x64);
    test_opcode!(0x65);
    test_opcode!(0x66);
    test_opcode!(0x67);
    test_opcode!(0x68);
    test_opcode!(0x69);
    test_opcode!(0x6A);
    test_opcode!(0x6B);
    test_opcode!(0x6C);
    test_opcode!(0x6D);
    test_opcode!(0x6E);
    test_opcode!(0x6F);
    test_opcode!(0x70);
    test_opcode!(0x71);
    test_opcode!(0x72);
    test_opcode!(0x73);
    test_opcode!(0x74);
    test_opcode!(0x75);
    test_opcode!(0x76);
    test_opcode!(0x77);
    test_opcode!(0x78);
    test_opcode!(0x79);
    test_opcode!(0x7A);
    test_opcode!(0x7B);
    test_opcode!(0x7C);
    test_opcode!(0x7D);
    test_opcode!(0x7E);
    test_opcode!(0x7F);
    test_opcode!(0x80);
    test_opcode!(0x81);
    test_opcode!(0x82);
    test_opcode!(0x83);
    test_opcode!(0x84);
    test_opcode!(0x85);
    test_opcode!(0x86);
    test_opcode!(0x87);
    test_opcode!(0x88);
    test_opcode!(0x89);
    test_opcode!(0x8A);
    test_opcode!(0x8B);
    test_opcode!(0x8C);
    test_opcode!(0x8D);
    test_opcode!(0x8E);
    test_opcode!(0x8F);
    test_opcode!(0x90);
    test_opcode!(0x91);
    test_opcode!(0x92);
    test_opcode!(0x93);
    test_opcode!(0x94);
    test_opcode!(0x95);
    test_opcode!(0x96);
    test_opcode!(0x97);
    test_opcode!(0x98);
    test_opcode!(0x99);
    test_opcode!(0x9A);
    test_opcode!(0x9B);
    test_opcode!(0x9C);
    test_opcode!(0x9D);
    test_opcode!(0x9E);
    test_opcode!(0x9F);
    test_opcode!(0xA0);
    test_opcode!(0xA1);
    test_opcode!(0xA2);
    test_opcode!(0xA3);
    test_opcode!(0xA4);
    test_opcode!(0xA5);
    test_opcode!(0xA6);
    test_opcode!(0xA7);
    test_opcode!(0xA8);
    test_opcode!(0xA9);
    test_opcode!(0xAA);
    test_opcode!(0xAB);
    test_opcode!(0xAC);
    test_opcode!(0xAD);
    test_opcode!(0xAE);
    test_opcode!(0xAF);
    test_opcode!(0xB0);
    test_opcode!(0xB1);
    test_opcode!(0xB2);
    test_opcode!(0xB3);
    test_opcode!(0xB4);
    test_opcode!(0xB5);
    test_opcode!(0xB6);
    test_opcode!(0xB7);
    test_opcode!(0xB8);
    test_opcode!(0xB9);
    test_opcode!(0xBA);
    test_opcode!(0xBB);
    test_opcode!(0xBC);
    test_opcode!(0xBD);
    test_opcode!(0xBE);
    test_opcode!(0xBF);
    test_opcode!(0xC0);
    test_opcode!(0xC1);
    test_opcode!(0xC2);
    test_opcode!(0xC3);
    test_opcode!(0xC4);
    test_opcode!(0xC5);
    test_opcode!(0xC6);
    test_opcode!(0xC7);
    test_opcode!(0xC8);
    test_opcode!(0xC9);
    test_opcode!(0xCA);
    test_opcode!(0xCB);
    test_opcode!(0xCC);
    test_opcode!(0xCD);
    test_opcode!(0xCE);
    test_opcode!(0xCF);
    test_opcode!(0xD0);
    test_opcode!(0xD1);
    test_opcode!(0xD2);
    test_opcode!(0xD4);
    test_opcode!(0xD5);
    test_opcode!(0xD6);
    test_opcode!(0xD7);
    test_opcode!(0xD8);
    test_opcode!(0xD9);
    test_opcode!(0xDA);
    test_opcode!(0xDC);
    test_opcode!(0xDE);
    test_opcode!(0xDF);
    test_opcode!(0xE0);
    test_opcode!(0xE1);
    test_opcode!(0xE2);
    test_opcode!(0xE5);
    test_opcode!(0xE6);
    test_opcode!(0xE7);
    test_opcode!(0xE8);
    test_opcode!(0xE9);
    test_opcode!(0xEA);
    test_opcode!(0xEE);
    test_opcode!(0xEF);
    test_opcode!(0xF0);
    test_opcode!(0xF1);
    test_opcode!(0xF2);
    test_opcode!(0xF3);
    test_opcode!(0xF5);
    test_opcode!(0xF6);
    test_opcode!(0xF7);
    test_opcode!(0xF8);
    test_opcode!(0xF9);
    test_opcode!(0xFA);
    test_opcode!(0xFB);
    test_opcode!(0xFE);
}
