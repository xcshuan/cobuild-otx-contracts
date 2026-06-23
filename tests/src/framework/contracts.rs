use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        core::ScriptHashType,
        packed::{CellDep, OutPoint, Script},
        prelude::*,
    },
    context::Context,
};

use crate::Loader;

use super::scripts::script_hash;

#[derive(Clone, Debug)]
pub struct DeployedScript {
    pub out_point: OutPoint,
    pub script: Script,
    pub script_hash: [u8; 32],
    pub cell_dep: CellDep,
}

pub fn deploy_loader_binary_code(
    context: &mut Context,
    name: &str,
    hash_type: ScriptHashType,
) -> DeployedScript {
    let bin = Loader::default().load_binary(name);
    deploy_script_bytes_code(context, bin, hash_type)
}

pub fn cell_dep_for_script(script: &DeployedScript) -> CellDep {
    script.cell_dep.clone()
}

pub fn deploy_script_bytes_code(
    context: &mut Context,
    bin: Bytes,
    hash_type: ScriptHashType,
) -> DeployedScript {
    let out_point = context.deploy_cell(bin);
    let cell_dep = CellDep::new_builder().out_point(out_point.clone()).build();
    let script = build_script(context, &out_point, hash_type, Vec::new());
    let script_hash = script_hash(&script);

    DeployedScript {
        out_point,
        script,
        script_hash,
        cell_dep,
    }
}

pub fn build_script(
    context: &mut Context,
    out_point: &OutPoint,
    hash_type: ScriptHashType,
    args: Vec<u8>,
) -> Script {
    context
        .build_script_with_hash_type(out_point, hash_type, Bytes::from(args))
        .expect("build deployed script")
}

pub fn build_deployed_script(
    context: &mut Context,
    code: &DeployedScript,
    hash_type: ScriptHashType,
    args: Vec<u8>,
) -> DeployedScript {
    let script = context
        .build_script_with_hash_type(&code.out_point, hash_type, Bytes::from(args))
        .expect("build deployed script");
    let script_hash = script_hash(&script);

    DeployedScript {
        out_point: code.out_point.clone(),
        script,
        script_hash,
        cell_dep: code.cell_dep.clone(),
    }
}
