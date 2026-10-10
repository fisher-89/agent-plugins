# desktop-change-orchestration Specification (Delta)

## ADDED Requirements

### Requirement: 相位 prompt 组装面静态化（静态主体 + 动态 append）

executor / evaluator 会话 prompt SHALL 以写面下发的静态角色知识文本（desktop-phase-prompts / desktop-workflow-write-face）为主体；动态面 SHALL 全部 append 而非模板替换：上下文头一行 `change: <name>`、回溯原因行、static-check / 测试修复失败注入、输出协议附录（evaluator）、decision 三列表（决策会话）。prompt 组装 MUST NOT 使用 `<change>` / `<phase>` 模板替换。`WorkerTurnRequest.provenance.source_ref = <change>/<phase>/<role>/<attempt>` 与 `root` / `model_level` 结构化通道不变——phase / attempt 盖戳不依赖 prompt 文本。

#### Scenario: 上下文头在场

- **WHEN** walker 发起 executor / evaluator 会话
- **THEN** prompt 含一行 `change: <name>`（change 名自写面记录 name 供给）
- **AND** prompt 主体为静态角色知识文本，无 `<change>` / `<phase>` 模板替换痕迹

#### Scenario: 反馈注入仍 append

- **WHEN** static-check 或 test-execution 失败走定向反馈边
- **THEN** 修复反馈文本以 append 注入同一 executor 会话 prompt（不重写静态主体）
- **AND** 回溯场景的回溯原因行照常 append

### Requirement: 角色要点静态化（role_brief 退役）

`core/orchestration/src/prompt.rs` SHALL 删除 `role_brief` 15 行静态表与 `strip_call_agent`（角色自述已溶解进 desktop-phase-prompts 各 md）。`executor_prompt` SHALL 不再前置「你以角色…」+ role_brief 段，改为直出静态 prompt 主体（不再消费 `agent_type`）；`evaluator_prompt` SHALL 仅追加输出协议附录。`prompt.rs` MUST NOT 残留按角色名分发要点的逻辑。

#### Scenario: role_brief 退役

- **WHEN** 审查 `prompt.rs` 与 executor prompt 组装
- **THEN** 无 `role_brief` 表、无 `strip_call_agent`、无 `__CALL_AGENT` 解析
- **AND** executor prompt 主体直出静态 md 文本（角色自述已含于其中）

#### Scenario: evaluator 协议仍唯一追加

- **WHEN** 组装 evaluator prompt
- **THEN** 仅追加输出协议附录，无角色要点前导、无 `__CALL_AGENT` 相关处理

### Requirement: evaluator 输出协议瘦身

`evaluator_protocol` 输出协议附录 SHALL 要求 evaluator 最终消息输出且仅输出一个 checklist JSON 对象，形状为 `{ verdict: "pass" | "fail", report: <≤2000 字符>, checklist: [{ item, pass, evidence }] }`；MUST NOT 要求 phase / attempt / skipped 回声（由 walker 按 provenance 盖戳）。协议 SHALL 保留禁调 MCP 写通道红线（phase_log / phase_next / phase_start / backtrack，落账由桌面编排代写）；checklist 至少一项、pass 为布尔、evidence 必须给事实依据。

#### Scenario: 协议形状瘦身

- **WHEN** 读取 evaluator 输出协议附录文本
- **THEN** 要求形状仅含 verdict / report / checklist 三键
- **AND** 无「输出 phase 字段」「输出 attempt 字段」「输出 skipped 字段」要求
- **AND** 仍含禁调 MCP 写通道红线

## MODIFIED Requirements

### Requirement: verdict 解析与 phase-log 代写

Evaluator 会话 SHALL 依 prompt 约定在最终消息输出 checklist JSON（不自调 MCP `phase_log`）。Walker SHALL 解析该 JSON 并代调写面 `phase_log`（进程内）落 eval 记录（PhaseRecord + ChecklistItemRecord 原子落库）；解析失败 SHALL 将运行停给用户（呈现原始输出与失败原因），MUST NOT 臆测 verdict、MUST NOT 静默跳过落账。checklist JSON 结构 SHALL 瘦身为 `{ verdict, report, checklist }`（checklist 逐项 `{ item, pass, evidence }`）；phase / attempt / skipped SHALL 由 walker 盖戳落账（evaluator 不回声），`EvaluatorChecklist` 解析类型 MUST NOT 要求 phase / attempt / skipped 字段。

#### Scenario: verdict 代写落账

- **WHEN** evaluator 最终消息输出合法 checklist JSON（verdict=pass，无 phase / attempt / skipped 字段）
- **THEN** walker 以该 checklist 盖 phase / attempt / skipped 后经写面 `phase_log` 落 eval 记录，`phase_next` 随后推进

#### Scenario: 解析失败停给用户

- **WHEN** evaluator 最终消息不含可解析的 checklist JSON（结构漂移或空输出）
- **THEN** run 停止并呈现 evaluator 原始输出与解析失败原因，状态库无新增 eval 记录，用户可显式重试该 attempt

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.31` 升级为 `0.4.32`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物），仅于仓库根 `AGENT.md` 顶部加 DEPRECATED 注记（插件端停止迭代、已安装副本照常可用、未来删除全部代码），MUST NOT 改任何插件功能代码。

#### Scenario: 版本号升级与插件废弃注记

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.32；`plugins/dev-team` 版本与交付产物零改动；根 `AGENT.md` 顶部含 DEPRECATED 注记且插件功能代码零改动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/prompt.rs` | executor / evaluator / decision prompt 组装 | 删除 `role_brief` / `strip_call_agent`；`executor_prompt` 直出静态主体；`evaluator_prompt` 仅追加输出协议附录（瘦身为 `{verdict, report, checklist}`）；`decision_prompt` 三列表不变 |
| `crates/core/orchestration/src/verdict.rs` | verdict 解析 | `EvaluatorChecklist` 去 `phase` / `attempt` / `skipped`（walker 盖戳）；report ≤2000 门不变 |
| `crates/core/orchestration/src/walker.rs` | 会话发起与动态 append | executor / evaluator 会话 prompt = 静态主体 + 上下文头 `change: <name>` + 动态面 append（回溯原因 / 反馈注入 / 输出协议）；decision 会话沿用 `decision_prompt` |
