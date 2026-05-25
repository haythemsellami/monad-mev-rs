# CLI

Global options:

```text
--json
--no-color
--log-level <level>
```

Exit codes:

- `0`: success.
- `1`: runtime error such as a missing fixture path.
- `2`: invalid CLI usage.

Commands:

```bash
monad-mev doctor
monad-mev inspect --fixture raw-events
monad-mev inspect monad-exec-events --live --duration 10s --summary
monad-mev decode --fixture defi-decoded --defi
monad-mev replay --fixture raw-events --report /tmp/report.json --events-jsonl /tmp/events.jsonl
monad-mev lifecycle --json
monad-mev strategy new /tmp/monad-mev-strategy
```

Use `--json` before the command for structured command output and structured
errors:

```bash
monad-mev --json doctor
monad-mev --json replay --fixture raw-events
monad-mev --json lifecycle
```
