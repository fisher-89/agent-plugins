mod canonical;
mod envelope;
mod model;
mod store;

pub use envelope::{ModelInfo, RecordEnvelope};
pub use model::{
    AgentEngineKind, AgentInstanceRecord, AgentModelTiers, AgentProviderRecord, AgentRunRecord,
    ExploreRecord, SessionConfigSnapshot, SessionEventRecord, SessionRecord, WorkspaceRecord,
};
pub use store::{DbDimension, Store, StoreError, WorkspaceStores};

#[cfg(test)]
mod envelope_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod store_test;
