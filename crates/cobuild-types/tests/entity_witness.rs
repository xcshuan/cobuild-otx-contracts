use cobuild_types::entity::witness::WitnessLayout;
use molecule::prelude::Entity as _;

#[test]
fn entity_witness_default_serializes() {
    let witness = WitnessLayout::default();
    let bytes = witness.as_slice();
    assert!(!bytes.is_empty());
}
