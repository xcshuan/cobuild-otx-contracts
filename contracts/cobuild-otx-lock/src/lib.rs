#![cfg_attr(not(feature = "library"), no_std)]
#![allow(special_module_name)]
#![allow(unused_attributes)]
#[cfg(feature = "library")]
mod main;
#[cfg(feature = "library")]
pub use main::program_entry;

extern crate alloc;
extern crate self as cobuild_otx_lock;

pub mod args;
mod chain;
pub mod entry;
pub mod error;
pub mod verify;
