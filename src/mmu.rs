use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::apu::{Apu, SOUND_END, SOUND_START, WAVE_END, WAVE_START};
use crate::cartridge::{HardwareSupport, CGB_BOOT_ROM};
use crate::interrupts::{InterruptHandler, INTERRUPT_ENABLE_ADDRESS, INTERRUPT_FLAG_ADDRESS};
use crate::ppu::{
    OAM_DMA_ADDRESS, OAM_DMA_CYCLES, PALETTE_END, PALETTE_START, PPU_REGISTERS_END,
    PPU_REGISTERS_START, VRAM_BANK_ADDRESS,
};
use crate::serial::{Serial, SERIAL_END, SERIAL_START};
use crate::timer::{Timer, TIMER_END, TIMER_START};
use crate::utils::Cycles;
use crate::{
    cartridge::{
        Cartridge, BOOT_ROM_END, BOOT_ROM_START, CART_RAM_END, CART_RAM_START, CART_ROM_END,
        CART_ROM_START,
    },
    joypad::{Joypad, JOYP_ADDRESS},
    memory::Memory,
    ppu::{Ppu, OAM_END, OAM_START, VRAM_END, VRAM_START},
    utils::{Byte, Word},
};

const CART_HEADER_START: Word = 0x100;
const CART_HEADER_END: Word = 0x1FF;

const WRAM_BANK_SIZE: usize = 1024 * 4; // 4KB
const HRAM_SIZE: usize = 0xFFFE - 0xFF80 + 1;

const WRAM_FIXED_START: Word = 0xC000;
const WRAM_FIXED_END: Word = 0xCFFF;
const WRAM_BANKED_START: Word = 0xD000;
const WRAM_BANKED_END: Word = 0xDFFF;
const WRAM_ECHO_START: Word = 0xE000;
const WRAM_ECHO_END: Word = 0xFDFF;

const UNUSED_START: Word = 0xFEA0;
const UNUSED_END: Word = 0xFEFF;

const KEY1: Word = 0xFF4D;

const BOOTROM_DISABLE: Word = 0xFF50;

const VRAM_DMA_START: Word = 0xFF51;
const VRAM_DMA_END: Word = 0xFF55;

const WRAM_BANK_SELECT: Word = 0xFF70;

const HRAM_START: Word = 0xFF80;
const HRAM_END: u16 = 0xFFFE;

struct OamDma {
    pending_cycles: Cycles,
    next_address: Word,
}

/// The MMU (Memory-Management Unit) is responsible for connecting the CPU to
/// the rest of the components. The `Mmu` implements the memory-map of the
/// GameBoy, redirecting reads and writes made by the CPU and PPU to the
/// correct component to which the address is mapped.
pub(crate) struct Mmu {
    pub(crate) cart: Box<dyn Cartridge>,
    ppu: Rc<RefCell<Ppu>>,
    apu: Rc<RefCell<Apu>>,

    /// Hardware supported by the current cartridge
    hardware_supported: HardwareSupport,

    // Only two banks are used in DMG mode
    // CGB mode uses all 8, with 0 being fixed, and 1-7 being switchable
    wram: [Byte; WRAM_BANK_SIZE * 8],
    wram_bank: usize,

    hram: [Byte; HRAM_SIZE],
    joypad: Rc<RefCell<Joypad>>,
    serial: Serial,
    timer: RefCell<Timer>,
    bootrom_enabled: bool,
    interrupts: Rc<RefCell<InterruptHandler>>,
    /// M-cycles taken by the CPU since start of execution. This will take a
    /// long time to overflow
    pub cpu_m_cycles: Cell<Cycles>,
    key1: Byte,

    // DMAs
    oam_dma: RefCell<Option<OamDma>>,
}

impl Mmu {
    pub fn new(
        cart: Box<dyn Cartridge>,
        hardware_supported: HardwareSupport,
        ppu: Rc<RefCell<Ppu>>,
        apu: Rc<RefCell<Apu>>,
        joypad: Rc<RefCell<Joypad>>,
        interrupts: Rc<RefCell<InterruptHandler>>,
    ) -> Self {
        let wram = [0x00; WRAM_BANK_SIZE * 8]; // 32KB
        let wram_bank = 0x1;

        let hram = [0x00; HRAM_SIZE];
        let serial = Serial::new();
        let timer = RefCell::new(Timer::new(Rc::clone(&interrupts)));

        let cpu_m_cycles = Cell::new(0);
        let key1 = 0x00;

        let bootrom_enabled = true;
        let oam_dma = RefCell::new(None);

        log::debug!("Initialized MMU for CGB");

        Self {
            cart,
            hardware_supported,
            wram,
            wram_bank,
            hram,
            ppu,
            joypad,
            bootrom_enabled,
            serial,
            timer,
            apu,
            interrupts,
            cpu_m_cycles,
            key1,
            oam_dma,
        }
    }

    pub fn tick(&self) {
        self.cpu_m_cycles.set(self.cpu_m_cycles.get() + 1);

        self.tick_oam_dma();

        self.timer.borrow_mut().tick();
        self.joypad.borrow_mut().tick();
        self.ppu.borrow_mut().tick(self.speed_multiplier());
        self.apu.borrow_mut().tick();
    }

    fn tick_oam_dma(&self) {
        // Perform DMA
        let mut oam_dma_completed = false;
        if let Some(oam_dma) = self.oam_dma.borrow_mut().as_mut() {
            let dest_address = 0xFE00 | (oam_dma.next_address & 0x00FF);
            let data = self.raw_read(oam_dma.next_address);
            self.ppu.borrow_mut().write(dest_address, data);

            oam_dma.next_address += 1;
            match oam_dma.pending_cycles.checked_sub(1) {
                None => oam_dma_completed = true,
                Some(x) => oam_dma.pending_cycles = x,
            }
        }

        if oam_dma_completed {
            *self.oam_dma.borrow_mut() = None;
        }
    }

    fn oam_dma_in_progress(&self) -> bool {
        self.oam_dma.borrow().is_some()
    }

    /// Raw Read: Read the contents of a memory location without ticking all the
    /// components
    pub fn raw_read(&self, address: u16) -> Byte {
        match address {
            CART_HEADER_START..=CART_HEADER_END => return self.cart.read(address),
            BOOT_ROM_START..=BOOT_ROM_END if self.bootrom_enabled => {
                return CGB_BOOT_ROM[address as usize]
            }
            CART_ROM_START..=CART_ROM_END => return self.cart.read(address),
            VRAM_START..=VRAM_END => return self.ppu.borrow().read(address),
            CART_RAM_START..=CART_RAM_END => return self.cart.read(address),
            WRAM_FIXED_START..=WRAM_FIXED_END => {
                return self.wram[(address - WRAM_FIXED_START) as usize]
            }
            // Switchable bank for WRAM
            WRAM_BANKED_START..=WRAM_BANKED_END => return self.wram_banked_read(address),
            WRAM_ECHO_START..=WRAM_ECHO_END => return self.raw_read(address - WRAM_ECHO_START),
            OAM_START..=OAM_END => return self.ppu.borrow().read(address),
            UNUSED_START..=UNUSED_END => {}
            JOYP_ADDRESS => return self.joypad.borrow().read(address),
            SERIAL_START..=SERIAL_END => return self.serial.read(address),
            TIMER_START..=TIMER_END => return self.timer.borrow().read(address),
            INTERRUPT_FLAG_ADDRESS => return self.interrupts.borrow().read(address),
            SOUND_START..=SOUND_END => return self.apu.borrow().read(address),
            WAVE_START..=WAVE_END => return self.apu.borrow().read(address),
            OAM_DMA_ADDRESS => return 0xFF, // TODO: Check if this is correct
            PPU_REGISTERS_START..=PPU_REGISTERS_END => return self.ppu.borrow().read(address),
            VRAM_BANK_ADDRESS => return self.ppu.borrow().read(address),
            KEY1 => return self.key1,
            BOOTROM_DISABLE => return u8::from(self.bootrom_enabled),
            VRAM_DMA_START..=VRAM_DMA_END => {}
            PALETTE_START..=PALETTE_END => return self.ppu.borrow().read(address),
            WRAM_BANK_SELECT => return self.wram_bank as Byte,
            HRAM_START..=HRAM_END => return self.hram[(address - HRAM_START) as usize],
            INTERRUPT_ENABLE_ADDRESS => return self.interrupts.borrow().read(address),
            _ => log::error!("Unknown address to Mmu::read {:#06X}", address),
        }
        0xFF
    }

    fn wram_banked_read(&self, address: Word) -> Byte {
        // FIXME: Index out of range errors
        let index = WRAM_BANK_SIZE
            * if self.wram_bank == 0x00 {
                1
            } else {
                self.wram_bank
            }
            + (address - WRAM_BANKED_START) as usize;

        self.wram[index]
    }

    fn wram_banked_write(&mut self, address: Word, data: Byte) {
        // FIXME: Index out of range errors
        let index = WRAM_BANK_SIZE
            * if self.wram_bank == 0x00 {
                1
            } else {
                self.wram_bank
            }
            + (address - WRAM_BANKED_START) as usize;

        self.wram[index] = data
    }

    fn raw_write(&mut self, address: Word, data: Byte) {
        match address {
            CART_HEADER_START..=CART_HEADER_END => self.cart.write(address, data),
            BOOT_ROM_START..=BOOT_ROM_END if self.bootrom_enabled => {
                log::error!("Write to boot ROM {:#06X} with {:#04X}", address, data)
            }
            CART_ROM_START..=CART_ROM_END => self.cart.write(address, data),
            VRAM_START..=VRAM_END => self.ppu.borrow_mut().write(address, data),
            CART_RAM_START..=CART_RAM_END => self.cart.write(address, data),
            WRAM_FIXED_START..=WRAM_FIXED_END => {
                self.wram[(address - WRAM_FIXED_START) as usize] = data
            }
            WRAM_BANKED_START..=WRAM_BANKED_END => self.wram_banked_write(address, data),
            WRAM_ECHO_START..=WRAM_ECHO_END => self.raw_write(address - WRAM_ECHO_START, data),
            OAM_START..=OAM_END => self.ppu.borrow_mut().write(address, data),
            UNUSED_START..=UNUSED_END => {}
            JOYP_ADDRESS => self.joypad.borrow_mut().write(address, data),
            SERIAL_START..=SERIAL_END => self.serial.write(address, data),
            TIMER_START..=TIMER_END => self.timer.borrow_mut().write(address, data),
            INTERRUPT_FLAG_ADDRESS => self.interrupts.borrow_mut().write(address, data),
            SOUND_START..=SOUND_END => self.apu.borrow_mut().write(address, data),
            WAVE_START..=WAVE_END => self.apu.borrow_mut().write(address, data),
            OAM_DMA_ADDRESS => {
                let oam_dma = OamDma {
                    pending_cycles: OAM_DMA_CYCLES,
                    next_address: (data as Word) << 8,
                };

                *self.oam_dma.borrow_mut() = Some(oam_dma);
            }
            PPU_REGISTERS_START..=PPU_REGISTERS_END => self.ppu.borrow_mut().write(address, data),
            VRAM_BANK_ADDRESS => self.ppu.borrow_mut().write(address, data),
            KEY1 => self.key1 = (self.key1 & 0x80) | (data & 0x7F),
            BOOTROM_DISABLE => self.disable_bootrom(data),
            VRAM_DMA_START..=VRAM_DMA_END => {}
            PALETTE_START..=PALETTE_END => self.ppu.borrow_mut().write(address, data),
            WRAM_BANK_SELECT => self.wram_bank = data as usize & 0b111,
            HRAM_START..=HRAM_END => self.hram[address as usize - 0xFF80] = data,
            INTERRUPT_ENABLE_ADDRESS => self.interrupts.borrow_mut().write(address, data),
            _ => log::error!("Unknown address to Mmu::write {:#06X}", address),
        }
    }

    fn disable_bootrom(&mut self, data: Byte) {
        self.bootrom_enabled = data == 0x00;
        if !self.bootrom_enabled {
            log::info!("Boot ROM disabled");
        }
    }

    pub fn save_ram(&self) -> Option<&Vec<Byte>> {
        self.cart.save_ram()
    }

    pub fn speed_multiplier(&self) -> Cycles {
        if self.key1 & 0x80 != 0 {
            2
        } else {
            1
        }
    }

    fn preparing_speed_switch(&self) -> bool {
        self.key1 & 0x1 != 0
    }

    pub fn switch_speed(&mut self) {
        todo!()
    }
}

impl Memory for Mmu {
    /// Read and Write for Mmu. The `Memory` trait is not implemented for the Mmu
    /// because `read` here needs to take a mutable reference to `self` due to
    /// using `tick` inside it. We want the other components to keep up with the
    /// CPU during each memory access
    fn read(&self, address: Word) -> Byte {
        self.tick();
        if self.oam_dma_in_progress() {
            // Only HRAM is accessible during OAM DMA
            if (HRAM_START..=HRAM_END).contains(&address) {
                self.hram[address as usize - 0xFF80]
            } else {
                0xFF
            }
        } else {
            self.raw_read(address)
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        // TODO: This order might influence how TIMA updates
        self.tick();
        if self.oam_dma_in_progress() {
            // Only HRAM is accessible during OAM DMA
            if (HRAM_START..=HRAM_END).contains(&address) {
                self.hram[address as usize - 0xFF80] = data;
            }
        } else {
            self.raw_write(address, data);
        };
    }
}
