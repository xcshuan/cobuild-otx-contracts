#![no_std]
extern crate alloc;

pub mod context;
pub mod error;
pub mod hash;
pub mod layout;
pub mod loader;
pub mod tasks;
pub mod view;
pub mod witness;

pub fn bootstrap_marker() -> bool {
    true
}
