use cobuild_types::entity::core::{Action, Otx, OtxStart, SealPair};
use molecule::prelude::Entity;

#[test]
fn generated_entities_exist_with_expected_names() {
    assert_eq!(Action::NAME, "Action");
    assert_eq!(SealPair::NAME, "SealPair");
    assert_eq!(OtxStart::NAME, "OtxStart");
    assert_eq!(Otx::NAME, "Otx");
}
