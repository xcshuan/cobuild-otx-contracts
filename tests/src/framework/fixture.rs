use ckb_testtool::{ckb_types::core::TransactionView, context::Context};

use super::{
    assertions,
    cobuild::{CobuildMessageBuilder, OtxBuilder},
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

    pub fn cobuild(&self) -> CobuildMessageBuilder {
        CobuildMessageBuilder::new()
    }

    pub fn otx(&self) -> OtxBuilder {
        OtxBuilder::new()
    }

    pub fn assert_pass(&self, tx: &TransactionView) {
        assertions::assert_pass(&self.context, tx);
    }

    pub fn assert_pass_with_max_cycles(&self, tx: &TransactionView, max_cycles: u64) {
        assertions::assert_pass_with_max_cycles(&self.context, tx, max_cycles);
    }

    pub fn assert_type_script_exit(&self, tx: &TransactionView, input_index: usize, code: i8) {
        assertions::assert_type_script_exit(&self.context, tx, input_index, code);
    }

    pub fn assert_output_type_script_exit(
        &self,
        tx: &TransactionView,
        output_index: usize,
        code: i8,
    ) {
        assertions::assert_output_type_script_exit(&self.context, tx, output_index, code);
    }

    pub fn assert_lock_script_exit(&self, tx: &TransactionView, input_index: usize, code: i8) {
        assertions::assert_lock_script_exit(&self.context, tx, input_index, code);
    }
}

impl Default for CobuildTestFixture {
    fn default() -> Self {
        Self::new()
    }
}
