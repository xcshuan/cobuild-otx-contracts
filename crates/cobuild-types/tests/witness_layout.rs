use cobuild_types::entity::core::SighashAllOnly;
use cobuild_types::entity::witness::{WitnessLayout, WitnessLayoutUnion};
use molecule::prelude::*;

#[test]
fn witness_layout_preserves_sighash_all_only_discriminant() {
    let carrier = SighashAllOnly::default();
    let witness = WitnessLayout::new_builder()
        .set(WitnessLayoutUnion::SighashAllOnly(carrier))
        .build();

    let parsed = WitnessLayout::from_slice(witness.as_slice()).unwrap();
    assert_eq!(parsed.item_id(), 4_278_190_082);
    assert_eq!(parsed.to_enum().item_name(), "SighashAllOnly");
}
