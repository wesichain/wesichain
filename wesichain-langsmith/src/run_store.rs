use dashmap::DashMap;
use uuid::Uuid;

use crate::events::RunStatus;

#[derive(Clone, Debug)]
pub struct RunMetadata {
    pub status: RunStatus,
    pub error: Option<String>,
    pub parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunUpdateDecision {
    pub status: RunStatus,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct RunContextStore {
    runs: DashMap<Uuid, RunMetadata>,
}

impl RunContextStore {
    pub fn record_start(&self, run_id: Uuid, parent_id: Option<Uuid>) {
        self.runs.insert(
            run_id,
            RunMetadata {
                status: RunStatus::Running,
                error: None,
                parent_id,
            },
        );
    }

    pub fn apply_update(&self, run_id: Uuid, error: Option<String>) -> RunUpdateDecision {
        let mut entry = self.runs.entry(run_id).or_insert(RunMetadata {
            status: RunStatus::Running,
            error: None,
            parent_id: None,
        });

        match (&entry.status, error) {
            (RunStatus::Running, Some(err)) => {
                entry.status = RunStatus::Failed;
                entry.error = Some(err.clone());
                RunUpdateDecision {
                    status: RunStatus::Failed,
                    error: Some(err),
                }
            }
            (RunStatus::Running, None) => {
                entry.status = RunStatus::Completed;
                RunUpdateDecision {
                    status: RunStatus::Completed,
                    error: None,
                }
            }
            (RunStatus::Failed, _) => RunUpdateDecision {
                status: RunStatus::Failed,
                error: entry.error.clone(),
            },
            (RunStatus::Completed, _) => RunUpdateDecision {
                status: RunStatus::Completed,
                error: None,
            },
        }
    }
}
