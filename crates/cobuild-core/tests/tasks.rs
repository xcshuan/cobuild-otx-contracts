use cobuild_core::{
    context::{CobuildContext, TxScriptHashes},
    layout::LayoutTx,
};

#[test]
fn lock_query_without_matching_lock_has_no_tasks() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: Vec::new(),
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32]],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    assert!(context.lock_query([2u8; 32]).tx_tasks().unwrap().is_empty());
}
