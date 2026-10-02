//! change 记录面命令组（建档写通道）：change 域三分职责的记录面——
//! `queries`（纯读）/ 本组（记录面写）/ `change_flow`（run 编排控制），
//! 沿 explores 组「读 + 记录面」同组先例。三件事纪律沿 `commands/queries`
//! 模板：参数转换 → 调写面 → 错误映射；sync 纯函数命令（无 State /
//! AppHandle / Channel，无需 `_with` 测试缝）；blank root 显式 `Err`（写
//! 无空结果语义，不进入写面链路）；name / goal 校验权威在写面，命令层
//! 不过关。
//! 能力 spec：`specs/desktop-change-create/spec.md`（路径相对域根）。

use std::path::Path;

use foundation::layout::resolve;
use workflow::write::{self, CreateOutcome};

#[cfg(test)]
mod mod_test;

/// 新建 change：目录建树、workflow.json 初始文档与 explore.md（落最初
/// goal）写出均在写面 `create`；blank root 显式 `Err`。
#[tauri::command]
#[specta::specta]
pub fn create_change(root: String, name: String, goal: String) -> Result<CreateOutcome, String> {
    if root.trim().is_empty() {
        return Err("非法 root: 不得为空白".to_owned());
    }
    let layout = resolve(Path::new(&root));
    write::create(&layout, &name, &goal)
}
