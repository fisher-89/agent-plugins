# desktop-change-archive（delta：SpecSync 会话引擎措辞随 CLI 退役）

## MODIFIED Requirements

### Requirement: spec 同步 agent 与归档链语义自持

delta specs 在场且用户未选择跳过时，SpecSync 步 SHALL 在 Merge 步之后发起一个 spec 同步 agent 会话：会话 cwd = 主 workspace root（worktree change 与 legacy 同锚——worktree change 的 delta specs 已随合入进入主仓 active 树，legacy 本就在主仓），同步编辑直接落在主仓主基线。prompt SHALL 逐 capability 将 delta spec 增量合并进 `openspec/specs/<capability>/spec.md` 主基线：`ADDED` 追加（requirement 已存在则更新为与 delta 一致）/ `MODIFIED` 增量应用（新增 scenario、修改列中 scenario、修订描述，不复制既有 scenario）/ `REMOVED` 整块移除 / `RENAMED` 改名，保留 delta 未提及的主 spec 内容，合并幂等（对已同步基线重跑零变化）；capability 主 spec 缺席时创建（简 Purpose + ADDED requirements）。以上增量合并语义 SHALL 以本 requirement 为单源定义（归档相位为唯一定义源），spec 与 prompt MUST NOT 引用 CLI skill 文档作为语义或护栏溯源；prompt MUST NOT 依赖 MCP 工具、`__TOOL_ASK_USER__` 或 workflow.json，change 名与路径上下文 SHALL 由桌面插值。会话 SHALL 走既有 WorkerAgent 通道（compose_turn 新会话、SDK 引擎（组合根默认 agent 解析——CLI 租户已随 retire-cli-engine 退役，SpecSync 会话由 SDK 租户承载）、bypassPermissions、provenance `source="change"` 且 source_ref 携 change id 与归档步语义段——定式由 design 定稿，创建时写入并持久化于 `SessionProvenance`）、转录可观测（实时 + 重放一致）且可停止（StopRegistry 既有终止面）；路径沙箱前缀校验以主仓 root 为锚，同步编辑囚于主仓内。前端转录联动 SHALL 经该归档步节点 `session_id` 直查（`source_ref` 定式不再作前端反向寻址依据，见 desktop-change-flow-view「节点会话转录联动」）。agent 失败或被停止时该步 SHALL `Terminal`（零归档变更：db 零变化、归档树零改名；已完成的提交与合入不回滚），可重试（同步幂等，重试自 SpecSync 步续走）。无 delta specs 或用户选择跳过时 SpecSync SHALL 零会话、`passed` + `detail` 记跳过因。

#### Scenario: 同步基线为主仓当前 specs

- **WHEN** 主仓 `openspec/specs/<capability>/spec.md` 已被先行的其他 change 归档更新（内容超出本 change worktree 创建基线）
- **THEN** 同步以主仓当前内容为合并基线（零冻结基线合并产物、零 git 层 spec 反向冲突）

#### Scenario: 同步产物由落盘步提交

- **WHEN** 带 delta specs 的 worktree change 走 archive 相位至收口
- **THEN** Rebase / Merge 只引入 change 内容（零同步产物——分支重放提交与快进均不含同步产物）；同步产物落主仓工作区，经 Finalize 步扩围 pathspec 提交（见「Seal 与 Finalize 收口」）

#### Scenario: 无 delta specs 零 agent 会话

- **WHEN** 对无 `specs/` 子树的 change 进入 archive 相位
- **THEN** 全程零归档 agent 会话记录（run 节点行与 PhaseRecord 会话槽位皆空、无 provenance 为 change 归档步的会话），SpecSync 步 `passed` + 跳过因，链直落 Seal / Finalize 收口

#### Scenario: 增量合并保真与幂等

- **WHEN** agent 对含 `## MODIFIED Requirements` 的 delta spec 执行同步，随后同一相位重试再同步一次
- **THEN** 主 spec 仅按增量语义变化（delta 未提及的 requirement 与 scenario 原样保留），两次同步结果一致（幂等）

#### Scenario: agent 失败停链可重试

- **WHEN** 同步 agent 会话以失败收场（或运行中被用户停止）
- **THEN** SpecSync 步 `Terminal`：db 与归档树零变化（已落主仓的提交与合入保留不回滚）；错误呈现后重试自 SpecSync 步续走（已合入跳过、同步幂等）成功至收口

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| SpecSync 会话发起面（walker 通道，不改） | 归档步 spec 同步会话 | 会话引擎措辞由「CLI 引擎」改为「SDK 引擎（组合根默认 agent 解析）」——机制（compose_turn 新会话、bypassPermissions、provenance 定式）零变化，仅租户口径随 retire-cli-engine 更新 |
