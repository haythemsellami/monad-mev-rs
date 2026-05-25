# v0.2 Production Runbook

Before production:

- run replay/golden tests
- run simulation tests with fake state providers
- run risk policy tests
- run fake transport executor tests
- enable production execution only in explicit config
- keep private keys outside source control
- monitor ingestion gaps, state freshness, simulation status, risk rejections,
  and submit receipts

Production executors should start in recording or dry-run mode, then move to
explicit production mode only after the strategy package has passed its own
protocol-specific tests.
