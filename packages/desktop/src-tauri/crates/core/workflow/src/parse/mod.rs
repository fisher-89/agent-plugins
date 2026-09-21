//! 代际探测 + serde 宽松解析。

pub mod detect;
pub mod workflow_file;

pub use detect::detect_inventory;
pub use workflow_file::{load_workflow, parse_workflow_file, WorkflowFileParse};

/// workflow.json 在 change 目录内的文件名（本 crate 内共享）。
pub(crate) const WORKFLOW_FILE_NAME: &str = "workflow.json";

#[cfg(test)]
mod detect_test;

#[cfg(test)]
mod workflow_file_test;
