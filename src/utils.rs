use once_cell::sync::Lazy;

pub(crate) type Byte = u8;
pub(crate) type Word = u16;
pub(crate) type Sbyte = i8;
/// Machine-Cycles (m)
pub(crate) type Cycles = u64;

pub(crate) const HEX_LOOKUP: Lazy<Vec<String>> = Lazy::new(|| {
    let mut lookup: Vec<String> = Vec::with_capacity(512);

    for i in 0..=0xFF {
        let repr = format!("{:#04X}", i);
        lookup.push(repr);
    }

    lookup
});

/// Create a `Word` from two `Byte`s with the first argument
/// as the most significant and the second argument as the
/// least significant
pub(crate) fn compose_word(msb: Byte, lsb: Byte) -> Word {
    Word::from(msb) << 8 | Word::from(lsb)
}

/// Extract the `Byte`s from a `Word` and return a tuple with
/// the most siginificant `Byte` as the first item and the
/// least significant `Byte` as the second
pub(crate) fn decompose_word(value: Word) -> (Byte, Byte) {
    let lsb = value as Byte;
    let msb = (value >> 8) as Byte;

    (msb, lsb)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_compose_word() {
        assert_eq!(compose_word(0x00, 0x00), 0x0000);
        assert_eq!(compose_word(0xFF, 0xFF), 0xFFFF);
        assert_eq!(compose_word(0xF0, 0x0F), 0xF00F);
    }

    #[test]
    fn test_decompose_word() {
        assert_eq!(decompose_word(0x0000), (0x00, 0x00));
        assert_eq!(decompose_word(0xFFFF), (0xFF, 0xFF));
        assert_eq!(decompose_word(0xF00F), (0xF0, 0x0F));
    }
}
