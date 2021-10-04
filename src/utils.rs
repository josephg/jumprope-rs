
// Get the byte offset after char_pos utf8 characters
pub(crate) fn str_get_byte_offset(s: &str, char_pos: usize) -> usize {
    // s.char_indices().nth(char_pos).map_or_else(
    //     || s.len(),
    //     |(i, _)| i
    // )

    ropey::str_utils::char_to_byte_idx(s, char_pos)
}

pub(crate) fn str_byte_to_char_idx(s: &str, bytes: usize) -> usize {
    ropey::str_utils::byte_to_char_idx(s, bytes)
}

pub(crate) fn count_chars(s: &str) -> usize {
    str_byte_to_char_idx(s, s.len())
}