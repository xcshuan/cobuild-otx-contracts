use std::path::{Path, PathBuf};

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn core_source_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rs_paths(&manifest_path("src"), &mut paths);
    paths.sort();
    paths
}

fn collect_rs_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in
        std::fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
    {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            collect_rs_paths(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

#[test]
fn core_source_does_not_import_entity_module() {
    for path in core_source_paths() {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let forbidden = ["cobuild_types", "entity"].join("::");
        assert!(
            !text.contains(&forbidden),
            "{} must not import {forbidden}",
            path.display()
        );
    }
}

#[test]
fn only_syscalls_module_imports_ckb_std() {
    let syscalls_path = manifest_path("src/syscalls.rs");
    for path in core_source_paths() {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let is_syscalls = path == syscalls_path;
        if is_syscalls {
            assert!(
                text.contains("ckb_std"),
                "syscalls.rs must own ckb_std access"
            );
        } else {
            assert!(
                !text.contains("ckb_std"),
                "{} must not import ckb_std directly",
                path.display()
            );
        }
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
    for path in core_source_paths() {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        assert!(
            !text.contains("unsafe"),
            "{} must not contain unsafe code",
            path.display()
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
