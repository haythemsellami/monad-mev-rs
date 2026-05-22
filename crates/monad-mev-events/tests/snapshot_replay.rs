use monad_mev_core::StreamItem;
use monad_mev_events::{ExecEventSource, SchemaPolicy, SnapshotSource, EXPECTED_EXEC_CONTENT_TYPE};

#[test]
#[ignore = "requires MONAD_MEV_SNAPSHOT pointing to a real Monad snapshot .zst file"]
fn opens_snapshot_from_env_and_replays_raw_descriptors() {
    let path = std::env::var("MONAD_MEV_SNAPSHOT").expect("set MONAD_MEV_SNAPSHOT");
    let source = SnapshotSource::open(path).expect("snapshot should open");

    assert_eq!(
        source.source_info().content_type.as_deref(),
        Some(EXPECTED_EXEC_CONTENT_TYPE)
    );
    let schema_hash = source
        .source_info()
        .schema_hash
        .expect("snapshot should expose schema hash");
    source
        .validate_source(schema_hash, SchemaPolicy::RequireMatch)
        .expect("snapshot should validate against its discovered schema hash");

    let mut reader = source.reader();
    let mut events_seen = 0_u64;
    let mut gaps_seen = 0_u64;
    let mut expired_payloads = 0_u64;

    loop {
        match reader.next_item() {
            StreamItem::Event(_) => events_seen += 1,
            StreamItem::Gap(_) => gaps_seen += 1,
            StreamItem::PayloadExpired(_) => expired_payloads += 1,
            StreamItem::SchemaMismatch(_) => panic!("schema validation is not part of raw replay"),
            StreamItem::SourceEnded => break,
        }
    }

    assert_eq!(
        events_seen + expired_payloads,
        source.summary().events_available
    );
    assert!(
        gaps_seen <= events_seen,
        "gap count should not exceed available descriptor count"
    );
}
