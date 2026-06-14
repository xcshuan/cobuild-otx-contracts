#![cfg_attr(not(feature = "library"), no_std)]

#[cfg(any(test, feature = "library"))]
extern crate alloc;

pub mod entry;
pub mod error;
pub mod types;
pub mod validation;

#[cfg(not(any(test, feature = "library")))]
ckb_std::entry!(program_entry);
#[cfg(not(any(test, feature = "library")))]
ckb_std::default_alloc!();

pub fn program_entry() -> i8 {
    match entry::main() {
        Ok(()) => 0,
        Err(error) => error.into(),
    }
}
