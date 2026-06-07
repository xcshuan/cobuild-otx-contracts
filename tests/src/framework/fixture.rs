use ckb_testtool::{ckb_types::core::TransactionView, context::Context};

use super::{
    assertions,
    cobuild::{CobuildMessageBuilder, OtxBuilder},
    contracts::{DeployedScript, deploy_always_success},
    tx::OtxTransactionBuilder,
};

pub struct CobuildTestFixture {
    context: Context,
}

impl CobuildTestFixture {
    pub fn new() -> Self {
        Self {
            context: Context::default(),
        }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    pub fn deploy_always_success(&mut self) -> DeployedScript {
        deploy_always_success(&mut self.context, Vec::new())
    }

    pub fn cobuild(&self) -> CobuildMessageBuilder {
        CobuildMessageBuilder::new()
    }

    pub fn otx(&self) -> OtxBuilder {
        OtxBuilder::new()
    }

    pub fn tx(&self) -> OtxTransactionBuilder {
        OtxTransactionBuilder::new()
    }

    pub fn assert_pass(&self, tx: &TransactionView) {
        assertions::assert_pass(&self.context, tx);
    }

    pub fn assert_type_script_exit(&self, tx: &TransactionView, input_index: usize, code: i8) {
        assertions::assert_type_script_exit(&self.context, tx, input_index, code);
    }
}

impl Default for CobuildTestFixture {
    fn default() -> Self {
        Self::new()
    }
}
