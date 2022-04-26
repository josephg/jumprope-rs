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
pub struct ContentIter<'a> {
    next: Option<&'a Node>,
    /// Are we at the start or the end of the gap buffer?
    at_start: bool,
}

impl<'a> ContentIter<'a> {
    pub fn substrings(self) -> Substrings<'a> {
        Substrings(self)
    }

    pub fn chars(self) -> Chars<'a> {
        self.into()
    }
}

impl<'a> Iterator for ContentIter<'a> {
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

/// Iterator over the substrings in some content. This is just a hand-written .map(|s, len| s)
/// iterator to make it possible to embed a jumprope iterator inside another iterator.
pub struct Substrings<'a, I: Iterator<Item=(&'a str, usize)> = ContentIter<'a>>(I);

impl<'a, I: Iterator<Item=(&'a str, usize)>> Substrings<'a, I> {
    /// Convert this content into a string
    pub fn into_string(self) -> String {
        self.collect::<String>()
    }
}

impl<'a, I: Iterator<Item=(&'a str, usize)>> Iterator for Substrings<'a, I> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(s, _)| s)
    }
}

/// Iterator over the individual characters in a rope (or rope slice).
pub struct Chars<'a, I: Iterator<Item=(&'a str, usize)> = ContentIter<'a>> {
    inner: I,
    current: std::str::Chars<'a>,
}

impl<'a, I: Iterator<Item=(&'a str, usize)>> From<I> for Chars<'a, I> {
    fn from(inner: I) -> Self {
        Self {
            inner,
            current: "".chars()
        }
    }
}

impl<'a, I: Iterator<Item=(&'a str, usize)>> Iterator for Chars<'a, I> {
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

/// Iterate over a sub-range of the rope.
pub struct SliceIter<'a> {
    inner: ContentIter<'a>,
    skip: usize,
    take_len: usize,
}

pub type SubstringsInRange<'a> = Substrings<'a, SliceIter<'a>>;
pub type CharsInRange<'a> = Chars<'a, SliceIter<'a>>;

impl<'a> SliceIter<'a> {
    pub fn substrings(self) -> SubstringsInRange<'a> {
        Substrings(self)
    }

    pub fn chars(self) -> CharsInRange<'a> {
        self.into()
    }
}

impl<'a> Iterator for SliceIter<'a> {
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
    pub(crate) fn node_iter_at_start(&self) -> NodeIter { NodeIter(Some(&self.head)) }

    /// Iterate over the rope, visiting each substring in [`str`] chunks. Whenever possible, this is
    /// the best way for a program to read back the contents of a rope, because it avoids allocating
    /// memory or copying the characters themselves (as you get with .to_string() or .chars()).
    ///
    /// ## Stability Warning
    ///
    /// This iterator will always return all the characters in document order, but the particular
    /// way characters are grouped together is based on internal implementation details. Thus it
    /// might change in arbitrary ways at any time. Your application should not depend on the
    /// specifics of this chunking.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("oh hai");
    /// let mut string = String::new();
    /// for str in rope.substrings() {
    ///     string.push_str(str);
    /// }
    /// assert_eq!(string, "oh hai");
    /// ```
    pub fn substrings(&self) -> Substrings<'_> {
        self.substrings_with_len().substrings()
    }

    /// Iterate over all substrings in the rope, but also yield the unicode character length for
    /// each item. A caller could obviously recalculate these lengths from the provided &str
    /// objects, but since the unicode lengths are known this allows small optimizations.
    ///
    /// The iterator yields pairs of (str, char_len).
    ///
    /// ## Stability Warning
    ///
    /// This iterator will always return all the characters in document order, but the particular
    /// way characters are grouped together is based on internal implementation details. Thus it
    /// might change in arbitrary ways at any time. Your application should not depend on the
    /// specifics of this chunking.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("oh hai");
    /// let mut string = String::new();
    /// for (str, char_len) in rope.substrings_with_len() {
    ///     assert_eq!(str.chars().count(), char_len);
    ///     string.push_str(str);
    /// }
    /// assert_eq!(string, "oh hai");
    /// ```
    pub fn substrings_with_len(&self) -> ContentIter {
        ContentIter {
            next: Some(&self.head),
            at_start: true
        }
    }

    /// Get an iterator over all characters in the rope.
    ///
    /// In most cases this will be less efficient than using [`substrings`](Self::substrings) to
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
        self.substrings_with_len().chars()
    }



    /// Iterate through all the substrings within the specified unicode character range in the
    /// document.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("xxxGreetings!xxx");
    /// let mut string = String::new();
    /// for s in rope.slice_substrings(3..rope.len_chars() - 3) {
    ///     string.push_str(s);
    /// }
    /// assert_eq!(string, "Greetings!");
    /// ```
    pub fn slice_substrings(&self, range: Range<usize>) -> SubstringsInRange {
        self.slice_substrings_with_len(range).substrings()
    }

    /// Iterate through chunks across a character range in the document.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("xxxGreetings!xxx");
    /// let mut string = String::new();
    /// for (str, char_len) in rope.slice_substrings_with_len(3..rope.len_chars() - 3) {
    ///     assert_eq!(str.chars().count(), char_len);
    ///     string.push_str(str);
    /// }
    /// assert_eq!(string, "Greetings!");
    /// ```
    ///
    /// Or more simply:
    ///
    /// ```
    /// # use jumprope::*;
    /// let rope = JumpRope::from("xxxGreetings!xxx");
    /// let string = rope.slice_substrings_with_len(3..13).map(|(str, _len)| str).collect::<String>();
    /// assert_eq!(string, "Greetings!");
    /// ```
    pub fn slice_substrings_with_len(&self, range: Range<usize>) -> SliceIter {
        let cursor = self.read_cursor_at_char(range.start, false);
        let node_gap_start = cursor.node.str.gap_start_chars as usize;
        let local_pos = cursor.offset_chars;

        let (at_start, skip) = if local_pos >= node_gap_start {
            (false, local_pos - node_gap_start)
        } else {
            (true, local_pos)
        };

        SliceIter {
            inner: ContentIter {
                next: Some(cursor.node), at_start
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
    pub fn slice_chars(&self, range: Range<usize>) -> CharsInRange {
        self.slice_substrings_with_len(range).chars()
    }
}

#[cfg(test)]
mod tests {
    use crate::fast_str_tools::*;
    use crate::JumpRope;
    use crate::jumprope::NODE_STR_SIZE;

    fn check(rope: &JumpRope) {
        for (s, len) in rope.substrings_with_len() {
            assert_eq!(count_chars(s), len);
            assert_ne!(len, 0); // Returned items may not be empty.
        }

        for (s, len) in rope.slice_substrings_with_len(0..rope.len_chars()) {
            assert_eq!(count_chars(s), len);
            assert_ne!(len, 0); // Returned items may not be empty.
        }

        assert_eq!(rope.substrings_with_len().chars().collect::<String>(), rope.to_string());
        assert_eq!(rope.chars().collect::<String>(), rope.to_string());
        assert_eq!(rope.slice_chars(0..rope.len_chars()).collect::<String>(), rope.to_string());

        let s = rope.to_string();
        for start in 0..=rope.len_chars() {
            let iter = rope.slice_chars(start..rope.len_chars());
            let str = iter.collect::<String>();

            let byte_start = char_to_byte_idx(&s, start);
            assert_eq!(str, &s[byte_start..]);
        }
    }

    #[test]
    fn iter_smoke_tests() {
        check(&JumpRope::new());
        check(&JumpRope::from("hi there"));

        let mut rope = JumpRope::from("aaaa");
        rope.insert(2, "b"); // This will force a gap.
        assert_eq!(rope.substrings_with_len().count(), 2);
        check(&rope);

        // Long enough that in debugging mode we'll spill into multiple items.
        let s = "XXXaaaaaaaaaaaaaaaaaaaaaaaaaaXXX";
        let rope = JumpRope::from(s);
        assert!(rope.substrings_with_len().count() > 1);
        check(&rope);

        assert_eq!(
            rope.slice_substrings_with_len(3..s.len() - 3).chars().collect::<String>(),
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