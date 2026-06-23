use super::*;

#[test]
fn signing_hash_oracle_otx_uses_remapped_witness_handle() {
    let secret_key = fixed_secret_key(1);
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::NonContiguousOtxWitness);

    let facts = sign_scope(
        &built,
        &TestSigningHashOracle,
        SignerId("owner"),
        &secret_key,
        [9; 32],
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    assert_eq!(facts.carrier, built.otx_witness(otx));
}

#[test]
fn signing_hash_oracle_tx_without_message_uses_resolved_facts() {
    let mut shape = TxShape::new();
    shape.push_prefix_input(signing_resolved_input(1, vec![0xaa, 0xbb]));
    let built = shape.build();
    let oracle = TestSigningHashOracle;

    let inputs: Vec<_> = built
        .resolved_inputs
        .iter()
        .map(|input| (input.output.as_slice(), input.data.as_ref()))
        .collect();
    let witnesses: Vec<_> = built
        .tx
        .witnesses()
        .into_iter()
        .map(|witness| witness.raw_data().to_vec())
        .collect();
    let expected = tx_without_message_hash_for_inputs(
        packed_hash_to_array(built.tx.hash()),
        &inputs,
        &witnesses,
    );

    assert_eq!(oracle.tx_without_message(&built), expected);
}

#[test]
fn signing_hash_oracle_tx_with_message_changes_when_message_changes() {
    let mut shape = TxShape::new();
    shape.push_prefix_input(signing_resolved_input(1, vec![0xaa]));
    let built = shape.build();
    let oracle = TestSigningHashOracle;
    let first = CobuildMessageBuilder::new()
        .input_lock_action([1; 32])
        .action_data(vec![1])
        .build();
    let second = CobuildMessageBuilder::new()
        .input_lock_action([1; 32])
        .action_data(vec![2])
        .build();

    assert_hash_changed(
        oracle.tx_with_message(&built, &first),
        oracle.tx_with_message(&built, &second),
    );
}

#[test]
fn signing_hash_oracle_otx_base_changes_when_resolved_input_data_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_cell_deps: vec![signing_cell_dep(3)],
        base_header_deps: vec![[4; 32]],
        ..Default::default()
    });
    let built = shape.build();
    let mut changed = built.clone();
    changed.resolved_inputs[0].data = Bytes::from(vec![0xcc]);
    let oracle = TestSigningHashOracle;

    assert_hash_changed(oracle.otx_base(&built, otx), oracle.otx_base(&changed, otx));
}

#[test]
fn signing_hash_oracle_otx_base_all_uncovered_fields_matches_default_slot_golden() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_cell_deps: vec![signing_cell_dep(3)],
        base_header_deps: vec![[4; 32]],
        base_input_masks: Some(base_input_mask(1).bytes()),
        base_output_masks: Some(base_output_mask(1).bytes()),
        base_cell_dep_masks: Some(base_cell_dep_item_mask(1).bytes()),
        base_header_dep_masks: Some(base_header_dep_item_mask(1).bytes()),
        ..Default::default()
    });
    let built = shape.build();

    let actual = TestSigningHashOracle.otx_base(&built, otx);
    let expected = [
        0x5e, 0xd5, 0x73, 0xc8, 0x6c, 0xa8, 0x64, 0xc8, 0x67, 0xe6, 0x52, 0x3c, 0x68, 0x57, 0x8c,
        0x89, 0x76, 0x26, 0x59, 0xc0, 0x8f, 0x33, 0xab, 0xdf, 0x50, 0xa8, 0x9c, 0x2f, 0xd7, 0x76,
        0x01, 0x20,
    ];

    assert_eq!(actual, expected);
}

#[test]
fn signing_hash_oracle_default_literals_match_packed_defaults() {
    let default_script = [
        53, 0, 0, 0, 16, 0, 0, 0, 48, 0, 0, 0, 49, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    assert_eq!(
        [0u8; 36].as_slice(),
        OutPoint::new_builder().build().as_slice()
    );
    assert_eq!(
        default_script.as_slice(),
        Script::new_builder().build().as_slice()
    );
    assert_eq!(&[] as &[u8], ScriptOpt::new_builder().build().as_slice());
    assert_eq!(
        [0u8; 37].as_slice(),
        CellDep::new_builder().build().as_slice()
    );
}

#[test]
fn signing_hash_oracle_otx_base_changes_when_covered_previous_output_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_input_masks: Some(
            base_input_mask(1)
                .cover_field(0, BaseInputMaskField::PreviousOutput)
                .bytes(),
        ),
        ..Default::default()
    });
    let input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input_with_previous_output_tag(1, 9, vec![0xaa]),
    });

    assert_hash_changed(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_since_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_input_masks: Some(
            base_input_mask(1)
                .cover_field(0, BaseInputMaskField::PreviousOutput)
                .bytes(),
        ),
        ..Default::default()
    });
    let input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input_with_since(1, 99, vec![0xaa]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_previous_output_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_input_masks: Some(
            base_input_mask(1)
                .cover_field(0, BaseInputMaskField::Since)
                .bytes(),
        ),
        ..Default::default()
    });
    let input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input_with_previous_output_tag(1, 9, vec![0xaa]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_capacity_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Lock)
                .cover_field(0, BaseOutputMaskField::Type)
                .cover_field(0, BaseOutputMaskField::Data)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output_with_lock_tag(9, 2, vec![0xbb]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_lock_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .cover_field(0, BaseOutputMaskField::Type)
                .cover_field(0, BaseOutputMaskField::Data)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output_with_lock_tag(2, 9, vec![0xbb]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_type_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .cover_field(0, BaseOutputMaskField::Lock)
                .cover_field(0, BaseOutputMaskField::Data)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_typed_output(2, 9, vec![0xbb]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_data_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .cover_field(0, BaseOutputMaskField::Lock)
                .cover_field(0, BaseOutputMaskField::Type)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output(2, vec![0xcc]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_cell_dep_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_cell_deps: vec![signing_cell_dep(3)],
        base_cell_dep_masks: Some(base_cell_dep_item_mask(1).bytes()),
        ..Default::default()
    });
    let cell_dep = shape.otx_base_cell_dep(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceCellDep {
        cell_dep,
        replacement: signing_cell_dep(9),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_header_dep_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_header_deps: vec![[4; 32]],
        base_header_dep_masks: Some(base_header_dep_item_mask(1).bytes()),
        ..Default::default()
    });
    let header_dep = shape.otx_base_header_dep(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceHeaderDep {
        header_dep,
        replacement: [9; 32],
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_input_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_inputs(vec![signing_resolved_input(2, vec![0xbb])]),
        ],
        ..Default::default()
    });
    let input = shape.otx_append_input(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input(9, vec![0xbb]),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_output_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![signing_output(2, vec![0xbb])]),
        ],
        ..Default::default()
    });
    let output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output(9, vec![0xbb]),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_output_order_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![append_segment_spec(0x00).with_outputs(vec![
            signing_output(2, vec![0xbb]),
            signing_output(3, vec![0xcc]),
        ])],
        ..Default::default()
    });
    let first = shape.otx_append_output(otx, 0);
    let second = shape.otx_append_output(otx, 1);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash);

    built.apply_shape_mutation(TxShapeMutation::SwapOutputs {
        left: first,
        right: second,
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_cell_dep_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![append_segment_spec(0x00).with_cell_deps(vec![signing_cell_dep(2)])],
        ..Default::default()
    });
    let cell_dep = shape.otx_append_cell_dep(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceCellDep {
        cell_dep,
        replacement: signing_cell_dep(9),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_header_dep_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![append_segment_spec(0x00).with_header_deps(vec![[2; 32]])],
        ..Default::default()
    });
    let header_dep: HeaderDepHandle = shape.otx_append_header_dep(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceHeaderDep {
        header_dep,
        replacement: [9; 32],
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_binds_base_hash() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(3, vec![0xcc])],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![signing_resolved_input(2, vec![0xbb])])
                .with_outputs(vec![signing_output(4, vec![0xdd])])
                .with_cell_deps(vec![signing_cell_dep(5)])
                .with_header_deps(vec![[6; 32]]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let oracle = TestSigningHashOracle;

    assert_hash_changed(
        oracle.otx_append_segment(&built, otx, 0, [1; 32]),
        oracle.otx_append_segment(&built, otx, 0, [2; 32]),
    );
}

#[test]
fn signing_hash_oracle_otx_append_message_is_bound_by_base_hash_only() {
    let mut shape = TxShape::new();
    let message = CobuildMessageBuilder::new()
        .input_lock_action([1; 32])
        .action_data(vec![1])
        .build();
    let changed_message = CobuildMessageBuilder::new()
        .input_lock_action([1; 32])
        .action_data(vec![2])
        .build();
    let otx = shape.push_otx(OtxSegment {
        message: Some(message),
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![signing_output(2, vec![0xbb])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let changed_otx = otx_witness(&built, otx)
        .as_builder()
        .message(changed_message)
        .build();
    let changed_witness = WitnessLayout::from(changed_otx);
    let changed = signing_replace_otx_witness(
        built.clone(),
        Bytes::copy_from_slice(changed_witness.as_slice()),
    );
    let base_hash = [3; 32];

    assert_hash_unchanged(
        TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash),
        TestSigningHashOracle.otx_append_segment(&changed, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_count_comes_from_witness() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![signing_output(2, vec![0xbb])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let changed = signing_replace_otx_witness(
        built.clone(),
        signing_otx_witness_with_append_output_count(0),
    );
    let oracle = TestSigningHashOracle;
    let base_hash = [3; 32];

    assert_hash_changed(
        oracle.otx_append_segment(&built, otx, 0, base_hash),
        oracle.otx_append_segment(&changed, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_segment_flags_come_from_witness() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![signing_output(2, vec![0xbb])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let changed = signing_replace_otx_witness(
        built.clone(),
        signing_otx_witness_with_append_segment_flags(0x01),
    );
    let oracle = TestSigningHashOracle;
    let base_hash = [3; 32];

    assert_hash_changed(
        oracle.otx_append_segment(&built, otx, 0, base_hash),
        oracle.otx_append_segment(&changed, otx, 0, base_hash),
    );
}

#[test]
#[should_panic(expected = "witness append segment count exceeds built range facts")]
fn signing_hash_oracle_rejects_witness_append_segment_count_expansion() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![signing_output(2, vec![0xbb])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let changed =
        signing_replace_otx_witness(built, signing_otx_witness_with_two_append_segments());

    TestSigningHashOracle.otx_append_segment(&changed, otx, 1, [3; 32]);
}

#[test]
fn signing_hash_oracle_segment_own_coverage_does_not_bind_later_segment() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x01)
                .with_inputs(vec![signing_resolved_input(2, vec![0xbb])])
                .with_outputs(vec![signing_output(3, vec![0xcc])]),
            append_segment_spec(0x00)
                .with_inputs(vec![signing_resolved_input(4, vec![0xdd])])
                .with_outputs(vec![signing_output(5, vec![0xee])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 0, base_hash);
    let later_output = built.otx_append_segment_output(otx, 1, 0);
    let mut changed = built.clone();

    changed.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output: later_output,
        replacement: signing_output(9, vec![0xff]),
    });

    assert_hash_unchanged(
        before,
        TestSigningHashOracle.otx_append_segment(&changed, otx, 0, base_hash),
    );
}

#[test]
fn signing_hash_oracle_own_only_segment_is_positionless() {
    let mut first_shape = TxShape::new();
    let first_otx = first_shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x00).with_outputs(vec![signing_output(3, vec![0xcc])]),
        ],
        ..Default::default()
    });
    let first = first_shape.build();

    let mut second_shape = TxShape::new();
    let second_otx = second_shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x01).with_outputs(vec![signing_output(2, vec![0xbb])]),
            append_segment_spec(0x00).with_outputs(vec![signing_output(3, vec![0xcc])]),
        ],
        ..Default::default()
    });
    let second = second_shape.build();

    let first_base_hash = TestSigningHashOracle.otx_base(&first, first_otx);
    let second_base_hash = TestSigningHashOracle.otx_base(&second, second_otx);
    assert_eq!(first_base_hash, second_base_hash);

    assert_hash_unchanged(
        TestSigningHashOracle.otx_append_segment(&first, first_otx, 0, first_base_hash),
        TestSigningHashOracle.otx_append_segment(&second, second_otx, 1, second_base_hash),
    );
}

#[test]
fn signing_hash_oracle_segment_previous_coverage_binds_previous_segment() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x01).with_outputs(vec![signing_output(2, vec![0xbb])]),
            append_segment_spec(0x02).with_outputs(vec![signing_output(3, vec![0xcc])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append_segment(&built, otx, 1, base_hash);
    let previous_output = built.otx_append_segment_output(otx, 0, 0);
    let mut changed = built.clone();

    changed.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output: previous_output,
        replacement: signing_output(9, vec![0xdd]),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append_segment(&changed, otx, 1, base_hash),
    );
}

#[test]
fn signing_hash_oracle_sign_scope_records_facts() {
    let mut shape = TxShape::new();
    shape.push_prefix_input(signing_resolved_input(1, vec![0xaa]));
    let built = shape.build();
    let oracle = TestSigningHashOracle;
    let secret_key = fixed_secret_key(7);
    let script_hash = [8; 32];
    let scope = SignatureScope::TxWithoutMessage;

    let facts = sign_scope(
        &built,
        &oracle,
        SignerId("alice"),
        &secret_key,
        script_hash,
        WitnessHandle::from_raw(0),
        scope,
    );

    assert_eq!(facts.signer, SignerId("alice"));
    assert_eq!(facts.scope, scope);
    assert_eq!(facts.carrier, WitnessHandle::from_raw(0));
    assert_eq!(facts.script_hash, script_hash);
    assert_eq!(facts.signing_hash, oracle.tx_without_message(&built));
    assert_eq!(facts.seal.len(), 65);
}
