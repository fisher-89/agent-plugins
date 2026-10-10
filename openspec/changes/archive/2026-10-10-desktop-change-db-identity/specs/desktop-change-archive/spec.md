# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 提供归档入口（按钮）：db `ChangeRecord.status=active` 的 change 可见可用（按 change id 寻址）；已归档（`source=archive` / status=archived）SHALL NOT 呈现入口；无 db 建档的 change（id 不可达——存量 CLI change 条目已不入清单，见 desktop-change-queries）SHALL NOT 提供归档（写面既有拒绝面沿袭）。点击 SHALL 先呈现确认对话再发起，对话 SHALL 呈现：完成度核对结论（以该 change id 的 db PhaseRecord eval 序列确定性核算——workflow 相位表全部相位存在 non-stale 的 pass / skipped 条目即完成，MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支」的告知。警告 SHALL NOT 阻断（inform + confirm 护栏为本能力自持语义），用户显式确认后照常发起；无警告时确认为轻量确认。该 change 存在运行中 run（运行注册表复合键 `(workspace root, change id)` 在案）时发起归档 SHALL 显式拒绝。

#### Scenario: 未完成警告确认后照常归档

- **WHEN** 打开一个存在 fail 相位（无全相位 non-stale pass）的 active change，点击归档并在确认对话中选择确认
- **THEN** 对话呈现「工作流未全部通过」警告，确认后归档链照常发起并走通收口；警告不构成阻断

#### Scenario: 已归档与无建档 id 无入口

- **WHEN** 打开已归档 change（archive 树条目）详情，并审查无 db 记录的 id（或存量 CLI change 目录）情形
- **THEN** 已归档详情无归档按钮；无建档 id 即使经命令面直调也被写面既有拒绝面拒绝（零落账、零目录改动）

#### Scenario: 运行中 run 拒绝归档

- **WHEN** 某 change（复合键 `(root, id)` 在案）的 run 运行中时点击归档发起
- **THEN** 显式拒绝（呈现运行中原因），归档链零阶段执行、db 与磁盘零变化

### Requirement: spec 同步 agent 与归档链语义自持

delta specs 在场且用户未选择跳过时，归档链 SHALL 在主仓合入段之后发起一个 spec 同步 agent 会话：会话 cwd = 主 workspace root（worktree change 与 legacy 同锚——worktree change 的 delta specs 已随合入进入主仓 active 树，legacy 本就在主仓），同步编辑直接落在主仓主基线。prompt SHALL 逐 capability 将 delta spec 增量合并进 `openspec/specs/<capability>/spec.md` 主基线：`ADDED` 追加（requirement 已存在则更新为与 delta 一致）/ `MODIFIED` 增量应用（新增 scenario、修改列中 scenario、修订描述，不复制既有 scenario）/ `REMOVED` 整块移除 / `RENAMED` 改名，保留 delta 未提及的主 spec 内容，合并幂等（对已同步基线重跑零变化）；capability 主 spec 缺席时创建（简 Purpose + ADDED requirements）。以上增量合并语义 SHALL 以本 requirement 为单源定义（归档链为唯一定义源），spec 与 prompt MUST NOT 引用 CLI skill 文档作为语义或护栏溯源；prompt MUST NOT 依赖 MCP 工具、`__TOOL_ASK_USER__` 或 workflow.json，change 名与路径上下文 SHALL 由桌面插值。会话 SHALL 走既有 WorkerAgent 通道（compose_turn 新会话、CLI 引擎、bypassPermissions、provenance `source="change"` 且 source_ref 携 change id 与归档语义段——`<id>/archive/spec-sync` 形，定式由 design 定稿）；转录可观测（实时 + 重放一致）且可停止（StopRegistry 既有终止面）；路径沙箱前缀校验以主仓 root 为锚，同步编辑囚于主仓内。agent 失败或被停止时链 SHALL 停在同步段：零归档变更（db 零变化、归档树零改名；已完成的提交与合入不回滚），可重试（同步幂等，重试自同步段续走）。

#### Scenario: 同步基线为主仓当前 specs

- **WHEN** 主仓 `openspec/specs/<capability>/spec.md` 已被先行的其他 change 归档更新（内容超出本 change worktree 创建基线）
- **THEN** 同步以主仓当前内容为合并基线（零冻结基线合并产物、零 git 层 spec 反向冲突）

#### Scenario: 同步产物由落盘段提交

- **WHEN** 带 delta specs 的 worktree change 走归档链至收口
- **THEN** 合入只引入 change 内容（零同步产物——分支重放提交与快进均不含同步产物）；同步产物落主仓工作区，经落盘段扩围 pathspec 提交（见「双写收口与结果摘要」）

#### Scenario: 无 delta specs 零 agent 会话

- **WHEN** 对无 `specs/` 子树的 change 发起归档链
- **THEN** 全程零归档 agent 会话记录（provenance 反查为空），链直落 git 段与双写收口；结果摘要 specs 行呈现「无 delta specs」语义

#### Scenario: 增量合并保真与幂等

- **WHEN** agent 对含 `## MODIFIED Requirements` 的 delta spec 执行同步，随后同一链重试再同步一次
- **THEN** 主 spec 仅按增量语义变化（delta 未提及的 requirement 与 scenario 原样保留），两次同步结果一致（幂等）

#### Scenario: agent 失败停链可重试

- **WHEN** 同步 agent 会话以失败收场（或运行中被用户停止）
- **THEN** 链停在同步段：db 与归档树零变化（已落主仓的提交与合入保留不回滚）；错误呈现后重试自同步段续走（合入段已合入跳过、同步幂等）成功至收口

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/archive_flow.rs`（键与 provenance） | 归档链 id 寻址 | `ArchiveControl` / `ArchiveRequest` / `ArchiveGuard` 键 `(root, change)` → `(root, id)`；分支判定 `change/<name>` 与 prompt 内文经 id → 记录 → name 供给（git 面零改名） |
| `crates/core/orchestration/src/archive_flow.rs`（agent 两会话） | provenance 身份段换锚 | spec 同步 `source_ref=<id>/archive/spec-sync`、解冲突 `source_ref=<id>/archive/merge-conflict`（形不变、身份段 id；prompt 内文仍插值 name） |
| `src/commands/archive_flow/` | 薄命令包装 id 化 | preflight / start / stop / state / watch 定位参数 == change id；互斥校验键 `(root, id)`；缺 id 记录 → 既有拒绝面（`Result<T, String>`） |
| `packages/desktop/src/views/changes/flow/archive-panel.tsx` + `hooks/use-archive-flow.ts` | 面板 id 透传 | 命令携 change id；完成度 / 摘要 / 归档目录（`archivedDir` 含日期前缀）展示面语义不变（name 展示值取自 detail） |
