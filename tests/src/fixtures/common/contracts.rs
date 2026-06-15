use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    ckb_types::{core::ScriptHashType, packed::Script},
    context::Context,
};

use crate::framework::contracts::{DeployedScript, deploy_loader_binary, deploy_script_bytes};

#[derive(Clone, Debug)]
pub struct ContractCatalog {
    pub always_success: DeployedScript,
    pub cobuild_otx_lock_code: DeployedScript,
    pub limit_order_type_code: DeployedScript,
    pub limit_order_lock_code: DeployedScript,
}

impl ContractCatalog {
    pub fn deploy(context: &mut Context) -> Self {
        Self {
            always_success: deploy_always_success(context, Vec::new()),
            cobuild_otx_lock_code: deploy_cobuild_otx_lock_code(context, Vec::new()),
            limit_order_type_code: deploy_limit_order_type(context),
            limit_order_lock_code: deploy_limit_order_lock(context),
        }
    }
}

pub fn deploy_always_success(context: &mut Context, args: Vec<u8>) -> DeployedScript {
    deploy_script_bytes(
        context,
        ALWAYS_SUCCESS.to_vec().into(),
        ScriptHashType::Data,
        args,
    )
}

pub fn deploy_test_udt(context: &mut Context, owner_lock_hash: [u8; 32]) -> DeployedScript {
    deploy_loader_binary(
        context,
        "test-udt",
        ScriptHashType::Data2,
        owner_lock_hash.to_vec(),
    )
}

pub fn deploy_test_nft(context: &mut Context, args: [u8; 32]) -> DeployedScript {
    deploy_loader_binary(context, "test-nft", ScriptHashType::Data2, args.to_vec())
}

pub fn deploy_nft_minter_type(context: &mut Context, args: Vec<u8>) -> DeployedScript {
    deploy_loader_binary(context, "nft-minter-type", ScriptHashType::Data2, args)
}

pub fn deploy_minted_nft_type(context: &mut Context, nft_id: [u8; 32]) -> DeployedScript {
    deploy_loader_binary(
        context,
        "minted-nft-type",
        ScriptHashType::Data2,
        nft_id.to_vec(),
    )
}

pub fn deploy_input_type_proxy_lock(
    context: &mut Context,
    owner_type_hash: [u8; 32],
) -> DeployedScript {
    deploy_loader_binary(
        context,
        "input-type-proxy-lock",
        ScriptHashType::Data2,
        owner_type_hash.to_vec(),
    )
}

pub fn deploy_wrong_owner_lock(context: &mut Context) -> DeployedScript {
    deploy_always_success(context, b"wrong-owner".to_vec())
}

pub fn deploy_cobuild_otx_lock_code(context: &mut Context, args: Vec<u8>) -> DeployedScript {
    deploy_loader_binary(context, "cobuild-otx-lock", ScriptHashType::Data2, args)
}

pub fn deploy_cobuild_otx_lock(
    context: &mut Context,
    auth_algorithm_id: u8,
    public_key_hash: &[u8],
) -> DeployedScript {
    let mut args = vec![auth_algorithm_id];
    args.extend_from_slice(public_key_hash);
    deploy_cobuild_otx_lock_code(context, args)
}

pub fn deploy_limit_order_type(context: &mut Context) -> DeployedScript {
    deploy_loader_binary(
        context,
        "limit-order-type",
        ScriptHashType::Data2,
        Vec::new(),
    )
}

pub fn deploy_limit_order_lock(context: &mut Context) -> DeployedScript {
    deploy_loader_binary(
        context,
        "limit-order-lock",
        ScriptHashType::Data2,
        Vec::new(),
    )
}

pub fn rebuild_data2_script(
    context: &mut Context,
    deployed: &DeployedScript,
    args: Vec<u8>,
) -> Script {
    context
        .build_script_with_hash_type(&deployed.out_point, ScriptHashType::Data2, args.into())
        .expect("build deployed Data2 script")
}
