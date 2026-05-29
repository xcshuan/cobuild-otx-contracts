use cobuild_types::lazy_reader::support::Cursor;
use cobuild_types::lazy_reader::witness::WitnessLayout;

#[test]
fn lazy_reader_witness_rejects_empty_cursor() {
    let cursor = Cursor::new(0, Box::new(Vec::new()));
    assert!(WitnessLayout::try_from(cursor).is_err());
}
