use super::*;
#[test]
fn mutation_move_output_to_remainder_keeps_output_handle_stable() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc]), signing_output(4, vec![0xdd])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let old_index = built.outputs.tx_index(moved_output);
    assert_eq!(old_index, 1);

    assert_eq!(
        built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
            output: moved_output,
        }),
        None
    );

    let new_index = built.outputs.tx_index(moved_output);
    assert_eq!(new_index, built.tx.outputs().len() - 1);
    assert_eq!(
        built.outputs.handle_at_tx_index(new_index),
        Some(moved_output)
    );
    assert!(!built.otx_ranges[0].append_outputs.contains(&new_index));
    assert_eq!(built.otx_ranges[0].append_outputs, 1..2);
}

#[test]
fn mutation_move_append_output_to_remainder_updates_otx_witness_count_for_signing_oracle() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_outputs: vec![signing_output(3, vec![0xcc]), signing_output(4, vec![0xdd])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    assert_eq!(
        molecule_u32(otx_witness(&built, otx).append_output_cells()),
        1
    );
    let oracle = TestSigningHashOracle;
    let actual_hash = oracle.otx_append(&built, otx, [3; 32]);
    let expected = signing_replace_otx_witness(
        built.clone(),
        signing_otx_witness_with_append_output_count(1),
    );
    assert_eq!(actual_hash, oracle.otx_append(&expected, otx, [3; 32]));
}

#[test]
fn mutation_move_base_output_to_remainder_updates_otx_witness_count_and_mask() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let moved_output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(molecule_u32(mutated.base_output_cells()), 1);
    assert_eq!(mutated.base_output_masks().raw_data().as_ref(), &[0x0f]);
    TestSigningHashOracle.otx_base(&built, otx);
}

#[test]
fn mutation_move_append_output_to_remainder_preserves_non_target_otx_witness_fields() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_inputs: vec![signing_resolved_input(5, vec![0xee])],
        base_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        append_outputs: vec![signing_output(4, vec![0xdd]), signing_output(6, vec![0xff])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let (witness, original_otx) = signing_otx_witness_with_message_seal_and_outputs(2, 2);
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(molecule_u32(mutated.append_output_cells()), 1);
    assert_same_message_seals_and_permissions(&mutated, &original_otx);
    assert_same_base_inputs(&mutated, &original_otx);
    assert_same_base_outputs(&mutated, &original_otx);
    assert_eq!(
        molecule_u32(mutated.append_input_cells()),
        molecule_u32(original_otx.append_input_cells()),
        "append input cells"
    );
    assert_eq!(
        molecule_u32(mutated.append_cell_deps()),
        molecule_u32(original_otx.append_cell_deps()),
        "append cell deps"
    );
    assert_eq!(
        molecule_u32(mutated.append_header_deps()),
        molecule_u32(original_otx.append_header_deps()),
        "append header deps"
    );
}

#[test]
fn mutation_move_base_output_to_remainder_preserves_non_target_otx_witness_fields() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_inputs: vec![signing_resolved_input(5, vec![0xee])],
        base_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        append_outputs: vec![signing_output(4, vec![0xdd]), signing_output(6, vec![0xff])],
        ..Default::default()
    });
    let moved_output = shape.otx_base_output(otx, 0);
    let (witness, original_otx) = signing_otx_witness_with_message_seal_and_outputs(2, 2);
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(molecule_u32(mutated.base_output_cells()), 1);
    assert_eq!(mutated.base_output_masks().raw_data().as_ref(), &[0x0a]);
    assert_same_message_seals_and_permissions(&mutated, &original_otx);
    assert_same_base_inputs(&mutated, &original_otx);
    assert_same_append_counts(&mutated, &original_otx);
}

#[test]
fn expected_outcome_output_type_resolves_moved_output_after_mutation() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc]), signing_output(4, vec![0xdd])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });
    let current_index = built.outputs.tx_index(moved_output);
    let error = ScriptError::ValidationFailure("by convention".to_owned(), 14)
        .output_type_script(current_index)
        .into();

    ExpectedOutcome::ScriptExit {
        location: ScriptLocation::OutputType(moved_output),
        code: 14,
    }
    .assert_result(Err(error), &built);
}

#[test]
fn mutation_replace_witness_updates_bytes_through_witness_handle() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let otx_witness = WitnessHandle::from_raw(1);
    let replacement = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]);

    built.apply_shape_mutation(TxShapeMutation::ReplaceWitness {
        witness: otx_witness,
        replacement: replacement.clone(),
    });

    assert_eq!(witness_bytes(&built, otx_witness), replacement);
}

#[test]
fn mutation_otx_start_raw_replaces_start_witness_bytes() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let start_witness = WitnessHandle::from_raw(0);
    let replacement = OtxStartSpec {
        start_input_cell: 1,
        start_output_cell: 2,
        start_cell_deps: 3,
        start_header_deps: 4,
    }
    .encode();

    built.apply_protocol_mutation(ProtocolMutation::OtxStartRaw(OtxStartSpec {
        start_input_cell: 1,
        start_output_cell: 2,
        start_cell_deps: 3,
        start_header_deps: 4,
    }));

    assert_eq!(witness_bytes(&built, start_witness), replacement);
}

#[test]
fn mutation_otx_start_raw_uses_start_handle_after_tx_level_witness() {
    let mut shape = TxShape::new();
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_lock_action([1; 32])
            .action_data(vec![1])
            .build(),
    );
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let tx_level_witness = WitnessHandle::from_raw(0);
    let tx_level_before = witness_bytes(&built, tx_level_witness);
    let start_witness = built.otx_start_witness();
    let replacement = OtxStartSpec {
        start_input_cell: 9,
        start_output_cell: 8,
        start_cell_deps: 7,
        start_header_deps: 6,
    }
    .encode();

    built.apply_protocol_mutation(ProtocolMutation::OtxStartRaw(OtxStartSpec {
        start_input_cell: 9,
        start_output_cell: 8,
        start_cell_deps: 7,
        start_header_deps: 6,
    }));

    assert_eq!(witness_bytes(&built, tx_level_witness), tx_level_before);
    assert_eq!(witness_bytes(&built, start_witness), replacement);
}

#[test]
fn mutation_duplicate_sighash_all_inserts_two_sighash_witnesses() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::DuplicateSighashAll);

    assert_eq!(built.tx.witnesses().len(), 4);
    for index in 0..2 {
        let witness = built
            .tx
            .witnesses()
            .into_iter()
            .nth(index)
            .expect("inserted sighash witness");
        match WitnessLayout::from_slice(witness.raw_data().as_ref())
            .expect("parse sighash witness")
            .to_enum()
        {
            WitnessLayoutUnion::SighashAll(_) => {}
            other => panic!("expected SighashAll witness, got {}", other.item_name()),
        }
    }
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 2);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 3);
}

#[test]
fn mutation_duplicate_otx_start_inserts_second_start_before_original() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::DuplicateOtxStart);

    assert_eq!(built.tx.witnesses().len(), 3);
    for index in 0..2 {
        let witness = built
            .tx
            .witnesses()
            .into_iter()
            .nth(index)
            .expect("OTX start witness");
        match WitnessLayout::from_slice(witness.raw_data().as_ref())
            .expect("parse OTX start witness")
            .to_enum()
        {
            WitnessLayoutUnion::OtxStart(_) => {}
            other => panic!("expected OtxStart witness, got {}", other.item_name()),
        }
    }
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 1);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 2);
}

#[test]
fn mutation_non_contiguous_otx_witness_inserts_gap_after_start() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::NonContiguousOtxWitness);

    assert_eq!(built.tx.witnesses().len(), 3);
    let gap = built
        .tx
        .witnesses()
        .into_iter()
        .nth(1)
        .expect("inserted witness gap");
    match WitnessLayout::from_slice(gap.raw_data().as_ref())
        .expect("parse witness gap")
        .to_enum()
    {
        WitnessLayoutUnion::SighashAllOnly(_) => {}
        other => panic!("expected SighashAllOnly witness, got {}", other.item_name()),
    }
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 0);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 2);
}

#[test]
fn mutation_otx_before_otx_start_swaps_witness_handles() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::OtxBeforeOtxStart);

    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 1);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 0);
    match WitnessLayout::from_slice(
        built
            .tx
            .witnesses()
            .into_iter()
            .next()
            .expect("first witness")
            .raw_data()
            .as_ref(),
    )
    .expect("parse first witness")
    .to_enum()
    {
        WitnessLayoutUnion::Otx(_) => {}
        other => panic!("expected OTX witness, got {}", other.item_name()),
    }
}

#[test]
fn mutation_seal_scope_raw_patches_matching_otx_seal() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::SealScopeRaw {
        otx,
        script_hash: [7; 32],
        scope: 0xfe,
    });

    let mutated = otx_witness(&built, otx);
    assert_ne!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert_eq!(
        mutated.message().as_slice(),
        original_otx.message().as_slice()
    );
    assert_eq!(
        mutated.append_permissions().as_slice(),
        original_otx.append_permissions().as_slice()
    );
}

#[test]
fn mutation_seal_raw_patches_matching_otx_seal_bytes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::SealRaw {
        otx,
        script_hash: [7; 32],
        scope: 0x42,
        seal: vec![0xde, 0xad],
    });

    let mutated = otx_witness(&built, otx);
    assert_ne!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert!(
        mutated
            .seals()
            .into_iter()
            .any(|seal| seal.seal().raw_data().as_ref() == [0xde, 0xad])
    );
}

#[test]
fn mutation_otx_raw_permission_changes_witness_bytes_and_signing_hash() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let otx_witness = WitnessHandle::from_raw(1);
    let before_witness = witness_bytes(&built, otx_witness);
    let oracle = TestSigningHashOracle;
    let before_hash = oracle.otx_base(&built, otx);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawPermission {
        otx,
        permissions: 0xf0,
    });

    assert_ne!(witness_bytes(&built, otx_witness), before_witness);
    assert_hash_changed(before_hash, oracle.otx_base(&built, otx));
}

#[test]
fn mutation_otx_raw_permission_preserves_existing_message_and_seals() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawPermission {
        otx,
        permissions: 0xf0,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(mutated.append_permissions().as_slice(), &[0xf0]);
    assert_eq!(
        mutated.message().as_slice(),
        original_otx.message().as_slice()
    );
    assert_eq!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert_same_base_inputs(&mutated, &original_otx);
    assert_same_base_outputs(&mutated, &original_otx);
    assert_same_append_counts(&mutated, &original_otx);
}

#[test]
fn mutation_otx_raw_base_input_masks_changes_witness_bytes_and_signing_hash() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let otx_witness = WitnessHandle::from_raw(1);
    let before_witness = witness_bytes(&built, otx_witness);
    let oracle = TestSigningHashOracle;
    let before_hash = oracle.otx_base(&built, otx);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawBaseInputMasks {
        otx,
        masks: vec![0xff],
    });

    assert_ne!(witness_bytes(&built, otx_witness), before_witness);
    assert_hash_changed(before_hash, oracle.otx_base(&built, otx));
}

#[test]
fn mutation_otx_raw_base_input_masks_preserves_existing_message_and_seals() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawBaseInputMasks {
        otx,
        masks: vec![0xff],
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(mutated.base_input_masks().raw_data().as_ref(), &[0xff]);
    assert_eq!(
        mutated.message().as_slice(),
        original_otx.message().as_slice()
    );
    assert_eq!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert_eq!(
        mutated.append_permissions().as_slice(),
        original_otx.append_permissions().as_slice(),
        "append permissions"
    );
    assert_eq!(
        molecule_u32(mutated.base_input_cells()),
        molecule_u32(original_otx.base_input_cells()),
        "base input cells"
    );
    assert_same_base_outputs(&mutated, &original_otx);
    assert_same_append_counts(&mutated, &original_otx);
}
