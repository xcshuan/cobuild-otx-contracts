use cobuild_core::layout::{build_layout, LayoutTx};

#[test]
fn empty_tx_has_no_otx_layouts() {
    let layout = build_layout(&LayoutTx {
        witnesses: Vec::new(),
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    })
    .unwrap();
    assert!(layout.otxs.is_empty());
}
