#![no_std]
extern crate alloc;

pub mod error;
pub mod view;
pub mod witness;

pub fn bootstrap_marker() -> bool {
    true
}
