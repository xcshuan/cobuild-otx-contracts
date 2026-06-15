#![cfg_attr(not(feature = "library"), no_std)]
#![allow(special_module_name)]
#![allow(unused_attributes)]
#[cfg(feature = "library")]
mod main;
#[cfg(feature = "library")]
pub use main::program_entry;

extern crate alloc;
extern crate self as limit_order_lock;

pub mod entry;
pub mod error;
pub mod input;
pub mod otx;
pub mod settlement;
pub mod types;
pub mod validation;
