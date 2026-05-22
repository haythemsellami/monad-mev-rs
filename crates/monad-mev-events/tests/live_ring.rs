#![cfg(feature = "live")]

use monad_mev_events::{ExecEventSource, LiveConfig, LiveEventRingSource};

#[test]
#[ignore = "requires MONAD_MEV_EVENT_RING pointing at a readable Linux execution event ring"]
fn opens_live_ring_from_env() {
    let ring = std::env::var("MONAD_MEV_EVENT_RING")
        .expect("MONAD_MEV_EVENT_RING must name a live event ring or path");
    let config = if ring.contains('/') {
        LiveConfig::path(ring)
    } else {
        LiveConfig::named(ring)
    };

    let source = LiveEventRingSource::open(config).expect("live event ring should open");

    assert_eq!(source.source_info().content_type.as_deref(), Some("exec"));
}
