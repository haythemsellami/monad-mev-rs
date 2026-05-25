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
