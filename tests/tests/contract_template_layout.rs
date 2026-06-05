use std::{
    fs,
    path::{Path, PathBuf},
};

fn rust_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(dir, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display())) {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            collect_rust_source_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

fn joined_rust_source_text(dirs: &[&Path]) -> String {
    dirs.iter()
        .flat_map(|dir| rust_source_files(dir))
        .map(|path| fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}")))
        .collect::<Vec<_>>()
        .join("\n")
}

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
    let lock_chain_module = format!("{} {}", "mod", "chain");
    assert!(
        !lib_rs.contains(&lock_chain_module),
        "contract crate should not keep chain-loading helpers"
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
        "high_level::{load_script, load_script_hash}",
        "load_script()?",
        "AuthContext::try_from",
        "load_script_hash()?",
        "CobuildContext::from_syscalls",
        "plan_lock_validation(current_script_hash)",
        "required_signatures",
        "LocalVerifier",
    ] {
        assert!(
            entry_rs.contains(expected),
            "entry.rs should expose the high-level contract flow via {expected}"
        );
    }
    for forbidden in [
        format!("{}_{}", "from_lock", "args"),
        format!("{}_{}_{}", "load_current", "script", "args"),
        format!("{}_{}", "prepare_cobuild_from", "syscalls"),
        format!("{}{}", "PreparedCobuild", "Context"),
        format!("{}.{}", "context", "tx_reader"),
        format!("{}{}", "chain", "::"),
        "CobuildEngine".to_string(),
    ] {
        assert!(
            !entry_rs.contains(&forbidden),
            "entry.rs should not use removed wrapper {forbidden}"
        );
    }
}

#[test]
fn cobuild_core_owns_syscall_streaming_without_full_transaction_load() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        core_src.join("syscalls.rs").is_file(),
        "syscalls.rs must own syscall-backed streaming"
    );
    assert!(
        !lock_src.join("chain.rs").exists(),
        "lock crate must not own syscall-backed streaming"
    );

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    assert!(
        !syscalls_rs.contains("fn load_transaction() -> Result<Vec<u8>"),
        "core syscall path must not load the full transaction into Vec"
    );
    assert!(
        !syscalls_rs.contains("parse_transaction_info(&load_transaction()?"),
        "core syscall path must parse transaction from syscall cursor"
    );
    for expected in [
        "pub(crate) struct SyscallTxReader",
        "impl SyscallTxReader",
        "struct SyscallBackedReader",
        "fn syscall_cursor(",
        "fn hash_transaction_cursor(",
        "fn map_syscall_read_error(",
        "high_level::load_tx_hash()",
        "high_level::load_cell_lock_hash(",
        "high_level::load_cell_type_hash(",
    ] {
        assert!(
            syscalls_rs.contains(expected),
            "syscalls.rs should keep syscall streaming helper {expected}"
        );
    }
    for forbidden in [
        "pub(crate) fn counts(",
        "pub(crate) fn witness_cursor(",
        "pub(crate) fn raw_input_cursor(",
        "pub(crate) fn resolved_input_output_cursor(",
    ] {
        assert!(
            !syscalls_rs.lines().any(|line| line.starts_with(forbidden)),
            "syscall transaction access should be exposed through SyscallTxReader methods, not free helper {forbidden}"
        );
    }
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

    let plan_rs = fs::read_to_string(core_src.join("plan.rs")).expect("core plan.rs");
    for expected in [
        "SigningRequirement",
        "SignatureOrigin",
        "TxLevel",
        "OtxBase",
        "OtxAppend",
    ] {
        assert!(
            plan_rs.contains(expected),
            "core plan layer should expose lock signature planning name {expected}"
        );
    }

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    for module in ["tasks", "signature"] {
        assert!(
            !core_src.join(format!("{module}.rs")).exists(),
            "core should not keep removed {module}.rs"
        );
        assert!(
            !lib_rs.contains(&format!("pub mod {module}")),
            "core should not export removed module {module}"
        );
    }
    for module in ["query", "sighash", "otx_request"] {
        assert!(
            !core_src.join(format!("{module}.rs")).exists(),
            "core should not keep removed old query module {module}.rs"
        );
        assert!(
            !lib_rs.contains(&format!("mod {module}")),
            "core should not compile removed old query module {module}"
        );
    }

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
    assert!(
        lib_rs.contains("mod seal"),
        "core should keep seal.rs as a focused internal module"
    );
    assert!(core_src.join("seal.rs").is_file(), "missing seal.rs");
    assert!(
        !lib_rs.contains("mod message"),
        "message target validation should move onto TxScriptHashes"
    );
    assert!(
        !core_src.join("message.rs").exists(),
        "message.rs should be deleted after validation moves onto TxScriptHashes"
    );

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("core context.rs");
    for moved_fn in [
        "collect_sighash_all_signatures",
        "collect_otx_signatures",
        "unique_otx_base_seal",
    ] {
        assert!(
            !context_rs.contains(moved_fn),
            "context.rs should not own {moved_fn}"
        );
    }
    assert!(
        context_rs.contains("validate_message_targets"),
        "TxScriptHashes should own message target validation"
    );
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
        "ActionView",
        "MaskView",
        "bytes: Vec<u8>",
    ] {
        assert!(
            view_rs.contains(expected),
            "view.rs should expose cursor-backed view {expected}"
        );
    }
    for expected in [
        "pub fn actions(&self)",
        "pub fn actions_for(",
        "pub fn unique_action_for(",
    ] {
        assert!(
            view_rs.contains(expected),
            "MessageView should expose action query API via {expected}"
        );
    }
    assert!(
        !view_rs.contains("pub struct MaskView {\n    cursor: Cursor"),
        "MaskView should store compact mask bytes directly, not a cursor"
    );
    for expected in [
        "pub fn validate(&self, bit_count: usize)",
        "pub fn get(&self, index: usize)",
    ] {
        assert!(
            view_rs.contains(expected),
            "MaskView should own mask behavior via {expected}"
        );
    }
    let layout_rs = fs::read_to_string(core_src.join("layout.rs")).expect("layout.rs");
    assert!(
        !layout_rs.contains("fn validate_mask"),
        "layout.rs should delegate mask validation to MaskView"
    );
}

#[test]
fn cobuild_core_uses_concrete_syscall_reader_without_source_traits() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        core_src.join("syscalls.rs").is_file(),
        "cobuild-core must own syscall-backed transaction reading"
    );
    let removed_file = format!("{}{}", "source", ".rs");
    let transaction_source = format!("{}{}", "Transaction", "Source");
    let hash_input_source = format!("{}{}", "HashInput", "Source");
    assert!(
        !core_src.join(&removed_file).exists(),
        "{removed_file} must be removed with {transaction_source}/{hash_input_source}"
    );
    assert!(
        !lock_src.join("chain.rs").exists(),
        "lock crate must not keep syscall tx reader logic"
    );
    assert!(
        !lock_src.join("chain").exists(),
        "lock crate must not keep chain/reader.rs"
    );

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        lib_rs.contains("mod syscalls"),
        "core should keep syscall helpers internal"
    );
    let public_source_module = format!("{} {}", "pub mod", "source");
    assert!(
        !lib_rs.contains(&public_source_module),
        "core should not export source traits"
    );

    let all_relevant_text = joined_rust_source_text(&[&core_src, &lock_src]);
    for forbidden in [
        format!("{}{}", "Transaction", "Source"),
        format!("{}{}", "HashInput", "Source"),
        format!("{}{}", "InMemory", "Source"),
        format!("{}{}", "Classified", "Cursor"),
        format!("{}{}", "CursorRead", "Context"),
        format!("{}{}", "PreparedCobuild", "Context"),
        format!("{} {}", "mod", "chain"),
        format!("{}{}", "source", ".rs"),
    ] {
        assert!(
            !all_relevant_text.contains(&forbidden),
            "deleted abstraction must not remain: {forbidden}"
        );
    }
    for forbidden in [
        format!("{}{}", "WitnessCursor", "Source"),
        format!("{}{}", "<", "S:"),
        format!("{}{}", "source", ": &S"),
    ] {
        assert!(
            !all_relevant_text.contains(&forbidden),
            "core production path must not keep deleted source abstraction {forbidden}"
        );
    }

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    for expected in [
        "ckb_std",
        "pub(crate) struct SyscallTxReader",
        "impl SyscallTxReader",
        "SyscallBackedReader",
        "SyscallReadTarget",
        "fn counts(",
        "fn witness_cursor(",
        "fn raw_input_cursor(",
        "fn hash_transaction_cursor(",
        "fn resolved_input_output_cursor(",
        "fn input_lock_hash(",
    ] {
        assert!(
            syscalls_rs.contains(expected),
            "syscalls.rs should contain concrete reader implementation {expected}"
        );
    }
}

#[test]
fn cobuild_core_context_preparation_is_owned_by_engine_context() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    assert!(
        !core_src.join("prepare.rs").exists(),
        "unused prepare.rs should be deleted"
    );
    assert!(
        !core_src.join("loader.rs").exists(),
        "core loader.rs should not be reintroduced"
    );

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        !lib_rs.contains("pub mod prepare"),
        "core should not export unused prepare module"
    );
    assert!(
        !lib_rs.contains("pub mod loader"),
        "core should not export loader"
    );

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("core context.rs");
    assert!(
        context_rs.contains("pub struct TxScriptHashes"),
        "context.rs should expose TxScriptHashes"
    );
    assert!(
        !context_rs.contains("ScriptHashIndex"),
        "ScriptHashIndex should be removed"
    );

    let engine_rs = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs");
    assert!(
        engine_rs.contains("pub struct CobuildContext"),
        "engine.rs should expose CobuildContext"
    );
    assert!(
        engine_rs.contains("pub fn from_syscalls()"),
        "CobuildContext should own syscall preparation"
    );
}

#[test]
fn cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    assert!(
        syscalls_rs.contains("pub(crate) struct SyscallTxReader"),
        "syscall tx access should be owned by SyscallTxReader"
    );
    assert!(
        syscalls_rs.contains("impl SyscallTxReader"),
        "SyscallTxReader should expose concrete reader methods"
    );

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("context.rs");
    for expected in [
        "pub struct TxScriptHashes",
        "impl TxScriptHashes",
        "from_reader",
        "SyscallTxReader",
        "first_input_with_lock",
        "lock_in_input_range",
        "type_relation_for_otx",
        "lock_group_fully_covered_by_otx",
        "validate_message_targets",
    ] {
        assert!(
            context_rs.contains(expected),
            "TxScriptHashes should own script-hash flow method {expected}"
        );
    }

    let witness_rs = fs::read_to_string(core_src.join("witness.rs")).expect("witness.rs");
    for expected in [
        "pub(crate) struct WitnessScan",
        "enum WitnessSummary",
        "impl WitnessScan",
        "push_witness",
        "tx_level_carrier_has_sighash_all_layout",
        "unique_sighash_all_message",
        "unique_sighash_all_message_with_index",
    ] {
        assert!(
            witness_rs.contains(expected),
            "WitnessScan should own witness scan method {expected}"
        );
    }

    let engine_rs = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs");
    for expected in [
        "pub struct CobuildContext",
        "impl CobuildContext",
        "from_syscalls()",
        "let mut tx = SyscallTxReader::default();",
        "tx.preload_counts_from_syscalls()?;",
        "let counts = tx.counts();",
        "struct LockPlanBuilder",
        "LockPlanBuilder",
        "struct TypePlanBuilder",
        "TypePlanBuilder",
    ] {
        assert!(
            engine_rs.contains(expected),
            "engine.rs should expose concrete flow object {expected}"
        );
    }

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("lib.rs");
    assert!(
        !core_src.join("flow.rs").exists(),
        "flow.rs should be deleted after its logic moves onto TxScriptHashes"
    );
    assert!(
        !lib_rs.contains("mod flow"),
        "lib.rs should not keep the deleted flow module"
    );

    for forbidden in [
        "pub struct CobuildEngine;",
        "PreparedCobuild",
        "ScriptHashIndex",
        "crate::flow::",
        "TxCountsCache",
        "SyscallTxReader::with_counts",
    ] {
        assert!(
            !engine_rs.contains(forbidden),
            "engine.rs should not keep old scattered flow name {forbidden}"
        );
    }
}

#[test]
fn cobuild_core_lock_plan_exposes_related_messages() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let plan_rs = fs::read_to_string(core_src.join("plan.rs")).expect("plan.rs");
    let engine_rs = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs");

    assert!(
        plan_rs.contains("pub related_messages: Vec<RelatedMessage>"),
        "LockValidationPlan should expose related messages for input_lock actions"
    );
    assert!(
        engine_rs.contains("related_messages: Vec<RelatedMessage>"),
        "LockPlanBuilder should collect lock related messages"
    );
    assert!(
        engine_rs.contains("self.related_messages.push(RelatedMessage"),
        "lock planning should push tx-level or OTX related messages"
    );
}

#[test]
fn cobuild_core_hashing_uses_syscalls_not_owned_hash_parts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let hash_mod_rs = fs::read_to_string(core_src.join("hash/mod.rs")).expect("hash/mod.rs");
    let hash_writer_rs =
        fs::read_to_string(core_src.join("hash/writer.rs")).expect("hash/writer.rs");
    let hash_text = format!("{hash_mod_rs}\n{hash_writer_rs}");

    for forbidden in [
        "struct RawTxParts",
        "struct ResolvedInputHashPart",
        "struct SigningHashParts",
        "trailing_witnesses",
    ] {
        assert!(
            !hash_text.contains(forbidden),
            "hash module must not define {forbidden}"
        );
    }
    assert!(
        hash_mod_rs.contains("syscalls::"),
        "hash/mod.rs should hash through concrete syscall helpers"
    );
    let hash_input_source = format!("{}{}", "HashInput", "Source");
    assert!(
        !hash_mod_rs.contains(&hash_input_source),
        "hash/mod.rs must not keep {hash_input_source} generic hashing"
    );
    assert!(
        hash_mod_rs.contains("mod writer"),
        "hash/mod.rs should keep preimage writer helpers in hash/writer.rs"
    );
    for expected in [
        "writer::write_cursor_with_error",
        "writer::write_len_prefixed_cursor_with_error",
    ] {
        assert!(
            hash_mod_rs.contains(expected),
            "hash/mod.rs should write preimages through helper {expected}"
        );
    }
    for forbidden in [
        format!("{}{}", "Classified", "Cursor"),
        "write_len_prefixed_classified_cursor".to_string(),
    ] {
        assert!(
            !hash_writer_rs.contains(&forbidden),
            "hash/writer.rs must not keep deleted classified cursor helper {forbidden}"
        );
    }
    assert!(
        !core_src.join("hash.rs").exists(),
        "core should keep hashing in hash/mod.rs instead of flat hash.rs"
    );
}
