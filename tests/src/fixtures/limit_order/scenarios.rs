use crate::framework::{fixture::CobuildTestFixture, tx::BuiltTxShape};

use super::LimitOrderExpectedOutcome;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowKind {
    Create,
    Fill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptRoleKind {
    InputLock,
    InputType,
    OutputType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtxScopeKind {
    Current,
    AnotherOtx,
    Remainder,
    TxLevel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionSourceKind {
    Otx,
    TxLevel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageTag {
    Flow(FlowKind),
    ScriptRole(ScriptRoleKind),
    OtxScope(OtxScopeKind),
    ActionSource(ActionSourceKind),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitOrderHappyPath {
    pub flow: FlowKind,
    pub script_role: ScriptRoleKind,
    pub action_source: ActionSourceKind,
}

impl LimitOrderHappyPath {
    pub fn fill_type_otx() -> Self {
        Self {
            flow: FlowKind::Fill,
            script_role: ScriptRoleKind::InputType,
            action_source: ActionSourceKind::Otx,
        }
    }

    pub fn fill_lock_otx() -> Self {
        Self {
            flow: FlowKind::Fill,
            script_role: ScriptRoleKind::InputLock,
            action_source: ActionSourceKind::Otx,
        }
    }

    pub fn create_type_tx_level() -> Self {
        Self {
            flow: FlowKind::Create,
            script_role: ScriptRoleKind::OutputType,
            action_source: ActionSourceKind::TxLevel,
        }
    }

    pub fn coverage(&self) -> Vec<CoverageTag> {
        vec![
            CoverageTag::Flow(self.flow),
            CoverageTag::ScriptRole(self.script_role),
            CoverageTag::ActionSource(self.action_source),
        ]
    }
}

pub struct BuiltLimitOrderCase {
    pub fixture: CobuildTestFixture,
    pub built: BuiltTxShape,
    pub expected: LimitOrderExpectedOutcome,
    pub coverage: Vec<CoverageTag>,
}

impl BuiltLimitOrderCase {
    pub fn assert_expected(&self) {
        self.expected.assert(&self.fixture, &self.built);
    }
}
