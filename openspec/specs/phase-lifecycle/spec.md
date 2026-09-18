# phase-lifecycle Specification

## Purpose

管理 phase 运行态生命周期：`phase_start` 显式开启运行态（last-wins 写入 `active_phase`），`phase_log` 落盘盖章 `start_at` 并清空运行态，UserPromptSubmit sweep 收口中断遗留；技能协议以 `phase_next → phase_start → executor → phase_log` 序列执行，为 per-attempt 耗时推导与文件归账门控提供数据契约。

## Requirements

### Requirement: workflow.json 持有 active_phase 运行态

`workflowFileSchema` SHALL 扩展可选字段 `active_phase`,取值为 `null` 或对象 `{ phase: string, attempt: number, start_at: string }`——`phase` 为该工作流类型 phase 表中的阶段标识,`attempt` 为正整数(与 `phase_log` 的 attempt 语义一致),`start_at` 为 ISO 8601 时间戳。SHALL 同步扩展可选字段 `interrupted: Array<{ phase: string, attempt: number, start_at: string, end_at: string }>` 作为被 sweep 关闭的 phase 留档。字段缺失、为 null 与空数组均表示"无运行中 phase"。未知键保留策略不变;既有 `workflow_type` / `created` / `eval` 字段行为不变。

#### Scenario: 含 active_phase 的文件通过校验

- **WHEN** 解析含 `active_phase: { phase: "implement", attempt: 2, start_at: "<iso>" }` 的 `workflow.json`
- **THEN** `workflowFileSchema` 校验通过,既有字段与未知键行为不变

#### Scenario: 无 active_phase 等价于无运行态

- **WHEN** 解析不含 `active_phase` 字段的 `workflow.json`
- **THEN** 校验通过,读方视其为 null(无运行中 phase)

### Requirement: phase_start MCP 工具显式开启运行态

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `phase_start` 的 MCP 工具(命名遵循 `mcp-tool-namespace` 的 `xx_yy` 规则),输入至少含:

- `change`:`string`,必填,目标 change 名
- `phase`:`string`,必填,要开启的阶段标识
- `project_root`:`string`,可选(经既有 `withResolvedProjectRoot` 解析注入)

工具 SHALL 校验 change 存在且 `workflow.json` 通过 `workflowFileSchema`,`phase` 属于该 `workflow_type` 的 phase 表(否则报错且 workflow.json 不变)。通过校验后 SHALL 以 last-wins 方式写入 `active_phase: { phase, attempt, start_at }`:`start_at` 取当前时刻;`attempt` 由既有 eval 历史按与 `phase_log` 相同的推导规则得出——retry 重跑在上一 fail 条目落盘后重新调用,attempt 自然递增。重复调用 SHALL 覆盖既有 active_phase(可重入)。输出 SHALL 为 `{ started: true, phase, attempt, start_at }`。`phase_next` 的输入协议与只读语义 MUST NOT 改变。phase_start 调用自身触发的 PostToolUse 事件对 session 注册表的作用见 workflow-file-inventory 规格。

#### Scenario: 开启运行态

- **WHEN** 对 change `my-change` 调用 `phase_start(change: "my-change", phase: "implement")`
- **THEN** `workflow.json` 的 `active_phase` 为 `{ phase: "implement", attempt: <n>, start_at: <iso> }`
- **AND** 返回 `{ started: true, phase: "implement", attempt: <n>, start_at: <iso> }`

#### Scenario: 非法 phase 报错且文件不变

- **WHEN** 调用 `phase_start` 传入不属于该工作流 phase 表的 `phase`
- **THEN** 返回错误且 `workflow.json` 逐字节不变

#### Scenario: retry 后 attempt 递增

- **WHEN** `implement` attempt 1 的 fail 条目已落盘,技能按协议重跑并再次调用 `phase_start(phase: "implement")`
- **THEN** 写入的 `active_phase.attempt` 为 2

### Requirement: phase_log 落盘盖章 start_at 并清空 active_phase

`phase_log` SHALL 在 `appendEntry` 成功后:当 `active_phase` 存在且其 `phase` 与本次落盘 `phase` 一致时,给该条目盖章 `start_at`(取 `active_phase.start_at`)并清空 `active_phase`(置 null);当 `active_phase` 缺失或 `phase` 不一致时,条目 MUST NOT 盖章且 `active_phase` MUST NOT 被改动(防御式;异常遗留由 UserPromptSubmit sweep 兜底回收)。`phaseLogSchema` SHALL 扩展可选字段 `start_at`(ISO 8601);既有 `timestamp` 字段语义不变,即条目的结束时刻(end_at)。工具输出 `{ written, phase, attempt }` 形状 MUST NOT 改变。

#### Scenario: 匹配时盖章并清场

- **WHEN** `active_phase` 为 `{ phase: "implement", attempt: 2, start_at: t0 }`,`phase_log` 落盘 `phase: "implement"` 的 pass 条目
- **THEN** 该条目含 `start_at: t0` 且 `timestamp` 为落盘时刻,`active_phase` 被清空

#### Scenario: 无 active_phase 时照常落盘

- **WHEN** 无 `active_phase` 时 `phase_log` 落盘条目
- **THEN** 条目不含 `start_at`,落盘成功,输出形状不变

### Requirement: per-attempt 耗时由既有字段推导

每个 attempt 的耗时 SHALL 可由该条目的 `timestamp − start_at` 推导,status SHALL 由既有 verdict / stale / skipped 字段推导;系统 MUST NOT 为耗时引入独立存储字段。本变更 SHALL NOT 新增耗时消费方命令(数据契约先行,消费方由后续 change 增量处理)。

#### Scenario: 耗时可推导

- **WHEN** 某 eval 条目含 `start_at: t0` 与 `timestamp: t1`
- **THEN** 该 attempt 耗时即 `t1 − t0`,无需额外字段

### Requirement: UserPromptSubmit sweep 收口中断遗留

构建产物 SHALL 注册 UserPromptSubmit hook 执行 sweep 子命令(claude 产物注册;cursor 产物 MUST NOT 注册——平台不支持该事件时降级保留中断窗口,属明确接受)。hook 子命令 SHALL:`session_id` 在注册表绑定 change 且该 change 的 `workflow.json` 存在遗留 `active_phase` 时,关闭它——以 `end_at` 为当前时刻追加进 `interrupted[]` 并清空 `active_phase`;MUST NOT 写 eval 条目(避免烧 retry 配额)。未绑定或无遗留时 SHALL 为 no-op。任何错误 SHALL 仅留 stderr 诊断并以 0 退出,MUST NOT 阻塞用户消息(fail-open,与记录器策略一致)。`hooks.canonical.json` SHALL 新增 `userPromptSubmit` 事件键,`build/hooks-profile.ts` SHALL 支持该键的双平台包装(claude 侧无 matcher 概念;cursor null 不产出)。

#### Scenario: 中断遗留被收口

- **WHEN** session 已绑定 change 且其 `workflow.json` 遗留 `active_phase`,新用户消息触发 sweep
- **THEN** 该 active_phase 以 `end_at` 移入 `interrupted[]`,`active_phase` 清空,`eval` 数组不变

#### Scenario: 未绑定为 no-op

- **WHEN** 触发 sweep 的 `session_id` 未绑定任何 change
- **THEN** `workflow.json` 不被改写,进程以 0 退出

#### Scenario: 失败不阻塞

- **WHEN** sweep 过程中注册表或 workflow.json 读取失败
- **THEN** 仅 stderr 留诊断,进程以 0 退出,用户消息不受阻塞

### Requirement: 技能协议增加 phase_start 调用

每个 phase 技能(`plugins/dev-team/skills/phase-*/SKILL.md`)SHALL 在 turn 开头 `phase_next` 返回目标 phase 之后、执行 executor 之前调用一次 `phase_start`;retry 重跑同样 SHALL 重新 `phase_start`(attempt 计时与归账门控均以 start 为准)。Verdict 阶段的 `phase_next`(返回 `done` 或错误终态)SHALL NOT 触发 `phase_start`。技能文本 MUST NOT 引导在无 `phase_next` 的普通 turn 调用 `phase_start`。编排技能(`workflow-requirement` / `workflow-test-only`)如内嵌协议文本 SHALL 同步修改。

#### Scenario: 单 phase turn 的协议序列

- **WHEN** 按技能协议执行一个 phase turn
- **THEN** 调用序列为 phase_next → phase_start → executor 写操作 → phase_log

#### Scenario: retry 重跑重新开启

- **WHEN** 某 phase 的 evaluator fail 后技能按 retry 协议重跑
- **THEN** 重跑先重新调用 phase_start 再执行 executor

## Module Contract

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段 | 值 |
|------|-----|
| 工具名 | `phase_start` |
| 输入 | `{ change: string, phase: string, project_root?: string }` |
| 输出 | `{ started: true, phase: string, attempt: number, start_at: string }` |
| handler | 解析 project_root → 校验 change 与 phase 表归属 → last-wins 写 `active_phase` → 返回 |

### Module: `plugins/dev-team/bin/src/commands/`

| 项 | 值 |
|----|-----|
| `phase-start.ts`(新) | `runPhaseStart(options)` — 校验 + 写 active_phase,返回 PhaseStartResult |
| `phase-log.ts`(改) | `runPhaseLog` — appendEntry 成功后按匹配盖章 `start_at` 并清空 active_phase;输出形状不变 |
| sweep 子命令(新,如 `sweep-phase.ts`) | UserPromptSubmit hook 入口:读注册表绑定 → 关闭遗留 active_phase 入 `interrupted[]`;fail-open exit 0 |
| `hooks.ts` | 注册 sweep 子命令分发 |

### Module: `plugins/dev-team/build/hooks-profile.ts`

| 项 | 值 |
|----|-----|
| canonical schema | 增 `userPromptSubmit: Array<{ matchers, commandTemplate }>` 键 |
| claude 包装 | 产出 `UserPromptSubmit`(无 matcher;matcher 字段省略) |
| cursor 包装 | `matchers.cursor` 为 null 时该事件不产出(降级) |

### 下沉与共享

| 项 | 值 |
|----|-----|
| `hasPhasePassed` | 自 `commands/phase-next.ts` 下沉至 `lib/`(落点 design 定),供 `phase_log` / sweep 与未来消费方直引;`phase_next` 行为不变 |
| `executor.agent_type` 解析 | phase 表 token(`__CALL_AGENT:...__`)→ 当前平台实际 agent 名的解析辅助,供门③消费(归账侧) |
