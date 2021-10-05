// This is an implementation of a Rope (fancy string) based on a skip list. This
// implementation is a rust port of librope:
// https://github.com/josephg/librope
// It does not support wide characters.

// Unlike other rust rope implementations, this implementation should be very
// fast; but it manages that through heavy use of unsafe pointers and C-style
// dynamic arrays.

// use rope::*;

use std::{mem, ptr, str};
use std::alloc::{alloc, dealloc, Layout};
use std::cmp::min;
use std::ops::Range;
use rand::prelude::*;
use crate::gapbuffer::GapBuffer;
use crate::utils::*;

// Must be <= UINT16_MAX. Benchmarking says this is pretty close to optimal
// (tested on a mac using clang 4.0 and x86_64).
//const NODE_SIZE: usize = 136;

// The likelyhood (out of 256) a node will have height (n+1) instead of n
const BIAS: u8 = 65;

// The rope will become less efficient after the string is 2 ^ ROPE_MAX_HEIGHT nodes.

#[cfg(debug_assertions)]
const NODE_STR_SIZE: usize = 10;
#[cfg(not(debug_assertions))]
const NODE_STR_SIZE: usize = 380;

const MAX_HEIGHT: usize = 20;//NODE_STR_SIZE / mem::size_of::<SkipEntry>();
const MAX_HEIGHT_U8: u8 = MAX_HEIGHT as u8;

// The node structure is designed in a very fancy way which would be more at home in C or something
// like that. The basic idea is that the node structure is fixed size in memory, but the proportion
// of that space taken up by characters and by the height are different depentant on a node's
// height.

#[repr(C)]
pub struct JumpRope {
    rng: SmallRng,
    // The total number of characters in the rope
    // num_chars: usize,

    // The total number of bytes which the characters in the rope take up
    num_bytes: usize,

    // The first node is inline. The height is the max height we've ever used in the rope + 1. The
    // highest entry points "past the end" of the list, including the entire list length.
    pub(super) head: Node,

    // This is so dirty. The first node is embedded in JumpRope; but we need to allocate enough room
    // for height to get arbitrarily large. I could insist on JumpRope always getting allocated on
    // the heap, but for small strings its better that the first string is just on the stack. So
    // this struct is repr(C) and I'm just padding out the struct directly.
    nexts: [SkipEntry; MAX_HEIGHT+1],

    // The nexts array contains an extra entry at [head.height-1] the which points past the skip
    // list. The size is the size of the entire list.
}

#[repr(C)] // Prevent parameter reordering.
pub(super) struct Node {
    // The first num_bytes of this store a valid utf8 string.
    // str: [u8; NODE_STR_SIZE],
    //
    // // Number of bytes in str in use
    // num_bytes: u8,
    pub(super) str: GapBuffer<NODE_STR_SIZE>,

    // Height of nexts array.
    height: u8,

    // #[repr(align(std::align_of::<SkipEntry>()))]

    // This array actually has the size of height; but we dynamically allocate the structure on the
    // heap to avoid wasting memory.
    // TODO: Honestly this memory saving is very small anyway. Reconsider this choice.
    nexts: [SkipEntry; 0],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct SkipEntry {
    pub(super) node: *mut Node,
    /// The number of *characters* between the start of the current node and the start of the next
    /// node.
    pub(super) skip_chars: usize,
}

// Make sure nexts uses correct alignment. This should be guaranteed by repr(C)
// This test will fail if this ever stops being true.
#[test]
fn test_align() {
    #[repr(C)] struct Check([SkipEntry; 0]);
    assert!(mem::align_of::<Check>() >= mem::align_of::<SkipEntry>());
}

fn random_height(rng: &mut SmallRng) -> u8 {
    let mut h: u8 = 1;
    // TODO: This is using the thread_local rng, which is secure (?!). Check
    // this is actually fast.
    while h < MAX_HEIGHT_U8 && rng.gen::<u8>() < BIAS { h+=1; }
    h
}


impl SkipEntry {
    fn new() -> Self {
        SkipEntry { node: ptr::null_mut(), skip_chars: 0 }
    }
}

impl Node {
    pub(super) fn next_ptr(&self) -> *const Self { // TODO: Pin.
        self.first_next().node
    }

    // Do I need to be explicit about the lifetime of the references being tied
    // to the lifetime of the node?
    fn nexts(&self) -> &[SkipEntry] {
        unsafe {
            std::slice::from_raw_parts(self.nexts.as_ptr(), self.height as usize)
        }
    }

    fn nexts_mut(&mut self) -> &mut [SkipEntry] {
        unsafe {
            std::slice::from_raw_parts_mut(self.nexts.as_mut_ptr(), self.height as usize)
        }
    }

    fn layout_with_height(height: u8) -> Layout {
        Layout::from_size_align(
            mem::size_of::<Node>() + mem::size_of::<SkipEntry>() * (height as usize),
            mem::align_of::<Node>()).unwrap()
    }

    fn alloc_with_height(height: u8, content: &str) -> *mut Node {
        //println!("height {} {}", height, max_height());
        assert!(height >= 1 && height <= MAX_HEIGHT_U8);

        unsafe {
            let node = alloc(Self::layout_with_height(height)) as *mut Node;
            (*node) = Node {
                str: GapBuffer::new_from_str(content),
                height,
                nexts: [],
            };

            for next in (*node).nexts_mut() {
                *next = SkipEntry::new();
            }

            node
        }
    }

    fn alloc(rng: &mut SmallRng, content: &str) -> *mut Node {
        Self::alloc_with_height(random_height(rng), content)
    }

    unsafe fn free(p: *mut Node) {
        dealloc(p as *mut u8, Self::layout_with_height((*p).height));
    }

    fn as_str_1(&self) -> &str {
        self.str.start_as_str()
    }
    fn as_str_2(&self) -> &str {
        self.str.end_as_str()
    }

    // The height is at least 1, so this is always valid.
    pub(super) fn first_next<'a>(&self) -> &'a SkipEntry {
        unsafe { &*self.nexts.as_ptr() }
    }

    fn first_next_mut<'a>(&mut self) -> &'a mut SkipEntry {
        unsafe { &mut *self.nexts.as_mut_ptr() }
    }

    fn num_chars(&self) -> usize {
        self.first_next().skip_chars
    }

    // fn mut_next<'a>(&mut self, i: usize) -> &'a mut SkipEntry {
    //     assert!(i < self.height);
    //     unsafe { &mut *self.nexts.as_mut_ptr() }
    // }
}

#[derive(Debug, Clone)]
struct RopeCursor([SkipEntry; MAX_HEIGHT+1]);

impl RopeCursor {
    fn update_offsets(&mut self, height: usize, by: isize) {
        for i in 0..height {
            unsafe {
                // This is weird but makes sense when you realise the nexts in
                // the cursor are pointers into the elements that have the
                // actual pointers.
                // Also adding a usize + isize is awful in rust :/
                let skip = &mut (*self.0[i].node).nexts_mut()[i].skip_chars;
                *skip = skip.wrapping_add(by as usize);
            }
        }
    }

    fn move_within_node(&mut self, height: usize, by: isize) {
        for e in &mut self.0[..height] {
            e.skip_chars = e.skip_chars.wrapping_add(by as usize);
        }
    }

    fn here_ptr(&self) -> *mut Node {
        self.0[0].node
    }

    fn global_char_pos(&self, head_height: u8) -> usize {
        self.0[head_height as usize - 1].skip_chars
    }

    fn local_char_pos(&self) -> usize {
        self.0[0].skip_chars
    }
}


impl JumpRope {
    pub fn new() -> Self {
        JumpRope {
            rng: SmallRng::seed_from_u64(123),
            // rng: if cfg!(debug_assertions) { SmallRng::seed_from_u64(123)
            // } else {
            //     // SmallRng::from_entropy()
            //     SmallRng::from_rng(thread_rng()).unwrap()
            // },
            num_bytes: 0,
            // nexts: [SkipEntry::new(); MAX_HEIGHT],

            // We don't ever store characters in the head node, but the height
            // here is the maximum height of the entire rope.
            head: Node {
                str: GapBuffer::new(),
                height: 1,
                nexts: [],
            },
            nexts: [SkipEntry::new(); MAX_HEIGHT+1],
        }
    }

    fn new_from_str(s: &str) -> Self {
        let mut rope = Self::new();
        rope.insert_at(0, s);
        rope
    }

    pub fn len_chars(&self) -> usize {
        self.head.nexts()[self.head.height as usize - 1].skip_chars
    }

    // Internal function for navigating to a particular character offset in the rope.  The function
    // returns the list of nodes which point past the position, as well as offsets of how far into
    // their character lists the specified characters are.
    fn cursor_at_char(&self, char_pos: usize) -> RopeCursor {
        assert!(char_pos <= self.len_chars());

        let mut e: *const Node = &self.head;
        let mut height = self.head.height as usize - 1;
        
        let mut offset = char_pos; // How many more chars to skip

        let mut iter = RopeCursor([SkipEntry::new(); MAX_HEIGHT+1]);

        loop { // while height >= 0
            let en = unsafe { &*e };
            let next = en.nexts()[height];
            let skip = next.skip_chars;
            if offset > skip {
                // Go right.
                assert!(e == &self.head || !en.str.is_empty());
                offset -= skip;
                e = next.node;
                assert!(!e.is_null(), "Internal constraint violation: Reached rope end prematurely");
            } else {
                // Record this and go down.
                iter.0[height] = SkipEntry {
                    skip_chars: offset,
                    node: e as *mut Node, // This is pretty gross
                };

                if height == 0 { break; } else { height -= 1; }
            }
        };

        assert!(offset <= NODE_STR_SIZE);
        iter
    }

    fn cursor_at_start(&self) -> RopeCursor {
        RopeCursor([SkipEntry {
            node: &self.head as *const _ as *mut _,
            skip_chars: 0
        }; MAX_HEIGHT+1])
    }

    fn cursor_at_end(&self) -> RopeCursor {
        self.cursor_at_char(self.len_chars())
    }

    // Internal fn to create a new node at the specified iterator filled with the specified
    // content.
    unsafe fn insert_node_at(&mut self, cursor: &mut RopeCursor, contents: &str, num_chars: usize, update_cursor: bool) {
        // println!("Insert_node_at {} len {}", contents.len(), self.num_bytes);
        // assert!(contents.len() < NODE_STR_SIZE);
        debug_assert_eq!(count_chars(contents), num_chars);
        debug_assert!(num_chars <= NODE_STR_SIZE);

        // TODO: Pin this sucka.
        // let new_node = Pin::new(Node::alloc());
        let new_node = Node::alloc(&mut self.rng, contents);
        // (*new_node).num_bytes = contents.len() as u8;
        // (*new_node).str[..contents.len()].copy_from_slice(contents.as_bytes());

        let new_height = (*new_node).height as usize;

        let mut head_height = self.head.height as usize;
        while head_height <= new_height {
            // TODO: Why do we copy here? Explain it in a comment. This is
            // currently lifted from the C code.
            self.nexts[head_height] = self.nexts[head_height - 1];
            cursor.0[head_height] = cursor.0[head_height - 1];

            self.head.height += 1; // Ends up 1 more than the max node height.
            head_height += 1;
        }

        for i in 0..new_height {
            let prev_skip = &mut (*cursor.0[i].node).nexts_mut()[i];
            let nexts = (*new_node).nexts_mut();
            nexts[i].node = prev_skip.node;
            nexts[i].skip_chars = num_chars + prev_skip.skip_chars - cursor.0[i].skip_chars;

            prev_skip.node = new_node;
            prev_skip.skip_chars = cursor.0[i].skip_chars;

            // & move the iterator to the end of the newly inserted node.
            if update_cursor {
                cursor.0[i].node = new_node;
                cursor.0[i].skip_chars = num_chars;
            }
        }

        for i in new_height..head_height {
            (*cursor.0[i].node).nexts_mut()[i].skip_chars += num_chars;
            if update_cursor {
                cursor.0[i].skip_chars += num_chars;
            }
        }

        // self.nexts[self.head.height as usize - 1].skip_chars += num_chars;
        self.num_bytes += contents.len();
    }

    unsafe fn insert_at_cursor(&mut self, cursor: &mut RopeCursor, contents: &str) {
        if contents.is_empty() { return; }
        // iter contains how far (in characters) into the current element to
        // skip. Figure out how much that is in bytes.
        let mut offset_bytes: usize = 0;
        // The insertion offset into the destination node.
        let offset: usize = cursor.0[0].skip_chars;
        let mut e = cursor.here_ptr();

        // We might be able to insert the new data into the current node, depending on
        // how big it is. We'll count the bytes, and also check that its valid utf8.
        let num_inserted_bytes = contents.len();
        let num_inserted_chars = count_chars(contents);

        // Adding this short curcuit makes the code about 2% faster for 1% more code
        if (*e).str.gap_start_chars as usize == offset && (*e).str.gap_len as usize >= num_inserted_bytes {
            // Short circuit. If we can just insert all the content right here in the gap, do so.
            (*e).str.insert_in_gap(contents);
            cursor.update_offsets(self.head.height as usize, num_inserted_chars as isize);
            cursor.move_within_node(self.head.height as usize, num_inserted_chars as isize);
            self.num_bytes += num_inserted_bytes;
            return;
        }

        if offset > 0 {
            assert!(offset <= (*e).nexts()[0].skip_chars);
            // This could be faster, but its not a big deal.
            // let s = (*e).as_str();
            // offset_bytes = str_get_byte_offset(s, offset);
            offset_bytes = (*e).str.count_bytes(offset);

            // println!("Offset {} offset_bytes {} s {:?}", offset, offset_bytes, s);
            // let v: Vec<(usize, char)> = s.char_indices().collect();
            // println!("{:?}", v);
        }

        // Can we insert into the current node?
        let current_len_bytes = (*e).str.len_bytes();
        let mut insert_here = current_len_bytes + num_inserted_bytes <= NODE_STR_SIZE;

        // Can we insert into the subsequent node? Check if we're inserting at the end...
        if !insert_here && offset_bytes == current_len_bytes {
            // We can insert into the subsequent node if:
            // - We can't insert into the current node
            // - There _is_ a next node to insert into
            // - The insert would be at the start of the next node
            // - There's room in the next node
            if let Some(next) = (*e).first_next_mut().node.as_mut() {
                if next.str.len_bytes() + num_inserted_bytes <= NODE_STR_SIZE {
                    offset_bytes = 0;
                    // TODO: Try this on:
                    // for e in &mut cursor.0[..next.height as usize] {
                    //     e.node = next;

                    for i in 0..next.height {
                        cursor.0[i as usize].node = next;
                        cursor.0[i as usize].skip_chars = 0;
                    }
                    e = next;

                    insert_here = true;
                }
            }
        }

        if insert_here {
            // println!("insert_here {}", contents);
            // First move the current bytes later on in the string.
            let c = &mut (*e).str;
            c.try_insert(offset_bytes, contents).unwrap();

            self.num_bytes += num_inserted_bytes;
            // .... aaaand update all the offset amounts.
            cursor.update_offsets(self.head.height as usize, num_inserted_chars as isize);
            cursor.move_within_node(self.head.height as usize, num_inserted_chars as isize);
        } else {
            // There isn't room. We'll need to add at least one new node to the rope.

            // If we're not at the end of the current node, we'll need to remove
            // the end of the current node's data and reinsert it later.
            (*e).str.move_gap(offset_bytes);
            // let trailing_data = (*e).str.end_as_str();

            let num_end_bytes = (*e).str.len_bytes() - offset_bytes;
            let mut num_end_chars: usize = 0;
            let end_str = if num_end_bytes > 0 {
                // We'll truncate the node, but leave the bytes themselves there (for later).

                // It would also be correct (and slightly more space efficient) to pack some of the
                // new string's characters into this node after trimming it.
                // let end_str = &(*e).as_str()[offset_bytes..];
                let end_str = (*e).str.take_rest();
                // (*e).num_bytes = offset_bytes as u8;
                num_end_chars = (*e).num_chars() - offset;

                cursor.update_offsets(self.head.height as usize, -(num_end_chars as isize));
                self.num_bytes -= num_end_bytes;
                Some(end_str)
            } else {
                // TODO: Don't just skip. Append as many characters as we can here.
                None
            };

            // Now we insert new nodes containing the new character data. The
            // data must be broken into pieces of with a maximum size of
            // NODE_STR_SIZE. Node boundaries must not occur in the middle of a
            // utf8 codepoint.
            // let mut str_offset: usize = 0;
            let mut remainder = contents;
            while !remainder.is_empty() {
                // println!(". {}", remainder);
                // Find the first index after STR_SIZE bytes
                let mut byte_pos = 0;
                let mut char_pos = 0;

                // Find a suitable cut point. We should take as many characters as we can fit in
                // the node, without splitting any unicode codepoints.
                for c in remainder.chars() {
                    // TODO: This could definitely be more efficient.
                    let cs = c.len_utf8();
                    if cs + byte_pos > NODE_STR_SIZE { break }
                    else {
                        char_pos += 1;
                        byte_pos += cs;
                    }
                }
                
                let (next, rem) = remainder.split_at(byte_pos);
                assert!(!next.is_empty());
                self.insert_node_at(cursor, next, char_pos, true);
                remainder = rem;
            }

            if let Some(end_str) = end_str {
                self.insert_node_at(cursor, end_str, num_end_chars, false);
            }
        }

        assert_ne!(cursor.local_char_pos(), 0);
    }

    unsafe fn del_at_cursor(&mut self, cursor: &mut RopeCursor, mut length: usize) {
        if length == 0 { return; }
        let mut offset = cursor.local_char_pos();
        let mut node = cursor.here_ptr();
        while length > 0 {
            {
                let s = (&*node).first_next();
                if offset == s.skip_chars {
                    // End of current node. Skip to the start of the next one.
                    node = s.node;
                    offset = 0;
                }
            }

            let num_chars = (&*node).num_chars();
            let removed = std::cmp::min(length, num_chars - offset);
            assert!(removed > 0);

            let height = (*node).height as usize;
            if removed < num_chars || std::ptr::eq(node, &self.head) {
                // Just trim the node down.
                let s = &mut (*node).str;
                let removed_bytes = s.remove_chars(offset, removed);
                self.num_bytes -= removed_bytes;

                for s in (*node).nexts_mut() {
                    s.skip_chars -= removed;
                }
            } else {
                // Remove the node from the skip list.
                for i in 0..(*node).height as usize {
                    let s = &mut (*cursor.0[i].node).nexts_mut()[i];
                    s.node = (*node).nexts_mut()[i].node;
                    s.skip_chars += (*node).nexts()[i].skip_chars - removed;
                }

                self.num_bytes -= (*node).str.len_bytes();
                let next = (*node).first_next().node;
                Node::free(node);
                node = next;
            }

            for i in height..self.head.height as usize {
                let s = &mut (*cursor.0[i].node).nexts_mut()[i];
                s.skip_chars -= removed;
            }

            length -= removed;
        }
    }
}

impl Default for JumpRope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for JumpRope {
    fn drop(&mut self) {
        let mut node = self.head.first_next().node;
        unsafe {
            while !node.is_null() {
                let next = (*node).first_next().node;
                Node::free(node);
                node = next;
            }
        }
    }
}

impl<'a> From<&'a str> for JumpRope {
    fn from(str: &str) -> Self {
        JumpRope::new_from_str(str)
    }
}

impl From<String> for JumpRope {
    fn from(str: String) -> Self {
        JumpRope::new_from_str(&str)
    }
}

impl PartialEq for JumpRope {
    // This is quite complicated. It would be cleaner to just write a bytes
    // iterator, then iterate over the bytes of both strings comparing along the
    // way.
    // However, this should be faster because it can memcmp().

    // Another way to implement this would be to rewrite it as a comparison with
    // an iterator over &str. Then the rope vs rope comparison would be trivial,
    // but also we could add comparison functions with a single &str and stuff
    // very easily.
    fn eq(&self, other: &JumpRope) -> bool {
        if self.num_bytes != other.num_bytes
                || self.len_chars() != other.len_chars() {
            return false
        }

        let mut other_iter = other.content_iter();

        // let mut os = other_iter.next();
        let mut os = "";

        for mut s in self.content_iter() {
            // Walk s.len() bytes through the other rope
            while !s.is_empty() {
                if os.is_empty() {
                    os = other_iter.next().unwrap();
                }
                debug_assert!(!os.is_empty());

                let amt = min(s.len(), os.len());
                debug_assert!(amt > 0);

                let (s_start, s_rem) = s.split_at(amt);
                let (os_start, os_rem) = os.split_at(amt);

                if s_start != os_start { return false; }

                s = s_rem;
                os = os_rem;
            }
        }

        true
    }
}
impl Eq for JumpRope {}

impl ToString for JumpRope {
    fn to_string(&self) -> String {
        let mut content = String::with_capacity(self.num_bytes);

        for node in self.node_iter() {
            content.push_str(node.as_str_1());
            content.push_str(node.as_str_2());
        }

        content
    }
}

impl<'a> Extend<&'a str> for JumpRope {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        let mut cursor = self.cursor_at_end();
        iter.into_iter().for_each(|s| {
            unsafe { self.insert_at_cursor(&mut cursor, s); }
        });
    }
}

impl Clone for JumpRope {
    fn clone(&self) -> Self {
        // This method could be a little bit more efficient, but I think improving clone()
        // performance isn't worth the extra effort.
        let mut r = JumpRope::new();
        let mut cursor = r.cursor_at_start();
        for node in self.node_iter() {
            unsafe {
                r.insert_at_cursor(&mut cursor, node.as_str_1());
                r.insert_at_cursor(&mut cursor, node.as_str_2());
            }
        }
        r
    }
}

impl JumpRope {
    pub fn insert_at(&mut self, mut pos: usize, contents: &str) {
        if contents.is_empty() { return; }
        pos = std::cmp::min(pos, self.len_chars());

        let mut cursor = self.cursor_at_char(pos);
        unsafe { self.insert_at_cursor(&mut cursor, contents); }

        debug_assert_eq!(cursor.global_char_pos(self.head.height), pos + count_chars(contents));
        // dbg!(&cursor.0[..self.head.height as usize]);
    }

    pub fn del_at(&mut self, pos: usize, length: usize) {
        let length = usize::min(length, self.len_chars() - pos);
        if length == 0 { return; }

        let mut cursor = self.cursor_at_char(pos);
        unsafe { self.del_at_cursor(&mut cursor, length); }

        debug_assert_eq!(cursor.global_char_pos(self.head.height), pos);
    }

    pub fn replace(&mut self, range: Range<usize>, content: &str) {
        let len = self.len_chars();
        let pos = usize::min(range.start, len);
        let del_len = usize::min(range.end, len) - pos;

        let mut cursor = self.cursor_at_char(pos);
        if del_len > 0 {
            unsafe { self.del_at_cursor(&mut cursor, del_len); }
        }
        if !content.is_empty() {
            unsafe { self.insert_at_cursor(&mut cursor, content); }
        }

        debug_assert_eq!(cursor.global_char_pos(self.head.height), pos + count_chars(content));
    }

    pub fn len(&self) -> usize { self.num_bytes }
    pub fn is_empty(&self) -> bool { self.num_bytes == 0 }

    pub fn check(&self) {
        // #[cfg(test)]
        {
            assert!(self.head.height >= 1);
            assert!(self.head.height < MAX_HEIGHT_U8 + 1);

            let skip_over = &self.nexts[self.head.height as usize - 1];
            // println!("Skip over skip chars {}, num bytes {}", skip_over.skip_chars, self.num_bytes);
            assert!(skip_over.skip_chars <= self.num_bytes as usize);
            assert!(skip_over.node.is_null());

            // The offsets store the total distance travelled since the start.
            let mut iter = [SkipEntry::new(); MAX_HEIGHT];
            for i in 0..self.head.height {
                // Bleh.
                iter[i as usize].node = &self.head as *const Node as *mut Node;
            }

            let mut num_bytes: usize = 0;
            let mut num_chars = 0;

            for n in self.node_iter() {
                // println!("visiting {:?}", n.as_str());
                assert!(!n.str.is_empty() || std::ptr::eq(n, &self.head));
                assert!(n.height <= MAX_HEIGHT_U8);
                assert!(n.height >= 1);
                n.str.check();

                assert_eq!(count_chars(n.as_str_1()) + count_chars(n.as_str_2()), n.num_chars());
                for (i, entry) in iter[0..n.height as usize].iter_mut().enumerate() {
                    assert_eq!(entry.node as *const Node, n as *const Node);
                    assert_eq!(entry.skip_chars, num_chars);

                    // println!("replacing entry {:?} with {:?}", entry, n.nexts()[i].node);
                    entry.node = n.nexts()[i].node;
                    entry.skip_chars += n.nexts()[i].skip_chars;
                }

                num_bytes += n.str.len_bytes();
                num_chars += n.num_chars();
            }

            for entry in iter[0..self.head.height as usize].iter() {
                // println!("{:?}", entry);
                assert!(entry.node.is_null());
                assert_eq!(entry.skip_chars, num_chars);
            }
            
            // println!("self bytes: {}, count bytes {}", self.num_bytes, num_bytes);
            assert_eq!(self.num_bytes, num_bytes);
            assert_eq!(self.len_chars(), num_chars);
        }
    }

    #[allow(unused)]
    pub(crate) fn print(&self) {
        println!("chars: {}\tbytes: {}\theight: {}", self.len_chars(), self.num_bytes, self.head.height);

        print!("HEAD:");
        for s in self.head.nexts() {
            print!(" |{} ", s.skip_chars);
        }
        println!();

        for (i, node) in self.node_iter().enumerate() {
            print!("{}:", i);
            for s in node.nexts() {
                print!(" |{} ", s.skip_chars);
            }
            println!("      : {:?} + {:?}", node.as_str_1(), node.as_str_2());
        }
    }
}
