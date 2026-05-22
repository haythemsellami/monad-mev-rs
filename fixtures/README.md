# Fixture Schema

Fixtures are small deterministic JSON documents used by CI. They avoid large
binary snapshots while keeping replay, decoding, and strategy tests stable.

Each fixture file uses:

```json
{
  "name": "fixture-name",
  "description": "short purpose",
  "events": [
    {
      "seqno": 1,
      "kind": "txn_log",
      "block": 1,
      "txn": 0,
      "address": "0x0000000000000000000000000000000000000000",
      "topics": [],
      "data": "0x"
    }
  ],
  "expected_report": {
    "events_seen": 1,
    "events_decoded": 1,
    "gaps": 0,
    "payload_expired": 0,
    "logs_seen": 1
  }
}
```

Fields beyond `seqno` and `kind` are fixture-specific. Golden JSONL files are
kept under `fixtures/golden/` and should be reviewed manually when changed.
