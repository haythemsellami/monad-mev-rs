//! Explicit execution interfaces for `monad-mev-rs`.

use monad_mev_core::{Error, Result, B256};
use monad_mev_risk::{AuditLog, ExecutionPlan};
use serde::{Deserialize, Serialize};

/// Executor mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorMode {
    /// Record the plan only.
    Recording,
    /// Validate the plan but do not submit it.
    DryRun,
    /// Submit through a configured transport.
    Production,
}

/// Executor config.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// Execution mode.
    pub mode: ExecutorMode,
    /// Production submission must be explicitly enabled.
    pub production_enabled: bool,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            mode: ExecutorMode::Recording,
            production_enabled: false,
        }
    }
}

/// Transport-level receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransportReceipt {
    /// Transaction hash or bundle hash.
    pub hash: B256,
    /// Transport status.
    pub status: String,
}

/// Production submission transport.
pub trait SubmitTransport {
    /// Submits a plan.
    ///
    /// # Errors
    ///
    /// Returns transport-specific failures.
    fn submit(&mut self, plan: &ExecutionPlan) -> Result<TransportReceipt>;
}

/// Execution receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmitReceipt {
    /// Plan ID.
    pub plan_id: String,
    /// Executor mode.
    pub mode: ExecutorMode,
    /// Whether a real submit was attempted.
    pub submitted: bool,
    /// Optional transport hash.
    pub hash: Option<B256>,
    /// Status.
    pub status: String,
}

/// Risk-checked executor.
#[derive(Clone, Debug)]
pub struct RiskCheckedExecutor<T> {
    /// Executor config.
    pub config: ExecutorConfig,
    /// Submission transport.
    pub transport: T,
    /// Audit log.
    pub audit: AuditLog,
}

impl<T: SubmitTransport> RiskCheckedExecutor<T> {
    /// Creates an executor.
    #[must_use]
    pub fn new(config: ExecutorConfig, transport: T) -> Self {
        Self {
            config,
            transport,
            audit: AuditLog::default(),
        }
    }

    /// Executes one plan according to config.
    ///
    /// # Errors
    ///
    /// Returns an error for rejected risk, missing explicit production opt-in,
    /// or transport failure.
    pub fn execute(&mut self, plan: &ExecutionPlan) -> Result<SubmitReceipt> {
        Self::validate_plan(plan)?;
        self.audit.push("execution_plan", &plan.id, plan)?;

        match self.config.mode {
            ExecutorMode::Recording => Ok(SubmitReceipt {
                plan_id: plan.id.clone(),
                mode: ExecutorMode::Recording,
                submitted: false,
                hash: None,
                status: "recorded".to_owned(),
            }),
            ExecutorMode::DryRun => Ok(SubmitReceipt {
                plan_id: plan.id.clone(),
                mode: ExecutorMode::DryRun,
                submitted: false,
                hash: None,
                status: "dry_run_validated".to_owned(),
            }),
            ExecutorMode::Production => {
                if !self.config.production_enabled {
                    return Err(Error::Message(
                        "production execution requires explicit opt-in".to_owned(),
                    ));
                }
                let receipt = self.transport.submit(plan)?;
                Ok(SubmitReceipt {
                    plan_id: plan.id.clone(),
                    mode: ExecutorMode::Production,
                    submitted: true,
                    hash: Some(receipt.hash),
                    status: receipt.status,
                })
            }
        }
    }

    fn validate_plan(plan: &ExecutionPlan) -> Result<()> {
        if !plan.unsafe_bypass && !plan.risk.approved {
            return Err(Error::Message(
                "execution requires risk-approved plan".to_owned(),
            ));
        }
        if !plan.unsafe_bypass && !plan.simulation.is_success() {
            return Err(Error::Message(
                "execution requires successful simulation".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Deterministic fake transport.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FakeTransport {
    /// Submitted plan IDs.
    pub submitted: Vec<String>,
    /// Fail the next submit.
    pub fail_next: bool,
}

impl SubmitTransport for FakeTransport {
    fn submit(&mut self, plan: &ExecutionPlan) -> Result<TransportReceipt> {
        if self.fail_next {
            self.fail_next = false;
            return Err(Error::Message("fake transport submit failed".to_owned()));
        }
        self.submitted.push(plan.id.clone());
        Ok(TransportReceipt {
            hash: deterministic_hash(&plan.id),
            status: "submitted".to_owned(),
        })
    }
}

/// RPC submit transport interface placeholder.
pub trait RpcSubmitTransport: SubmitTransport {
    /// Returns the RPC endpoint name or URL.
    fn endpoint(&self) -> &str;
}

/// Relay/bundle request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleRequest {
    /// Bundle ID.
    pub id: String,
    /// Execution plans in submission order.
    pub plans: Vec<ExecutionPlan>,
}

/// Relay/bundle receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleReceipt {
    /// Bundle ID.
    pub id: String,
    /// Bundle hash.
    pub hash: B256,
    /// Status.
    pub status: String,
}

/// Relay/bundle transport interface.
pub trait BundleTransport {
    /// Submits a bundle.
    ///
    /// # Errors
    ///
    /// Returns relay-specific failures.
    fn submit_bundle(&mut self, bundle: &BundleRequest) -> Result<BundleReceipt>;
}

/// Deterministic fake bundle transport.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FakeBundleTransport {
    /// Submitted bundle IDs.
    pub submitted: Vec<String>,
}

impl BundleTransport for FakeBundleTransport {
    fn submit_bundle(&mut self, bundle: &BundleRequest) -> Result<BundleReceipt> {
        self.submitted.push(bundle.id.clone());
        Ok(BundleReceipt {
            id: bundle.id.clone(),
            hash: deterministic_hash(&bundle.id),
            status: format!("submitted:{}plans", bundle.plans.len()),
        })
    }
}

fn deterministic_hash(input: &str) -> B256 {
    let mut bytes = [0_u8; 32];
    for (index, byte) in input.as_bytes().iter().enumerate() {
        bytes[index % 32] ^= *byte;
    }
    B256::from(bytes)
}

#[cfg(test)]
mod tests {
    use monad_mev_core::{Address, CommitState};
    use monad_mev_risk::{RiskDecision, RiskPolicy};
    use monad_mev_sim::{
        SimulationRequest, SimulationResult, SimulationStatus, TransactionCandidate,
    };
    use monad_mev_store::StateVersion;

    use super::*;

    fn plan(approved: bool) -> ExecutionPlan {
        let request = SimulationRequest {
            id: "req".to_owned(),
            candidate: TransactionCandidate::new(
                "cand",
                Some(Address::from([1_u8; 20])),
                vec![1, 2, 3, 4],
            ),
            state_version: StateVersion {
                revision: 1,
                last_seqno: 1,
                commit_state: CommitState::Finalized,
            },
            required_reads: Vec::new(),
            sources: Vec::new(),
        };
        let simulation = SimulationResult {
            request_id: "req".to_owned(),
            status: SimulationStatus::Success,
            gas_used: 21_000,
            value_delta_wei: 1,
            state_reads: Vec::new(),
            state_version: request.state_version,
            error: None,
        };
        let risk = RiskDecision {
            policy: RiskPolicy::named("default").name,
            approved,
            rejections: Vec::new(),
            request_id: "req".to_owned(),
        };
        ExecutionPlan {
            id: "plan".to_owned(),
            candidate: request.candidate,
            simulation,
            risk,
            unsafe_bypass: false,
        }
    }

    #[test]
    fn default_executor_records_only() {
        let mut executor =
            RiskCheckedExecutor::new(ExecutorConfig::default(), FakeTransport::default());

        let receipt = executor.execute(&plan(true)).expect("execute");

        assert_eq!(receipt.mode, ExecutorMode::Recording);
        assert!(!receipt.submitted);
        assert_eq!(executor.transport.submitted.len(), 0);
    }

    #[test]
    fn dry_run_validates_without_submit() {
        let mut executor = RiskCheckedExecutor::new(
            ExecutorConfig {
                mode: ExecutorMode::DryRun,
                production_enabled: false,
            },
            FakeTransport::default(),
        );

        let receipt = executor.execute(&plan(true)).expect("execute");

        assert_eq!(receipt.status, "dry_run_validated");
        assert_eq!(executor.transport.submitted.len(), 0);
    }

    #[test]
    fn production_requires_explicit_opt_in() {
        let mut executor = RiskCheckedExecutor::new(
            ExecutorConfig {
                mode: ExecutorMode::Production,
                production_enabled: false,
            },
            FakeTransport::default(),
        );

        assert!(executor.execute(&plan(true)).is_err());
    }

    #[test]
    fn fake_submit_records_hash() {
        let mut executor = RiskCheckedExecutor::new(
            ExecutorConfig {
                mode: ExecutorMode::Production,
                production_enabled: true,
            },
            FakeTransport::default(),
        );

        let receipt = executor.execute(&plan(true)).expect("execute");

        assert!(receipt.submitted);
        assert!(receipt.hash.is_some());
        assert_eq!(executor.transport.submitted, vec!["plan"]);
    }

    #[test]
    fn rejected_plan_is_not_executed() {
        let mut executor = RiskCheckedExecutor::new(
            ExecutorConfig {
                mode: ExecutorMode::Production,
                production_enabled: true,
            },
            FakeTransport::default(),
        );

        assert!(executor.execute(&plan(false)).is_err());
        assert!(executor.transport.submitted.is_empty());
    }

    #[test]
    fn fake_bundle_transport_records_submission() {
        let execution_plan = plan(true);
        let bundle = BundleRequest {
            id: "bundle-1".to_owned(),
            plans: vec![execution_plan],
        };
        let mut transport = FakeBundleTransport::default();

        let receipt = transport.submit_bundle(&bundle).expect("bundle");

        assert_eq!(receipt.status, "submitted:1plans");
        assert_eq!(transport.submitted, vec!["bundle-1"]);
    }
}
