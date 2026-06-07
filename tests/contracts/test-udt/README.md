# test-udt

Minimal UDT type script used only by repository tests.

Cell data is a 16-byte little-endian `u128` amount. Script args are the
32-byte owner lock hash. If any transaction input uses the owner lock, owner
mode is active and mint/burn is allowed. Otherwise, group input amount must be
greater than or equal to group output amount.

*This contract was bootstrapped with [ckb-script-templates].*

[ckb-script-templates]: https://github.com/cryptape/ckb-script-templates
