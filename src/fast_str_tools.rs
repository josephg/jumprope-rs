//! Utility functions for utf8 string slices.
//!
//! This file mostly defers to str_indicies but overrides some methods because the compiler is
//! smart.

/// Converts from byte-index to char-index in a string slice.
///
/// If the byte is in the middle of a multi-byte char, returns the index of
/// the char that the byte belongs to.
///
/// Any past-the-end index will return the one-past-the-end char index.
///
/// Runs in O(N) time.
#[inline]
#[allow(unused)]
pub fn byte_to_char_idx(text: &str, byte_idx: usize) -> usize {
    let count = count_chars_in_bytes(&text.as_bytes()[0..(byte_idx + 1).min(text.len())]);
    if byte_idx < text.len() {
        count - 1
    } else {
        count
    }
}

/// Converts from char-index to byte-index in a string slice.
///
/// Any past-the-end index will return the one-past-the-end byte index.
///
/// Runs in O(N) time.
#[inline]
pub fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    if cfg!(not(miri)) {
        str_indices::chars::to_byte_idx(text, char_idx)
    } else {
        // Naive version.
        let mut byte_count = 0;
        let mut char_count = 0;

        let mut i = 0;
        let text = text.as_bytes();
        while i < text.len() && char_count <= char_idx {
            char_count += ((text[i] & 0xC0) != 0x80) as usize;
            i += 1;
        }
        byte_count += i;

        if byte_count == text.len() && char_count <= char_idx {
            byte_count
        } else {
            byte_count - 1
        }
    }
}

// #[allow(unused)]
// #[inline(always)]
// fn char_to_byte_idx_naive(text: &[u8], char_idx: usize) -> usize {
//     let mut byte_count = 0;
//     let mut char_count = 0;
//
//     let mut i = 0;
//     while i < text.len() && char_count <= char_idx {
//         char_count += ((text[i] & 0xC0) != 0x80) as usize;
//         i += 1;
//     }
//     byte_count += i;
//
//     if byte_count == text.len() && char_count <= char_idx {
//         byte_count
//     } else {
//         byte_count - 1
//     }
// }

/// Counts the utf16 surrogate pairs that would be in `text` if it were encoded
/// as utf16.
#[inline]
pub(crate) fn count_utf16_surrogates(text: &str) -> usize {
    unsafe { count_utf16_surrogates_in_bytes(text.as_bytes()) }
}

/// SAFETY: Passed text array must be a valid UTF8 string. This will not be checked at runtime.
#[inline]
pub(crate) unsafe fn count_utf16_surrogates_in_bytes(text: &[u8]) -> usize {
    if cfg!(miri) {
        // Naive version
        let mut utf16_surrogate_count = 0;

        for byte in text.iter() {
            utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
        }

        utf16_surrogate_count
    } else {
        str_indices::utf16::count_surrogates(std::str::from_utf8_unchecked(text))
    }
}

// This is an alternate naive method which may make sense later.
// #[inline]
// #[allow(unused)]
// pub(crate) fn count_utf16_surrogates_in_bytes_naive(text: &[u8]) -> usize {
//     let mut utf16_surrogate_count = 0;
//
//     for byte in text.iter() {
//         utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
//     }
//
//     utf16_surrogate_count
// }

#[inline(always)]
#[allow(unused)]
pub(crate) fn byte_to_utf16_surrogate_idx(text: &str, byte_idx: usize) -> usize {
    count_utf16_surrogates(&text[..byte_idx])
}

#[inline(always)]
#[allow(unused)]
pub(crate) fn utf16_code_unit_to_char_idx(text: &str, utf16_idx: usize) -> usize {
    // TODO: optimized version.  This is pretty slow.  It isn't expected to be
    // used in performance critical functionality, so this isn't urgent.  But
    // might as well make it faster when we get the chance.
    let mut char_i = 0;
    let mut utf16_i = 0;
    for c in text.chars() {
        if utf16_idx <= utf16_i {
            break;
        }
        char_i += 1;
        utf16_i += c.len_utf16();
    }

    if utf16_idx < utf16_i {
        char_i -= 1;
    }

    char_i
}

//===========================================================================
// Internal
//===========================================================================

/// Uses bit-fiddling magic to count utf8 chars really quickly.
/// We actually count the number of non-starting utf8 bytes, since
/// they have a consistent starting two-bit pattern.  We then
/// subtract from the byte length of the text to get the final
/// count.
#[inline]
#[allow(unused)]
pub(crate) fn count_chars(text: &str) -> usize {
    count_chars_in_bytes(text.as_bytes())
}

#[inline]
pub(crate) fn count_chars_in_bytes(text: &[u8]) -> usize {
    if text.len() <= 1 { text.len() }
    else if !cfg!(miri) {
        unsafe { str_indices::chars::count(std::str::from_utf8_unchecked(text)) }
    } else {
        let mut inv_count = 0;
        for byte in text.iter() {
            inv_count += ((byte & 0xC0) != 0x80) as usize;
        }
        inv_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    #[test]
    fn count_chars_01() {
        let text = "Hello せかい! Hello せかい! Hello せかい! Hello せかい! Hello せかい!";

        assert_eq!(54, count_chars(text));
    }

    #[test]
    fn count_chars_02() {
        assert_eq!(100, count_chars(TEXT_LINES));
    }

    #[test]
    fn byte_to_char_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(6, byte_to_char_idx(text, 6));
        assert_eq!(6, byte_to_char_idx(text, 7));
        assert_eq!(6, byte_to_char_idx(text, 8));
        assert_eq!(7, byte_to_char_idx(text, 9));
        assert_eq!(7, byte_to_char_idx(text, 10));
        assert_eq!(7, byte_to_char_idx(text, 11));
        assert_eq!(8, byte_to_char_idx(text, 12));
        assert_eq!(8, byte_to_char_idx(text, 13));
        assert_eq!(8, byte_to_char_idx(text, 14));
        assert_eq!(9, byte_to_char_idx(text, 15));
        assert_eq!(10, byte_to_char_idx(text, 16));
        assert_eq!(10, byte_to_char_idx(text, 17));
        assert_eq!(10, byte_to_char_idx(text, 18));
        assert_eq!(10, byte_to_char_idx(text, 19));
    }

    #[test]
    fn byte_to_char_idx_02() {
        let text = "";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(0, byte_to_char_idx(text, 1));

        let text = "h";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(1, byte_to_char_idx(text, 2));

        let text = "hi";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(1, byte_to_char_idx(text, 1));
        assert_eq!(2, byte_to_char_idx(text, 2));
        assert_eq!(2, byte_to_char_idx(text, 3));
    }

    #[test]
    fn byte_to_char_idx_03() {
        let text = "せかい";
        assert_eq!(0, byte_to_char_idx(text, 0));
        assert_eq!(0, byte_to_char_idx(text, 1));
        assert_eq!(0, byte_to_char_idx(text, 2));
        assert_eq!(1, byte_to_char_idx(text, 3));
        assert_eq!(1, byte_to_char_idx(text, 4));
        assert_eq!(1, byte_to_char_idx(text, 5));
        assert_eq!(2, byte_to_char_idx(text, 6));
        assert_eq!(2, byte_to_char_idx(text, 7));
        assert_eq!(2, byte_to_char_idx(text, 8));
        assert_eq!(3, byte_to_char_idx(text, 9));
        assert_eq!(3, byte_to_char_idx(text, 10));
        assert_eq!(3, byte_to_char_idx(text, 11));
        assert_eq!(3, byte_to_char_idx(text, 12));
    }

    #[test]
    fn byte_to_char_idx_04() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, byte_to_char_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..125 {
            assert_eq!(88 + ((i - 88) / 3), byte_to_char_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(100, byte_to_char_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn char_to_byte_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(1, char_to_byte_idx(text, 1));
        assert_eq!(2, char_to_byte_idx(text, 2));
        assert_eq!(5, char_to_byte_idx(text, 5));
        assert_eq!(6, char_to_byte_idx(text, 6));
        assert_eq!(12, char_to_byte_idx(text, 8));
        assert_eq!(15, char_to_byte_idx(text, 9));
        assert_eq!(16, char_to_byte_idx(text, 10));
    }

    #[test]
    fn char_to_byte_idx_02() {
        let text = "せかい";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(3, char_to_byte_idx(text, 1));
        assert_eq!(6, char_to_byte_idx(text, 2));
        assert_eq!(9, char_to_byte_idx(text, 3));
    }

    #[test]
    fn char_to_byte_idx_03() {
        let text = "Hello world!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(1, char_to_byte_idx(text, 1));
        assert_eq!(8, char_to_byte_idx(text, 8));
        assert_eq!(11, char_to_byte_idx(text, 11));
        assert_eq!(12, char_to_byte_idx(text, 12));
    }

    #[test]
    fn char_to_byte_idx_04() {
        let text = "Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい!";
        assert_eq!(0, char_to_byte_idx(text, 0));
        assert_eq!(30, char_to_byte_idx(text, 24));
        assert_eq!(60, char_to_byte_idx(text, 48));
        assert_eq!(90, char_to_byte_idx(text, 72));
        assert_eq!(115, char_to_byte_idx(text, 93));
        assert_eq!(120, char_to_byte_idx(text, 96));
        assert_eq!(150, char_to_byte_idx(text, 120));
        assert_eq!(180, char_to_byte_idx(text, 144));
        assert_eq!(210, char_to_byte_idx(text, 168));
        assert_eq!(239, char_to_byte_idx(text, 191));
    }

    #[test]
    fn char_to_byte_idx_05() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, char_to_byte_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..100 {
            assert_eq!(88 + ((i - 88) * 3), char_to_byte_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(124, char_to_byte_idx(TEXT_LINES, i));
        }
    }
}
