use std::path::PathBuf;

use std::{fs, io};

use crate::cartridge::{Cartridge, CartridgeHeader};
use crate::debug::CpuDebug;
use crate::framebuffer::access;
use crate::joypad::JoypadKeys;
use crate::memory::SystemBus;
use crate::HardwareSupport;
use crate::{cpu::Cpu, mmu::Mmu, GameFrame};

const CYCLES_PER_FRAME: u64 = 17556;

pub struct Gameboy {
    mmu: Mmu,
    cpu: Cpu<Mmu>,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>, ram: Option<Vec<u8>>) -> (Self, CartridgeHeader) {
        let cart = Cartridge::new(rom, ram).unwrap();
        let header = cart.header.clone();

        log::info!("Loaded a cartridge with title: {}", header.title);
        log::info!("MBC type: {}", header.cart_type);
        log::info!("Number of ROM banks: {}", header.rom_banks());
        log::info!("ROM size (Bytes): {}", header.rom_size());
        log::info!("Number of RAM banks: {}", header.ram_banks());
        log::info!("RAM size (Bytes): {}", header.ram_size());

        match header.hardware_supported {
            HardwareSupport::CgbOnly => log::info!("Game supports CGB hardware only"),
            HardwareSupport::DmgCgb => log::info!("Game supports both CGB and DMG"),
            HardwareSupport::DmgCompat => log::info!("Game is running in DMG compatibility mode"),
        }

        let mmu = Mmu::new(cart);
        let cpu = Cpu::new();
        (Gameboy { mmu, cpu }, header)
    }

    pub fn load_cpu_debug(&self) -> CpuDebug {
        self.cpu.debug()
    }

    pub fn run_one_frame(&mut self) {
        let machine_cycles = self.mmu.system_state().total_cycles;
        let carry_over_cycles = self.mmu.system_state().carry_over_cycles;
        let speed_multiplier = self.mmu.system_state().speed_divider();

        let target_machine_cycles =
            machine_cycles + CYCLES_PER_FRAME * speed_multiplier - carry_over_cycles;

        while self.mmu.system_state().total_cycles < target_machine_cycles {
            self.cpu.execute(&mut self.mmu);
        }

        let carry_over_cycles = self.mmu.system_state().total_cycles - target_machine_cycles;
        self.mmu.system_state().carry_over_cycles = carry_over_cycles;
    }

    pub fn write_frame(&self, frame_writer: &mut access::AccessW<GameFrame>) {
        self.mmu.ppu.write_frame(frame_writer);
    }

    pub fn keydown(&mut self, key: JoypadKeys) {
        self.mmu.keydown(key);
    }

    pub fn keyup(&mut self, key: JoypadKeys) {
        self.mmu.keyup(key);
    }

    pub fn save(&self, path: &PathBuf) -> io::Result<String> {
        if let Some(ram) = self.mmu.save_ram() {
            fs::write(path, ram).map(|_| "Save RAM to file".into())
        } else {
            Ok("Game does not have battery-backed saves".into())
        }
    }
}
