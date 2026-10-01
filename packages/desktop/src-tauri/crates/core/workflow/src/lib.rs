//! 模块边界：
//! - `model`：领域类型（含三代数据形状）
//! - `parse`：代际探测 + serde 宽松解析
//! - `queries`：列表 / 详情聚合（纯读）
//! - `artifacts`：ArtifactEnvelope 信封 + matcher/parser 静态注册表 + 第一波三插件
//! - `write`：workflow.json 写面（相位表单源 + 相位机四操作，sync 零 tokio）

pub mod artifacts;
pub mod model;
pub mod parse;
pub mod queries;
pub mod write;
