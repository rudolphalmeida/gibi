use crate::SystemState;

/// This trait defines all the memory-mapped components of the Game Boy
/// The CPU can pass data to and from these components using the `Memory::read`
/// and the `Memory::write` functions.
pub(crate) trait Memory {
    /// Read the data (`Byte`) at `address` and return it. `address` can be
    /// mapped to something else. This function should take exactly
    /// `1` m-cycle or `4` t-cycles in the Game Boy clock timings
    fn read(&mut self, address: u16) -> u8;

    /// Write the `data` (`Byte`) to `address`. `address` can be mapped to
    /// else. This method should take exactly `1` m-cycle or `4` t-cycles in
    /// the Game Boy clock timings.
    fn write(&mut self, address: u16, data: u8);
}

pub(crate) trait SystemBus: Memory {
    fn unticked_read(&mut self, address: u16) -> u8;
    fn unticked_write(&mut self, address: u16, data: u8);
    fn tick(&mut self);
    fn system_state(&mut self) -> &mut SystemState;
}
