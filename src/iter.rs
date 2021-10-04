use crate::{JumpRope, Node};

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

impl<'a> Iterator for ContentIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.next {
            let s = if self.at_start {
                self.at_start = false;
                n.str.start_as_str()
            } else {
                self.next = unsafe { n.next_ptr().as_ref() };
                self.at_start = true;
                n.str.end_as_str()
            };

            if !s.is_empty() {
                return Some(s);
            }
        }

        None
    }
}

impl JumpRope {
    pub(crate) fn node_iter(&self) -> NodeIter { NodeIter(Some(&self.head)) }

    pub fn content_iter(&self) -> ContentIter {
        ContentIter {
            next: Some(&self.head),
            at_start: true
        }
    }
}