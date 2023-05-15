// These tests are also adapted from the C code tests here:
// https://github.com/josephg/librope/blob/master/test/tests.c

use rand::prelude::*;

use std::cmp::min;
use std::ops::Range;
use std::ptr;
use jumprope::JumpRope;
use jumprope::JumpRopeBuf;

const UNI_CHARS: [char; 24] = [
  '\n', 'a', 'b', 'c', '1', '2', '3', ' ', '_', // ASCII.
  '©', '¥', '½', // The Latin-1 suppliment (U+80 - U+ff)
  'Ύ', 'Δ', 'δ', 'Ϡ', // Greek (U+0370 - U+03FF)
  '←', '↯', '↻', '⇈', // Arrows (U+2190 – U+21FF)
  '𐆐', '𐆔', '𐆘', '𐆚', // Ancient roman symbols (U+10190 – U+101CF)
];

fn random_unicode_string(len: usize, rng: &mut SmallRng) -> String {
    let mut s = String::new();
    for _ in 0..len {
        s.push(UNI_CHARS[rng.gen_range(0 .. UNI_CHARS.len())] as char);
    }
    s
}

const ASCII_CHARS: &[u8; 83] = b" ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()[]{}<>?,./";

// Gross. Find a way to reuse the code from random_unicode_string.
#[allow(unused)]
fn random_ascii_string(len: usize, rng: &mut SmallRng) -> String {
    let mut s = String::new();
    for _ in 0..len {
        s.push(ASCII_CHARS[rng.gen_range(0 .. ASCII_CHARS.len())] as char);
    }
    s
}

fn check(r: &JumpRope, expected: &str) {
    // println!("--- rope ---");
    // r.print();

    r.check();
    assert_eq!(r.to_string(), expected);
    assert_eq!(r.len_bytes(), expected.len());
    assert_eq!(r.len_chars(), expected.chars().count());
    #[cfg(feature = "wchar_conversion")] {
        assert_eq!(r.len_wchars(), expected.chars().map(|c| c.len_utf16()).sum());

        assert_eq!(r.chars_to_wchars(r.len_chars()), r.len_wchars());
        assert!(r.len_wchars() >= r.len_chars());

        // And if we convert back, we should get the number of characters.
        assert_eq!(r.wchars_to_chars(r.len_wchars()), r.len_chars());
    }
    assert_eq!(*r, JumpRope::from(expected), "Rope comparison fails");

    let clone = r.clone();
    // println!("--- clone ---");
    // clone.print();
    clone.check();
    assert_eq!(*r, clone, "Rope does not equal its clone");
}

#[test]
fn empty_rope_has_no_contents() {
    let mut r = JumpRope::new();
    check(&r, "");

    r.insert(0, "");
    check(&r, "");
}

#[test]
fn from_str_and_string() {
    let r1 = JumpRope::from("hi");
    check(&r1, "hi");

    let r2 = JumpRope::from(String::from("hi"));
    check(&r2, "hi");
}

#[test]
fn insert_at_location() {
    let mut r = JumpRope::new();

    r.insert(0, "AAA");
    check(&r, "AAA");

    r.insert(0, "BBB");
    check(&r, "BBBAAA");

    r.insert(6, "CCC");
    check(&r, "BBBAAACCC");

    r.insert(5, "DDD");
    check(&r, "BBBAADDDACCC");
}

#[test]
fn new_string_has_content() {
    let r = JumpRope::from("hi there");
    check(&r, "hi there");

    let mut r = JumpRope::from("κόσμε");
    check(&r, "κόσμε");
    r.insert(2, "𝕐𝕆😘");
    check(&r, "κό𝕐𝕆😘σμε");
}

#[test]
fn del_at_location() {
    let mut r = JumpRope::from("012345678");
    check(&r, "012345678");

    r.remove(8..9);
    check(&r, "01234567");

    r.remove(0..1);
    check(&r, "1234567");

    r.remove(5..6);
    check(&r, "123457");

    r.remove(5..6);
    check(&r, "12345");

    r.remove(0..5);
    check(&r, "");
}

#[test]
fn del_past_end_of_string() {
    let mut r = JumpRope::new();

    r.remove(0..100);
    check(&r, "");

    r.insert(0, "hi there");
    r.remove(3..13);
    check(&r, "hi ");
}

#[test]
fn really_long_ascii_string() {
    let mut rng = SmallRng::seed_from_u64(1234);
    let len = 2000;
    let s = random_ascii_string(len, &mut rng);
    // let s = random_unicode_string(len, &mut rng);

    let mut r = JumpRope::from(s.as_str());
    check(&r, s.as_str());

    // Delete everything but the first and last characters
    r.remove(1..len - 1);
    let expect = format!("{}{}", s.chars().next().unwrap(), s.chars().rev().next().unwrap());
    check(&r, expect.as_str());
}

fn string_insert_at(s: &mut String, char_pos: usize, contents: &str) {
    // If you try to write past the end of the string for now I'll just write at the end.
    // Panicing might be a better policy.
    let byte_pos = s.char_indices().skip(char_pos).next()
        .map(|(p, _)| p).unwrap_or(s.len());

    let old_len = s.len();
    let new_bytes = contents.len();

    // This didn't work because it didn't change the string's length
    //s.reserve(new_bytes);

    // This is sort of ugly but its fine.
    for _ in 0..new_bytes { s.push('\0'); }

    //println!("new bytes {} {} {}", new_bytes, byte_pos, s.len() - byte_pos);
    unsafe {
        let bytes = s.as_mut_vec().as_mut_ptr();
        ptr::copy(
            bytes.offset(byte_pos as isize),
            bytes.offset((byte_pos + new_bytes) as isize),
            old_len - byte_pos
        );
        ptr::copy_nonoverlapping(
            contents.as_ptr(),
            bytes.offset(byte_pos as isize),
            new_bytes
        );
    }
}

fn char_range_to_byte_range(s: &String, range: Range<usize>) -> Range<usize> {
    let mut iter = s.char_indices().map(|(p, _)| p).skip(range.start).peekable();

    let start = iter.peek().map_or_else(|| s.len(), |&p| p);
    let mut iter = iter.skip(range.end - range.start).peekable();
    let end = iter.peek().map_or_else(|| s.len(), |&p| p);

    start..end
}

fn string_del_at(s: &mut String, pos: usize, length: usize) {
    let byte_range = char_range_to_byte_range(s, pos..pos+length);

    s.drain(byte_range);
}

fn random_edits(seed: u64, verbose: bool) {
    let mut r = JumpRope::new();
    let mut s = String::new();

    // let mut rng = rand::thread_rng();
    let mut rng = SmallRng::seed_from_u64(seed);

    for _i in 0..400 {
        if verbose { println!("{_i} s: '{s}'"); }
        // r.print();

        let len = s.chars().count();

        // if _i == 1 {
        //     println!("haaayyy");
        // }
        // println!("i {}: {}", i, len);

        if len == 0 || (len < 1000 && rng.gen::<f32>() < 0.5) {
            // Insert.
            let pos = rng.gen_range(0..len+1);
            // Sometimes generate strings longer than a single node to stress everything.
            let text = random_unicode_string(rng.gen_range(0..20), &mut rng);
            if verbose {
                println!("Inserting '{text}' at char {pos} (Byte length: {}, char len: {}, wchar len: {})",
                         text.len(), text.chars().count(),
                         text.chars().map(|c| c.len_utf16()).sum::<usize>()
                );
            }

            r.insert(pos, text.as_str());
            string_insert_at(&mut s, pos, text.as_str());
        } else {
            // Delete
            let pos = rng.gen_range(0..len);
            let dlen = min(rng.gen_range(0..10), len - pos);
            if verbose {
                println!("Removing {dlen} characters at {pos}");
            }

            r.remove(pos..pos+dlen);
            string_del_at(&mut s, pos, dlen);
        }

        // Calling check() is super slow with miri, and it doesn't matter much so long as we test
        // for correctness normally.
        if !cfg!(miri) {
            check(&r, s.as_str());
        }
    }

    if cfg!(miri) {
        check(&r, s.as_str());
    }
}

#[test]
fn fuzz_once() {
    random_edits(10, false);
}

// Run with:
// cargo test --release fuzz_forever -- --ignored --nocapture
#[test]
#[ignore]
fn fuzz_forever() {
    for seed in 0.. {
        if seed % 100 == 0 { println!("seed: {seed}"); }
        random_edits(seed, false);
    }
}

#[cfg(feature = "wchar_conversion")]
fn random_edits_wchar(seed: u64, verbose: bool) {
    let mut r = JumpRope::new();
    let mut s = String::new();

    // let mut rng = rand::thread_rng();
    let mut rng = SmallRng::seed_from_u64(seed);

    for _i in 0..400 {
        if verbose { println!("{_i} s: '{s}'"); }
        // r.print();
        let len_chars = s.chars().count();

        // println!("i {}: {}", i, len);

        if len_chars == 0 || (len_chars < 1000 && rng.gen::<f32>() < 0.5) {
            // Insert.
            let pos_chars = rng.gen_range(0..len_chars + 1);
            // Convert pos to wchars
            let pos_wchar = s
                .chars()
                .take(pos_chars)
                .map(|c| c.len_utf16())
                .sum();
            // Sometimes generate strings longer than a single node to stress everything.
            let text = random_unicode_string(rng.gen_range(0..20), &mut rng);
            if verbose {
                println!("Inserting '{text}' at char {pos_chars} / wchar {pos_wchar}");
                println!("Byte length {} char len {} / wchar len {}",
                         text.len(), text.chars().count(), text.chars().map(|c| c.len_utf16()).sum::<usize>());
            }
            r.insert_at_wchar(pos_wchar, text.as_str());
            // r.print();
            string_insert_at(&mut s, pos_chars, text.as_str());
        } else {
            // Delete
            let pos_chars = rng.gen_range(0..len_chars);
            let dlen_chars = min(rng.gen_range(0..10), len_chars - pos_chars);
            let char_range = pos_chars..pos_chars+dlen_chars;
            let byte_range = char_range_to_byte_range(&s, char_range.clone());
            // Now convert it to a wchar range :p
            let start_wchar = s[..byte_range.start].chars().map(|c| c.len_utf16()).sum::<usize>();
            let len_wchar = s[byte_range.clone()].chars().map(|c| c.len_utf16()).sum::<usize>();
            let wchar_range = start_wchar..start_wchar + len_wchar;

            if verbose {
                println!("Removing {}..{} (wchar {}..{})",
                         char_range.start, char_range.end,
                         wchar_range.start, wchar_range.end
                );
            }

            // r.remove(pos_chars..pos_chars + dlen_chars);
            r.remove_at_wchar(wchar_range);
            // r.print();
            // string_del_at(&mut s, pos_chars, dlen_chars);
            s.drain(byte_range);
        }

        if !cfg!(miri) {
            check(&r, s.as_str());
        }
    }
}

#[cfg(feature = "wchar_conversion")]
#[test]
fn fuzz_wchar_once() {
    random_edits_wchar(22, false);
}

// Run with:
// cargo test --release fuzz_forever -- --ignored --nocapture
#[cfg(feature = "wchar_conversion")]
#[test]
#[ignore]
fn fuzz_wchar_forever() {
    for seed in 0.. {
        if seed % 100 == 0 { println!("seed: {seed}"); }
        random_edits_wchar(seed, false);
    }
}

fn random_edits_buffered(seed: u64, verbose: bool) {
    let mut r = JumpRopeBuf::new();
    let mut s = String::new();

    // let mut rng = rand::thread_rng();
    let mut rng = SmallRng::seed_from_u64(seed);

    for _i in 0..400 {
    // for _i in 0..19 {
        if verbose { println!("{_i} s: '{s}'"); }
        // r.print();

        let len = s.chars().count();

        // if _i == 1 {
        //     println!("haaayyy");
        // }
        // println!("i {}: {}", i, len);

        if len == 0 || (len < 1000 && rng.gen::<f32>() < 0.5) {
            // Insert.
            let pos = rng.gen_range(0..len+1);
            // Sometimes generate strings longer than a single node to stress everything.
            let text = random_unicode_string(rng.gen_range(0..20), &mut rng);
            if verbose {
                println!("Inserting '{text}' at char {pos} (Byte length: {}, char len: {}, wchar len: {})",
                         text.len(), text.chars().count(),
                         text.chars().map(|c| c.len_utf16()).sum::<usize>()
                );
            }

            r.insert(pos, text.as_str());
            string_insert_at(&mut s, pos, text.as_str());
        } else {
            // Delete
            let pos = rng.gen_range(0..len);
            let dlen = min(rng.gen_range(0..10), len - pos);
            if verbose {
                println!("Removing {dlen} characters at {pos}");
            }

            r.remove(pos..pos+dlen);
            string_del_at(&mut s, pos, dlen);
        }
        // dbg!(&r);

        assert_eq!(r.is_empty(), s.is_empty());

        // Checking the length flushes the buffered op - which is a useful test, but if we do it
        // every time, the buffer won't build up and the test won't have the right coverage.
        if rng.gen_bool(0.05) {
            assert_eq!(r.len_chars(), s.chars().count());
        }
    }

    let rope = r.into_inner();
    check(&rope, s.as_str());
}

#[test]
fn fuzz_buffered_once() {
    random_edits_buffered(0, false);
}

#[test]
#[ignore]
fn fuzz_buffered_forever() {
    for seed in 0.. {
        if seed % 1000 == 0 { println!("seed: {seed}"); }
        random_edits_buffered(seed, false);
    }
}

#[test]
fn eq_variants() {
    let rope = JumpRope::from("Hi there");

    assert_eq!(rope.clone(), "Hi there");
    assert_eq!(rope.clone(), String::from("Hi there"));
    assert_eq!(rope.clone(), &String::from("Hi there"));

    assert_eq!(&rope, "Hi there");
    assert_eq!(&rope, String::from("Hi there"));
    assert_eq!(&rope, &String::from("Hi there"));
}

#[test]
fn buffered_eq_variants() {
    let rope = JumpRopeBuf::from("Hi there");

    assert_eq!(rope.clone(), "Hi there");
    assert_eq!(rope.clone(), String::from("Hi there"));
    assert_eq!(rope.clone(), &String::from("Hi there"));

    assert_eq!(&rope, "Hi there");
    assert_eq!(&rope, String::from("Hi there"));
    assert_eq!(&rope, &String::from("Hi there"));
}