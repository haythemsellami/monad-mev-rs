# v0.2 Engine

The engine runs this lifecycle:

```text
Execution Events stream
-> capture filters
-> event adapters
-> state adapters
-> opportunity detectors
-> recording, simulation, risk, and execution stages
```

The default engine is deterministic and replay-friendly. Production submission
is handled by `monad-mev-exec` and remains explicit.

Finite replay uses `Engine::run`. Continuous sources use
`ContinuousEngineRunner`, which retains state across polls, invokes detectors
at configured event boundaries, and preserves stream-health failures. See
[`continuous-engine.md`](continuous-engine.md).
