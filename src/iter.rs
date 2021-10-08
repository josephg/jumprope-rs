use std::ops::Range;
use crate::jumprope::*;
use crate::utils::str_chars_to_bytes;

/// An iterator over chunks (nodes) in the list.
pub(crate) struct NodeIter<'a>(Option<&'a Node>);

impl<'a> Iterator for NodeIter<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        let prev = self.0;
        if let Some(n) = self.0 {
            // TODO: What?
            *self = NodeIter(unsafe { n.next_ptr().as_ref() });
        }
        prev
    }
}

/// A content iterator iterates over the strings in the rope
pub struct RawContentIter<'a> {
    next: Option<&'a Node>,
    /// Are we at the start or the end of the gap buffer?
    at_start: bool,
}

impl<'a> Iterator for RawContentIter<'a> {
    type Item = (&'a str, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.next {
            let s = if self.at_start {
                self.at_start = false;
                (n.str.start_as_str(), n.str.gap_start_chars as usize)
            } else {
                self.next = unsafe { n.next_ptr().as_ref() };
                self.at_start = true;
                (n.str.end_as_str(), n.num_chars() - n.str.gap_start_chars as usize)
            };

            if s.1 > 0 {
                return Some(s);
            }
        }

        None
    }
}

pub struct StrSlices<'a, I: Iterator<Item=(&'a str, usize)>>(I);

impl<'a, I: Iterator<Item=(&'a str, usize)>> Iterator for StrSlices<'a, I> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(s, _)| s)
    }
}

pub struct CharsRaw<'a, I: Iterator<Item=(&'a str, usize)>> {
    inner: I,
    current: std::str::Chars<'a>,
}

impl<'a, I: Iterator<Item=(&'a str, usize)>> From<I> for CharsRaw<'a, I> {
    fn from(inner: I) -> Self {
        Self {
            inner,
            current: "".chars()
        }
    }
}

impl<'a, I: Iterator<Item=(&'a str, usize)>> Iterator for CharsRaw<'a, I> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.next().or_else(|| {
            self.current = self.inner.next()?.0.chars();
            let next = self.current.next();
            // None of the items returned from our inner iterator should be empty.
            debug_assert!(next.is_some());
            next
        })
    }
}

pub type StrContentIter<'a> = StrSlices<'a, RawContentIter<'a>>;
pub type Chars<'a> = CharsRaw<'a, RawContentIter<'a>>;

impl<'a> RawContentIter<'a> {
    pub fn strings(self) -> StrContentIter<'a> {
        StrSlices(self)
    }

    pub fn chars(self) -> Chars<'a> {
        self.into()
    }
}

pub struct ContentRangeIter<'a> {
    inner: RawContentIter<'a>,
    skip: usize,
    take_len: usize,
}

pub type StrRangeIter<'a> = StrSlices<'a, ContentRangeIter<'a>>;
pub type CharsSlice<'a> = CharsRaw<'a, ContentRangeIter<'a>>;

impl<'a> ContentRangeIter<'a> {
    pub fn strings(self) -> StrRangeIter<'a> {
        StrSlices(self)
    }

    pub fn chars(self) -> CharsSlice<'a> {
        self.into()
    }
}

impl<'a> Iterator for ContentRangeIter<'a> {
    type Item = (&'a str, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.take_len == 0 { return None; }

        self.inner.next().map(|(mut s, mut char_len)| {
            if self.skip > 0 {
                let byte = str_chars_to_bytes(s, self.skip);
                assert!(byte < s.len());

                s = &s[byte..];
                char_len -= self.skip;
                self.skip = 0;
            }

            if self.take_len < char_len {
                let byte = str_chars_to_bytes(s, self.take_len);
                s = &s[0..byte];
                char_len = self.take_len;
            }

            self.take_len -= char_len;

            (s, char_len)
        })
    }
}

impl JumpRope {
    pub(crate) fn node_iter(&self) -> NodeIter { NodeIter(Some(&self.head)) }

    /// Iterate over all "string chunks" in the rope. Iterated chunks are pairs of (str, char_len)
    /// items. The way items are split by the library is undefined, and should not be relied upon.
    /// (It may change in minor point releases).
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("oh hai");
    /// let mut string = String::new();
    /// for (str, char_len) in rope.chunks() {
    ///     assert_eq!(str.chars().count(), char_len);
    ///     string.push_str(str);
    /// }
    /// assert_eq!(string, "oh hai");
    /// ```
    pub fn chunks(&self) -> RawContentIter {
        RawContentIter {
            next: Some(&self.head),
            at_start: true
        }
    }

    /// Get an iterator over all characters in the rope.
    ///
    /// In most cases this will be less efficient than using [`chunks`](Self::chunks) to
    /// iterate over all &str items contained in the rope.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("oh hai");
    /// assert_eq!("oh hai", rope.chars().collect::<String>());
    /// ```
    pub fn chars(&self) -> Chars {
        self.chunks().chars()
    }

    /// Iterate through chunks across a character range in the document.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("xxxGreetings!xxx");
    /// let mut string = String::new();
    /// for (str, char_len) in rope.slice_chunks(3..rope.len_chars() - 3) {
    ///     assert_eq!(str.chars().count(), char_len);
    ///     string.push_str(str);
    /// }
    /// assert_eq!(string, "Greetings!");
    /// ```
    pub fn slice_chunks(&self, range: Range<usize>) -> ContentRangeIter {
        let cursor = self.cursor_at_char(range.start, false);
        let node = unsafe { cursor.here_ptr().as_ref().unwrap() };
        let node_gap_start = node.str.gap_start_chars as usize;
        let local_pos = cursor.local_char_pos();

        let (at_start, skip) = if local_pos >= node_gap_start {
            (false, local_pos - node_gap_start)
        } else {
            (true, local_pos)
        };

        ContentRangeIter {
            inner: RawContentIter {
                next: Some(node), at_start
            },
            skip,
            take_len: range.end - range.start
        }
    }

    /// Iterate through characters in the rope within the specified range. The range is specified
    /// using unicode characters, not bytes.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("xxxGreetings!xxx");
    ///
    /// assert_eq!("Greetings!",
    ///     rope.slice_chars(3..rope.len_chars() - 3).collect::<String>()
    /// );
    /// ```
    pub fn slice_chars(&self, range: Range<usize>) -> CharsSlice {
        self.slice_chunks(range).chars()
    }
}

#[cfg(test)]
mod tests {
    use crate::JumpRope;
    use crate::jumprope::NODE_STR_SIZE;
    use crate::utils::{count_chars, str_chars_to_bytes};

    fn check(rope: &JumpRope) {
        for (s, len) in rope.chunks() {
            assert_eq!(count_chars(s), len);
            assert_ne!(len, 0); // Returned items may not be empty.
        }

        for (s, len) in rope.slice_chunks(0..rope.len_chars()) {
            assert_eq!(count_chars(s), len);
            assert_ne!(len, 0); // Returned items may not be empty.
        }

        assert_eq!(rope.chunks().chars().collect::<String>(), rope.to_string());
        assert_eq!(rope.chars().collect::<String>(), rope.to_string());
        assert_eq!(rope.slice_chars(0..rope.len_chars()).collect::<String>(), rope.to_string());

        let s = rope.to_string();
        for start in 0..=rope.len_chars() {
            let iter = rope.slice_chars(start..rope.len_chars());
            let str = iter.collect::<String>();

            let byte_start = str_chars_to_bytes(&s, start);
            assert_eq!(str, &s[byte_start..]);
        }
    }

    #[test]
    fn iter_smoke_tests() {
        check(&JumpRope::new());
        check(&JumpRope::from("hi there"));

        let mut rope = JumpRope::from("aaaa");
        rope.insert(2, "b"); // This will force a gap.
        assert_eq!(rope.chunks().count(), 2);
        check(&rope);

        // Long enough that in debugging mode we'll spill into multiple items.
        let s = "XXXaaaaaaaaaaaaaaaaaaaaaaaaaaXXX";
        let rope = JumpRope::from(s);
        assert!(rope.chunks().count() > 1);
        check(&rope);

        assert_eq!(
            rope.slice_chunks(3..s.len() - 3).chars().collect::<String>(),
            &s[3..s.len() - 3]
        );
    }

    #[test]
    fn iter_non_ascii() {
        check(&JumpRope::from("κό𝕐𝕆😘σμε"));
    }

    #[test]
    fn iter_chars_tricky() {
        let mut rope = JumpRope::new();
        rope.extend(std::iter::repeat("x").take(NODE_STR_SIZE * 2));
        check(&rope);
    }
}