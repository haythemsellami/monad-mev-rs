# monad-mev-rs V2 Implementation Plan

Status date: 2026-05-25

## 1. Goal

V2 continues the V1 design. `monad-mev-rs` remains an Execution Events-first
MEV framework, not a generic bot engine where Execution Events are just another
optional source.

The V2 implementation goal is:

> Let a developer capture the Execution Events relevant to DeFi protocol X,
> attach local or imported strategy Y, maintain protocol state, optionally
> simulate and risk-check transaction candidates, and execute or record outcomes
> without touching raw event-ring APIs.

V2 is not a refactor reset. Every work package should extend V1's existing
public concepts where practical:

- `StreamItem<T>`
- `EventEnvelope<T>`
- `ChainEvent`
- replay and fixture APIs
- live observe APIs
- strategy and executor traits
- recording and dry-run executors

## 2. V2 Delivery Shape

V2 should be delivered in staged capability bands:

| Band | Purpose | Production submission |
|---|---|---|
| V2-A | Execution Events stream and protocol capture | No |
| V2-B | Strategy packages and engine runtime | No |
| V2-C | Protocol state stores and read-only opportunities | No |
| V2-D | Simulation and state hydration | No |
| V2-E | Risk and explicit execution plans | No by default |
| V2-F | Production executors behind opt-in features/config | Yes, explicit only |
| V2-G | Observability, persistence, docs, release hardening | Depends on config |

The first concrete V2 target is a read-only liquidation detector. It should run
over fixtures and snapshots before any production submission path exists.

## 3. Engineering Rules

- Keep Execution Events as the source of truth for on-chain activity.
- Treat RPC, timers, config reloads, and metadata fetches as auxiliary inputs.
- Preserve replay/live semantic parity.
- Do not introduce production transaction submission until simulation, risk,
  nonce, audit, and fake-transport tests exist.
- Default examples must stay recording or dry-run only.
- Do not require users to understand raw descriptors or SDK payload lifetimes
  for ordinary protocol/strategy development.
- Add abstractions only where V1 APIs become insufficient for real protocol
  capture or strategy execution.
- Keep production behavior explicit and auditable.

## 4. Milestones

### M1: Execution Events Stream And Capture

Goal: make V1 event sources look like one reusable stream contract and let users
capture protocol-specific subsets.

Work packages:

- WP-01: shared Execution Events stream abstraction.
- WP-02: protocol capture filters.
- WP-03: replay/live parity checks.

Exit criteria:

- Fixture, snapshot, and live observe paths can expose the same stream item
  shape.
- A protocol filter can select events by address, topic, kind, transaction, and
  commit-state requirement.
- Capture output is typed enough for strategies to consume without raw
  descriptor access.

### M2: Strategy Packages And Engine Runtime

Goal: support local/imported strategies and route captured protocol events into
them.

Work packages:

- WP-04: strategy package conventions.
- WP-05: `monad-mev-engine` crate.
- WP-06: strategy subscriptions and routing.
- WP-07: adapters for typed events/actions.

Exit criteria:

- One Execution Events stream can feed multiple protocol adapters and
  strategies.
- Strategies can be local crates or generated/imported templates.
- Strategy packages can expose metadata, subscriptions, and tests.

### M3: State Stores And Protocol Adapters

Goal: maintain protocol state from captured Execution Events.

Work packages:

- WP-08: `monad-mev-store` crate.
- WP-09: ERC20 metadata and balance primitives.
- WP-10: DEX V2 pool state and exact math.
- WP-11: DEX swap monitor template.
- WP-12: lending protocol adapter skeleton.
- WP-13: oracle state primitives.

Exit criteria:

- State can be updated from replayed events deterministically.
- State views include commit-state context.
- DEX V2 math has exact fixture tests.
- Lending adapter can represent markets, borrowers, debt, collateral, and
  oracle inputs.

### M4: Read-Only Liquidation Detector

Goal: produce liquidation candidates without transaction construction or
submission.

Work packages:

- WP-14: liquidation opportunity model.
- WP-15: read-only liquidation detector strategy.
- WP-16: liquidation fixtures and golden output.

Exit criteria:

- Healthy accounts are ignored.
- Unhealthy accounts emit deterministic candidates.
- Stale oracle data rejects candidates.
- Candidate records include source event refs and state version.

### M5: Simulation

Goal: turn opportunities into transaction candidates and simulate them.

Work packages:

- WP-17: `monad-mev-sim` request/result API.
- WP-18: RPC-backed lazy state hydration.
- WP-19: Monad EVM simulation integration.
- WP-20: simulation golden tests.

Exit criteria:

- Simulation records success/revert, gas, deltas, expected profit, and state
  freshness.
- Missing state reads are auditable.
- Failed or stale simulations prevent execution by default.

### M6: Risk And Execution Plans

Goal: make execution decisions explicit, explainable, and safe by default.

Work packages:

- WP-21: `monad-mev-risk` crate.
- WP-22: execution plan and audit log model.
- WP-23: risk policy engine.
- WP-24: nonce and replacement policy.

Exit criteria:

- Risk approvals/rejections are structured and explainable.
- Execution plans cannot be built without simulation and risk output unless an
  explicit unsafe mode is enabled.
- Audit logs are deterministic JSONL.

### M7: Explicit Production Executors

Goal: add submission interfaces behind opt-in features and config.

Work packages:

- WP-25: production executor interfaces.
- WP-26: fake transport and executor harness.
- WP-27: RPC submit executor.
- WP-28: relay/bundle executor interfaces if Monad exposes a stable path.
- WP-29: simulated liquidation template.
- WP-30: risk-checked liquidation submit path.

Exit criteria:

- Production submission is impossible by default.
- Every production executor has fake transport tests.
- Private keys are never embedded in examples.
- Submitted receipts are visibly distinct from dry-run and simulation receipts.

### M8: Operations And Release

Goal: make V2 observable, testable, and usable outside local development.

Work packages:

- WP-31: metrics and tracing.
- WP-32: persistent state backend.
- WP-33: snapshot/live parity test suite.
- WP-34: production runbook.
- WP-35: release hardening.

Exit criteria:

- Metrics cover ingestion, strategy, simulation, risk, execution, backpressure,
  gaps, and drops.
- Persistent state has deterministic restore tests.
- Docs explain replay, dry-run, simulation, and production modes.

## 5. Work Packages

### WP-01: Shared Execution Events Stream Abstraction

Status: [ ]

Goal: expose fixture, snapshot, and live observe sources through one
Execution Events-first stream contract.

Implementation tasks:

- [ ] Define `ExecutionEventStream<T>` or equivalent wrapper around
  `StreamItem<T>`.
- [ ] Add source metadata carrying fixture/snapshot/live source kind.
- [ ] Preserve V1 `StreamItem` variants.
- [ ] Preserve `EventEnvelope` metadata.
- [ ] Add stream construction from fixture documents.
- [ ] Add stream construction from snapshot readers.
- [ ] Add live observe stream constructor behind `live`.
- [ ] Keep source-specific raw APIs available for advanced users.

Testing tasks:

- [ ] Fixture stream parity test.
- [ ] Snapshot stream parity test using synthetic snapshot data.
- [ ] Live observe stream unit test behind `live`.
- [ ] Source metadata serialization test.
- [ ] Gap and payload-expiration propagation test.

Acceptance:

```bash
cargo test -p monad-mev-events execution_stream
cargo test -p monad-mev-events --features live live
```

### WP-02: Protocol Capture Filters

Status: [ ]

Goal: let users capture events for protocol X without raw event-ring or ABI
plumbing.

Implementation tasks:

- [ ] Define `ProtocolCapture`.
- [ ] Define capture criteria: addresses, topic0, event kinds, transaction
  index, block range, commit-state minimum.
- [ ] Define capture output carrying `EventEnvelope<ChainEvent>`.
- [ ] Add reusable `AddressSet` and topic filters.
- [ ] Add ABI-backed event capture helper.
- [ ] Add built-in DeFi capture presets: ERC20, DEX swaps, DEX syncs.
- [ ] Add explain output for why an event matched or was ignored.

Testing tasks:

- [ ] Address filter test.
- [ ] Topic filter test.
- [ ] Combined filter test.
- [ ] Commit-state minimum test.
- [ ] ABI-backed filter test.
- [ ] Determinism test over fixture streams.

Acceptance:

```bash
cargo test -p monad-mev-events protocol_capture
```

### WP-03: Replay/Live Parity Checks

Status: [ ]

Goal: ensure replay, snapshot, and live observe paths use the same event-time,
filtering, and routing semantics.

Implementation tasks:

- [ ] Define framework event time.
- [ ] Normalize fixture event time to the same representation.
- [ ] Normalize snapshot record timestamps.
- [ ] Normalize live record timestamps.
- [ ] Ensure filtering happens at the same semantic point for replay and live.
- [ ] Add parity report output.

Testing tasks:

- [ ] Fixture vs snapshot-equivalent parity test.
- [ ] Multi-stream interleaving test.
- [ ] Filter-before-time and time-before-filter regression test.
- [ ] Golden parity report.

Acceptance:

```bash
cargo test --workspace replay_live_parity
```

### WP-04: Strategy Package Conventions

Status: [ ]

Goal: let developers attach local or imported strategies to captured protocol
events.

Implementation tasks:

- [ ] Define strategy package manifest fields.
- [ ] Define strategy metadata: name, version, event subscriptions, action
  output type, risk requirements.
- [ ] Extend `strategy new` for V2 strategy packages.
- [ ] Add generated tests using protocol capture fixtures.
- [ ] Add examples for local crate import.
- [ ] Document how third-party strategy crates integrate.

Testing tasks:

- [ ] Generated package compiles.
- [ ] Generated package test passes.
- [ ] Imported package fixture test.
- [ ] Manifest validation test.

Acceptance:

```bash
cargo test -p monad-mev-cli strategy_package
```

### WP-05: `monad-mev-engine`

Status: [ ]

Goal: add an engine crate that routes Execution Events streams into strategies
and executors.

Implementation tasks:

- [ ] Add `crates/monad-mev-engine`.
- [ ] Define engine config.
- [ ] Define collector/source registration for Execution Events streams.
- [ ] Define auxiliary collector registration for RPC/timers/config.
- [ ] Define strategy registration.
- [ ] Define executor registration.
- [ ] Add bounded channel config.
- [ ] Add shutdown handling.
- [ ] Add sync and async compatibility decision.
- [ ] Add recording executor integration.

Testing tasks:

- [ ] Single stream single strategy test.
- [ ] Single stream multiple strategy test.
- [ ] Multiple protocol adapters test.
- [ ] Bounded channel backpressure test.
- [ ] Strategy error shutdown test.
- [ ] Source-ended shutdown test.

Acceptance:

```bash
cargo test -p monad-mev-engine
```

### WP-06: Strategy Subscriptions And Routing

Status: [ ]

Goal: route events only to strategies that requested them.

Implementation tasks:

- [ ] Define `StrategySubscription`.
- [ ] Support event kind filters.
- [ ] Support address/topic filters.
- [ ] Support protocol adapter output type.
- [ ] Support minimum commit-state requirement.
- [ ] Add routing explain/debug mode.

Testing tasks:

- [ ] Route matching event test.
- [ ] Drop non-matching event test.
- [ ] Multiple subscribers test.
- [ ] Commit-state routing test.
- [ ] Routing explain test.

Acceptance:

```bash
cargo test -p monad-mev-engine routing
```

### WP-07: Typed Event And Action Adapters

Status: [ ]

Goal: let strategies use local event/action types while still plugging into the
shared engine.

Implementation tasks:

- [ ] Define event adapter trait.
- [ ] Define action adapter trait.
- [ ] Add map/filter adapter helpers.
- [ ] Add boxed adapter support.
- [ ] Add adapter error type.
- [ ] Add examples adapted from Artemis' wrapper pattern.

Testing tasks:

- [ ] Event map test.
- [ ] Event filter test.
- [ ] Action map test.
- [ ] Adapter error propagation test.
- [ ] Strategy with local action type test.

Acceptance:

```bash
cargo test -p monad-mev-engine adapter
```

### WP-08: `monad-mev-store`

Status: [ ]

Goal: provide commit-state-aware state views for protocol adapters and
strategies.

Implementation tasks:

- [ ] Add `crates/monad-mev-store`.
- [ ] Define `StateVersion`.
- [ ] Define `CommitScopedView`.
- [ ] Add in-memory store.
- [ ] Add snapshot/restore for tests.
- [ ] Add rollback handling model.
- [ ] Add state update audit records.

Testing tasks:

- [ ] Insert/update/read test.
- [ ] Commit-state scoped read test.
- [ ] Rollback/regression test.
- [ ] Snapshot/restore determinism test.
- [ ] Audit record test.

Acceptance:

```bash
cargo test -p monad-mev-store
```

### WP-09: ERC20 Metadata And Balance Primitives

Status: [ ]

Goal: support protocol adapters that need token metadata and balance deltas.

Implementation tasks:

- [ ] Define token metadata model.
- [ ] Define balance state model.
- [ ] Add ERC20 transfer state update helper.
- [ ] Add optional RPC metadata fetch interface.
- [ ] Add fixture metadata loader.

Testing tasks:

- [ ] Transfer balance update test.
- [ ] Unknown metadata fallback test.
- [ ] Fixture metadata test.
- [ ] Decimal handling test.

Acceptance:

```bash
cargo test -p monad-mev-protocols erc20
```

### WP-10: DEX V2 Pool State And Exact Math

Status: [ ]

Goal: maintain Uniswap V2-style pool state and compute exact swap outputs.

Implementation tasks:

- [ ] Add `crates/monad-mev-protocols`.
- [ ] Define V2 pool model.
- [ ] Update reserves from Sync logs.
- [ ] Update inferred state from Swap logs where safe.
- [ ] Implement constant-product exact output math.
- [ ] Respect token decimals.
- [ ] Add stale/unknown pool state marker.

Testing tasks:

- [ ] Sync updates reserves.
- [ ] Swap math exact known cases.
- [ ] Fee handling test.
- [ ] Stale pool rejects route.
- [ ] Golden fixture replay test.

Acceptance:

```bash
cargo test -p monad-mev-protocols dex_v2
```

### WP-11: DEX Swap Monitor Template

Status: [ ]

Goal: provide the first V2 strategy template over protocol capture/state.

Implementation tasks:

- [ ] Add DEX swap monitor strategy package.
- [ ] Subscribe to DEX swap/sync captures.
- [ ] Maintain pool state.
- [ ] Emit structured records.
- [ ] Add CLI scaffold option.

Testing tasks:

- [ ] Fixture replay monitor test.
- [ ] Golden JSONL output test.
- [ ] Missing pool state warning test.

Acceptance:

```bash
cargo test -p monad-mev-strategies dex_swap_monitor
```

### WP-12: Lending Protocol Adapter Skeleton

Status: [ ]

Goal: define protocol-neutral lending state needed for liquidation detection.

Implementation tasks:

- [ ] Define market config.
- [ ] Define borrower collateral/debt state.
- [ ] Define liquidation thresholds.
- [ ] Define close factor and liquidation bonus.
- [ ] Define protocol pause flags.
- [ ] Add adapter trait for protocol-specific event mapping.
- [ ] Add fixture protocol implementation.

Testing tasks:

- [ ] Market config validation.
- [ ] Borrower state update.
- [ ] Pause flag behavior.
- [ ] Fixture adapter replay test.

Acceptance:

```bash
cargo test -p monad-mev-protocols lending
```

### WP-13: Oracle State Primitives

Status: [ ]

Goal: represent oracle prices and reject stale inputs by default.

Implementation tasks:

- [ ] Define oracle price model.
- [ ] Track decimals and update time.
- [ ] Add freshness policy.
- [ ] Add confidence/source fields where available.
- [ ] Add fixture oracle source.

Testing tasks:

- [ ] Fresh price accepted.
- [ ] Stale price rejected.
- [ ] Decimal conversion test.
- [ ] Missing oracle price test.

Acceptance:

```bash
cargo test -p monad-mev-protocols oracle
```

### WP-14: Liquidation Opportunity Model

Status: [ ]

Goal: define typed liquidation candidates independent of execution.

Implementation tasks:

- [ ] Define `LiquidationCandidate`.
- [ ] Include borrower, market, collateral, debt, oracle state, and source
  event refs.
- [ ] Include expected repay/seize bounds.
- [ ] Include rejection reasons.
- [ ] Add serialization.

Testing tasks:

- [ ] Candidate serialization test.
- [ ] Rejection reason test.
- [ ] Source event refs test.

Acceptance:

```bash
cargo test -p monad-mev-protocols liquidation_candidate
```

### WP-15: Read-Only Liquidation Detector Strategy

Status: [ ]

Goal: detect unhealthy accounts and record candidates without building or
submitting transactions.

Implementation tasks:

- [ ] Add read-only liquidation strategy.
- [ ] Subscribe to lending and oracle captures.
- [ ] Compute account health.
- [ ] Reject healthy accounts.
- [ ] Reject stale oracle state.
- [ ] Emit `RecordAction` for candidates.
- [ ] Add CLI scaffold/template.

Testing tasks:

- [ ] Healthy account ignored.
- [ ] Unhealthy account emits candidate.
- [ ] Stale oracle rejects candidate.
- [ ] Pause flag rejects candidate.
- [ ] Golden JSONL output.

Acceptance:

```bash
cargo test -p monad-mev-strategies liquidation_read_only
```

### WP-16: Liquidation Fixtures And Goldens

Status: [ ]

Goal: provide deterministic fixture coverage for liquidation strategy work.

Implementation tasks:

- [ ] Add lending market fixture.
- [ ] Add borrower state fixture.
- [ ] Add oracle fixture.
- [ ] Add healthy and unhealthy account cases.
- [ ] Add stale oracle case.
- [ ] Add golden candidate JSONL.

Testing tasks:

- [ ] Fixture loader test.
- [ ] Golden output determinism test.
- [ ] Replay through engine test.

Acceptance:

```bash
cargo test --workspace liquidation_fixture
cargo test --workspace liquidation_golden
```

### WP-17: `monad-mev-sim` Request/Result API

Status: [ ]

Goal: define simulation contracts before selecting the final EVM backend.

Implementation tasks:

- [ ] Add `crates/monad-mev-sim`.
- [ ] Define `SimulationRequest`.
- [ ] Define `SimulationResult`.
- [ ] Define `SimulationContext`.
- [ ] Define placement model.
- [ ] Define balance delta model.
- [ ] Define revert diagnostics.
- [ ] Add JSON serialization.

Testing tasks:

- [ ] Request serialization.
- [ ] Result serialization.
- [ ] Placement model validation.
- [ ] Revert diagnostic formatting.

Acceptance:

```bash
cargo test -p monad-mev-sim
```

### WP-18: RPC-Backed Lazy State Hydration

Status: [ ]

Goal: fetch missing state for simulation with auditable reads.

Implementation tasks:

- [ ] Define state provider trait.
- [ ] Add RPC provider interface.
- [ ] Add cached provider wrapper.
- [ ] Record every remote state read.
- [ ] Add timeout and retry policy.
- [ ] Add fake provider.

Testing tasks:

- [ ] Fake provider happy path.
- [ ] Cache hit/miss test.
- [ ] Missing state error test.
- [ ] Audit remote reads test.
- [ ] Timeout policy test.

Acceptance:

```bash
cargo test -p monad-mev-sim state_hydration
```

### WP-19: Monad EVM Simulation Integration

Status: [ ]

Goal: execute transaction candidates against Monad-compatible EVM semantics.

Implementation tasks:

- [ ] Select backend crate/version.
- [ ] Add feature-gated backend dependency.
- [ ] Map `TxCandidate` into backend transaction.
- [ ] Map state provider into backend DB.
- [ ] Map result into `SimulationResult`.
- [ ] Add gas and balance delta extraction.
- [ ] Add revert diagnostics.

Testing tasks:

- [ ] Simple successful call simulation.
- [ ] Revert simulation.
- [ ] Missing state hydration test.
- [ ] Balance delta test.
- [ ] Feature-gated compile test.

Acceptance:

```bash
cargo test -p monad-mev-sim --features monad-evm
```

### WP-20: Simulation Golden Tests

Status: [ ]

Goal: make simulation deterministic enough to trust and review.

Implementation tasks:

- [ ] Add simulation fixture format.
- [ ] Add golden success result.
- [ ] Add golden revert result.
- [ ] Add stale state fixture.
- [ ] Add profit calculation fixture.

Testing tasks:

- [ ] Golden success result test.
- [ ] Golden revert result test.
- [ ] Profit determinism test.
- [ ] Stale state rejection test.

Acceptance:

```bash
cargo test -p monad-mev-sim golden
```

### WP-21: `monad-mev-risk`

Status: [ ]

Goal: add mandatory risk decisions before production execution.

Implementation tasks:

- [ ] Add `crates/monad-mev-risk`.
- [ ] Define `RiskPolicy`.
- [ ] Define `RiskDecision`.
- [ ] Define `RiskRejection`.
- [ ] Add max gas, max notional, min profit, max loss.
- [ ] Add allowed target and selector policies.
- [ ] Add oracle freshness and simulation freshness checks.
- [ ] Add circuit breaker.

Testing tasks:

- [ ] Max gas reject.
- [ ] Min profit reject.
- [ ] Stale oracle reject.
- [ ] Stale simulation reject.
- [ ] Allowed target/selector reject.
- [ ] Circuit breaker test.

Acceptance:

```bash
cargo test -p monad-mev-risk
```

### WP-22: Execution Plan And Audit Log

Status: [ ]

Goal: create a risk-approved execution object and durable audit records.

Implementation tasks:

- [ ] Define `TxCandidate`.
- [ ] Define `ExecutionPlan`.
- [ ] Define `AuditRecord`.
- [ ] Include source event refs.
- [ ] Include state version.
- [ ] Include simulation and risk output.
- [ ] Add JSONL writer.
- [ ] Add deterministic IDs.

Testing tasks:

- [ ] Execution plan serialization.
- [ ] Audit JSONL determinism.
- [ ] Missing simulation cannot build default plan.
- [ ] Missing risk approval cannot build default plan.

Acceptance:

```bash
cargo test -p monad-mev-core execution_plan
cargo test -p monad-mev-core audit
```

### WP-23: Risk Policy Engine

Status: [ ]

Goal: evaluate execution plans against configured risk policies.

Implementation tasks:

- [ ] Add policy config parser.
- [ ] Add policy composition.
- [ ] Add rejection explanation output.
- [ ] Add risk audit records.
- [ ] Add dry-run risk report CLI.

Testing tasks:

- [ ] Policy config test.
- [ ] Multiple rejection aggregation test.
- [ ] Risk audit record test.
- [ ] CLI dry-run report test.

Acceptance:

```bash
cargo test -p monad-mev-risk policy_engine
```

### WP-24: Nonce And Replacement Policy

Status: [ ]

Goal: define safe transaction sequencing behavior.

Implementation tasks:

- [ ] Define nonce provider trait.
- [ ] Define pending nonce state.
- [ ] Define replacement policy.
- [ ] Define max pending tx policy.
- [ ] Add fake provider.
- [ ] Add audit records.

Testing tasks:

- [ ] Fresh nonce test.
- [ ] Pending nonce test.
- [ ] Replacement allowed test.
- [ ] Replacement rejected test.
- [ ] Max pending reject test.

Acceptance:

```bash
cargo test -p monad-mev-exec nonce
```

### WP-25: Production Executor Interfaces

Status: [ ]

Goal: define production submission interfaces without enabling default
submission.

Implementation tasks:

- [ ] Add `crates/monad-mev-exec`.
- [ ] Define signer trait.
- [ ] Define submit transport trait.
- [ ] Define submit receipt.
- [ ] Define production mode config.
- [ ] Require explicit production enable flag.
- [ ] Redact signer config in debug output.

Testing tasks:

- [ ] Production disabled by default.
- [ ] Signer redaction test.
- [ ] Submit receipt serialization.
- [ ] Missing explicit config rejects submission.

Acceptance:

```bash
cargo test -p monad-mev-exec production_interface
```

### WP-26: Fake Transport And Executor Harness

Status: [ ]

Goal: test production executor logic without real network submission.

Implementation tasks:

- [ ] Add fake submit transport.
- [ ] Add fake signer.
- [ ] Add response scripting.
- [ ] Add latency/error injection.
- [ ] Add receipt capture.

Testing tasks:

- [ ] Successful fake submit.
- [ ] Failed fake submit.
- [ ] Retry policy test.
- [ ] Receipt audit test.

Acceptance:

```bash
cargo test -p monad-mev-exec fake_transport
```

### WP-27: RPC Submit Executor

Status: [ ]

Goal: submit approved execution plans over RPC behind explicit config.

Implementation tasks:

- [ ] Add RPC transport.
- [ ] Wire signer.
- [ ] Wire nonce manager.
- [ ] Wire replacement policy.
- [ ] Wire risk-approved execution plan input.
- [ ] Add feature gate if needed.
- [ ] Add docs warning.

Testing tasks:

- [ ] Fake RPC submit test.
- [ ] Nonce use test.
- [ ] Replacement test.
- [ ] Runtime config reject test.
- [ ] No private key in logs test.

Acceptance:

```bash
cargo test -p monad-mev-exec rpc_submit
```

### WP-28: Relay/Bundle Executor Interfaces

Status: [ ]

Goal: prepare extension points for private relay or bundle submission if Monad
standardizes a path.

Implementation tasks:

- [ ] Define relay submit trait.
- [ ] Define bundle request/receipt types.
- [ ] Define relay capability discovery.
- [ ] Add placeholder docs for unsupported mode.

Testing tasks:

- [ ] Type serialization.
- [ ] Unsupported relay returns clear error.
- [ ] Fake relay submit test.

Acceptance:

```bash
cargo test -p monad-mev-exec relay
```

### WP-29: Simulated Liquidation Template

Status: [ ]

Goal: extend read-only liquidation detector to build and simulate transaction
candidates.

Implementation tasks:

- [ ] Add liquidation tx candidate builder.
- [ ] Add simulation integration.
- [ ] Add profit calculation.
- [ ] Add stale state rejection.
- [ ] Add simulated candidate audit record.

Testing tasks:

- [ ] Candidate builder test.
- [ ] Successful simulation test.
- [ ] Revert rejection test.
- [ ] Profit calculation test.
- [ ] Golden audit output.

Acceptance:

```bash
cargo test -p monad-mev-strategies liquidation_simulated
```

### WP-30: Risk-Checked Liquidation Submit Path

Status: [ ]

Goal: wire liquidation candidates through simulation, risk, nonce, and explicit
submission.

Implementation tasks:

- [ ] Add composite executor stage chain.
- [ ] Require simulation result.
- [ ] Require risk approval.
- [ ] Require explicit production config.
- [ ] Add submit dry-run mode.
- [ ] Add audit log for each stage.

Testing tasks:

- [ ] Default mode records only.
- [ ] Dry-run mode validates only.
- [ ] Missing risk rejects.
- [ ] Fake submit success.
- [ ] Fake submit failure audit.

Acceptance:

```bash
cargo test -p monad-mev-strategies liquidation_submit
```

### WP-31: Metrics And Tracing

Status: [ ]

Goal: expose operational visibility for live and production runs.

Implementation tasks:

- [ ] Define metric names.
- [ ] Add ingestion latency metrics.
- [ ] Add strategy latency metrics.
- [ ] Add simulation latency metrics.
- [ ] Add risk rejection counters.
- [ ] Add execution receipt counters.
- [ ] Add channel depth/dropped item metrics.
- [ ] Add structured tracing spans.

Testing tasks:

- [ ] Metric registration test.
- [ ] Counter increment test.
- [ ] Trace span smoke test.
- [ ] No-cardinality-explosion review test where practical.

Acceptance:

```bash
cargo test --workspace metrics
```

### WP-32: Persistent State Backend

Status: [ ]

Goal: add optional durable state for long-running strategies.

Implementation tasks:

- [ ] Choose first backend: SQLite or RocksDB.
- [ ] Add feature-gated backend.
- [ ] Persist protocol state snapshots.
- [ ] Persist audit records or index.
- [ ] Add restore path.
- [ ] Add migration/versioning model.

Testing tasks:

- [ ] Persist/restore test.
- [ ] Version mismatch test.
- [ ] Crash-like partial write test.
- [ ] Feature-gated compile test.

Acceptance:

```bash
cargo test -p monad-mev-store --features persistent
```

### WP-33: Snapshot/Live Parity Test Suite

Status: [ ]

Goal: prove strategy behavior is consistent between replay and live semantic
paths.

Implementation tasks:

- [ ] Add parity fixture format.
- [ ] Add captured stream golden.
- [ ] Add strategy output golden.
- [ ] Add timing parity assertions.
- [ ] Add live ignored test.

Testing tasks:

- [ ] Fixture parity test.
- [ ] Snapshot parity test.
- [ ] Live ignored parity test.
- [ ] Golden strategy output.

Acceptance:

```bash
cargo test --workspace parity
MONAD_MEV_EVENT_RING=monad-exec-events cargo test --workspace parity -- --ignored
```

### WP-34: Production Runbook

Status: [ ]

Goal: document how to run safely from replay to production.

Implementation tasks:

- [ ] Update getting-started docs for V2.
- [ ] Add protocol capture guide.
- [ ] Add strategy package guide.
- [ ] Add simulation guide.
- [ ] Add risk config guide.
- [ ] Add production executor guide.
- [ ] Add incident response guide.

Testing tasks:

- [ ] README smoke commands.
- [ ] Docs command smoke tests where practical.
- [ ] Link check where practical.

Acceptance:

```bash
cargo test --doc --workspace --all-features
```

### WP-35: Release Hardening

Status: [ ]

Goal: prepare V2 for use by external developers.

Implementation tasks:

- [ ] Review public API names.
- [ ] Review feature flags.
- [ ] Review error messages.
- [ ] Review unsafe/production gates.
- [ ] Review docs for platform claims.
- [ ] Update changelog.
- [ ] Update release checklist.
- [ ] Run full CI locally.

Testing tasks:

- [ ] Full workspace tests.
- [ ] Full all-feature clippy.
- [ ] Doctests.
- [ ] Generated strategy package tests.
- [ ] Fake production executor tests.

Acceptance:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc --workspace --all-features
```

## 6. Testing Strategy

### 6.1 Always-On Tests

These should run in CI:

- unit tests
- fixture tests
- golden JSON/JSONL tests
- protocol math tests
- engine routing tests
- state rollback tests
- risk policy tests
- fake executor tests
- doctests

### 6.2 Feature-Gated Tests

These run in CI when dependencies are available:

- Monad EVM simulation backend tests
- persistent backend tests
- RPC submit fake transport tests

### 6.3 Ignored Integration Tests

These require external infrastructure:

- real live event ring
- real snapshot corpus
- real RPC state hydration endpoint
- real production submit smoke test on a safe devnet account, if ever allowed

Ignored tests must be documented and must never require private keys in source.

## 7. CI Plan

V2 CI should include:

- format
- clippy all targets/all features
- workspace tests all features
- doctests
- coverage
- feature matrix for `live`, `persistent`, `monad-evm`, and executor features
- generated strategy package smoke test

Production executor tests must run with fake transports by default.

## 8. Documentation Plan

Required V2 docs:

- `docs/v2/getting-started-protocol-capture.md`
- `docs/v2/strategy-packages.md`
- `docs/v2/engine.md`
- `docs/v2/state-stores.md`
- `docs/v2/liquidation-read-only.md`
- `docs/v2/simulation.md`
- `docs/v2/risk.md`
- `docs/v2/execution.md`
- `docs/v2/production-runbook.md`

Docs must keep the mode boundaries clear:

- replay
- live observe
- recording
- dry-run
- simulation
- production submission

## 9. Open Decisions Before WP-10

These decisions should be resolved before DEX and liquidation work becomes too
concrete:

- async trait approach: native async traits, boxed futures, or sync wrappers
- first DEX protocol family
- first lending protocol target
- first simulation backend version
- first persistent backend
- first production executor transport
- crate publishing strategy
- feature naming for unsafe/production capabilities

## 10. Initial V2 MVP

The recommended MVP is WPs 1-16:

```text
Execution Events stream
-> protocol capture filters
-> strategy package conventions
-> engine runtime
-> in-memory state
-> protocol adapters
-> read-only liquidation detector
-> fixtures and goldens
```

This MVP proves the developer experience:

```text
capture events for protocol X -> attach strategy Y -> produce deterministic
opportunity records
```

Simulation, risk, and production execution should start only after this path is
clear and tested.
