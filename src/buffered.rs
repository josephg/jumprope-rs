// This is an experimental approach to jumprope where we buffer all changes


#[derive(Debug, Clone, Copy)]
enum Tag { Ins, Del }

use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::ops::{DerefMut, Range};
use Op::*;
use crate::fast_str_tools::count_chars;
use crate::JumpRope;

// #[derive(Debug, Clone)]
pub struct JumpRopeBuf(RefCell<(JumpRope, BufferedOp)>);

impl Debug for JumpRopeBuf {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let inner = self.0.borrow();
        f.debug_struct("BufferedRope")
            // .field("rope", &inner.0)
            .field("op", &inner.1)
            .finish()
    }
}

#[derive(Debug, Clone)]
struct BufferedOp {
    tag: Tag,
    ins_content: String,

    // len: usize, // Either ins_content len or del_len.
    // TODO: Alternately, replace range.
    // del_length: usize,
    // pos: usize,
    range: Range<usize>,
}

#[derive(Debug, Clone, Copy)]
enum Op<'a> {
    Ins(usize, &'a str),
    Del(usize, usize), // start, end.
}

impl BufferedOp {
    fn new() -> Self {
        Self {
            tag: Tag::Ins,
            ins_content: "".to_string(),
            range: Range::default(),
            // len: 0,
            // pos: 0
        }
    }

    fn is_empty(&self) -> bool {
        // self.len == 0
        self.range.is_empty()
    }

    fn clear(&mut self) {
        // We don't care about the tag.
        self.ins_content.clear();
        self.range = Range::default();
    }

    fn try_append(&mut self, op: Op) -> Result<(), ()> {
        if self.is_empty() {
            // Just set to op.
            match op {
                Ins(pos, content) => {
                    self.tag = Tag::Ins;
                    self.ins_content.push_str(content);
                    self.range.start = pos;
                    self.range.end = pos + count_chars(content);
                }
                Del(start, end) => {
                    self.tag = Tag::Del;
                    debug_assert!(self.ins_content.is_empty());
                    self.range = start..end;
                }
            }
            Ok(())
        } else {
            match (self.tag, op) {
                (Tag::Ins, Op::Ins(pos, content)) => {
                    if pos == self.range.end {
                        // We can merge this.
                        self.ins_content.push_str(content);
                        self.range.end += count_chars(content);
                        Ok(())
                    } else { Err(()) }
                }
                (Tag::Del, Op::Del(start, end)) => {
                    // We can merge if our delete is inside the operation.
                    // let self_len = self.range.len();
                    if start <= self.range.start && end >= self.range.start {
                        // dbg!(&self.range, (start, end));
                        self.range.end += end - self.range.start;
                        self.range.start = start;
                        Ok(())
                    } else { Err(()) }
                }
                (_, _) => Err(()),
            }
        }
    }
}

impl JumpRopeBuf {
    pub fn new() -> Self {
        Self(RefCell::new((JumpRope::new(), BufferedOp::new())))
    }

    fn flush_mut(inner: &mut (JumpRope, BufferedOp)) {
        if !inner.1.is_empty() {
            match inner.1.tag {
                Tag::Ins => {
                    inner.0.insert(inner.1.range.start, &inner.1.ins_content);
                },
                Tag::Del => {
                    inner.0.remove(inner.1.range.clone());
                }
            }
            inner.1.clear();
        }
    }

    // fn flush(&self) {
    //     let mut inner = self.0.borrow_mut();
    //     Self::flush_mut(inner.deref_mut());
    // }

    fn internal_push_op(&mut self, op: Op) {
        // let mut inner = self.0.borrow_mut();
        let inner = self.0.get_mut();
        match inner.1.try_append(op) {
            Ok(_) => {}
            Err(_) => {
                // Self::flush_mut(inner.deref_mut());
                Self::flush_mut(inner);
                // inner.0.insert(pos, content);
                inner.1.try_append(op).unwrap();
            }
        }

    }

    pub fn insert(&mut self, pos: usize, content: &str) {
        self.internal_push_op(Op::Ins(pos, content))
    }

    pub fn remove(&mut self, range: Range<usize>) {
        self.internal_push_op(Op::Del(range.start, range.end))
    }

    pub fn len_chars(&self) -> usize {
        let borrow = self.0.borrow();
        match borrow.1.tag {
            Tag::Ins => borrow.0.len_chars() + borrow.1.range.len(),
            Tag::Del => borrow.0.len_chars() - borrow.1.range.len()
        }
    }

    pub fn len_bytes(&self) -> usize {
        let mut borrow = self.0.borrow_mut();
        match borrow.1.tag {
            Tag::Ins => borrow.0.len_bytes() + borrow.1.ins_content.len(),
            Tag::Del => {
                // Unfortunately we have to flush to calculate byte length.
                Self::flush_mut(borrow.deref_mut());
                borrow.0.len_bytes()
            }
        }
    }

    pub fn into_inner(self) -> JumpRope {
        let mut contents = self.0.into_inner();
        Self::flush_mut(&mut contents);
        contents.0
    }
}