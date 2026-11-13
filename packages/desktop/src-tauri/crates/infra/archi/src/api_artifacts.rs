//! 生成物写盘单点（本 crate 第二处写盘面，与 [`crate::report::write_report`] 并列）。
//!
//! 只做 dumb IO：越根复核 → 读盘比对 → 建目录 → 写文件；零判定、零渲染、零路径
//! 归一（串一律经 `archi::display`）——应生成集由 `archi::workspace_render` 给定。
//!
//! **只增只改、从不删**：孤儿生成物只由检查命令报告、由人决定如何处置，故本模块零
//! 删除分支。**非原子**：逐份 `fs::write`，单份失败即 `Err` 且不回滚已写部分（错误
//! 文案 MUST NOT 声称「全或无」）。

use std::path::Path;

use archi::dto::ArchiApiWriteOutcome;
use archi::GeneratedFile;

/// 写一批应生成物（先比对后写；字节相同即计入 `unchanged` 且不重写）。
///
/// 越根复核是**第二层**拦截（第一层在 `archi::api::paths` 的形态校验）：写盘是本
/// crate 唯一的危险动作（建目录 + 写文件可触达任意路径），两层判据交叉验证——任一层
/// 失效另一层仍拦得住。越根即 `Err` 且该份不写。
pub fn write_artifacts(
    root: &Path,
    files: &[GeneratedFile],
) -> Result<ArchiApiWriteOutcome, String> {
    let mut written: Vec<String> = Vec::new();
    let mut unchanged: Vec<String> = Vec::new();
    for file in files {
        if !file.path.starts_with(root) {
            return Err(format!(
                "生成物目标越出观察根（{} 不在 {} 之下）——拒绝写盘",
                file.path.display(),
                root.display()
            ));
        }
        let relative = archi::display(root, &file.path);
        if std::fs::read_to_string(&file.path).is_ok_and(|actual| actual == file.content) {
            unchanged.push(relative);
            continue;
        }
        if let Some(parent) = file.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("目录创建失败（{}）：{error}", parent.display()))?;
        }
        std::fs::write(&file.path, &file.content).map_err(|error| {
            format!(
                "写入失败（{}）：{error}（此前已写入 {} 份，不回滚）",
                file.path.display(),
                written.len()
            )
        })?;
        written.push(relative);
    }
    Ok(ArchiApiWriteOutcome { written, unchanged })
}
