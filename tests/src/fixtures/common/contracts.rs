use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    ckb_types::{core::ScriptHashType, packed::Script},
    context::Context,
};

use crate::framework::contracts::{
    DeployedScript, build_deployed_script, build_script, deploy_loader_binary_code,
    deploy_script_bytes_code,
};

#[derive(Clone, Debug)]
pub struct ContractCatalog {
    pub always_success: DeployedScript,
    pub cobuild_otx_lock_code: DeployedScript,
    pub limit_order_type_code: DeployedScript,
    pub limit_order_lock_code: DeployedScript,
}

impl ContractCatalog {
    pub fn deploy(context: &mut Context) -> Self {
        let always_success_code = deploy_always_success_code(context);
        Self {
            always_success: build_always_success_script(context, &always_success_code, Vec::new()),
            cobuild_otx_lock_code: deploy_cobuild_otx_lock_code(context),
            limit_order_type_code: deploy_limit_order_type(context),
            limit_order_lock_code: deploy_limit_order_lock(context),
        }
    }
}

pub fn deploy_always_success_code(context: &mut Context) -> DeployedScript {
    deploy_script_bytes_code(
        context,
        ALWAYS_SUCCESS.to_vec().into(),
        ScriptHashType::Data,
    )
}

pub fn build_always_success_script(
    context: &mut Context,
    code: &DeployedScript,
    args: Vec<u8>,
) -> DeployedScript {
    build_deployed_script(context, code, ScriptHashType::Data, args)
}

pub fn deploy_test_udt_code(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "test-udt", ScriptHashType::Data2)
}

pub fn build_test_udt_script(
    context: &mut Context,
    code: &DeployedScript,
    owner_lock_hash: [u8; 32],
) -> DeployedScript {
    build_deployed_script(
        context,
        code,
        ScriptHashType::Data2,
        owner_lock_hash.to_vec(),
    )
}

pub fn deploy_test_nft_code(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "test-nft", ScriptHashType::Data2)
}

pub fn build_test_nft_script(
    context: &mut Context,
    code: &DeployedScript,
    args: [u8; 32],
) -> DeployedScript {
    build_deployed_script(context, code, ScriptHashType::Data2, args.to_vec())
}

pub fn deploy_nft_minter_type_code(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "nft-minter-type", ScriptHashType::Data2)
}

pub fn build_nft_minter_type_script(
    context: &mut Context,
    code: &DeployedScript,
    args: Vec<u8>,
) -> DeployedScript {
    build_deployed_script(context, code, ScriptHashType::Data2, args)
}

pub fn deploy_minted_nft_type_code(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "minted-nft-type", ScriptHashType::Data2)
}

pub fn build_minted_nft_type_script(
    context: &mut Context,
    code: &DeployedScript,
    nft_id: [u8; 32],
) -> DeployedScript {
    build_deployed_script(context, code, ScriptHashType::Data2, nft_id.to_vec())
}

pub fn deploy_input_type_proxy_lock_code(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "input-type-proxy-lock", ScriptHashType::Data2)
}

pub fn build_input_type_proxy_lock_script(
    context: &mut Context,
    code: &DeployedScript,
    owner_type_hash: [u8; 32],
) -> DeployedScript {
    build_deployed_script(
        context,
        code,
        ScriptHashType::Data2,
        owner_type_hash.to_vec(),
    )
}

pub fn build_wrong_owner_lock(context: &mut Context, code: &DeployedScript) -> DeployedScript {
    build_always_success_script(context, code, b"wrong-owner".to_vec())
}

pub fn deploy_cobuild_otx_lock_code(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "cobuild-otx-lock", ScriptHashType::Data2)
}

pub fn build_cobuild_otx_lock(
    context: &mut Context,
    code: &DeployedScript,
    public_key_hash: &[u8],
) -> DeployedScript {
    build_deployed_script(
        context,
        code,
        ScriptHashType::Data2,
        public_key_hash.to_vec(),
    )
}

pub fn deploy_limit_order_type(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "limit-order-type", ScriptHashType::Data2)
}

pub fn deploy_limit_order_lock(context: &mut Context) -> DeployedScript {
    deploy_loader_binary_code(context, "limit-order-lock", ScriptHashType::Data2)
}

pub fn build_data2_script(
    context: &mut Context,
    deployed: &DeployedScript,
    args: Vec<u8>,
) -> Script {
    build_script(context, &deployed.out_point, ScriptHashType::Data2, args)
}
