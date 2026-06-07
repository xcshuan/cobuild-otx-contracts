use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
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

pub fn deploy_data2_script(context: &mut Context, name: &str, args: Vec<u8>) -> DeployedScript {
    let bin = Loader::default().load_binary(name);
    deploy_script(context, bin, ScriptHashType::Data2, args)
}

pub fn deploy_always_success(context: &mut Context, args: Vec<u8>) -> DeployedScript {
    deploy_script(
        context,
        ALWAYS_SUCCESS.to_vec().into(),
        ScriptHashType::Data,
        args,
    )
}

pub fn cell_dep_for_script(script: &DeployedScript) -> CellDep {
    script.cell_dep.clone()
}

fn deploy_script(
    context: &mut Context,
    bin: Bytes,
    hash_type: ScriptHashType,
    args: Vec<u8>,
) -> DeployedScript {
    let out_point = context.deploy_cell(bin);
    let cell_dep = CellDep::new_builder().out_point(out_point.clone()).build();
    let script = context
        .build_script_with_hash_type(&out_point, hash_type, Bytes::from(args))
        .expect("build deployed script");
    let script_hash = script_hash(&script);

    DeployedScript {
        out_point,
        script,
        script_hash,
        cell_dep,
    }
}
