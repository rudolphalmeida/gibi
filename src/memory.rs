use crate::utils::{Byte, Word};

/// This trait defines all the memory-mapped components of the GameBoy
/// The CPU can pass data to and from these components using the `Memory::read`
/// and the `Memory::write` functions.
pub(crate) trait Memory {
    /// Read the data (`Byte`) at `address` and return it. `address` can be
    /// mapped to something else. This function should take exactly
    /// `1` m-cycle or `4` t-cycles in the GameBoy clock timings
    fn read(&self, address: Word) -> Byte;

    /// Write the `data` (`Byte`) to `address`. `address` can be mapped to
    /// else. This method should take exactly `1` m-cycle or `4` t-cycles in
    /// the GameBoy clock timings.
    fn write(&mut self, address: Word, data: Byte);
}
