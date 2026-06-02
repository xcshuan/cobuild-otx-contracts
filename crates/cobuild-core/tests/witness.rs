use cobuild_core::witness::{parse_witness, ParsedWitness};

#[test]
fn non_cobuild_witness_returns_none() {
    assert!(matches!(
        parse_witness(&[0, 1, 2, 3]),
        Ok(ParsedWitness::None)
    ));
}

#[test]
fn truncated_cobuild_union_returns_none() {
    assert!(matches!(
        parse_witness(&0xFF000001u32.to_le_bytes()),
        Ok(ParsedWitness::None)
    ));
}
