use std::cell::RefCell;
use std::rc::Rc;

use crate::apu::{Apu, SOUND_END, SOUND_START, WAVE_END, WAVE_START};
use crate::ppu::{PALETTE_END, PALETTE_START, PPU_REGISTERS_END, PPU_REGISTERS_START};
use crate::serial::{Serial, SERIAL_END, SERIAL_START};
use crate::timer::{Timer, TIMER_END, TIMER_START};
use crate::{
    cartridge::{
        load_from_file, Cartridge, BOOT_ROM, BOOT_ROM_END, BOOT_ROM_START, CART_RAM_END,
        CART_RAM_START, CART_ROM_END, CART_ROM_START,
    },
    joypad::{Joypad, JOYP_ADDRESS},
    memory::Memory,
    options::Options,
    ppu::{Ppu, OAM_END, OAM_START, VRAM_END, VRAM_START},
    utils::{Byte, Word},
};

const WRAM_BANK_SIZE: usize = 1024 * 4;
// 4KB
const HRAM_SIZE: usize = 0xFFFE - 0xFF80 + 1;

const WRAM_START: Word = 0xC000;
const WRAM_END: Word = 0xDFFF;
const WRAM_ECHO_START: Word = 0xE000;
const WRAM_ECHO_END: Word = 0xFDFF;

const UNUSED_START: Word = 0xFEA0;
const UNUSED_END: Word = 0xFEFF;

const BOOTROM_DISABLE: Word = 0xFF50;

const VRAM_DMA_START: Word = 0xFF51;
const VRAM_DMA_END: Word = 0xFF55;

const WRAM_BANK_SELECT: Word = 0xFF70;

const HRAM_START: Word = 0xFF80;
const HRAM_END: u16 = 0xFFFE;

/// The MMU (Memory-Management Unit) is responsible for connecting the CPU to
/// the rest of the components. The `Mmu` implements the memory-map of the
/// GameBoy, redirecting reads and writes made by the CPU and PPU to the
/// correct component to which the address is mapped.
pub(crate) struct Mmu {
    pub(crate) cart: Box<dyn Cartridge>,
    ppu: Rc<RefCell<Ppu>>,
    apu: Rc<RefCell<Apu>>,
    wram: Vec<Byte>,
    hram: Vec<Byte>,
    joypad: Joypad,
    serial: Serial,
    timer: Timer,
    bootrom_enabled: bool,
}

impl Mmu {
    pub fn new(options: &Options, ppu: Rc<RefCell<Ppu>>, apu: Rc<RefCell<Apu>>) -> Self {
        let cart = load_from_file(options).unwrap();
        let wram = Vec::from([0x00; WRAM_BANK_SIZE * 2]); // 8KB
        let hram = Vec::from([0x00; HRAM_SIZE]);
        let joypad = Joypad::new();
        let serial = Serial::new();
        let timer = Timer::new();

        let bootrom_enabled = true;

        log::debug!("Initialized MMU for DMG");

        Self {
            cart,
            wram,
            hram,
            ppu,
            joypad,
            bootrom_enabled,
            serial,
            timer,
            apu,
        }
    }

    pub fn tick(&self) {
        self.timer.tick();
        self.ppu.borrow_mut().tick();
        self.apu.borrow_mut().tick();
    }

    /// Raw Read: Read the contents of a memory location without ticking all the
    /// components
    pub fn raw_read(&self, address: u16) -> Byte {
        match address {
            BOOT_ROM_START..=BOOT_ROM_END if self.bootrom_enabled => {
                return BOOT_ROM[address as usize]
            }
            CART_ROM_START..=CART_ROM_END => return self.cart.read(address),
            VRAM_START..=VRAM_END => return self.ppu.borrow().read(address),
            CART_RAM_START..=CART_RAM_END => return self.cart.read(address),
            WRAM_START..=WRAM_END => return self.wram[(address - WRAM_START) as usize],
            WRAM_ECHO_START..=WRAM_ECHO_END => {
                return self.wram[(address - WRAM_ECHO_START) as usize]
            }
            OAM_START..=OAM_END => return self.ppu.borrow().read(address),
            UNUSED_START..=UNUSED_END => log::error!("Read from unused area {:#06X}", address),
            JOYP_ADDRESS => return self.joypad.read(address),
            SERIAL_START..=SERIAL_END => return self.serial.read(address),
            TIMER_START..=TIMER_END => return self.timer.read(address),
            SOUND_START..=SOUND_END => return self.apu.borrow().read(address),
            WAVE_START..=WAVE_END => return self.apu.borrow().read(address),
            PPU_REGISTERS_START..=PPU_REGISTERS_END => return self.ppu.borrow().read(address),
            BOOTROM_DISABLE => return if self.bootrom_enabled { 0x01 } else { 0x00 },
            VRAM_DMA_START..=VRAM_DMA_END => {}
            PALETTE_START..=PALETTE_END => return self.ppu.borrow().read(address),
            WRAM_BANK_SELECT => {}
            HRAM_START..=HRAM_END => return self.hram[(address - HRAM_START) as usize],
            0xFFFF => log::info!("Read from IE register {:#06X}", address),
            _ => log::error!("Unknown address to Mmu::read {:#06X}", address),
        }
        0xFF
    }

    fn raw_write(&mut self, address: Word, data: Byte) {
        match address {
            BOOT_ROM_START..=BOOT_ROM_END if self.bootrom_enabled => {
                log::error!("Write to boot ROM {:#06X} with {:#04X}", address, data)
            }
            CART_ROM_START..=CART_ROM_END => self.cart.write(address, data),
            VRAM_START..=VRAM_END => self.ppu.borrow_mut().write(address, data),
            CART_RAM_START..=CART_RAM_END => self.cart.write(address, data),
            WRAM_START..=WRAM_END => self.wram[address as usize - 0xC000] = data,
            WRAM_ECHO_START..=WRAM_ECHO_END => self.wram[address as usize - 0xE000] = data,
            OAM_START..=OAM_END => self.ppu.borrow_mut().write(address, data),
            UNUSED_START..=UNUSED_END => {
                log::error!("Write to unused area {:#6X} with {:#04X}", address, data)
            }
            JOYP_ADDRESS => self.joypad.write(address, data),
            SERIAL_START..=SERIAL_END => self.serial.write(address, data),
            TIMER_START..=TIMER_END => self.timer.write(address, data),
            SOUND_START..=SOUND_END => self.apu.borrow_mut().write(address, data),
            WAVE_START..=WAVE_END => self.apu.borrow_mut().write(address, data),
            PPU_REGISTERS_START..=PPU_REGISTERS_END => self.ppu.borrow_mut().write(address, data),
            BOOTROM_DISABLE => {
                self.bootrom_enabled = data == 0x00;
                if !self.bootrom_enabled {
                    log::info!("Boot ROM disabled");
                }
            }
            VRAM_DMA_START..=VRAM_DMA_END => {}
            PALETTE_START..=PALETTE_END => self.ppu.borrow_mut().write(address, data),
            WRAM_BANK_SELECT => {}
            HRAM_START..=HRAM_END => self.hram[address as usize - 0xFF80] = data,
            0xFFFF => log::info!("Write to IE register {:#06X} with {:#04X}", address, data),
            _ => log::error!("Unknown address to Mmu::write {:#06X}", address),
        }
    }
}

impl Memory for Mmu {
    /// Read and Write for Mmu. The `Memory` trait is not implemented for the Mmu
    /// because `read` here needs to take a mutable reference to `self` due to
    /// using `tick` inside it. We want the other components to keep up with the
    /// CPU during each memory access
    fn read(&self, address: Word) -> Byte {
        self.tick();
        self.raw_read(address)
    }

    fn write(&mut self, address: Word, data: Byte) {
        self.tick();
        self.raw_write(address, data);
    }
}
