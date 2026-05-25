# v0.2 Risk

`monad-mev-risk` makes execution decisions explicit. Policies can enforce
successful simulation, gas limits, value thresholds, loss limits, target and
selector allowlists, external-data freshness, simulation freshness, and circuit
breakers.

`ExecutionPlan::build` rejects candidates unless simulation succeeded and risk
approved the request.
