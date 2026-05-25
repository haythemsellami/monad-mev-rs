# v0.2 Strategy Packages

Strategy packages describe their metadata, capabilities, and subscriptions with
`PackageManifest`. A package can provide capture filters, event adapters, state
adapters, opportunity detectors, transaction candidate builders, or strategy
logic.

Core crates do not define protocol math. Third-party packages own protocol
models and expose them through the generic adapter interfaces.
