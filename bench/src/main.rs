
// #[macro_use]
extern crate criterion;
use criterion::*;

use crdt_testdata::*;

// extern crate rand;
// use rand::seq::IteratorRandom;
use rand::prelude::*;

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
fn random_ascii_string(rng: &mut SmallRng, len: usize) -> String {
    let mut s = String::new();
    for _ in 0..len {
        // s.push(*rng.choose(CHARS).unwrap() as char);
        s.push(CHARS[rng.gen_range(0 .. CHARS.len())] as char);
    }
    s
}

impl Rope for JumpRope {
    const NAME: &'static str = "JumpRope";

    fn new() -> Self { JumpRope::new() }

    fn insert_at(&mut self, pos: usize, contents: &str) { self.insert(pos, contents); }
    fn del_at(&mut self, pos: usize, len: usize) { self.remove(pos..pos+len); }
    fn edit_at(&mut self, pos: usize, del_len: usize, ins_content: &str) {
        self.replace(pos..pos+del_len, ins_content);
    }

    fn to_string(&self) -> String { ToString::to_string(self) }
    
    // fn len(&self) -> usize { self.len() } // in bytes
    fn char_len(&self) -> usize { self.len_chars() } // in unicode values
}

impl Rope for AnRope {
    const NAME: &'static str = "AnRope";

    fn new() -> Self { AnRope::new() }

    fn insert_at(&mut self, pos: usize, contents: &str) { *self = self.insert_str(pos, contents); }
    fn del_at(&mut self, pos: usize, len: usize) { *self = self.delete(pos..pos+len); }

    fn to_string(&self) -> String { ToString::to_string(self) }
    
    // fn len(&self) -> usize { self.len() } // in bytes
    fn char_len(&self) -> usize { self.len() } // in unicode values
}

impl Rope for XiRope {
    const NAME: &'static str = "XiRope";

    fn new() -> Self { XiRope::from("") }

    fn insert_at(&mut self, pos: usize, contents: &str) {
        self.edit(pos..pos, contents);
    }
    fn del_at(&mut self, pos: usize, len: usize) {
        self.edit(pos..pos+len, "");
    }
    fn edit_at(&mut self, pos: usize, del_len: usize, ins_content: &str) {
        self.edit(pos..pos+del_len, ins_content);
    }

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
    const NAME: &'static str = "Ropey";

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
use crdt_testdata::{load_testing_data, TestData};
use criterion::measurement::WallTime;

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
    const NAME: &'static str = "C-JumpRope";

    fn new() -> Self { unsafe { CRope(rope_new()) } }

    fn insert_at(&mut self, pos: usize, contents: &str) {
        unsafe {
            let cstr = CString::new(contents).unwrap();
            rope_insert(self.0, pos, cstr.as_ptr());
        }
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
        let cstr = CString::new(s).unwrap();
        CRope(unsafe { rope_new_with_utf8(cstr.as_ptr()) })
    }
}

#[test]
fn foo() {
    unsafe {
        let r = rope_new();
        println!("size {}", rope_char_count(r));
    }
}

fn gen_strings(rng: &mut SmallRng) -> Vec<String> {
    // I wish there was a better syntax for just making an array here.
    let mut strings = Vec::<String>::new();
    for _ in 0..100 {
        let len = rng.gen_range(1 .. 3);
        strings.push(random_ascii_string(rng, len));
    }

    strings
}

fn ins_append<R: Rope>(b: &mut Bencher) {
    let mut rng = SmallRng::seed_from_u64(123);
    let strings = gen_strings(&mut rng);

    let mut r = R::new();
    let mut len = 0;
    b.iter(|| {
        // let pos = rng.gen_range(0, len+1);
        let text = &strings[rng.gen_range(0 .. strings.len())];
        r.insert_at(len, text.as_str());
        len += text.chars().count();
    });

    black_box(r.char_len());
}

fn ins_random<R: Rope>(b: &mut Bencher) {
    let mut rng = SmallRng::seed_from_u64(123);
    let strings = gen_strings(&mut rng);

    let mut r = R::new();
    // Len isn't needed, but its here to allow direct comparison with ins_append.
    let mut len = 0;
    b.iter(|| {
        let pos = rng.gen_range(0 .. len+1);
        let text = &strings[rng.gen_range(0 .. strings.len())];
        r.insert_at(pos, text.as_str());
        len += text.chars().count();
    });

    black_box(r.char_len());
    black_box(len); 
}

fn stable_ins_del<R: Rope + From<String>>(b: &mut Bencher, target_length: &u64) {
    let target_length = *target_length as usize;
    let mut rng = SmallRng::seed_from_u64(123);

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
            let pos = rng.gen_range(0 .. len+1);
            let text = &strings[rng.gen_range(0 .. strings.len())];
            r.insert_at(pos, text.as_str());
            len += text.chars().count();
        } else {
            // Delete
            let pos = rng.gen_range(0 .. len);
            let dlen = min(rng.gen_range(0 .. 10), len - pos);
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

#[allow(unused)]
fn bench_ins_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("ins_append");

    group.bench_function("jumprope", ins_append::<JumpRope>);
    group.bench_function("ropey", ins_append::<RopeyRope>);
    // group.bench_function("anrope", ins_append::<AnRope>);
    group.bench_function("xirope", ins_append::<XiRope>);
    group.bench_function("jumprope_c", ins_append::<CRope>);
    group.bench_function("raw_string", ins_append::<String>);
    group.finish();
}

#[allow(unused)]
fn bench_ins_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("ins_random");

    group.bench_function("jumprope", ins_random::<JumpRope>);
    group.bench_function("ropey", ins_random::<RopeyRope>);
    // group.bench_function("anrope", ins_random::<AnRope>);
    group.bench_function("xirope", ins_random::<XiRope>);
    group.bench_function("jumprope_c", ins_random::<CRope>);
    group.bench_function("raw_string", ins_random::<String>);
    group.finish();
}

#[allow(unused)]
fn bench_stable_ins_del(c: &mut Criterion) {
    let mut group = c.benchmark_group("stable_ins_del");

    for size in [1000, 10000, 100000, 1000000, 10000000].iter() {
        group.throughput(Throughput::Elements(*size));
        group.bench_with_input(BenchmarkId::new("jumprope", size), size, stable_ins_del::<JumpRope>);
        group.bench_with_input(BenchmarkId::new("ropey", size), size, stable_ins_del::<RopeyRope>);
        // group.bench_with_input(BenchmarkId::new("anrope", size), size, stable_ins_del::<AnRope>);
        group.bench_with_input(BenchmarkId::new("xirope", size), size, stable_ins_del::<XiRope>);
        group.bench_with_input(BenchmarkId::new("jumprope_c", size), size, stable_ins_del::<CRope>);
    }
    group.finish();
}

fn load_named_data(name: &str) -> TestData {
    let filename = format!("/home/seph/src/diamond-types/benchmark_data/{}.json.gz", name);
    load_testing_data(&filename)
}

// const DATASETS: &[&str] = &["automerge-paper"];
const DATASETS: &[&str] = &["automerge-paper", "rustcode", "sveltecomponent", "seph-blog1"];

fn realworld(c: &mut Criterion) {
    for name in DATASETS {
        let mut group = c.benchmark_group("realworld");
        let test_data = load_named_data(name);
        group.throughput(Throughput::Elements(test_data.len() as u64));

        let mut all_ascii = true;
        for txn in &test_data.txns {
            for TestPatch(_pos, _del, ins) in &txn.patches {
                if ins.chars().count() != ins.len() { all_ascii = false; }
            }
        }

        fn x<R: Rope>(group: &mut BenchmarkGroup<WallTime>, name: &str, test_data: &TestData) {
            group.bench_function(BenchmarkId::new(R::NAME, name), |b| {
                b.iter(|| {
                    let mut r = R::new();
                    for txn in &test_data.txns {
                        for TestPatch(pos, del, ins) in &txn.patches {
                            r.edit_at(*pos, *del, ins);
                        }
                    }
                    assert_eq!(r.char_len(), test_data.end_content.len());
                    black_box(r.char_len());
                })
            });
        }

        x::<RopeyRope>(&mut group, name, &test_data);
        x::<JumpRope>(&mut group, name, &test_data);
        x::<CRope>(&mut group, name, &test_data);

        // These two crash on non-ascii characters for some reason.
        if all_ascii {
            // Extremely slow.
            x::<XiRope>(&mut group, name, &test_data);

            // Crashes.
            // x::<AnRope>(&mut group, name, &test_data);
        }

        // This takes a long time to run.
        // x::<String>(&mut group, name, &test_data);

        group.finish();
    }
}

criterion_group!(benches,
    bench_ins_append,
    bench_ins_random,
    bench_stable_ins_del,
    realworld
);
// criterion_group!(benches, bench_all);
criterion_main!(benches);