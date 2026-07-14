use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

use monad_event_ring::{
    DecodedEventRing, EventDecoder, EventDescriptorInfo, EventNextResult, EventPayloadResult,
    EventRingPath,
};
use monad_exec_events::{
    ExecEventDecoder, ExecEventDescriptorExt, ExecEventReaderExt, ExecEventRing, ExecEventType,
};
use monad_mev_core::{
    CommitState, Error, EventEnvelope, EventKind, EventMeta, EventSourceKind, FlowTags, GapEvent,
    PayloadExpired, Result, StreamItem, B256,
};

use crate::{raw_event_from_snapshot, RawExecEvent, SnapshotDescriptor};

#[derive(Debug)]
pub(crate) struct SdkLiveReader {
    receiver: Receiver<LiveMessage>,
    stop: Arc<AtomicBool>,
}

enum LiveMessage {
    Item(StreamItem<RawExecEvent>),
    Error(String),
}

impl SdkLiveReader {
    pub(crate) fn open(path: PathBuf, poll_interval_millis: u64, capacity: usize) -> Result<Self> {
        let (sender, receiver) = sync_channel(capacity);
        let (ready_sender, ready_receiver) = sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        thread::Builder::new()
            .name("monad-mev-live-ring".to_owned())
            .spawn(move || {
                run_reader(
                    path,
                    poll_interval_millis,
                    sender,
                    ready_sender,
                    &thread_stop,
                );
            })
            .map_err(|error| {
                Error::Message(format!("failed to spawn live ring reader: {error}"))
            })?;

        ready_receiver
            .recv()
            .map_err(|_| Error::Message("live ring reader exited during startup".to_owned()))?
            .map_err(Error::Message)?;

        Ok(Self { receiver, stop })
    }

    pub(crate) fn try_next(&self) -> Result<Option<StreamItem<RawExecEvent>>> {
        match self.receiver.try_recv() {
            Ok(LiveMessage::Item(item)) => Ok(Some(item)),
            Ok(LiveMessage::Error(error)) => Err(Error::Message(error)),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(Error::Message(
                "live ring reader disconnected unexpectedly".to_owned(),
            )),
        }
    }
}

impl Drop for SdkLiveReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

pub(crate) fn schema_hash() -> B256 {
    B256::from(*ExecEventDecoder::ring_schema_hash())
}

fn run_reader(
    path: PathBuf,
    poll_interval_millis: u64,
    sender: SyncSender<LiveMessage>,
    ready_sender: SyncSender<std::result::Result<(), String>>,
    stop: &AtomicBool,
) {
    let result = open_ring(&path);
    let Ok(ring) = result else {
        let _ = ready_sender.send(result.map(|_| ()));
        return;
    };
    if ready_sender.send(Ok(())).is_err() {
        return;
    }

    let mut reader = ring.create_reader();
    reader.consensus_prev(Some(ExecEventType::BlockStart));
    let mut expected_seqno = None;
    let sleep = Duration::from_millis(poll_interval_millis.max(1));

    while !stop.load(Ordering::Acquire) {
        match reader.next_descriptor() {
            EventNextResult::NotReady => thread::sleep(sleep),
            EventNextResult::Gap => {}
            EventNextResult::Ready(descriptor) => {
                let info = descriptor.info();
                if let Some(expected) = expected_seqno {
                    if info.seqno != expected
                        && sender
                            .send(LiveMessage::Item(StreamItem::Gap(GapEvent::new(
                                expected,
                                info.seqno,
                                EventSourceKind::Live,
                            ))))
                            .is_err()
                    {
                        return;
                    }
                }
                expected_seqno = info.seqno.checked_add(1);

                let block_number = descriptor.get_block_number();
                let txn_idx = info.flow_info.txn_idx;
                let seqno = info.seqno;
                let event_type = info.event_type;
                match descriptor.try_filter_map_raw(copy_descriptor) {
                    EventPayloadResult::Expired => {
                        let item = StreamItem::PayloadExpired(PayloadExpired {
                            seqno,
                            event_kind: Some(EventKind::Unknown(event_type)),
                            source: EventSourceKind::Live,
                        });
                        if sender.send(LiveMessage::Item(item)).is_err() {
                            return;
                        }
                    }
                    EventPayloadResult::Ready(Some(snapshot)) => {
                        let mut envelope = EventEnvelope::new(
                            snapshot,
                            EventMeta {
                                seqno,
                                record_epoch_nanos: info.record_epoch_nanos,
                                event_kind: EventKind::Unknown(event_type),
                                source: EventSourceKind::Live,
                                block: block_number,
                                txn: txn_idx.and_then(|index| u32::try_from(index).ok()),
                                flow: FlowTags {
                                    block_seqno: info.flow_info.block_seqno,
                                    txn_id: txn_idx
                                        .and_then(|index| u64::try_from(index).ok())
                                        .and_then(|index| index.checked_add(1))
                                        .unwrap_or(0),
                                    account_index: info.flow_info.account_idx,
                                    reserved: 0,
                                },
                                commit_state: CommitState::Unknown,
                                schema_hash: Some(schema_hash()),
                            },
                        );
                        envelope.payload.record_epoch_nanos = info.record_epoch_nanos;
                        let item = StreamItem::Event(raw_event_from_snapshot(envelope));
                        if sender.send(LiveMessage::Item(item)).is_err() {
                            return;
                        }
                    }
                    EventPayloadResult::Ready(None) => {
                        let _ = sender.send(LiveMessage::Error(
                            "live descriptor could not be copied".to_owned(),
                        ));
                        return;
                    }
                }
            }
        }
    }
}

fn open_ring(path: &PathBuf) -> std::result::Result<ExecEventRing, String> {
    let path = EventRingPath::resolve(path)?;
    ExecEventRing::new(path)
}

fn copy_descriptor(
    info: EventDescriptorInfo<ExecEventDecoder>,
    payload: &[u8],
) -> Option<SnapshotDescriptor> {
    let txn_id = info
        .flow_info
        .txn_idx
        .and_then(|index| u64::try_from(index).ok())
        .and_then(|index| index.checked_add(1))
        .unwrap_or(0);
    Some(SnapshotDescriptor {
        seqno: info.seqno,
        event_type: info.event_type,
        payload_size: u32::try_from(payload.len()).ok()?,
        record_epoch_nanos: info.record_epoch_nanos,
        payload_buf_offset: 0,
        content_ext: [
            info.flow_info.block_seqno,
            txn_id,
            info.flow_info.account_idx,
            0,
        ],
        payload: payload.to_vec(),
    })
}
