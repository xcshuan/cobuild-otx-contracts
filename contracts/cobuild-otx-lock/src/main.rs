#![cfg_attr(all(not(feature = "library"), target_arch = "riscv64"), no_std)]
#![cfg_attr(all(not(test), target_arch = "riscv64"), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

#[cfg(all(not(any(feature = "library", test)), target_arch = "riscv64"))]
ckb_std::entry!(program_entry);
#[cfg(all(not(any(feature = "library", test)), target_arch = "riscv64"))]
ckb_std::default_alloc!(16384, 1258306, 64);

use cobuild_otx_lock as contract_crate;

pub fn program_entry() -> i8 {
    match contract_crate::entry::main() {
        Ok(()) => 0,
        Err(err) => err.into(),
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[cfg_attr(feature = "library", allow(dead_code))]
fn main() {
    std::process::exit(i32::from(program_entry()));
}
