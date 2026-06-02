use std::fs;
use std::path::Path;

#[test]
fn workspace_declares_clean_cobuild_members() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("workspace manifest");
    for member in [
        "\"xtask\"",
        "\"crates/cobuild-types\"",
        "\"crates/cobuild-core\"",
        "\"contracts/cobuild-otx-lock\"",
        "\"tests\"",
    ] {
        assert!(
            manifest.contains(member),
            "missing workspace member {member}"
        );
    }
    assert!(
        !manifest.contains("[patch.crates-io]\ncritical-section"),
        "clean workspace must not patch critical-section"
    );
}
