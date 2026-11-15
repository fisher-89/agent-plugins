# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: Cleanup 步与 worktree 自动清理

archive 相位 SHALL 在 SpecSync 步之后、Seal 步之前执行 `Cleanup` 步，对带 worktree 记录（`ChangeRecord.worktree = Some`）的 change 经 `crates/infra/vcs` 的 worktree 回收单点移除该 worktree（两段式：`git worktree remove --force` 失败即以 OS 层兜底回收并 `prune` 残留 admin，见 desktop-change-worktree「worktree 回收两段式」；`--force` 带走 bootstrap 的 `node_modules` / `target` 残留）；branch `change/<name>` MUST NOT 删除（回收本就不动分支，历史可溯）。Cleanup SHALL 为**尽力步**：清理失败 SHALL 仅以 step detail 警告呈现，步状态 `passed`、链照常收口（Seal / Finalize / PhaseLog 续走——归档必收口，残留是警告不是回滚），MUST NOT 以 `Terminal` 停链、MUST NOT 扩 `ArchiveOutcome` 字段面（golden wire 冻结不动）。清理失败面 SHALL 收窄为「两段式皆失败」（git 与 OS 层删除均失败），且该 `detail` SHALL 携**成因**——`ArchiveVcsPort` 缝透出的 `Err` 原文（worktree 绝对路径 + 原 git 错误 + OS 错误 + 占用成因清单），MUST NOT 只呈现原始 git 错误串、MUST NOT 以「`git worktree list` 单源对账」收尾（已注销孤儿正是 `list` 不显示的那一类，单源指引会让用户无从自救）。Cleanup SHALL 幂等：worktree 目录已不在场（用户手动清理 / 重试链）时 SHALL `passed` + `detail` 注记跳过（MUST NOT 失败）；**已注销孤儿**（git 登记已拆而目录仍在场，如上一次回收半程失败）SHALL 经兜底回收并以 `passed` + `detail` 记「已移除」收口（MUST NOT 因 `not a working tree` 判失败）；legacy 记录（`worktree=None`）SHALL 跳过该步（`passed` + `detail` 记跳过因）。清理的 git 触点 SHALL 经 orchestration 侧 `ArchiveVcsPort` 缝承载（进程执行驻 `crates/infra/vcs`），MUST NOT 进入写面 `archive` 双写本体（desktop-change-state-store「归档双写」零 git 触点红线保持）。清理完成后 `ChangeRecord.worktree` 字段 SHALL 原样保留（历史锚——change 已 archived 无后续操作，字段作创建期执行锚留痕；详情信息行照旧呈现路径，「已清理」注记为 design 可选非必须项）。

#### Scenario: 清理成功收口

- **WHEN** 带 worktree 记录的 change 走 archive 相位至 Cleanup 步成功
- **THEN** worktree 目录自磁盘消失（`git worktree list` 不再列出），Seal 照常双写收口（db status=archived + 主仓目录改名）；`ChangeRecord.worktree` 字段原值保留

#### Scenario: 已注销孤儿经兜底回收成功

- **WHEN** 该 change 的 worktree 已在上一次回收中半程失败（git 登记已拆、目录仍在场，`git worktree list` 不显示），重试链再次走到 Cleanup 步
- **THEN** 回收单点的 OS 层兜底删除该目录，Cleanup 步 `passed` + `detail` 记「已移除 worktree（<path>）」，Seal / Finalize / PhaseLog 照常收口、run 正常完成；`ArchiveOutcome` 字段面零变化

#### Scenario: 两段式皆失败仅警告不断链

- **WHEN** Cleanup 步的回收两段式皆失败（`git worktree remove` 与 OS 层删除均失败，如目录被常驻子进程持有）
- **THEN** Cleanup 步 `passed` + `detail` 携成因警告（worktree 绝对路径 + 原 git 错误 + OS 错误 + 占用成因清单，不含「执行 `git worktree remove --force <path>`」单源指引），Seal / Finalize / PhaseLog 照常收口、run 正常完成；`ArchiveOutcome` 字段面零变化

#### Scenario: worktree 缺席幂等跳过

- **WHEN** 用户已手动删除 worktree 目录后经重试链再次走到 Cleanup 步
- **THEN** Cleanup 步 `passed` + `detail` 注记「worktree 已不在场，跳过清理」，零失败面、链续走

#### Scenario: legacy 记录跳过清理

- **WHEN** 对 `worktree=None` 的存量建档 change 进入 archive 相位
- **THEN** Cleanup 步 `passed` + `detail` 记跳过因，零 `git worktree` 触点，链径直经 Seal / Finalize 收口

#### Scenario: 分支保留可溯

- **WHEN** 归档全链（含 Cleanup 成功与失败两路径）完成后审查主仓 branch 清单
- **THEN** branch `change/<name>` 在案（全程零 `git branch -d` 调用），历史可溯

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/orchestration/src/walker.rs`（`step_cleanup`） | Cleanup 步执行体 | 实现零改动（`StepSettle::passed` + detail 前缀 + `ArchiveVcsPort::remove_worktree` 的 `Err` 原文）；doc 随动：失败面 = 两段式皆失败、成因由 vcs 单点提供、已注销孤儿经兜底回收后记「已移除」 |
| `crates/core/orchestration/src/port.rs`（`ArchiveVcsPort::remove_worktree`） | 归档缝方法 | 签名 `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` 逐字不变 |
| `crates/infra/vcs/src/lib.rs`（`ProcessArchiveVcs::remove_worktree`） | 缝实现 | 纯透传 `git::remove_worktree`（自组手动命令指引退役）；两段式与成因文案归单点，见 desktop-change-worktree「worktree 回收两段式」 |
| `crates/core/orchestration/src/walker_test.rs` | Cleanup 面覆盖 | 失败 detail 的断言形态随动（携成因原文 + 不含单源指引）；结构断言（步恒 `passed` / Seal / Finalize / branch 零触碰）零改动；新增「已注销孤儿回收 `passed`」断言 |
| `packages/desktop/package.json` | 版本交付 | 落地时 `HEAD` 现值 + 1 patch（现值 0.6.42 → 0.6.43）；`src-tauri/Cargo.toml` 不随动 |
