use crate::{
    apu::{Apu, SOUND_END, SOUND_START, WAVE_END, WAVE_START},
    cartridge::{
        Cartridge, BOOT_ROM_END, BOOT_ROM_START, CART_RAM_END, CART_RAM_START, CART_ROM_END,
        CART_ROM_START, CGB_BOOT_ROM,
    },
    interrupts::{InterruptHandler, INTERRUPT_ENABLE_ADDRESS, INTERRUPT_FLAG_ADDRESS},
    joypad::{Joypad, JoypadKeys, JOYP_ADDRESS},
    memory::{Memory, SystemBus},
    ppu::{
        Ppu, OAM_DMA_CYCLES, OAM_END, OAM_START, PALETTE_END, PALETTE_START, VRAM_BANK_ADDRESS,
        VRAM_END, VRAM_START,
    },
    serial::{Serial, SERIAL_END, SERIAL_START},
    timer::{Timer, TIMER_END, TIMER_START},
    ExecutionState, HardwareSupport, HdmaState, SystemState,
};

const WRAM_BANK_SIZE: usize = 1024 * 4; // 4KB
const HRAM_SIZE: usize = 0xFFFE - 0xFF80 + 1;

/// VRAM DMA Source High
const HDMA1: u16 = 0xFF51;
/// VRAM DMA Source Low
const HDMA2: u16 = 0xFF52;
/// VRAM DMA Dest High
const HDMA3: u16 = 0xFF53;
/// VRAM DMA Dest Low
const HDMA4: u16 = 0xFF54;
/// VRAM DMA Length/Mode/Start
const HDMA5: u16 = 0xFF55;
const VRAM_DMA_END: u16 = 0xFF55;

const WRAM_BANK_SELECT: u16 = 0xFF70;

const HRAM_START: u16 = 0xFF80;
const HRAM_END: u16 = 0xFFFE;

struct OamDma {
    pending_cycles: u64,
    next_address: u16,
}

/// The MMU (Memory-Management Unit) is responsible for connecting the CPU to
/// the rest of the components. The `Mmu` implements the memory-map of the
/// Game Boy, redirecting reads and writes made by the CPU and PPU to the
/// correct component to which the address is mapped.
pub(crate) struct Mmu {
    pub(crate) cart: Cartridge,
    pub(crate) ppu: Ppu,
    apu: Apu,
    joypad: Joypad,
    timer: Timer,
    serial: Serial,

    pub(crate) interrupts: InterruptHandler,
    pub(crate) system_state: SystemState,

    // Only two banks are used in DMG mode
    // CGB mode uses all 8, with 0 being fixed, and 1-7 being switchable
    wram: [u8; WRAM_BANK_SIZE * 8],
    wram_bank: usize,

    hram: [u8; HRAM_SIZE],

    // DMAs
    oam_dma: Option<OamDma>,
}

impl Mmu {
    pub fn new(cart: Cartridge) -> Self {
        let wram = [0x00; WRAM_BANK_SIZE * 8]; // 32KB
        let wram_bank = 0x1;

        let hram = [0x00; HRAM_SIZE];
        let serial = Serial::new();

        let interrupts = InterruptHandler::default();

        let timer = Timer::new();
        let ppu = Ppu::new();
        let apu = Apu::new();
        let joypad = Joypad::new();

        let system_state = SystemState {
            execution_state: ExecutionState::ExecutingProgram,
            hardware_support: cart.header.hardware_supported,
            carry_over_cycles: 0,
            total_cycles: 0,
            key1: 0x00,
            bootrom_mapped: true,
            hdma_state: HdmaState {
                source_addr: 0xFFFF,
                dest_addr: 0xFFFF,
                hdma_stat: 0x00,
            },
        };

        Self {
            cart,
            system_state,
            wram,
            wram_bank,
            hram,
            ppu,
            joypad,
            serial,
            timer,
            apu,
            interrupts,
            oam_dma: None,
        }
    }

    fn tick_oam_dma(&mut self) {
        // Perform DMA
        let mut oam_dma_completed = false;
        if self.oam_dma.is_some() {
            let (next_address, pending_cycles) = self
                .oam_dma
                .as_ref()
                .map(|oam_dma| (oam_dma.next_address, oam_dma.pending_cycles))
                .unwrap();
            let dest_address = 0xFE00 | (next_address & 0x00FF);

            let data = self.unticked_read(next_address);
            self.ppu.write(dest_address, data);

            self.oam_dma.as_mut().unwrap().next_address += 1;
            match pending_cycles.checked_sub(1) {
                None => oam_dma_completed = true,
                Some(x) => self.oam_dma.as_mut().unwrap().pending_cycles = x,
            }
        }

        if oam_dma_completed {
            self.oam_dma = None;
        }
    }

    fn oam_dma_in_progress(&self) -> bool {
        self.oam_dma.is_some()
    }

    fn wram_banked_read(&self, address: u16) -> u8 {
        // FIXME: Index out of range errors
        let index = WRAM_BANK_SIZE
            * if self.wram_bank == 0x00 {
                1
            } else {
                self.wram_bank
            }
            + (address - 0xD000) as usize;

        self.wram[index]
    }

    fn wram_banked_write(&mut self, address: u16, data: u8) {
        // FIXME: Index out of range errors
        let index = WRAM_BANK_SIZE
            * if self.wram_bank == 0x00 {
                1
            } else {
                self.wram_bank
            }
            + (address - 0xD000) as usize;

        self.wram[index] = data
    }

    fn on_hdma5_write(&mut self, data: u8) {
        if (data & 0x80) == 0 {
            // GDMA
            if self.system_state.hdma_state.is_hdma_active() {
                // If HDMA is active, cancel it keeping the remaining length
                self.system_state.hdma_state.hdma_stat |= 0x80;
            } else {
                let len = ((data as usize & 0x7F) + 1) * 0x10;
                let mut src_addr = self.system_state.hdma_state.source_addr & 0xFFF0;
                let mut dest_addr = (self.system_state.hdma_state.dest_addr & 0x1FF0) | 0x8000;

                for _ in 0..len {
                    let value = self.unticked_read(src_addr);
                    self.unticked_write(dest_addr, value);
                    src_addr += 1;
                    dest_addr += 1;
                }

                self.system_state.hdma_state.hdma_stat = 0xFF;
            }
        } else {
            // HDMA
            log::info!("TODO: Setup HDMA");
        }
    }

    pub fn keydown(&mut self, key: JoypadKeys) {
        self.joypad.keydown(key);
    }

    pub fn keyup(&mut self, key: JoypadKeys) {
        self.joypad.keyup(key);
    }

    fn disable_bootrom(&mut self, data: u8) {
        self.system_state.bootrom_mapped = data == 0x00;
        if !self.system_state.bootrom_mapped {
            log::info!("Boot ROM disabled");
        }
    }

    pub fn save_ram(&self) -> Option<&Vec<u8>> {
        self.cart.save_ram()
    }
}

impl Memory for Mmu {
    /// Read and Write for Mmu. The `Memory` trait is not implemented for the Mmu
    /// because `read` here needs to take a mutable reference to `self` due to
    /// using `tick` inside it. We want the other components to keep up with the
    /// CPU during each memory access
    fn read(&mut self, address: u16) -> u8 {
        self.tick();
        if self.oam_dma_in_progress() {
            // Only HRAM is accessible during OAM DMA
            if (HRAM_START..=HRAM_END).contains(&address) {
                self.hram[address as usize - 0xFF80]
            } else {
                0xFF
            }
        } else {
            self.unticked_read(address)
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        self.tick();
        if self.oam_dma_in_progress() {
            // Only HRAM is accessible during OAM DMA
            if (HRAM_START..=HRAM_END).contains(&address) {
                self.hram[address as usize - 0xFF80] = data;
            }
        } else {
            self.unticked_write(address, data);
        };
    }
}

impl SystemBus for Mmu {
    /// Raw Read: Read the contents of a memory location without ticking all the
    /// components
    fn unticked_read(&mut self, address: u16) -> u8 {
        match address {
            0x100..=0x1FF => return self.cart.read(address),
            BOOT_ROM_START..=BOOT_ROM_END if self.system_state.bootrom_mapped => {
                return CGB_BOOT_ROM[address as usize]
            }
            CART_ROM_START..=CART_ROM_END => return self.cart.read(address),
            VRAM_START..=VRAM_END => return self.ppu.read(address),
            CART_RAM_START..=CART_RAM_END => return self.cart.read(address),
            0xC000..=0xCFFF => return self.wram[(address - 0xC000) as usize],
            // Switchable bank for WRAM
            0xD000..=0xDFFF => return self.wram_banked_read(address),
            0xE000..=0xFDFF => return self.unticked_read(address - 0xE000),
            OAM_START..=OAM_END => return self.ppu.read(address),
            0xFEA0..=0xFEFF => {}
            JOYP_ADDRESS => return self.joypad.read(address),
            SERIAL_START..=SERIAL_END => return self.serial.read(address),
            TIMER_START..=TIMER_END => return self.timer.read(address),
            INTERRUPT_FLAG_ADDRESS => return self.interrupts.read(address),
            SOUND_START..=SOUND_END => return self.apu.read(address),
            WAVE_START..=WAVE_END => return self.apu.read(address),
            0xFF46 => return 0xFF, // TODO: Check if this is correct
            0xFF40..=0xFF4B => return self.ppu.read(address),
            VRAM_BANK_ADDRESS => return self.ppu.read(address),
            0xFF4D => return self.system_state.key1,
            0xFF50 => return u8::from(self.system_state.bootrom_mapped),
            HDMA1..=HDMA4 => return 0xFF,
            HDMA5 => return self.system_state.hdma_state.hdma_stat,
            PALETTE_START..=PALETTE_END => return self.ppu.read(address),
            WRAM_BANK_SELECT => return self.wram_bank as u8,
            HRAM_START..=HRAM_END => return self.hram[(address - HRAM_START) as usize],
            INTERRUPT_ENABLE_ADDRESS => return self.interrupts.read(address),
            _ => log::error!("Unknown address to Mmu::read {:#06X}", address),
        }
        0xFF
    }

    fn unticked_write(&mut self, address: u16, data: u8) {
        match address {
            0x100..=0x1FF => self.cart.write(address, data),
            BOOT_ROM_START..=BOOT_ROM_END if self.system_state.bootrom_mapped => {
                log::error!("Write to boot ROM {:#06X} with {:#04X}", address, data)
            }
            CART_ROM_START..=CART_ROM_END => self.cart.write(address, data),
            VRAM_START..=VRAM_END => self.ppu.write(address, data),
            CART_RAM_START..=CART_RAM_END => self.cart.write(address, data),
            0xC000..=0xCFFF => self.wram[(address - 0xC000) as usize] = data,
            0xD000..=0xDFFF => self.wram_banked_write(address, data),
            0xE000..=0xFDFF => self.unticked_write(address - 0xE000, data),
            OAM_START..=OAM_END => self.ppu.write(address, data),
            0xFEA0..=0xFEFF => {}
            JOYP_ADDRESS => self.joypad.write(address, data),
            SERIAL_START..=SERIAL_END => self.serial.write(address, data),
            TIMER_START..=TIMER_END => self.timer.write(address, data),
            INTERRUPT_FLAG_ADDRESS => self.interrupts.write(address, data),
            SOUND_START..=SOUND_END => self.apu.write(address, data),
            WAVE_START..=WAVE_END => self.apu.write(address, data),
            0xFF46 => {
                let oam_dma = OamDma {
                    pending_cycles: OAM_DMA_CYCLES,
                    next_address: (data as u16) << 8,
                };

                self.oam_dma = Some(oam_dma);
            }
            0xFF40..=0xFF4B => self.ppu.write(address, data),
            VRAM_BANK_ADDRESS => self.ppu.write(address, data),
            0xFF4D => {
                let key1 = (self.system_state.key1 & 0x80) | (data & 0x7F);
                self.system_state.key1 = key1;
            }
            0xFF50 => self.disable_bootrom(data),
            HDMA1 => self.system_state.hdma_state.write_src_high(data),
            HDMA2 => self.system_state.hdma_state.write_src_low(data),
            HDMA3 => self.system_state.hdma_state.write_dest_high(data),
            HDMA4 => self.system_state.hdma_state.write_dest_low(data),
            HDMA5 if self.system_state.hardware_support != HardwareSupport::DmgCompat => {
                self.on_hdma5_write(data)
            }
            HDMA5 => {}
            PALETTE_START..=PALETTE_END => self.ppu.write(address, data),
            WRAM_BANK_SELECT => self.wram_bank = data as usize & 0b111,
            HRAM_START..=HRAM_END => self.hram[address as usize - 0xFF80] = data,
            INTERRUPT_ENABLE_ADDRESS => self.interrupts.write(address, data),
            _ => log::error!("Unknown address to Mmu::write {:#06X}", address),
        }
    }

    fn tick(&mut self) {
        self.system_state.total_cycles += 1;
        self.tick_oam_dma();

        self.timer
            .tick(&mut self.system_state, &mut self.interrupts);
        self.joypad.tick(&mut self.interrupts);
        self.ppu.tick(&mut self.system_state, &mut self.interrupts);
        self.apu.tick(&mut self.system_state, &mut self.interrupts);
    }

    fn system_state(&mut self) -> &mut SystemState {
        &mut self.system_state
    }

    fn interrupt_handler(&mut self) -> &mut InterruptHandler {
        &mut self.interrupts
    }
}
