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