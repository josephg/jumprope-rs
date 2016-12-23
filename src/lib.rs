
use std::ptr;
use std::str;



#[derive(Debug)]
pub enum RopeError {
	PositionOutOfBounds,
	InvalidCodepoint,
}

pub trait Rope {
    fn new() -> Self;

    fn insert(&mut self, pos: usize, contents: &str) -> Result<(), RopeError>;
    fn del(&mut self, pos: usize, len: usize) -> Result<(), RopeError>;

    fn slice(&self, pos: usize, len: usize) -> Result<String, RopeError>;

	fn to_string(&self) -> String;

	fn len(&self) -> usize; // in bytes
	fn char_len(&self) -> usize; // in unicode values
}


// Must be <= UINT16_MAX. Benchmarking says this is pretty close to optimal
// (tested on a mac using clang 4.0 and x86_64).
const ROPE_NODE_STR_SIZE: usize = 136;

// The likelyhood (%) a node will have height (n+1) instead of n
const ROPE_BIAS: u32 = 25;

// The rope will become less efficient after the string is 2 ^ ROPE_MAX_HEIGHT
// nodes.
const ROPE_MAX_HEIGHT: usize = 60;

#[derive(Clone)]
#[derive(Copy)]
struct SkipEntry {
	// The number of *characters* between the start of the current node and the
	// start of the next node.
	num_chars: usize,

    node: *mut Node,
}

#[repr(C)]
struct Node {
	contents: [u8; ROPE_NODE_STR_SIZE],

	// Number of bytes in contents in use
	num_bytes: u8,
	// And the number of characters those bytes take up
	num_chars: u8,

    // Height of skips array.
    height: u8,

    // Owned pointer for the next node.
    next: Option<Box<Node>>,

    // height skips. Using ptr transmute to manage these.
    skips: [SkipEntry; 0],
}

pub struct JumpRope {
    // The total number of characters in the rope
	num_chars: usize,

	// The total number of bytes which the characters in the rope take up
	num_bytes: usize,

    head: Option<Box<Node>>,
    skips: Vec<SkipEntry>,
}


impl SkipEntry {
    fn new() -> Self {
        SkipEntry { num_chars: 0, node: ptr::null_mut() }
    }
}

impl Node {
    fn to_str(&self) -> &str {
        let slice = &self.contents[..self.num_bytes as usize];
        // The contents must be valid utf8 content.
        str::from_utf8(slice).unwrap()
    }

    fn new_with_size(height: usize) -> Box<Node> {
        //use alloc::heap;
        //use std::mem;

        //let size = mem::size_of::<Node>() + mem::size_of::<[SkipEntry; 1]>() * height;
        //let ptr = heap::allocate(size, mem::align_of::<Node>());
        Box::new(Node {
            contents: [0; ROPE_NODE_STR_SIZE],
            num_bytes: 0,
            num_chars: 0,
            next: None,
            height: 0,
            skips: [SkipEntry::new(); 0],
        })
    }
}

impl JumpRope {
    pub fn new() -> Self {
        JumpRope {
            num_chars: 0,
            num_bytes: 0,
            head: None,
            skips: Vec::new(),
        }
    }
}

impl Rope for JumpRope {
    fn new() -> Self {
        JumpRope::new()
    }

	fn insert(&mut self, pos: usize, contents: &str) -> Result<(), RopeError> {
		unimplemented!();

	}
    fn del(&mut self, pos: usize, len: usize) -> Result<(), RopeError> {
		unimplemented!();
	}

    fn slice(&self, pos: usize, len: usize) -> Result<String, RopeError> {
	   	unimplemented!();
   	}
	fn to_string(&self) -> String {
        unimplemented!();
        /*
        // TODO: Rewrite this using the node iterator.
        let mut content = String::with_capacity(self.num_bytes);

        let mut node: &Node<[SkipEntry]> = &self.head;
        loop {
            content.push_str(node.to_str());
            match node.next {
                Some(ref next) => node = next,
                None => break,
            }
        }

        content*/
	}
	fn len(&self) -> usize { self.num_bytes }
	fn char_len(&self) -> usize { self.num_chars }
}

