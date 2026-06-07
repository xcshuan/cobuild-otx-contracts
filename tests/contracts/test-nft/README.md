# test-nft

Minimal NFT type script used only by repository tests.

Script args start with a 32-byte `type_id`, validated through
`ckb_std::type_id::check_type_id(0, 32)`. The type ID is the NFT's unique
identifier.

Cell data is a compact test-only NFT record:

```text
name_len:   u8
name:       name_len bytes, 1..=32 bytes
attributes: 4 bytes
created_at: u64 little-endian timestamp
```

Mint is output-only, burn is input-only, and transfer requires the NFT data to
remain unchanged.

*This contract was bootstrapped with [ckb-script-templates].*

[ckb-script-templates]: https://github.com/cryptape/ckb-script-templates
