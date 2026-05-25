# v0.2 Execution

`monad-mev-exec` separates recording, dry-run, and production modes.

Production submission requires explicit config and a risk-approved execution
plan. Tests use fake transports by default, so executor behavior can be
validated without network access or private keys.
