#![cfg_attr(not(test), no_std)]
pub mod physical;

#[cfg(not(test))]
extern crate alloc;
#[cfg(not(test))]
extern crate core;
