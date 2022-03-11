# Rust rope benchmarks

This is a small collection of benchmarks of various rope implementations in rust.

I'm comparing:

- Jumprope (this library)
- The [C version of this rope library](https://github.com/josephg/librope)
- [ropey](https://crates.io/crates/ropey/)
- [xi-rope](https://crates.io/crates/xi-rope)
- [an-rope](https://crates.io/crates/an-rope)

To run the benchmarks, navigate into this directory and run:

```
cargo run --release -- --bench
```

This will produce a report in *target/criterion/report/index.html*.

Current benchmark results are published [here](https://home.seph.codes/public/c4/report/)