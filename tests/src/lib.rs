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

// The exact same Loader code from capsule's template, except that
// now we use MODE as the environment variable
const TEST_ENV_VAR: &str = "MODE";

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

impl Default for Loader {
    fn default() -> Self {
        let test_env = match env::var(TEST_ENV_VAR) {
            Ok(val) => val.parse().expect("test env"),
            Err(_) => TestEnv::Release,
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

    use crate::Loader;

    pub struct Case {
        context: Context,
        tx: TransactionView,
    }

    impl Case {
        pub fn verify(self) -> Result<Cycle, Error> {
            self.context.verify_tx(&self.tx, 10_000_000)
        }
    }

    pub fn invalid_args_case() -> Case {
        build_case(Bytes::from(vec![0u8]))
    }

    pub fn no_relevant_task_case() -> Case {
        let mut args = vec![0u8];
        args.extend_from_slice(&[1u8; 20]);
        build_case(Bytes::from(args))
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
            .input(CellInput::new_builder().previous_output(input_out_point).build())
            .output(output)
            .output_data(Bytes::new().pack())
            .witness(Bytes::new().pack())
            .build();
        Case { context, tx }
    }

    fn always_success_script(context: &mut Context) -> ckb_testtool::ckb_types::packed::Script {
        let out_point = context.deploy_cell(ALWAYS_SUCCESS.to_vec().into());
        context
            .build_script_with_hash_type(&out_point, ScriptHashType::Data, Bytes::new())
            .expect("build always-success script")
    }
}
