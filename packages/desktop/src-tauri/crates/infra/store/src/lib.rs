mod canonical;
mod change_port;
mod describe;
mod envelope;
mod model;
mod notification;
mod query;
mod store;

pub use describe::{FieldDoc, ModelDoc, TableDoc};
pub use envelope::{dimension_of, ModelInfo, RecordEnvelope, RecordsPage};
pub use model::{
    AgentEffortTiers, AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderProtocol,
    AgentProviderRecord, AgentRunRecord, ChangeActivePhase, ChangeRecord, ChecklistItemRecord,
    ExploreRecord, NotificationRecord, PhaseRecord, RunEdgeRecord, RunNodeRecord, RunRecord,
    SessionConfigSnapshot, SessionEventRecord, SessionPreambleRecord, SessionRecord, StepRecord,
    WorkspaceRecord, CLI_TENANT_RETIRED,
};
pub use notification::StoreNotificationSink;
pub use query::{
    QueryCond, QueryError, QueryOrder, QueryPage, QuerySpec, DEFAULT_QUERY_LIMIT, MAX_QUERY_LIMIT,
    QUERY_OPS,
};
pub use store::{
    DbDimension, Store, StoreError, WorkspaceStores, EXPLORE_PLACEHOLDER_PREFIX,
    GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME,
};

#[cfg(test)]
mod change_port_test;
#[cfg(test)]
mod describe_test;
#[cfg(test)]
mod envelope_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod notification_test;
#[cfg(test)]
mod query_test;
#[cfg(test)]
mod store_test;
