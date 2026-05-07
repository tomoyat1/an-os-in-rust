#![cfg_attr(not(test), no_std)]
pub mod mapping;
pub mod physical;
pub mod table;
pub mod x86_64;

#[cfg(not(test))]
extern crate alloc;
#[cfg(not(test))]
extern crate core;

pub use mapping::*;
pub use table::*;
pub use x86_64::*;
