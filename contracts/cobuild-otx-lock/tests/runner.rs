#[test]
fn host_binary_exits_with_contract_exit_code() {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_cobuild-otx-lock"))
        .status()
        .expect("run host binary");

    assert_eq!(status.code(), Some(3));
}
