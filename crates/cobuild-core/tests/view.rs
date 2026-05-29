use cobuild_core::view::SliceReader;
use cobuild_core::view::WitnessLayoutView;
use cobuild_types::lazy_reader::support::{Error as MoleculeError, Read};

#[test]
fn empty_witness_is_not_a_cobuild_layout() {
    assert!(WitnessLayoutView::from_slice(&[]).is_err());
}

#[test]
fn slice_reader_reports_out_of_bound_offsets() {
    let reader = SliceReader::new(&[]);
    let mut buf = [0u8; 1];
    assert!(matches!(
        reader.read(&mut buf, 1),
        Err(MoleculeError::OutOfBound(1, 0))
    ));
}
