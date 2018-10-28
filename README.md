# Rope in Rust

This is a straight rust port of my [C rope library](https://github.com/josephg/librope). Its mostly complete - although its missing wide character conversion

This library was largely written as a learning exercise, to compare high performance rust vs the equivalent C code. Interestingly, while application code written in rust seems to end up smaller than its C equivalent, this library has ended up about the same size. My hot take is that rust's expressive advantages don't seem to amount to much when implementing deep data structures.

That said, I suspect there's a way to use rust's generics to add wide character support, newline iteration, and stuff like that in a templated way. That would be a huge win over the C version, which is littered with #ifdefs.

I've uploaded [benchmarks here](https://josephg.com/ropereport/report/). Given [ropey](https://crates.io/crates/ropey) is both faster and more feature rich than this library, I'm not going to upload it to cargo or continue developing. Well played [@cessen](https://github.com/cessen).


# LICENSE

Licensed under the ISC license:

Copyright 2018 Joseph Gentle

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.