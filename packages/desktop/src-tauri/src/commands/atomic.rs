//! 命令层原子写回单点：同目录临时文件 + rename 覆盖（失败清临时文件）。
//!
//! 本文件是命令层的唯一落盘点（`fs::write` 只此一处）：模型写面
//! （`commands::archi::archi_set_gate`）与配置写面
//! （`commands::config::workspace_config_apply`）共用同一实现，MUST NOT 各自
//! 复制一份。临时后缀 `.archi-tmp` 是历史命名（本函数原驻 `commands/archi`），
//! 保留原样以免既有「临时文件不得残留」判据随改名失稳。

use std::fs;
use std::path::Path;

/// 原子写回：同目录临时文件 + rename 覆盖（失败清临时文件）。
pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
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

#[cfg(test)]
#[path = "atomic_test.rs"]
mod atomic_test;
