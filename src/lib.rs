#![allow(dead_code)] // Only for development

mod cartridge;
mod cpu;
pub mod gameboy;
mod interrupts;
mod memory;
mod mmu;
mod opcodes;
pub mod options;
mod timer;
mod utils;

#[cfg(test)]
mod tests {}
