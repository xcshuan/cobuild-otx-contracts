use std::{fs, path::Path};

#[test]
fn cobuild_otx_lock_uses_ckb_contract_template_scaffold() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let contract_dir = workspace_root.join("contracts/cobuild-otx-lock");

    for path in [".gitignore", "README.md", "Makefile", "Cargo.toml"] {
        assert!(
            contract_dir.join(path).is_file(),
            "missing template file {path}"
        );
    }

    let manifest = fs::read_to_string(contract_dir.join("Cargo.toml")).expect("contract manifest");
    assert!(
        manifest.contains("ckb-std"),
        "contract must depend on ckb-std"
    );
    assert!(
        manifest.contains("native-simulator"),
        "contract must expose native-simulator feature"
    );

    let main_rs = fs::read_to_string(contract_dir.join("src/main.rs")).expect("main.rs");
    assert!(
        main_rs.contains("ckb_std::entry!(program_entry);"),
        "contract main must use ckb_std entry macro"
    );
    assert!(
        main_rs.contains("ckb_std::default_alloc!"),
        "contract main must configure ckb-std allocator"
    );
}

#[test]
fn cobuild_otx_lock_entry_owns_contract_flow() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let contract_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        !contract_src.join("runner.rs").exists(),
        "runner.rs must not own the contract entry flow"
    );

    let lib_rs = fs::read_to_string(contract_src.join("lib.rs")).expect("lib.rs");
    assert!(
        lib_rs.contains("mod chain"),
        "contract crate should keep chain-loading helpers in chain.rs"
    );
    assert!(
        !lib_rs.contains("mod loader"),
        "contract crate should not keep the old loader module name"
    );
    assert!(
        !lib_rs.contains("pub mod runner"),
        "contract crate must not export a runner module"
    );

    let entry_rs = fs::read_to_string(contract_src.join("entry.rs")).expect("entry.rs");
    assert!(
        !entry_rs.contains("runner::run"),
        "entry.rs must not delegate the full contract flow to runner::run"
    );
    assert!(
        !entry_rs.contains("tx_tasks") && !entry_rs.contains("otx_tasks"),
        "entry.rs must consume unified signature requests instead of separate source branches"
    );
    for expected in [
        "parse_auth_args",
        "load_current_script_args",
        "load_script_hash",
        "load_prepared_context",
        "plan_lock_validation",
        "required_signatures",
        "loaded.source",
        "LocalVerifier",
    ] {
        assert!(
            entry_rs.contains(expected),
            "entry.rs should expose the high-level contract flow via {expected}"
        );
    }
}

#[test]
fn cobuild_otx_lock_streams_chain_data_without_full_transaction_load() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");
    assert!(
        lock_src.join("chain.rs").is_file(),
        "chain.rs must own syscall-backed source"
    );
    assert!(
        !lock_src.join("loader.rs").exists(),
        "loader.rs should be renamed to chain.rs"
    );
    let chain_rs = fs::read_to_string(lock_src.join("chain.rs")).expect("chain.rs");
    let chain_reader_rs =
        fs::read_to_string(lock_src.join("chain/reader.rs")).expect("chain/reader.rs");
    assert!(
        chain_rs.contains("struct ChainSource"),
        "chain.rs should define ChainSource"
    );
    assert!(
        chain_rs.contains("mod reader"),
        "chain.rs should keep source-backed reader details in chain/reader.rs"
    );
    assert!(
        !chain_rs.contains("fn load_transaction() -> Result<Vec<u8>"),
        "lock path must not load the full transaction into Vec"
    );
    assert!(
        !chain_rs.contains("parse_transaction_info(&load_transaction()?"),
        "lock path must parse transaction from source cursor"
    );
    for expected in [
        "struct SourceBackedReader",
        "fn chain_cursor(",
        "fn transaction_cursor(",
        "fn script_cursor(",
        "fn resolved_input_cell_cursor(",
        "fn resolved_input_data_cursor(",
    ] {
        assert!(
            chain_reader_rs.contains(expected),
            "chain/reader.rs should expose source-backed lazy helper {expected}"
        );
    }
    assert!(
        chain_reader_rs.contains("fn map_source_read_error("),
        "chain/reader.rs should make source-backed read error mapping explicit"
    );
    assert!(
        !chain_rs.contains("struct SourceBackedReader"),
        "chain.rs should not own source-backed reader internals"
    );
    for expected in [
        "high_level::load_script()",
        "high_level::load_script_hash()",
        "high_level::load_tx_hash()",
        "high_level::load_cell_lock_hash(",
        "high_level::load_cell_type_hash(",
    ] {
        assert!(
            chain_rs.contains(expected),
            "chain.rs should use high-level fixed/owned load helper {expected}"
        );
    }
    assert!(
        !chain_rs.contains("fn load_owned("),
        "chain.rs should not reintroduce generic owned syscall loading"
    );

    let core_prepare_rs =
        fs::read_to_string(workspace_root.join("crates/cobuild-core/src/prepare.rs"))
            .expect("prepare.rs");
    assert!(
        core_prepare_rs.contains("pub struct SourcePreparedContext"),
        "source-backed prepare should return an explicit context-only type"
    );
    assert!(
        !core_prepare_rs.contains("InMemorySource::default()"),
        "source-backed prepare must not hide a dummy signing_source"
    );
}

#[test]
fn cobuild_core_uses_explicit_signature_request_names() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    let view_rs = fs::read_to_string(core_src.join("view.rs")).expect("core view.rs");
    assert!(
        !view_rs.contains("TxLevel"),
        "core view layer should describe sighash-all witness layout, not legacy transaction-level names"
    );
    for expected in [
        "SighashAllWitnessView",
        "WithMessage",
        "SealOnly",
        "sighash_all_witness_layout",
    ] {
        assert!(
            view_rs.contains(expected),
            "core view layer should expose explicit witness layout name {expected}"
        );
    }

    let signature_rs =
        fs::read_to_string(core_src.join("signature.rs")).expect("core signature.rs");
    for expected in [
        "SignatureRequest",
        "SignatureOrigin",
        "SighashAll",
        "OtxBase",
        "OtxAppend",
    ] {
        assert!(
            signature_rs.contains(expected),
            "core signature layer should expose unified signature request name {expected}"
        );
    }

    assert!(
        !core_src.join("tasks.rs").exists(),
        "core should use signature.rs instead of tasks.rs"
    );
    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        !lib_rs.contains("pub mod tasks"),
        "core should not export the old tasks module"
    );

    let protocol_rs = fs::read_to_string(core_src.join("protocol.rs")).expect("core protocol.rs");
    for expected in ["ScriptRole", "SealScope", "AppendPermissions"] {
        assert!(
            protocol_rs.contains(expected),
            "core protocol layer should expose typed protocol value {expected}"
        );
    }
    assert!(
        lib_rs.contains("pub mod protocol"),
        "core should export the protocol value module"
    );
    for module in ["message", "otx_request", "query", "seal", "sighash"] {
        assert!(
            lib_rs.contains(&format!("mod {module}")),
            "core should keep {module}.rs as a focused internal module"
        );
        assert!(
            core_src.join(format!("{module}.rs")).is_file(),
            "missing focused core module {module}.rs"
        );
    }

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("core context.rs");
    for moved_fn in [
        "collect_sighash_all_signatures",
        "collect_otx_signatures",
        "validate_message_targets",
        "unique_otx_base_seal",
    ] {
        assert!(
            !context_rs.contains(moved_fn),
            "context.rs should not own {moved_fn}"
        );
    }
}

#[test]
fn cobuild_core_reader_helpers_are_not_owned_by_view() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    assert!(
        core_src.join("reader.rs").is_file(),
        "reader.rs must own cursor helpers"
    );
    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        lib_rs.contains("pub mod reader"),
        "core should export reader helpers"
    );

    let helper_definitions = [
        ("pub struct OwnedReader", "struct OwnedReader"),
        ("pub fn cursor_from_slice(", "fn cursor_from_slice("),
        ("pub fn cursor_bytes(", "fn cursor_bytes("),
        ("pub fn update_cursor(", "fn update_cursor("),
        (
            "pub fn update_cursor_with_error(",
            "fn update_cursor_with_error(",
        ),
        (
            "pub fn update_len_prefixed_cursor(",
            "fn update_len_prefixed_cursor(",
        ),
    ];

    let reader_rs = fs::read_to_string(core_src.join("reader.rs")).expect("reader.rs");
    for (reader_definition, _) in helper_definitions {
        assert!(
            reader_rs.contains(reader_definition),
            "reader.rs should define {reader_definition}"
        );
    }

    let view_rs = fs::read_to_string(core_src.join("view.rs")).expect("view.rs");
    for (_, view_definition) in helper_definitions {
        assert!(
            !view_rs.contains(view_definition),
            "view.rs must not define {view_definition}"
        );
    }
}

#[test]
fn cobuild_core_view_is_cursor_backed_protocol_boundary() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let view_rs = fs::read_to_string(core_src.join("view.rs")).expect("view.rs");

    for forbidden in [
        "OtxStartData",
        "OtxData",
        "SealPairData",
        "ActionData",
        "message: Vec<u8>",
        "base_input_masks: Vec<u8>",
        "seal: Vec<u8>",
    ] {
        assert!(
            !view_rs.contains(forbidden),
            "view.rs should not expose owned DTO pattern {forbidden}"
        );
    }

    for expected in [
        "SighashAllWitnessView",
        "OtxStartView",
        "OtxView",
        "SealPairView",
        "MessageActionView",
        "MaskView",
    ] {
        assert!(
            view_rs.contains(expected),
            "view.rs should expose cursor-backed view {expected}"
        );
    }
}

#[test]
fn cobuild_core_exposes_source_boundary_without_ckb_std() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    assert!(
        core_src.join("source.rs").is_file(),
        "source.rs must own source traits"
    );

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        lib_rs.contains("pub mod source"),
        "core should export source traits"
    );

    let source_rs = fs::read_to_string(core_src.join("source.rs")).expect("source.rs");
    for expected in [
        "ClassifiedCursor",
        "CursorReadContext",
        "TransactionSource",
        "TxCounts",
        "HashInputSource",
        "InMemorySource",
    ] {
        assert!(
            source_rs.contains(expected),
            "source.rs should define {expected}"
        );
    }
    for expected in [
        "CursorReadContext::Protocol => CoreError::MalformedCobuild",
        "CursorReadContext::SourceInput => CoreError::InvalidContextInput",
        "CursorReadContext::HashInput => CoreError::MissingHashInput",
    ] {
        assert!(
            source_rs.contains(expected),
            "source.rs should map read errors via {expected}"
        );
    }
    for expected in [
        "fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError>",
        "fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError>",
        "fn tx_hash(&self) -> Result<[u8; 32], CoreError>",
        "fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError>",
        "fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>",
        "fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>",
        "fn counts(&self) -> Result<TxCounts, CoreError>",
        "fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>",
        "fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>",
    ] {
        assert!(
            source_rs.contains(expected),
            "source.rs should expose source API {expected}"
        );
    }
    assert!(
        !source_rs.contains("ckb_std"),
        "core source boundary must not import ckb_std"
    );
}

#[test]
fn cobuild_core_prepares_context_in_prepare_module() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    assert!(
        core_src.join("prepare.rs").is_file(),
        "prepare.rs must own context preparation"
    );
    assert!(
        !core_src.join("loader.rs").exists(),
        "core loader.rs should be renamed to prepare.rs"
    );

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        lib_rs.contains("pub mod prepare"),
        "core should export prepare"
    );
    assert!(
        !lib_rs.contains("pub mod loader"),
        "core should not export loader"
    );

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("core context.rs");
    assert!(
        context_rs.contains("ScriptHashIndex"),
        "context.rs should expose ScriptHashIndex"
    );
    assert!(
        !context_rs.contains("TxScriptHashes"),
        "context.rs should not expose TxScriptHashes"
    );
}

#[test]
fn cobuild_core_hashing_uses_source_not_owned_hash_parts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let hash_rs = fs::read_to_string(core_src.join("hash.rs")).expect("hash.rs");

    for forbidden in [
        "struct RawTxParts",
        "struct ResolvedInputHashPart",
        "struct SigningHashParts",
        "trailing_witnesses",
    ] {
        assert!(
            !hash_rs.contains(forbidden),
            "hash.rs must not define {forbidden}"
        );
    }
    assert!(
        hash_rs.contains("HashInputSource"),
        "hash.rs should hash through HashInputSource"
    );
}
