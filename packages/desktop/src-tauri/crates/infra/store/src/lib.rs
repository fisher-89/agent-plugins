//! `store`：native_db 本地库（构建于 redb 之上），db 边界第一成员。
//!
//! # 模型层架构
//!
//! store 为模型层：三个模型（`WorkspaceRecord` / `AgentRunRecord` /
//! `AgentEventRecord`）经 `#[native_model]` + `#[native_db]` 注册，记录经
//! native_model bincode 编码落库，shape 演进的版本治理由 native_model 版本
//! 机制承担。底层仍是 redb——事务 / ACID / 文件格式不变；升级实质是加模型
//! 层，不是换引擎。legacy redb 手写表库（`user_*` 表 + `schema_version`
//! 手工轮账）由 `migrate` 模块一次性迁移（旧文件 `.bak` 留档），META 轮账
//! 随之退役。数据维度语义由 db 文件归属承载（user db 落 app data dir，见
//! desktop-data-dimensions），不再依赖表名前缀。
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

mod canonical;
mod envelope;
mod migrate;
mod model;
mod store;

pub use envelope::{ModelInfo, RecordEnvelope};
pub use model::{AgentEventRecord, AgentRunRecord, WorkspaceRecord};
pub use store::{Store, StoreError};

#[cfg(test)]
mod envelope_test;
#[cfg(test)]
mod migrate_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod store_test;
