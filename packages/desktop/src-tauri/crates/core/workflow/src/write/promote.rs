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
/// 全文、title 显式继承、workflow_type 透传入参），成功后删主仓笔记文件（移走
/// 半边）。主仓根注入（`main_root`）；worktree 落位父锚由 create 经 vcs port
/// 单点派生（git 根同级 `.worktrees/<仓库目录名>/`）。目标 change 目录 / 记录 /
/// branch / worktree 冲突面与 workflow_type 合法值校验均由 create 前置校验承担
/// （拒绝面零副作用）。
#[allow(clippy::too_many_arguments)]
pub fn promote_explore(
    main_root: &Path,
    store: &dyn ChangeStateStore,
    vcs: &dyn WorktreePort,
    project_roots: &[String],
    name: &str,
    note: &str,
    title: &str,
    workflow_type: &str,
) -> Result<PromoteOutcome, String> {
    let outcome = super::create(
        main_root,
        store,
        vcs,
        project_roots,
        name,
        note,
        title,
        workflow_type,
    )?;
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
