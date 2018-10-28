
// #[macro_use]
extern crate criterion;
use criterion::*;

extern crate rand;
use rand::*;

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
fn random_ascii_string<R: Rng>(rng: &mut R, len: usize) -> String {
    let mut s = String::new();
    for _ in 0..len {
        s.push(*rng.choose(CHARS).unwrap() as char);
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

    fn insert_at(&mut self, pos: usize, contents: &str) { self.edit_str(pos, pos, contents); }
    fn del_at(&mut self, pos: usize, len: usize) { self.edit_str(pos, pos+len, ""); }

    fn to_string(&self) -> String {
        let mut output = String::new();
        self.push_to_string(&mut output);
        output
    }
    
    // fn len(&self) -> usize { self.len() } // in bytes
    fn char_len(&self) -> usize {
        let mut len = 0;
        for s in self.iter_chunks() {
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

fn bench_type<R: Rope + From<String>>(b: &mut Bencher, target_length: &usize) {
    let target_length = *target_length;
    let mut rng = prng::XorShiftRng::from_seed([1,2,3,4,1,2,3,4,1,2,3,4,1,2,3,4]);

    // I wish there was a better syntax for just making an array here.
    let mut strings = Vec::<String>::new();
    for _ in 0..100 {
        let len = rng.gen_range(1, 3);
        strings.push(random_ascii_string(&mut rng, len));
    }
    
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

fn bench_all(c: &mut Criterion) {
    let params = vec![
        1000,
        10000,
        100000,
        1000000,
        10000000,
    ];
    let benchmark = ParameterizedBenchmark::new("raw_string", bench_type::<String>, params)
        .with_function("ropey", bench_type::<RopeyRope>)
        .with_function("anrope", bench_type::<AnRope>)
        .with_function("xirope", bench_type::<XiRope>)
        .with_function("jumprope_c", bench_type::<CRope>)
        .with_function("jumprope", bench_type::<JumpRope>)
    ;

    c.bench("ropes", benchmark);
}

fn bench_simple(c: &mut Criterion) {
    c.bench_functions("simple", vec![
        Fun::new("ropey", bench_type::<RopeyRope>),
        // Fun::new("anrope", bench_type::<AnRope>),
        Fun::new("xirope", bench_type::<XiRope>),
        Fun::new("jumprope", bench_type::<JumpRope>),
        Fun::new("jumprope_c", bench_type::<CRope>),
    ], 1000000);
}

criterion_group!(benches, bench_all, bench_simple);
// criterion_group!(benches, bench_all);
criterion_main!(benches);