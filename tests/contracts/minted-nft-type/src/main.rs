#![cfg_attr(not(any(test, feature = "library")), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!();

pub fn program_entry() -> i8 {
    minted_nft_type::program_entry()
}
