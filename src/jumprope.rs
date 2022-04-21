// This is an implementation of a Rope (fancy string) based on a skip list. This
// implementation is a rust port of librope:
// https://github.com/josephg/librope
// It does not support wide characters.

// Unlike other rust rope implementations, this implementation should be very
// fast; but it manages that through heavy use of unsafe pointers and C-style
// dynamic arrays.

// use rope::*;

use std::{ptr, str};
// use std::alloc::{alloc, dealloc, Layout};
use std::cmp::min;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::Range;
use std::ptr::null_mut;
use rand::prelude::*;
use rand::Rng;
use crate::fast_str_tools::*;
use crate::gapbuffer::GapBuffer;
// use crate::utils::*;
// use crate::params::*;

// Must be <= UINT16_MAX. Benchmarking says this is pretty close to optimal
// (tested on a mac using clang 4.0 and x86_64).
//const NODE_SIZE: usize = 136;

// The likelyhood (out of 256) a node will have height (n+1) instead of n
const BIAS: u8 = 65;
// const BIAS: u8 = XX_BIAS;

// The rope will become less efficient after the string is 2 ^ ROPE_MAX_HEIGHT nodes.

#[cfg(debug_assertions)]
pub(crate) const NODE_STR_SIZE: usize = 10;
#[cfg(not(debug_assertions))]
pub(crate) const NODE_STR_SIZE: usize = 392;
// pub(crate) const NODE_STR_SIZE: usize = XX_SIZE;

const MAX_HEIGHT: usize = 20;//NODE_STR_SIZE / mem::size_of::<SkipEntry>();
const MAX_HEIGHT_U8: u8 = MAX_HEIGHT as u8;

// Using StdRng notably increases wasm code size, providing some tiny extra protection against
// ddos attacks. See main module documentation for details.
#[cfg(feature = "ddos_protection")]
type RopeRng = StdRng;
#[cfg(not(feature = "ddos_protection"))]
type RopeRng = SmallRng;


// The node structure is designed in a very fancy way which would be more at home in C or something
// like that. The basic idea is that the node structure is fixed size in memory, but the proportion
// of that space taken up by characters and by the height are different depentant on a node's
// height.
#[repr(C)]
pub struct JumpRope {
    rng: RopeRng,
    // The total number of characters in the rope
    // num_chars: usize,

    // The total number of bytes which the characters in the rope take up
    num_bytes: usize,

    // The first node is inline. The height is the max height we've ever used in the rope + 1. The
    // highest entry points "past the end" of the list, including the entire list length.
    // TODO: Get rid of this and just rely on nexts out of here.
    pub(super) head: Node,

    // This is so dirty. The first node is embedded in JumpRope; but we need to allocate enough room
    // for height to get arbitrarily large. I could insist on JumpRope always getting allocated on
    // the heap, but for small strings its better that the first string is just on the stack. So
    // this struct is repr(C) and I'm just padding out the struct directly.
    // nexts: [SkipEntry; MAX_HEIGHT+1],

    // The nexts array contains an extra entry at [head.height-1] the which points past the skip
    // list. The size is the size of the entire list.
}

#[derive(Debug)]
pub(super) struct MutCursor<'a> {
    inner: RopeCursor,

    // head_nexts: &'a mut [SkipEntry; MAX_HEIGHT+1],

    // head_height: &'a mut u8,
    rng: &'a mut RopeRng,
    num_bytes: &'a mut usize,

    phantom: PhantomData<&'a mut JumpRope>,
}

impl<'a> MutCursor<'a> {
    fn height(&self) -> usize {
        unsafe {
            (*self.inner.0[MAX_HEIGHT].node).height as usize
        }
    }

    fn set_height(&mut self, new_height: usize) {
        unsafe {
            (*self.inner.0[MAX_HEIGHT].node).height = new_height as u8
        }
    }

    fn is_head(&self, ptr: *const Node) -> bool {
        std::ptr::eq(ptr, self.inner.0[MAX_HEIGHT].node)
    }
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
    pub(super) height: u8,

    // #[repr(align(std::align_of::<SkipEntry>()))]

    // This array actually has the size of height; but we dynamically allocate the structure on the
    // heap to avoid wasting memory.
    // TODO: Honestly this memory saving is very small anyway. Reconsider this choice.
    // nexts: [SkipEntry; 0],
    nexts: [SkipEntry; MAX_HEIGHT+1],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct SkipEntry {
    pub(super) node: *mut Node,
    /// The number of *characters* between the start of the current node and the start of the next
    /// node.
    pub(super) skip_chars: usize,

    #[cfg(feature = "wchar_conversion")]
    pub(super) skip_pairs: usize,
}

impl Default for SkipEntry {
    fn default() -> Self {
        Self {
            node: null_mut(),
            skip_chars: 0,
            #[cfg(feature = "wchar_conversion")]
            skip_pairs: 0
        }
    }
}

// Make sure nexts uses correct alignment. This should be guaranteed by repr(C)
// This test will fail if this ever stops being true.
#[test]
fn test_align() {
    #[repr(C)] struct Check([SkipEntry; 0]);
    assert!(std::mem::align_of::<Check>() >= std::mem::align_of::<SkipEntry>());
}

fn random_height(rng: &mut RopeRng) -> u8 {
    let mut h: u8 = 1;
    // TODO: This is using the thread_local rng, which is secure (?!). Check
    // this is actually fast.
    while h < MAX_HEIGHT_U8 && rng.gen::<u8>() < BIAS { h+=1; }
    h
}


impl SkipEntry {
    fn new() -> Self {
        SkipEntry {
            node: ptr::null_mut(),
            skip_chars: 0,
            #[cfg(feature = "wchar_conversion")]
            skip_pairs: 0
        }
    }
}

impl Node {
    pub(super) fn next_ptr(&self) -> *const Self { // TODO: Pin.
        self.first_next().node
    }

    // Do I need to be explicit about the lifetime of the references being tied
    // to the lifetime of the node?
    fn nexts(&self) -> &[SkipEntry] {
        &self.nexts[..self.height as usize]
        // unsafe {
        //     std::slice::from_raw_parts(self.nexts.as_ptr(), self.height as usize)
        // }
    }

    fn nexts_mut(&mut self) -> &mut [SkipEntry] {
        &mut self.nexts[..self.height as usize]
        // unsafe {
        //     std::slice::from_raw_parts_mut(self.nexts.as_mut_ptr(), self.height as usize)
        // }
    }

    fn new_with_height(height: u8, content: &str) -> Self {
        Self {
            str: GapBuffer::new_from_str(content),
            height,
            nexts: [SkipEntry::default(); MAX_HEIGHT+1]
        }
    }

    // fn layout_with_height(height: u8) -> Layout {
    //     Layout::from_size_align(
    //         mem::size_of::<Node>() + mem::size_of::<SkipEntry>() * (height as usize),
    //         mem::align_of::<Node>()).unwrap()
    // }

    // fn alloc_with_height(height: u8, content: &str) -> *mut Node {
    //     //println!("height {} {}", height, max_height());
    //     #![allow(clippy::manual_range_contains)]
    //     assert!(height >= 1 && height <= MAX_HEIGHT_U8);
    //
    //     unsafe {
    //         let node = alloc(Self::layout_with_height(height)) as *mut Node;
    //         (*node) = Node {
    //             str: GapBuffer::new_from_str(content),
    //             height,
    //             nexts: [SkipEntry::default(); MAX_HEIGHT+1],
    //         };
    //
    //         for next in (*node).nexts_mut() {
    //             *next = SkipEntry::new();
    //         }
    //
    //         node
    //     }
    // }

    // fn alloc(rng: &mut RopeRng, content: &str) -> *mut Node {
    //     Self::alloc_with_height(random_height(rng), content)
    // }

    // fn new_random_height(rng: &mut RopeRng, content: &str) -> Node {
    //     Self::new_with_height(random_height(rng), content)
    // }

    // unsafe fn free(p: *mut Node) {
    //     dealloc(p as *mut u8, Self::layout_with_height((*p).height));
    // }

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

    pub(super) fn num_chars(&self) -> usize {
        self.first_next().skip_chars
    }

    #[cfg(feature = "wchar_conversion")]
    pub(super) fn num_surrogate_pairs(&self) -> usize {
        self.first_next().skip_pairs
    }
}

/// Cursors are a bit weird, and they deserve an explanation.
///
/// Cursors express the location that an edit will happen. But because this is a skip list, when
/// items are added or removed we need to not just splice in / remove elements, but also update:
///
/// - The next pointers of *previous* items
/// - The index item. Each next pointer in a node names how many items are being "skipped over" by
///   that pointer. Those "skipped over" counts need to be updated based on the change.
///
/// Anyway, to do all of this, a cursor names the item which *points to* the current location.
///
/// TODO: A cursor should store a PhantomData<Pin<&mut JumpRope>>.
#[derive(Debug, Clone)]
pub(crate) struct RopeCursor([SkipEntry; MAX_HEIGHT+1]);

// TODO: Move these methods to MutCursor.
impl RopeCursor {
    fn update_offsets(&mut self, height: usize, by_chars: isize, #[cfg(feature = "wchar_conversion")] by_pairs: isize) {
        for i in 0..height {
            unsafe {
                // This is weird but makes sense when you realise the nexts in
                // the cursor are pointers into the elements that have the
                // actual pointers.

                // Also adding a usize + isize is awful in rust :/
                let entry = &mut (*self.0[i].node).nexts_mut()[i];
                entry.skip_chars = entry.skip_chars.wrapping_add(by_chars as usize);
                #[cfg(feature = "wchar_conversion")] {
                    entry.skip_pairs = entry.skip_pairs.wrapping_add(by_pairs as usize);
                }
            }
        }
    }

    fn move_within_node(&mut self, height: usize, by_chars: isize, #[cfg(feature = "wchar_conversion")] by_pairs: isize) {
        for e in &mut self.0[..height] {
            e.skip_chars = e.skip_chars.wrapping_add(by_chars as usize);
            #[cfg(feature = "wchar_conversion")] {
                e.skip_pairs = e.skip_pairs.wrapping_add(by_pairs as usize);
            }
        }
    }

    pub(crate) fn here_ptr(&self) -> *mut Node {
        self.0[0].node
    }

    pub(crate) fn here_mut_ptr(&mut self) -> *mut Node {
        self.0[0].node
    }

    pub(crate) fn global_char_pos(&self, head_height: u8) -> usize {
        self.0[head_height as usize - 1].skip_chars
    }

    #[cfg(feature = "wchar_conversion")]
    pub(crate) fn wchar_pos(&self, head_height: u8) -> usize {
        let entry = &self.0[head_height as usize - 1];
        entry.skip_chars + entry.skip_pairs
    }

    pub(crate) fn local_char_pos(&self) -> usize {
        self.0[0].skip_chars
    }
}

/// A rope is a "rich string" data structure for storing fancy strings, like the contents of a
/// text editor. See module level documentation for more information.
impl JumpRope {
    fn new_with_rng(rng: RopeRng) -> Self {
        JumpRope {
            rng,
            num_bytes: 0,
            // nexts: [SkipEntry::new(); MAX_HEIGHT],

            // We don't ever store characters in the head node, but the height
            // here is the maximum height of the entire rope.
            head: Node::new_with_height(1, ""),
            // head: Node {
            //     str: GapBuffer::new(),
            //     height: 1,
            //     nexts: [],
            // },
            // nexts: [SkipEntry::new(); MAX_HEIGHT+1],
        }
    }

    /// Creates and returns a new, empty rope.
    ///
    /// In release mode this method is an alias for [`new_from_entropy`](Self::new_from_entropy).
    /// But when compiled for testing (or in debug mode), we use a fixed seed in order to keep tests
    /// fully deterministic.
    ///
    /// Note using this method in wasm significantly increases bundle size. Use
    /// [`new_with_seed`](Self::new_from_seed) instead.
    pub fn new() -> Self {
        if cfg!(test) || cfg!(debug_assertions) || !cfg!(feature = "ddos_protection") {
            Self::new_from_seed(123)
        } else {
            Self::new_from_entropy()
        }
    }

    /// Creates a new, empty rope seeded from an entropy source.
    pub fn new_from_entropy() -> Self {
        Self::new_with_rng(RopeRng::from_entropy())
    }

    /// Creates a new, empty rope using an RNG seeded from the passed u64 parameter.
    ///
    /// The performance of this library with any particular data set will vary by a few percent
    /// within a range based on the seed provided. It may be useful to fix the seed within tests or
    /// benchmarks in order to make the program entirely deterministic, though bear in mind:
    ///
    /// - Jumprope will always use a fixed seed
    pub fn new_from_seed(seed: u64) -> Self {
        Self::new_with_rng(RopeRng::seed_from_u64(seed))
    }

    fn new_from_str(s: &str) -> Self {
        let mut rope = Self::new();
        rope.insert(0, s);
        rope
    }

    /// Return the length of the rope in unicode characters. Note this is not the same as either
    /// the number of bytes the characters take, or the number of grapheme clusters in the string.
    ///
    /// This method returns the length in constant-time (*O(1)*).
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// assert_eq!("↯".len(), 3);
    ///
    /// let rope = JumpRope::from("↯");
    /// assert_eq!(rope.len_chars(), 1);
    ///
    /// // The unicode snowman grapheme cluster needs 2 unicode characters.
    /// let snowman = JumpRope::from("☃️");
    /// assert_eq!(snowman.len_chars(), 2);
    /// ```
    pub fn len_chars(&self) -> usize {
        self.head.nexts()[self.head.height as usize - 1].skip_chars
    }

    /// String length in wide characters (as would be reported by javascript / C# / etc).
    ///
    /// The byte length of this string when encoded to UTF16 will be exactly
    /// `rope.len_wchars() * 2`.
    #[cfg(feature = "wchar_conversion")]
    pub fn len_wchars(&self) -> usize {
        let SkipEntry {
            skip_chars,
            skip_pairs: skip_surrogate_pairs,
            ..
        } = self.head.nexts()[self.head.height as usize - 1];

        skip_surrogate_pairs + skip_chars
    }

    pub(super) fn mut_cursor_at_char(&mut self, char_pos: usize, stick_end: bool) -> MutCursor<'_> {
        assert!(char_pos <= self.len_chars());

        let mut e: *mut Node = &mut self.head;
        let mut height = self.head.height as usize - 1;

        let mut offset = char_pos; // How many more chars to skip

        #[cfg(feature = "wchar_conversion")]
        let mut surrogate_pairs = 0; // Current wchar pos from the start of the rope

        let mut iter = RopeCursor([SkipEntry {
            node: e,
            skip_chars: 0,
            #[cfg(feature = "wchar_conversion")]
            skip_pairs: 0,
        }; MAX_HEIGHT+1]);

        loop { // while height >= 0
            let en = unsafe { &*e };
            let next = en.nexts()[height];
            let skip = next.skip_chars;
            if offset > skip || (!stick_end && offset == skip && !next.node.is_null()) {
                // Go right.
                assert!(e == &mut self.head || !en.str.is_empty());
                offset -= skip;
                #[cfg(feature = "wchar_conversion")] {
                    surrogate_pairs += next.skip_pairs;
                }
                e = next.node;
                assert!(!e.is_null(), "Internal constraint violation: Reached rope end prematurely");
            } else {
                // Record this and go down.
                iter.0[height] = SkipEntry {
                    // node: e as *mut Node, // This is pretty gross
                    node: e,
                    skip_chars: offset,
                    #[cfg(feature = "wchar_conversion")]
                    skip_pairs: surrogate_pairs
                };

                if height != 0 {
                    height -= 1;
                } else {
                    #[cfg(feature = "wchar_conversion")] {
                        // Add on the wchar length at the current node.
                        surrogate_pairs += en.str.count_surrogate_pairs(offset);
                        if surrogate_pairs > 0 {
                            for entry in &mut iter.0[0..self.head.height as usize] {
                                entry.skip_pairs = surrogate_pairs - entry.skip_pairs;
                            }
                        }
                    }
                    break;
                }
            }
        };

        assert!(offset <= NODE_STR_SIZE);

        MutCursor {
            inner: iter,
            // head_nexts: &mut self.head.nexts,
            // head_height: h,
            rng: &mut self.rng,
            num_bytes: &mut self.num_bytes,
            phantom: PhantomData,
        }
    }

    // Internal function for navigating to a particular character offset in the rope.  The function
    // returns the list of nodes which point past the position, as well as offsets of how far into
    // their character lists the specified characters are.
    pub(crate) fn old_cursor_at_char(&self, char_pos: usize, stick_end: bool) -> RopeCursor {
        assert!(char_pos <= self.len_chars());

        let mut e: *const Node = &self.head;
        let mut height = self.head.height as usize - 1;
        
        let mut offset = char_pos; // How many more chars to skip

        #[cfg(feature = "wchar_conversion")]
        let mut surrogate_pairs = 0; // Current wchar pos from the start of the rope

        let mut iter = RopeCursor([SkipEntry::new(); MAX_HEIGHT+1]);

        loop { // while height >= 0
            let en = unsafe { &*e };
            let next = en.nexts()[height];
            let skip = next.skip_chars;
            if offset > skip || (!stick_end && offset == skip && !next.node.is_null()) {
                // Go right.
                assert!(e == &self.head || !en.str.is_empty());
                offset -= skip;
                #[cfg(feature = "wchar_conversion")] {
                    surrogate_pairs += next.skip_pairs;
                }
                e = next.node;
                assert!(!e.is_null(), "Internal constraint violation: Reached rope end prematurely");
            } else {
                // Record this and go down.
                iter.0[height] = SkipEntry {
                    node: e as *mut Node, // This is pretty gross
                    skip_chars: offset,
                    #[cfg(feature = "wchar_conversion")]
                    skip_pairs: surrogate_pairs
                };

                if height != 0 {
                    height -= 1;
                } else {
                    #[cfg(feature = "wchar_conversion")] {
                        // Add on the wchar length at the current node.
                        surrogate_pairs += en.str.count_surrogate_pairs(offset);
                        if surrogate_pairs > 0 {
                            for entry in &mut iter.0[0..self.head.height as usize] {
                                entry.skip_pairs = surrogate_pairs - entry.skip_pairs;
                            }
                        }
                    }
                    break;
                }
            }
        };

        assert!(offset <= NODE_STR_SIZE);
        iter
    }

    /// Create a cursor pointing wchar characters into the rope
    #[cfg(feature = "wchar_conversion")]
    pub(crate) fn cursor_at_wchar(&self, wchar_pos: usize, stick_end: bool) -> RopeCursor {
        assert!(wchar_pos <= self.len_wchars());

        let mut e: *const Node = &self.head;
        let mut height = self.head.height as usize - 1;

        let mut offset = wchar_pos; // How many more chars to skip

        let mut char_pos = 0; // Char pos from the start of the rope

        let mut iter = RopeCursor([SkipEntry::new(); MAX_HEIGHT+1]);

        loop {
            let en = unsafe { &*e };
            let next = en.nexts()[height];
            let skip = next.skip_chars + next.skip_pairs;
            if offset > skip || (!stick_end && offset == skip && !next.node.is_null()) {
                // Go right.
                assert!(e == &self.head || !en.str.is_empty());
                offset -= skip;
                char_pos += next.skip_chars;
                e = next.node;
                assert!(!e.is_null(), "Internal constraint violation: Reached rope end prematurely");
            } else {
                // Record this and go down.
                iter.0[height] = SkipEntry {
                    node: e as *mut Node, // This is pretty gross
                    skip_chars: char_pos,
                    skip_pairs: offset
                };

                if height != 0 {
                    height -= 1;
                } else {
                    char_pos += en.str.count_chars_in_wchars(offset);
                    for entry in &mut iter.0[0..self.head.height as usize] {
                        let skip_chars = char_pos - entry.skip_chars;
                        entry.skip_chars = skip_chars;
                        entry.skip_pairs -= skip_chars;
                    }
                    break;
                }
            }
        };

        assert!(offset <= NODE_STR_SIZE);
        iter
    }

    fn mut_cursor_at_start(&mut self) -> MutCursor<'_> {
        MutCursor {
            inner: RopeCursor([SkipEntry {
                node: &mut self.head,
                skip_chars: 0,
                #[cfg(feature = "wchar_conversion")]
                skip_pairs: 0
            }; MAX_HEIGHT+1]),
            rng: &mut self.rng,
            num_bytes: &mut self.num_bytes,
            phantom: PhantomData,
        }
    }

    fn mut_cursor_at_end(&mut self) -> MutCursor {
        self.mut_cursor_at_char(self.len_chars(), true)
    }

    fn insert_node_at(cursor: &mut MutCursor, contents: &str, num_chars: usize, update_cursor: bool, #[cfg(feature = "wchar_conversion")] num_pairs: usize) {
        // println!("Insert_node_at {} len {}", contents.len(), self.num_bytes);
        // assert!(contents.len() < NODE_STR_SIZE);
        debug_assert_eq!(count_chars(contents), num_chars);
        #[cfg(feature = "wchar_conversion")] {
            debug_assert_eq!(count_utf16_surrogates(contents), num_pairs);
        }
        debug_assert!(num_chars <= NODE_STR_SIZE);

        // TODO: Pin this sucka.
        // let new_node = Pin::new(Node::alloc());
        // let new_node = Node::alloc(cursor.rng, contents);

        let new_height = random_height(cursor.rng);
        let new_node = Box::into_raw(Box::new(Node::new_with_height(new_height, contents)));

        let new_height = new_height as usize;

        // let new_height = unsafe { (*new_node).height as usize };

        let mut head_height = cursor.height();
        while head_height <= new_height {
            // TODO: Why do we copy here? Explain it in a comment. This is
            // currently lifted from the C code.
            // cursor.head_nexts[head_height] = cursor.head_nexts[head_height - 1];
            unsafe {
                let head = &mut (*cursor.inner.0[head_height].node);
                head.nexts[head_height] = head.nexts[head_height - 1];
            }

            cursor.inner.0[head_height] = cursor.inner.0[head_height - 1];

            // *cursor.head_height += 1; // Ends up 1 more than the max node height.
            head_height += 1;
            cursor.set_height(head_height);
        }

        for i in 0..new_height {
            let prev_skip = unsafe { &mut (*cursor.inner.0[i].node).nexts_mut()[i] };
            let nexts = unsafe { (*new_node).nexts_mut() };
            nexts[i].node = prev_skip.node;
            nexts[i].skip_chars = num_chars + prev_skip.skip_chars - cursor.inner.0[i].skip_chars;

            prev_skip.node = new_node;
            prev_skip.skip_chars = cursor.inner.0[i].skip_chars;

            #[cfg(feature = "wchar_conversion")] {
                nexts[i].skip_pairs = num_pairs + prev_skip.skip_pairs - cursor.inner.0[i].skip_pairs;
                prev_skip.skip_pairs = cursor.inner.0[i].skip_pairs;
            }

            // & move the iterator to the end of the newly inserted node.
            if update_cursor {
                cursor.inner.0[i].node = new_node;
                cursor.inner.0[i].skip_chars = num_chars;
                #[cfg(feature = "wchar_conversion")] {
                    cursor.inner.0[i].skip_pairs = num_pairs;
                }
            }
        }

        for i in new_height..head_height {
            // I don't know why miri needs me to use nexts[] rather than nexts_mut() here but ??.
            unsafe {
                (*cursor.inner.0[i].node).nexts[i].skip_chars += num_chars;
                #[cfg(feature = "wchar_conversion")] {
                    (*cursor.inner.0[i].node).nexts[i].skip_pairs += num_pairs;
                }
            }
            if update_cursor {
                cursor.inner.0[i].skip_chars += num_chars;
                #[cfg(feature = "wchar_conversion")] {
                    cursor.inner.0[i].skip_pairs += num_pairs;
                }
            }
        }

        // self.nexts[self.head.height as usize - 1].skip_chars += num_chars;
        *cursor.num_bytes += contents.len();
    }

    fn insert_at_cursor(cursor: &mut MutCursor, contents: &str) {
        if contents.is_empty() { return; }
        // iter contains how far (in characters) into the current element to
        // skip. Figure out how much that is in bytes.
        let mut offset_bytes: usize = 0;
        // The insertion offset into the destination node.
        let offset_chars: usize = cursor.inner.0[0].skip_chars;
        let head_height = cursor.height();

        let mut e = cursor.inner.here_mut_ptr();

        // We might be able to insert the new data into the current node, depending on
        // how big it is. We'll count the bytes, and also check that its valid utf8.
        let num_inserted_bytes = contents.len();
        let mut num_inserted_chars = count_chars(contents);
        #[cfg(feature = "wchar_conversion")]
            let mut num_inserted_pairs = if num_inserted_bytes != num_inserted_chars {
            count_utf16_surrogates(contents)
        } else { 0 };

        // Adding this short circuit makes the code about 2% faster for 1% more code
        unsafe {
            if (*e).str.gap_start_chars as usize == offset_chars && (*e).str.gap_len as usize >= num_inserted_bytes {
                // Short circuit. If we can just insert all the content right here in the gap, do so.
                (*e).str.insert_in_gap(contents);

                #[cfg(feature = "wchar_conversion")] {
                    cursor.inner.update_offsets(head_height, num_inserted_chars as isize, num_inserted_pairs as isize);
                    cursor.inner.move_within_node(head_height, num_inserted_chars as isize, num_inserted_pairs as isize);
                }
                #[cfg(not(feature = "wchar_conversion"))] {
                    cursor.inner.update_offsets(head_height, num_inserted_chars as isize);
                    cursor.inner.move_within_node(head_height, num_inserted_chars as isize);
                }

                *cursor.num_bytes += num_inserted_bytes;
                return;
            }

            if offset_chars > 0 {
                // Changing this to debug_assert reduces performance by a few % for some reason.
                assert!(offset_chars <= (*e).nexts()[0].skip_chars);
                // This could be faster, but its not a big deal.
                offset_bytes = (*e).str.count_bytes(offset_chars);
            }

            // Can we insert into the current node?
            let current_len_bytes = (*e).str.len_bytes();
            let mut insert_here = current_len_bytes + num_inserted_bytes <= NODE_STR_SIZE;

            // If we can't insert here, see if we can move the cursor forward and insert into the
            // subsequent node.
            if !insert_here && offset_bytes == current_len_bytes {
                // We can insert into the subsequent node if:
                // - We can't insert into the current node
                // - There _is_ a next node to insert into
                // - The insert would be at the start of the next node
                // - There's room in the next node
                if let Some(next) = (*e).first_next_mut().node.as_mut() {
                    if next.str.len_bytes() + num_inserted_bytes <= NODE_STR_SIZE {
                        offset_bytes = 0;

                        // Could do this with slice::fill but this seems slightly faster.
                        for e in &mut cursor.inner.0[..next.height as usize] {
                            *e = SkipEntry {
                                node: next,
                                skip_chars: 0,
                                #[cfg(feature = "wchar_conversion")]
                                skip_pairs: 0
                            };
                        }
                        e = next;

                        insert_here = true;
                    }
                }
            }

            if insert_here {
                // First move the current bytes later on in the string.
                let c = &mut (*e).str;
                c.try_insert(offset_bytes, contents).unwrap();

                *cursor.num_bytes += num_inserted_bytes;
                // .... aaaand update all the offset amounts.

                #[cfg(feature = "wchar_conversion")] {
                    cursor.inner.update_offsets(head_height, num_inserted_chars as isize, num_inserted_pairs as isize);
                    cursor.inner.move_within_node(head_height, num_inserted_chars as isize, num_inserted_pairs as isize);
                }
                #[cfg(not(feature = "wchar_conversion"))] {
                    cursor.inner.update_offsets(head_height, num_inserted_chars as isize);
                    cursor.inner.move_within_node(head_height, num_inserted_chars as isize);
                }
            } else {
                // There isn't room. We'll need to add at least one new node to the rope.

                // If we're not at the end of the current node, we'll need to remove
                // the end of the current node's data and reinsert it later.
                (*e).str.move_gap(offset_bytes);

                let num_end_bytes = (*e).str.len_bytes() - offset_bytes;
                let mut num_end_chars: usize = 0;
                #[cfg(feature = "wchar_conversion")]
                let mut num_end_pairs: usize = 0;

                // let end_str = if num_end_bytes > 0 {
                if num_end_bytes > 0 {
                    // We'll truncate the node, but leave the bytes themselves there (for later).

                    // It would also be correct (and slightly more space efficient) to pack some of the
                    // new string's characters into this node after trimming it.
                    num_end_chars = (*e).num_chars() - offset_chars;

                    #[cfg(feature = "wchar_conversion")] {
                        num_end_pairs = (*e).num_surrogate_pairs() - (*e).str.gap_start_surrogate_pairs as usize;
                        debug_assert_eq!(num_end_pairs, count_utf16_surrogates((*e).str.end_as_str()));
                        cursor.inner.update_offsets(head_height, -(num_end_chars as isize), -(num_end_pairs as isize));
                    }
                    #[cfg(not(feature = "wchar_conversion"))]
                    cursor.inner.update_offsets(head_height, -(num_end_chars as isize));

                    *cursor.num_bytes -= num_end_bytes;
                }

                // Now we insert new nodes containing the new character data. The
                // data must be broken into pieces of with a maximum size of
                // NODE_STR_SIZE. Node boundaries must not occur in the middle of a
                // utf8 codepoint.
                // let mut str_offset: usize = 0;
                let mut remainder = contents;
                // while !remainder.is_empty() {
                loop {
                    // println!(". {}", remainder);
                    // Find the first index after STR_SIZE bytes

                    if remainder.len() <= NODE_STR_SIZE {
                        Self::insert_node_at(cursor, remainder, num_inserted_chars, true, #[cfg(feature = "wchar_conversion")] num_inserted_pairs);
                        break;
                    } else {
                        // Find a suitable cut point. We should take as many characters as we can fit in
                        // the node, without splitting any unicode codepoints.
                        let mut byte_pos = NODE_STR_SIZE;
                        loop { // Slide back to a character boundary.
                            let c = remainder.as_bytes()[byte_pos];
                            if c & 0b1100_0000 != 0b1000_0000 {
                                break;
                            }
                            byte_pos -= 1;
                        }

                        let slice = &remainder.as_bytes()[..byte_pos];
                        let char_pos = count_chars_in_bytes(slice);
                        num_inserted_chars -= char_pos;
                        #[cfg(feature = "wchar_conversion")]
                            let pairs = unsafe { count_utf16_surrogates_in_bytes(slice) };
                        #[cfg(feature = "wchar_conversion")] {
                            num_inserted_pairs -= pairs;
                        }

                        let (next, rem) = remainder.split_at(byte_pos);
                        assert!(!next.is_empty());
                        Self::insert_node_at(cursor, next, char_pos, true, #[cfg(feature = "wchar_conversion")] pairs);
                        remainder = rem;
                    }
                }

                if num_end_bytes > 0 {
                    let end_str = (*e).str.take_rest();
                    Self::insert_node_at(cursor, end_str, num_end_chars, false, #[cfg(feature = "wchar_conversion")] num_end_pairs);
                }
                // if let Some(end_str) = end_str {
                //     Self::insert_node_at(cursor, end_str, num_end_chars, false, #[cfg(feature = "wchar_conversion")] num_end_pairs);
                // }
            }

            assert_ne!(cursor.inner.local_char_pos(), 0);
        }
    }

    fn del_at_cursor(cursor: &mut MutCursor, mut length: usize) {
        if length == 0 { return; }
        let mut offset_chars = cursor.inner.local_char_pos();
        let mut node = cursor.inner.here_ptr();
        unsafe {
            while length > 0 {
                {
                    let s = (&*node).first_next();
                    if offset_chars == s.skip_chars {
                        // End of current node. Skip to the start of the next one.
                        node = s.node;
                        offset_chars = 0;
                    }
                }

                let num_chars = (&*node).num_chars();
                let removed = std::cmp::min(length, num_chars - offset_chars);
                assert!(removed > 0);

                // TODO: Figure out a better way to calculate this.
                #[cfg(feature = "wchar_conversion")]
                    let removed_pairs = (*node).str.count_surrogate_pairs(offset_chars + removed)
                    - (*node).str.count_surrogate_pairs(offset_chars);

                let height = (*node).height as usize;
                if removed < num_chars || cursor.is_head(node) {
                    // Just trim the node down.
                    let s = &mut (*node).str;
                    let removed_bytes = s.remove_chars(offset_chars, removed);
                    *cursor.num_bytes -= removed_bytes;

                    for s in (*node).nexts_mut() {
                        s.skip_chars -= removed;
                        #[cfg(feature = "wchar_conversion")] {
                            s.skip_pairs -= removed_pairs;
                        }
                    }
                } else {
                    // Remove the node from the skip list. This works because the cursor must be
                    // pointing from the previous element to the start of this element.
                    assert_ne!(cursor.inner.0[0].node, node);

                    for i in 0..(*node).height as usize {
                        let s = &mut (*cursor.inner.0[i].node).nexts_mut()[i];
                        s.node = (*node).nexts_mut()[i].node;
                        s.skip_chars += (*node).nexts()[i].skip_chars - removed;
                        #[cfg(feature = "wchar_conversion")] {
                            s.skip_pairs += (*node).nexts()[i].skip_pairs - removed_pairs;
                        }
                    }

                    *cursor.num_bytes -= (*node).str.len_bytes();
                    let next = (*node).first_next().node;
                    // Node::free(node);
                    drop(Box::from_raw(node));
                    node = next;
                }

                for i in height..cursor.height() {
                    let s = &mut (*cursor.inner.0[i].node).nexts_mut()[i];
                    s.skip_chars -= removed;
                    #[cfg(feature = "wchar_conversion")] {
                        s.skip_pairs -= removed_pairs;
                    }
                }

                length -= removed;
            }
        }
    }

    fn eq_str(&self, mut other: &str) -> bool {
        if self.len_bytes() != other.len() { return false; }

        for s in self.chunks().strings() {
            let (start, rem) = other.split_at(s.len());
            if start != s { return false; }
            other = rem;
        }

        true
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
                // Node::free(node);
                drop(Box::from_raw(node));
                node = next;
            }
        }
    }
}

impl From<&str> for JumpRope {
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

        let mut other_iter = other.chunks().strings();

        // let mut os = other_iter.next();
        let mut os = "";

        for mut s in self.chunks().strings() {
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

impl Debug for JumpRope {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.chunks().strings())
            .finish()
    }
}

impl Display for JumpRope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (s, _) in self.chunks() {
            f.write_str(s)?;
        }
        Ok(())
    }
}

// I don't know why I need all three of these, but I do.

impl<T: AsRef<str>> PartialEq<T> for JumpRope {
    fn eq(&self, other: &T) -> bool {
        self.eq_str(other.as_ref())
    }
}

// Needed for assert_eq!(&rope, "Hi there");
impl PartialEq<str> for JumpRope {
    fn eq(&self, other: &str) -> bool {
        self.eq_str(other)
    }
}

// Needed for assert_eq!(&rope, String::from("Hi there"));
impl PartialEq<String> for &JumpRope {
    fn eq(&self, other: &String) -> bool {
        self.eq_str(other.as_str())
    }
}

impl<'a> Extend<&'a str> for JumpRope {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        let mut cursor = self.mut_cursor_at_end();
        iter.into_iter().for_each(|s| {
            Self::insert_at_cursor(&mut cursor, s);
        });
    }
}

impl Clone for JumpRope {
    fn clone(&self) -> Self {
        // This method could be a little bit more efficient, but I think improving clone()
        // performance isn't worth the extra effort.
        let mut r = JumpRope::new();
        let mut cursor = r.mut_cursor_at_start();
        for node in self.node_iter() {
            JumpRope::insert_at_cursor(&mut cursor, node.as_str_1());
            JumpRope::insert_at_cursor(&mut cursor, node.as_str_2());
        }
        r
    }
}

impl JumpRope {
    /// Insert new content into the rope. The content is inserted at the specified unicode character
    /// offset, which is different from a byte offset for non-ASCII characters.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let mut rope = JumpRope::from("--");
    /// rope.insert(1, "hi there");
    /// assert_eq!(rope.to_string(), "-hi there-");
    /// ```
    ///
    /// If the position names a location past the end of the rope, it is truncated.
    pub fn insert(&mut self, mut pos: usize, contents: &str) {
        // if cfg!(debug_assertions) { self.check(); }

        if contents.is_empty() { return; }
        pos = std::cmp::min(pos, self.len_chars());

        let mut cursor = self.mut_cursor_at_char(pos, true);

        Self::insert_at_cursor(&mut cursor, contents);

        debug_assert_eq!(cursor.inner.global_char_pos(self.head.height), pos + count_chars(contents));
        // dbg!(&cursor.0[..self.head.height as usize]);
    }

    /// Delete a span of unicode characters from the rope. The span is specified in unicode
    /// characters, not bytes.
    ///
    /// Any attempt to delete past the end of the rope will be silently ignored.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let mut rope = JumpRope::from("Whoa dawg!");
    /// rope.remove(4..9); // delete " dawg"
    /// assert_eq!(rope.to_string(), "Whoa!");
    /// ```
    pub fn remove(&mut self, mut range: Range<usize>) {
        // if cfg!(debug_assertions) { self.check(); }

        range.end = range.end.min(self.len_chars());
        if range.start >= range.end { return; }

        // We need to stick_end so we can delete entries.
        let mut cursor = self.mut_cursor_at_char(range.start, true);
        Self::del_at_cursor(&mut cursor, range.end - range.start);
        // let mut cursor = self.old_cursor_at_char(range.start, true);
        // unsafe { self.old_del_at_cursor(&mut cursor, range.end - range.start); }

        debug_assert_eq!(cursor.inner.global_char_pos(self.head.height), range.start);
    }

    /// Replace the specified range with new content. This is equivalent to calling
    /// [`remove`](Self::remove) followed by [`insert`](Self::insert), but it is simpler and faster.
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let mut rope = JumpRope::from("Hi Mike!");
    /// rope.replace(3..7, "Duane"); // replace "Mike" with "Duane"
    /// assert_eq!(rope.to_string(), "Hi Duane!");
    /// ```
    pub fn replace(&mut self, range: Range<usize>, content: &str) {
        let len = self.len_chars();
        let pos = usize::min(range.start, len);
        let del_len = usize::min(range.end, len) - pos;

        let mut cursor = self.mut_cursor_at_char(pos, true);
        if del_len > 0 {
            Self::del_at_cursor(&mut cursor, del_len);
        }
        if !content.is_empty() {
            Self::insert_at_cursor(&mut cursor, content);
        }

        debug_assert_eq!(cursor.inner.global_char_pos(self.head.height), pos + count_chars(content));
    }

    /// Get the number of bytes used for the UTF8 representation of the rope. This will always match
    /// the .len() property of the equivalent String.
    ///
    /// Note: This is only useful in specific situations - like preparing a byte buffer for saving
    /// or sending over the internet. In many cases it is preferable to use
    /// [`len_chars`](Self::len_chars).
    ///
    /// # Example
    ///
    /// ```
    /// # use jumprope::*;
    /// let str = "κόσμε"; // "Cosmos" in ancient greek
    /// assert_eq!(str.len(), 11); // 11 bytes over the wire
    ///
    /// let rope = JumpRope::from(str);
    /// assert_eq!(rope.len_bytes(), str.len());
    /// ```
    pub fn len_bytes(&self) -> usize { self.num_bytes }

    /// Returns `true` if the rope contains no elements.
    pub fn is_empty(&self) -> bool { self.num_bytes == 0 }

    pub fn check(&self) {
        assert!(self.head.height >= 1);
        assert!(self.head.height < MAX_HEIGHT_U8 + 1);

        let skip_over = &self.head.nexts[self.head.height as usize - 1];
        // println!("Skip over skip chars {}, num bytes {}", skip_over.skip_chars, self.num_bytes);
        assert!(skip_over.skip_chars <= self.num_bytes as usize);
        #[cfg(feature = "wchar_conversion")] {
            assert!(skip_over.skip_pairs <= skip_over.skip_chars);
        }
        assert!(skip_over.node.is_null());

        // The offsets store the total distance travelled since the start.
        let mut iter = [SkipEntry::new(); MAX_HEIGHT];
        for i in 0..self.head.height {
            // Bleh.
            iter[i as usize].node = &self.head as *const Node as *mut Node;
        }

        let mut num_bytes: usize = 0;
        let mut num_chars = 0;
        #[cfg(feature = "wchar_conversion")]
        let mut num_pairs = 0;

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
                #[cfg(feature = "wchar_conversion")] {
                    assert_eq!(entry.skip_pairs, num_pairs);
                }

                // println!("replacing entry {:?} with {:?}", entry, n.nexts()[i].node);
                entry.node = n.nexts()[i].node;
                entry.skip_chars += n.nexts()[i].skip_chars;
                #[cfg(feature = "wchar_conversion")] {
                    entry.skip_pairs += n.nexts()[i].skip_pairs;
                }
            }

            num_bytes += n.str.len_bytes();
            num_chars += n.num_chars();

            #[cfg(feature = "wchar_conversion")] {
                assert_eq!(n.num_surrogate_pairs(), n.str.count_surrogate_pairs(n.num_chars()));
                num_pairs += n.num_surrogate_pairs();
            }
        }

        for entry in iter[0..self.head.height as usize].iter() {
            // println!("{:?}", entry);
            assert!(entry.node.is_null());
            assert_eq!(entry.skip_chars, num_chars);
            #[cfg(feature = "wchar_conversion")] {
                assert_eq!(entry.skip_pairs, num_pairs);
            }
        }

        // println!("self bytes: {}, count bytes {}", self.num_bytes, num_bytes);
        assert_eq!(self.num_bytes, num_bytes);
        assert_eq!(self.len_chars(), num_chars);
        #[cfg(feature = "wchar_conversion")] {
            assert_eq!(self.len_wchars(), num_chars + num_pairs);
        }
    }

    /// This method counts the number of bytes of memory allocated in the rope. This is purely for
    /// debugging.
    ///
    /// Notes:
    ///
    /// - This method (its existence, its signature and its return value) is not considered part of
    ///   the stable API provided by jumprope. This may disappear or change in point releases.
    /// - This method walks the entire rope. It has time complexity O(n).
    /// - If a rope is owned inside another structure, this method will double-count the bytes
    ///   stored in the rope's head.
    pub fn mem_size(&self) -> usize {
        let mut nodes = self.node_iter();
        let mut size = 0;
        // The first node is the head. Count the actual head size.
        size += std::mem::size_of::<Self>();
        nodes.next(); // And discard it from the iterator.

        for _n in nodes {
            // let layout = Node::layout_with_height(n.height);
            // size += layout.size();
            size += std::mem::size_of::<Node>();
        }

        size
    }

    #[allow(unused)]
    // pub fn print(&self) {
    pub(crate) fn print(&self) {
        println!("chars: {}\tbytes: {}\theight: {}", self.len_chars(), self.num_bytes, self.head.height);

        print!("HEAD:");
        for s in self.head.nexts() {
            print!(" |{} ", s.skip_chars);
            #[cfg(feature = "wchar_conversion")] {
                print!("({}) ", s.skip_pairs);
            }
        }
        println!();

        for (i, node) in self.node_iter().enumerate() {
            print!("{}:", i);
            for s in node.nexts() {
                print!(" |{} ", s.skip_chars);
                #[cfg(feature = "wchar_conversion")] {
                    print!("({}) ", s.skip_pairs);
                }
            }
            println!("      : {:?}(s{}) + {:?}(s{})",
                     node.as_str_1(), count_utf16_surrogates(node.as_str_1()),
                     node.as_str_2(), count_utf16_surrogates(node.as_str_2())
            );
        }
    }
}

/// These methods are only available if the `wchar_conversion` feature is enabled.
#[cfg_attr(doc_cfg, doc(cfg(feature = "wchar_conversion")))]
#[cfg(feature = "wchar_conversion")]
impl JumpRope {
    /// Convert from a unicode character count to a wchar index, like what you'd use in Javascript,
    /// Java or C#.
    pub fn chars_to_wchars(&self, chars: usize) -> usize {
        let cursor = self.old_cursor_at_char(chars, true);
        cursor.wchar_pos(self.head.height)
    }

    /// Convert a wchar index back to a unicode character count.
    ///
    /// **NOTE:** This method's behaviour is undefined if the wchar offset is invalid. Eg, given a
    /// rope with contents `𐆚` (a single character with wchar length 2), `wchars_to_chars(1)` is
    /// undefined and may panic / change in future versions of diamond types.
    pub fn wchars_to_chars(&self, wchars: usize) -> usize {
        let cursor = self.cursor_at_wchar(wchars, true);
        cursor.global_char_pos(self.head.height)
    }

    /// Insert the given utf8 string into the rope at the specified wchar position.
    /// This is compatible with NSString, Javascript, etc.
    ///
    /// Returns the insertion position in characters.
    ///
    /// **NOTE:** This method's behaviour is undefined if the wchar offset is invalid. Eg, given a
    /// rope with contents `𐆚` (a single character with wchar length 2), `insert_at_wchar(1, ...)`
    /// is undefined and may panic / change in future versions of diamond types.
    pub fn insert_at_wchar(&mut self, mut pos_wchar: usize, contents: &str) -> usize {
        pos_wchar = pos_wchar.min(self.len_wchars());

        let mut cursor = self.cursor_at_wchar(pos_wchar, true);
        // dbg!(pos_wchar, &cursor.0[0..3]);
        unsafe { self.old_insert_at_cursor(&mut cursor, contents); }

        debug_assert_eq!(
            cursor.wchar_pos(self.head.height),
            pos_wchar + count_chars(contents) + count_utf16_surrogates(contents)
        );

        cursor.global_char_pos(self.head.height)
    }

    /// Remove items from the rope, specified by the passed range. The indexes are interpreted
    /// as wchar offsets (like you'd get in javascript / C# / etc).
    ///
    /// **NOTE:** This method's behaviour is undefined if the wchar offset is invalid. Eg, given a
    /// rope with contents `𐆚` (a single character with wchar length 2), `remove_at_wchar(1..2)`
    /// is undefined and may panic / change in future versions of diamond types.
    pub fn remove_at_wchar(&mut self, mut range: Range<usize>) {
        range.end = range.end.min(self.len_wchars());
        if range.is_empty() { return; }

        // Rather than making some fancy custom remove function, I'm just going to convert the
        // removed range into a char range and delete that.
        let cursor_end = self.cursor_at_wchar(range.end, true);
        let char_end = cursor_end.global_char_pos(self.head.height);
        drop(cursor_end);

        // We need to stick_end so we can delete entries.
        let mut cursor = self.cursor_at_wchar(range.start, true);
        let char_start = cursor.global_char_pos(self.head.height);

        unsafe { self.old_del_at_cursor(&mut cursor, char_end - char_start); }

        debug_assert_eq!(cursor.wchar_pos(self.head.height), range.start);
    }

    /// Replace the characters in the specified wchar range with content.
    ///
    /// **NOTE:** This method's behaviour is undefined if the wchar offset is invalid. Eg, given a
    /// rope with contents `𐆚` (a single character with wchar length 2),
    /// `replace_at_wchar(1..2, ...)` is undefined and may panic / change in future versions of
    /// diamond types.
    pub fn replace_at_wchar(&mut self, range: Range<usize>, content: &str) {
        // TODO: Optimize this. This method should work similarly to replace(), where we create
        // a single cursor and use it in both contexts.
        if !range.is_empty() {
            self.remove_at_wchar(range.clone());
        }
        if !content.is_empty() {
            self.insert_at_wchar(range.start, content);
        }
    }
}