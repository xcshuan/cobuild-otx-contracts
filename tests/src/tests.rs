use crate::{default_test_env, TestEnv};

#[test]
fn loader_defaults_to_debug_build_when_mode_is_unset() {
    assert_eq!(default_test_env(), TestEnv::Debug);
}
