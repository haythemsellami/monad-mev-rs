# monad-mev-rs V2 Spec

Status date: 2026-05-25

## 1. Purpose

V2 continues the V1 design: `monad-mev-rs` is a modular MEV framework built on
top of the Monad Execution Events client. The execution-event pipeline is the
spine of the framework, not an interchangeable side input.

V1 answered:

> Can a developer consume, replay, normalize, decode, and test Monad Execution
> Events without learning the raw event-ring API first?

V2 should answer:

> Can a developer subscribe to the Execution Events relevant to protocol X,
> feed those normalized events into strategy Y, and run that strategy through
> state, simulation, risk, and execution layers without working directly with
> the raw event-ring API?

V2 should stay modular, but modular around the Execution Events-first design.
Protocol adapters, strategy packages, state stores, simulators, risk gates, and
executors should plug into the event pipeline that V1 established.

V2 is not a rewrite or architecture reset. It should extend V1's raw event,
normalized event, replay, live observe, strategy, and executor abstractions into
a more complete searcher framework.

## 2. Artemis Lessons For The Framework Layer

Artemis is useful because it keeps the MEV bot mental model small:

- Collectors turn external data sources into internal events.
- Strategies consume events and produce actions.
- Executors process actions.
- An engine wires collectors, strategies, and executors together.

References:

- https://github.com/paradigmxyz/artemis
- https://github.com/paradigmxyz/artemis/tree/main/crates/artemis-core

V2 should borrow the mental model, not the exact implementation. In
`monad-mev-rs`, the primary collector family is the V1 Monad Execution Events
client: fixture replay, snapshot replay, and live event ring ingestion. Other
collectors such as RPC, timers, and config reloads are auxiliary inputs that
enrich or act on the Execution Events pipeline.

### 2.1 What To Borrow

Borrow the top-level pipeline:

```text
collector -> event -> strategy -> action -> executor
```

This maps cleanly to Monad:

- Execution Events collectors produce raw and normalized Monad event streams.
- Protocol adapters capture event subsets for DeFi protocol X.
- Strategy packages consume protocol-specific event/state views and produce
  actions or opportunities.
- JSONL recorders, dry-run validators, simulators, RPC submitters, and private
  relays can be executors or executor stages.

Borrow `sync_state` as a first-class strategy lifecycle step. Many DeFi
strategies need reserves, account state, token metadata, oracle data, or protocol
configuration before processing the first live event.

Borrow adapter wrappers. Artemis has mapping helpers that let a collector or
executor transform between local and shared event/action types. V2 should use
that pattern so users can keep strategy-specific types without forcing every
strategy into one giant enum.

Borrow example-driven development. Artemis ships a real strategy example, not
only abstract traits. V2 should ship realistic but safety-bounded examples:

- read-only liquidation detector
- simulated liquidation candidate
- DEX swap monitor with state
- simple two-pool arbitrage detector

### 2.2 What To Change

Artemis' simple broadcast-engine shape is a good starting point, but V2 needs
stronger production semantics:

- bounded channels with explicit overflow behavior
- graceful shutdown and cancellation
- deterministic replay/live parity
- per-strategy subscriptions and filtering, not always broadcast everything
- explicit stream health events
- typed simulation and risk stages before execution
- commit-state-aware strategy policies
- observability for latency, gaps, dropped items, simulation errors, and
  execution outcomes

V2 should avoid a single global `Events` or `Actions` enum as the only extension
mechanism. A core enum is fine for framework-provided types, but user strategies
should be able to use typed local events/actions through adapters.

V2 should also avoid moving away from the V1 event client contract. If a
strategy works over normalized `ChainEvent` streams in replay, the same strategy
should be able to run over snapshot and live event-ring streams through the same
semantic path.

### 2.3 What Not To Copy Blindly

Do not treat execution as simply "send tx." On Monad, production execution
needs:

- commit-state requirements
- nonce and replacement policy
- account balance checks
- simulation before submission
- loss limits and circuit breakers
- replayable audit logs

Do not make the engine hide failure. Backpressure, source gaps, payload
expiration, simulation failures, and executor errors must become observable
events or terminal errors according to policy.

## 3. V2 Product Definition

### 3.1 V2 One-Liner

A Monad Execution Events-first MEV framework where developers capture protocol
events, attach or import strategies, maintain protocol state, simulate
transaction candidates, apply risk gates, and execute through explicit audited
executors.

### 3.2 V2 Users

Primary:

- Monad searchers building liquidation, arbitrage, monitoring, and routing
  systems.
- DeFi protocol teams building real-time risk monitors and keeper bots.
- Infrastructure teams running event-driven indexers or alerting systems.

Secondary:

- Researchers comparing strategies over replay data.
- Teams that want a staged path from read-only detection to simulated execution
  to production submission.

### 3.3 V2 Success Criteria

V2 is successful when a developer can:

1. Select event filters for a DeFi protocol without touching raw event-ring
   descriptors.
2. Run a protocol adapter over fixture, snapshot, or live Execution Events.
3. Attach a local or imported strategy to the protocol event/state stream.
4. Maintain protocol state from replay or live events.
5. Detect a liquidation candidate without transaction submission.
6. Simulate a liquidation candidate against hydrated state.
7. Apply risk rules and reject unsafe candidates.
8. Submit only through an explicit production executor.
9. Reproduce decisions from audit logs.
10. Run the same strategy against fixture, snapshot, and live sources with the
    same event-time semantics.

## 4. Scope

### 4.0 V1 Capability Gap

V1 provides the raw materials:

- normalized Execution Events
- deterministic fixture and replay paths
- basic DeFi and ABI decoders
- strategy and executor traits
- recording and dry-run executors
- observe-only live scaffolding

V1 does not yet provide the full developer experience of:

```text
capture events for protocol X -> attach strategy Y -> maintain protocol state
-> simulate/risk-check -> execute or record the result
```

V2 exists to close that gap without forcing developers down into raw
execution-event descriptors or payload decoding.

### 4.1 Included In V2

V2 includes:

- Execution Events-first pipeline engine.
- Strategy lifecycle with `sync_state`, `on_event`, `on_gap`, and shutdown.
- Event/action adapter layer.
- Strategy subscriptions and routing.
- Protocol event capture filters.
- Local and imported strategy package support.
- Protocol state stores.
- DEX state adapters.
- Lending protocol state adapters.
- Oracle and token metadata adapters.
- Read-only liquidation detector template.
- Counterfactual simulation API.
- RPC-backed lazy state hydration for simulation.
- Transaction candidate and execution-plan types.
- Risk engine.
- Recording, dry-run, simulation, RPC, and relay-style executor interfaces.
- Nonce and replacement policy.
- Persistent audit logs.
- Metrics and tracing integration.
- Snapshot/live parity tests.

### 4.2 Excluded From V2 Unless Explicitly Promoted

V2 should not initially include:

- Full UI/dashboard.
- Python bindings.
- Cross-chain strategies.
- CEX connectors.
- Fully automated market-making systems.
- Custodial key management.
- Hosted infrastructure.

These can be later V2.x or V3 work.

## 5. Proposed Workspace Additions

### 5.1 `monad-mev-engine`

Owns the multi-stage runtime around the V1 Execution Events pipeline:

- Execution Events source registration
- auxiliary collector registration
- strategy registration
- executor registration
- event/action channels
- subscriptions
- backpressure policy
- shutdown
- replay/live clock policy
- metrics hooks

Core traits:

```rust
trait Collector<E> {
    async fn stream(&mut self) -> Result<CollectorStream<E>>;
}

trait Strategy<E, A> {
    async fn sync_state(&mut self, ctx: &mut StrategyContext) -> Result<()>;
    async fn on_event(&mut self, ctx: &mut StrategyContext, event: &EventEnvelope<E>) -> Result<Vec<A>>;
    async fn on_gap(&mut self, ctx: &mut StrategyContext, gap: &GapEvent) -> Result<GapDecision>;
}

trait Executor<A> {
    async fn execute(&mut self, action: A) -> Result<ExecutionReceipt>;
}
```

V1 already has synchronous strategy/executor traits. V2 can either add async
variants or introduce a runtime crate that wraps synchronous strategies.

### 5.2 `monad-mev-store`

Owns state storage abstractions:

- in-memory state
- snapshot-able state
- rollback-aware state views
- optional RocksDB/SQLite backends
- block/commit-state scoped reads

State stores must expose what commit state they represent:

```text
proposed -> voted -> finalized -> verified
```

Strategies must be able to require a minimum state.

### 5.3 `monad-mev-protocols`

Owns protocol-specific state and decoders:

- ERC20 metadata and balances
- Uniswap V2-style pools
- Uniswap V3-style pools
- lending markets
- oracle feeds
- protocol configuration

This crate should avoid production execution logic. It should produce state,
events, and typed opportunity inputs.

### 5.4 `monad-mev-sim`

Owns counterfactual simulation:

- `SimulationRequest`
- `SimulationResult`
- state hydration
- revert diagnostics
- gas estimates
- balance deltas
- profit accounting
- placement model

Likely dependencies:

- `monad-revm`
- `alloy-monad-evm`
- Alloy RPC/state types

Simulation must be treated as a safety boundary. A failed or stale simulation
must prevent production submission unless an explicit unsafe policy is enabled.

### 5.5 `monad-mev-risk`

Owns risk decisions:

- max notional
- max gas
- min expected profit
- max loss
- deadline block
- commit-state requirement
- balance checks
- nonce freshness
- duplicate opportunity guard
- protocol cooldown
- circuit breaker

Risk output:

```rust
enum RiskDecision {
    Approve(ExecutionPlan),
    Reject(RiskRejection),
    RequireMoreData(Vec<MissingInput>),
}
```

### 5.6 `monad-mev-exec`

Owns execution infrastructure:

- recording executor
- dry-run executor
- simulation executor
- RPC submit executor
- private relay executor if available
- bundle executor if available
- nonce manager
- replacement manager
- execution receipts

Production executors must never be default. The user should have to explicitly
opt into signing and submission.

### 5.7 `monad-mev-strategies`

Owns example templates:

- read-only liquidation detector
- simulated liquidation detector
- DEX swap monitor
- two-pool arbitrage detector
- protocol health monitor

Templates should be small enough to read but realistic enough to show state,
simulation, risk, and execution boundaries.

## 6. Core Data Model

### 6.1 Opportunity

An opportunity is a typed strategy output before simulation:

```rust
struct Opportunity {
    id: OpportunityId,
    kind: OpportunityKind,
    source_event: EventRef,
    observed_at: ReplayOrLiveTime,
    required_commit_state: CommitState,
    payload: OpportunityPayload,
}
```

Examples:

- liquidation candidate
- DEX arbitrage candidate
- stale oracle alert
- pool imbalance

### 6.2 Transaction Candidate

A transaction candidate is an executable shape before risk approval:

```rust
struct TxCandidate {
    opportunity_id: OpportunityId,
    to: Option<Address>,
    calldata: Bytes,
    value: U256,
    gas_limit_hint: Option<u64>,
    deadline: Deadline,
}
```

### 6.3 Simulation Result

Simulation result should include:

- success/failure
- revert reason
- gas used
- state diff
- balance deltas
- expected profit
- observed stale-state markers
- simulation block/context

### 6.4 Execution Plan

Execution plan is the risk-approved object:

```rust
struct ExecutionPlan {
    candidate: TxCandidate,
    simulation: SimulationResult,
    risk: RiskApproval,
    nonce_policy: NoncePolicy,
    submit_policy: SubmitPolicy,
}
```

### 6.5 Audit Record

Every production path must emit audit records:

- source event refs
- state version
- opportunity
- simulation request/result
- risk decision
- execution plan
- submission receipt
- errors and retries

Audit records should be JSONL-compatible and deterministic enough for
postmortem replay.

## 7. Engine Design

### 7.1 Pipeline

V2 engine pipeline:

```text
Execution Events client
  -> raw/normalized event stream
  -> protocol capture filters
  -> protocol event/state adapters
  -> strategy router
  -> strategy workers
  -> opportunity/action bus
  -> simulation/risk stages
  -> executor router
  -> receipts/audit log
```

Auxiliary collectors can feed configuration updates, RPC backfills, timers, or
oracle metadata into the same runtime, but they should not replace the Execution
Events spine for Monad on-chain activity.

### 7.2 Subscriptions

Strategies should declare subscriptions:

- event kinds
- addresses
- topics
- protocol IDs
- block commit-state minimum
- replay/live mode compatibility

Subscriptions let the engine avoid broadcasting every event to every strategy.

### 7.3 Backpressure

Every channel must have a policy:

- block producer
- drop oldest
- drop newest
- fail fast
- enter risk-off

Defaults:

- replay: fail fast
- live observe: risk-off then fail
- production execution: risk-off then fail

### 7.4 Shutdown

Engine must support:

- source ended
- ctrl-c
- max duration
- fatal gap
- fatal simulation/risk/executor error
- strategy-requested shutdown

Shutdown should flush audit logs and receipts.

## 8. Protocol State

### 8.1 DEX State

V2 DEX state should include:

- pool address
- token0/token1
- reserves for V2-style pools
- V3 slot0/tick/liquidity where supported
- fee tier
- last update event ref
- commit-state scoped view

V2 should support exact math for supported pool families before enabling
strategy templates that depend on it.

### 8.2 Lending State

V2 lending state should include:

- market config
- collateral factors
- liquidation thresholds
- borrower collateral
- borrower debt
- oracle prices
- close factor
- liquidation bonus
- protocol pause flags

The first liquidation template should be read-only. It should emit candidates
and audit records, not transactions.

### 8.3 Oracle State

Oracle adapters should expose:

- price
- decimals
- last update
- staleness window
- source
- confidence if available

Strategies must reject stale oracle inputs by default.

## 9. Simulation

### 9.1 Requirements

Simulation must answer:

- Would the transaction revert?
- What gas would it consume?
- What balance deltas would it create?
- What profit remains after gas?
- Was the simulated state fresh enough?

### 9.2 State Hydration

Initial V2 simulation can use RPC-backed lazy state hydration. Later V2 can add
snapshot-backed local state.

The simulator must record every missing state read that was fetched remotely.
This matters for reproducibility.

### 9.3 Placement Model

Simulation must encode where the transaction is assumed to land:

- after current event
- end of current transaction
- end of current block
- next block
- private relay/bundle-specific placement

Bad placement assumptions can invalidate profits, so placement must be explicit.

## 10. Execution

### 10.1 Executor Stages

Execution should be staged:

```text
record -> dry-run validate -> simulate -> risk approve -> sign -> submit -> receipt
```

Users can stop at any stage.

### 10.2 Production Submission

Production submission requires:

- signer configuration
- nonce manager
- replacement policy
- gas policy
- balance checks
- RPC/relay health checks
- retry policy
- audit logs

No V2 example should encourage putting a private key in source code.

### 10.3 Executor Types

Executor types:

- `RecordingExecutor`
- `DryRunExecutor`
- `SimulationExecutor`
- `RpcSubmitExecutor`
- `RelayExecutor`
- `CompositeExecutor`

`CompositeExecutor` should enforce stage order.

## 11. Risk Engine

Risk engine must be a mandatory production gate.

Minimum policies:

- max gas
- max notional
- min expected profit
- max expected loss
- max retries
- max pending transactions
- allowed target contracts
- allowed function selectors
- oracle freshness
- simulation freshness
- duplicate opportunity suppression
- circuit breaker

Risk decisions must be explainable in logs.

## 12. Liquidation V2 Target

The first V2 strategy target should be a staged liquidation workflow:

### 12.1 Stage 1: Read-Only Detector

Inputs:

- lending protocol config
- borrower positions
- oracle prices
- collateral/debt updates

Output:

- `Opportunity::LiquidationCandidate`
- JSONL audit record
- no transaction candidate

Acceptance:

- fixture-backed test detects an unhealthy account
- healthy accounts are ignored
- stale oracle data rejects candidate
- replay and live observe use same state update path

### 12.2 Stage 2: Simulated Liquidation

Adds:

- transaction candidate builder
- RPC-backed state hydration
- simulation result
- profit calculation

Acceptance:

- successful liquidation simulation produces positive expected profit
- reverted liquidation is rejected
- stale state is rejected

### 12.3 Stage 3: Risk-Checked Execution

Adds:

- risk policy
- signer abstraction
- nonce manager
- submit executor

Acceptance:

- default config still records/dry-runs only
- production config requires explicit submit mode
- all submit attempts produce audit records

## 13. Advanced DEX V2 Target

DEX work should progress from state correctness to routing:

1. V2-style pool reserve state.
2. V2 exact swap math.
3. V3 tick/liquidity state.
4. V3 exact swap math for supported path.
5. Two-pool arbitrage detector.
6. Multi-hop route search.

Do not build routing on top of log decoding alone. Routing requires state.

## 14. Observability

V2 should expose:

- event ingestion latency
- strategy processing latency
- simulation latency
- executor latency
- channel depth
- dropped items
- gaps
- payload expirations
- risk rejects
- execution receipts

Targets:

- structured logs
- JSONL audit
- Prometheus metrics
- OpenTelemetry tracing where practical

## 15. Testing

V2 tests should include:

- unit tests for math and risk policies
- fixture tests for protocol adapters
- replay/live parity tests
- simulation golden tests
- executor dry-run tests
- nonce/replacement policy tests
- state rollback tests
- audit log determinism tests
- ignored integration tests for live rings and RPC simulation

Every production executor must have a fake transport test.

## 16. Security

V2 must treat production execution as high risk.

Required safeguards:

- no private keys in examples
- signer config redaction
- explicit production mode
- allowed contract list
- allowed selector list
- simulation required by default
- risk engine required by default
- clear dry-run vs submitted receipts
- audit records for every decision

## 17. V2 Work Packages

Suggested implementation order:

1. Extend V1 event sources into a shared Execution Events stream abstraction.
2. Add protocol capture filters over normalized `ChainEvent` streams.
3. Add strategy package/import conventions.
4. `monad-mev-engine`: async runtime, subscriptions, adapters, and routing.
5. Replay/live parity: one event-time and routing path for replay and live.
6. `monad-mev-store`: in-memory commit-state-aware state store.
7. `monad-mev-protocols`: ERC20 metadata and DEX V2 state.
8. DEX V2 exact math and swap monitor template.
9. Lending protocol adapter skeleton.
10. Read-only liquidation detector template.
11. `monad-mev-sim`: simulation request/result types.
12. RPC-backed state hydration.
13. Monad EVM simulation integration.
14. `monad-mev-risk`: risk policy engine.
15. Execution plan and audit log.
16. Nonce manager and replacement policy.
17. Production executor interfaces.
18. RPC submit executor behind explicit feature/config.
19. Simulated liquidation template.
20. Risk-checked liquidation submit path.
21. Metrics and tracing.
22. Persistent state backend.
23. Release hardening and production runbook.

## 18. V2 Acceptance Criteria

V2 is complete when:

- A read-only liquidation detector runs over fixtures and snapshots.
- Developers can define protocol event capture filters without raw event-ring
  API usage.
- Developers can attach or import a strategy package against captured protocol
  events.
- A simulated liquidation candidate can be built and rejected/approved by risk.
- A production executor exists behind explicit opt-in and is covered by fake
  transport tests.
- The engine can route one Execution Events stream through multiple protocol
  adapters, strategies, and executors.
- Auxiliary collectors can enrich strategies without replacing the Execution
  Events source of truth for on-chain activity.
- Replay and live modes share event-time semantics and routing.
- Every production decision emits audit records.
- Docs explain how to stay in replay, dry-run, simulation, and production modes.

## 19. Open Questions

1. Which Monad lending protocol should be the first liquidation adapter?
2. Which Monad DEX pool family should be the first exact-math adapter?
3. Which Monad EVM simulation crate/version should be pinned?
4. Should async traits replace V1 sync strategy traits, or should V2 add an
   async engine wrapper?
5. What is the minimum safe production executor: public RPC only, relay, or
   bundle path?
6. How should commit-state rollback work for protocol stores?
7. What persistent state backend is worth adding first?
8. Which metrics backend should be first: Prometheus or OpenTelemetry?
9. Should V2 publish crates or remain Git-only until production executor APIs
   stabilize?
