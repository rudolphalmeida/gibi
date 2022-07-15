use crate::{
    memory::Memory,
    utils::{Byte, Word},
};

/// The MMU (Memory-Management Unit) is responsible for connecting the CPU to
/// the rest of the components. The `Mmu` implements the memory-map of the
/// GameBoy, redirecting reads and writes made by the CPU and PPU to the
/// correct component to which the address is mapped.
pub(crate) struct Mmu {}

impl Mmu {
    pub fn new() -> Self {
        todo!()
    }
}

impl Memory for Mmu {
    fn read(&self, _address: Word) -> Byte {
        todo!()
    }

    fn write(&mut self, _address: Word, _data: Byte) {
        todo!()
    }
}
