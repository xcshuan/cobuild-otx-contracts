use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    context::ScriptHashIndex, error::CoreError, protocol::ScriptRole, view::message_actions,
};

pub(crate) fn validate_message_targets(
    message: &Cursor,
    script_hashes: &ScriptHashIndex,
) -> Result<(), CoreError> {
    for action in message_actions(message)? {
        let role = ScriptRole::try_from(action.script_role)?;
        let target_exists = match role {
            ScriptRole::InputLock => script_hashes.input_locks.contains(&action.script_hash),
            ScriptRole::InputType => script_hashes
                .input_types
                .iter()
                .flatten()
                .any(|hash| *hash == action.script_hash),
            ScriptRole::OutputType => script_hashes
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
