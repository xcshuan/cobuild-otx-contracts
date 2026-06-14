# NFT Minter Test Type Script Design

## Status

This spec defines the NFT Minter test type scripts for the Cobuild OTX test
suite. The contracts are test fixtures under `tests/contracts`; they are not
production NFT contracts and must not change `cobuild-types` public protocol
schemas.

This design follows the test type-script vision document and borrows the
Spore-style operation split: creation, transfer, and burn are detected from the
current type group shape, while Cobuild actions bind user intent to concrete
cell data.

## Goals

- Validate stateful minting from a minter cell with a monotonic counter.
- Verify `MintNft` actions targeting the minter `input_type`.
- Prove that multiple mint actions consume the counter in a deterministic
  global action order.
- Bind minted NFT type args and data to the minter type hash, serial, rarity,
  and action seed.
- Let the minted NFT type reject standalone forged creation without a matching
  minter transition.
- Cover tx-level and OTX-level mint actions without relying on the current
  `TypeValidationPlan.related_actions` vector order.

## Non-Goals

- Full Spore compatibility.
- Full NFT metadata, content MIME validation, extension hooks, or royalties.
- NFT burn semantics that decrement or reclaim supply.
- A production randomness source.
- A reusable off-chain NFT SDK.
- Adding the test action schema to `cobuild-types`.

## Contracts

Two new test contracts are added:

```text
tests/contracts/nft-minter-type
tests/contracts/minted-nft-type
```

`nft-minter-type` owns collection issuance state and performs the full
action-derived output binding. `minted-nft-type` owns each minted NFT cell's
permanent data and verifies that mint creation is backed by a matching minter
counter transition.

## Minter State

`nft-minter-type` script args:

```text
minter_type_id: [u8; 32]
```

The minter type hash is the collection identity. The minter cell data does not
store a separate collection id:

```text
mint_counter: u64
supply_cap: u64
```

The encoded minter data length is 16 bytes. Values are little-endian.

Minter operations are determined from the current type group:

```text
0 group inputs, 1 group output  => create minter
1 group input, 1 group output   => mint update
1 group input, 0 group outputs  => burn, rejected in MVP
other shape                     => invalid
```

Create minter rules:

- script args must be exactly 32 bytes;
- type-id validation must pass;
- output data must be well formed;
- initial `mint_counter` must be `0`;
- `supply_cap` may be any `u64`, including `0`.

Mint update rules:

- input and output data must be well formed;
- `supply_cap` must not change;
- at least one related `MintNft` action must exist when the counter changes;
- `new_counter = old_counter + mint_action_count`;
- overflow fails closed;
- `new_counter <= supply_cap`;
- expected minted NFT outputs must exist and match the action-derived data.

Burn is rejected. Fixtures that need a pre-existing counter, such as
`mint_counter = 6`, may construct an input cell directly; the create operation
does not support non-zero initial counters.

## Minted NFT State

`minted-nft-type` script args:

```text
nft_id: [u8; 32]
```

`minted-nft-type` cell data:

```text
minter_type_hash: [u8; 32]
serial: u64
rarity: u8
attributes_hash: [u8; 32]
```

The encoded NFT data length is 73 bytes. Values are little-endian where
numeric.

The NFT id is derived from the minter collection identity and serial:

```text
nft_id = blake2b256(minter_type_hash || serial_le)
```

NFT operations are determined from the current type group:

```text
0 group inputs, 1 group output  => mint creation
1 group input, 1 group output   => transfer
1 group input, 0 group outputs  => burn, rejected in MVP
other shape                     => invalid
```

Mint creation rules:

- script args must be exactly 32 bytes;
- output data must be well formed;
- `nft_id` must equal `blake2b256(minter_type_hash || serial_le)`;
- the transaction must contain exactly one `nft-minter-type` input and exactly
  one `nft-minter-type` output whose type hash equals `minter_type_hash`;
- that minter transition must increase the counter, keep `supply_cap`
  unchanged, and satisfy `old_counter <= serial < new_counter`.

The minted NFT type does not independently parse minter `MintNft` actions.
Those actions target the minter `input_type`, and full action-derived output
binding is the minter type's responsibility. This avoids duplicating Cobuild
message parsing in the NFT type while still preventing standalone forged NFT
creation without a corresponding minter transition.

Transfer rules:

- input and output data must be byte-for-byte identical;
- args are naturally the same because the type group is the same.

Burn is rejected in MVP.

## Rarity

Rarity is derived only from the consumed serial:

```text
serial % 77 == 0 => rarity 3
serial % 11 == 0 => rarity 2
serial % 7  == 0 => rarity 1
otherwise        => rarity 0
```

`serial = 0` is allowed and produces rarity `3`.

## Action ABI

The test action schema should use a local Molecule-style union, following the
Spore action pattern without importing Spore schemas:

```text
table MintNft {
    metadata_seed: Byte32,
}

union NftMinterAction {
    MintNft,
}
```

Only `MintNft` is accepted by these contracts. Unknown variants, malformed
action bytes, and mismatched payload lengths fail closed.

`MintNft` target:

```text
script_role = InputType
script_hash = nft-minter-type hash
```

The action targets the consumed minter state because minting is a state update
from the old counter to the new counter.

## Action Ordering

Contracts must not rely on `TypeValidationPlan.related_actions` vector order.
Current core collection adds OTX related actions before tx-level related
actions, so the vector order is not a global witness order.

Instead, the minter computes the canonical mint order from each action:

```text
primary key:   origin.witness_index
secondary key: action.index
```

`ActionView.index` is the action's index inside its `Message.actions` array.
Each OTX message is carried by a distinct `Otx` witness, and the tx-level
message is carried by a `SighashAll` witness, so this key defines a stable
global order across tx-level and OTX-level actions.

For the action at sorted position `i`:

```text
serial = old_counter + i
rarity = rarity(serial)
nft_id = blake2b256(minter_type_hash || serial_le)
attributes_hash = blake2b256(minter_type_hash || serial_le || rarity || metadata_seed)
```

The minter must use checked arithmetic for `old_counter + i` and
`old_counter + mint_action_count`.

## Minted Output Binding

The minter validates minted NFT outputs against the sorted expected list. It
scans transaction outputs in global output index order and selects outputs with
`minted-nft-type` whose args match each expected `nft_id`.

For each expected mint at position `i`, the matched output must have:

```text
type = minted-nft-type(args = expected_nft_id)
data.minter_type_hash = current minter type hash
data.serial = expected_serial
data.rarity = expected_rarity
data.attributes_hash = expected_attributes_hash
```

The number of matched minted NFT outputs for the current minter must equal the
number of `MintNft` actions. Missing outputs, extra outputs for the same minter,
wrong output ordering, wrong args, wrong serial, wrong rarity, or wrong
attributes hash fail.

The minted NFT type performs the lightweight reverse check during mint
creation. It loads its output data, verifies the derived `nft_id`, then checks
that the transaction contains a minter counter transition for
`data.minter_type_hash` whose minted serial range includes this NFT's serial.
It does not recalculate `attributes_hash` from action seeds. The minter type
does that exact output binding and will reject a transaction where any minted
NFT output for the minter is missing, extra, or mismatched.

## Tx-Level And OTX Scope

The MVP accepts both tx-level and OTX-level `MintNft` actions.

Ordering across both origins is defined by `(witness_index, action.index)`.
This makes mixed tx-level and OTX minting deterministic. OTX layout and
signature semantics are still handled by Cobuild Core; the minter only consumes
the related actions and validates output cells.

The minter does not count unrelated actions. Actions targeting other script
hashes or roles are ignored by the minter plan and should not affect the
counter.

## Error Boundaries

Cobuild Core is expected to reject:

- malformed Cobuild witness data;
- invalid OTX layout;
- `MintNft` target role other than `InputType`;
- target hash that does not exist in transaction input types;
- action target pointing at the wrong minter type.

`nft-minter-type` rejects:

- invalid group shape;
- invalid type-id on creation;
- malformed minter data;
- initial `mint_counter != 0`;
- burn;
- `supply_cap` mutation;
- counter overflow;
- `new_counter != old_counter + mint_action_count`;
- `new_counter > supply_cap`;
- unsupported or malformed action data;
- missing, extra, or incorrectly ordered minted NFT outputs;
- minted NFT output args or data that do not match derived expectations.

`minted-nft-type` rejects:

- invalid group shape;
- malformed NFT data;
- args length other than 32 bytes;
- `nft_id` not equal to `blake2b256(minter_type_hash || serial_le)`;
- standalone forged mint creation without a matching minter transition;
- mint creation whose serial is outside the matching minter counter increment
  range;
- transfer that changes data;
- burn.

## Test Matrix

Positive tests:

- create minter with counter `0`;
- mint first NFT, producing serial `0` and rarity `3`;
- mint from fixture counter `6`, producing serial `6` rarity `0`;
- mint next from counter `7`, producing rarity `1`;
- mint serial `11`, producing rarity `2`;
- mint serial `77`, producing rarity `3`;
- multiple `MintNft` actions in one message consume serials in action order;
- mixed tx-level and OTX actions consume serials by `(witness_index,
  action.index)`;
- supply cap passes exactly at the cap;
- minted NFT transfer preserves data.

Negative tests:

- minter creation with non-zero counter fails;
- counter increment does not match mint action count;
- supply cap overflow fails;
- `supply_cap` mutation fails;
- missing minted NFT output fails;
- extra minted NFT output for the same minter fails;
- wrong NFT output order fails;
- wrong NFT args fails;
- wrong rarity for serial `0`, `7`, `11`, or `77` fails;
- wrong `attributes_hash` fails;
- standalone forged NFT creation without minter update fails;
- NFT creation with a serial outside the minter increment range fails;
- NFT transfer data mutation fails;
- minter or NFT burn fails;
- malformed `MintNft` action data fails;
- wrong action target role fails through Core.

## Implementation Notes

- Keep action encoding local to tests, in the same spirit as the existing
  limit-order fixture action envelope, or introduce a small test-only Molecule
  schema if the surrounding fixture framework is ready for generated test
  actions.
- Keep shared parsing helpers small: fixed-width readers for minter data, NFT
  data, action seed, and derived hashes are enough for MVP.
- The minter and NFT type may share a test-only library module only if it stays
  under `tests/contracts` or test fixture code. Do not expose these types from
  production crates.
- Use the existing test framework style for deploying test contracts, building
  Cobuild messages, mutating transactions, and asserting errors.
