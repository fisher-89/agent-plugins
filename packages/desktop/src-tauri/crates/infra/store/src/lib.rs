mod canonical;
mod change_port;
mod envelope;
mod model;
mod store;

pub use envelope::{ModelInfo, RecordEnvelope, RecordsPage};
pub use model::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, AgentRunRecord, ChangeActivePhase, ChangeRecord, ChecklistItemRecord,
    ExploreRecord, PhaseRecord, SessionConfigSnapshot, SessionEventRecord, SessionRecord,
    StepRecord, WorkspaceRecord, CLI_TENANT_RETIRED,
};
pub use store::{
    DbDimension, Store, StoreError, WorkspaceStores, EXPLORE_PLACEHOLDER_PREFIX,
    GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME,
};

#[cfg(test)]
mod change_port_test;
#[cfg(test)]
mod envelope_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod store_test;
