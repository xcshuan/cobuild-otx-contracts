# Cobuild Core Community Redraft Design

## Status

This document defines a proposed redraft of the CKB Cobuild core protocol.
It is intended to replace the current scattered combination of overview posts,
appendixes, PoC-specific choices, and discussion drafts with a single design
baseline for future implementation work.

This document is the authoritative design baseline for subsequent development
in this workspace unless it is explicitly revised.

## Scope

This document defines:

- the boundary between Cobuild Core, standard extensions, and reference flows;
- the normative witness/data model for Cobuild Core;
- the normative hashing and signature rules for Cobuild Core;
- the normative minimum responsibilities of lock scripts and type scripts;
- the coexistence rules between Cobuild witnesses and legacy `WitnessArgs`;
- the error model and extension/versioning boundaries.

This document does not define:

- application-specific `Action.data` schemas;
- a universal asset action standard;
- a mandatory off-chain packet or agent workflow;
- application-specific sequencing, batching, or market structure.

## Goals

- Define a stable community-oriented Cobuild Core that can be implemented by
  multiple lock scripts, type scripts, wallets, and builders.
- Preserve forward compatibility with legacy witness encoding and non-Cobuild
  scripts.
- Support both dynamic OTX and fine-grained signing control in the core
  witness/signature model.
- Keep `Action` in the core data model without making action existence a global
  validity precondition.
- Move higher-level action patterns such as approved-action out of the core and
  into standard extensions.

## Non-Goals

- Standardizing all application semantics in the core protocol.
- Forcing all scripts to support both Cobuild and legacy flows.
- Standardizing one mandatory off-chain collaboration flow.
- Preserving every incidental design decision found in the current PoCs.

## Layering

Cobuild is split into three layers.

### 1. Cobuild Core

Cobuild Core is the normative witness-and-validation protocol.

It standardizes:

- `WitnessLayout` encoding and union-id separation from legacy witnesses;
- the core witness variants and their semantics;
- OTX scope partitioning;
- dynamic OTX semantics;
- fine-grained signing coverage;
- standard signature-hash construction;
- transaction-global Cobuild activation and local validation selection rules;
- minimum lock/type validation responsibilities;
- compatibility and extension/versioning rules.

Cobuild Core is not an application action standard.

### 2. Standard Extensions

Standard extensions define optional higher-level semantics on top of Cobuild
Core. They may standardize:

- common `Action.data` schemas;
- cross-protocol interaction patterns;
- approved-action and similar patterns;
- application-domain-specific action families;
- stronger script-level constraints than Core requires.

Extensions must not redefine Core hashing, witness semantics, or minimum
validation responsibilities.

### 3. Reference Flows

Reference flows are recommended off-chain engineering patterns only.

They may standardize:

- `BuildingPacket`;
- `OtxBatch`;
- wallet presentation flows;
- signer/builder/agent interaction patterns;
- recommended packet versioning.

Reference flows are not part of chain-level validity.

## Terminology

- `Core`: the normative protocol defined in this document.
- `Extension`: an optional standard built on top of Core.
- `Reference flow`: an off-chain recommendation, not a validity rule.
- `Tx-level flow`: a non-OTX Cobuild signing flow using transaction-level
  witnesses.
- `OTX flow`: a Cobuild flow using `OtxStart` and one or more `Otx` witnesses.
- `Base scope`: the part of an OTX signed by the original OTX creator.
- `Append scope`: the part of an OTX appended later and signed separately by
  append-scope input owners.
- `TxWithMessage`: a document term meaning a tx-level flow in which the
  transaction contains exactly one valid `SighashAll` witness carrying the
  unique transaction `Message`.
- `TxWithoutMessage`: a document term meaning a tx-level flow in which no valid
  `SighashAll` witness exists and therefore no transaction-level `Message`
  exists.

`TxWithMessage` and `TxWithoutMessage` are descriptive terms for hash-rule
selection. They are not standalone on-chain objects and do not occupy explicit
fields in the transaction.

## Core Data Model

### WitnessLayout

`WitnessLayout` remains the entry point for Cobuild witnesses.

Its union ids MUST continue to live in the high custom-id range so that
`WitnessLayout` and legacy `WitnessArgs` are encoding-distinguishable without
ambiguity.

Core v1 uses the following witness variants:

- `SighashAll`
- `SighashAllOnly`
- `OtxStart`
- `Otx`

### Action

Core retains `Action` as a first-class data object, but action presence is not
itself a universal validity requirement.

```text
table Action {
  script_info_hash: Byte32,
  script_role: byte,   // 0=input_lock, 1=input_type, 2=output_type
  script_hash: Byte32,
  data: Bytes,
}
```

`script_role` is part of the core object so that an `Action` can unambiguously
identify which script position it targets.

Core v1 assigns:

- `0`: `input_lock`
- `1`: `input_type`
- `2`: `output_type`

All other values are invalid in Core v1.

### Message

```text
table Message {
  actions: ActionVec,
}
```

`actions` MAY be empty.

An empty `actions` vector means that the witness carries no standardized action
semantics for current scripts to consume. This is valid in Core.

### SighashAll

```text
table SighashAll {
  seal: Bytes,
  message: Message,
}
```

There MUST be at most one valid `SighashAll` witness in a transaction when a
script needs a transaction-level `Message`.

### SighashAllOnly

```text
table SighashAllOnly {
  seal: Bytes,
}
```

`SighashAllOnly` is a witness carrier containing only a seal.

It does not carry its own `Message`. In `TxWithMessage`, a signer using
`SighashAllOnly` still signs the same transaction-level signing hash that
covers the unique `SighashAll.message`.

### SealPair

`SealPair` is used inside OTX witnesses and explicitly labels which OTX scope a
seal belongs to.

```text
table SealPair {
  script_hash: Byte32,
  scope: byte,   // 0=base, 1=append
  seal: Bytes,
}
vector SealPairVec <SealPair>;
```

Core v1 assigns:

- `0`: `base`
- `1`: `append`

All other values are invalid in Core v1.

The same lock script may appear in both base and append input scopes. In that
case the lock MUST use two distinct `SealPair`s, one per scope.

### OtxStart

```text
table OtxStart {
  start_input_cell: Uint32,
  start_output_cell: Uint32,
  start_cell_deps: Uint32,
  start_header_deps: Uint32,
}
```

`OtxStart` marks the first indices of the transaction entities belonging to the
first OTX in the transaction. The witness index of `OtxStart` itself marks the
start of the OTX witness sequence.

`OtxStart` is runtime partition metadata for the final aggregated transaction.
It is not part of the creator-signed `OtxBase` or `OtxAppend` hash domain.

### Otx

Core v1 defines one unified `Otx` object that includes both dynamic OTX and
fine-grained signing control.

```text
table Otx {
  message: Message,

  append_permissions: byte,   // bit 0=input, 1=output, 2=cell_dep, 3=header_dep

  base_input_cells: Uint32,
  base_input_masks: Bytes,

  base_output_cells: Uint32,
  base_output_masks: Bytes,

  base_cell_deps: Uint32,
  base_cell_dep_masks: Bytes,

  base_header_deps: Uint32,
  base_header_dep_masks: Bytes,

  append_input_cells: Uint32,
  append_output_cells: Uint32,
  append_cell_deps: Uint32,
  append_header_deps: Uint32,

  seals: SealPairVec,
}
```

Design intent:

- `append_permissions` is the creator-signed permission map for whether append
  scope is allowed to contain additional inputs, outputs, cell deps, or header
  deps.
- `base_*` fields define the original creator-signed OTX scope.
- `append_*` fields define tail entities appended later and signed separately.
- Fine-grained coverage applies only to the base scope in Core v1.
- Append scope uses full-field coverage in Core v1.

`base_input_cells` MUST be greater than zero for a valid `Otx`.

Rationale:

- the base scope carries the creator-authorized `Message` and
  `append_permissions`;
- without at least one base input, no lock owner signs the base scope;
- Core v1 therefore disallows unsigned "base shells" with only append-scope
  authorization following them.

Core v1 assigns `append_permissions` bits as:

- bit 0: append inputs permitted
- bit 1: append outputs permitted
- bit 2: append cell deps permitted
- bit 3: append header deps permitted

Bits 4 through 7 are reserved and MUST be zero.

If an append count is non-zero while its corresponding permission bit is zero,
the `Otx` is invalid.

## OTX Scope Model

For each `Otx`, Core defines two contiguous sub-scopes:

- `base scope`
- `append scope`

The entities covered by one `Otx` are laid out in this order:

- base inputs
- append inputs
- base outputs
- append outputs
- base cell deps
- append cell deps
- base header deps
- append header deps

Each OTX consumes a contiguous slice of the transaction for each entity type.
Different OTXs are laid out consecutively according to the `OtxStart` anchor
and the counts accumulated while iterating the `Otx` sequence.

Core does not define any "global transaction mode". Scope is interpreted
locally by scripts that consume the relevant OTX witnesses.

## Fine-Grained Coverage Model

### General Rules

- Fine-grained signing control applies only to base scope in Core v1.
- `1` means the corresponding field or item is covered by the base-scope
  signing hash.
- `0` means it is not covered by the base-scope signing hash.
- Mask bytes are bit-packed item-by-item, with least-significant-bit-first
  ordering inside each byte.
- Any unused padding bits in the last mask byte MUST be zero.
- Mask byte length MUST exactly match the expected number of bits for the
  corresponding count.

For Core v1, the required mask byte lengths are:

- `base_input_masks.len == ceil(base_input_cells * 2 / 8)`
- `base_output_masks.len == ceil(base_output_cells * 4 / 8)`
- `base_cell_dep_masks.len == ceil(base_cell_deps / 8)`
- `base_header_dep_masks.len == ceil(base_header_deps / 8)`

### Base Input Masks

Each base input uses 2 bits:

- bit 0: `since`
- bit 1: `previous_output`

For base inputs, the mask applies only to the `CellInput` fields.

The corresponding resolved input `CellOutput` and its data are always covered by
the base-scope signing hash. This preserves the security intent of the earlier
OTX design while allowing finer control over `CellInput` attributes.

More precisely:

- masking out `previous_output` relaxes commitment to the exact consumed UTXO
  identity;
- continuing to hash resolved input `CellOutput` and data preserves commitment
  to the content of the consumed state;
- therefore, when `previous_output` is not covered, Core v1 permits
  substitution by another input cell only if that substituted input resolves to
  the same `CellOutput` and data from the perspective of the signing hash.

Core v1 intentionally does not allow base input masks to omit resolved input
`CellOutput` and data coverage. Omitting both outpoint identity and resolved
cell/data content would make base input replacement far too unconstrained for
the core protocol.

### Base Output Masks

Each base output uses 4 bits:

- bit 0: `capacity`
- bit 1: `lock`
- bit 2: `type`
- bit 3: `data`

An output slot MAY cover any subset of these fields, including none of them.
Core permits this. Individual scripts or extensions may impose stricter
policies.

### Base CellDep Masks

Each base cell dep uses 1 bit:

- bit 0: the entire `CellDep`

### Base HeaderDep Masks

Each base header dep uses 1 bit:

- bit 0: the entire `Byte32`

## Hashing and Signature Domains

### General Rules

Core v1 standardizes exact signing-preimage structure. Implementations MUST NOT
choose their own concatenation order or field framing while still claiming Core
compatibility.

Domain separation MUST be implemented using BLAKE2b personalization, not a
separate witness field.

Signing-preimage serialization MUST be injective over the ordered field
sequence. In particular, Core v1 MUST prevent ambiguity where different logical
field tuples could collapse to the same byte string after concatenation, such
as:

- `(23, 4)` becoming indistinguishable from
- `(2, 34)`

To guarantee this property, Core v1 uses the following framing rules:

- fixed-width scalar values such as `byte`, `u32`, and `u64` are serialized in
  their fixed canonical width;
- variable-length raw byte sequences MUST be prefixed with their
  little-endian `u32` byte length before the raw bytes;
- variable-length lists MUST include their item count before item payloads;
- canonical Molecule-encoded objects MAY be appended directly when their
  encoding is already self-delimiting or their field boundary is otherwise
  unambiguous in the standardized sequence;
- implementations MUST NOT drop required count or length framing even if local
  code could reconstruct boundaries by other means.

Core v1 uses four signature domains:

- `TxWithMessage`
- `TxWithoutMessage`
- `OtxBase`
- `OtxAppend`

These are hash-rule names only. They are not standalone witness variants.

Core v1 also fixes the exact 16-byte BLAKE2b personalization constants:

- `TxWithMessage`: `b"ckbcb_twm_core1\0"`
- `TxWithoutMessage`: `b"ckbcb_tnm_core1\0"`
- `OtxBase`: `b"ckbcb_otb_core1\0"`
- `OtxAppend`: `b"ckbcb_ota_core1\0"`

These byte strings are normative. Implementations MUST use them exactly and
MUST NOT substitute longer human-readable names at runtime.

### TxWithMessage

`TxWithMessage` is selected when a transaction contains exactly one valid
`SighashAll` witness.

All tx-level Cobuild lock signers in the transaction, including those using
`SighashAllOnly`, MUST sign the same `TxWithMessage` signing hash.

The preimage is:

1. `Message` in Molecule bytes from the unique `SighashAll`
2. tx hash
3. for each input index `i`:
   - resolved input `CellOutput` in Molecule bytes
   - input data length as little-endian `u32`
   - input data bytes
4. for each witness with index `>= inputs_len`:
   - witness length as little-endian `u32`
   - witness bytes

### TxWithoutMessage

`TxWithoutMessage` is selected when no valid `SighashAll` witness exists and a
tx-level Cobuild lock script uses `SighashAllOnly`.

The preimage is the same as `TxWithMessage` except that step 1 is omitted.

### OtxBase

`OtxBase` covers only the base scope of one OTX.

Its preimage is:

1. `Message` in Molecule bytes from the current `Otx`
2. `append_permissions` as one byte
3. `base_input_cells` as little-endian `u32`
4. `base_input_masks` length as little-endian `u32`
5. `base_input_masks` bytes
6. for each base input slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - if mask bit 0 is `1`, `since` as little-endian `u64`
   - if mask bit 1 is `1`, `previous_output` in canonical Molecule bytes
   - resolved input `CellOutput` in Molecule bytes
   - resolved input data length as little-endian `u32`
   - resolved input data bytes
7. `base_output_cells` as little-endian `u32`
8. `base_output_masks` length as little-endian `u32`
9. `base_output_masks` bytes
10. for each base output slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - for each covered field, append the field in this order:
     - `capacity` as little-endian `u64`
     - `lock` in canonical Molecule bytes
     - `type` as canonical Molecule option bytes
     - output data length as little-endian `u32`, then data bytes
11. `base_cell_deps` as little-endian `u32`
12. `base_cell_dep_masks` length as little-endian `u32`
13. `base_cell_dep_masks` bytes
14. for each covered base cell dep slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - `CellDep` in canonical Molecule bytes
15. `base_header_deps` as little-endian `u32`
16. `base_header_dep_masks` length as little-endian `u32`
17. `base_header_dep_masks` bytes
18. for each covered base header dep slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - header dep `Byte32`

Rationale:

- masks themselves are hashed so that different coverage policies cannot share
  the same semantic preimage;
- OTX-local slot indices are hashed so that omitted fields cannot create
  ambiguity via reordering within the OTX scope, without binding the OTX to an
  absolute transaction position;
- resolved input cells and input data remain fully covered.
- append permissions are hashed so that append-scope availability is creator
  authorized rather than implicitly assumed.

### OtxAppend

`OtxAppend` covers only append scope and binds itself to one specific base
scope.

Define:

`base_scope_commitment = Blake2b_OtxBase(OtxBase_preimage)`

where:

- `OtxBase_preimage` is the exact standardized `OtxBase` preimage defined in
  the previous subsection;
- `Blake2b_OtxBase` means one BLAKE2b hash invocation using the standardized
  `OtxBase` personalization.

The resulting 32-byte digest is the `base scope commitment`.

Core v1 does not apply an additional second hash on top of this digest.

The `OtxAppend` preimage is:

1. `Message` in Molecule bytes from the current `Otx`
2. `base scope commitment`
3. `append_input_cells` as little-endian `u32`
4. for each append input slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - full `CellInput` in canonical Molecule bytes
   - resolved input `CellOutput` in Molecule bytes
   - resolved input data length as little-endian `u32`
   - resolved input data bytes
5. `append_output_cells` as little-endian `u32`
6. for each append output slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - full output `CellOutput` in Molecule bytes
   - output data length as little-endian `u32`
   - output data bytes
7. `append_cell_deps` as little-endian `u32`
8. for each append cell dep slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - full `CellDep` in Molecule bytes
9. `append_header_deps` as little-endian `u32`
10. for each append header dep slot `i`:
   - OTX-local slot index `i` as little-endian `u32`
   - full header dep `Byte32`

Core v1 deliberately does not support fine-grained masks for append scope.

## Cobuild Activation and Local Validation

Cobuild activation is transaction-global for scripts that support Cobuild.

If any witness in the transaction is encoded as `WitnessLayout`, every
Cobuild-aware lock or type script in that transaction MUST evaluate its
validation under the Cobuild Core rule set. Such a script MUST NOT ignore the
Cobuild envelope and fall back to legacy-only validation merely because its own
script group witness or message is not locally relevant.

Activation depends on the presence of a Cobuild `WitnessLayout` envelope, not
on uniqueness or on whether all Cobuild witnesses already satisfy the remaining
Core validity rules.

This transaction-global activation rule does not require every script in the
transaction to support Cobuild. Legacy-only scripts may coexist in the same
transaction and continue to validate using their own legacy rules.

After Cobuild is activated, each Cobuild-aware script's concrete validation
obligations are still local, relevance-driven, and fail-closed.

### Lock Script Flow Selection

For a Cobuild-aware lock script in an activated Cobuild transaction:

- If the first witness in the current script group is a valid `SighashAll` or
  `SighashAllOnly`, the script MUST enter tx-level Cobuild flow for the part of
  the transaction outside any relevant OTX-covered inputs.
- If the transaction contains a valid OTX sequence and the current lock appears
  in the base or append input scope of one or more OTXs, the script MUST also
  enter the corresponding OTX flow for those OTX scopes.
- If neither condition applies, the script has no Cobuild signature obligation
  for that execution. It still MUST NOT treat the transaction as legacy-only
  solely to bypass the activated Cobuild rule set.

The same lock script execution MAY validate both:

- one or more OTX seals; and
- one tx-level remainder seal.

### Type Script Flow Selection

For a Cobuild-aware type script in an activated Cobuild transaction:

- If the script appears in the input/output range of a relevant OTX scope, it
  MAY read that OTX's `Message`.
- If the script appears outside all relevant OTX scopes and the transaction has
  a unique valid `SighashAll`, it MAY read the tx-level `Message`.
- If no related valid CoBuild `Message` or `Action` exists, Core does not
  require failure. The script MUST still perform its own native state
  transition validation.
- A type script MAY impose stricter policy and reject transactions missing a
  related `Message` or `Action`. This is script-specific policy, not a Core
  default.

### OTX Sequence Detection

An OTX flow exists only when all of the following are true:

- exactly one valid `OtxStart` exists;
- starting from the witness immediately after `OtxStart`, there is a contiguous
  sequence of valid `Otx` witnesses;
- the accumulated OTX scope partition over inputs, outputs, cell deps, and
  header deps is non-overflowing, non-overlapping, and consistent.

### Validation Procedure

This subsection gives the normative validation procedure for Cobuild-aware
scripts. Implementations MAY organize the code differently, but the resulting
validation decisions MUST be equivalent.

For every activated Cobuild transaction, a Cobuild-aware script first prepares
the shared Cobuild view:

1. Detect whether any witness is encoded as `WitnessLayout`. If none is found,
   this procedure is not activated and the script may use its legacy rules.
2. If at least one Cobuild `WitnessLayout` envelope is found, activate Cobuild
   validation for this Cobuild-aware script. Activation is based on existence,
   not uniqueness and not validity of all Cobuild witnesses.
3. Scan witnesses for the optional OTX sequence:
   - find valid `OtxStart` witnesses;
   - if more than one valid `OtxStart` exists, fail;
   - if exactly one valid `OtxStart` exists, treat its witness index and entity
     indices as the OTX anchor;
   - starting at the witness immediately after `OtxStart`, collect the
     contiguous run of valid `Otx` witnesses;
   - no valid `Otx` witness may appear outside this contiguous run;
   - compute every OTX's base and append scopes by accumulating counts from the
     anchor through the collected OTX sequence.
   For each entity type, the transaction remainder is the union of the range
   before the `OtxStart` anchor and the range after the final accumulated OTX
   scope for that entity type.
4. Build the tx-level message view:
   - if valid `SighashAll` witnesses exist where tx-level uniqueness is
     required, there MUST be exactly one;
   - that unique `SighashAll.message`, if present, is the tx-level `Message`;
   - `SighashAllOnly` never carries its own `Message`, but signs the same
     tx-level hash when a tx-level `Message` exists.
5. For every tx-level or OTX-level `Message` that the current script consumes
   or uses for signature verification, validate all action targets against the
   full transaction:
   - `input_lock` actions MUST point to an existing input lock script hash;
   - `input_type` actions MUST point to an existing input type script hash;
   - `output_type` actions MUST point to an existing output type script hash.

A Cobuild-aware lock script then validates lock ownership as follows:

1. For each collected OTX relevant to the current lock script:
   - determine whether the current lock script hash appears in the OTX base
     input scope, append input scope, or both;
   - if it appears in base scope, find exactly one `SealPair` for
     `(current_lock_hash, base)`, compute `OtxBase`, and verify the seal using
     the lock's own cryptographic rules;
   - if it appears in append scope, find exactly one `SealPair` for
     `(current_lock_hash, append)`, compute `OtxAppend`, and verify the seal
     using the lock's own cryptographic rules;
   - missing, duplicate, malformed, or invalid seals in a relevant OTX scope
     MUST fail.
2. Determine whether the current lock has tx-level remainder inputs outside all
   relevant OTX-covered input scopes. If it does not, no tx-level seal is
   required for this lock execution.
3. If tx-level remainder inputs exist:
   - the first witness in the current lock script group MUST be a valid
     `SighashAll` or `SighashAllOnly`;
   - all non-leading witnesses in the same lock script group MUST be absent or
     empty, unless the lock's own non-Cobuild ABI explicitly defines additional
     data that is still covered by its Cobuild signing rule;
   - select `TxWithMessage` when there is a unique valid `SighashAll`;
   - select `TxWithoutMessage` when no valid `SighashAll` exists and the
     group-leading witness is `SighashAllOnly`;
   - compute the selected tx-level signing hash and verify the group-leading
     seal using the lock's own cryptographic rules.
4. If the transaction is Cobuild-activated but neither OTX nor tx-level
   remainder validation is relevant to the current lock, the lock has no
   Cobuild signature obligation for this execution. It still MUST NOT treat the
   transaction as legacy-only merely to ignore a relevant Cobuild error.

A Cobuild-aware type script then validates message consistency as follows:

1. Execute its native state-transition validation first. Cobuild does not
   replace the script's application-specific validity rules.
2. For each collected OTX whose base or append input/output scope contains the
   current type script hash, or whose OTX `Message` contains an `input_type` or
   `output_type` action targeting the current type script hash, the type script
   MAY consume that OTX `Message`. The action target may refer to a type script
   outside that OTX's local cell ranges, as long as the target exists in the
   complete transaction.
3. For the transaction remainder outside all OTX scopes, if the current type
   script hash appears in the relevant input or output ranges, or if the unique
   tx-level `SighashAll.message` contains an `input_type` or `output_type`
   action targeting the current type script hash, the type script MAY consume
   that tx-level `Message`.
4. When consuming a `Message`, the type script:
   - MUST only consume actions targeting itself via `(script_role,
     script_hash)`;
   - SHOULD reject multiple matching actions unless its ABI explicitly defines
     multi-action semantics;
   - MUST validate the consumed action's `data` against the action target and
     any relevant OTX scope or transaction-remainder cells according to its own
     application rules;
   - MUST fail-closed for malformed or inconsistent consumed action data.
   A type script cannot fetch or verify the complete off-chain `ScriptInfo`
   for an action, and Core does not require it to validate
   `Action.script_info_hash` on chain. Wallets and reference-flow tooling are
   responsible for resolving `ScriptInfo`, checking the hash, parsing
   `Action.data`, and presenting the parsed meaning to signers.
5. If no related valid Cobuild `Message` or `Action` exists, Core does not
   require the type script to fail. The type script MAY define a stricter
   policy that requires one.

## Lock Script Responsibilities

In Core, a lock script is responsible for:

- verifying that the current owner authorizes the relevant consumption; and
- verifying that the relevant Cobuild-signed data has not been tampered with.

A lock script is not responsible for interpreting application-specific
`Action.data` semantics.

In tx-level Cobuild flow, a lock script MUST:

- obtain the group-leading `seal` from `SighashAll` or `SighashAllOnly`;
- select `TxWithMessage` or `TxWithoutMessage` according to transaction shape;
- compute the standard signing hash;
- verify the seal according to its own cryptographic logic.

In OTX flow, a lock script MUST:

- determine whether it appears in base input scope, append input scope, or
  both;
- find exactly one `SealPair` for each required `(script_hash, scope)` pair;
- compute `OtxBase` and/or `OtxAppend` as required;
- verify the corresponding seals according to its own cryptographic logic.

If a tx-level or OTX-level `Message` is present and non-empty, a lock script
MUST verify that each `Action.script_role + Action.script_hash` points to an
actually existing script position in the full transaction:

- `input_lock` must match at least one input lock script hash;
- `input_type` must match at least one input type script hash;
- `output_type` must match at least one output type script hash.

The lock script MUST NOT be required by Core to interpret `Action.data`.

## Type Script Responsibilities

In Core, a type script is always responsible for its own native state
transition rules first.

Cobuild adds an optional message-consistency layer on top.

If a type script chooses to consume `Action`s:

- it MUST only consume actions targeting itself via
  `(script_role, script_hash)`;
- it SHOULD reject ambiguous multiple matching actions unless the script's own
  ABI explicitly defines multi-action semantics;
- once it consumes an action, it MUST fail-closed when validating that action
  against current scope.

Core does not require every type script to require an action. A type script MAY
require one as a script-specific policy.

## Malformed Witness Handling

OTX layout errors fail closed for every Cobuild-aware script processing the
transaction.

- Malformed `OtxStart` or `Otx` witnesses fail OTX layout scanning.
- Multiple valid `OtxStart` witnesses fail OTX layout scanning.
- Any valid `Otx` witness outside the single contiguous OTX sequence fails OTX
  layout scanning.
- Tx-level `SighashAll` / `SighashAllOnly` malformed handling remains scoped to
  tx-level flow selection.

## Error Model

Core standardizes failure categories, not universal numeric error codes.

The following OTX layout conditions MUST fail:

- malformed selected `WitnessLayout`;
- multiple valid `OtxStart` witnesses;
- non-contiguous or malformed `Otx` witness sequence;
- `Otx` with `base_input_cells == 0`;
- overflow, overlap, or inconsistent OTX scope partitioning;
- invalid `script_role` or `scope` values;
- invalid `append_permissions` reserved bits;
- append counts that are non-zero when the corresponding append-permission bit
  is zero;
- invalid mask length;
- non-zero reserved padding bits in masks;
- missing required `SealPair`;
- duplicate `SealPair` for the same `(script_hash, scope)` within one `Otx`;
- invalid or failed signature verification;
- multiple `SighashAll` witnesses where uniqueness is required;
- any other failure in the exact Core hashing/selection rules the script chose
  to consume.

Core does not define "missing action" or "missing message" as a universal error.
That remains script-specific policy.

## Legacy Coexistence

Core allows legacy `WitnessArgs` and `WitnessLayout` to coexist in the same
transaction.

Core guarantees only that:

- the encodings are distinguishable;
- Cobuild activation and local validation selection are deterministic;
- scripts can safely remain legacy-only if they choose.

Core does not require:

- every lock script to support both legacy and Cobuild;
- every type script to support both legacy and Cobuild;
- every script in a transaction to validate under the same witness mode.

## Versioning

This document defines Core v1.

Core v1 follows these evolution rules:

- existing field meanings MUST NOT be repurposed;
- existing bit meanings MUST NOT be changed in place;
- reserved values/bits/bytes MUST be zero in v1;
- incompatible semantic changes MUST use new witness variants or new table
  structures;
- permissive parser widening MUST NOT be used as an implicit upgrade strategy.

If a future protocol revision needs incompatible OTX semantics, it SHOULD add a
new witness variant such as a new `OtxV2`-style object rather than mutate the
meaning of the v1 `Otx`.

## Standard Extension Boundary

Standard extensions MAY:

- define common `Action.data` schemas;
- define stronger action requirements for specific scripts;
- define approved-action and similar higher-level patterns;
- define domain-specific packet and batching conventions;
- define off-chain UX expectations.

Standard extensions MUST NOT:

- redefine Core witness variants;
- redefine Core signature domains or hash construction;
- redefine Core Cobuild activation or local validation selection rules;
- redefine the Core minimum lock/type responsibility split;
- require every Core-compatible script to understand the extension.

### Approved-Action Placement

Approved-action is explicitly outside Core and belongs to the standard
extension layer.

It may become a standardized action family for assets and DeFi interaction, but
it is not a precondition for Core validity and Core does not require generic
scripts to understand it.

## Reference Flow Boundary

`BuildingPacket`, `OtxBatch`, signer/builder/agent roles, and wallet display
procedures belong to the reference-flow layer.

They may evolve independently of Core provided they do not claim to change
chain-level validity.

Reference-flow revisions do not by themselves constitute a Core protocol
version upgrade.

## Migration Guidance

For future implementation work based on this document:

- treat this document as the normative target, not the current PoC schemas;
- treat existing PoCs as implementation references and migration inputs only;
- prefer new libraries and contracts that implement Core v1 directly, even if
  adapter layers are needed for older PoC layouts;
- keep extension-specific logic separate from the Core parsing/hashing layer.

## Summary of Major Design Decisions

- Use a three-layer model: Core, standard extensions, reference flows.
- Keep `Action` in Core but make action presence optional at the Core level.
- Add `script_role` to `Action` so the action target is unambiguous.
- Put dynamic OTX and fine-grained signing control into Core.
- Model OTX as `base scope + append scope`.
- Require creator-signed `append_permissions` so append scope is explicitly
  authorized.
- Apply fine-grained masks only to base scope in Core v1.
- Use explicit `SealPair.scope` instead of positional seal-search tricks.
- Standardize exact preimage construction and use BLAKE2b personalization for
  domain separation.
- Keep `TxWithMessage` and `TxWithoutMessage` as document terms for hash-rule
  selection, not on-chain fields.
- Use transaction-global Cobuild activation for Cobuild-aware scripts, while
  keeping concrete validation obligations local and relevance-driven.
- Let type scripts choose whether missing action/message is acceptable; Core
  does not force one answer.
- Place approved-action in the standard-extension layer.
- Keep `BuildingPacket` and related flows as reference-flow recommendations,
  not validity rules.
