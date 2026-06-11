# Cobuild Lock Coverage Reachability Follow-up Plan

**Goal:** Close the remaining P1-A caveat from the security coverage follow-up: document whether `MissingLockGroupCoverage` is reachable in core planning, and add one direct E2E guard that unrelated lock groups do not pollute current lock coverage.

**Constraints:**
- Do not refactor the completed framework/fixtures architecture.
- Keep framework independent from fixtures.
- Keep expected outcomes in fixtures/scenarios.
- Use typed handles and existing `TxShape`, `SigningFacts`, and `ExpectedOutcome` patterns.

## Tasks

- [x] Add a focused core test proving `MissingLockGroupCoverage` is emitted when OTX-scoped signatures exist but the current lock group also has inputs outside the OTX aggregate range and no tx-level signature is present.
- [x] Add a focused core test proving other-lock inputs outside the OTX aggregate range do not trigger current-lock coverage failure.
- [x] Add one `cobuild_otx_lock` E2E pass case with current lock fully covered in OTX plus an unrelated outside lock input.
- [x] Verify targeted tests and workspace tests.
