//! 命令层原子写回的薄委托：唯一实现在 infra 侧
//! `crates/infra/agent/src/sdk/atomic.rs`（`fs::write` / `fs::rename` 的唯一位点）。
//!
//! 模型写面（`commands::archi::archi_set_gate`）与配置写面
//! （`commands::config::workspace_config_apply`）共用同一实现，MUST NOT 各自复制一份。
//! 本文件零 `fs::write` / 零 `fs::rename` / 零 `create_dir_all`——临时后缀 `.archi-tmp`、
//! 失败清临时文件、父目录缺失即 `Err` 不建目录、错误文案全由被委托实现透传。

use std::path::Path;

/// 原子写回（薄委托，签名与错误文案逐字不变）。
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    agent_runtime::write_atomic(path, content)
}

#[cfg(test)]
#[path = "atomic_test.rs"]
mod atomic_test;
