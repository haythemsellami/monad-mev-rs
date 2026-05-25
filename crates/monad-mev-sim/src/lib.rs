//! Simulation contracts for `monad-mev-rs`.

use std::collections::BTreeMap;

use monad_mev_core::{Address, Error, Result, B256, U256};
use monad_mev_store::{SourceEventRef, StateVersion};
use serde::{Deserialize, Serialize};

/// Transaction candidate produced before execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransactionCandidate {
    /// Stable candidate ID.
    pub id: String,
    /// Optional recipient. `None` represents contract creation.
    pub to: Option<Address>,
    /// Calldata or initcode.
    pub data: Vec<u8>,
    /// Native value.
    pub value: U256,
    /// Optional gas limit.
    pub gas_limit: Option<u64>,
    /// Strategy-provided value estimate in wei.
    pub value_estimate_wei: i64,
}

impl TransactionCandidate {
    /// Creates a transaction candidate.
    #[must_use]
    pub fn new(id: impl Into<String>, to: Option<Address>, data: Vec<u8>) -> Self {
        Self {
            id: id.into(),
            to,
            data,
            value: U256::ZERO,
            gas_limit: None,
            value_estimate_wei: 0,
        }
    }
}

/// State read requested by simulation.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct StateRead {
    /// Account address.
    pub address: Address,
    /// Storage slot. `None` means account-level metadata.
    pub slot: Option<B256>,
}

/// Value returned for a state read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StateValue {
    /// Read key.
    pub read: StateRead,
    /// Raw value.
    pub value: B256,
    /// True when the value came from remote hydration.
    pub hydrated: bool,
}

/// State provider used by simulation.
pub trait StateProvider {
    /// Reads one state value.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is missing or unavailable.
    fn read(&mut self, read: &StateRead) -> Result<StateValue>;
}

/// Transport used by RPC-backed state hydration.
pub trait RpcStateTransport {
    /// Fetches one state value.
    ///
    /// # Errors
    ///
    /// Returns transport failures.
    fn fetch_state(&mut self, read: &StateRead) -> Result<B256>;
}

/// RPC-backed lazy state provider with local cache and auditable reads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RpcBackedStateProvider<T> {
    cache: BTreeMap<StateRead, B256>,
    reads: Vec<StateRead>,
    /// State transport.
    pub transport: T,
}

impl<T> RpcBackedStateProvider<T> {
    /// Creates a provider.
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            cache: BTreeMap::new(),
            reads: Vec::new(),
            transport,
        }
    }

    /// Inserts a cached value.
    pub fn insert_cached(&mut self, read: StateRead, value: B256) {
        self.cache.insert(read, value);
    }

    /// Returns reads in call order.
    #[must_use]
    pub fn reads(&self) -> &[StateRead] {
        &self.reads
    }
}

impl<T: RpcStateTransport> StateProvider for RpcBackedStateProvider<T> {
    fn read(&mut self, read: &StateRead) -> Result<StateValue> {
        self.reads.push(read.clone());
        if let Some(value) = self.cache.get(read).copied() {
            return Ok(StateValue {
                read: read.clone(),
                value,
                hydrated: false,
            });
        }
        let value = self.transport.fetch_state(read)?;
        self.cache.insert(read.clone(), value);
        Ok(StateValue {
            read: read.clone(),
            value,
            hydrated: true,
        })
    }
}

/// Deterministic in-memory state provider for tests and dry-runs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordingStateProvider {
    values: BTreeMap<StateRead, B256>,
    reads: Vec<StateRead>,
}

impl RecordingStateProvider {
    /// Inserts a value.
    pub fn insert(&mut self, read: StateRead, value: B256) {
        self.values.insert(read, value);
    }

    /// Returns reads in deterministic call order.
    #[must_use]
    pub fn reads(&self) -> &[StateRead] {
        &self.reads
    }
}

impl StateProvider for RecordingStateProvider {
    fn read(&mut self, read: &StateRead) -> Result<StateValue> {
        self.reads.push(read.clone());
        let Some(value) = self.values.get(read).copied() else {
            return Err(Error::Message(format!(
                "missing state read for address {:?} slot {:?}",
                read.address, read.slot
            )));
        };
        Ok(StateValue {
            read: read.clone(),
            value,
            hydrated: false,
        })
    }
}

/// Simulation request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimulationRequest {
    /// Request ID.
    pub id: String,
    /// Candidate to simulate.
    pub candidate: TransactionCandidate,
    /// State version used to build the candidate.
    pub state_version: StateVersion,
    /// Required reads.
    pub required_reads: Vec<StateRead>,
    /// Source events.
    pub sources: Vec<SourceEventRef>,
}

/// Simulation status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationStatus {
    /// Simulation succeeded.
    Success,
    /// Simulation reverted.
    Revert,
    /// Simulation could not run because state was missing.
    MissingState,
}

/// Simulation result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Request ID.
    pub request_id: String,
    /// Simulation status.
    pub status: SimulationStatus,
    /// Gas used by simulated execution.
    pub gas_used: u64,
    /// Value delta in wei.
    pub value_delta_wei: i64,
    /// State reads performed.
    pub state_reads: Vec<StateValue>,
    /// State version used.
    pub state_version: StateVersion,
    /// Error or revert reason.
    pub error: Option<String>,
}

impl SimulationResult {
    /// Returns true when the candidate is executable by default.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self.status, SimulationStatus::Success)
    }
}

/// Simulator interface.
pub trait Simulator {
    /// Runs one simulation.
    ///
    /// # Errors
    ///
    /// Returns infrastructure or state-provider errors.
    fn simulate(
        &mut self,
        request: &SimulationRequest,
        provider: &mut impl StateProvider,
    ) -> Result<SimulationResult>;
}

/// Backend contract for a Monad EVM-compatible simulator.
pub trait SimulationBackend {
    /// Runs a backend simulation.
    ///
    /// # Errors
    ///
    /// Returns backend failures.
    fn simulate_backend(
        &mut self,
        request: &SimulationRequest,
        reads: Vec<StateValue>,
    ) -> Result<SimulationResult>;
}

/// Simulator adapter for backend implementations.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BackendSimulator<B> {
    /// Backend implementation.
    pub backend: B,
}

impl<B> BackendSimulator<B> {
    /// Creates a backend simulator.
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B: SimulationBackend> Simulator for BackendSimulator<B> {
    fn simulate(
        &mut self,
        request: &SimulationRequest,
        provider: &mut impl StateProvider,
    ) -> Result<SimulationResult> {
        let mut reads = Vec::new();
        for read in &request.required_reads {
            reads.push(provider.read(read)?);
        }
        self.backend.simulate_backend(request, reads)
    }
}

/// Deterministic simulator for tests and local dry-runs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FakeSimulator {
    /// Gas charged for successful calls.
    pub success_gas: u64,
}

impl FakeSimulator {
    /// Creates a fake simulator with stable defaults.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            success_gas: 21_000,
        }
    }
}

impl Simulator for FakeSimulator {
    fn simulate(
        &mut self,
        request: &SimulationRequest,
        provider: &mut impl StateProvider,
    ) -> Result<SimulationResult> {
        let mut reads = Vec::new();
        for read in &request.required_reads {
            match provider.read(read) {
                Ok(value) => reads.push(value),
                Err(err) => {
                    return Ok(SimulationResult {
                        request_id: request.id.clone(),
                        status: SimulationStatus::MissingState,
                        gas_used: 0,
                        value_delta_wei: 0,
                        state_reads: reads,
                        state_version: request.state_version,
                        error: Some(err.to_string()),
                    });
                }
            }
        }

        if request.candidate.data.first() == Some(&0xff) {
            return Ok(SimulationResult {
                request_id: request.id.clone(),
                status: SimulationStatus::Revert,
                gas_used: self.success_gas / 2,
                value_delta_wei: 0,
                state_reads: reads,
                state_version: request.state_version,
                error: Some("fake simulator forced revert".to_owned()),
            });
        }

        Ok(SimulationResult {
            request_id: request.id.clone(),
            status: SimulationStatus::Success,
            gas_used: self.success_gas,
            value_delta_wei: request.candidate.value_estimate_wei,
            state_reads: reads,
            state_version: request.state_version,
            error: None,
        })
    }
}

/// Builds transaction candidates from domain-specific opportunities.
pub trait TransactionCandidateBuilder<O> {
    /// Builds a candidate.
    ///
    /// # Errors
    ///
    /// Returns an error when the opportunity cannot be represented as a
    /// transaction candidate.
    fn build_candidate(&self, opportunity: &O) -> Result<TransactionCandidate>;
}

#[cfg(test)]
mod tests {
    use monad_mev_core::CommitState;

    use super::*;

    #[derive(Clone, Debug, Default)]
    struct FakeRpcTransport {
        values: BTreeMap<StateRead, B256>,
    }

    impl RpcStateTransport for FakeRpcTransport {
        fn fetch_state(&mut self, read: &StateRead) -> Result<B256> {
            self.values
                .get(read)
                .copied()
                .ok_or_else(|| Error::Message("rpc state missing".to_owned()))
        }
    }

    #[derive(Clone, Debug)]
    struct EchoBackend;

    impl SimulationBackend for EchoBackend {
        fn simulate_backend(
            &mut self,
            request: &SimulationRequest,
            reads: Vec<StateValue>,
        ) -> Result<SimulationResult> {
            Ok(SimulationResult {
                request_id: request.id.clone(),
                status: SimulationStatus::Success,
                gas_used: 30_000,
                value_delta_wei: request.candidate.value_estimate_wei,
                state_reads: reads,
                state_version: request.state_version,
                error: None,
            })
        }
    }

    fn read() -> StateRead {
        StateRead {
            address: Address::from([1_u8; 20]),
            slot: Some(B256::from([2_u8; 32])),
        }
    }

    fn request(data: Vec<u8>, reads: Vec<StateRead>) -> SimulationRequest {
        SimulationRequest {
            id: "sim-1".to_owned(),
            candidate: TransactionCandidate {
                value_estimate_wei: 42,
                ..TransactionCandidate::new("candidate-1", Some(Address::from([3_u8; 20])), data)
            },
            state_version: StateVersion {
                revision: 1,
                last_seqno: 7,
                commit_state: CommitState::Finalized,
            },
            required_reads: reads,
            sources: Vec::new(),
        }
    }

    #[test]
    fn fake_simulator_records_success() {
        let state_read = read();
        let mut provider = RecordingStateProvider::default();
        provider.insert(state_read.clone(), B256::from([9_u8; 32]));
        let mut simulator = FakeSimulator::new();

        let result = simulator
            .simulate(&request(vec![1, 2, 3], vec![state_read]), &mut provider)
            .expect("simulation");

        assert!(result.is_success());
        assert_eq!(result.value_delta_wei, 42);
        assert_eq!(result.state_reads.len(), 1);
        assert_eq!(provider.reads().len(), 1);
    }

    #[test]
    fn fake_simulator_records_revert() {
        let mut provider = RecordingStateProvider::default();
        let mut simulator = FakeSimulator::new();

        let result = simulator
            .simulate(&request(vec![0xff], Vec::new()), &mut provider)
            .expect("simulation");

        assert_eq!(result.status, SimulationStatus::Revert);
        assert!(result.error.expect("error").contains("revert"));
    }

    #[test]
    fn missing_state_is_auditable() {
        let mut provider = RecordingStateProvider::default();
        let mut simulator = FakeSimulator::new();

        let result = simulator
            .simulate(&request(vec![1], vec![read()]), &mut provider)
            .expect("simulation");

        assert_eq!(result.status, SimulationStatus::MissingState);
        assert_eq!(provider.reads().len(), 1);
    }

    #[test]
    fn rpc_backed_state_provider_hydrates_and_caches_reads() {
        let state_read = read();
        let mut transport = FakeRpcTransport::default();
        transport
            .values
            .insert(state_read.clone(), B256::from([8_u8; 32]));
        let mut provider = RpcBackedStateProvider::new(transport);

        let first = provider.read(&state_read).expect("hydrated");
        let second = provider.read(&state_read).expect("cached");

        assert!(first.hydrated);
        assert!(!second.hydrated);
        assert_eq!(provider.reads().len(), 2);
    }

    #[test]
    fn backend_simulator_uses_state_provider_reads() {
        let state_read = read();
        let mut provider = RecordingStateProvider::default();
        provider.insert(state_read.clone(), B256::from([7_u8; 32]));
        let mut simulator = BackendSimulator::new(EchoBackend);

        let result = simulator
            .simulate(&request(vec![1], vec![state_read]), &mut provider)
            .expect("simulation");

        assert!(result.is_success());
        assert_eq!(result.gas_used, 30_000);
        assert_eq!(result.state_reads.len(), 1);
    }

    #[test]
    fn simulation_result_has_stable_json_shape() {
        let mut simulator = FakeSimulator::new();
        let mut provider = RecordingStateProvider::default();

        let result = simulator
            .simulate(&request(vec![1], Vec::new()), &mut provider)
            .expect("simulation");
        let json = serde_json::to_value(result).expect("json");

        assert_eq!(json["status"], "success");
        assert_eq!(json["gas_used"], 21_000);
        assert!(json.get("state_version").is_some());
    }
}
