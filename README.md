# Jumprope

Because inserting into a string should be fast.

This is a simple, fast data structure for efficiently editing large strings in text editors and things like that.

Unlike traditional strings:

- You can efficiently insert or delete arbitrary keystrokes from anywhere in the document. Using real world editing traces, jumprope can process about 35-40 million edits per second.
- You can index into a document using unicode character offsets.

This library is similar to [ropey](https://crates.io/crates/ropey), which has more features and is more mature. However, ropey is about 3x slower than jumprope when processing real editing operations (see below) and compiles to a wasm bundle thats over twice as large. (Ropey is 30kb brotli compressed, vs 12kb for jumprope).

XiRope is 20x slower than jumprope. I love the Xi stuff but their rope implementation is poorly optimized.

---

This code is based on an older [skiplist based C rope library](https://github.com/josephg/librope) I wrote several years ago as an excuse to play with skip lists. It has a few notable differences:

- Instead of simply being implemented as a skiplist, jumprope is a skiplist where each leaf node contains a [Gap Buffer](https://en.wikipedia.org/wiki/Gap_buffer).
- Jumprope is faster with real data. On real world data sets, jumprope is over 2x as fast. For example, in the [seph-blog1 dataset](https://github.com/josephg/crdt-benchmarks), jumprope processes edits at 36Mops/sec (compared to librope with 13.6Mops/sec, or ropey with 10Mops/sec).
- Jumprope does not (currently) support wchar conversion present in librope. This is something that may change in time, especially given how useful it is in a wasm context.

I've uploaded some benchmarks of the different algorithms [here](https://home.seph.codes/public/rope_bench/report/). Using the real user typing datasets found [in crdt-benchmarks](https://github.com/josephg/crdt-benchmarks), document processing performance is as follows:

| Dataset | Ropey | librope (C) | Jumprope |
|---------|-------|-------------|----------|
automerge-paper | 27.84 ms | 16.3 ms | 7.24 ms
rustcode | 5.07 ms | 4.12 ms | 1.76 ms
sveltecomponent | 2.53 ms | 1.63 ms | 0.64 ms
seph-blog1 | 14.57 ms | 10.35 ms | 4.13 ms



# LICENSE

Licensed under the ISC license:

Copyright 2018 Joseph Gentle

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.