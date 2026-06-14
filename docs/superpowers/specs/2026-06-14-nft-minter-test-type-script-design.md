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
- Verify `CreateMinter` actions targeting the minter `output_type`.
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
- exactly one related `CreateMinter` action must target this output minter type;
- initial `mint_counter` must be `0`;
- output `supply_cap` must equal the action `supply_cap`.

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
1 group input, 0 group outputs  => burn
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

Burn rules:

- no Cobuild action is required;
- no minter state check is required;
- no counter or supply value is changed;
- the owner's lock authorization is enough to make destruction voluntary.

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
table CreateMinter {
    supply_cap: Uint64,
}

table MintNft {
    metadata_seed: Byte32,
}

union NftMinterAction {
    CreateMinter,
    MintNft,
}
```

`nft-minter-type` accepts only `CreateMinter` during minter creation and only
`MintNft` during mint updates. Unknown variants, malformed action bytes,
mismatched payload lengths, or action variants that do not match the current
operation fail closed.

`CreateMinter` target:

```text
script_role = OutputType
script_hash = new nft-minter-type hash
```

The action targets the newly created minter state because creation has no input
state and the output type is the new collection identity.

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

Instead, the minter computes the canonical mint order from each `MintNft`
action:

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

The minter validates that every `MintNft` action has a corresponding expected
NFT output. Candidate outputs are scoped by the action origin:

- tx-level `MintNft` actions search transaction outputs for an output whose
  type args equal the expected `nft_id`;
- OTX `MintNft` actions search only the same OTX's `append_outputs` range.

OTX minted NFTs must not be placed in `base_outputs`: the OTX signer cannot
know the final minted NFT data before the minter consumes the counter and
assigns the action's serial. Binding an OTX mint action to the same OTX's
`append_outputs` proves that the action and the produced NFT belong to the same
user-extension part of the OTX, and it lets the minter avoid scanning unrelated
transaction outputs for OTX-origin actions.

For each expected mint at position `i`, the matched output must have:

```text
type = minted-nft-type(args = expected_nft_id)
data.minter_type_hash = current minter type hash
data.serial = expected_serial
data.rarity = expected_rarity
data.attributes_hash = expected_attributes_hash
```

Missing expected outputs, duplicate matches for the same expected `nft_id`
inside the action's candidate range, wrong args, wrong serial, wrong rarity,
wrong attributes hash, or an OTX expected output outside the action's
`append_outputs` range fail.

The minter does not try to discover every possible minted NFT output in the
transaction. This follows the Spore-style boundary: a real `minted-nft-type`
cell must run its own creation checks, while a fake NFT type is outside this
test protocol. The minter's job is to prove that each minter action materialized
as the expected NFT output; the minted NFT's job is to prove that its own
creation is backed by a matching minter transition.

The minted NFT type performs the lightweight reverse check during mint
creation. It loads its output data, verifies the derived `nft_id`, then checks
that the transaction contains a minter counter transition for
`data.minter_type_hash` whose minted serial range includes this NFT's serial.
It does not recalculate `attributes_hash` from action seeds. The minter type
does that exact output binding and will reject a transaction where any minted
NFT output expected from an action is missing or mismatched.

## Tx-Level And OTX Scope

The MVP accepts both tx-level and OTX-level `MintNft` actions.

Ordering across both origins is defined by `(witness_index, action.index)`.
This makes mixed tx-level and OTX minting deterministic. OTX layout and
signature semantics are still handled by Cobuild Core; the minter consumes the
related actions, uses each OTX action's `ActionOrigin::Otx.layout`, and
validates output cells only in the action's allowed output range.

The minter does not count unrelated actions. Actions targeting other script
hashes or roles are ignored by the minter plan and should not affect the
counter.

## Error Boundaries

Cobuild Core is expected to reject:

- malformed Cobuild witness data;
- invalid OTX layout;
- `CreateMinter` target role other than `OutputType`;
- `MintNft` target role other than `InputType`;
- `CreateMinter` target hash that does not exist in transaction output types;
- `MintNft` target hash that does not exist in transaction input types;
- action target pointing at the wrong minter type.

`nft-minter-type` rejects:

- invalid group shape;
- invalid type-id on creation;
- malformed minter data;
- missing, duplicate, malformed, or mismatched `CreateMinter` action on create;
- initial `mint_counter != 0`;
- create output `supply_cap` not equal to action `supply_cap`;
- burn;
- `supply_cap` mutation;
- counter overflow;
- `new_counter != old_counter + mint_action_count`;
- `new_counter > supply_cap`;
- unsupported or malformed action data;
- missing, duplicate, or mismatched expected minted NFT outputs;
- minted NFT output args or data that do not match derived expectations.

`minted-nft-type` rejects:

- invalid group shape;
- malformed NFT data on creation or transfer;
- args length other than 32 bytes;
- `nft_id` not equal to `blake2b256(minter_type_hash || serial_le)`;
- standalone forged mint creation without a matching minter transition;
- mint creation whose serial is outside the matching minter counter increment
  range;
- transfer that changes data.

## Test Matrix

Positive tests:

- create minter with a `CreateMinter` output-type action, counter `0`, and
  matching `supply_cap`;
- mint first NFT, producing serial `0` and rarity `3`;
- mint from fixture counter `6`, producing serial `6` rarity `0`;
- mint next from counter `7`, producing rarity `1`;
- mint serial `11`, producing rarity `2`;
- mint serial `77`, producing rarity `3`;
- multiple `MintNft` actions in one message consume serials in action order;
- mixed tx-level and OTX actions consume serials by `(witness_index,
  action.index)`;
- supply cap passes exactly at the cap;
- minted NFT transfer preserves data;
- minted NFT burn succeeds without a minter action.

Negative tests:

- minter creation without `CreateMinter` action fails;
- duplicate `CreateMinter` action fails;
- `CreateMinter` action with wrong target role fails through Core;
- `CreateMinter.supply_cap` mismatch fails;
- minter creation with non-zero counter fails;
- counter increment does not match mint action count;
- supply cap overflow fails;
- `supply_cap` mutation fails;
- missing minted NFT output fails;
- duplicate expected minted NFT output fails;
- wrong NFT args fails;
- wrong rarity for serial `0`, `7`, `11`, or `77` fails;
- wrong `attributes_hash` fails;
- standalone forged NFT creation without minter update fails;
- NFT creation with a serial outside the minter increment range fails;
- NFT transfer data mutation fails;
- minter burn fails;
- malformed `MintNft` action data fails;
- malformed `CreateMinter` action data fails;
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
