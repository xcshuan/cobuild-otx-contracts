use std::path::{Path, PathBuf};

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

const CORE_SOURCE_PATHS: &[&str] = &[
    "src/context.rs",
    "src/hash/mod.rs",
    "src/hash/writer.rs",
    "src/layout.rs",
    "src/lib.rs",
    "src/message.rs",
    "src/otx_request.rs",
    "src/prepare.rs",
    "src/protocol.rs",
    "src/query.rs",
    "src/reader.rs",
    "src/seal.rs",
    "src/signature.rs",
    "src/sighash.rs",
    "src/source.rs",
    "src/view.rs",
    "src/witness.rs",
];

#[test]
fn core_source_does_not_import_entity_module() {
    for path in CORE_SOURCE_PATHS {
        let full_path = manifest_path(path);
        let text = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", full_path.display()));
        let forbidden = ["cobuild_types", "entity"].join("::");
        assert!(
            !text.contains(&forbidden),
            "{path} must not import {forbidden}"
        );
    }
}

#[test]
fn core_source_does_not_import_ckb_std() {
    for path in CORE_SOURCE_PATHS {
        let full_path = manifest_path(path);
        let text = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", full_path.display()));
        assert!(!text.contains("ckb_std"), "{path} must not import ckb_std");
    }
}

#[test]
fn view_does_not_publicly_expose_generated_inner_reader() {
    let path = manifest_path("src/view.rs");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(
        !text.contains("pub fn inner("),
        "view must not expose generated lazy-reader internals outside cobuild-core"
    );
}

#[test]
fn core_source_contains_no_unsafe() {
    for path in CORE_SOURCE_PATHS {
        let full_path = manifest_path(path);
        let text = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", full_path.display()));
        assert!(
            !text.contains("unsafe"),
            "{path} must not contain unsafe code"
        );
    }
}

#[test]
fn engine_prepare_does_not_cache_all_witness_byte_vectors() {
    let path = manifest_path("src/engine.rs");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));

    assert!(
        !text.contains("CachedWitnesses"),
        "engine prepare must not use an all-witness byte cache"
    );
    assert!(
        !text.contains("witness_summaries_and_bytes_from_source"),
        "engine prepare must not return compact summaries paired with cached witness bytes"
    );
    assert!(
        !text.contains("Vec<Vec<u8>>"),
        "engine prepare must not store all witness bytes as Vec<Vec<u8>>"
    );
}
