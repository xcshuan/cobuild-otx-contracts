use ckb_testtool::ckb_types::prelude::{Builder, Entity};
use cobuild_types::entity::core::{Action, ActionVec, Message as CobuildMessage};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionRole {
    InputLock,
    InputType,
    OutputType,
}

impl From<ActionRole> for u8 {
    fn from(role: ActionRole) -> Self {
        match role {
            ActionRole::InputLock => 0,
            ActionRole::InputType => 1,
            ActionRole::OutputType => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub script_hash: [u8; 32],
    pub script_role: u8,
    pub action_data: Vec<u8>,
}

impl ActionSpec {
    pub fn new(role: ActionRole, script_hash: [u8; 32], action_data: Vec<u8>) -> Self {
        Self {
            script_hash,
            script_role: role.into(),
            action_data,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MessageBuilder {
    script_hash: [u8; 32],
    script_role: u8,
    action_data: Vec<u8>,
    actions: Vec<ActionSpec>,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            script_hash: [0; 32],
            script_role: ActionRole::InputType.into(),
            action_data: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn input_type_action(mut self, script_hash: [u8; 32]) -> Self {
        self.script_hash = script_hash;
        self.script_role = ActionRole::InputType.into();
        self
    }

    pub fn input_lock_action(mut self, script_hash: [u8; 32]) -> Self {
        self.script_hash = script_hash;
        self.script_role = ActionRole::InputLock.into();
        self
    }

    pub fn output_type_action(mut self, script_hash: [u8; 32]) -> Self {
        self.script_hash = script_hash;
        self.script_role = ActionRole::OutputType.into();
        self
    }

    pub fn action_data(mut self, action_data: Vec<u8>) -> Self {
        self.action_data = action_data;
        self
    }

    pub fn push_action(
        mut self,
        script_role: u8,
        script_hash: [u8; 32],
        action_data: Vec<u8>,
    ) -> Self {
        self.actions.push(ActionSpec {
            script_hash,
            script_role,
            action_data,
        });
        self
    }

    pub fn build(self) -> CobuildMessage {
        let actions = if self.actions.is_empty() {
            vec![ActionSpec {
                script_hash: self.script_hash,
                script_role: self.script_role,
                action_data: self.action_data,
            }]
        } else {
            self.actions
        }
        .into_iter()
        .map(|spec| {
            Action::new_builder()
                .script_info_hash([0u8; 32])
                .script_role(spec.script_role)
                .script_hash(spec.script_hash)
                .data(spec.action_data)
                .build()
        });
        CobuildMessage::new_builder()
            .actions(ActionVec::new_builder().extend(actions).build())
            .build()
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub type CobuildActionSpec = ActionSpec;
pub type CobuildMessageBuilder = MessageBuilder;

pub fn empty_message() -> CobuildMessage {
    CobuildMessage::new_builder()
        .actions(ActionVec::new_builder().build())
        .build()
}
