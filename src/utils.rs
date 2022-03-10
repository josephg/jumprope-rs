use crate::fast_str_tools::*;

// Get the byte offset after char_pos utf8 characters
pub(crate) fn str_chars_to_bytes(s: &str, char_pos: usize) -> usize {
    // s.char_indices().nth(char_pos).map_or_else(
    //     || s.len(),
    //     |(i, _)| i
    // )

    char_to_byte_idx(s, char_pos)
}

// pub(crate) fn str_bytes_to_chars(s: &str, bytes: usize) -> usize {
//     byte_to_char_idx(s, bytes)
// }
//
// pub(crate) fn count_chars(s: &str) -> usize {
//     str_bytes_to_chars(s, s.len())
// }

pub(crate) fn str_chars_to_bytes_rev(s: &str, char_len: usize) -> usize {
    if char_len == 0 { return 0; }

    // Scan backwards, looking for utf8 start bytes (marked by 0b0x or 0b
    let mut chars_remaining = char_len;
    for (i, byte) in s.as_bytes().iter().rev().enumerate() {
        if (*byte & 0b11_00_0000) != 0b10_00_0000 {
            chars_remaining -= 1;
            if chars_remaining == 0 { return i+1; }
        }
    }
    panic!("Insufficient characters in string");
}

// #[cfg(feature = "wchar_conversion")]
// pub(crate) fn count_wchars(s: &str) -> usize {
//     // TODO: There's a better way to write this.
//     s.chars()
//         .map(|c| c.len_utf16())
//         .sum()
// }
//
// #[cfg(feature = "wchar_conversion")]
// pub(crate) fn str_chars_to_wchars(s: &str, char_len: usize) -> usize {
//     // TODO: There's a better way to write this.
//     // TODO: Compare this with char_len + filter + count.
//     s.chars()
//         .take(char_len)
//         .map(|c| c.len_utf16())
//         .sum()
// }

#[cfg(test)]
mod tests {
    use crate::utils::*;

    fn check_counts(s: &str) {
        let num_chars = s.chars().count();
        assert_eq!(count_chars(s), num_chars);

        for i in 0..=num_chars {
            let byte_offset = str_chars_to_bytes(s, i);
            assert_eq!(count_chars(&s[..byte_offset]), i);

            let end_offset = str_chars_to_bytes_rev(s, num_chars - i);
            assert_eq!(end_offset, s.len() - byte_offset);
        }
    }

    #[test]
    fn backwards_smoke_tests() {
        check_counts("hi there");
        check_counts("κό𝕐𝕆😘σμε");
    }
}
