#![cfg_attr(not(feature = "library"), no_std)]
#![allow(special_module_name)]
#![allow(unused_attributes)]
#[cfg(feature = "library")]
mod main;
#[cfg(feature = "library")]
pub use main::program_entry;

extern crate alloc;

pub fn bootstrap_marker() -> bool {
    cobuild_core::bootstrap_marker()
}
