#[cfg(test)]
mod test {
    extern crate jumprope;
    use self::jumprope::{Rope, JumpRope};

    static UCHARS: [char; 23] = [
      'a', 'b', 'c', '1', '2', '3', ' ', '\n', // ASCII
      '©', '¥', '½', // The Latin-1 suppliment (U+80 - U+ff)
      'Ύ', 'Δ', 'δ', 'Ϡ', // Greek (U+0370 - U+03FF)
      '←', '↯', '↻', '⇈', // Arrows (U+2190 – U+21FF)
      '𐆐', '𐆔', '𐆘', '𐆚', // Ancient roman symbols (U+10190 – U+101CF)
    ];

    fn check<T: Rope>(r: &T, expected: &str) {
        assert_eq!(r.len(), expected.len());
        assert_eq!(r.to_string(), expected);
        assert_eq!(r.char_len(), expected.chars().count());
    }

    #[test]
    fn empty_rope_has_no_contents() {
        let mut r = JumpRope::new();
        check(&r, "");

        r.insert(0, "");
        check(&r, "");
    }

    #[test]
    fn insert_at_location() {
        let mut r = JumpRope::new();

        r.insert(0, "AAA").unwrap();
        check(&r, "AAA");

        r.insert(0, "BBB").unwrap();
        check(&r, "BBBAAA");

        r.insert(6, "CCC").unwrap();
        check(&r, "BBBAAACCC");

        r.insert(5, "DDD").unwrap();
        check(&r, "BBBAADDDACCC");
    }


}
