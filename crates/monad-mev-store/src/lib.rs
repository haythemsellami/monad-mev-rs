//! Commit-state-aware state storage for `monad-mev-rs`.

use std::collections::BTreeMap;

use monad_mev_core::{CommitState, Error, EventEnvelope, EventKind, Result};
use serde::{Deserialize, Serialize};

/// Stable key for framework or adapter state.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct StateKey {
    /// Logical namespace, usually the adapter or package name.
    pub namespace: String,
    /// Namespace-local identifier.
    pub id: String,
}

impl StateKey {
    /// Creates a state key.
    #[must_use]
    pub fn new(namespace: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            id: id.into(),
        }
    }
}

/// Event reference attached to a state update or opportunity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceEventRef {
    /// Descriptor sequence number.
    pub seqno: u64,
    /// Framework event kind.
    pub event_kind: EventKind,
    /// Proposed block number when known.
    pub block_number: Option<u64>,
    /// Transaction index when known.
    pub txn_idx: Option<u64>,
    /// Commit state observed for this event.
    pub commit_state: CommitState,
}

impl<T> From<&EventEnvelope<T>> for SourceEventRef {
    fn from(event: &EventEnvelope<T>) -> Self {
        Self {
            seqno: event.meta.seqno,
            event_kind: event.meta.event_kind.clone(),
            block_number: event.block_number(),
            txn_idx: event.txn_idx(),
            commit_state: event.meta.commit_state,
        }
    }
}

/// Monotonic state version produced by the store.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StateVersion {
    /// Monotonic store revision.
    pub revision: u64,
    /// Sequence number that last modified the entry.
    pub last_seqno: u64,
    /// Commit state of the source event that last modified the entry.
    pub commit_state: CommitState,
}

/// A typed state mutation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateUpdate {
    /// Key to update.
    pub key: StateKey,
    /// New value.
    pub value: serde_json::Value,
    /// Source event reference.
    pub source: SourceEventRef,
}

impl StateUpdate {
    /// Creates a state update from an event.
    ///
    /// # Errors
    ///
    /// Returns an error when JSON serialization of `value` fails.
    pub fn from_event<T: Serialize, E>(
        key: StateKey,
        value: T,
        event: &EventEnvelope<E>,
    ) -> Result<Self> {
        let value = serde_json::to_value(value)
            .map_err(|err| Error::Message(format!("failed to serialize state value: {err}")))?;
        Ok(Self {
            key,
            value,
            source: SourceEventRef::from(event),
        })
    }
}

/// Current value stored for one key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateEntry {
    /// State key.
    pub key: StateKey,
    /// State value.
    pub value: serde_json::Value,
    /// Version assigned by the store.
    pub version: StateVersion,
    /// Source events that contributed to the current value.
    pub sources: Vec<SourceEventRef>,
}

/// Historical audit record for a state mutation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateAuditRecord {
    /// Applied update.
    pub update: StateUpdate,
    /// Version assigned after applying the update.
    pub version: StateVersion,
    /// Previous entry, when the key existed.
    pub previous: Option<StateEntry>,
}

/// Immutable state view with commit-state filtering.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateView {
    entries: BTreeMap<StateKey, StateEntry>,
}

impl StateView {
    /// Returns an entry by key.
    #[must_use]
    pub fn get(&self, key: &StateKey) -> Option<&StateEntry> {
        self.entries.get(key)
    }

    /// Returns entries in deterministic key order.
    pub fn entries(&self) -> impl Iterator<Item = (&StateKey, &StateEntry)> {
        self.entries.iter()
    }

    /// Returns the number of visible entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when no entries are visible.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// In-memory deterministic state store.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InMemoryStateStore {
    entries: BTreeMap<StateKey, StateEntry>,
    audit: Vec<StateAuditRecord>,
    next_revision: u64,
}

impl InMemoryStateStore {
    /// Applies one state update.
    pub fn apply(&mut self, update: StateUpdate) -> StateVersion {
        self.next_revision += 1;
        let version = StateVersion {
            revision: self.next_revision,
            last_seqno: update.source.seqno,
            commit_state: update.source.commit_state,
        };
        let previous = self.entries.get(&update.key).cloned();
        let mut sources = previous
            .as_ref()
            .map_or_else(Vec::new, |entry| entry.sources.clone());
        sources.push(update.source.clone());
        let entry = StateEntry {
            key: update.key.clone(),
            value: update.value.clone(),
            version,
            sources,
        };
        self.entries.insert(update.key.clone(), entry);
        self.audit.push(StateAuditRecord {
            update,
            version,
            previous,
        });
        version
    }

    /// Applies updates in descriptor order for deterministic state.
    pub fn apply_all(&mut self, mut updates: Vec<StateUpdate>) -> Vec<StateVersion> {
        updates.sort_by_key(|update| update.source.seqno);
        updates
            .into_iter()
            .map(|update| self.apply(update))
            .collect()
    }

    /// Returns the latest entry for a key.
    #[must_use]
    pub fn get(&self, key: &StateKey) -> Option<&StateEntry> {
        self.entries.get(key)
    }

    /// Returns a commit-state-filtered view.
    #[must_use]
    pub fn view(&self, min_commit_state: CommitState) -> StateView {
        let entries = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.version.commit_state.satisfies_min(min_commit_state))
            .map(|(key, entry)| (key.clone(), entry.clone()))
            .collect();
        StateView { entries }
    }

    /// Rolls the store back to the last update at or before `max_seqno`.
    pub fn rollback_to_seqno(&mut self, max_seqno: u64) {
        let retained: Vec<StateAuditRecord> = self
            .audit
            .iter()
            .filter(|record| record.update.source.seqno <= max_seqno)
            .cloned()
            .collect();
        self.entries.clear();
        self.audit.clear();
        self.next_revision = 0;
        for record in retained {
            self.apply(record.update);
        }
    }

    /// Returns deterministic audit records.
    #[must_use]
    pub fn audit(&self) -> &[StateAuditRecord] {
        &self.audit
    }
}

/// Projection from events into state updates.
pub trait Projection<E> {
    /// Projects one event into zero or more updates.
    ///
    /// # Errors
    ///
    /// Returns an error when projection logic fails.
    fn project(&mut self, event: &EventEnvelope<E>) -> Result<Vec<StateUpdate>>;
}

/// Replays events through a projection into an in-memory store.
///
/// # Errors
///
/// Returns projection errors.
pub fn project_events<E>(
    events: impl IntoIterator<Item = EventEnvelope<E>>,
    projection: &mut impl Projection<E>,
) -> Result<InMemoryStateStore> {
    let mut store = InMemoryStateStore::default();
    for event in events {
        store.apply_all(projection.project(&event)?);
    }
    Ok(store)
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{EventMeta, EventSourceKind, FlowTags};

    use super::*;

    fn event(seqno: u64, state: CommitState, payload: &str) -> EventEnvelope<String> {
        EventEnvelope::new(
            payload.to_owned(),
            EventMeta {
                seqno,
                record_epoch_nanos: seqno,
                event_kind: EventKind::TxnLog,
                source: EventSourceKind::Fixture,
                block: None,
                txn: None,
                flow: FlowTags::default(),
                commit_state: state,
                schema_hash: None,
            },
        )
    }

    #[test]
    fn projection_updates_store() {
        struct EchoProjection;

        impl Projection<String> for EchoProjection {
            fn project(&mut self, event: &EventEnvelope<String>) -> Result<Vec<StateUpdate>> {
                StateUpdate::from_event(
                    StateKey::new("fixture", event.payload.clone()),
                    serde_json::json!({ "seen": event.seqno() }),
                    event,
                )
                .map(|update| vec![update])
            }
        }

        let store = project_events(
            vec![event(2, CommitState::Finalized, "a")],
            &mut EchoProjection,
        )
        .expect("projection works");

        let entry = store
            .get(&StateKey::new("fixture", "a"))
            .expect("entry exists");
        assert_eq!(entry.value["seen"], 2);
        assert_eq!(entry.version.revision, 1);
    }

    #[test]
    fn apply_all_orders_by_seqno() {
        let event_two = event(2, CommitState::Finalized, "two");
        let event_one = event(1, CommitState::Finalized, "one");
        let mut store = InMemoryStateStore::default();

        let versions = store.apply_all(vec![
            StateUpdate::from_event(StateKey::new("n", "k"), "two", &event_two).expect("update"),
            StateUpdate::from_event(StateKey::new("n", "k"), "one", &event_one).expect("update"),
        ]);

        assert_eq!(versions[0].last_seqno, 1);
        assert_eq!(versions[1].last_seqno, 2);
        assert_eq!(
            store.get(&StateKey::new("n", "k")).expect("entry").value,
            serde_json::json!("two")
        );
    }

    #[test]
    fn view_filters_by_commit_state() {
        let mut store = InMemoryStateStore::default();
        store.apply(
            StateUpdate::from_event(
                StateKey::new("n", "proposed"),
                true,
                &event(1, CommitState::Proposed, "p"),
            )
            .expect("update"),
        );
        store.apply(
            StateUpdate::from_event(
                StateKey::new("n", "finalized"),
                true,
                &event(2, CommitState::Finalized, "f"),
            )
            .expect("update"),
        );

        let view = store.view(CommitState::Finalized);

        assert!(view.get(&StateKey::new("n", "proposed")).is_none());
        assert!(view.get(&StateKey::new("n", "finalized")).is_some());
    }

    #[test]
    fn rollback_rebuilds_deterministically() {
        let mut store = InMemoryStateStore::default();
        let key = StateKey::new("n", "k");
        store.apply(
            StateUpdate::from_event(key.clone(), 1, &event(1, CommitState::Finalized, "a"))
                .expect("update"),
        );
        store.apply(
            StateUpdate::from_event(key.clone(), 2, &event(2, CommitState::Finalized, "b"))
                .expect("update"),
        );

        store.rollback_to_seqno(1);

        let entry = store.get(&key).expect("entry");
        assert_eq!(entry.value, serde_json::json!(1));
        assert_eq!(entry.version.revision, 1);
        assert_eq!(store.audit().len(), 1);
    }
}
