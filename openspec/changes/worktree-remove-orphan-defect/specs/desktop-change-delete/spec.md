# desktop-change-delete Specification (Delta)

## MODIFIED Requirements

### Requirement: 写面删除与执行序

core/workflow SHALL 提供 sync 写面 `write::delete(main_root, layout, store, vcs, id, clean_worktree) -> Result<DeleteOutcome, String>`，按 change **id** 寻址（`get_change(id)` miss → 显式 `Err`，零副作用），对解析出的 `record.name` 施单分量名校验（与 archive / create 同一单点）。执行序 SHALL 为**磁盘先、库最后**（沿 archive 的「先改名后翻库」哲学）：

1. **worktree 清理（条件步）**：仅当 `clean_worktree = true` 且 `record.worktree = Some` 时执行——目录在场则 `WorktreePort::remove_worktree`（worktree 回收单点，两段式：`git worktree remove --force` 失败即以 OS 层兜底回收并 `prune`，见 desktop-change-worktree「worktree 回收两段式」——**已注销孤儿**（git 登记已拆而目录仍在场）因此同样可回收，勾选「同时清空 worktree」不再永续失败），目录缺席则跳过（重试链幂等）；随后 branch `change/<name>` 经 `branch_exists` 命中才 `delete_branch`（**不随目录缺席跳过**——残留分支会撞 create 前置⑥「branch 已存在」）。任一 vcs 步 `Err` → 整体 `Err`（库记录与产物目录原样未动）；该 `Err` 的成因面 SHALL 收窄为「回收两段式皆失败」，文案 SHALL 携路径与成因（原 git 错误 + OS 错误 + 占用成因清单，由 vcs 单点提供），MUST NOT 只呈原始 git 错误串。
2. **主仓产物目录清理（恒执行）**：候选定位序 = 归档记录的自身 `archived_at` 推导目录 `<archive_root>/<YYYY-MM-DD>-<name>` → `locate_change` 的 active → archive 精确 → 日期前缀后缀扫描；命中即 `remove_dir_all`，失败 → `Err`（携路径）。
3. **级联删库（最后一步）**：`ChangeStateStore::delete_change_cascade(id)`；`Err` 原样透出（此时目录已删，重试由步骤 1/2 的缺席跳过收敛，仅补库半边）。

`DeleteOutcome` SHALL 携 `name` / `removed_worktree`（本次是否实际移除 worktree 目录）/ `warnings`（未勾选而目录仍在场时的保留注记）。各步 SHALL 幂等可重入；写面 MUST NOT 自行停止或观察 run（并发护栏归命令层）。git 触点 SHALL 全经 `WorktreePort` 缝（进程 spawn 驻 `crates/infra/vcs`），写面零直接 spawn。

#### Scenario: 勾选清空后同名可重建

- **WHEN** 对带 worktree 记录的 active change 以 `clean_worktree = true` 调用删除
- **THEN** worktree 目录消失、`git worktree list` 不再列出、branch `change/<name>` 删除、主仓产物目录删除、库记录级联删除；随后以同名创建 change 成功（create 前置⑥/⑦不再撞）

#### Scenario: 已注销孤儿经删除面回收

- **WHEN** 记录的 worktree 已成已注销孤儿（git 登记已拆、目录仍在场，`git worktree list` 不显示），以 `clean_worktree = true` 调用删除
- **THEN** 回收单点以 OS 层兜底删除该目录，`removed_worktree = true`；branch `change/<name>` 按既有判据删除、主仓产物目录删除、库记录级联删除（无 `Err`、无需用户手动干预）

#### Scenario: 不勾选保留 worktree 并给出注记

- **WHEN** 以 `clean_worktree = false` 删除带 worktree 记录的 active change
- **THEN** worktree 目录与 branch 原样保留，`warnings` 携保留注记（含路径与分支名）；主仓产物目录与库记录照常删除

#### Scenario: worktree 目录已缺席仍清分支

- **WHEN** 记录的 worktree 目录已被用户手动删除，以 `clean_worktree = true` 删除
- **THEN** 目录步幂等跳过、`removed_worktree = false`，branch `change/<name>` 若在场仍被删除

#### Scenario: 回收两段式皆失败即中断

- **WHEN** worktree 回收两段式皆失败（`git worktree remove` 与 OS 层删除均失败，如目录被常驻子进程持有）
- **THEN** 写面显式 `Err`（含路径与成因：原 git 错误 + OS 错误 + 占用成因清单）；库记录与主仓产物目录原样未动，branch 零触碰

#### Scenario: 归档条目按自身日期前缀命中

- **WHEN** 删除一条 archived 记录（`archived_at` 在场、磁盘目录为 `YYYY-MM-DD-<name>`）
- **THEN** 删除对象为该记录日期前缀命中的目录（同名归档共存时各自删自己那份），不留残余目录

#### Scenario: 归档条目与 legacy 记录的清理边界

- **WHEN** 删除归档条目（无清空勾选）或 legacy 记录（`worktree = None`）
- **THEN** 二者均不清空 worktree / 分支（归档路径的分支保留为历史可溯，见 desktop-change-worktree V1 边界 #1；legacy 记录无 worktree 可清）；`clean_worktree` 对无 worktree 记录零作用。归档路径遗留的 branch `change/<name>` 仍由 create 前置⑥的错误文案引导手动清理

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/workflow/src/write/delete.rs` | 删除执行序 | 实现零改动（第 1 步仍 `WorktreePort::remove_worktree` + `is_dir` 缺席跳过）；语义随动：`Err` 成因面收窄为两段式皆失败、成因文案由 vcs 单点提供；`DeleteOutcome` 三字段与级联范围不变 |
| `crates/core/workflow/src/write/worktree.rs`（`WorktreePort::remove_worktree`） | 写面 → vcs 缝 | 签名 `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` 逐字不变 |
| `crates/infra/vcs/src/git.rs`（`remove_worktree`） | 两段式回收实现 | 见 desktop-change-worktree「worktree 回收两段式」与本 spec 执行段一；`Ok` 即「磁盘已无该目录」的契约不变 |
| `crates/core/workflow/src/write/delete_test.rs` | 删除面覆盖 | 新增：孤儿回收 `Ok` → `removed_worktree = true` 且链续走；两段式皆失败 `Err` → 整体 `Err`、库记录与产物目录未动；既有六条用例零改动通过 |
| `packages/desktop/package.json` | 版本交付 | 落地时 `HEAD` 现值 + 1 patch（现值 0.6.42 → 0.6.43）；`src-tauri/Cargo.toml` 不随动 |
