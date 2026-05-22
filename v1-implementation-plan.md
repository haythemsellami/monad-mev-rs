# monad-mev-rs V1 Implementation Plan

Status date: 2026-05-22

This file is the development tracker for V1. The product and architecture source of truth is `v1-spec.md`; this file turns that spec into ordered implementation work, test coverage requirements, and release gates.

V1 has 6 spec milestones, but this plan breaks them into 20 concrete work packages so progress can be tracked at implementation granularity.

## 1. How To Use This Plan

- Keep `v1-spec.md` stable as the product contract.
- Update this file as work lands.
- Each work package should be implemented with code, tests, examples or docs where listed.
- A work package is not done until its acceptance checks pass.
- Any intentional scope change should update both this file and `v1-spec.md`.

### PR And Commit Discipline

- Every work package must land through a GitHub pull request.
- If a work package is large, split it into multiple smaller PRs with clear sequencing.
- Do not push work directly to the default branch except for repository setup emergencies that are explicitly agreed.
- Each PR should state which work package it advances, for example `WP-06: Snapshot Event-Ring Source`.
- Each PR should include the relevant tests, docs, examples, or follow-up notes required by that work package.
- Commits inside a PR should group related changes together.
- Avoid unrelated code changes in a single commit.
- Avoid mixing mechanical formatting, generated output, feature code, and test changes in one commit when they can be reviewed separately.
- A PR is ready for review only after its listed acceptance checks pass locally or in CI, unless the PR description clearly marks what is still failing and why.

Suggested status markers:

- `[ ]`: not started
- `[~]`: in progress
- `[x]`: complete
- `[!]`: blocked

## 2. V1 Release Gates

V1 is releasable only when all of these are true:

- [ ] All 20 work packages are complete or explicitly moved to V2.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --all --all-features` passes.
- [ ] CLI smoke tests pass for `doctor`, `inspect`, `decode`, `replay`, and `strategy new`.
- [ ] Fixture/golden tests pass deterministically.
- [ ] Snapshot integration tests pass against at least one real snapshot.
- [ ] Live observe test has been run on Linux against a Monad node with `--exec-event-ring`, or the release notes clearly mark live testing as pending.
- [ ] Coverage thresholds are met, excluding generated bindings and official SDK internals.
- [ ] License decision is documented.
- [ ] SDK version and schema compatibility policy are documented.
- [ ] Examples compile.
- [ ] Docs cover macOS snapshot mode and Linux live mode.

## 3. Coverage Policy

"Full coverage" for V1 means full behavioral coverage of framework-owned logic, not 100% line coverage of generated FFI bindings or third-party SDK code.

Minimum coverage targets:

| Area | Target |
|---|---:|
| `monad-mev-core` | 95% line coverage |
| `monad-mev-events` framework-owned code | 90% line coverage |
| `monad-mev-cli` command logic | 85% line coverage |
| Gap/commit/schema state machines | 100% case coverage |
| Decoders for built-in event signatures | 100% success and malformed-input case coverage |
| CLI exit code behavior | 100% command-path coverage |

Coverage exclusions:

- Bindgen-generated types.
- Official Monad SDK implementation.
- Example binaries, except that they must compile.
- Platform-specific live tests that require a running Monad node.

Recommended coverage command once the workspace exists:

```bash
cargo llvm-cov --workspace --all-features --ignore-filename-regex 'target|bindings|examples' --fail-under-lines 90
```

If `cargo llvm-cov` is not installed, CI should install it or coverage should be run in a documented release checklist.

## 4. Work Package Overview

| Work Package | Spec Milestone | Summary |
|---|---|---|
| WP-01 | M1 | Project bootstrap and repository hygiene |
| WP-02 | M1 | SDK pinning, license decision, dependency isolation |
| WP-03 | M1 | Workspace and crate skeleton |
| WP-04 | M1 | Core framework types |
| WP-05 | M2 | Event source abstraction and schema checks |
| WP-06 | M2 | Snapshot event-ring source |
| WP-07 | M2 | Raw event conversion and descriptor metadata |
| WP-08 | M2/M3 | Stream health, gap, payload expiration handling |
| WP-09 | M3 | Commit-state tracker |
| WP-10 | M3 | Flow tags and transaction grouping |
| WP-11 | M3 | Chain event normalization |
| WP-12 | M3 | Built-in DeFi decoders |
| WP-13 | M3 | Generic ABI decoder |
| WP-14 | M4 | Replay engine, clocks, filters, reports |
| WP-15 | M4 | Strategy API and executors |
| WP-16 | M4/M6 | Fixtures, golden tests, and normalized test data |
| WP-17 | M4 | Strategy scaffolding and examples |
| WP-18 | M1/M2/M3/M4 | CLI commands |
| WP-19 | M5 | Live observe mode |
| WP-20 | M6 | Documentation, CI, release hardening |

## 5. Detailed Work Packages

### WP-01: Project Bootstrap And Repository Hygiene

Status: [x]

Goal: create a maintainable Rust repository baseline.

Implementation tasks:

- [x] Initialize git repository if one does not exist.
- [x] Add root `Cargo.toml` workspace.
- [x] Add minimal Rust target so formatter and metadata checks are meaningful.
- [x] Add `.gitignore` for Rust, editor, coverage, local snapshots, and run outputs.
- [x] Add `rustfmt.toml` if the defaults are insufficient. Not needed for WP-01.
- [x] Add `README.md` with V1 positioning and "not production trading yet" warning.
- [x] Leave `LICENSE` to WP-02 because it depends on SDK/license review.
- [x] Add `docs/` directory.
- [x] Add `fixtures/README.md` explaining fixture policy.
- [x] Add `data/` or `snapshots/` to `.gitignore` for local snapshot files.

Testing tasks:

- [ ] Verify `cargo metadata` works.
- [ ] Verify `cargo fmt --all -- --check` works on empty/skeleton workspace.

Acceptance:

```bash
cargo metadata --no-deps
cargo fmt --all -- --check
```

### WP-02: SDK Pinning, License Decision, Dependency Isolation

Status: [x]

Goal: make the Monad SDK dependency explicit, isolated, and legally understood.

Implementation tasks:

- [x] Identify the exact SDK repository and tag/revision for V1.
- [x] Confirm whether `monad-event-ring` and `monad-exec-events` come from `category-labs/monad` or another repository for the chosen version.
- [x] Record SDK dependency declarations only inside `crates/monad-mev-events`; active git dependencies are deferred because Cargo fetches upstream submodules during ordinary lockfile resolution.
- [x] Create `docs/sdk-versioning.md`.
- [x] Document the SDK tag/revision.
- [x] Document the schema-hash compatibility policy.
- [x] Document platform dependencies for macOS snapshot mode.
- [x] Document platform dependencies for Linux live mode.
- [x] Decide project license based on SDK linking implications.
- [x] Add root `LICENSE` and, if needed, `NOTICE`.
- [x] Reserve the `monad-mev-events/sdk` feature for SDK-backed APIs so default checks stay usable without fetching/building the upstream SDK.

Testing tasks:

- [x] Add a compile-only reserved SDK feature path through `monad-mev-events`.
- [x] Add SDK metadata tests that pass without the native SDK toolchain.
- [x] Document the schema hash API that should be exposed once active SDK deps are enabled.
- [x] Document that the first SDK-backed build can be slow because Cargo fetches upstream submodules.

Acceptance:

```bash
cargo test -p monad-mev-events sdk
cargo test -p monad-mev-events --features sdk sdk
```

The active upstream SDK dependency build is intentionally deferred. During WP-02, enabling active optional git dependencies still caused ordinary Cargo tests to fetch upstream submodules. Revisit this before WP-05.

Manual acceptance:

- [x] `docs/sdk-versioning.md` names exact SDK source and version.
- [x] License decision is visible in root files.

### WP-03: Workspace And Crate Skeleton

Status: [x]

Goal: create the three V1 crates with clean ownership boundaries.

Implementation tasks:

- [x] Create `crates/monad-mev-core`.
- [x] Create `crates/monad-mev-events`.
- [x] Create `crates/monad-mev-cli`.
- [x] Make `monad-mev-cli` produce the `monad-mev` binary.
- [x] Add crate-level docs describing each crate's responsibilities.
- [ ] Add feature flags:
  - [x] `sdk` or default SDK integration feature if useful.
  - [x] `live` for live event-ring support if live dependencies are platform-sensitive.
  - [x] `test-fixtures` for fixture helpers if needed.
- [x] Ensure only `monad-mev-events` depends directly on official Monad SDK crates.
- [x] Add a root prelude only after APIs stabilize; do not over-export early. No root prelude added in WP-03.

Testing tasks:

- [x] Add one trivial unit test per crate.
- [x] Add compile test that `monad-mev-cli --help` can be built.

Acceptance:

```bash
cargo test --workspace
cargo run -p monad-mev-cli -- --help
```

### WP-04: Core Framework Types

Status: [x]

Goal: implement stable framework-owned types before SDK details leak upward.

Implementation tasks:

- [x] Add project error type.
- [x] Add `Result<T>` alias.
- [x] Add `StreamItem<T>`.
- [x] Add `EventEnvelope<T>`.
- [x] Add `EventMeta`.
- [x] Add `BlockRef`.
- [x] Add `TxnRef`.
- [x] Add `FlowTags`.
- [x] Add `CommitState`.
- [x] Add `EventSourceKind`.
- [x] Add `EventKind`.
- [x] Add `GapEvent`.
- [x] Add `PayloadExpired`.
- [x] Add `SchemaMismatch`.
- [x] Add `GapPolicy`.
- [x] Add `PayloadMode`.
- [x] Add `ReplayClock`.
- [x] Add `ReplayReport`.
- [x] Add serialization support where needed for CLI JSON output.
- [x] Add human-readable display formatting for errors and health events.

Testing tasks:

- [x] Unit test `CommitState` ordering if strategy min-state checks use ordering.
- [x] Unit test serialization shape for public report and envelope metadata.
- [x] Unit test display messages for `GapEvent`, `PayloadExpired`, and `SchemaMismatch`.
- [x] Unit test default policies by run mode.

Acceptance:

```bash
cargo test -p monad-mev-core
```

### WP-05: Event Source Abstraction And Schema Checks

Status: [x]

Goal: define how snapshot and live sources expose event streams safely.

Implementation tasks:

- [x] Define `ExecEventSource` trait or equivalent.
- [x] Define `SourceInfo` containing source kind, path/name, content type, schema hash, and SDK version.
- [x] Implement content-type validation.
- [x] Implement schema-hash validation.
- [x] Map SDK errors into framework errors.
- [x] Add `SchemaPolicy`:
  - [x] `RequireMatch`
  - [x] `Warn`
  - [x] `SkipCheck` only for debug/testing
- [x] Ensure schema mismatch can be returned as an error and also represented as `StreamItem::SchemaMismatch` when appropriate.

Testing tasks:

- [x] Unit test matching schema hash.
- [x] Unit test mismatched schema hash.
- [x] Unit test missing/unreadable source path.
- [x] Unit test wrong content type if the SDK allows constructing a fixture.
- [x] Unit test error messages include expected and observed schema hashes.

Acceptance:

```bash
cargo test -p monad-mev-events schema
```

### WP-06: Snapshot Event-Ring Source

Status: [x]

Goal: read compressed snapshot event-ring files and expose framework stream items.

Implementation note: V1 uses an SDK-compatible direct parser for compressed event-ring snapshots so normal workspace builds do not fetch the large upstream Monad SDK repository. Raw execution-event decoding remains in WP-07.

Implementation tasks:

- [x] Implement `SnapshotSource::open(path)`.
- [x] Support explicit paths like `./snapshot.zst` and `/tmp/snapshot.zst`.
- [x] Preserve SDK path resolution behavior where relevant, but make CLI behavior obvious.
- [x] Read source metadata.
- [x] Iterate descriptors from the correct initial point.
- [x] Support owned payload mode.
- [x] Support end-of-source detection.
- [x] Add source summary counters.
- [x] Handle corrupt or truncated zstd files with clear errors.

Testing tasks:

- [x] Unit test path validation.
- [x] Unit test missing file error.
- [x] Unit test unsupported extension warning or neutral behavior.
- [x] Snapshot integration test with real `.zst` file behind `MONAD_MEV_SNAPSHOT`.
- [x] Fixture-based test using normalized events if a tiny real snapshot is not available.

Acceptance:

```bash
cargo test -p monad-mev-events snapshot
MONAD_MEV_SNAPSHOT=./data/snapshot.zst cargo test -p monad-mev-events --test snapshot_replay -- --ignored
```

### WP-07: Raw Event Conversion And Descriptor Metadata

Status: [ ]

Goal: convert SDK events into framework-owned raw events with descriptor context.

Implementation tasks:

- [ ] Define `RawExecEvent`.
- [ ] Implement conversion from SDK copied event type to `RawExecEvent`.
- [ ] Preserve unknown event types as `RawExecEvent::Unknown`.
- [ ] Extract descriptor sequence number.
- [ ] Extract record timestamp.
- [ ] Extract payload size.
- [ ] Extract flow tags.
- [ ] Extract event kind.
- [ ] Attach schema hash from source info.
- [ ] Avoid exposing bindgen-generated payload names in high-level APIs.
- [ ] Add debug formatting suitable for `inspect`.

Testing tasks:

- [ ] Unit test conversion for each event kind that can be fixture-built.
- [ ] Unit test unknown event type is preserved.
- [ ] Unit test descriptor metadata is carried into `EventMeta`.
- [ ] Unit test timestamp conversion boundaries.

Acceptance:

```bash
cargo test -p monad-mev-events raw_event
```

### WP-08: Stream Health, Gaps, Payload Expiration

Status: [ ]

Goal: make stream correctness failures impossible to ignore accidentally.

Implementation tasks:

- [ ] Implement sequence tracking.
- [ ] Emit `GapEvent` when expected sequence differs from observed sequence.
- [ ] Emit or return `PayloadExpired` when SDK reports expired payload.
- [ ] Apply `GapPolicy` consistently.
- [ ] Default snapshot replay to `FailFast`.
- [ ] Default CLI inspect to `LogAndContinue` with warning summary.
- [ ] Default live observe to `RiskOffThenFail`.
- [ ] Count gaps and payload expirations in reports.
- [ ] Ensure strategy `on_gap` can override or confirm policy decisions.

Testing tasks:

- [ ] Table-driven tests for no gap, single gap, multi-event gap, restart-like sequence behavior if detectable.
- [ ] Test snapshot `FailFast` exits non-zero on gap.
- [ ] Test inspect can continue and report gap count.
- [ ] Test payload expiration is counted.
- [ ] Test strategy `on_gap` is called before fail/continue decision when applicable.

Acceptance:

```bash
cargo test -p monad-mev-events gap
cargo test -p monad-mev-core gap
```

### WP-09: Commit-State Tracker

Status: [ ]

Goal: track Monad block lifecycle explicitly.

Implementation tasks:

- [ ] Implement `CommitStateTracker`.
- [ ] On `BLOCK_START`, create proposed block state keyed by block ID and block-start seqno.
- [ ] On `BLOCK_QC`, move block to voted.
- [ ] On `BLOCK_FINALIZED`, move block to finalized.
- [ ] On `BLOCK_VERIFIED`, mark block number verified.
- [ ] Maintain mapping from block-start seqno flow tag to block ref.
- [ ] Attach current commit state to envelopes.
- [ ] Represent unknown or unresolved block context explicitly.
- [ ] Detect invalid regressions or duplicate transitions.
- [ ] Decide how long old block state remains in memory during replay/live observe.

Testing tasks:

- [ ] Table-driven transition tests for proposed -> voted -> finalized -> verified.
- [ ] Test finalized without prior local start is handled as unknown/block-number-only when needed.
- [ ] Test duplicate state event is idempotent or reported according to chosen policy.
- [ ] Test invalid regression does not corrupt state.
- [ ] Test events scoped by flow tag receive the correct block state.

Acceptance:

```bash
cargo test -p monad-mev-events commit_state
```

### WP-10: Flow Tags And Transaction Grouping

Status: [ ]

Goal: support interleaved transaction events without assuming stream order equals transaction index order.

Implementation tasks:

- [ ] Implement `FlowTags` extraction from descriptors.
- [ ] Implement transaction-flow key type: block-start seqno plus transaction id/index.
- [ ] Implement `TxnFlowTracker`.
- [ ] Track transaction header start/end.
- [ ] Track transaction hash when available.
- [ ] Attach transaction context to log/call/access events.
- [ ] Provide helper to aggregate transaction events into a completed transaction bundle.
- [ ] Make incomplete transaction flows visible in summaries.
- [ ] Bound memory for live observe mode.

Testing tasks:

- [ ] Build fixture with two interleaved transactions.
- [ ] Verify per-transaction event order is preserved.
- [ ] Verify transaction bundles complete correctly.
- [ ] Verify orphan event with missing transaction header remains usable but marked incomplete.
- [ ] Verify memory cleanup after transaction end.

Acceptance:

```bash
cargo test -p monad-mev-events flow
cargo test -p monad-mev-events transaction_grouping
```

### WP-11: Chain Event Normalization

Status: [ ]

Goal: provide an EVM-oriented layer that is easier than raw execution events but not DeFi-specific.

Implementation tasks:

- [ ] Define `ChainEvent`.
- [ ] Define `BlockEvent`.
- [ ] Define `TransactionEvent`.
- [ ] Define `LogEvent`.
- [ ] Define `CallFrameEvent`.
- [ ] Define `AccountAccessEvent`.
- [ ] Define `StorageAccessEvent`.
- [ ] Define `TxnOutputEvent`.
- [ ] Define `CommitStateEvent`.
- [ ] Convert raw `TxnLog` into `LogEvent`.
- [ ] Preserve raw context in normalized events.
- [ ] Add normalizer pipeline from `StreamItem<RawExecEvent>` to `StreamItem<ChainEvent>`.

Testing tasks:

- [ ] Unit test raw log normalization.
- [ ] Unit test block boundary normalization.
- [ ] Unit test commit event normalization.
- [ ] Unit test malformed or unknown raw events do not panic.
- [ ] Snapshot integration test counts logs from real snapshot.

Acceptance:

```bash
cargo test -p monad-mev-events normalize
```

### WP-12: Built-In DeFi Decoders

Status: [ ]

Goal: decode the most useful common log events for V1 examples and strategies.

Implementation tasks:

- [ ] Define `DeFiEvent`.
- [ ] Define `Erc20Transfer`.
- [ ] Define `Erc20Approval`.
- [ ] Define `DexSwap`.
- [ ] Define `DexSync`.
- [ ] Define `UnknownLog`.
- [ ] Implement ERC20 `Transfer(address,address,uint256)`.
- [ ] Implement ERC20 `Approval(address,address,uint256)`.
- [ ] Implement Uniswap V2-style `Swap(address,uint256,uint256,uint256,uint256,address)`.
- [ ] Implement Uniswap V2-style `Sync(uint112,uint112)`.
- [ ] Implement basic Uniswap V3-style `Swap(address,address,int256,int256,uint160,uint128,int24)`.
- [ ] Include decoder name and event signature in decoded output.
- [ ] Preserve original `LogEvent`.
- [ ] Never panic on malformed topics or data.

Testing tasks:

- [ ] Golden unit tests for each valid event signature.
- [ ] Malformed tests for missing topics.
- [ ] Malformed tests for wrong data length.
- [ ] Malformed tests for invalid indexed/non-indexed layout.
- [ ] Unknown signature test returns `UnknownLog`.
- [ ] Round-trip JSON serialization tests for decoded event structs.

Acceptance:

```bash
cargo test -p monad-mev-events defi_decoder
```

### WP-13: Generic ABI Decoder

Status: [ ]

Goal: let users decode protocol logs without waiting for built-in framework support.

Implementation tasks:

- [ ] Load ABI JSON from file.
- [ ] Parse event definitions.
- [ ] Build topic0 signature map.
- [ ] Support address filtering.
- [ ] Decode indexed fields.
- [ ] Decode non-indexed data fields.
- [ ] Represent decoded values in JSON-friendly form.
- [ ] Return `DecodedAbiEvent`.
- [ ] Include ABI name/source path in output.
- [ ] Handle overloaded events correctly by topic signature.
- [ ] Handle anonymous events explicitly: either support or reject with clear error.

Testing tasks:

- [ ] Decode a simple ERC20 ABI event through generic path.
- [ ] Decode custom event with mixed indexed/non-indexed fields.
- [ ] Test unknown topic returns no match.
- [ ] Test invalid ABI JSON returns clear error.
- [ ] Test overloaded event signatures.
- [ ] Test address filter includes/excludes expected logs.

Acceptance:

```bash
cargo test -p monad-mev-events abi_decoder
```

### WP-14: Replay Engine, Clocks, Filters, Reports

Status: [ ]

Goal: make deterministic snapshot replay the central V1 workflow.

Implementation tasks:

- [ ] Implement replay runner.
- [ ] Support `ReplayClock::AsFastAsPossible`.
- [ ] Support `ReplayClock::FixedDelay`.
- [ ] Support `ReplayClock::SpeedMultiplier`.
- [ ] Implement filters:
  - [ ] `from_seqno`
  - [ ] `to_seqno`
  - [ ] `from_block`
  - [ ] `to_block`
  - [ ] `event_kind`
  - [ ] `address`
  - [ ] `topic0`
  - [ ] `txn_idx`
- [ ] Implement `ReplayReport` aggregation.
- [ ] Support human summary output.
- [ ] Support JSON report output.
- [ ] Support JSONL event output.
- [ ] Ensure replay is deterministic by default.
- [ ] Ensure wall-clock options do not affect report content except timing fields.

Testing tasks:

- [ ] Unit test every filter.
- [ ] Unit test combined filters.
- [ ] Unit test deterministic report across two runs.
- [ ] Unit test speed parser: `10x`, `1.5x`, invalid values.
- [ ] Unit test report counters for events, gaps, payload expiration, logs, decoded events, and actions.
- [ ] Integration test replay over normalized fixture.

Acceptance:

```bash
cargo test -p monad-mev-core replay
cargo test -p monad-mev-events replay
```

### WP-15: Strategy API And Executors

Status: [ ]

Goal: let users write simple replay/live-observe strategies without production trading machinery.

Implementation tasks:

- [ ] Implement `Strategy` trait.
- [ ] Implement `StrategyContext`.
- [ ] Implement `GapDecision`.
- [ ] Implement `Action`.
- [ ] Implement `RecordAction`.
- [ ] Implement `AlertAction`.
- [ ] Implement `SubmitTxDryRun`.
- [ ] Implement `Executor` trait.
- [ ] Implement `RecordingExecutor`.
- [ ] Implement `DryRunExecutor`.
- [ ] Implement `ExecutionReceipt`.
- [ ] Wire replay runner to strategy and executor.
- [ ] Ensure V1 executors cannot submit transactions.

Testing tasks:

- [ ] Unit test strategy receives events in deterministic order.
- [ ] Unit test strategy actions are sent to executor.
- [ ] Unit test recording executor writes JSONL.
- [ ] Unit test dry-run executor accepts well-formed actions.
- [ ] Unit test dry-run executor rejects malformed actions.
- [ ] Unit test gap callbacks.
- [ ] Compile test for example strategy.

Acceptance:

```bash
cargo test -p monad-mev-core strategy
cargo test -p monad-mev-core executor
```

### WP-16: Fixtures, Golden Tests, And Normalized Test Data

Status: [ ]

Goal: keep CI deterministic without requiring large binary snapshots.

Implementation tasks:

- [ ] Define normalized fixture JSON schema.
- [ ] Add small fixture for raw events.
- [ ] Add small fixture for chain events.
- [ ] Add small fixture for DeFi decoded events.
- [ ] Add fixture with interleaved transactions.
- [ ] Add fixture with malformed logs.
- [ ] Add fixture with gap event.
- [ ] Add fixture with commit-state transitions.
- [ ] Add golden report for replay fixture.
- [ ] Add helper to load fixtures in tests.
- [ ] Document fixture schema in `fixtures/README.md`.

Testing tasks:

- [ ] Golden report test.
- [ ] Golden decoded JSONL test.
- [ ] Golden action JSONL test.
- [ ] Determinism test runs same fixture twice and compares stable output.

Acceptance:

```bash
cargo test --workspace fixture
cargo test --workspace golden
```

### WP-17: Strategy Scaffolding And Examples

Status: [ ]

Goal: make the framework learnable by reading and running small examples.

Implementation tasks:

- [ ] Add `examples/raw-event-printer`.
- [ ] Add `examples/erc20-transfer-monitor`.
- [ ] Add `examples/dex-swap-monitor`.
- [ ] Add `examples/replay-strategy-test`.
- [ ] Implement `monad-mev strategy new`.
- [ ] Generated strategy should compile immediately.
- [ ] Generated strategy should include a fixture-backed test.
- [ ] Generated strategy should default to recording executor.
- [ ] Examples should avoid production private keys or RPC submission.

Testing tasks:

- [ ] Compile all examples.
- [ ] Test `strategy new` creates expected files in a temp dir.
- [ ] Test generated strategy's tests pass.
- [ ] Smoke test example over fixture replay.

Acceptance:

```bash
cargo test --workspace --examples
monad-mev strategy new /tmp/dex-swap-monitor
```

### WP-18: CLI Commands

Status: [ ]

Goal: provide the main V1 user interface.

Implementation tasks:

- [ ] Choose CLI parser crate.
- [ ] Implement global flags:
  - [ ] `--json`
  - [ ] `--no-color`
  - [ ] `--log-level`
- [ ] Implement `doctor`.
- [ ] Implement `inspect`.
- [ ] Implement `decode`.
- [ ] Implement `replay`.
- [ ] Implement `strategy new`.
- [ ] Implement stable exit codes.
- [ ] Implement structured error output for `--json`.
- [ ] Implement human-readable summaries.
- [ ] Add shell completion only if cheap; otherwise delay.

Testing tasks:

- [ ] Use CLI integration tests with temp dirs.
- [ ] Test `--help` for every command.
- [ ] Test invalid command exits `2`.
- [ ] Test missing snapshot exits `1`.
- [ ] Test JSON output parses.
- [ ] Test `doctor` reports live unavailable on non-Linux or missing ring without panicking.
- [ ] Test `inspect` summary over fixture.
- [ ] Test `decode` JSONL over fixture.
- [ ] Test `replay` report over fixture.
- [ ] Test `strategy new` idempotency/error behavior.

Acceptance:

```bash
cargo test -p monad-mev-cli
cargo run -p monad-mev-cli -- doctor
cargo run -p monad-mev-cli -- --help
```

### WP-19: Live Observe Mode

Status: [ ]

Goal: prove the replay pipeline can graduate to a live Linux event ring without enabling production execution.

Implementation tasks:

- [ ] Gate live support behind feature flag if needed.
- [ ] Implement `LiveEventRingSource`.
- [ ] Support event ring name resolution.
- [ ] Support explicit event ring path.
- [ ] Validate content type.
- [ ] Validate schema hash.
- [ ] Poll descriptors.
- [ ] Convert to stream items.
- [ ] Apply live gap policy.
- [ ] Add bounded channel between poller and consumer if needed.
- [ ] Add graceful shutdown on duration/ctrl-c.
- [ ] Add live metrics counters.
- [ ] Wire `inspect --live`.
- [ ] Wire live observe example.
- [ ] Ensure no V1 path submits transactions.

Testing tasks:

- [ ] Unit test event ring path resolution where possible.
- [ ] Unit test live config parsing.
- [ ] Unit test bounded-channel backpressure behavior with fake source.
- [ ] Ignored live integration test using `MONAD_MEV_EVENT_RING`.
- [ ] Manual Linux runbook in docs.

Acceptance:

```bash
cargo test -p monad-mev-events --features live live
MONAD_MEV_EVENT_RING=monad-exec-events cargo test --test live_ring --features live -- --ignored
monad-mev inspect monad-exec-events --live --duration 10s --summary
```

### WP-20: Documentation, CI, Release Hardening

Status: [ ]

Goal: make V1 usable and maintainable after the first implementation pass.

Implementation tasks:

- [ ] Complete `docs/getting-started-snapshot.md`.
- [ ] Complete `docs/getting-started-live.md`.
- [ ] Complete `docs/event-semantics.md`.
- [ ] Complete `docs/writing-strategies.md`.
- [ ] Complete `docs/cli.md`.
- [ ] Complete `docs/troubleshooting.md`.
- [ ] Complete `docs/sdk-versioning.md`.
- [ ] Add GitHub Actions or equivalent CI.
- [ ] Add CI job for fmt.
- [ ] Add CI job for clippy.
- [ ] Add CI job for tests.
- [ ] Add CI job for coverage.
- [ ] Add ignored snapshot/live test documentation.
- [ ] Add release checklist.
- [ ] Add changelog.
- [ ] Review public API names.
- [ ] Review error messages.
- [ ] Review docs for claims about Monad platform support.

Testing tasks:

- [ ] Run full local verification.
- [ ] Run CI from clean checkout.
- [ ] Run docs examples or doctests where practical.
- [ ] Verify README quickstart works.

Acceptance:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features
cargo test --doc --all
```

## 6. Test Suite Design

### 6.1 Test Categories

V1 should use these test categories:

| Category | Purpose | Runs in CI |
|---|---|---|
| Unit tests | Fast validation of pure logic | Yes |
| Fixture tests | Deterministic event pipeline tests | Yes |
| Golden tests | Stable output compatibility | Yes |
| CLI integration tests | Real command behavior and exit codes | Yes |
| Snapshot integration tests | Real Monad snapshot compatibility | Optional/ignored |
| Live integration tests | Real Linux event-ring compatibility | Optional/ignored/self-hosted |
| Coverage job | Enforce behavioral coverage | Yes |
| Doctests | Keep public examples compiling | Yes where practical |

### 6.2 Unit Test Matrix

`monad-mev-core`:

- [ ] Error conversion and display.
- [ ] `StreamItem` helpers.
- [ ] `CommitState` ordering/min-state checks.
- [ ] `GapPolicy` defaults.
- [ ] `ReplayClock` parsing.
- [ ] `ReplayReport` aggregation.
- [ ] Strategy lifecycle behavior with fake strategy.
- [ ] Recording executor.
- [ ] Dry-run executor.
- [ ] JSON serialization of public report/action types.

`monad-mev-events`:

- [ ] SDK schema hash access.
- [ ] Content type validation.
- [ ] Schema match/mismatch.
- [ ] Snapshot source path errors.
- [ ] Descriptor metadata extraction.
- [ ] Raw event conversion.
- [ ] Unknown raw event preservation.
- [ ] Gap detection.
- [ ] Payload expiration.
- [ ] Commit-state transitions.
- [ ] Flow tag extraction.
- [ ] Transaction interleaving grouping.
- [ ] Chain log normalization.
- [ ] ERC20 transfer decode.
- [ ] ERC20 approval decode.
- [ ] Uniswap V2 swap decode.
- [ ] Uniswap V2 sync decode.
- [ ] Uniswap V3 swap decode.
- [ ] ABI JSON parsing.
- [ ] Generic ABI event decode.
- [ ] Malformed log handling.
- [ ] Replay filters.

`monad-mev-cli`:

- [ ] Help output.
- [ ] Invalid usage exit code.
- [ ] `doctor` no-snapshot mode.
- [ ] `doctor --json`.
- [ ] `inspect` summary over fixture.
- [ ] `inspect --json` parses.
- [ ] `decode --basic-defi`.
- [ ] `decode --abi`.
- [ ] `replay --report`.
- [ ] `replay --events-jsonl`.
- [ ] `replay --actions-jsonl`.
- [ ] `strategy new`.
- [ ] Existing destination error.
- [ ] Missing file error.

### 6.3 Fixture Test Matrix

Required fixtures:

- [ ] `empty.json`: no events.
- [ ] `single-block.json`: block start/end with no txs.
- [ ] `erc20-transfer.json`: one transaction with transfer log.
- [ ] `erc20-approval.json`: one transaction with approval log.
- [ ] `v2-swap-sync.json`: V2 swap and sync.
- [ ] `v3-swap.json`: V3 swap.
- [ ] `interleaved-transactions.json`: two or more tx flows interleaved.
- [ ] `commit-states.json`: block start, QC, finalized, verified.
- [ ] `gap.json`: sequence gap.
- [ ] `payload-expired.json`: payload expiration marker.
- [ ] `malformed-logs.json`: invalid topics/data.
- [ ] `unknown-events.json`: unknown event type and unknown log signature.

Fixture assertions:

- [ ] Event counts match.
- [ ] Decoded event counts match.
- [ ] Reports are deterministic.
- [ ] JSON output is stable.
- [ ] Golden files are easy to review.

### 6.4 Snapshot Integration Tests

Snapshot tests are ignored by default because large real snapshots should not be required for normal CI.

Environment:

```bash
MONAD_MEV_SNAPSHOT=./data/snapshot.zst
```

Tests:

- [x] Open snapshot.
- [x] Validate content type.
- [x] Validate schema hash.
- [x] Count raw events.
- [ ] Count logs.
- [ ] Decode basic DeFi events.
- [ ] Verify no unexpected panic on unknown events.
- [ ] Run replay report twice and compare stable counters.

Command:

```bash
MONAD_MEV_SNAPSHOT=./data/snapshot.zst cargo test -p monad-mev-events --test snapshot_replay -- --ignored
```

### 6.5 Live Integration Tests

Live tests are ignored by default and require Linux plus a Monad node running with `--exec-event-ring`.

Environment:

```bash
MONAD_MEV_EVENT_RING=monad-exec-events
```

Tests:

- [ ] Resolve event ring path/name.
- [ ] Open live ring.
- [ ] Validate content type.
- [ ] Validate schema hash.
- [ ] Read for fixed duration.
- [ ] Count events.
- [ ] Detect and report gaps.
- [ ] Graceful shutdown.
- [ ] No transaction submission path is called.

Command:

```bash
MONAD_MEV_EVENT_RING=monad-exec-events cargo test --test live_ring --features live -- --ignored
```

### 6.6 CLI Test Strategy

Use a real binary invocation in tests where practical.

Recommended crates:

- `assert_cmd`
- `predicates`
- `tempfile`
- `serde_json`

CLI assertions:

- [ ] Exit code is stable.
- [ ] Stderr contains actionable error on failure.
- [ ] `--json` output is valid JSON.
- [ ] Human summaries do not include unstable fields unless expected.
- [ ] JSON reports use stable field names.
- [ ] Snapshot paths in golden output are normalized or redacted.

### 6.7 Golden Output Policy

Golden files are allowed for:

- Replay reports.
- Decoded JSONL events.
- Recorded action JSONL.
- CLI JSON output.

Golden files should not include:

- Absolute local paths.
- Wall-clock timestamps unless normalized.
- Platform-specific error text.
- Random IDs unless seeded.

Golden update command should be documented before adding many golden files. Until then, update golden files manually and review diffs carefully.

### 6.8 Failure Injection Tests

V1 must test failure paths explicitly:

- [ ] Missing snapshot file.
- [ ] Corrupt snapshot file.
- [ ] Schema mismatch.
- [ ] Wrong content type.
- [ ] Gap in replay.
- [ ] Payload expired.
- [ ] Unknown raw event.
- [ ] Unknown log signature.
- [ ] Malformed ABI JSON.
- [ ] Malformed log data.
- [ ] Strategy returns error.
- [ ] Executor rejects action.
- [ ] CLI invalid flag.
- [ ] Output path unwritable.

### 6.9 Determinism Tests

Required deterministic checks:

- [ ] Same fixture replay produces identical report counters.
- [ ] Same fixture replay produces identical decoded JSONL, excluding normalized run metadata.
- [ ] Same strategy over same fixture produces identical action JSONL.
- [ ] Replay speed settings do not change event/action output.
- [ ] Hash maps used in public output are sorted or serialized deterministically.

## 7. CI Plan

Minimum CI jobs:

```text
ci-fmt
ci-clippy
ci-test
ci-doc
ci-coverage
ci-cli-smoke
```

### 7.1 `ci-fmt`

```bash
cargo fmt --all -- --check
```

### 7.2 `ci-clippy`

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### 7.3 `ci-test`

```bash
cargo test --workspace --all-features
```

### 7.4 `ci-doc`

```bash
cargo test --doc --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

### 7.5 `ci-coverage`

```bash
cargo llvm-cov --workspace --all-features --ignore-filename-regex 'target|bindings|examples' --fail-under-lines 90
```

### 7.6 `ci-cli-smoke`

```bash
cargo run -p monad-mev-cli -- --help
cargo run -p monad-mev-cli -- doctor
```

`doctor` may exit non-zero if live mode is unavailable, so CI should either run it in snapshot-only mode or assert the expected diagnostic behavior.

## 8. Development Order

Recommended order:

1. WP-01: Project bootstrap.
2. WP-02: SDK pinning and license.
3. WP-03: Workspace skeleton.
4. WP-04: Core types.
5. WP-05: Source abstraction and schema checks.
6. WP-06: Snapshot source.
7. WP-07: Raw event conversion.
8. WP-08: Gaps and stream health.
9. WP-09: Commit-state tracker.
10. WP-10: Flow tags and grouping.
11. WP-11: Chain normalization.
12. WP-16: Fixtures and golden foundation.
13. WP-12: Built-in DeFi decoders.
14. WP-13: Generic ABI decoder.
15. WP-14: Replay engine.
16. WP-15: Strategy API and executors.
17. WP-18: CLI commands.
18. WP-17: Examples and strategy scaffolding.
19. WP-19: Live observe.
20. WP-20: Docs, CI, hardening.

WP-16 appears before all user-facing replay work is complete because fixtures should stabilize the rest of development.

## 9. Open Decisions To Resolve Early

These should be resolved before or during WP-02:

- [ ] Exact Monad SDK tag/revision.
- [ ] License choice.
- [ ] Whether live support is behind a cargo feature.
- [ ] Whether V1 crates are published or kept Git-only.
- [ ] Whether a tiny official snapshot can be redistributed.
- [ ] Whether to expose official SDK raw types behind an advanced feature.

## 10. V1 Out-Of-Scope Guardrails

During V1 development, do not add:

- [ ] Real transaction submission.
- [ ] Private relay or bundle support.
- [ ] Local counterfactual EVM simulation.
- [ ] Production key management.
- [ ] Production risk engine.
- [ ] Full pool-state engine.
- [ ] CEX connectors.
- [ ] Dashboard.
- [ ] Python bindings.

If one of these becomes necessary, update `v1-spec.md` first and explicitly move the feature from V2 to V1.
