
// #[macro_use]
extern crate criterion;
use criterion::*;

// extern crate rand;
// use rand::seq::IteratorRandom;
use rand::prelude::*;
use rand_xorshift::*;

mod rope;
use self::rope::*;
use jumprope::*;

mod edittablestr;

use std::cmp::min;

extern crate ropey;
use self::ropey::Rope as RopeyRope;

extern crate an_rope;
use an_rope::Rope as AnRope;

extern crate xi_rope;
use xi_rope::Rope as XiRope;

const CHARS: &[u8; 83] = b" ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()[]{}<>?,./";

// Gross. Find a way to reuse the code from random_unicode_string.
fn random_ascii_string(rng: &mut XorShiftRng, len: usize) -> String {
    let mut s = String::new();
    for _ in 0..len {
        // s.push(*rng.choose(CHARS).unwrap() as char);
        s.push(CHARS[rng.gen_range(0, CHARS.len())] as char);
    }
    s
}

impl Rope for JumpRope {
    fn new() -> Self { JumpRope::new() }

    fn insert_at(&mut self, pos: usize, contents: &str) { self.insert_at(pos, contents); }
    fn del_at(&mut self, pos: usize, len: usize) { self.del_at(pos, len); }

    fn to_string(&self) -> String { self.to_string() }
    
    // fn len(&self) -> usize { self.len() } // in bytes
    fn char_len(&self) -> usize { self.char_len() } // in unicode values
}

impl Rope for AnRope {
    fn new() -> Self { AnRope::new() }

    fn insert_at(&mut self, pos: usize, contents: &str) { *self = self.insert_str(pos, contents); }
    fn del_at(&mut self, pos: usize, len: usize) { self.delete(pos..pos+len); }

    fn to_string(&self) -> String { std::string::ToString::to_string(self) }
    
    // fn len(&self) -> usize { self.len() } // in bytes
    fn char_len(&self) -> usize { self.len() } // in unicode values
}

impl Rope for XiRope {
    fn new() -> Self { XiRope::from("") }

    fn insert_at(&mut self, pos: usize, contents: &str) { self.edit(pos..pos, contents); }
    fn del_at(&mut self, pos: usize, len: usize) { self.edit(pos..pos+len, ""); }

    fn to_string(&self) -> String {
        String::from(self)
    }
    
    // fn len(&self) -> usize { self.len() } // in bytes
    fn char_len(&self) -> usize {
        let mut len = 0;
        for s in self.iter_chunks(..) {
            len += s.chars().count();
        }
        len
    } // in unicode values
}

impl Rope for RopeyRope {
    fn new() -> Self { RopeyRope::new() }

    fn insert_at(&mut self, pos: usize, contents: &str) {
        self.insert(pos, contents);
    }
    fn del_at(&mut self, pos: usize, len: usize) {
        self.remove(pos..pos+len);
    }
    // fn del_at<R: RangeBounds<usize>>(&mut self, range: R);

    // fn slice(&self, pos: usize, len: usize) -> Result<String, RopeError>;

    fn to_string(&self) -> String { unimplemented!() }
    
    // fn len(&self) -> usize { self.len_bytes() } // in bytes
    fn char_len(&self) -> usize { self.len_chars() } // in unicode values
}

use std::os::raw::c_char;
use std::ffi::CString;

#[repr(C)]
struct CRopeRaw { _unused : [ u8 ; 0 ] }

extern {
    fn rope_new() -> *mut CRopeRaw;
    fn rope_new_with_utf8(s: *const c_char) -> *mut CRopeRaw;
    fn rope_free(r: *mut CRopeRaw);
    fn rope_char_count(r: *const CRopeRaw) -> usize;
    // fn rope_byte_count(r: *const CRopeRaw) -> usize;

    fn rope_insert(r: *mut CRopeRaw, pos: usize, s: *const c_char) -> u32;
    fn rope_del(r: *mut CRopeRaw, pos: usize, len: usize) -> u32;
}

struct CRope(*mut CRopeRaw);
impl Rope for CRope {
    fn new() -> Self { unsafe { CRope(rope_new()) } }

    fn insert_at(&mut self, pos: usize, contents: &str) {
        unsafe { rope_insert(self.0, pos, CString::new(contents).unwrap().as_ptr()); }
    }
    fn del_at(&mut self, pos: usize, len: usize) {
        unsafe { rope_del(self.0, pos, len); }
    }
    fn to_string(&self) -> String { unimplemented!() }
    
    // fn len(&self) -> usize { unsafe { rope_byte_count(self.0) } } // in bytes
    fn char_len(&self) -> usize { unsafe { rope_char_count(self.0) } } // in unicode values
}
impl Drop for CRope {
    fn drop(&mut self) {
        unsafe { rope_free(self.0); }
    }
}
impl From<String> for CRope {
    fn from(s: String) -> Self {
        CRope(unsafe { rope_new_with_utf8(CString::new(s).unwrap().as_ptr()) })
    }
}

#[test]
fn foo() {
    unsafe {
        let r = rope_new();
        println!("size {}", rope_char_count(r));
    }
}

fn gen_strings(rng: &mut XorShiftRng) -> Vec<String> {
    // I wish there was a better syntax for just making an array here.
    let mut strings = Vec::<String>::new();
    for _ in 0..100 {
        let len = rng.gen_range(1, 3);
        strings.push(random_ascii_string(rng, len));
    }

    strings
}

fn ins_append<R: Rope>(b: &mut Bencher) {
    let mut rng = XorShiftRng::from_seed([1,2,3,4,1,2,3,4,1,2,3,4,1,2,3,4]);
    let strings = gen_strings(&mut rng);

    let mut r = R::new();
    let mut len = 0;
    b.iter(|| {
        // let pos = rng.gen_range(0, len+1);
        let text = &strings[rng.gen_range(0, strings.len())];
        r.insert_at(len, text.as_str());
        len += text.chars().count();
    });

    black_box(r.char_len());
}

fn ins_random<R: Rope>(b: &mut Bencher) {
    let mut rng = XorShiftRng::from_seed([1,2,3,4,1,2,3,4,1,2,3,4,1,2,3,4]);
    let strings = gen_strings(&mut rng);

    let mut r = R::new();
    // Len isn't needed, but its here to allow direct comparison with ins_append.
    let mut len = 0;
    b.iter(|| {
        let pos = rng.gen_range(0, len+1);
        let text = &strings[rng.gen_range(0, strings.len())];
        r.insert_at(pos, text.as_str());
        len += text.chars().count();
    });

    black_box(r.char_len());
    black_box(len); 
}

fn stable_ins_del<R: Rope + From<String>>(b: &mut Bencher, target_length: &usize) {
    let target_length = *target_length;
    let mut rng = XorShiftRng::from_seed([1,2,3,4,1,2,3,4,1,2,3,4,1,2,3,4]);

    // I wish there was a better syntax for just making an array here.
    let strings = gen_strings(&mut rng);
    
    // let target_length = 100000;
    // let mut r = R::new();
    // while r.char_len() < target_length {
    //     // The rope should be a hot mess.
    //     let pos = rng.gen_range(0, r.char_len()+1);
    //     r.insert_at(pos, strings[rng.gen_range(0, strings.len())].as_str()).unwrap();
    // }
    let mut r = R::from(random_ascii_string(&mut rng, target_length));
    let mut len = target_length;

    b.iter(|| {
        // let len = r.char_len();
        // if len == 0 || rng.gen::<bool>() {
        if len <= target_length {
            // Insert
            let pos = rng.gen_range(0, len+1);
            let text = &strings[rng.gen_range(0, strings.len())];
            r.insert_at(pos, text.as_str());
            len += text.chars().count();
        } else {
            // Delete
            let pos = rng.gen_range(0, len);
            let dlen = min(rng.gen_range(0, 10), len - pos);
            len -= dlen;

            r.del_at(pos, dlen);
        }
    });

    // Return something based on the computation to avoid it being optimized
    // out. Although right now the compiler isn't smart enough for that
    // anyway.
    // r.len()
    black_box(r.char_len());
}

fn bench_ins_append(c: &mut Criterion) {
    let benchmark = Benchmark::new("jumprope", ins_append::<JumpRope>)
        .with_function("ropey", ins_append::<RopeyRope>)
        // anrope runs out of stack and crashes in this test.
        // .with_function("anrope", ins_append::<AnRope>)
        .with_function("xirope", ins_append::<XiRope>)
        .with_function("jumprope_c", ins_append::<CRope>)
        .with_function("raw_string", ins_append::<String>)
    ;
    c.bench("ins_append", benchmark);
}

fn bench_ins_random(c: &mut Criterion) {
    let benchmark = Benchmark::new("jumprope", ins_random::<JumpRope>)
        .with_function("jumprope_c", ins_random::<CRope>)
        .with_function("ropey", ins_random::<RopeyRope>)
        .with_function("anrope", ins_random::<AnRope>)
        .with_function("xirope", ins_random::<XiRope>)
        .with_function("raw_string", ins_random::<String>)
    ;
    c.bench("ins_random", benchmark);
}

fn bench_stable_ins_del(c: &mut Criterion) {
    let params = vec![
        1000,
        10000,
        100000,
        1000000,
        10000000,
    ];
    let benchmark = ParameterizedBenchmark::new("raw_string", stable_ins_del::<String>, params)
        .with_function("ropey", stable_ins_del::<RopeyRope>)
        .with_function("anrope", stable_ins_del::<AnRope>)
        .with_function("xirope", stable_ins_del::<XiRope>)
        .with_function("jumprope_c", stable_ins_del::<CRope>)
        .with_function("jumprope", stable_ins_del::<JumpRope>)
    ;

    c.bench("stable_ins_del", benchmark);
}

fn bench_simple(c: &mut Criterion) {
    c.bench_functions("simple", vec![
        Fun::new("ropey", stable_ins_del::<RopeyRope>),
        // Fun::new("anrope", stable_ins_del::<AnRope>),
        Fun::new("xirope", stable_ins_del::<XiRope>),
        Fun::new("jumprope", stable_ins_del::<JumpRope>),
        Fun::new("jumprope_c", stable_ins_del::<CRope>),
    ], 1000000);
}

criterion_group!(benches, bench_ins_append, bench_ins_random, bench_simple, bench_stable_ins_del);
// criterion_group!(benches, bench_all);
criterion_main!(benches);