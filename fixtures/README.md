# Fixtures

This directory will hold small deterministic fixtures for unit, integration, and golden tests.

V1 should prefer normalized JSON fixtures in normal CI so tests do not require large binary snapshot files. Real `.zst` snapshot files should stay local unless a tiny redistributable snapshot is explicitly approved.

Local snapshot files belong under ignored directories such as `data/` or `snapshots/`.
