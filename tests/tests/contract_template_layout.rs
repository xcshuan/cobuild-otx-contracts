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
        !lib_rs.contains("mod chain"),
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
        "CobuildEngine::prepare_from_syscalls",
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
        "from_lock_args",
        "load_current_script_args",
        "prepare_cobuild_from_syscalls",
        "PreparedCobuildContext",
        "context.tx_reader",
        "chain::",
    ] {
        assert!(
            !entry_rs.contains(forbidden),
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
        "struct SyscallBackedReader",
        "fn syscall_cursor(",
        "fn transaction_cursor(",
        "fn resolved_input_output_cursor(",
        "fn resolved_input_data_cursor(",
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
    for module in ["message", "seal"] {
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
fn cobuild_core_uses_concrete_syscall_reader_without_source_traits() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        core_src.join("syscalls.rs").is_file(),
        "cobuild-core must own syscall-backed transaction reading"
    );
    assert!(
        !core_src.join("source.rs").exists(),
        "source.rs must be removed with TransactionSource/HashInputSource"
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
    assert!(
        !lib_rs.contains("pub mod source"),
        "core should not export source traits"
    );

    let core_text = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs")
        + &fs::read_to_string(core_src.join("hash/mod.rs")).expect("hash/mod.rs")
        + &fs::read_to_string(core_src.join("hash/writer.rs")).expect("hash/writer.rs")
        + &fs::read_to_string(core_src.join("layout.rs")).expect("layout.rs");
    for forbidden in [
        "TransactionSource",
        "HashInputSource",
        "InMemorySource",
        "ClassifiedCursor",
        "CursorReadContext",
        "WitnessCursorSource",
        "<S:",
        "source: &S",
    ] {
        assert!(
            !core_text.contains(forbidden),
            "core production path must not keep deleted source abstraction {forbidden}"
        );
    }

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    for expected in [
        "ckb_std",
        "SyscallBackedReader",
        "SyscallReadTarget",
        "pub(crate) fn counts(",
        "pub(crate) fn witness_cursor(",
        "pub(crate) fn raw_input_cursor(",
        "pub(crate) fn resolved_input_output_cursor(",
        "pub(crate) fn input_lock_hash(",
    ] {
        assert!(
            syscalls_rs.contains(expected),
            "syscalls.rs should expose concrete helper {expected}"
        );
    }
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
        hash_mod_rs.contains("crate::syscalls"),
        "hash/mod.rs should hash through concrete syscall helpers"
    );
    assert!(
        !hash_mod_rs.contains("HashInputSource"),
        "hash/mod.rs must not keep HashInputSource generic hashing"
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
        "ClassifiedCursor",
        "write_len_prefixed_classified_cursor",
    ] {
        assert!(
            !hash_writer_rs.contains(forbidden),
            "hash/writer.rs must not keep deleted classified cursor helper {forbidden}"
        );
    }
    assert!(
        !core_src.join("hash.rs").exists(),
        "core should keep hashing in hash/mod.rs instead of flat hash.rs"
    );
}
