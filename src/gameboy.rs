use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::{cell::RefCell, rc::Rc};
use std::{fs, io};

use crate::apu::Apu;
use crate::cartridge::init_mbc_from_rom;
use crate::interrupts::InterruptHandler;
use crate::joypad::{Joypad, JoypadKeys};
use crate::{cpu::Cpu, mmu::Mmu, ppu::Ppu};
use crate::{EmulatorEvent, ExecutionState, Frame, HardwareSupport, HdmaState, SystemState};

const CYCLES_PER_FRAME: u64 = 17556;

pub struct Gameboy {
    mmu: Rc<RefCell<Mmu>>,
    joypad: Rc<RefCell<Joypad>>,
    cpu: Cpu,

    system_state: Rc<RefCell<SystemState>>,
    event_tx: Sender<EmulatorEvent>,
}

impl Gameboy {
    pub fn new(
        frame: Frame,
        rom: Vec<u8>,
        ram: Option<Vec<u8>>,
        event_tx: Sender<EmulatorEvent>,
    ) -> Self {
        let cart = init_mbc_from_rom(rom, ram);
        let hardware_support = cart.hardware_supported();

        match hardware_support {
            HardwareSupport::CgbOnly => log::info!("Game supports CGB hardware only"),
            HardwareSupport::DmgCgb => log::info!("Game supports both CGB and DMG"),
            HardwareSupport::DmgCompat => log::info!("Game is running in DMG compatability mode"),
        }

        let system_state = Rc::new(RefCell::new(SystemState {
            execution_state: ExecutionState::ExecutingProgram,
            hardware_support,
            carry_over_cycles: 0,
            total_cycles: 0,
            key1: 0x00,
            bootrom_mapped: true,
            hdma_state: HdmaState {
                source_addr: 0xFFFF,
                dest_addr: 0xFFFF,
                hdma_stat: 0x00,
            },
        }));

        let interrupts = Rc::new(RefCell::new(InterruptHandler::default()));
        let joypad = Rc::new(RefCell::new(Joypad::new(Rc::clone(&interrupts))));
        let mmu = Rc::new(RefCell::new(Mmu::new(
            cart,
            Rc::clone(&system_state),
            Rc::clone(&joypad),
            Rc::clone(&interrupts),
            frame,
            event_tx.clone(),
        )));
        let cpu = Cpu::new(Rc::clone(&mmu), interrupts, Rc::clone(&system_state));

        {
            let mmu = mmu.borrow();
            log::info!("Loaded a cartridge with MBC: {}", mmu.cart.name());
            log::info!("Number of ROM banks: {}", mmu.cart.rom_banks());
            log::info!("ROM size (Bytes): {}", mmu.cart.rom_size());
            log::info!("Number of RAM banks: {}", mmu.cart.ram_banks());
            log::info!("RAM size (Bytes): {}", mmu.cart.ram_size());
        }
        Gameboy {
            system_state,
            mmu,
            cpu,
            joypad,
            event_tx,
        }
    }

    pub fn send_debug_data(&self) {
        let cpu_registers = self.cpu.regs;
        self.event_tx
            .send(EmulatorEvent::CpuRegisters(cpu_registers))
            .unwrap();
    }

    pub fn run_one_frame(&mut self) {
        let machine_cycles = self.system_state.borrow().total_cycles;
        let carry_over_cycles = self.system_state.borrow().carry_over_cycles;
        let speed_multiplier = self.system_state.borrow().speed_multiplier();

        let target_machine_cycles =
            machine_cycles + CYCLES_PER_FRAME * speed_multiplier - carry_over_cycles;

        while self.system_state.borrow().total_cycles < target_machine_cycles {
            self.cpu.execute();
        }

        let carry_over_cycles = self.system_state.borrow().total_cycles - target_machine_cycles;
        self.system_state.borrow_mut().carry_over_cycles = carry_over_cycles;
    }

    pub fn keydown(&mut self, key: JoypadKeys) {
        self.joypad.borrow_mut().keydown(key);
    }

    pub fn keyup(&mut self, key: JoypadKeys) {
        self.joypad.borrow_mut().keyup(key);
    }

    pub fn save(&self, path: PathBuf) -> io::Result<String> {
        if let Some(ram) = self.mmu.borrow().save_ram() {
            fs::write(path, ram).map(|_| "Save RAM to file".into())
        } else {
            Ok("Game does not have battery-backed saves".into())
        }
    }
}
