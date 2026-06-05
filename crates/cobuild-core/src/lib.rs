#![no_std]
extern crate alloc;

pub mod context;
pub mod engine;
pub mod error;
mod hash;
pub mod layout;
pub mod plan;
pub mod protocol;
pub mod reader;
mod seal;
mod syscalls;
pub mod view;
pub mod witness;

pub fn bootstrap_marker() -> bool {
    true
}
