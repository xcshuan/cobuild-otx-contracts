#[test]
fn core_source_does_not_import_entity_module() {
    for path in [
        "src/context.rs",
        "src/hash.rs",
        "src/layout.rs",
        "src/lib.rs",
        "src/loader.rs",
        "src/tasks.rs",
        "src/view.rs",
        "src/witness.rs",
    ] {
        let text =
            std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let forbidden = ["cobuild_types", "entity"].join("::");
        assert!(
            !text.contains(&forbidden),
            "{path} must not import {forbidden}"
        );
    }
}

#[test]
fn view_does_not_publicly_expose_generated_inner_reader() {
    let text = std::fs::read_to_string(format!("{}/src/view.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("view source");
    assert!(
        !text.contains("pub fn inner("),
        "view must not expose generated lazy-reader internals outside cobuild-core"
    );
}
