//! 探索笔记搬运（promote 的 move 半边，design D3/D8）：复用 change 写面
//! [`create`](crate::write::create) 建 change（`name = explore.name`、
//! `goal = 笔记全文`、`title = explore.title`——worktree 内
//! `changes/<name>/explore.md` 即笔记全文，含首行 `# <标题>`），成功后删主仓
//! `explores/<name>.md`（移走半边；explore 笔记在主仓、change 的 explore.md 在
//! worktree 内，跨 worktree 边界是内容搬运，MUST NOT `fs::rename`）。
//!
//! 失败面：create 失败 → 既有 create 补偿链语义零改动（半成品回收归 create）；
//! 删笔记失败 → `Err` 携 change id 与「笔记全文已在 change explore.md 留底，
//! 可手动删除或回写」指引（残留对象显式呈现，不静默自愈）。
//!
//! sync 零 Tauri 零 tokio；磁盘路径全部经 [`foundation::layout::Layout`] 取得
//! （MUST NOT 自拼 `openspec` 目录名字面量）；落库经
//! [`ChangeStateStore`](crate::state::ChangeStateStore) port 缝、进程执行经
//! [`WorktreePort`] port 缝。
//! 能力 spec：`specs/desktop-explore-queries/spec.md`（路径相对域根）。

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

use foundation::layout::resolve;

use crate::state::ChangeStateStore;
use crate::write::worktree::WorktreePort;

/// promote 产出（IPC DTO）：本次铸出的 change 身份锚（前端跳转 / 一切后续寻
/// 址入参）与 change 名（= explore name，promote 免转换语义的显式呈现）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromoteOutcome {
    /// 本次铸出的 change id（uuid v7 形态；库内记录同值逐字一致）
    pub change_id: String,
    /// change 名（= explore 记录 name，恒裸名）
    pub change_name: String,
}

/// promote（move）：调 [`create`](crate::write::create) 建 change（goal = 笔记
/// 全文、title 显式继承），成功后删主仓笔记文件（移走半边）。双根注入语义同
/// create（`main_root` 主仓根、`worktree_root` 落位父锚）；目标 change 目录 /
/// 记录 / branch / worktree 冲突面均由 create 前置校验承担（拒绝面零副作用）。
pub fn promote_explore(
    main_root: &Path,
    worktree_root: &Path,
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    name: &str,
    note: &str,
    title: &str,
) -> Result<PromoteOutcome, String> {
    let outcome = super::create(main_root, worktree_root, store, vcs, name, note, title)?;
    let layout = resolve(main_root);
    let note_path = layout.explores_root.join(format!("{name}.md"));
    match fs::remove_file(&note_path) {
        Ok(()) => {}
        // 竞态窗口内笔记已被外部删除：搬运语义已达成（change 内已有全文），
        // 不误报半完成态
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "探索笔记搬运未完成: change \"{}\" 已建档（id={}）且笔记全文已写入其 explore.md，\
                 但主仓笔记删除失败（{}）: {error}；笔记全文已在 change explore.md 留底，\
                 可手动删除或回写",
                outcome.name,
                outcome.id,
                note_path.display()
            ));
        }
    }
    Ok(PromoteOutcome {
        change_id: outcome.id,
        change_name: outcome.name,
    })
}
