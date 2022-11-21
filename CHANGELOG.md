# 1.1.1

- Fixed bug where reflexive eq (a == a) would fail for `&JumpRopeBuf`.

# 1.1.0

- The JumpRopeBuf feature has a lot more methods and is now stable, and included by default. The `buffered` feature flag is no longer needed. It now has no effect, and it will be removed in JumpRope 2.0 (whenever that happens). Please file issues if other useful methods are missing.
- Added Send and Sync markers to `JumpRope`. Thanks to P. Vijay for the suggestion!

# 1.0.0

- Woohoo!
- **Breaking API change**: Renamed the iterator methods. `rope.chunks()` -> `rope.substrings_with_len()`. Added `rope.substrings()` and `rope.slice_substrings()`.
- Added buffered API, though for now its experimental and behind a feature flag.
- Made miri pass against jumprope. This involved some changes:
  - The dynamically allocated heights in node.nexts lists have been removed. This results in less unsafe code, but increases the memory overhead of the library.
  - Wasm bundle size has grown
  - Performance is mostly unaffected.
- Bumped to str_indices 0.3.2
- Added Eq trait support to all the combinations of `rope` / `&rope` vs `&str` / `String` / `&String`.


# 0.5.3

- Made Jumprope::new() use a hardcoded seed when ddos_protection is disabled. This makes the module 5kb smaller in wasm and avoids getrandom.

# 0.5.2

- Swapped from inlined string methods to [`str_indices`](https://crates.io/crates/str_indices). Thanks @cessen!

# 0.5.1

- Only cosmetic (documentation) changes.

# 0.5.0

- Added support for wchar based indexing, behind a feature flag. (See documentation for details)
- General performance improvements
- Removed ropey as an explicit dependency, inlining the borrowed methods (for now).

# 0.4.0

- Breaking API change: Renamed `rope.len()` to `rope.len_bytes()`
- Added `rope.mem_size() -> usize` method for debugging

# 0.3.1

- Fixed a few critical bugs in iterator code which caused slice_chars() to return incorrect results or crash

# 0.3.0

- Added iterator support (to iterate by character range)
- Added proper rustdocs for core methods