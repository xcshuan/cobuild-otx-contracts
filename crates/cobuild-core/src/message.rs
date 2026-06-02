use crate::{
    context::LockScriptQuery, error::CoreError, protocol::ScriptRole, view::message_actions,
};

impl LockScriptQuery<'_> {
    pub(crate) fn validate_message_targets(&self, message: &[u8]) -> Result<(), CoreError> {
        for action in message_actions(message)? {
            let role = ScriptRole::try_from(action.script_role)?;
            let target_exists = match role {
                ScriptRole::InputLock => self
                    .context
                    .script_hashes
                    .input_locks
                    .contains(&action.script_hash),
                ScriptRole::InputType => self
                    .context
                    .script_hashes
                    .input_types
                    .iter()
                    .flatten()
                    .any(|hash| *hash == action.script_hash),
                ScriptRole::OutputType => self
                    .context
                    .script_hashes
                    .output_types
                    .iter()
                    .flatten()
                    .any(|hash| *hash == action.script_hash),
            };
            if !target_exists {
                return Err(CoreError::InvalidMessageTarget);
            }
        }
        Ok(())
    }
}
