use cobuild_core::{
    error::CoreError,
    reader::cursor_from_slice,
    source::{
        ClassifiedCursor, CursorReadContext, HashInputSource, InMemorySource, TransactionSource,
        TxCounts,
    },
};

#[test]
fn classified_cursor_read_error_matches_context() {
    let cursor = cursor_from_slice(&[]);

    assert_eq!(
        ClassifiedCursor::protocol(cursor.clone()).read_error(),
        CoreError::MalformedCobuild
    );
    assert_eq!(
        ClassifiedCursor::source_input(cursor.clone()).read_error(),
        CoreError::InvalidContextInput
    );
    assert_eq!(
        ClassifiedCursor::hash_input(cursor).read_error(),
        CoreError::MissingHashInput
    );
}

#[test]
fn in_memory_source_classifies_hash_payload_misses_as_missing_hash_input() {
    let source = InMemorySource::default();

    assert!(matches!(
        source.raw_input_cursor(0),
        Err(CoreError::MissingHashInput)
    ));
    assert!(matches!(
        source.raw_header_dep_hash(0),
        Err(CoreError::MissingHashInput)
    ));
}

#[test]
fn in_memory_source_classifies_source_and_hash_cursors() {
    let source = InMemorySource {
        transaction: vec![0x11],
        raw_inputs: vec![vec![0x22]],
        witnesses: vec![vec![0x33]],
        ..InMemorySource::default()
    };

    assert_eq!(
        source.transaction_cursor().unwrap().read_context,
        CursorReadContext::SourceInput
    );
    assert_eq!(
        source.raw_input_cursor(0).unwrap().read_context,
        CursorReadContext::HashInput
    );
    assert_eq!(
        source.witness_cursor(0).unwrap().read_context,
        CursorReadContext::HashInput
    );
}

#[test]
fn in_memory_source_exposes_counts_as_one_value() {
    let source = InMemorySource {
        raw_inputs: vec![Vec::new(); 2],
        raw_outputs: vec![Vec::new(); 1],
        raw_cell_deps: vec![Vec::new(); 3],
        raw_header_deps: vec![[0u8; 32]; 1],
        witnesses: vec![Vec::new(); 4],
        ..InMemorySource::default()
    };

    assert_eq!(
        source.counts().unwrap(),
        TxCounts {
            inputs: 2,
            outputs: 1,
            cell_deps: 3,
            header_deps: 1,
            witnesses: 4,
        }
    );
}
