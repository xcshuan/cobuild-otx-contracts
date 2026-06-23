use cobuild_types::entity::core::{
    Action, LockSeal, LockSealVec, Otx, OtxAppendSegment, OtxAppendSegmentVec, OtxStart,
};
use molecule::prelude::Entity;

#[test]
fn generated_entities_exist_with_expected_names() {
    assert_eq!(Action::NAME, "Action");
    assert_eq!(LockSeal::NAME, "LockSeal");
    assert_eq!(LockSealVec::NAME, "LockSealVec");
    assert_eq!(OtxAppendSegment::NAME, "OtxAppendSegment");
    assert_eq!(OtxAppendSegmentVec::NAME, "OtxAppendSegmentVec");
    assert_eq!(OtxStart::NAME, "OtxStart");
    assert_eq!(Otx::NAME, "Otx");
}
