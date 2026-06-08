use tests::fixtures::limit_order::limit_order_lock_nft_for_udt_case;

#[test]
fn limit_order_lock_accepts_nft_for_udt_otx_fill() {
    let (fixture, tx) = limit_order_lock_nft_for_udt_case();

    fixture.assert_pass(&tx);
}
