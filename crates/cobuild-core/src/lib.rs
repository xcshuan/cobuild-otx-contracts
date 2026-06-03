#![no_std]
extern crate alloc;

pub mod context;
pub mod error;
pub mod hash;
pub mod layout;
mod message;
mod otx_request;
pub mod prepare;
pub mod protocol;
mod query;
pub mod reader;
mod seal;
mod sighash;
pub mod signature;
pub mod source;
pub mod view;
pub mod witness;

pub fn bootstrap_marker() -> bool {
    true
}
