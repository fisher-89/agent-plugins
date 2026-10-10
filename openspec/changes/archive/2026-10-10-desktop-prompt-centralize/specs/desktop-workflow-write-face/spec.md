# desktop-workflow-write-face Specification (Delta)

## ADDED Requirements

### Requirement: PhaseAgentSpec 静态化与通配符退役

`PhaseAgentSpec` SHALL 由 `{ agent_type: String, prompt: String, model_level }` 收敛为 `{ prompt: &'static str, model_level }`：`agent_type` 字段随 `__CALL_AGENT:<role>__` 令牌退役删除；prompt SHALL 为编译期静态文本（`include_str!("prompts/<role>.md")` 产物，见 desktop-phase-prompts）。写面 SHALL 删除 `interpolate`（`<change>` / `<phase>` 占位符插值）；`phase_next` 的 `build_phase_response` SHALL 以静态 prompt + 上下文头 `change: <name>`（append）+ 回溯原因行（append）组装，MUST NOT 做模板替换。desktop core（workflow + orchestration）SHALL 零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`。

#### Scenario: 结构收敛与令牌退役

- **WHEN** 审查 `PhaseAgentSpec` 定义与 `phase_table` 表条目
- **THEN** 结构仅含 `prompt: &'static str` 与 `model_level`；无 `agent_type` 字段；无 `__CALL_AGENT` 字样
- **AND** 相位序 / model_level / 白名单 / 前置依赖语义不变

#### Scenario: 无模板替换

- **WHEN** `phase_next` 下发某相位 executor / evaluator prompt
- **THEN** prompt 主体为静态 md 文本，仅 append 上下文头 `change: <name>` 与（backtrack 时）回溯原因行
- **AND** 无 `<change>` / `<phase>` 替换行为

#### Scenario: 删除面零残留

- **WHEN** 全仓扫描 desktop core（workflow / orchestration）源码
- **THEN** 无 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>` 残留

## MODIFIED Requirements

### Requirement: 相位表与 prompt 模板单源收敛

requirement 工作流相位表（相位序、executor / evaluator prompt 模板）SHALL 单源驻 core/workflow：prompt 模板 SHALL 为 `core/workflow/src/prompts/` 全静态角色知识 md（清单与内容约定见 desktop-phase-prompts），经 `phase_table.rs` 编译期 `include_str!("prompts/<role>.md")` 显式字面路径装配，MUST NOT 按 `<role>` 动态读取（无运行时文件 IO / 目录遍历）；walker MUST NOT 自持相位转移规则（每步过渡问写面 `phase_next`）；`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份 MUST NOT 上位为路由权威；TS 侧相位表（`lib/workflow.ts`）冻结，不再是桌面路径依赖，phase_table 与 `lib/workflow.ts` 的 AC-9 镜像对照口径随插件废弃终结。

#### Scenario: walker 不自持转移规则

- **WHEN** 审查 orchestration walker 源码对相位先后关系的编码
- **THEN** 零硬编码相位转移表；walker 每步过渡均以写面 `phase_next` 的返回为准

#### Scenario: 相位表单一来源

- **WHEN** 同一 requirement 工作流的相位序在 core/workflow 与 walker 两侧分别查阅
- **THEN** 仅 core/workflow 相位表定义相位序与 prompt 模板；walker 侧无第二份定义
- **AND** prompt 模板单源 = `prompts/*.md` 静态文件，经 `include_str!` 装配，无第二份 prompt 文本

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs` | requirement 相位表单源 | `PhaseAgentSpec { prompt: &'static str, model_level }`（无 agent_type）；表条目 `include_str!("prompts/<role>.md")` 显式字面路径；删除 `interpolate`；相位序 / model_level / 白名单 / 前置依赖不变 |
| `crates/core/workflow/src/write/phase_next.rs` | 相位路由状态机 | 静态 prompt + append 上下文头 `change: <name>` + 回溯原因行；无 `<change>`/`<phase>` 模板替换；`build_phase_response` 组装面随之调整 |
| `crates/core/workflow/src/prompts/*.md` | 角色知识单一真源 | 全静态零占位，内容约定见 desktop-phase-prompts |
| `crates/core/orchestration`（消费方） | 经进程内直调消费写面 | 依赖方向 orchestration → workflow；walker 零自持转移规则；不再消费 `agent_type` / `interpolate` |
