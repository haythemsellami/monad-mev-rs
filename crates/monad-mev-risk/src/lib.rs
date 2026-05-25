//! Risk policy and execution-plan types for `monad-mev-rs`.

use std::collections::BTreeSet;

use monad_mev_core::{Address, Error, Result, U256};
use monad_mev_sim::{SimulationRequest, SimulationResult, TransactionCandidate};
use serde::{Deserialize, Serialize};

/// Context used while evaluating risk.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RiskContext {
    /// Current observed descriptor sequence number.
    pub current_seqno: u64,
    /// Most recent external data sequence number, if applicable.
    pub external_data_seqno: Option<u64>,
}

/// Configurable risk policy.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RiskPolicy {
    /// Policy name.
    pub name: String,
    /// Stop all execution when true.
    pub circuit_breaker: bool,
    /// Require successful simulation.
    pub require_successful_simulation: bool,
    /// Maximum simulated gas.
    pub max_gas: Option<u64>,
    /// Minimum value delta in wei.
    pub min_value_delta_wei: Option<i64>,
    /// Maximum allowed loss in wei.
    pub max_loss_wei: Option<i64>,
    /// Maximum native value sent by the candidate.
    pub max_notional_wei: Option<U256>,
    /// Allowed target addresses. Empty means any target.
    pub allowed_targets: BTreeSet<Address>,
    /// Allowed function selectors. Empty means any selector.
    pub allowed_selectors: BTreeSet<[u8; 4]>,
    /// Maximum allowed age for external data in descriptor sequence numbers.
    pub max_external_data_lag: Option<u64>,
    /// Maximum allowed age for simulation state in descriptor sequence numbers.
    pub max_simulation_lag: Option<u64>,
}

impl RiskPolicy {
    /// Creates a named policy with production-safe defaults.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            require_successful_simulation: true,
            ..Self::default()
        }
    }

    /// Evaluates a candidate and simulation result.
    #[must_use]
    pub fn evaluate(
        &self,
        context: &RiskContext,
        request: &SimulationRequest,
        result: &SimulationResult,
    ) -> RiskDecision {
        let mut rejections = Vec::new();

        if self.circuit_breaker {
            rejections.push(RiskRejection::new(
                "circuit_breaker",
                "circuit breaker is active",
            ));
        }
        if self.require_successful_simulation && !result.is_success() {
            rejections.push(RiskRejection::new(
                "simulation_status",
                "simulation was not successful",
            ));
        }
        if self.max_gas.is_some_and(|max| result.gas_used > max) {
            rejections.push(RiskRejection::new(
                "max_gas",
                format!("gas {} exceeds policy", result.gas_used),
            ));
        }
        if self
            .min_value_delta_wei
            .is_some_and(|min| result.value_delta_wei < min)
        {
            rejections.push(RiskRejection::new(
                "min_value_delta",
                format!("value delta {} is below policy", result.value_delta_wei),
            ));
        }
        if self
            .max_loss_wei
            .is_some_and(|max_loss| result.value_delta_wei < -max_loss)
        {
            rejections.push(RiskRejection::new(
                "max_loss",
                format!("value delta {} exceeds loss policy", result.value_delta_wei),
            ));
        }
        if self
            .max_notional_wei
            .is_some_and(|max| request.candidate.value > max)
        {
            rejections.push(RiskRejection::new(
                "max_notional",
                "candidate native value exceeds policy",
            ));
        }
        if !self.allowed_targets.is_empty() {
            match request.candidate.to {
                Some(target) if self.allowed_targets.contains(&target) => {}
                _ => rejections.push(RiskRejection::new(
                    "target",
                    "candidate target is not allowed",
                )),
            }
        }
        if !self.allowed_selectors.is_empty() {
            match selector(&request.candidate) {
                Some(sel) if self.allowed_selectors.contains(&sel) => {}
                _ => rejections.push(RiskRejection::new(
                    "selector",
                    "candidate selector is not allowed",
                )),
            }
        }
        if let (Some(max_lag), Some(data_seqno)) =
            (self.max_external_data_lag, context.external_data_seqno)
        {
            if context.current_seqno.saturating_sub(data_seqno) > max_lag {
                rejections.push(RiskRejection::new(
                    "external_data_freshness",
                    "external data is stale",
                ));
            }
        }
        if self.max_simulation_lag.is_some_and(|max_lag| {
            context
                .current_seqno
                .saturating_sub(result.state_version.last_seqno)
                > max_lag
        }) {
            rejections.push(RiskRejection::new(
                "simulation_freshness",
                "simulation state is stale",
            ));
        }

        RiskDecision {
            policy: self.name.clone(),
            approved: rejections.is_empty(),
            rejections,
            request_id: request.id.clone(),
        }
    }
}

fn selector(candidate: &TransactionCandidate) -> Option<[u8; 4]> {
    let bytes: [u8; 4] = candidate.data.get(0..4)?.try_into().ok()?;
    Some(bytes)
}

/// Risk rejection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RiskRejection {
    /// Rejection kind.
    pub kind: String,
    /// Rejection reason.
    pub reason: String,
}

impl RiskRejection {
    /// Creates a rejection.
    #[must_use]
    pub fn new(kind: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            reason: reason.into(),
        }
    }
}

/// Risk decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RiskDecision {
    /// Policy name.
    pub policy: String,
    /// True when approved.
    pub approved: bool,
    /// Rejection list.
    pub rejections: Vec<RiskRejection>,
    /// Simulation request ID.
    pub request_id: String,
}

/// Risk-approved execution plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Plan ID.
    pub id: String,
    /// Candidate to execute.
    pub candidate: TransactionCandidate,
    /// Simulation result.
    pub simulation: SimulationResult,
    /// Risk decision.
    pub risk: RiskDecision,
    /// True only for explicitly unsafe bypass plans.
    pub unsafe_bypass: bool,
}

impl ExecutionPlan {
    /// Builds a default execution plan.
    ///
    /// # Errors
    ///
    /// Returns an error when simulation failed or risk rejected the candidate.
    pub fn build(
        id: impl Into<String>,
        request: SimulationRequest,
        simulation: SimulationResult,
        risk: RiskDecision,
    ) -> Result<Self> {
        if !simulation.is_success() {
            return Err(Error::Message(
                "execution plan requires successful simulation".to_owned(),
            ));
        }
        if !risk.approved {
            return Err(Error::Message(
                "execution plan requires risk approval".to_owned(),
            ));
        }
        Ok(Self {
            id: id.into(),
            candidate: request.candidate,
            simulation,
            risk,
            unsafe_bypass: false,
        })
    }

    /// Builds an explicit unsafe bypass plan.
    #[must_use]
    pub fn unsafe_bypass(
        id: impl Into<String>,
        request: SimulationRequest,
        simulation: SimulationResult,
        risk: RiskDecision,
    ) -> Self {
        Self {
            id: id.into(),
            candidate: request.candidate,
            simulation,
            risk,
            unsafe_bypass: true,
        }
    }
}

/// Nonce replacement policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NoncePolicy {
    /// Sender address.
    pub sender: Address,
    /// Current nonce.
    pub current_nonce: u64,
    /// Maximum pending transactions allowed for the sender.
    pub max_pending: u64,
    /// Allow replacement.
    pub allow_replacement: bool,
}

impl NoncePolicy {
    /// Reserves the next nonce.
    ///
    /// # Errors
    ///
    /// Returns an error when pending count exceeds policy.
    pub fn reserve(&self, pending: u64) -> Result<u64> {
        if pending >= self.max_pending {
            return Err(Error::Message("pending nonce limit reached".to_owned()));
        }
        Ok(self.current_nonce + pending)
    }
}

/// Structured audit record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Stage name.
    pub stage: String,
    /// Stable object ID.
    pub id: String,
    /// JSON payload.
    pub payload: serde_json::Value,
}

/// Deterministic audit log.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditLog {
    records: Vec<AuditRecord>,
}

impl AuditLog {
    /// Appends a record.
    ///
    /// # Errors
    ///
    /// Returns an error when payload serialization fails.
    pub fn push<T: Serialize>(
        &mut self,
        stage: impl Into<String>,
        id: impl Into<String>,
        payload: &T,
    ) -> Result<()> {
        self.records.push(AuditRecord {
            stage: stage.into(),
            id: id.into(),
            payload: serde_json::to_value(payload).map_err(|err| {
                Error::Message(format!("failed to serialize audit payload: {err}"))
            })?,
        });
        Ok(())
    }

    /// Returns audit records.
    #[must_use]
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    /// Returns deterministic JSONL.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization fails.
    pub fn jsonl(&self) -> Result<String> {
        let mut out = String::new();
        for record in &self.records {
            out.push_str(&serde_json::to_string(record).map_err(|err| {
                Error::Message(format!("failed to serialize audit record: {err}"))
            })?);
            out.push('\n');
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{CommitState, B256};
    use monad_mev_sim::{SimulationStatus, StateRead, StateValue};
    use monad_mev_store::StateVersion;

    use super::*;

    fn request(data: Vec<u8>) -> SimulationRequest {
        SimulationRequest {
            id: "req-1".to_owned(),
            candidate: TransactionCandidate {
                value_estimate_wei: 100,
                ..TransactionCandidate::new("cand-1", Some(Address::from([1_u8; 20])), data)
            },
            state_version: StateVersion {
                revision: 1,
                last_seqno: 10,
                commit_state: CommitState::Finalized,
            },
            required_reads: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn result(status: SimulationStatus, value_delta_wei: i64) -> SimulationResult {
        SimulationResult {
            request_id: "req-1".to_owned(),
            status,
            gas_used: 21_000,
            value_delta_wei,
            state_reads: Vec::new(),
            state_version: StateVersion {
                revision: 1,
                last_seqno: 10,
                commit_state: CommitState::Finalized,
            },
            error: None,
        }
    }

    #[test]
    fn risk_policy_approves_successful_candidate() {
        let policy = RiskPolicy {
            max_gas: Some(30_000),
            min_value_delta_wei: Some(1),
            ..RiskPolicy::named("default")
        };

        let decision = policy.evaluate(
            &RiskContext {
                current_seqno: 11,
                external_data_seqno: Some(11),
            },
            &request(vec![1, 2, 3, 4]),
            &result(SimulationStatus::Success, 100),
        );

        assert!(decision.approved);
    }

    #[test]
    fn risk_policy_rejects_stale_external_data() {
        let policy = RiskPolicy {
            max_external_data_lag: Some(2),
            ..RiskPolicy::named("default")
        };

        let decision = policy.evaluate(
            &RiskContext {
                current_seqno: 10,
                external_data_seqno: Some(7),
            },
            &request(vec![1]),
            &result(SimulationStatus::Success, 1),
        );

        assert!(!decision.approved);
        assert_eq!(decision.rejections[0].kind, "external_data_freshness");
    }

    #[test]
    fn risk_policy_rejects_selector() {
        let mut policy = RiskPolicy::named("default");
        policy.allowed_selectors.insert([1, 2, 3, 4]);

        let decision = policy.evaluate(
            &RiskContext::default(),
            &request(vec![9, 9, 9, 9]),
            &result(SimulationStatus::Success, 1),
        );

        assert_eq!(decision.rejections[0].kind, "selector");
    }

    #[test]
    fn execution_plan_requires_approval() {
        let req = request(vec![1]);
        let sim = result(SimulationStatus::Success, 1);
        let decision = RiskDecision {
            policy: "p".to_owned(),
            approved: false,
            rejections: vec![RiskRejection::new("x", "no")],
            request_id: "req-1".to_owned(),
        };

        assert!(ExecutionPlan::build("plan", req, sim, decision).is_err());
    }

    #[test]
    fn nonce_policy_reserves_pending_nonce() {
        let policy = NoncePolicy {
            sender: Address::from([1_u8; 20]),
            current_nonce: 10,
            max_pending: 3,
            allow_replacement: false,
        };

        assert_eq!(policy.reserve(2).expect("nonce"), 12);
        assert!(policy.reserve(3).is_err());
    }

    #[test]
    fn audit_log_is_jsonl() {
        let mut log = AuditLog::default();
        let value = StateValue {
            read: StateRead {
                address: Address::from([1_u8; 20]),
                slot: None,
            },
            value: B256::from([2_u8; 32]),
            hydrated: false,
        };

        log.push("state_read", "read-1", &value).expect("audit");

        assert_eq!(log.records().len(), 1);
        assert!(log.jsonl().expect("jsonl").contains("state_read"));
    }
}
