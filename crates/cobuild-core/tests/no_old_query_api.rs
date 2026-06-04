use std::fs;
use std::path::PathBuf;

#[test]
fn core_no_longer_exports_old_lock_query_api() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let lib = fs::read_to_string(root.join("lib.rs")).expect("read lib.rs");

    assert!(!lib.contains("mod query"));
    assert!(!lib.contains("mod sighash"));
    assert!(!lib.contains("mod otx_request"));
    assert!(!lib.contains("pub mod signature"));
}
