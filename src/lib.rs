//! # JumpRope
//!
//! A small, fast rope library for rust built on a skip list of gap buffers
//!
//! This library enables super fast in-memory string editing, where an edit might insert, delete
//! or modify text from anywhere in the string. Unlike inserting and deleting in a String directly,
//! jumprope avoids expensive memcopy / memmove operations. All editing operations are O(log n)
//! based on the size of the string.
//!
//! ## Example
//!
//! ```
//! use jumprope::JumpRope;
//!
//! let mut rope = JumpRope::from("Some large text document");
//! rope.insert(5, "really "); // "Some really large text document"
//! rope.replace(0..4, "My rad");  // "My rad really large text document"
//! assert_eq!(rope, "My rad really large text document");
//! ```
//!
//! See the [`JumpRope`] type for more usage details.
//!
//! # A rant on character lengths
//!
//! There are 3 different, useful ways to measure string lengths. All of them are useful in certain
//! situations:
//!
//! - The number of bytes needed to represent the string, in some specific encoding (eg UTF8)
//! - The number of unicode characters contained within
//! - The number of grapheme clusters in the string. This is the number of characters drawn to
//! the screen.
//!
//! For example, the unicode polar bear ("🐻‍❄️") has a single grapheme cluster (only one
//! character is drawn). It contains 4 unicode characters (Bear emoji + zero width joiner + snow
//! emoji + variation selector). And it takes 16 bytes to store in UTF8.
//!
//! ```
//! # use jumprope::*;
//! assert_eq!("🐻‍❄️".len(), 13);
//! assert_eq!("🐻‍❄️".chars().count(), 4);
//!
//! let rope = JumpRope::from("🐻‍❄️"); // One grapheme cluster
//! assert_eq!(rope.len(), 13); // 16 UTF8 bytes
//! assert_eq!(rope.len_chars(), 4); // 4 unicode characters
//! ```
//!
//! Worse, many popular languages (including javascript and C#) use UCS2 internally and thus their
//! `string.length` property doesn't give you a useful value for any application. Javascript reports
//! a snowman's length as 5 - which is useless:
//!
//! ```shell
//! $ node
//! Welcome to Node.js v16.6.1.
//! > "🐻‍❄️".length
//! 5
//! ```
//!
//! But there is no perfect "length" property for a string anyway:
//!
//! - The number of bytes is encoding-specific. The polar bear takes 16 bytes in UTF8, but only 10
//! bytes in UTF16.
//! - The number of grapheme clusters varies by device, font and software version. The conversion
//! from characters to grapheme clusters is complex, and changes all the time. The polar bear
//! icon was only added in May 2019. If your software is older than that (or uses a text library
//! older than that), you will just see "🐻❄️".
//!
//! Most CRDTs and OT systems are slowly standardizing on counting unicode character positions as
//! the default "length" property. The number of unicode characters isn't human-meaningful, but it
//! has a number of useful properties:
//!
//! - Its simple and easy to define
//! - Its stable across time (unlike grapheme clusters)
//! - Its rarely convenient, but its very portable across different programming languages,
//! regardless of that language's character encoding system.
//!
//! Jumprope follows this approach, using unicode character positions everywhere internally:
//!
//! ```
//! # use jumprope::*;
//! let mut rope = JumpRope::from("🐻‍❄️");
//! rope.remove(1..4); // Remove "polar" from our polar bear
//! assert_eq!(rope, "🐻");
//! ```

mod jumprope;
mod gapbuffer;
mod utils;
mod iter;
// mod params;

pub use crate::jumprope::JumpRope;