//! Utility functions for utf8 string slices.
//!
//! This code is adapted from [Ropey](https://github.com/cessen/ropey).
//!
//! This module provides various utility functions that operate on string
//! slices in ways compatible with Ropey.  They may be useful when building
//! additional functionality on top of Ropey.

/*
This code was provided under the following license:

Copyright (c) 2017 Nathan Vegdahl

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

 */

// Get the appropriate module (if any) for sse2 types and intrinsics for the
// platform we're compiling for.
#[cfg(target_arch = "x86")]
use std::arch::x86 as sse2;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as sse2;

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
    #[cfg(not(miri))]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
        if is_x86_feature_detected!("sse2") {
            return char_to_byte_idx_inner::<sse2::__m128i>(text, char_idx);
        }
    }

    // Fallback for non-sse2 platforms.
    char_to_byte_idx_inner::<usize>(text, char_idx)
    // char_to_byte_idx_naive(text.as_bytes(), char_idx)
}

#[allow(unused)]
#[inline(always)]
fn char_to_byte_idx_naive(text: &[u8], char_idx: usize) -> usize {
    let mut byte_count = 0;
    let mut char_count = 0;

    let mut i = 0;
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

#[inline(always)]
fn char_to_byte_idx_inner<T: ByteChunk>(text: &str, char_idx: usize) -> usize {
    // Get `middle` so we can do more efficient chunk-based counting.
    // We can't use this to get `end`, however, because the start index of
    // `end` actually depends on the accumulating char counts during the
    // counting process.
    let (start, middle, _) = unsafe { text.as_bytes().align_to::<T>() };

    let mut byte_count = 0;
    let mut char_count = 0;

    // Take care of any unaligned bytes at the beginning.
    let mut i = 0;
    while i < start.len() && char_count <= char_idx {
        char_count += ((start[i] & 0xC0) != 0x80) as usize;
        i += 1;
    }
    byte_count += i;

    // Use chunks to count multiple bytes at once, using bit-fiddling magic.
    let mut i = 0;
    let mut acc = T::splat(0);
    let mut acc_i = 0;
    while i < middle.len() && (char_count + (T::size() * (acc_i + 1))) <= char_idx {
        acc = acc.add(middle[i].bitand(T::splat(0xc0)).cmp_eq_byte(0x80));
        acc_i += 1;
        if acc_i == T::max_acc() || (char_count + (T::size() * (acc_i + 1))) >= char_idx {
            char_count += (T::size() * acc_i) - acc.sum_bytes();
            acc_i = 0;
            acc = T::splat(0);
        }
        i += 1;
    }
    char_count += (T::size() * acc_i) - acc.sum_bytes();
    byte_count += i * T::size();

    // Take care of any unaligned bytes at the end.
    let end = &text.as_bytes()[byte_count..];
    let mut i = 0;
    while i < end.len() && char_count <= char_idx {
        char_count += ((end[i] & 0xC0) != 0x80) as usize;
        i += 1;
    }
    byte_count += i;

    // Finish up
    if byte_count == text.len() && char_count <= char_idx {
        byte_count
    } else {
        byte_count - 1
    }
}

/// Counts the utf16 surrogate pairs that would be in `text` if it were encoded
/// as utf16.
#[inline]
pub(crate) fn count_utf16_surrogates(text: &str) -> usize {
    count_utf16_surrogates_in_bytes(text.as_bytes())
}

#[inline]
pub(crate) fn count_utf16_surrogates_in_bytes(text: &[u8]) -> usize {
    // This is smaller and faster than the simd version in my tests.
    let mut utf16_surrogate_count = 0;

    for byte in text.iter() {
        utf16_surrogate_count += ((byte & 0xf0) == 0xf0) as usize;
    }

    utf16_surrogate_count
}

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
    // Smaller and faster than the simd version.
    let mut inv_count = 0;
    for byte in text.iter() {
        inv_count += ((byte & 0xC0) != 0x80) as usize;
    }
    inv_count
}

// /// Returns the alignment difference between the start of `bytes` and the
// /// type `T`.
// ///
// /// Or put differently: returns how many bytes into `bytes` you need to walk
// /// to reach the alignment of `T` in memory.
// ///
// /// Will return 0 if already aligned at the start, and will return the length
// /// of `bytes` if alignment is beyond the end of `bytes`.
// #[inline(always)]
// fn alignment_diff<T>(bytes: &[u8]) -> usize {
//     let alignment = std::mem::align_of::<T>();
//     let ptr = bytes.as_ptr() as usize;
//     (alignment - ((ptr - 1) & (alignment - 1)) - 1).min(bytes.len())
// }

//======================================================================

/// Interface for working with chunks of bytes at a time, providing the
/// operations needed for the functionality in str_utils.
trait ByteChunk: Copy + Clone + std::fmt::Debug {
    /// Returns the size of the chunk in bytes.
    fn size() -> usize;

    /// Returns the maximum number of iterations the chunk can accumulate
    /// before sum_bytes() becomes inaccurate.
    fn max_acc() -> usize;

    /// Creates a new chunk with all bytes set to n.
    fn splat(n: u8) -> Self;

    /// Returns whether all bytes are zero or not.
    fn is_zero(&self) -> bool;

    /// Shifts bytes back lexographically by n bytes.
    fn shift_back_lex(&self, n: usize) -> Self;

    /// Shifts bits to the right by n bits.
    fn shr(&self, n: usize) -> Self;

    /// Compares bytes for equality with the given byte.
    ///
    /// Bytes that are equal are set to 1, bytes that are not
    /// are set to 0.
    fn cmp_eq_byte(&self, byte: u8) -> Self;

    /// Compares bytes to see if they're in the non-inclusive range (a, b),
    /// where a < b <= 127.
    ///
    /// Bytes in the range are set to 1, bytes not in the range are set to 0.
    fn bytes_between_127(&self, a: u8, b: u8) -> Self;

    /// Performs a bitwise and on two chunks.
    fn bitand(&self, other: Self) -> Self;

    /// Adds the bytes of two chunks together.
    fn add(&self, other: Self) -> Self;

    /// Subtracts other's bytes from this chunk.
    fn sub(&self, other: Self) -> Self;

    /// Increments the nth-from-last lexographic byte by 1.
    fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self;

    /// Decrements the last lexographic byte by 1.
    fn dec_last_lex_byte(&self) -> Self;

    /// Returns the sum of all bytes in the chunk.
    fn sum_bytes(&self) -> usize;

    fn count_bits(&self) -> u32;
}

impl ByteChunk for usize {
    #[inline(always)]
    fn size() -> usize {
        std::mem::size_of::<usize>()
    }

    #[inline(always)]
    fn max_acc() -> usize {
        (256 / std::mem::size_of::<usize>()) - 1
    }

    #[inline(always)]
    fn splat(n: u8) -> Self {
        const ONES: usize = std::usize::MAX / 0xFF;
        ONES * n as usize
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        *self == 0
    }

    #[inline(always)]
    fn shift_back_lex(&self, n: usize) -> Self {
        if cfg!(target_endian = "little") {
            *self >> (n * 8)
        } else {
            *self << (n * 8)
        }
    }

    #[inline(always)]
    fn shr(&self, n: usize) -> Self {
        *self >> n
    }

    #[inline(always)]
    fn cmp_eq_byte(&self, byte: u8) -> Self {
        const ONES: usize = std::usize::MAX / 0xFF;
        const ONES_HIGH: usize = ONES << 7;
        let word = *self ^ (byte as usize * ONES);
        (!(((word & !ONES_HIGH) + !ONES_HIGH) | word) & ONES_HIGH) >> 7
    }

    #[inline(always)]
    fn bytes_between_127(&self, a: u8, b: u8) -> Self {
        const ONES: usize = std::usize::MAX / 0xFF;
        const ONES_HIGH: usize = ONES << 7;
        let tmp = *self & (ONES * 127);
        (((ONES * (127 + b as usize) - tmp) & !*self & (tmp + (ONES * (127 - a as usize))))
            & ONES_HIGH)
            >> 7
    }

    #[inline(always)]
    fn bitand(&self, other: Self) -> Self {
        *self & other
    }

    #[inline(always)]
    fn add(&self, other: Self) -> Self {
        *self + other
    }

    #[inline(always)]
    fn sub(&self, other: Self) -> Self {
        *self - other
    }

    #[inline(always)]
    fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self {
        if cfg!(target_endian = "little") {
            *self + (1 << ((Self::size() - 1 - n) * 8))
        } else {
            *self + (1 << (n * 8))
        }
    }

    #[inline(always)]
    fn dec_last_lex_byte(&self) -> Self {
        if cfg!(target_endian = "little") {
            *self - (1 << ((Self::size() - 1) * 8))
        } else {
            *self - 1
        }
    }

    #[inline(always)]
    fn sum_bytes(&self) -> usize {
        const ONES: usize = std::usize::MAX / 0xFF;
        self.wrapping_mul(ONES) >> ((Self::size() - 1) * 8)
    }

    fn count_bits(&self) -> u32 {
        self.count_ones()
    }
}

#[cfg(not(miri))]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl ByteChunk for sse2::__m128i {
    #[inline(always)]
    fn size() -> usize {
        std::mem::size_of::<sse2::__m128i>()
    }

    #[inline(always)]
    fn max_acc() -> usize {
        255
    }
    // #[inline(always)]
    // fn max_acc() -> usize {
    //     (256 / 8) - 1
    // }

    #[inline(always)]
    fn splat(n: u8) -> Self {
        unsafe { sse2::_mm_set1_epi8(n as i8) }
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        let tmp = unsafe { std::mem::transmute::<Self, (u64, u64)>(*self) };
        tmp.0 == 0 && tmp.1 == 0
    }

    #[inline(always)]
    fn shift_back_lex(&self, n: usize) -> Self {
        match n {
            0 => *self,
            1 => unsafe { sse2::_mm_srli_si128::<1>(*self) },
            2 => unsafe { sse2::_mm_srli_si128::<2>(*self) },
            3 => unsafe { sse2::_mm_srli_si128::<3>(*self) },
            4 => unsafe { sse2::_mm_srli_si128::<4>(*self) },
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn shr(&self, n: usize) -> Self {
        match n {
            0 => *self,
            1 => unsafe { sse2::_mm_srli_epi64::<1>(*self) },
            2 => unsafe { sse2::_mm_srli_epi64::<2>(*self) },
            3 => unsafe { sse2::_mm_srli_epi64::<3>(*self) },
            4 => unsafe { sse2::_mm_srli_epi64::<4>(*self) },
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn cmp_eq_byte(&self, byte: u8) -> Self {
        let tmp = unsafe { sse2::_mm_cmpeq_epi8(*self, Self::splat(byte)) };
        unsafe { sse2::_mm_and_si128(tmp, Self::splat(1)) }
    }

    #[inline(always)]
    fn bytes_between_127(&self, a: u8, b: u8) -> Self {
        let tmp1 = unsafe { sse2::_mm_cmpgt_epi8(*self, Self::splat(a)) };
        let tmp2 = unsafe { sse2::_mm_cmplt_epi8(*self, Self::splat(b)) };
        let tmp3 = unsafe { sse2::_mm_and_si128(tmp1, tmp2) };
        unsafe { sse2::_mm_and_si128(tmp3, Self::splat(1)) }
    }

    #[inline(always)]
    fn bitand(&self, other: Self) -> Self {
        unsafe { sse2::_mm_and_si128(*self, other) }
    }

    #[inline(always)]
    fn add(&self, other: Self) -> Self {
        unsafe { sse2::_mm_add_epi8(*self, other) }
    }

    #[inline(always)]
    fn sub(&self, other: Self) -> Self {
        unsafe { sse2::_mm_sub_epi8(*self, other) }
    }

    #[inline(always)]
    fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self {
        let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 16]>(*self) };
        tmp[15 - n] += 1;
        unsafe { std::mem::transmute::<[u8; 16], Self>(tmp) }
    }

    #[inline(always)]
    fn dec_last_lex_byte(&self) -> Self {
        let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 16]>(*self) };
        tmp[15] -= 1;
        unsafe { std::mem::transmute::<[u8; 16], Self>(tmp) }
    }

    #[inline(always)]
    fn sum_bytes(&self) -> usize {
        // const ONES: u64 = std::u64::MAX / 0xFF;
        // let tmp = unsafe { std::mem::transmute::<Self, (u64, u64)>(*self) };
        // let a = tmp.0.wrapping_mul(ONES) >> (7 * 8);
        // let b = tmp.1.wrapping_mul(ONES) >> (7 * 8);
        // (a + b) as usize
        unsafe {
            let zero = sse2::_mm_setzero_si128();
            let diff = sse2::_mm_sad_epu8(*self, zero);
            let (low, high) = std::mem::transmute::<Self, (u64, u64)>(diff);
            (low + high) as usize
        }
    }

    #[inline(always)]
    fn count_bits(&self) -> u32 {
        // sse2::_mm_popcnt_epi64(self)
        // sse2::_mm_popcnt_epi8(self)
        let (low, high) = unsafe { std::mem::transmute::<Self, (u64, u64)>(*self) };
        low.count_ones() + high.count_ones()
    }
}

// AVX2, currently unused because it actually runs slower than SSE2 for most
// of the things we're doing, oddly.
// impl ByteChunk for x86_64::__m256i {
//     #[inline(always)]
//     fn size() -> usize {
//         std::mem::size_of::<x86_64::__m256i>()
//     }

//     #[inline(always)]
//     fn max_acc() -> usize {
//         (256 / 8) - 1
//     }

//     #[inline(always)]
//     fn splat(n: u8) -> Self {
//         unsafe { x86_64::_mm256_set1_epi8(n as i8) }
//     }

//     #[inline(always)]
//     fn is_zero(&self) -> bool {
//         let tmp = unsafe { std::mem::transmute::<Self, (u64, u64, u64, u64)>(*self) };
//         tmp.0 == 0 && tmp.1 == 0 && tmp.2 == 0 && tmp.3 == 0
//     }

//     #[inline(always)]
//     fn shift_back_lex(&self, n: usize) -> Self {
//         let mut tmp1;
//         let tmp2 = unsafe { std::mem::transmute::<Self, [u8; 32]>(*self) };
//         match n {
//             0 => return *self,
//             1 => {
//                 tmp1 = unsafe {
//                     std::mem::transmute::<Self, [u8; 32]>(x86_64::_mm256_srli_si256(*self, 1))
//                 };
//                 tmp1[15] = tmp2[16];
//             }
//             2 => {
//                 tmp1 = unsafe {
//                     std::mem::transmute::<Self, [u8; 32]>(x86_64::_mm256_srli_si256(*self, 2))
//                 };
//                 tmp1[15] = tmp2[17];
//                 tmp1[14] = tmp2[16];
//             }
//             _ => unreachable!(),
//         }
//         unsafe { std::mem::transmute::<[u8; 32], Self>(tmp1) }
//     }

//     #[inline(always)]
//     fn shr(&self, n: usize) -> Self {
//         match n {
//             0 => *self,
//             1 => unsafe { x86_64::_mm256_srli_epi64(*self, 1) },
//             2 => unsafe { x86_64::_mm256_srli_epi64(*self, 2) },
//             3 => unsafe { x86_64::_mm256_srli_epi64(*self, 3) },
//             4 => unsafe { x86_64::_mm256_srli_epi64(*self, 4) },
//             _ => unreachable!(),
//         }
//     }

//     #[inline(always)]
//     fn cmp_eq_byte(&self, byte: u8) -> Self {
//         let tmp = unsafe { x86_64::_mm256_cmpeq_epi8(*self, Self::splat(byte)) };
//         unsafe { x86_64::_mm256_and_si256(tmp, Self::splat(1)) }
//     }

//     #[inline(always)]
//     fn bytes_between_127(&self, a: u8, b: u8) -> Self {
//         let tmp2 = unsafe { x86_64::_mm256_cmpgt_epi8(*self, Self::splat(a)) };
//         let tmp1 = {
//             let tmp = unsafe { x86_64::_mm256_cmpgt_epi8(*self, Self::splat(b + 1)) };
//             unsafe { x86_64::_mm256_andnot_si256(tmp, Self::splat(0xff)) }
//         };
//         let tmp3 = unsafe { x86_64::_mm256_and_si256(tmp1, tmp2) };
//         unsafe { x86_64::_mm256_and_si256(tmp3, Self::splat(1)) }
//     }

//     #[inline(always)]
//     fn bitand(&self, other: Self) -> Self {
//         unsafe { x86_64::_mm256_and_si256(*self, other) }
//     }

//     #[inline(always)]
//     fn add(&self, other: Self) -> Self {
//         unsafe { x86_64::_mm256_add_epi8(*self, other) }
//     }

//     #[inline(always)]
//     fn sub(&self, other: Self) -> Self {
//         unsafe { x86_64::_mm256_sub_epi8(*self, other) }
//     }

//     #[inline(always)]
//     fn inc_nth_from_end_lex_byte(&self, n: usize) -> Self {
//         let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 32]>(*self) };
//         tmp[31 - n] += 1;
//         unsafe { std::mem::transmute::<[u8; 32], Self>(tmp) }
//     }

//     #[inline(always)]
//     fn dec_last_lex_byte(&self) -> Self {
//         let mut tmp = unsafe { std::mem::transmute::<Self, [u8; 32]>(*self) };
//         tmp[31] -= 1;
//         unsafe { std::mem::transmute::<[u8; 32], Self>(tmp) }
//     }

//     #[inline(always)]
//     fn sum_bytes(&self) -> usize {
//         const ONES: u64 = std::u64::MAX / 0xFF;
//         let tmp = unsafe { std::mem::transmute::<Self, (u64, u64, u64, u64)>(*self) };
//         let a = tmp.0.wrapping_mul(ONES) >> (7 * 8);
//         let b = tmp.1.wrapping_mul(ONES) >> (7 * 8);
//         let c = tmp.2.wrapping_mul(ONES) >> (7 * 8);
//         let d = tmp.3.wrapping_mul(ONES) >> (7 * 8);
//         (a + b + c + d) as usize
//     }
// }

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

    #[test]
    fn usize_flag_bytes_01() {
        let v: usize = 0xE2_09_08_A6_E2_A6_E2_09;
        assert_eq!(0x00_00_00_00_00_00_00_00, v.cmp_eq_byte(0x07));
        assert_eq!(0x00_00_01_00_00_00_00_00, v.cmp_eq_byte(0x08));
        assert_eq!(0x00_01_00_00_00_00_00_01, v.cmp_eq_byte(0x09));
        assert_eq!(0x00_00_00_01_00_01_00_00, v.cmp_eq_byte(0xA6));
        assert_eq!(0x01_00_00_00_01_00_01_00, v.cmp_eq_byte(0xE2));
    }

    #[test]
    fn usize_bytes_between_127_01() {
        let v: usize = 0x7E_09_00_A6_FF_7F_08_07;
        assert_eq!(0x01_01_00_00_00_00_01_01, v.bytes_between_127(0x00, 0x7F));
        assert_eq!(0x00_01_00_00_00_00_01_00, v.bytes_between_127(0x07, 0x7E));
        assert_eq!(0x00_01_00_00_00_00_00_00, v.bytes_between_127(0x08, 0x7E));
    }
}
