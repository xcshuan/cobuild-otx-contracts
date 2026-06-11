use crate::framework::{fixture::CobuildTestFixture, tx::BuiltTxShape};

use super::{BusinessMutation, LimitOrderExpectedOutcome};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderHappyPath {
    TypeNftForUdt,
    LockNftForUdt,
    MixedTypeAndLock,
    CreateTypeOrder,
    TwoTypeOrders,
    TwoLockOrders,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowKind {
    TxLevel,
    OtxOnly,
    TxLevelAndOtx,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptRoleKind {
    InputLock,
    InputType,
    OutputType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OtxScopeKind {
    BaseInput,
    AppendInput,
    BaseOutput,
    AppendOutput,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionSourceKind {
    TxLevel,
    Otx,
    Absent,
    WrongTarget,
    Duplicate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverageTag {
    pub flow: FlowKind,
    pub script_role: ScriptRoleKind,
    pub otx_scope: OtxScopeKind,
    pub action_source: ActionSourceKind,
    pub mutation: Option<BusinessMutation>,
}

impl CoverageTag {
    pub fn new(
        flow: FlowKind,
        script_role: ScriptRoleKind,
        otx_scope: OtxScopeKind,
        action_source: ActionSourceKind,
    ) -> Self {
        Self {
            flow,
            script_role,
            otx_scope,
            action_source,
            mutation: None,
        }
    }

    pub fn with_mutation(mut self, mutation: BusinessMutation) -> Self {
        self.mutation = Some(mutation);
        self
    }
}

impl LimitOrderHappyPath {
    pub fn default_coverage(self) -> CoverageTag {
        match self {
            Self::TypeNftForUdt => CoverageTag::new(
                FlowKind::OtxOnly,
                ScriptRoleKind::InputType,
                OtxScopeKind::BaseInput,
                ActionSourceKind::Otx,
            ),
            Self::LockNftForUdt => CoverageTag::new(
                FlowKind::OtxOnly,
                ScriptRoleKind::InputLock,
                OtxScopeKind::BaseInput,
                ActionSourceKind::Otx,
            ),
            Self::MixedTypeAndLock => CoverageTag::new(
                FlowKind::OtxOnly,
                ScriptRoleKind::InputLock,
                OtxScopeKind::BaseInput,
                ActionSourceKind::Duplicate,
            ),
            Self::CreateTypeOrder => CoverageTag::new(
                FlowKind::TxLevel,
                ScriptRoleKind::OutputType,
                OtxScopeKind::Remainder,
                ActionSourceKind::TxLevel,
            ),
            Self::TwoTypeOrders => CoverageTag::new(
                FlowKind::OtxOnly,
                ScriptRoleKind::InputType,
                OtxScopeKind::BaseInput,
                ActionSourceKind::Duplicate,
            ),
            Self::TwoLockOrders => CoverageTag::new(
                FlowKind::OtxOnly,
                ScriptRoleKind::InputLock,
                OtxScopeKind::BaseInput,
                ActionSourceKind::Duplicate,
            ),
        }
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
