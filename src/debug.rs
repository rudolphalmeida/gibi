use crate::cpu::Registers;

#[derive(Debug, Copy, Clone, Default)]
pub struct ExecutedOpcode {
    pub pc: u16,
    pub opcode: u8,
    pub arg1: u8,
    pub arg2: u8,
}

#[derive(Debug, Clone, Default)]
pub struct CpuDebug {
    pub registers: Registers,
    pub opcodes: Vec<ExecutedOpcode>,
}