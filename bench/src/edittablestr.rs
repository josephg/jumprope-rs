use super::Rope;
use std::ptr;

// pub trait EditableText {
//     // pos is in utf8 codepoints
//     fn insert_at(&mut self, pos: usize, contents: &str);
//     fn remove_at(&mut self, pos: usize, length: usize);
// }

impl Rope for String {
    fn new() -> Self { String::new() }
    
    fn insert_at(&mut self, char_pos: usize, contents: &str) {
        // If you try to write past the end of the string for now I'll just write at the end.
        // Panicing might be a better policy.
        let byte_pos = self.char_indices().skip(char_pos).next()
            .map(|(p, _)| p).unwrap_or(self.len());
        //println!("pos {}", byte_pos);
        //self.insert_str(byte_pos, contents);
        
        let old_len = self.len();
        let new_bytes = contents.len();

        // This didn't work because it didn't change the string's length
        //self.reserve(new_bytes);

        // This is sort of ugly but its fine.
        for _ in 0..new_bytes { self.push('\0'); }

        //println!("new bytes {} {} {}", new_bytes, byte_pos, self.len() - byte_pos);
        unsafe {
            let bytes = self.as_mut_vec().as_mut_ptr();
            //println!("{:?}", self.as_mut_vec());
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
            //println!("{:?}", self.as_mut_vec());
        }
    }
    fn del_at(&mut self, pos: usize, length: usize) {
        let byte_range = {
            let mut iter = self.char_indices().map(|(p, _)| p).skip(pos).peekable();

            let start = iter.peek().map_or_else(|| self.len(), |&p| p);
            let mut iter = iter.skip(length).peekable();
            let end = iter.peek().map_or_else(|| self.len(), |&p| p);

            start..end
        };

        self.drain(byte_range);
    }

    // fn len(&self) -> usize { self.len() }
    fn char_len(&self) -> usize { self.chars().count() }
    fn to_string(&self) -> String { self.clone() }
}



#[cfg(test)]
mod tests {
    use super::Rope;

    #[test]
    fn insert_simple() {
        let mut s = "".to_string();
        s.insert_at(0, "hi").unwrap();
        assert_eq!(s, "hi");

        let mut s = "a".to_string();
        s.insert_at(0, "hi").unwrap();
        assert_eq!(s, "hia");

        let mut s = "a".to_string();
        s.insert_at(1, "hi").unwrap();
        assert_eq!(s, "ahi");

        let mut s = "ac".to_string();
        s.insert_at(1, "b").unwrap();
        assert_eq!(s, "abc");
    }

    #[test]
    fn insert_unicode() {
        // I mean, its all unicode but ....
        let mut s = "𝄞𝄞".to_string();
        s.insert_at(0, "à").unwrap();
        assert_eq!(s, "à𝄞𝄞");
        s.insert_at(2, "ë").unwrap();
        assert_eq!(s, "à𝄞ë𝄞");
        s.insert_at(4, "ç").unwrap();
        assert_eq!(s, "à𝄞ë𝄞ç");
        s.insert_at(6, "𝒲").unwrap();
        assert_eq!(s, "à𝄞ë𝄞ç𝒲");
    }

    #[test]
    fn remove_simple() {
        let mut s = "à".to_string();
        s.del_at(0, 1).unwrap();
        assert_eq!(s, "");
        s.del_at(0, 0).unwrap();
        assert_eq!(s, "");

        let mut s = "à𝄞ç".to_string();
        s.del_at(0, 1).unwrap();
        assert_eq!(s, "𝄞ç");
        s.del_at(1, 1).unwrap();
        assert_eq!(s, "𝄞");
        s.del_at(0, 1).unwrap();
        assert_eq!(s, "");
    }
}
