//! change 域纯读库：对任意 workspace 的过程记录做宽松解析、查询聚合与产物发现。
//!
//! 模块边界：
//! - `model`：领域类型（含三代数据形状）
//! - `parse`：代际探测 + serde 宽松解析
//! - `queries`：列表 / 详情聚合（纯读）
//! - `artifacts`：ArtifactEnvelope 信封 + matcher/parser 静态注册表 + 第一波三插件

pub mod artifacts;
pub mod model;
pub mod parse;
pub mod queries;
