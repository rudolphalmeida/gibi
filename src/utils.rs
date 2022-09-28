pub(crate) type Byte = u8;
pub(crate) type Word = u16;
pub(crate) type Sbyte = i8;
/// Machine-Cycles (m)
pub(crate) type Cycles = u64;

/// Create a `Word` from two `Byte`s with the first argument
/// as the most significant and the second argument as the
/// least significant
pub(crate) fn compose_word(msb: Byte, lsb: Byte) -> Word {
    Word::from(msb) << 8 | Word::from(lsb)
}

/// Extract the `Byte`s from a `Word` and return a tuple with
/// the most significant `Byte` as the first item and the
/// least significant `Byte` as the second
pub(crate) fn decompose_word(value: Word) -> (Byte, Byte) {
    let lsb = value as Byte;
    let msb = (value >> 8) as Byte;

    (msb, lsb)
}

pub(crate) fn bit_value(value: Byte, index: Byte) -> Byte {
    if value & (1 << index) != 0 {
        0x1
    } else {
        0x0
    }
}

/// Calculate the minimum number of bits required to store a value
pub(crate) fn min_number_of_bits(mut value: Byte) -> Byte {
    let mut count = 0;
    while value > 0 {
        count += 1;
        value >>= 1;
    }

    count
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

    #[test]
    fn test_min_number_of_bits() {
        assert_eq!(min_number_of_bits(4), 3);
        assert_eq!(min_number_of_bits(5), 3);
        assert_eq!(min_number_of_bits(8), 4);
        assert_eq!(min_number_of_bits(16), 5);
        assert_eq!(min_number_of_bits(32), 6);
        assert_eq!(min_number_of_bits(64), 7);
        assert_eq!(min_number_of_bits(128), 8);
    }
}
