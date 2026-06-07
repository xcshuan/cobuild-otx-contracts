use ckb_testtool::{
    ckb_error::Error,
    ckb_types::{
        bytes::Bytes,
        core::{Cycle, TransactionView},
    },
    context::Context,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[cfg(test)]
mod tests;

pub mod framework;

// The exact same Loader code from capsule's template, except that
// now we use MODE as the environment variable
const TEST_ENV_VAR: &str = "MODE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestEnv {
    Debug,
    Release,
}

impl FromStr for TestEnv {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(TestEnv::Debug),
            "release" => Ok(TestEnv::Release),
            _ => Err("no match"),
        }
    }
}

pub struct Loader(PathBuf);

pub fn default_test_env() -> TestEnv {
    TestEnv::Debug
}

impl Default for Loader {
    fn default() -> Self {
        let test_env = match env::var(TEST_ENV_VAR) {
            Ok(val) => val.parse().expect("test env"),
            Err(_) => default_test_env(),
        };
        Self::with_test_env(test_env)
    }
}

impl Loader {
    fn with_test_env(env: TestEnv) -> Self {
        let load_prefix = match env {
            TestEnv::Debug => "debug",
            TestEnv::Release => "release",
        };
        let mut base_path = match env::var("TOP") {
            Ok(val) => {
                let mut base_path: PathBuf = val.into();
                base_path.push("build");
                base_path
            }
            Err(_) => {
                let mut base_path = PathBuf::new();
                // cargo may use a different cwd when running tests, for example:
                // when running debug in vscode, it will use workspace root as cwd by default,
                // when running test by `cargo test`, it will use tests directory as cwd,
                // so we need a fallback path
                base_path.push("build");
                if !base_path.exists() {
                    base_path.pop();
                    base_path.push("..");
                    base_path.push("build");
                }
                base_path
            }
        };

        base_path.push(load_prefix);
        Loader(base_path)
    }

    pub fn load_binary(&self, name: &str) -> Bytes {
        let mut path = self.0.clone();
        path.push(name);
        let result = fs::read(&path);
        if result.is_err() {
            panic!("Binary {path:?} is missing!");
        }
        result.unwrap().into()
    }
}

// This helper method runs Context::verify_tx, but in case error happens,
// it also dumps current transaction to failed_txs folder.
pub fn verify_and_dump_failed_tx(
    context: &Context,
    tx: &TransactionView,
    max_cycles: u64,
) -> Result<Cycle, Error> {
    let result = context.verify_tx(tx, max_cycles);
    if result.is_err() {
        let mut path = env::current_dir().expect("current dir");
        path.push("failed_txs");
        std::fs::create_dir_all(&path).expect("create failed_txs dir");
        let mock_tx = context.dump_tx(tx).expect("dump failed tx");
        let json = serde_json::to_string_pretty(&mock_tx).expect("json");
        path.push(format!("0x{:x}.json", tx.hash()));
        println!("Failed tx written to {path:?}");
        std::fs::write(path, json).expect("write");
    }
    result
}

pub mod fixtures {
    use blake2b_ref::Blake2bBuilder;
    use ckb_testtool::{
        builtin::ALWAYS_SUCCESS,
        ckb_error::Error,
        ckb_types::{
            bytes::Bytes,
            core::{Cycle, ScriptHashType, TransactionBuilder, TransactionView},
            packed::{CellDep, CellInput, CellOutput},
            prelude::*,
        },
        context::Context,
    };
    use cobuild_core::{
        layout::{OtxLayout, Range},
        reader::{cursor_from_slice, update_cursor_with_error},
        view::{MaskView, OtxView},
    };
    use cobuild_types::entity::{
        core::{ActionVec, Message as CobuildMessage, Otx, OtxStart, SealPair, SighashAllOnly},
        witness::WitnessLayout,
    };
    use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

    use crate::Loader;

    const TX_WITHOUT_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_tnm_core1\0";
    const OTX_BASE_PERSONAL: &[u8; 16] = b"ckbcb_otb_core1\0";
    const OTX_APPEND_PERSONAL: &[u8; 16] = b"ckbcb_ota_core1\0";

    pub struct Case {
        context: Context,
        tx: TransactionView,
    }

    impl Case {
        pub fn verify(self) -> Result<Cycle, Error> {
            self.context.verify_tx(&self.tx, 50_000_000)
        }
    }

    pub fn invalid_args_case() -> Case {
        build_case(Bytes::from(vec![0u8]))
    }

    pub fn no_relevant_signature_request_case() -> Case {
        let mut args = vec![0u8];
        args.extend_from_slice(&[1u8; 20]);
        build_case(Bytes::from(args))
    }

    pub fn signed_sighash_all_case() -> Case {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

        let mut args = vec![0u8];
        args.extend_from_slice(&public_key_hash[..20]);

        let mut context = Context::default();
        let contract_bin = Loader::default().load_binary("cobuild-otx-lock");
        let contract_out_point = context.deploy_cell(contract_bin);
        let contract_dep = CellDep::new_builder()
            .out_point(contract_out_point.clone())
            .build();
        let lock = context
            .build_script_with_hash_type(&contract_out_point, ScriptHashType::Data2, args.into())
            .expect("build cobuild-otx-lock script");
        let input_output = CellOutput::new_builder()
            .capacity(100_000_000_000u64)
            .lock(lock)
            .build();
        let input_out_point = context.create_cell(input_output.clone(), Bytes::new());
        let output = CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(always_success_script(&mut context))
            .build();
        let unsigned_tx = TransactionBuilder::default()
            .cell_dep(contract_dep)
            .input(
                CellInput::new_builder()
                    .previous_output(input_out_point)
                    .build(),
            )
            .output(output)
            .output_data(Bytes::new().pack())
            .witness(Bytes::new().pack())
            .build();

        let signing_message_hash = tx_without_message_hash(
            packed_hash_to_array(unsigned_tx.hash()),
            1,
            input_output.as_slice(),
            &[Vec::new()],
        );
        let seal = sign_recoverable(&secp, &secret_key, signing_message_hash);
        let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
        let tx = unsigned_tx
            .as_advanced_builder()
            .set_witnesses(vec![Bytes::copy_from_slice(witness.as_slice()).pack()])
            .build();

        Case { context, tx }
    }

    pub fn signed_sighash_all_offset_lock_case() -> Case {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

        let mut args = vec![0u8];
        args.extend_from_slice(&public_key_hash[..20]);

        let mut context = Context::default();
        let contract_bin = Loader::default().load_binary("cobuild-otx-lock");
        let contract_out_point = context.deploy_cell(contract_bin);
        let contract_dep = CellDep::new_builder()
            .out_point(contract_out_point.clone())
            .build();
        let lock = context
            .build_script_with_hash_type(&contract_out_point, ScriptHashType::Data2, args.into())
            .expect("build cobuild-otx-lock script");

        let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.to_vec().into());
        let always_success_dep = CellDep::new_builder()
            .out_point(always_success_out_point.clone())
            .build();
        let other_lock = context
            .build_script_with_hash_type(
                &always_success_out_point,
                ScriptHashType::Data,
                Bytes::new(),
            )
            .expect("build always-success script");

        let other_input_output = CellOutput::new_builder()
            .capacity(100_000_000_000u64)
            .lock(other_lock)
            .build();
        let input_output = CellOutput::new_builder()
            .capacity(100_000_000_000u64)
            .lock(lock)
            .build();
        let other_input_out_point = context.create_cell(other_input_output.clone(), Bytes::new());
        let input_out_point = context.create_cell(input_output.clone(), Bytes::new());
        let output = CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(always_success_script(&mut context))
            .build();
        let unsigned_tx = TransactionBuilder::default()
            .cell_dep(contract_dep)
            .cell_dep(always_success_dep)
            .input(
                CellInput::new_builder()
                    .previous_output(other_input_out_point)
                    .build(),
            )
            .input(
                CellInput::new_builder()
                    .previous_output(input_out_point)
                    .build(),
            )
            .output(output)
            .output_data(Bytes::new().pack())
            .witness(Bytes::new().pack())
            .witness(Bytes::new().pack())
            .build();

        let signing_message_hash = tx_without_message_hash_for_inputs(
            packed_hash_to_array(unsigned_tx.hash()),
            &[
                (other_input_output.as_slice(), &[][..]),
                (input_output.as_slice(), &[][..]),
            ],
            &[Vec::new(), Vec::new()],
        );
        let seal = sign_recoverable(&secp, &secret_key, signing_message_hash);
        let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
        let tx = unsigned_tx
            .as_advanced_builder()
            .set_witnesses(vec![
                Bytes::new().pack(),
                Bytes::copy_from_slice(witness.as_slice()).pack(),
            ])
            .build();

        Case { context, tx }
    }

    pub fn signed_otx_dual_scope_case() -> Case {
        signed_otx_case(false, false)
    }

    pub fn signed_otx_full_preimage_case() -> Case {
        signed_otx_case_with_config(OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            override_append_permissions: None,
            include_full_preimage: true,
        })
    }

    pub fn mixed_sighash_all_and_otx_case() -> Case {
        signed_otx_case(true, false)
    }

    pub fn bad_seal_case() -> Case {
        signed_otx_case(false, true)
    }

    pub fn malformed_cobuild_witness_case() -> Case {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

        let mut args = vec![0u8];
        args.extend_from_slice(&public_key_hash[..20]);

        let mut case = build_case(Bytes::from(args));
        case.tx = case
            .tx
            .as_advanced_builder()
            .set_witnesses(vec![
                Bytes::from(malformed_sighash_all_only_witness()).pack(),
            ])
            .build();
        case
    }

    pub fn malformed_otx_layout_case() -> Case {
        signed_otx_case_with_options(false, false, Some(0x10))
    }

    fn signed_otx_case(include_sighash_all: bool, corrupt_append_seal: bool) -> Case {
        signed_otx_case_with_config(OtxCaseConfig {
            include_sighash_all,
            corrupt_append_seal,
            override_append_permissions: None,
            include_full_preimage: false,
        })
    }

    fn signed_otx_case_with_options(
        include_sighash_all: bool,
        corrupt_append_seal: bool,
        override_append_permissions: Option<u8>,
    ) -> Case {
        signed_otx_case_with_config(OtxCaseConfig {
            include_sighash_all,
            corrupt_append_seal,
            override_append_permissions,
            include_full_preimage: false,
        })
    }

    struct OtxCaseConfig {
        include_sighash_all: bool,
        corrupt_append_seal: bool,
        override_append_permissions: Option<u8>,
        include_full_preimage: bool,
    }

    fn signed_otx_case_with_config(config: OtxCaseConfig) -> Case {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).expect("fixed secret key");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_hash = ckb_hash::blake2b_256(public_key.serialize());

        let mut args = vec![0u8];
        args.extend_from_slice(&public_key_hash[..20]);

        let mut context = Context::default();
        let contract_bin = Loader::default().load_binary("cobuild-otx-lock");
        let contract_out_point = context.deploy_cell(contract_bin);
        let contract_dep = CellDep::new_builder()
            .out_point(contract_out_point.clone())
            .build();
        let lock = context
            .build_script_with_hash_type(&contract_out_point, ScriptHashType::Data2, args.into())
            .expect("build cobuild-otx-lock script");
        let script_hash = packed_hash_to_array(lock.calc_script_hash());

        let input_output = CellOutput::new_builder()
            .capacity(100_000_000_000u64)
            .lock(lock)
            .build();
        let input_count = if config.include_sighash_all { 3 } else { 2 };
        let mut input_out_points = Vec::with_capacity(input_count);
        for _ in 0..input_count {
            input_out_points.push(context.create_cell(input_output.clone(), Bytes::new()));
        }
        let cell_inputs: Vec<CellInput> = input_out_points
            .into_iter()
            .map(|previous_output| {
                CellInput::new_builder()
                    .previous_output(previous_output)
                    .build()
            })
            .collect();
        let mut builder = TransactionBuilder::default().cell_dep(contract_dep);
        for input in &cell_inputs {
            builder = builder.input(input.clone());
        }
        let mut base_cell_deps = Vec::new();
        let mut append_cell_deps = Vec::new();
        let mut base_header_deps = Vec::new();
        let mut append_header_deps = Vec::new();
        let outputs = if config.include_full_preimage {
            let base_cell_dep = CellDep::new_builder()
                .out_point(context.deploy_cell(Bytes::from(vec![0x51])))
                .build();
            let append_cell_dep = CellDep::new_builder()
                .out_point(context.deploy_cell(Bytes::from(vec![0x52])))
                .build();
            base_cell_deps.push(base_cell_dep.as_slice().to_vec());
            append_cell_deps.push(append_cell_dep.as_slice().to_vec());
            base_header_deps.push([0x61u8; 32]);
            append_header_deps.push([0x62u8; 32]);
            builder = builder
                .cell_dep(base_cell_dep.clone())
                .cell_dep(append_cell_dep.clone())
                .header_dep([0x61u8; 32].pack())
                .header_dep([0x62u8; 32].pack());

            vec![
                OtxFixtureOutput {
                    cell: CellOutput::new_builder()
                        .capacity(91_000_000_000u64)
                        .lock(always_success_script(&mut context))
                        .build(),
                    data: vec![0x71, 0x72],
                },
                OtxFixtureOutput {
                    cell: CellOutput::new_builder()
                        .capacity(92_000_000_000u64)
                        .lock(always_success_script(&mut context))
                        .build(),
                    data: vec![0x81, 0x82, 0x83],
                },
            ]
        } else {
            vec![OtxFixtureOutput {
                cell: CellOutput::new_builder()
                    .capacity(90_000_000_000u64)
                    .lock(always_success_script(&mut context))
                    .build(),
                data: Vec::new(),
            }]
        };

        for output in &outputs {
            builder = builder
                .output(output.cell.clone())
                .output_data(Bytes::from(output.data.clone()).pack());
        }
        let unsigned_tx = builder.witness(Bytes::new().pack()).build();

        let start_input = if config.include_sighash_all { 1 } else { 0 };
        let (base_outputs, append_outputs, base_output_masks) = if config.include_full_preimage {
            (
                vec![OtxFixtureOutputPart {
                    raw: outputs[0].cell.as_slice().to_vec(),
                    data: outputs[0].data.clone(),
                }],
                vec![OtxFixtureOutputPart {
                    raw: outputs[1].cell.as_slice().to_vec(),
                    data: outputs[1].data.clone(),
                }],
                vec![0b0000_1111],
            )
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
        let default_append_permissions = if config.include_full_preimage {
            0x0f
        } else {
            0x01
        };
        let otx_parts = OtxFixtureParts {
            start_input,
            input_count,
            message: empty_message_entity().as_slice().to_vec(),
            append_permissions: config
                .override_append_permissions
                .unwrap_or(default_append_permissions),
            base_input_masks: vec![0b0000_0011],
            base_inputs: vec![OtxFixtureInput {
                raw: cell_inputs[start_input].as_slice().to_vec(),
                resolved_output: input_output.as_slice().to_vec(),
                data: Vec::new(),
            }],
            append_inputs: vec![OtxFixtureInput {
                raw: cell_inputs[start_input + 1].as_slice().to_vec(),
                resolved_output: input_output.as_slice().to_vec(),
                data: Vec::new(),
            }],
            base_output_masks,
            base_outputs,
            append_outputs,
            base_cell_dep_masks: if config.include_full_preimage {
                vec![0b0000_0001]
            } else {
                Vec::new()
            },
            base_cell_deps,
            append_cell_deps,
            base_header_dep_masks: if config.include_full_preimage {
                vec![0b0000_0001]
            } else {
                Vec::new()
            },
            base_header_deps,
            append_header_deps,
        };
        let base_hash = otx_base_hash(&otx_parts);
        let append_hash = otx_append_hash(&otx_parts, base_hash);
        let base_seal = sign_recoverable(&secp, &secret_key, base_hash);
        let mut append_seal = sign_recoverable(&secp, &secret_key, append_hash);
        if config.corrupt_append_seal {
            append_seal[0] ^= 0x01;
        }

        let otx_start = WitnessLayout::from(
            OtxStart::new_builder()
                .start_input_cell((start_input as u32).to_le_bytes())
                .start_output_cell(0u32.to_le_bytes())
                .start_cell_deps(1u32.to_le_bytes())
                .start_header_deps(0u32.to_le_bytes())
                .build(),
        );
        let otx = WitnessLayout::from(otx_witness(script_hash, &otx_parts, base_seal, append_seal));

        let mut witnesses = Vec::new();
        if config.include_sighash_all {
            let signing_message_hash = tx_without_message_hash(
                packed_hash_to_array(unsigned_tx.hash()),
                input_count,
                input_output.as_slice(),
                &vec![Vec::new(); input_count],
            );
            let tx_seal = sign_recoverable(&secp, &secret_key, signing_message_hash);
            witnesses.push(
                Bytes::copy_from_slice(
                    WitnessLayout::from(SighashAllOnly::new_builder().seal(tx_seal).build())
                        .as_slice(),
                )
                .pack(),
            );
        }
        witnesses.push(Bytes::copy_from_slice(otx_start.as_slice()).pack());
        witnesses.push(Bytes::copy_from_slice(otx.as_slice()).pack());

        let tx = unsigned_tx
            .as_advanced_builder()
            .set_witnesses(witnesses)
            .build();

        Case { context, tx }
    }

    #[derive(Clone)]
    struct OtxFixtureInput {
        raw: Vec<u8>,
        resolved_output: Vec<u8>,
        data: Vec<u8>,
    }

    #[derive(Clone)]
    struct OtxFixtureOutput {
        cell: CellOutput,
        data: Vec<u8>,
    }

    #[derive(Clone)]
    struct OtxFixtureOutputPart {
        raw: Vec<u8>,
        data: Vec<u8>,
    }

    struct OtxFixtureParts {
        start_input: usize,
        input_count: usize,
        message: Vec<u8>,
        append_permissions: u8,
        base_input_masks: Vec<u8>,
        base_inputs: Vec<OtxFixtureInput>,
        append_inputs: Vec<OtxFixtureInput>,
        base_output_masks: Vec<u8>,
        base_outputs: Vec<OtxFixtureOutputPart>,
        append_outputs: Vec<OtxFixtureOutputPart>,
        base_cell_dep_masks: Vec<u8>,
        base_cell_deps: Vec<Vec<u8>>,
        append_cell_deps: Vec<Vec<u8>>,
        base_header_dep_masks: Vec<u8>,
        base_header_deps: Vec<[u8; 32]>,
        append_header_deps: Vec<[u8; 32]>,
    }

    struct OtxHashFixture {
        raw_inputs: Vec<Vec<u8>>,
        resolved_outputs: Vec<Vec<u8>>,
        resolved_data: Vec<Vec<u8>>,
        raw_outputs: Vec<Vec<u8>>,
        output_data: Vec<Vec<u8>>,
        raw_cell_deps: Vec<Vec<u8>>,
        header_deps: Vec<[u8; 32]>,
    }

    fn otx_witness(
        script_hash: [u8; 32],
        parts: &OtxFixtureParts,
        base_seal: Vec<u8>,
        append_seal: Vec<u8>,
    ) -> Otx {
        let message = empty_message_entity();
        let seals = vec![
            SealPair::new_builder()
                .script_hash(script_hash)
                .scope(0u8)
                .seal(base_seal)
                .build(),
            SealPair::new_builder()
                .script_hash(script_hash)
                .scope(1u8)
                .seal(append_seal)
                .build(),
        ];
        Otx::new_builder()
            .message(message)
            .append_permissions(parts.append_permissions)
            .base_input_cells((parts.base_inputs.len() as u32).to_le_bytes())
            .base_input_masks(parts.base_input_masks.clone())
            .base_output_cells((parts.base_outputs.len() as u32).to_le_bytes())
            .base_output_masks(parts.base_output_masks.clone())
            .base_cell_deps((parts.base_cell_deps.len() as u32).to_le_bytes())
            .base_cell_dep_masks(parts.base_cell_dep_masks.clone())
            .base_header_deps((parts.base_header_deps.len() as u32).to_le_bytes())
            .base_header_dep_masks(parts.base_header_dep_masks.clone())
            .append_input_cells((parts.append_inputs.len() as u32).to_le_bytes())
            .append_output_cells((parts.append_outputs.len() as u32).to_le_bytes())
            .append_cell_deps((parts.append_cell_deps.len() as u32).to_le_bytes())
            .append_header_deps((parts.append_header_deps.len() as u32).to_le_bytes())
            .seals(seals)
            .build()
    }

    fn empty_message_entity() -> CobuildMessage {
        CobuildMessage::new_builder()
            .actions(ActionVec::new_builder().build())
            .build()
    }

    fn otx_base_hash(parts: &OtxFixtureParts) -> [u8; 32] {
        let (otx, layout, fixture) = otx_hash_inputs(parts);
        let mut out = [0u8; 32];
        let mut hasher = Blake2bBuilder::new(32).personal(OTX_BASE_PERSONAL).build();

        update_cursor_with_error(
            &mut hasher,
            &otx.message,
            cobuild_core::error::CoreError::MalformedCobuild,
        )
        .expect("message cursor");
        hasher.update(&[otx.append_permissions]);
        write_count(&mut hasher, otx.base_input_cells);
        write_len_prefixed_bytes(&mut hasher, otx.base_input_masks.bytes());
        for local_index in 0..otx.base_input_cells {
            let tx_index = layout.base_inputs.start + local_index;
            let input = cursor_from_slice(&fixture.raw_inputs[tx_index]);
            let input_view = cobuild_types::lazy_reader::blockchain::CellInput::from(input.clone());

            write_count(&mut hasher, local_index);
            if otx
                .includes_base_input_since(local_index)
                .expect("input mask")
            {
                hasher.update(&input_view.since().expect("since").to_le_bytes());
            }
            if otx
                .includes_base_input_previous_output(local_index)
                .expect("input mask")
            {
                update_cursor_with_error(
                    &mut hasher,
                    &input_view
                        .previous_output()
                        .expect("previous output")
                        .cursor,
                    cobuild_core::error::CoreError::MissingHashInput,
                )
                .expect("previous output cursor");
            }
            hasher.update(&fixture.resolved_outputs[tx_index]);
            hasher.update(&checked_len_prefix(fixture.resolved_data[tx_index].len()));
            hasher.update(&fixture.resolved_data[tx_index]);
        }

        write_count(&mut hasher, otx.base_output_cells);
        write_len_prefixed_bytes(&mut hasher, otx.base_output_masks.bytes());
        for local_index in 0..otx.base_output_cells {
            let tx_index = layout.base_outputs.start + local_index;
            let output = cursor_from_slice(&fixture.raw_outputs[tx_index]);
            let output_view =
                cobuild_types::lazy_reader::blockchain::CellOutput::from(output.clone());

            write_count(&mut hasher, local_index);
            if otx
                .includes_base_output_capacity(local_index)
                .expect("output mask")
            {
                hasher.update(&output_view.capacity().expect("capacity").to_le_bytes());
            }
            if otx
                .includes_base_output_lock(local_index)
                .expect("output mask")
            {
                update_cursor_with_error(
                    &mut hasher,
                    &output_view.lock().expect("lock").cursor,
                    cobuild_core::error::CoreError::MissingHashInput,
                )
                .expect("lock cursor");
            }
            if otx
                .includes_base_output_type(local_index)
                .expect("output mask")
            {
                update_cursor_with_error(
                    &mut hasher,
                    &output
                        .table_slice_by_index(2)
                        .expect("output type option cursor"),
                    cobuild_core::error::CoreError::MissingHashInput,
                )
                .expect("type cursor");
            }
            if otx
                .includes_base_output_data(local_index)
                .expect("output mask")
            {
                hasher.update(&checked_len_prefix(fixture.output_data[tx_index].len()));
                hasher.update(&fixture.output_data[tx_index]);
            }
        }

        write_count(&mut hasher, otx.base_cell_deps);
        write_len_prefixed_bytes(&mut hasher, otx.base_cell_dep_masks.bytes());
        for local_index in 0..otx.base_cell_deps {
            if otx
                .base_cell_dep_masks
                .get(local_index)
                .expect("cell dep mask")
            {
                let tx_index = layout.base_cell_deps.start + local_index;
                write_count(&mut hasher, local_index);
                hasher.update(&fixture.raw_cell_deps[tx_index]);
            }
        }

        write_count(&mut hasher, otx.base_header_deps);
        write_len_prefixed_bytes(&mut hasher, otx.base_header_dep_masks.bytes());
        for local_index in 0..otx.base_header_deps {
            if otx
                .base_header_dep_masks
                .get(local_index)
                .expect("header dep mask")
            {
                let tx_index = layout.base_header_deps.start + local_index;
                write_count(&mut hasher, local_index);
                hasher.update(&fixture.header_deps[tx_index]);
            }
        }

        hasher.finalize(&mut out);
        out
    }

    fn otx_append_hash(parts: &OtxFixtureParts, base_hash: [u8; 32]) -> [u8; 32] {
        let (otx, layout, fixture) = otx_hash_inputs(parts);
        let mut out = [0u8; 32];
        let mut hasher = Blake2bBuilder::new(32)
            .personal(OTX_APPEND_PERSONAL)
            .build();

        update_cursor_with_error(
            &mut hasher,
            &otx.message,
            cobuild_core::error::CoreError::MalformedCobuild,
        )
        .expect("message cursor");
        hasher.update(&base_hash);
        write_count(&mut hasher, otx.append_input_cells);
        for local_index in 0..otx.append_input_cells {
            let tx_index = layout.append_inputs.start + local_index;
            write_count(&mut hasher, local_index);
            hasher.update(&fixture.raw_inputs[tx_index]);
            hasher.update(&fixture.resolved_outputs[tx_index]);
            hasher.update(&checked_len_prefix(fixture.resolved_data[tx_index].len()));
            hasher.update(&fixture.resolved_data[tx_index]);
        }

        write_count(&mut hasher, otx.append_output_cells);
        for local_index in 0..otx.append_output_cells {
            let tx_index = layout.append_outputs.start + local_index;
            write_count(&mut hasher, local_index);
            hasher.update(&fixture.raw_outputs[tx_index]);
            hasher.update(&checked_len_prefix(fixture.output_data[tx_index].len()));
            hasher.update(&fixture.output_data[tx_index]);
        }

        write_count(&mut hasher, otx.append_cell_deps);
        for local_index in 0..otx.append_cell_deps {
            let tx_index = layout.append_cell_deps.start + local_index;
            write_count(&mut hasher, local_index);
            hasher.update(&fixture.raw_cell_deps[tx_index]);
        }

        write_count(&mut hasher, otx.append_header_deps);
        for local_index in 0..otx.append_header_deps {
            let tx_index = layout.append_header_deps.start + local_index;
            write_count(&mut hasher, local_index);
            hasher.update(&fixture.header_deps[tx_index]);
        }

        hasher.finalize(&mut out);
        out
    }

    fn otx_hash_inputs(parts: &OtxFixtureParts) -> (OtxView, OtxLayout, OtxHashFixture) {
        let base_start = parts.start_input;
        let append_start = base_start + parts.base_inputs.len();
        let append_output_start = parts.base_outputs.len();
        let base_cell_dep_start = 1;
        let append_cell_dep_start = base_cell_dep_start + parts.base_cell_deps.len();
        let append_header_dep_start = parts.base_header_deps.len();
        let mut raw_inputs = vec![Vec::new(); parts.input_count];
        let mut resolved_outputs = vec![Vec::new(); parts.input_count];
        let mut resolved_data = vec![Vec::new(); parts.input_count];
        let mut raw_outputs =
            vec![Vec::new(); parts.base_outputs.len() + parts.append_outputs.len()];
        let mut output_data =
            vec![Vec::new(); parts.base_outputs.len() + parts.append_outputs.len()];
        let mut raw_cell_deps =
            vec![
                Vec::new();
                base_cell_dep_start + parts.base_cell_deps.len() + parts.append_cell_deps.len()
            ];
        let mut header_deps =
            vec![[0u8; 32]; parts.base_header_deps.len() + parts.append_header_deps.len()];

        for (offset, input) in parts.base_inputs.iter().enumerate() {
            let index = base_start + offset;
            raw_inputs[index] = input.raw.clone();
            resolved_outputs[index] = input.resolved_output.clone();
            resolved_data[index] = input.data.clone();
        }
        for (offset, input) in parts.append_inputs.iter().enumerate() {
            let index = append_start + offset;
            raw_inputs[index] = input.raw.clone();
            resolved_outputs[index] = input.resolved_output.clone();
            resolved_data[index] = input.data.clone();
        }
        for (offset, output) in parts.base_outputs.iter().enumerate() {
            raw_outputs[offset] = output.raw.clone();
            output_data[offset] = output.data.clone();
        }
        for (offset, output) in parts.append_outputs.iter().enumerate() {
            let index = append_output_start + offset;
            raw_outputs[index] = output.raw.clone();
            output_data[index] = output.data.clone();
        }
        for (offset, cell_dep) in parts.base_cell_deps.iter().enumerate() {
            raw_cell_deps[base_cell_dep_start + offset] = cell_dep.clone();
        }
        for (offset, cell_dep) in parts.append_cell_deps.iter().enumerate() {
            raw_cell_deps[append_cell_dep_start + offset] = cell_dep.clone();
        }
        for (offset, header_dep) in parts.base_header_deps.iter().enumerate() {
            header_deps[offset] = *header_dep;
        }
        for (offset, header_dep) in parts.append_header_deps.iter().enumerate() {
            header_deps[append_header_dep_start + offset] = *header_dep;
        }

        let otx = OtxView {
            message: cursor_from_slice(&parts.message),
            append_permissions: parts.append_permissions,
            base_input_cells: parts.base_inputs.len(),
            base_input_masks: mask_view(&parts.base_input_masks),
            base_output_cells: parts.base_outputs.len(),
            base_output_masks: mask_view(&parts.base_output_masks),
            base_cell_deps: parts.base_cell_deps.len(),
            base_cell_dep_masks: mask_view(&parts.base_cell_dep_masks),
            base_header_deps: parts.base_header_deps.len(),
            base_header_dep_masks: mask_view(&parts.base_header_dep_masks),
            append_input_cells: parts.append_inputs.len(),
            append_output_cells: parts.append_outputs.len(),
            append_cell_deps: parts.append_cell_deps.len(),
            append_header_deps: parts.append_header_deps.len(),
            seals: Vec::new(),
        };
        let layout = OtxLayout {
            witness_index: 0,
            base_inputs: range(base_start, parts.base_inputs.len()),
            append_inputs: range(append_start, parts.append_inputs.len()),
            base_outputs: range(0, parts.base_outputs.len()),
            append_outputs: range(append_output_start, parts.append_outputs.len()),
            base_cell_deps: range(base_cell_dep_start, parts.base_cell_deps.len()),
            append_cell_deps: range(append_cell_dep_start, parts.append_cell_deps.len()),
            base_header_deps: range(0, parts.base_header_deps.len()),
            append_header_deps: range(append_header_dep_start, parts.append_header_deps.len()),
        };
        let fixture = OtxHashFixture {
            raw_inputs,
            resolved_outputs,
            resolved_data,
            raw_outputs,
            output_data,
            raw_cell_deps,
            header_deps,
        };
        (otx, layout, fixture)
    }

    fn mask_view(bytes: &[u8]) -> MaskView {
        MaskView::new(bytes.to_vec())
    }

    fn range(start: usize, count: usize) -> Range {
        Range { start, count }
    }

    fn build_case(args: Bytes) -> Case {
        let mut context = Context::default();
        let contract_bin = Loader::default().load_binary("cobuild-otx-lock");
        let contract_out_point = context.deploy_cell(contract_bin);
        let contract_dep = CellDep::new_builder()
            .out_point(contract_out_point.clone())
            .build();
        let lock = context
            .build_script_with_hash_type(&contract_out_point, ScriptHashType::Data2, args)
            .expect("build cobuild-otx-lock script");
        let input_out_point = context.create_cell(
            CellOutput::new_builder()
                .capacity(100_000_000_000u64)
                .lock(lock)
                .build(),
            Bytes::new(),
        );
        let output = CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(always_success_script(&mut context))
            .build();
        let tx = TransactionBuilder::default()
            .cell_dep(contract_dep)
            .input(
                CellInput::new_builder()
                    .previous_output(input_out_point)
                    .build(),
            )
            .output(output)
            .output_data(Bytes::new().pack())
            .witness(Bytes::new().pack())
            .build();
        Case { context, tx }
    }

    fn packed_hash_to_array(hash: ckb_testtool::ckb_types::packed::Byte32) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(hash.as_slice());
        out
    }

    fn malformed_sighash_all_only_witness() -> Vec<u8> {
        witness_union(0xff00_0002, &table(&[Vec::new()]))
    }

    fn witness_union(item_id: u32, item: &[u8]) -> Vec<u8> {
        let mut witness = Vec::with_capacity(4 + item.len());
        witness.extend_from_slice(&item_id.to_le_bytes());
        witness.extend_from_slice(item);
        witness
    }

    fn table(fields: &[Vec<u8>]) -> Vec<u8> {
        let header_size = 4 + fields.len() * 4;
        let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
        let mut out = Vec::with_capacity(total_size);
        out.extend_from_slice(&(total_size as u32).to_le_bytes());
        let mut offset = header_size as u32;
        for field in fields {
            out.extend_from_slice(&offset.to_le_bytes());
            offset += field.len() as u32;
        }
        for field in fields {
            out.extend_from_slice(field);
        }
        out
    }

    fn tx_without_message_hash(
        tx_hash: [u8; 32],
        input_count: usize,
        resolved_output: &[u8],
        witnesses: &[Vec<u8>],
    ) -> [u8; 32] {
        let inputs = vec![(resolved_output, &[][..]); input_count];
        tx_without_message_hash_for_inputs(tx_hash, &inputs, witnesses)
    }

    fn tx_without_message_hash_for_inputs(
        tx_hash: [u8; 32],
        inputs: &[(&[u8], &[u8])],
        witnesses: &[Vec<u8>],
    ) -> [u8; 32] {
        let mut out = [0u8; 32];
        let mut hasher = Blake2bBuilder::new(32)
            .personal(TX_WITHOUT_MESSAGE_PERSONAL)
            .build();
        hasher.update(&tx_hash);
        for (resolved_output, data) in inputs {
            hasher.update(resolved_output);
            hasher.update(&checked_len_prefix(data.len()));
            hasher.update(data);
        }
        for witness in witnesses.iter().skip(inputs.len()) {
            hasher.update(&checked_len_prefix(witness.len()));
            hasher.update(witness);
        }
        hasher.finalize(&mut out);
        out
    }

    fn checked_len_prefix(len: usize) -> [u8; 4] {
        u32::try_from(len)
            .expect("fixture length fits u32")
            .to_le_bytes()
    }

    fn write_count(hasher: &mut blake2b_ref::Blake2b, count: usize) {
        hasher.update(&checked_len_prefix(count));
    }

    fn write_len_prefixed_bytes(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) {
        hasher.update(&checked_len_prefix(bytes.len()));
        hasher.update(bytes);
    }

    fn sign_recoverable(
        secp: &Secp256k1<secp256k1::All>,
        secret_key: &SecretKey,
        signing_message_hash: [u8; 32],
    ) -> Vec<u8> {
        let message = Message::from_digest(signing_message_hash);
        let signature = secp.sign_ecdsa_recoverable(&message, secret_key);
        let (recovery_id, compact) = signature.serialize_compact();
        let mut seal = Vec::with_capacity(65);
        seal.extend_from_slice(&compact);
        seal.push(i32::from(recovery_id) as u8);
        seal
    }

    fn always_success_script(context: &mut Context) -> ckb_testtool::ckb_types::packed::Script {
        let out_point = context.deploy_cell(ALWAYS_SUCCESS.to_vec().into());
        context
            .build_script_with_hash_type(&out_point, ScriptHashType::Data, Bytes::new())
            .expect("build always-success script")
    }
}
