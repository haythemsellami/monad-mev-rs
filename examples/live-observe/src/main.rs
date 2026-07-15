use std::{path::Path, thread, time::Duration};

use monad_mev_core::{Error, Result};
use monad_mev_engine::{ContinuousEngineConfig, ContinuousEngineRunner, Engine, ShutdownHandle};
use monad_mev_events::{
    host_supports_live_event_ring, live_availability_reason, LiveConfig, LiveExecutionEventStream,
};

const DEFAULT_DURATION_MILLIS: u64 = 10_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    if !host_supports_live_event_ring() {
        println!("{}", live_availability_reason());
        return Ok(());
    }

    let ring =
        std::env::var("MONAD_MEV_EVENT_RING").unwrap_or_else(|_| "monad-exec-events".to_owned());
    let duration_millis = std::env::var("MONAD_MEV_DURATION_MILLIS").map_or(
        Ok(DEFAULT_DURATION_MILLIS),
        |value| {
            value.parse::<u64>().map_err(|error| {
                Error::Message(format!("invalid MONAD_MEV_DURATION_MILLIS: {error}"))
            })
        },
    )?;
    let mut source = LiveExecutionEventStream::open(live_config(&ring))?;
    let shutdown = ShutdownHandle::default();
    let timer_shutdown = shutdown.clone();
    let _timer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(duration_millis));
        timer_shutdown.request_shutdown();
    });
    let mut runner = ContinuousEngineRunner::new(Engine::new(), ContinuousEngineConfig::default());
    let completed = runner.run_until_stopped(&mut source, &shutdown)?;
    let output = serde_json::to_string_pretty(&completed)
        .map_err(|error| Error::Message(format!("failed to serialize live report: {error}")))?;
    println!("{output}");
    Ok(())
}

fn live_config(ring: &str) -> LiveConfig {
    if Path::new(ring).components().count() > 1 {
        LiveConfig::path(ring)
    } else {
        LiveConfig::named(ring)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_observe_accepts_ring_names_and_paths() {
        let named = live_config("monad-exec-events");
        let path = live_config("/tmp/monad-exec-events");

        assert_eq!(named.ring_name.as_deref(), Some("monad-exec-events"));
        assert_eq!(
            path.ring_path.as_deref(),
            Some(Path::new("/tmp/monad-exec-events"))
        );
    }
}
