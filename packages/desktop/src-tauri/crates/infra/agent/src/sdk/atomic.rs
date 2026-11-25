//! 原子写回单点：同目录临时文件 + rename 覆盖（失败清临时文件）。
//!
//! 本文件是全仓的**唯一落盘实现**（`fs::write` / `fs::rename` 的唯一位点）：模型写面
//! （壳层 `archi_set_gate` 与 agent 写工具 `archi_element` / `archi_relation`）与配置写面
//! （`workspace_config_apply`）共用同一实现，MUST NOT 各自复制一份。
//!
//! 临时后缀 `.archi-tmp` 是历史命名（本函数原驻壳层 `commands/archi`，后迁
//! `commands/atomic`），保留原样以免既有「临时文件不得残留」判据随改名失稳。父目录缺失
//! 即 `Err` 且**不代建目录**——写面只在既有目录内替换文件。

use std::fs;
use std::path::Path;

/// 原子写回：同目录临时文件 + rename 覆盖（失败清临时文件）。
pub fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("目标文件无父目录：{}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("目标文件名不可用：{}", path.display()))?;
    let temporary = parent.join(format!("{name}.archi-tmp"));
    if let Err(error) = fs::write(&temporary, content) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "写入临时文件失败（{}）：{error}",
            temporary.display()
        ));
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("替换目标文件失败（{}）：{error}", path.display()));
    }
    Ok(())
}
