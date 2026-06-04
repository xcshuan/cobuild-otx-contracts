use cobuild_core::{
    engine::CobuildEngine,
    source::{InMemorySource, TxCounts},
};

#[test]
fn engine_returns_empty_lock_plan_when_lock_is_absent() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        raw_inputs: vec![Vec::new()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();

    assert_eq!(plan.lock_script_hash, [2u8; 32]);
    assert!(plan.required_signatures.is_empty());
}

#[test]
fn engine_preparation_uses_source_counts() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32], [2u8; 32], [3u8; 32]],
        input_types: vec![Some([4u8; 32]), None, Some([5u8; 32])],
        output_types: vec![None, Some([6u8; 32])],
        raw_inputs: vec![Vec::new(); 3],
        raw_outputs: vec![Vec::new(); 2],
        raw_cell_deps: vec![Vec::new(); 1],
        raw_header_deps: vec![[7u8; 32]; 2],
        witnesses: vec![Vec::new(); 4],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.counts(),
        TxCounts {
            inputs: 3,
            outputs: 2,
            cell_deps: 1,
            header_deps: 2,
            witnesses: 4,
        }
    );
}
