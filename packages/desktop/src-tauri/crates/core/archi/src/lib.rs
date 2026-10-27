//! 架构模型纯读域：解析 / 内存图 / 结构诊断 / 文档关联 / 检索 / 结构 diff /
//! `gate` 写回纯函数 / 三面共用投影。
//!
//! 四个消费面同源（进程内结构校验、agent 工具 `archi_*`、IPC 命令面、画布），
//! 三条机械纪律：
//! 1. **单点构建**：`ModelGraph` 只在 [`graph`] 建一次（外部只经 [`dto`] 的
//!    投影入口消费），消费面 MUST NOT 各自再建图；
//! 2. **投影零改写**：投影只裁剪（1-hop / 字段子集），MUST NOT 重算、MUST NOT
//!    推导、MUST NOT 补默认值；
//! 3. **词汇全量对拍**：封闭词汇（元素种类 / gate 档 / 结构诊断 / 文档警告 / 边
//!    标记 / 节点标记）只有一份定义（[`dto`]），三面覆盖同一词汇集。
//!
//! 边界：零 UI 框架 / 零库句柄 / 零子进程 / 零工作流与编排 crate 依赖 / 零工作区
//! 配置读取（工作区配置唯一出口纪律零变化）；本 crate 只读盘不写盘（唯一写动作
//! 在命令层）。

pub mod diff;
pub mod docs;
pub mod dto;
pub mod edit;
pub mod graph;
pub mod model;
pub mod parse;
pub mod search;

pub use dto::{
    ArchiChangeSummary, ArchiGraph, ArchiModuleDetail, ArchiSearchResult, DiagnosticKind, DocKind,
    DocRef, DocSource, EdgeMark, ElementKind, GateTier, NodeMark, SearchScope, WarningKind,
};
pub use model::{Diagnostic, Element, Model, Relation};

#[cfg(test)]
mod diff_test;
#[cfg(test)]
mod docs_test;
#[cfg(test)]
mod dto_test;
#[cfg(test)]
mod edit_test;
#[cfg(test)]
mod graph_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod search_test;
