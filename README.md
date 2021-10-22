# Jumprope

Because inserting into a string should be fast.

This is a simple, fast data structure for efficiently editing large strings in text editors and things like that.

Unlike traditional strings:

- You can efficiently insert or delete arbitrary keystrokes from anywhere in the document. Using real world editing traces, jumprope can process about 35-40 million edits per second.
- You can index into a document using unicode character offsets.

This library is similar to [ropey](https://crates.io/crates/ropey), which has more features and is more mature. However, ropey is about 3x slower than jumprope when processing real editing operations (see below) and compiles to a wasm bundle thats over twice as large. (Ropey is 30kb brotli compressed, vs 12kb for jumprope).

[API documentation](https://docs.rs/jumprope/)

[Jumprope on crates.io](https://crates.io/crates/jumprope)

# Usage

JumpRope isn't a drop-in replacement for string, but it supports many similar methods. The most important additions are the [`insert`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.insert), [`remove`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.remove) and [`replace`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.replace) methods - which let you edit strings in-place in (typically) `log(n)` time relative to the size of the existing document.

```rust
use jumprope::JumpRope;

fn main() {
    let mut rope = JumpRope::from("Some large text document");
    rope.insert(5, "really "); // "Some really large text document"
    rope.replace(0..4, "My rad");  // "My rad really large text document"
    assert_eq!(rope, "My rad really large text document");

    // Extract to a string
    let s: String = rope.to_string();
    assert_eq!(s, "My rad really large text document");
}
```

You can read content back out of a rope by:

- Converting the rope to a string using `rope.to_string()` (requires allocations)
- Iterating over characters using [`rope.chars()`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.chars)
- (Fastest) iterating over &str chunks with [`rope.chunks()`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.chunks). This returns an iterator over `(&str, usize)` pairs. The second parameter names the number of unicode characters in the returned chunk - which can often be discarded depending on your use case.

If you want to read a subsection of the rope, you can use [`rope.slice_chunks()`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.slice_chunks) to read all the chunks within a given range in the rope. Eg:

```rust
fn main() {
    let rope = JumpRope::from("xxxGreetings!xxx");

    let string = rope.slice_chunks(3..13).map(|(str, _len)| str).collect::<String>();
    assert_eq!(string, "Greetings!");
}
```

Or more simply, with [`rope.slice_chars`](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html#method.slice_chars):

```rust
fn main() {
    let rope = JumpRope::from("xxxGreetings!xxx");

    let string = rope.slice_chars(3..13).collect::<String>();
    assert_eq!(string, "Greetings!");
}
```

I think this might be slower? I'm not sure. Needs more benchmarking!

For more detail, see [JumpRope API documentation](https://docs.rs/jumprope/latest/jumprope/struct.JumpRope.html)


## History / motivation

This code is based on an older [skiplist based C rope library](https://github.com/josephg/librope) I wrote several years ago as an excuse to play with skip lists. It has a few notable differences:

- Instead of simply being implemented as a skiplist, jumprope is a skiplist where each leaf node contains a [Gap Buffer](https://en.wikipedia.org/wiki/Gap_buffer).
- Jumprope is faster. (See table below)
- Jumprope does not (currently) support wchar conversion present in librope. This is something that may change in time, especially given how useful it is in a wasm context.


## Benchmarks

Running the [editing traces from crdt-benchmarks](https://github.com/josephg/crdt-benchmarks), jumprope is faster than any other library in cargo that I know of:

Running on a single core of a Ryzen 5800X:

| Dataset | Raw string | XiRope | Ropey | librope (C) | Jumprope |
|---------|------------|--------|-------|-------------|----------|
automerge-paper | 3908.13 ms | 518.75 ms | 25.16 ms | 16.28 ms | 6.66 ms
rustcode | 569.44 ms | DNF | 4.71 ms | 3.93 ms | 1.66 ms
sveltecomponent | 41.05 ms | 24.83 ms | 2.31 ms | 1.59 ms | 0.59 ms
seph-blog1 | 1238.44 ms | DNF | 13.04 ms | 10.01 ms | 3.81 ms

Full criterion report is [here](https://home.seph.codes/public/rope_bench/report/).

I tried AnRope as well, but couldn't get it to process these datasets correctly at all.


# LICENSE

Licensed under the ISC license:

Copyright 2018 Joseph Gentle

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.