#![cfg_attr(not(feature = "library"), no_std)]

extern crate alloc;

pub mod entry;
pub mod error;
pub mod types;

pub fn program_entry() -> i8 {
    match entry::main() {
        Ok(()) => 0,
        Err(error) => error.into(),
    }
}
