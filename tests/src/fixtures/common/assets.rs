use ckb_testtool::ckb_types::{bytes::Bytes, packed::Script};

use crate::framework::contracts::{DeployedScript, cell_dep_for_script};

#[derive(Clone, Debug)]
pub struct TestUdt {
    pub script: Script,
    pub script_hash: [u8; 32],
    pub cell_dep: ckb_testtool::ckb_types::packed::CellDep,
}

#[derive(Clone, Debug)]
pub struct TestNft {
    pub script: Script,
    pub script_hash: [u8; 32],
    pub cell_dep: ckb_testtool::ckb_types::packed::CellDep,
}

impl TestUdt {
    pub fn from_deployed(script: &DeployedScript) -> Self {
        Self {
            script: script.script.clone(),
            script_hash: script.script_hash,
            cell_dep: cell_dep_for_script(script),
        }
    }
}

impl TestNft {
    pub fn from_deployed(script: &DeployedScript) -> Self {
        Self {
            script: script.script.clone(),
            script_hash: script.script_hash,
            cell_dep: cell_dep_for_script(script),
        }
    }
}

pub fn udt_amount_data(amount: u128) -> Bytes {
    Bytes::from(amount.to_le_bytes().to_vec())
}

pub fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Bytes {
    let mut data = Vec::with_capacity(1 + name.len() + 4 + 8);
    data.push(name.len() as u8);
    data.extend_from_slice(name);
    data.extend_from_slice(&attributes);
    data.extend_from_slice(&created_at.to_le_bytes());
    data.into()
}
