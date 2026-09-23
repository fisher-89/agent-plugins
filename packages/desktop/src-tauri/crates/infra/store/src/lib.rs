//! `store`：redb 本地库，db 边界第一成员（infra 基础设施侧，自含模型）。
//!
//! # 单进程约束
//!
//! redb 面向单进程嵌入（进程内 MVCC、单写多读）。**双开不保证安全**——
//! dev 版与已装正式版指向同一 db 文件时可能数据竞争甚至损坏。store
//! 不自研跨进程锁兜底；进程单实例治理（tauri-plugin-single-instance 等）
//! 为后手，不在本 crate 范围。
//!
//! # db 路径注入约定
//!
//! `home_dir` 依赖 Tauri 上下文，store 内不做任何环境路径解析：
//! db 文件路径完全来自 [`Store::open`] 入参，由 dev-team 解析后注入。
//!
//! # schema_version 演进
//!
//! redb 无内建迁移：META 表自第一天记录 `schema_version`，打开时写入
//! 当前版本，或拒绝由更高版本应用创建的库；将来迁移逻辑在打开流程收口。

mod canonical;
mod model;
mod store;

pub use model::{AgentRunRecord, WorkspaceRecord};
pub use store::{Store, StoreError};

#[cfg(test)]
mod model_test;
#[cfg(test)]
mod store_test;
