use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use monad_mev_core::{
    CommitState, Error, EventEnvelope, EventKind, EventMeta, EventSourceKind, FlowTags,
    PayloadExpired, PayloadMode, Result, StreamItem, B256,
};
use serde::{Deserialize, Serialize};

use crate::{validate_readable_path, ExecEventSource, SourceInfo, EXPECTED_EXEC_CONTENT_TYPE};

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const RING_MAGIC: &[u8; 6] = b"RING01";
const EVENT_RING_HEADER_LEN: usize = 192;
const EVENT_DESCRIPTOR_LEN: usize = 64;
const CONTENT_TYPE_NONE: u16 = 0;
const CONTENT_TYPE_TEST: u16 = 1;
const CONTENT_TYPE_EXEC: u16 = 2;

/// Default maximum decompressed snapshot size.
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: usize = 512 * 1024 * 1024;

/// Options used when opening a snapshot source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotOpenOptions {
    /// Maximum decompressed bytes accepted.
    pub max_decompressed_bytes: usize,
    /// Payload mode used by snapshot descriptors.
    pub payload_mode: PayloadMode,
}

impl Default for SnapshotOpenOptions {
    fn default() -> Self {
        Self {
            max_decompressed_bytes: DEFAULT_MAX_DECOMPRESSED_BYTES,
            payload_mode: PayloadMode::Owned,
        }
    }
}

/// Path-backed Monad event-ring snapshot source.
#[derive(Clone, Debug)]
pub struct SnapshotSource {
    info: SourceInfo,
    summary: SnapshotSummary,
    payload_offset: usize,
    payload_buf_size: usize,
    data: Vec<u8>,
    descriptors: Vec<DescriptorRecord>,
}

impl SnapshotSource {
    /// Opens a snapshot source with default options.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is unreadable, the file is not zstd-compressed, the
    /// decompressed bytes do not look like an event ring, or metadata is internally inconsistent.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::open_with_options(path, SnapshotOpenOptions::default())
    }

    /// Opens a snapshot source with explicit options.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, decompression, or event-ring parsing fails.
    pub fn open_with_options(
        path: impl Into<PathBuf>,
        options: SnapshotOpenOptions,
    ) -> Result<Self> {
        if options.payload_mode != PayloadMode::Owned {
            return Err(Error::Message(
                "snapshot source currently supports only owned payload mode".to_owned(),
            ));
        }

        let path = path.into();
        validate_readable_path(&path)?;

        let compressed_size = std::fs::metadata(&path)
            .map_err(|err| {
                Error::Message(format!(
                    "source path is not readable {}: {err}",
                    path.display()
                ))
            })?
            .len();

        let data = decompress_snapshot_file(&path, options.max_decompressed_bytes)?;
        let parsed = ParsedSnapshot::parse(&data, compressed_size)?;
        let info = SourceInfo::path(EventSourceKind::Snapshot, path)
            .with_content_type(parsed.content_type_name())
            .with_schema_hash(parsed.schema_hash);
        let descriptors = DescriptorRecord::scan(&data, &parsed)?;
        let summary = SnapshotSummary {
            compressed_size,
            decompressed_size: usize_to_u64(data.len(), "decompressed snapshot size")?,
            descriptor_capacity: usize_to_u64(parsed.descriptor_capacity, "descriptor capacity")?,
            payload_buf_size: usize_to_u64(parsed.payload_buf_size, "payload buffer size")?,
            context_area_size: usize_to_u64(parsed.context_area_size, "context area size")?,
            last_seqno: parsed.last_seqno,
            next_payload_byte: parsed.next_payload_byte,
            buffer_window_start: parsed.buffer_window_start,
            first_available_seqno: descriptors.first().map(|descriptor| descriptor.seqno),
            last_available_seqno: descriptors.last().map(|descriptor| descriptor.seqno),
            events_available: usize_to_u64(descriptors.len(), "available descriptor count")?,
        };
        let payload_offset = parsed.payload_offset;
        let payload_buf_size = parsed.payload_buf_size;

        Ok(Self {
            info,
            summary,
            payload_offset,
            payload_buf_size,
            data,
            descriptors,
        })
    }

    /// Returns source metadata.
    #[must_use]
    pub const fn info(&self) -> &SourceInfo {
        &self.info
    }

    /// Returns snapshot summary metadata.
    #[must_use]
    pub const fn summary(&self) -> &SnapshotSummary {
        &self.summary
    }

    /// Creates a descriptor reader positioned at the first available descriptor.
    #[must_use]
    pub fn reader(&self) -> SnapshotReader<'_> {
        SnapshotReader {
            source: self,
            cursor: 0,
            expected_seqno: self.summary.first_available_seqno,
            ended: false,
        }
    }
}

impl ExecEventSource for SnapshotSource {
    fn source_info(&self) -> &SourceInfo {
        &self.info
    }
}

/// Summary information extracted from a snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSummary {
    /// Compressed file size in bytes.
    pub compressed_size: u64,
    /// Decompressed event-ring size in bytes.
    pub decompressed_size: u64,
    /// Descriptor ring capacity.
    pub descriptor_capacity: u64,
    /// Payload buffer size.
    pub payload_buf_size: u64,
    /// Context area size.
    pub context_area_size: u64,
    /// Last sequence number allocated by the writer.
    pub last_seqno: u64,
    /// Next payload byte recorded by the writer.
    pub next_payload_byte: u64,
    /// Payload buffer window start.
    pub buffer_window_start: u64,
    /// First descriptor sequence number found in the snapshot.
    pub first_available_seqno: Option<u64>,
    /// Last descriptor sequence number found in the snapshot.
    pub last_available_seqno: Option<u64>,
    /// Number of descriptors found in the snapshot.
    pub events_available: u64,
}

/// Owned snapshot descriptor with copied payload bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDescriptor {
    /// Descriptor sequence number.
    pub seqno: u64,
    /// Source event type.
    pub event_type: u16,
    /// Payload size in bytes.
    pub payload_size: u32,
    /// Source-provided record timestamp in epoch nanoseconds.
    pub record_epoch_nanos: u64,
    /// Unwrapped payload buffer offset.
    pub payload_buf_offset: u64,
    /// Content-specific descriptor extension values.
    pub content_ext: [u64; 4],
    /// Owned payload bytes.
    pub payload: Vec<u8>,
}

/// Reader over snapshot descriptors.
#[derive(Debug)]
pub struct SnapshotReader<'snapshot> {
    source: &'snapshot SnapshotSource,
    cursor: usize,
    expected_seqno: Option<u64>,
    ended: bool,
}

impl SnapshotReader<'_> {
    /// Returns the next descriptor stream item.
    #[must_use]
    pub fn next_item(&mut self) -> StreamItem<SnapshotDescriptor> {
        let Some(record) = self.source.descriptors.get(self.cursor) else {
            if self.ended {
                return StreamItem::SourceEnded;
            }

            self.ended = true;
            return StreamItem::SourceEnded;
        };

        if let Some(expected) = self.expected_seqno {
            if record.seqno != expected {
                self.expected_seqno = Some(record.seqno);
                return StreamItem::Gap(monad_mev_core::GapEvent::new(
                    expected,
                    record.seqno,
                    EventSourceKind::Snapshot,
                ));
            }
        }

        self.cursor += 1;
        self.expected_seqno = record.seqno.checked_add(1);

        let Some(payload) = copy_payload(
            &self.source.data,
            self.source.payload_offset,
            self.source.payload_buf_size,
            self.source.summary.buffer_window_start,
            self.source.summary.next_payload_byte,
            record,
        ) else {
            return StreamItem::PayloadExpired(PayloadExpired {
                seqno: record.seqno,
                event_kind: Some(EventKind::Unknown(record.event_type)),
                source: EventSourceKind::Snapshot,
            });
        };

        let descriptor = SnapshotDescriptor {
            seqno: record.seqno,
            event_type: record.event_type,
            payload_size: record.payload_size,
            record_epoch_nanos: record.record_epoch_nanos,
            payload_buf_offset: record.payload_buf_offset,
            content_ext: record.content_ext,
            payload,
        };
        let envelope = EventEnvelope::new(
            descriptor,
            EventMeta {
                seqno: record.seqno,
                record_epoch_nanos: record.record_epoch_nanos,
                event_kind: EventKind::Unknown(record.event_type),
                source: EventSourceKind::Snapshot,
                block: None,
                txn: None,
                flow: FlowTags {
                    block_seqno: Some(record.content_ext[0]).filter(|value| *value != 0),
                    txn_id: Some(record.content_ext[1]).filter(|value| *value != 0),
                    account_index: Some(record.content_ext[2]).filter(|value| *value != 0),
                },
                commit_state: CommitState::Unknown,
                schema_hash: self.source.info.schema_hash,
            },
        );

        StreamItem::Event(envelope)
    }
}

#[derive(Clone, Debug)]
struct ParsedSnapshot {
    content_type: u16,
    schema_hash: B256,
    descriptor_capacity: usize,
    payload_buf_size: usize,
    context_area_size: usize,
    last_seqno: u64,
    next_payload_byte: u64,
    buffer_window_start: u64,
    descriptor_offset: usize,
    payload_offset: usize,
}

impl ParsedSnapshot {
    fn parse(data: &[u8], compressed_size: u64) -> Result<Self> {
        if data.len() < EVENT_RING_HEADER_LEN {
            return Err(Error::Message(format!(
                "snapshot decompressed to {} bytes, smaller than event ring header; compressed size {compressed_size}",
                data.len()
            )));
        }

        if &data[0..RING_MAGIC.len()] != RING_MAGIC {
            return Err(Error::Message(
                "snapshot is not an event ring: missing RING01 header".to_owned(),
            ));
        }

        let content_type = read_u16(data, 6)?;
        let schema_hash = B256::from(read_array_32(data, 8)?);
        let descriptor_capacity = read_usize(data, 40)?;
        let payload_buf_size = read_usize(data, 48)?;
        let context_area_size = read_usize(data, 56)?;
        let last_seqno = read_u64(data, 64)?;
        let next_payload_byte = read_u64(data, 72)?;
        let buffer_window_start = read_u64(data, 128)?;

        if descriptor_capacity == 0 {
            return Err(Error::Message(
                "snapshot event ring has zero descriptor capacity".to_owned(),
            ));
        }
        if payload_buf_size == 0 {
            return Err(Error::Message(
                "snapshot event ring has zero payload buffer size".to_owned(),
            ));
        }
        if !payload_buf_size.is_power_of_two() {
            return Err(Error::Message(format!(
                "snapshot event ring payload buffer size must be a power of two, got {payload_buf_size}"
            )));
        }

        let descriptor_bytes = descriptor_capacity
            .checked_mul(EVENT_DESCRIPTOR_LEN)
            .ok_or_else(|| {
                Error::Message("snapshot descriptor section overflows usize".to_owned())
            })?;
        let descriptor_offset = EVENT_RING_HEADER_LEN;
        let payload_offset = descriptor_offset
            .checked_add(descriptor_bytes)
            .ok_or_else(|| {
                Error::Message("snapshot payload section offset overflows usize".to_owned())
            })?;
        let context_offset = payload_offset
            .checked_add(payload_buf_size)
            .ok_or_else(|| {
                Error::Message("snapshot context section offset overflows usize".to_owned())
            })?;
        let expected_len = context_offset
            .checked_add(context_area_size)
            .ok_or_else(|| {
                Error::Message("snapshot total storage size overflows usize".to_owned())
            })?;

        if data.len() < expected_len {
            return Err(Error::Message(format!(
                "snapshot is truncated after decompression: expected at least {expected_len} bytes, got {}",
                data.len()
            )));
        }

        Ok(Self {
            content_type,
            schema_hash,
            descriptor_capacity,
            payload_buf_size,
            context_area_size,
            last_seqno,
            next_payload_byte,
            buffer_window_start,
            descriptor_offset,
            payload_offset,
        })
    }

    fn content_type_name(&self) -> String {
        match self.content_type {
            CONTENT_TYPE_NONE => "none".to_owned(),
            CONTENT_TYPE_TEST => "test".to_owned(),
            CONTENT_TYPE_EXEC => EXPECTED_EXEC_CONTENT_TYPE.to_owned(),
            unknown => format!("unknown({unknown})"),
        }
    }
}

#[derive(Clone, Debug)]
struct DescriptorRecord {
    seqno: u64,
    event_type: u16,
    payload_size: u32,
    record_epoch_nanos: u64,
    payload_buf_offset: u64,
    content_ext: [u64; 4],
}

impl DescriptorRecord {
    fn scan(data: &[u8], parsed: &ParsedSnapshot) -> Result<Vec<Self>> {
        let mut descriptors = Vec::new();

        for index in 0..parsed.descriptor_capacity {
            let offset = parsed.descriptor_offset + index * EVENT_DESCRIPTOR_LEN;
            let seqno = read_u64(data, offset)?;

            if seqno == 0 {
                continue;
            }

            descriptors.push(Self {
                seqno,
                event_type: read_u16(data, offset + 8)?,
                payload_size: read_u32(data, offset + 12)?,
                record_epoch_nanos: read_u64(data, offset + 16)?,
                payload_buf_offset: read_u64(data, offset + 24)?,
                content_ext: [
                    read_u64(data, offset + 32)?,
                    read_u64(data, offset + 40)?,
                    read_u64(data, offset + 48)?,
                    read_u64(data, offset + 56)?,
                ],
            });
        }

        descriptors.sort_by_key(|descriptor| descriptor.seqno);

        Ok(descriptors)
    }
}

fn decompress_snapshot_file(path: &Path, max_decompressed_bytes: usize) -> Result<Vec<u8>> {
    let mut file = File::open(path).map_err(|err| {
        Error::Message(format!(
            "source path is not readable {}: {err}",
            path.display()
        ))
    })?;
    let mut magic = [0_u8; ZSTD_MAGIC.len()];
    file.read_exact(&mut magic).map_err(|err| {
        Error::Message(format!(
            "snapshot is not a readable zstd file {}: {err}",
            path.display()
        ))
    })?;

    if magic != ZSTD_MAGIC {
        return Err(Error::Message(format!(
            "snapshot is not a zstd frame: {}",
            path.display()
        )));
    }

    let file = File::open(path).map_err(|err| {
        Error::Message(format!(
            "source path is not readable {}: {err}",
            path.display()
        ))
    })?;
    let mut decoder = zstd::stream::read::Decoder::new(file).map_err(|err| {
        Error::Message(format!(
            "snapshot decompression failed for {}: {err}",
            path.display()
        ))
    })?;
    let mut data = Vec::new();
    let max_bytes = max_decompressed_bytes as u64;
    let read_result = decoder
        .by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut data);

    if data.len() > max_decompressed_bytes {
        return Err(Error::Message(format!(
            "snapshot decompressed size exceeds limit of {max_decompressed_bytes} bytes: {}",
            path.display()
        )));
    }

    read_result.map_err(|err| {
        Error::Message(format!(
            "snapshot decompression failed for {}: {err}",
            path.display()
        ))
    })?;

    Ok(data)
}

fn copy_payload(
    data: &[u8],
    payload_offset: usize,
    payload_buf_size: usize,
    buffer_window_start: u64,
    next_payload_byte: u64,
    descriptor: &DescriptorRecord,
) -> Option<Vec<u8>> {
    if descriptor.payload_buf_offset < buffer_window_start {
        return None;
    }
    if descriptor
        .payload_buf_offset
        .checked_add(u64::from(descriptor.payload_size))?
        > next_payload_byte
    {
        return None;
    }

    let payload_size = usize::try_from(descriptor.payload_size).ok()?;
    if payload_size == 0 {
        return Some(Vec::new());
    }
    if payload_size > payload_buf_size || payload_buf_size == 0 {
        return None;
    }

    let absolute_payload_offset = usize::try_from(descriptor.payload_buf_offset).ok()?;
    let start = absolute_payload_offset & (payload_buf_size - 1);
    let payload_end = payload_offset.checked_add(payload_buf_size)?;

    if payload_end > data.len() {
        return None;
    }

    let payload_buf = &data[payload_offset..payload_end];

    if start + payload_size <= payload_buf.len() {
        Some(payload_buf[start..start + payload_size].to_vec())
    } else {
        let first = &payload_buf[start..];
        let remaining = payload_size - first.len();

        if remaining > start {
            return None;
        }

        let mut payload = Vec::with_capacity(payload_size);
        payload.extend_from_slice(first);
        payload.extend_from_slice(&payload_buf[..remaining]);
        Some(payload)
    }
}

fn read_array_32(data: &[u8], offset: usize) -> Result<[u8; 32]> {
    let bytes = data
        .get(offset..offset + 32)
        .ok_or_else(|| Error::Message(format!("snapshot missing 32 bytes at offset {offset}")))?;

    bytes
        .try_into()
        .map_err(|_| Error::Message(format!("snapshot invalid 32-byte array at offset {offset}")))
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| Error::Message(format!("snapshot missing u16 at offset {offset}")))?;

    Ok(u16::from_le_bytes(
        bytes
            .try_into()
            .expect("slice length was checked before conversion"),
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| Error::Message(format!("snapshot missing u32 at offset {offset}")))?;

    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .expect("slice length was checked before conversion"),
    ))
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or_else(|| Error::Message(format!("snapshot missing u64 at offset {offset}")))?;

    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .expect("slice length was checked before conversion"),
    ))
}

fn read_usize(data: &[u8], offset: usize) -> Result<usize> {
    let value = read_u64(data, offset)?;

    usize::try_from(value).map_err(|_| {
        Error::Message(format!(
            "snapshot usize field at offset {offset} exceeds target usize"
        ))
    })
}

fn usize_to_u64(value: usize, label: &str) -> Result<u64> {
    u64::try_from(value).map_err(|_| Error::Message(format!("snapshot {label} exceeds target u64")))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_path(extension: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "monad-mev-rs-snapshot-{}-{nanos}.{extension}",
            std::process::id()
        ))
    }

    fn write_snapshot(path: &Path, ring: &[u8]) {
        let compressed = zstd::stream::encode_all(ring, 0).expect("ring should compress");

        fs::write(path, compressed).expect("snapshot should write");
    }

    fn fake_ring(schema_hash: B256, descriptors: &[FakeDescriptor]) -> Vec<u8> {
        let descriptor_capacity = 4_usize;
        let payload_buf_size = 64_usize;
        let context_area_size = 0_usize;
        let mut ring = vec![
            0_u8;
            EVENT_RING_HEADER_LEN
                + descriptor_capacity * EVENT_DESCRIPTOR_LEN
                + payload_buf_size
                + context_area_size
        ];

        ring[0..6].copy_from_slice(RING_MAGIC);
        write_u16(&mut ring, 6, CONTENT_TYPE_EXEC);
        ring[8..40].copy_from_slice(schema_hash.as_slice());
        write_u64(&mut ring, 40, descriptor_capacity as u64);
        write_u64(&mut ring, 48, payload_buf_size as u64);
        write_u64(&mut ring, 56, context_area_size as u64);
        write_u64(
            &mut ring,
            64,
            descriptors
                .iter()
                .map(|descriptor| descriptor.seqno)
                .max()
                .unwrap_or_default(),
        );
        write_u64(&mut ring, 72, 32);
        write_u64(&mut ring, 128, 0);

        let payload_offset = EVENT_RING_HEADER_LEN + descriptor_capacity * EVENT_DESCRIPTOR_LEN;

        for descriptor in descriptors {
            let index = usize::try_from(
                descriptor
                    .seqno
                    .checked_sub(1)
                    .expect("fake descriptor seqno should be one-indexed"),
            )
            .expect("fake descriptor seqno should fit usize")
                & (descriptor_capacity - 1);
            let offset = EVENT_RING_HEADER_LEN + index * EVENT_DESCRIPTOR_LEN;
            let payload_start = payload_offset
                + usize::try_from(descriptor.payload_offset)
                    .expect("fake descriptor payload offset should fit usize");

            ring[payload_start..payload_start + descriptor.payload.len()]
                .copy_from_slice(&descriptor.payload);

            write_u64(&mut ring, offset, descriptor.seqno);
            write_u16(&mut ring, offset + 8, descriptor.event_type);
            write_u32(
                &mut ring,
                offset + 12,
                u32::try_from(descriptor.payload.len())
                    .expect("fake descriptor payload length should fit u32"),
            );
            write_u64(&mut ring, offset + 16, descriptor.record_epoch_nanos);
            write_u64(&mut ring, offset + 24, descriptor.payload_offset);
            write_u64(&mut ring, offset + 32, descriptor.content_ext[0]);
            write_u64(&mut ring, offset + 40, descriptor.content_ext[1]);
            write_u64(&mut ring, offset + 48, descriptor.content_ext[2]);
            write_u64(&mut ring, offset + 56, descriptor.content_ext[3]);
        }

        ring
    }

    #[derive(Debug)]
    struct FakeDescriptor {
        seqno: u64,
        event_type: u16,
        record_epoch_nanos: u64,
        payload_offset: u64,
        payload: Vec<u8>,
        content_ext: [u64; 4],
    }

    fn write_u16(data: &mut [u8], offset: usize, value: u16) {
        data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(data: &mut [u8], offset: usize, value: u32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(data: &mut [u8], offset: usize, value: u64) {
        data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn snapshot_open_reads_metadata_and_descriptors() {
        let path = temp_path("zst");
        let schema_hash = B256::from([9_u8; 32]);
        let ring = fake_ring(
            schema_hash,
            &[FakeDescriptor {
                seqno: 1,
                event_type: 7,
                record_epoch_nanos: 100,
                payload_offset: 0,
                payload: vec![1, 2, 3],
                content_ext: [11, 12, 13, 0],
            }],
        );
        write_snapshot(&path, &ring);

        let source = SnapshotSource::open(&path).expect("snapshot should open");

        assert_eq!(source.info().kind, EventSourceKind::Snapshot);
        assert_eq!(
            source.info().content_type.as_deref(),
            Some(EXPECTED_EXEC_CONTENT_TYPE)
        );
        assert_eq!(source.info().schema_hash, Some(schema_hash));
        assert_eq!(source.summary().events_available, 1);

        let mut reader = source.reader();
        let item = reader.next_item();

        let StreamItem::Event(envelope) = item else {
            panic!("expected event item");
        };

        assert_eq!(envelope.seqno(), 1);
        assert_eq!(envelope.payload.event_type, 7);
        assert_eq!(envelope.payload.payload, vec![1, 2, 3]);
        assert_eq!(envelope.meta.flow.block_seqno, Some(11));
        assert_eq!(envelope.meta.flow.txn_id, Some(12));
        assert_eq!(envelope.meta.flow.account_index, Some(13));
        assert!(reader.next_item().is_source_end());

        fs::remove_file(path).ok();
    }

    #[test]
    fn snapshot_open_missing_file_errors() {
        let path = temp_path("zst");

        let error = SnapshotSource::open(&path).expect_err("missing file should fail");

        assert!(error.to_string().contains("source path is not readable"));
    }

    #[test]
    fn snapshot_open_rejects_corrupt_zstd() {
        let path = temp_path("zst");
        fs::write(&path, b"not a zstd frame").expect("corrupt file should write");

        let error = SnapshotSource::open(&path).expect_err("corrupt zstd should fail");

        assert!(error.to_string().contains("not a zstd frame"));

        fs::remove_file(path).ok();
    }

    #[test]
    fn unsupported_extension_is_neutral_for_valid_snapshot() {
        let path = temp_path("bin");
        let ring = fake_ring(B256::from([1_u8; 32]), &[]);
        write_snapshot(&path, &ring);

        let source = SnapshotSource::open(&path).expect("extension should not matter");

        assert_eq!(source.summary().events_available, 0);

        fs::remove_file(path).ok();
    }

    #[test]
    fn snapshot_reader_emits_gap_before_next_descriptor() {
        let path = temp_path("zst");
        let ring = fake_ring(
            B256::from([1_u8; 32]),
            &[
                FakeDescriptor {
                    seqno: 1,
                    event_type: 7,
                    record_epoch_nanos: 100,
                    payload_offset: 0,
                    payload: vec![1],
                    content_ext: [0; 4],
                },
                FakeDescriptor {
                    seqno: 3,
                    event_type: 8,
                    record_epoch_nanos: 200,
                    payload_offset: 16,
                    payload: vec![2],
                    content_ext: [0; 4],
                },
            ],
        );
        write_snapshot(&path, &ring);

        let source = SnapshotSource::open(&path).expect("snapshot should open");
        let mut reader = source.reader();

        assert!(matches!(reader.next_item(), StreamItem::Event(_)));

        let gap = reader.next_item();
        assert!(matches!(gap, StreamItem::Gap(_)));

        let event = reader.next_item();
        assert!(matches!(event, StreamItem::Event(_)));

        fs::remove_file(path).ok();
    }
}
