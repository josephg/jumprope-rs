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

// Must be <= UINT16_MAX. Benchmarking says this is pretty close to optimal
// (tested on a mac using clang 4.0 and x86_64).
//const NODE_SIZE: usize = 136;

// The likelyhood (out of 256) a node will have height (n+1) instead of n
const BIAS: u8 = 100;

// The rope will become less efficient after the string is 2 ^ ROPE_MAX_HEIGHT nodes.
const NODE_STR_SIZE: usize = 100;
const MAX_HEIGHT: usize = 20;//NODE_STR_SIZE / mem::size_of::<SkipEntry>();
const MAX_HEIGHT_U8: u8 = MAX_HEIGHT as u8;

#[derive(Copy, Clone, Debug)]
struct SkipEntry {
    // The number of *characters* between the start of the current node and the
    // start of the next node.
    node: *mut Node,
    skip_chars: usize,
}

// The node structure is designed in a very fancy way which would be more at home in C or something
// like that. The basic idea is that the node structure is fixed size in memory, but the proportion
// of that space taken up by characters and by the height are different depentant on a node's
// height.

#[repr(C)] // Prevent parameter reordering.
struct Node {
    // The first num_bytes of this store a valid utf8 string.
    str: [u8; NODE_STR_SIZE],

    // Number of bytes in str in use
    num_bytes: u8,

    // Height of nexts array.
    height: u8,

    // #[repr(align(std::align_of::<SkipEntry>()))]
    
    // This array actually has the size of height. It would be cleaner to
    // declare it as [SkipEntry; 0], but I haven't done that because we always
    // have at least a height of 1 anyway, and this makes it a bit cheaper to
    // look at the first skipentry item.
    nexts: [SkipEntry; 0],
}

// Make sure nexts uses correct alignment. This should be guaranteed by repr(C)
// This test will fail if this ever stops being true.
#[test]
fn test_align() {
    #[repr(C)] struct Check([SkipEntry; 0]);
    assert!(mem::align_of::<Check>() >= mem::align_of::<SkipEntry>());
}

fn random_height() -> u8 {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let mut h: u8 = 1;
    while h < MAX_HEIGHT_U8 && rng.gen::<u8>() < BIAS { h+=1; }
    h
}

#[repr(C)]
pub struct JumpRope {
    // The total number of characters in the rope
    // num_chars: usize,

    // The total number of bytes which the characters in the rope take up
    num_bytes: usize,

    // The first node is inline. The height is the max height we've ever used in
    // the rope.
    head: Node,

    // This is so dirty. The first node is embedded in JumpRope; but we need to
    // allocate enough room for height to get arbitrarily large. I could insist
    // on JumpRope always getting allocated on the heap, but for small strings
    // its better that the first string is just on the stack.
    // So this struct is repr(C) and I'm just padding out the struct directly.
    nexts: [SkipEntry; MAX_HEIGHT+1],

    // The nexts array contains an extra entry at [head.height-1] the which
    // points past the skip list. The size is the size of the entire list.
}


impl SkipEntry {
    fn new() -> Self {
        SkipEntry { node: ptr::null_mut(), skip_chars: 0 }
    }
}

impl Node {
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

    fn alloc_with_height(height: u8) -> *mut Node {
        //println!("height {} {}", height, max_height());
        assert!(height >= 1 && height <= MAX_HEIGHT_U8);

        unsafe {
            let node = alloc(Self::layout_with_height(height)) as *mut Node;
            (*node) = Node {
                str: [0; NODE_STR_SIZE],
                num_bytes: 0,
                height: height,
                nexts: [],
            };

            for next in (*node).nexts_mut() {
                *next = SkipEntry::new();
            }

            node
        }
    }

    fn alloc() -> *mut Node {
        Self::alloc_with_height(random_height())
    }

    unsafe fn free(p: *mut Node) {
        dealloc(p as *mut u8, Self::layout_with_height((*p).height));
    }

    fn content_slice(&self) -> &[u8] {
        &self.str[..self.num_bytes as usize]
    }

    fn as_str(&self) -> &str {
        if cfg!(debug_assertions) {
            str::from_utf8(self.content_slice()).unwrap()
        } else {
            unsafe { str::from_utf8_unchecked(self.content_slice()) }
        }
    }

    // The height is at least 1, so this is always valid.
    fn first_next<'a>(&self) -> &'a SkipEntry {
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

struct NodeIter<'a>(Option<&'a Node>);
impl<'a> Iterator for NodeIter<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        let prev = self.0;
        if let Some(n) = self.0 {
            *self = NodeIter(unsafe { n.first_next().node.as_ref() });
        }
        prev
    }
}

struct RopeCursor ([SkipEntry; MAX_HEIGHT+1]);

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

    fn here_ptr(&self) -> *mut Node {
        self.0[0].node
    }
}

// Get the byte offset after char_pos utf8 characters
fn str_get_byte_offset(s: &str, char_pos: usize) -> usize {
    s.char_indices().nth(char_pos).map_or_else(
        || s.len(),
        |(i, _)| i
    )
}

impl JumpRope {
    pub fn new() -> Self {
        JumpRope {
            num_bytes: 0,
            // nexts: [SkipEntry::new(); MAX_HEIGHT],

            // We don't ever store characters in the head node, but the height
            // here is the maximum height of the entire rope.
            head: Node {
                str: [0; NODE_STR_SIZE],
                num_bytes: 0,
                height: 1,
                nexts: [],
            },
            nexts: [SkipEntry::new(); MAX_HEIGHT+1],
        }
    }

    // TODO: Add From trait.
    pub fn new_from_str(s: &str) -> Self {
        let mut rope = Self::new();
        rope.insert_at(0, s);
        rope
    }

    // fn head(&self) -> Option<&Node> {
    //     unsafe { self.head.nexts[0].next() }
    // }

    fn num_chars(&self) -> usize {
        self.head.nexts()[self.head.height as usize - 1].skip_chars
    }

    fn iter(&self) -> NodeIter { NodeIter(Some(&self.head)) }
    
    // Internal function for navigating to a particular character offset in the rope.  The function
    // returns the list of nodes which point past the position, as well as offsets of how far into
    // their character lists the specified characters are.
    fn iter_at_char(&self, char_pos: usize) -> RopeCursor {
        assert!(char_pos <= self.num_chars());

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
                assert!(e == &self.head || en.num_bytes > 0);
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

    // Internal fn to create a new node at the specified iterator filled with the specified
    // content.
    unsafe fn insert_node_at(&mut self, iter: &mut RopeCursor, contents: &str, num_chars: usize) {
        // println!("Insert_node_at {} len {}", contents.len(), self.num_bytes);
        // assert!(contents.len() < NODE_STR_SIZE);
        debug_assert_eq!(contents.chars().count(), num_chars);

        let new_node = Node::alloc();
        (*new_node).num_bytes = contents.len() as u8;
        (*new_node).str[..contents.len()].copy_from_slice(contents.as_bytes());
        let new_height = (*new_node).height;

        let mut head_height = self.head.height as usize;
        let new_height_usize = new_height as usize;
        while head_height <= new_height_usize {
            // TODO: Why do we copy here? Explain it in a comment. This is
            // currently lifted from the C code.
            self.nexts[head_height] = self.nexts[head_height - 1];
            iter.0[head_height] = iter.0[head_height - 1];

            self.head.height += 1;
            head_height += 1;
        }

        for i in 0..new_height_usize {
            let prev_skip = &mut (*iter.0[i].node).nexts_mut()[i];
            let nexts = (*new_node).nexts_mut();
            nexts[i].node = prev_skip.node;
            nexts[i].skip_chars = num_chars + prev_skip.skip_chars - iter.0[i].skip_chars;

            prev_skip.node = new_node;
            prev_skip.skip_chars = iter.0[i].skip_chars;

            // & move the iterator to the end of the newly inserted node.
            iter.0[i].node = new_node;
            iter.0[i].skip_chars = num_chars;
        }

        for i in new_height_usize..head_height {
            (*iter.0[i].node).nexts_mut()[i].skip_chars += num_chars;
            iter.0[i].skip_chars += num_chars;
        }

        // self.nexts[self.head.height as usize - 1].skip_chars += num_chars;
        self.num_bytes += contents.len();
    }

    unsafe fn insert_at_iter(&mut self, iter: &mut RopeCursor, contents: &str) {
        // iter contains how far (in characters) into the current element to
        // skip. Figure out how much that is in bytes.
        let mut offset_bytes: usize = 0;
        // The insertion offset into the destination node.
        let mut offset: usize = iter.0[0].skip_chars;
        let mut e = iter.here_ptr();
        if offset > 0 {
            assert!(offset <= (*e).nexts()[0].skip_chars);
            // This could be faster, but its not a big deal.
            let s = (*e).as_str();
            offset_bytes = str_get_byte_offset(s, offset);

            // println!("Offset {} offset_bytes {} s {:?}", offset, offset_bytes, s);
            // let v: Vec<(usize, char)> = s.char_indices().collect();
            // println!("{:?}", v);
        }

        // We might be able to insert the new data into the current node, depending on
        // how big it is. We'll count the bytes, and also check that its valid utf8.
        let num_inserted_bytes = contents.len();
        let num_inserted_chars = contents.chars().count();

        // Can we insert into the current node?
        let mut insert_here = (*e).num_bytes as usize + num_inserted_bytes <= NODE_STR_SIZE;

        // Can we insert into the subsequent node?
        if !insert_here && offset_bytes == (*e).num_bytes as usize {
            // We can insert into the subsequent node if:
            // - We can't insert into the current node
            // - There _is_ a next node to insert into
            // - The insert would be at the start of the next node
            // - There's room in the next node
            if let Some(next) = (*e).first_next_mut().node.as_mut() {
                if next.num_bytes as usize + num_inserted_bytes <= NODE_STR_SIZE {
                    offset = 0; offset_bytes = 0;
                    for i in 0..next.height {
                        iter.0[i as usize].node = next;
                        // tree offset nodes will not be used.
                    }
                    e = next;

                    insert_here = true;
                }
            }
        }

        if insert_here {
            // println!("insert_here {}", contents);
            // First move the current bytes later on in the string.
            // let c = (*e).content_mut();
            let c = &mut (*e).str;
            if offset_bytes < (*e).num_bytes as usize {
                ptr::copy(
                    &c[offset_bytes],
                    &mut c[offset_bytes + num_inserted_bytes],
                    (*e).num_bytes as usize - offset_bytes);
            }

            // Then copy in the string bytes
            ptr::copy_nonoverlapping(
                &contents.as_bytes()[0],
                &mut c[offset_bytes],
                num_inserted_bytes
            );

            (*e).num_bytes += num_inserted_bytes as u8;
            self.num_bytes += num_inserted_bytes;
            // self.num_chars += num_inserted_chars;

            // .... aaaand update all the offset amounts.
            iter.update_offsets(self.head.height as usize, num_inserted_chars as isize);
        } else {
            // There isn't room. We'll need to add at least one new node to the rope.

            // If we're not at the end of the current node, we'll need to remove
            // the end of the current node's data and reinsert it later.
            let num_end_bytes = (*e).num_bytes as usize - offset_bytes;
            let mut num_end_chars: usize = 0;
            let end_str = if num_end_bytes > 0 {
                // We'll pretend like the character have been deleted from the
                // node, while leaving the bytes themselves there (for later).

                // Note that if we wanted to, it would also be correct (and
                // slightly more space efficient) to pack some of the new
                // string's characters into this node after trimming it.
                let end_str = &(*e).as_str()[offset_bytes..];
                (*e).num_bytes = offset_bytes as u8;
                num_end_chars = (*e).num_chars() - offset;

                iter.update_offsets(self.head.height as usize, -(num_end_chars as isize));
                // self.num_chars -= num_end_chars;
                self.num_bytes -= num_end_bytes;
                Some(end_str)
            } else {
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

                for c in remainder.chars() {
                    let cs = c.len_utf8();
                    if cs + byte_pos > NODE_STR_SIZE { break }
                    else {
                        char_pos += 1;
                        byte_pos += cs;
                    }
                }
                
                let (next, rem) = remainder.split_at(byte_pos);
                assert!(!next.is_empty());
                self.insert_node_at(iter, next, char_pos);
                remainder = rem;
            }

            if let Some(end_str) = end_str {
                self.insert_node_at(iter, end_str, num_end_chars);
            }
        }
    }

    unsafe fn del_at_iter(&mut self, iter: &mut RopeCursor, mut length: usize) {
        let mut offset = iter.0[0].skip_chars;
        let mut e = iter.here_ptr();
        while length > 0 {
            {
                let s = (&*e).first_next();
                if offset == s.skip_chars {
                    // End of current node. Skip to the start of the next one.
                    e = s.node;
                    offset = 0;
                }
            }

            let num_chars = (&*e).num_chars();
            let removed = std::cmp::min(length, num_chars - offset);
            assert!(removed > 0);

            let height = (*e).height as usize;
            if removed < num_chars || e as *const Node == &self.head as *const Node {
                // Just trim the node down.
                let s = (*e).as_str();
                let leading_bytes = str_get_byte_offset(s, offset);
                let removed_bytes = str_get_byte_offset(&s[leading_bytes..], removed);
                let trailing_bytes = (*e).num_bytes as usize - leading_bytes - removed_bytes;

                let c = &mut (*e).str;
                if trailing_bytes > 0 {
                    ptr::copy(
                        &c[leading_bytes + removed_bytes],
                        &mut c[leading_bytes],
                        trailing_bytes);
                }

                (*e).num_bytes -= removed_bytes as u8;
                self.num_bytes -= removed_bytes;

                for s in (*e).nexts_mut() {
                    s.skip_chars -= removed;
                }
            } else {
                // Remove the node from the skip list.
                for i in 0..(*e).height as usize {
                    let s = &mut (*iter.0[i].node).nexts_mut()[i];
                    s.node = (*e).nexts_mut()[i].node;
                    s.skip_chars += (*e).nexts()[i].skip_chars - removed;
                }

                self.num_bytes -= (*e).num_bytes as usize;
                let next = (*e).first_next().node;
                Node::free(e);
                e = next;
            }

            for i in height..self.head.height as usize {
                let s = &mut (*iter.0[i].node).nexts_mut()[i];
                s.skip_chars -= removed;
            }

            length -= removed;
        }
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
                || self.num_chars() != other.num_chars() {
            return false
        }

        let mut other_iter = other.iter().map(|n| { n.as_str() });

        let mut os = other_iter.next();
        let mut opos: usize = 0; // Byte offset in os.
        for n in self.iter() {
            let s = n.as_str();
            let mut pos: usize = 0; // Current byte offset in s
            debug_assert_eq!(s.len(), n.num_bytes as usize);

            // Walk s.len() bytes through the other rope
            while pos < n.num_bytes as usize {
                if let Some(oss) = os {
                    let amt = min(s.len() - pos, oss.len() - opos);
                    // println!("iter slen {} pos {} osslen {} amt {}", s.len(), pos, oss.len(), amt);

                    if &s[pos..pos+amt] != &oss[opos..opos+amt] {
                        return false
                    }

                    pos += amt;
                    opos += amt;
                    debug_assert!(opos <= oss.len());

                    if opos == oss.len() {
                        os = other_iter.next();
                        opos = 0;
                    }
                } else {
                    panic!("Internal string length does not match");
                }
            }
        }

        true
    }
}
impl Eq for JumpRope {}


impl<'a> From<&'a str> for JumpRope {
    fn from(s: &str) -> JumpRope {
        JumpRope::new_from_str(s)
    }
}

impl From<String> for JumpRope {
    fn from(s: String) -> JumpRope {
        JumpRope::new_from_str(s.as_str())
    }
}

impl<'a> Into<String> for &'a JumpRope {
    fn into(self) -> String {
        let mut content = String::with_capacity(self.num_bytes);

        for node in self.iter() {
            content.push_str(node.as_str());
        }

        content
    }
}

impl Clone for JumpRope {
    fn clone(&self) -> Self {
        let mut r = JumpRope::new();
        r.num_bytes = self.num_bytes;
        let head_str = self.head.as_str();
        r.head.str[..head_str.len()].copy_from_slice(head_str.as_bytes());
        r.head.num_bytes = self.head.num_bytes;
        r.head.height = self.head.height;
        
        {
            // I could just edit the overflow memory directly, but this is safer
            // because of aliasing rules.
            let head_nexts = r.head.nexts_mut();
            for i in 0..self.head.height as usize {
                head_nexts[i].skip_chars = self.nexts[i].skip_chars;
            }
        }

        let mut nodes = [&mut r.head as *mut Node; MAX_HEIGHT];

        // The first node the iterator will return is the head. Ignore it.
        let mut iter = self.iter();
        iter.next();
        for other in iter {
            // This also sets height.
            let height = other.height;
            let node = Node::alloc_with_height(height);
            unsafe {
                (*node).num_bytes = other.num_bytes;
                let len = other.num_bytes as usize;
                (*node).str[..len].copy_from_slice(&other.str[..len]);

                let other_nexts = other.nexts();
                let nexts = (*node).nexts_mut();
                for i in 0..height as usize {
                    nexts[i].skip_chars = other_nexts[i].skip_chars;
                    (*nodes[i]).nexts_mut()[i].node = node;
                    nodes[i] = node;
                }
            }
        }

        r
    }
}

impl JumpRope {
    // fn new() -> Self {
    //     JumpRope::new()
    // }

    pub fn insert_at(&mut self, mut pos: usize, contents: &str) {
        if contents.len() == 0 { return; }
        
        pos = std::cmp::min(pos, self.num_chars());
        let mut cursor = self.iter_at_char(pos);
        unsafe { self.insert_at_iter(&mut cursor, contents); }
    }

    pub fn del_at(&mut self, pos: usize, length: usize) {
        let length = std::cmp::min(length, self.num_chars() - pos);
        if length == 0 { return; }

        let mut cursor = self.iter_at_char(pos);
        unsafe { self.del_at_iter(&mut cursor, length); }
    }

    // fn slice(&self, pos: usize, len: usize) -> Result<String, RopeError> {
    //        unimplemented!();
       // }

    pub fn len(&self) -> usize { self.num_bytes }
    pub fn char_len(&self) -> usize { self.num_chars() }
    pub fn to_string(&self) -> String { self.into() }

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

            for n in self.iter() {
                // println!("visiting {:?}", n.as_str());
                assert!((n as *const Node == &self.head as *const Node) || n.num_bytes > 0);
                assert!(n.height <= MAX_HEIGHT_U8);

                assert_eq!(n.as_str().chars().count(), n.num_chars());
                for (i, entry) in iter[0..n.height as usize].iter_mut().enumerate() {
                    assert_eq!(entry.node as *const Node, n as *const Node);
                    assert_eq!(entry.skip_chars, num_chars);

                    // println!("replacing entry {:?} with {:?}", entry, n.nexts()[i].node);
                    entry.node = n.nexts()[i].node;
                    entry.skip_chars += n.nexts()[i].skip_chars;
                }

                num_bytes += n.num_bytes as usize;
                num_chars += n.num_chars();
            }

            for entry in iter[0..self.head.height as usize].iter() {
                // println!("{:?}", entry);
                assert!(entry.node.is_null());
                assert_eq!(entry.skip_chars, num_chars);
            }
            
            // println!("self bytes: {}, count bytes {}", self.num_bytes, num_bytes);
            assert_eq!(self.num_bytes, num_bytes);
            assert_eq!(self.num_chars(), num_chars);
        }
    }

    // TODO: Don't export this.
    pub fn print(&self) {
        println!("chars: {}\tbytes: {}\theight: {}", self.num_chars(), self.num_bytes, self.head.height);

        print!("HEAD:");
        for s in self.head.nexts() {
            print!(" |{} ", s.skip_chars);
        }
        println!("");

        for (i, node) in self.iter().enumerate() {
            print!("{}:", i);
            for s in node.nexts() {
                print!(" |{} ", s.skip_chars);
            }
            println!("      : {:?}", node.as_str());
        }
    }
}
