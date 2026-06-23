use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LegacySettlementCase {
    AtLimitPrice,
    BelowLimitPrice,
}

pub fn type_script_legacy_settlement_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        legacy_settlement_case(LegacySettlementCase::AtLimitPrice),
        legacy_settlement_case(LegacySettlementCase::BelowLimitPrice),
    ]
}

fn legacy_settlement_case(case: LegacySettlementCase) -> BuiltLimitOrderCase {
    let mut fixture = CobuildTestFixture::new();
    let limit_order = fixture.deploy_limit_order();
    let always_success_code = deploy_always_success_code(fixture.context_mut());
    let always_success =
        build_always_success_script(fixture.context_mut(), &always_success_code, Vec::new());
    let owner_lock = always_success.script.clone();
    let settlement_amount = match case {
        LegacySettlementCase::AtLimitPrice => 30,
        LegacySettlementCase::BelowLimitPrice => 29,
    };

    let order_input = limit_order_input(
        &mut fixture,
        owner_lock.clone(),
        OFFERED_ASSET_ID,
        REQUESTED_ASSET_ID,
        30,
        &limit_order.script,
    );
    let settlement_output = TestCellOutput::new(
        normal_output(owner_lock.clone(), 90_000_000_000),
        settlement_data(REQUESTED_ASSET_ID, settlement_amount),
    );

    let mut shape = TxShape::new();
    push_deps(&mut shape, [&limit_order, &always_success]);
    let otx = shape.push_otx(OtxSpec {
        base_inputs: vec![order_input],
        append_segments: vec![append_segment_spec(0x00).with_outputs(vec![settlement_output])],
        ..Default::default()
    });
    let order = shape.otx_base_input(otx, 0);
    let payment = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let message = fill_message(
        &fixture,
        limit_order.script_hash,
        payment,
        script_hash(&owner_lock),
        &built,
    );
    replace_otx_message(&mut built, otx, message);

    built_case(
        format!("legacy_settlement::{case:?}"),
        fixture,
        built,
        input_type_error(order, LimitOrderTypeError::InvalidPayment),
        coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputType,
            OtxScopeKind::AppendOutput,
            super::ActionSourceKind::Otx,
            Some(match case {
                LegacySettlementCase::AtLimitPrice => BusinessMutation::PaymentOutputWrongUdt,
                LegacySettlementCase::BelowLimitPrice => {
                    BusinessMutation::PaymentOutputInsufficient
                }
            }),
        ),
    )
}
