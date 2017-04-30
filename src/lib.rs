//! This crate provides a pure-rust library for reading ZIM files.
//!
//! ZIM files are a format used primarily to store wikis (such as Wikipedia and others based on
//! MediaWiki).
//!
//! For more into, see the [OpenZIM website](http://www.openzim.org/wiki/OpenZIM)
//!

extern crate byteorder;
extern crate memmap;
extern crate xz2;

mod cluster;
mod directory_entry;
mod directory_iterator;
mod errors;
mod mime_type;
mod target;
mod zim;

pub use target::Target;
pub use zim::Zim;
