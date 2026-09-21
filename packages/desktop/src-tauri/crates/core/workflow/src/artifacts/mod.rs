//! 产物模块根：信封契约 + 静态注册表 + 第一波三插件。

pub mod envelope;
pub mod eval_checklist;
pub mod markdown_doc;
pub mod registry;
pub mod tasks_progress;

pub use envelope::{ArtifactCandidate, ArtifactDescriptor, ArtifactEnvelope};
pub use registry::{discover_artifacts, read_artifact, ArtifactInput, ArtifactPlugin};

#[cfg(test)]
mod registry_test;

#[cfg(test)]
mod markdown_doc_test;

#[cfg(test)]
mod eval_checklist_test;

#[cfg(test)]
mod tasks_progress_test;
