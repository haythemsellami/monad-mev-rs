# v0.2 Simulation

`monad-mev-sim` defines transaction candidates, simulation requests/results,
state-read auditing, state providers, and deterministic fake simulation for
tests.

Real Monad EVM backends can implement the same `Simulator` contract without
changing strategy or risk code.
