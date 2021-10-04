// const LEN: usize = 5;
// const LEN: usize = 100;

#[derive(Debug, Clone, Eq)]
pub struct GapBuffer<const LEN: usize> {
    data: [u8; LEN],

    gap_start: u8,
    gap_len: u8,
}

impl<const LEN: usize> GapBuffer<LEN> {
    pub fn new() -> Self {
        Self {
            data: [0; LEN],
            gap_start: 0,
            gap_len: LEN as u8,
        }
    }

    pub fn new_from_str(s: &str) -> Self {
        let mut val = Self::new();
        val.try_insert(0, s).unwrap();
        val
    }

    pub fn len_space(&self) -> usize {
        self.gap_len as usize
    }

    pub fn len(&self) -> usize {
        LEN - self.gap_len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.gap_len as usize == LEN
    }

    pub fn move_gap(&mut self, new_start: usize) {
        let current_start = self.gap_start as usize;

        if new_start != current_start {
            let len = self.gap_len as usize;
            debug_assert!(new_start <= LEN-len);

            if new_start < current_start {
                // move characters to the right.
                self.data.copy_within(new_start..current_start, new_start + len);
            } else if current_start < new_start {
                // Move characters to the left
                self.data.copy_within(current_start+len..new_start+len, current_start);
            }

            if cfg!(debug_assertions) {
                // This is unnecessary but tidy, and makes debugging easier.
                self.data[new_start..new_start+len].fill(0);
            }

            self.gap_start = new_start as u8;
        }
    }

    pub fn try_insert(&mut self, pos: usize, s: &str) -> Result<(), ()> {
        let len = s.len();
        if len > self.gap_len as usize {
            // No space in this node!
            Result::Err(())
        } else {
            self.move_gap(pos);
            let start = self.gap_start as usize;
            self.data[start..start+len].copy_from_slice(s.as_bytes());
            self.gap_start += len as u8;
            self.gap_len -= len as u8;
            Result::Ok(())
        }
    }

    // Returns the number of items actually removed.
    pub fn remove(&mut self, pos: usize, del_len: usize) -> usize {
        let len = self.len();

        if pos >= len { return 0; }
        let remove = del_len.min(len - pos);

        self.move_gap(pos);

        if cfg!(debug_assertions) {
            self.data[
                (self.gap_start+self.gap_len) as usize..(self.gap_start+self.gap_len) as usize + remove
            ].fill(0);
        }
        self.gap_len += remove as u8;
        remove
    }

    pub fn start_as_str(&self) -> &str {
        unsafe {
            std::str::from_utf8_unchecked(&self.data[0..self.gap_start as usize])
        }
    }
    pub fn end_as_str(&self) -> &str {
        unsafe {
            std::str::from_utf8_unchecked(&self.data[(self.gap_start+self.gap_len) as usize..LEN])
        }
    }
}

impl<const LEN: usize> ToString for GapBuffer<LEN> {
    fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str(self.start_as_str());
        result.push_str(self.end_as_str());
        result
    }
}

impl<const LEN: usize> PartialEq for GapBuffer<LEN> {
    // Eq is interesting because we need to ignore where the gap is.
    fn eq(&self, other: &Self) -> bool {
        if self.gap_len != other.gap_len { return false; }
        // There's 3 sections to check:
        // - Before our gap
        // - The inter-gap part
        // - The last, common part.
        let (a, b) = if self.gap_start < other.gap_start {
            (self, other)
        } else {
            (other, self)
        };
        // a has its gap first (or the gaps are at the same time).
        let a_start = a.gap_start as usize;
        let b_start = b.gap_start as usize;
        let gap_len = a.gap_len as usize;

        // Section before the gaps
        if a.data[0..a_start] != b.data[0..a_start] { return false; }

        // Gappy bit
        if a.data[a_start+gap_len..b_start+gap_len] != b.data[a_start..b_start] { return false; }

        // Last bit
        let end_idx = b_start + gap_len;
        a.data[end_idx..LEN] == b.data[end_idx..LEN]
    }
}

#[cfg(test)]
mod test {
    use gapbuffer::GapBuffer;

    fn check_eq<const LEN: usize>(b: &GapBuffer<LEN>, s: &str) {
        assert_eq!(b.to_string(), s);
        assert_eq!(b.len(), s.len());
        assert_eq!(s.is_empty(), b.is_empty());
    }

    #[test]
    fn smoke_test() {
        let mut b = GapBuffer::<5>::new();

        b.try_insert(0, "hi").unwrap();
        b.try_insert(0, "x").unwrap(); // 'xhi'
        // b.move_gap(2);
        b.try_insert(2, "x").unwrap(); // 'xhxi'
        check_eq(&b, "xhxi");
    }

    #[test]
    fn remove() {
        let mut b = GapBuffer::<5>::new_from_str("hi");
        assert_eq!(b.remove(2, 2), 0);
        check_eq(&b, "hi");

        assert_eq!(b.remove(0, 1), 1);
        check_eq(&b, "i");

        assert_eq!(b.remove(0, 1000), 1);
        check_eq(&b, "");
    }

    #[test]
    fn eq() {
        let hi = GapBuffer::<5>::new_from_str("hi");
        let yo = GapBuffer::<5>::new_from_str("yo");
        assert_ne!(hi, yo);
        assert_eq!(hi, hi);

        let mut hi2 = GapBuffer::<5>::new_from_str("hi");
        hi2.move_gap(1);
        assert_eq!(hi, hi2);

        hi2.move_gap(0);
        assert_eq!(hi, hi2);
    }
}