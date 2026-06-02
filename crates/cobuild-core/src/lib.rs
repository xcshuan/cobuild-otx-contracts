#![no_std]
extern crate alloc;

pub mod context;
pub mod error;
pub mod hash;
pub mod layout;
pub mod loader;
mod message;
mod otx_request;
pub mod protocol;
mod query;
mod seal;
mod sighash;
pub mod signature;
pub mod view;
pub mod witness;

pub fn bootstrap_marker() -> bool {
    true
}
