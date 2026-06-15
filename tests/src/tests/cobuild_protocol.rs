#[test]
fn framework_otx_builder_defaults_to_neutral_layout() {
    let built = crate::framework::cobuild::OtxBuilder::new().build_with_layout();

    assert_eq!(built.base_input_cells, 0);
    assert_eq!(built.base_output_cells, 0);
    assert_eq!(built.base_cell_deps, 0);
    assert_eq!(built.base_header_deps, 0);
    assert_eq!(built.append_input_cells, 0);
    assert_eq!(built.append_output_cells, 0);
    assert_eq!(built.append_cell_deps, 0);
    assert_eq!(built.append_header_deps, 0);
}

#[test]
fn cobuild_protocol_builders_encode_raw_permissions_and_masks() {
    let built = crate::framework::cobuild::RawOtxBuilder::new()
        .append_permissions(0x10)
        .base_input_masks(vec![0xff])
        .build_with_layout();

    assert_eq!(built.otx.append_permissions().as_slice(), &[0x10]);
    assert_eq!(built.otx.base_input_masks().raw_data().as_ref(), &[0xff]);
}

#[test]
fn cobuild_protocol_builders_set_append_dep_permission_bits() {
    let built = crate::framework::cobuild::OtxBuilder::new()
        .allow_append_cell_deps()
        .allow_append_header_deps()
        .build_with_layout();

    assert_eq!(built.otx.append_permissions().as_slice(), &[0b1100]);
}

#[test]
fn cobuild_protocol_builders_size_semantic_base_masks() {
    let input_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_input_cells(5)
        .build()
        .base_input_masks();
    assert_eq!(input_masks.len(), 2);

    let cell_dep_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_cell_deps(9)
        .build()
        .base_cell_dep_masks();
    assert_eq!(cell_dep_masks.len(), 2);

    let header_dep_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_header_deps(9)
        .build()
        .base_header_dep_masks();
    assert_eq!(header_dep_masks.len(), 2);
}

#[test]
fn cobuild_protocol_builders_preserve_malformed_raw_mask_lengths() {
    let cell_dep_masks = crate::framework::cobuild::RawOtxBuilder::new()
        .base_cell_deps(9)
        .base_cell_dep_masks(vec![0xff])
        .build()
        .base_cell_dep_masks();

    assert_eq!(cell_dep_masks.len(), 1);
    assert_eq!(cell_dep_masks.raw_data().as_ref(), &[0xff]);
}

#[test]
fn cobuild_protocol_builders_cover_and_uncover_input_mask_bits() {
    let covered = crate::framework::cobuild::OtxBuilder::new()
        .base_input_cells(1)
        .cover_base_input_since(0)
        .cover_base_input_previous_output(0)
        .build_with_layout();
    assert_eq!(
        covered.otx.base_input_masks().raw_data().as_ref(),
        &[0b0011]
    );

    let uncovered = crate::framework::cobuild::OtxBuilder::new()
        .base_input_cells(1)
        .cover_base_input_since(0)
        .cover_base_input_previous_output(0)
        .uncover_base_input_since(0)
        .uncover_base_input_previous_output(0)
        .build_with_layout();
    assert_eq!(uncovered.otx.base_input_masks().raw_data().as_ref(), &[0]);
}

#[test]
fn cobuild_protocol_builders_cover_and_uncover_output_mask_bits() {
    let uncovered = crate::framework::cobuild::OtxBuilder::new()
        .base_output_cells(1)
        .uncover_base_output_capacity(0)
        .uncover_base_output_lock(0)
        .uncover_base_output_type(0)
        .uncover_base_output_data(0)
        .build_with_layout();
    assert_eq!(
        uncovered.otx.base_output_masks().raw_data().as_ref(),
        &[0b0000]
    );

    let covered = crate::framework::cobuild::OtxBuilder::new()
        .base_output_cells(1)
        .uncover_base_output_capacity(0)
        .uncover_base_output_lock(0)
        .uncover_base_output_type(0)
        .uncover_base_output_data(0)
        .cover_base_output_capacity(0)
        .cover_base_output_lock(0)
        .cover_base_output_type(0)
        .cover_base_output_data(0)
        .build_with_layout();
    assert_eq!(
        covered.otx.base_output_masks().raw_data().as_ref(),
        &[0b1111]
    );
}

#[test]
fn cobuild_protocol_builders_create_full_base_masks() {
    assert_eq!(
        crate::framework::cobuild::full_base_input_masks(1),
        vec![0b0011]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_input_masks(5),
        vec![0xff, 0b0011]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_output_masks(1),
        vec![0b1111]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_output_masks(3),
        vec![0xff, 0b1111]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_cell_dep_masks(1),
        vec![0b0001]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_cell_dep_masks(9),
        vec![0xff, 0b0001]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_header_dep_masks(1),
        vec![0b0001]
    );
    assert_eq!(
        crate::framework::cobuild::full_base_header_dep_masks(9),
        vec![0xff, 0b0001]
    );
}

#[test]
fn cobuild_protocol_builders_create_partial_base_masks() {
    use crate::framework::cobuild::{
        BaseInputMaskField, BaseOutputMaskField, base_cell_dep_masks, base_header_dep_masks,
        base_input_masks, base_output_masks,
    };

    assert_eq!(
        base_input_masks(2, &[(0, BaseInputMaskField::PreviousOutput)]),
        vec![0b0010]
    );
    assert_eq!(
        base_input_masks(
            2,
            &[
                (0, BaseInputMaskField::Since),
                (1, BaseInputMaskField::PreviousOutput),
            ],
        ),
        vec![0b1001]
    );
    assert_eq!(
        base_output_masks(
            2,
            &[
                (0, BaseOutputMaskField::Lock),
                (1, BaseOutputMaskField::Data),
            ],
        ),
        vec![0b1000_0010]
    );
    assert_eq!(base_cell_dep_masks(3, &[0, 2]), vec![0b0101]);
    assert_eq!(
        base_header_dep_masks(9, &[0, 8]),
        vec![0b0000_0001, 0b0000_0001]
    );
}

#[test]
fn cobuild_protocol_mask_dsl_distinguishes_field_and_item_masks() {
    use crate::framework::cobuild::{
        BaseInputMaskField, BaseOutputMaskField, base_cell_dep_item_mask,
        base_header_dep_item_mask, base_input_mask, base_output_mask,
    };

    assert_eq!(
        base_input_mask(2)
            .cover_field(0, BaseInputMaskField::Since)
            .cover_field(1, BaseInputMaskField::PreviousOutput)
            .bytes(),
        vec![0b1001]
    );
    assert_eq!(
        base_output_mask(2)
            .cover_field(0, BaseOutputMaskField::Lock)
            .cover_field(1, BaseOutputMaskField::Data)
            .bytes(),
        vec![0b1000_0010]
    );
    assert_eq!(
        base_cell_dep_item_mask(3)
            .cover_item(0)
            .cover_item(2)
            .bytes(),
        vec![0b0101]
    );
    assert_eq!(
        base_header_dep_item_mask(9)
            .cover_item(0)
            .cover_item(8)
            .bytes(),
        vec![0b0000_0001, 0b0000_0001]
    );
}

#[test]
fn cobuild_protocol_builders_encode_custom_otx_start_spec() {
    let witness = crate::framework::cobuild::OtxStartSpec {
        start_input_cell: 1,
        start_output_cell: 2,
        start_cell_deps: 3,
        start_header_deps: 4,
    }
    .encode();

    assert!(
        witness
            .windows(4)
            .any(|window| window == 1u32.to_le_bytes())
    );
    assert!(
        witness
            .windows(4)
            .any(|window| window == 2u32.to_le_bytes())
    );
    assert!(
        witness
            .windows(4)
            .any(|window| window == 3u32.to_le_bytes())
    );
    assert!(
        witness
            .windows(4)
            .any(|window| window == 4u32.to_le_bytes())
    );
}

#[test]
fn cobuild_protocol_builders_preserve_raw_cobuild_witness_bytes() {
    let raw = ckb_testtool::ckb_types::bytes::Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]);
    let encoded = crate::framework::cobuild::WitnessSpec::RawCobuild(raw.clone()).encode();

    assert_eq!(encoded, raw);
}
