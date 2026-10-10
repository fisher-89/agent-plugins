# 设计: desktop-prompt-centralize

> **变更**: desktop-prompt-centralize
> **日期**: 2026-10-10

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（3 能力 delta：desktop-phase-prompts / desktop-workflow-write-face / desktop-change-orchestration）已由提案阶段定稿；本设计不重复其内容、不将其列为待办，只在 proposal 四项「待决问题」之上逐项定稿（D1–D4），并补设计面决策 D5–D9。

改动基线核实在场（逐点可查）：

- 相位表单源与 `PhaseAgentSpec { agent_type, prompt, model_level }`（`crates/core/workflow/src/write/phase_table.rs`），`interpolate`（`phase_table.rs:206`）为全桌面唯一模板替换；`proposal_explore_handoff()` 经 `domain_dir_name()` 动态取域根目录名（layout 命名隔离双禁令的既有挂点）；
- `build_phase_response`（`phase_next.rs:223`）以 `interpolate(&spec.prompt, name, Some(def.id))` + 回溯原因后缀组装；`PhaseNextOutcome.executor/evaluator: Option<PhaseAgentSpec>`；
- `role_brief` 15 行静态表 + `strip_call_agent` + `executor_prompt`（角色前导）+ `evaluator_prompt`（追加协议附录）+ `evaluator_protocol`（含 phase/attempt/skipped 回声要求）（`crates/core/orchestration/src/prompt.rs`）；
- `EvaluatorChecklist { phase, attempt, verdict, report, checklist, skipped }`（`crates/core/orchestration/src/verdict.rs`）；walker 落账 `step_verdict_phase_log` 已单点盖戳 `phase`（walker 形参）/ `skipped: false`（硬编码）/ attempt（phase_log 写面推导），**从不消费** `checklist.phase/.attempt/.skipped`；
- 插件端 14 个相位角色 agent 描述（`plugins/dev-team/agents/*.md`，含 `architecture.md` / `code-analyze-*.md` 三个非相位角色，不迁移）；根 `AGENT.md` 现无废弃注记；`packages/desktop/package.json` version = 0.4.31。

**关键既有约束（设计必须绕开的）**：

1. **layout 命名隔离双禁令**：产品 `.rs` 源码不得含 `openspec` / `config.json` 字面量，唯一合法触点为 `foundation/src/layout/mod.rs`（`mod_test.rs` 的 `全包产品源码双禁令扫描` 只扫 `.rs`、排除 `*_test.rs`）。——本变更 14 个 md 属 `.md` 内容文件，不在扫描面；`proposal_explore_handoff()` 退役后 phase_table.rs 不再含动态域名取用。
2. **store 零改动红线**：`AgentRunRecordV3.prompt` 只剩升级链解码用途，转录走 `SessionEventRecord` 事件流——本重构对 store schema 零改动，不触 golden wire contract。
3. **相位机语义零漂移**：相位序 / model_level / 白名单 / 前置依赖 / 重试上限 / attempt 计时 / eval 落账 / stale 传播全部不变（AC-8）。
4. **插件零功能改动**：`plugins/dev-team` 版本 2.10.44 与三类交付产物不动，仅根 `AGENT.md` 加废弃注记。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 角色知识集中地（新） | 14 个全静态 md（6 executor + 8 evaluator）作为 executor / evaluator 会话 prompt 主体的单一真源；每文件 = 角色自述 + 阶段要求（输入/过程/输出/约束）+ 静态 checklist（仅 evaluator） | `crates/core/workflow/src/prompts/*.md`（新增） | 无（纯静态文本） | md 纯文本，`include_str!` 编译期嵌入 |
| 相位表装配 | `PhaseAgentSpec { prompt: &'static str, model_level }`；表条目以 `include_str!("../prompts/<role>.md")` 显式字面路径装配；`interpolate` / `proposal_explore_handoff` 退役；`OnceLock<Vec>` 收敛为 `static &[PhaseDefinition]` | `crates/core/workflow/src/write/phase_table.rs`（修改） | `agent::ModelLevel` | 编译期静态表，零运行时文件 IO |
| 路由状态机 | `build_phase_response` 以静态主体 + 上下文头 `change: <name>`（append）+ 回溯原因行（append）组装；产出 `ResolvedPhaseSpec`（已组装 String） | `crates/core/workflow/src/write/phase_next.rs`（修改） | phase_table | 零模板替换，append 组装 |
| 写面导出 | 导出面随 `PhaseAgentSpec` / `ResolvedPhaseSpec` 形态调整（删 `interpolate`） | `crates/core/workflow/src/write/mod.rs`（修改） | — | 进程内直调 |
| prompt 组装面 | `role_brief` / `strip_call_agent` / `executor_prompt` 退役；`evaluator_prompt` 仅追加瘦身协议附录；`decision_prompt` 三列表不变 | `crates/core/orchestration/src/prompt.rs`（修改） | verdict::MAX_REPORT_CHARS / decision | 动态面 append |
| verdict 解析 | `EvaluatorChecklist` 去 `phase` / `attempt` / `skipped`（walker 单点盖戳）；report ≤2000 门不变 | `crates/core/orchestration/src/verdict.rs`（修改） | workflow::model | serde `Deserialize` |
| walker 消费面 | executor 分支直出静态主体（不前置角色要点）；evaluator 分支追加协议附录；不再消费 `agent_type` | `crates/core/orchestration/src/walker.rs`（修改） | prompt | 会话发起与动态 append |
| 版本 / 废弃注记 | 根 `AGENT.md` 顶部 DEPRECATED 注记；`packages/desktop` 0.4.31 → 0.4.32 | `AGENT.md`（修改）、`packages/desktop/package.json`（修改） | — | 文档 + 版本号 |

**不变组件（零触点核对结论）**：`crates/infra/store/**`（store schema 与 `SessionEventRecord` 转录）、`crates/infra/checks/**`（static-check / test-execution 确定性门禁实现）、`crates/infra/agent/**` 的 `WorkerTurnRequest`（root / provenance / role / model_level 结构化通道）与 `SessionProvenance` 组装、`phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive` 相位机操作语义（路由 / 重试上限 / 白名单 / attempt 计时 / eval 落账 / stale 传播）、`decision.rs` 决策解析与四动作封闭集、`plugins/dev-team` 全部功能代码 / MCP 工具 / hooks / CLI 工作流 / 版本 2.10.44 / 三类交付产物、`openspec/specs/**` 既有基线 spec。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由（含被拒备选） |
|---|------|------|------|
| D1 | 上下文头 `change: <name>` 精确落点与回溯原因行拼接次序（待决 2） | **落点在 `phase_next::build_phase_response`（写面响应组装面），非 walker**。拼接次序：`change: <name>\n\n` 头部 → 静态主体 →（backtrack 时）`\n\n⚠️ 回溯原因: <reason>` 尾部。`name` 取 `record.name`（id → 记录 → name 分辨率单点，现有 `build_phase_response` 已持有），MUST NOT 模板插值。**被拒**：walker 组装——`RunRequest` 只携 `root` + `change_id`，walker 需经 `snapshot.detail` 二次解析 name（第二解析点），且 `backtrack_reason` 需从 phase_next 侧额外透传；写面组装复用既有 `record.name` 分辨率，单点不裂 | spec `desktop-workflow-write-face`「`build_phase_response` 以静态 prompt + 上下文头 + 回溯原因行组装」；AC-5「首部含一行 `change: <name>`」 |
| D2 | `executor_prompt` / `evaluator_prompt` 简化后签名形态（待决 3） | **删除 `executor_prompt`；保留 `evaluator_prompt(prompt: &str) -> String`（追加瘦身协议）与 `decision_prompt`（零改动）**。walker executor 分支 `executor.prompt.clone()` 直出；evaluator 分支 `evaluator_prompt(&evaluator.prompt)` 不变。**被拒**：保留 `executor_prompt(prompt)` 作恒等直出——role_brief 删除后即恒等函数，零价值（AGENT.md「No abstractions for single-use」） | 角色自述已溶解进静态 md；executor 无动态面需追加 |
| D3 | 插件 `AGENT.md` 废弃注记文案与位置（待决 4） | **顶部注记**：置于 `# AGENT.md` 标题之后、`## Project Overview` 之前。文案语义三要素（确切措辞 implement 阶段落笔）：① 本仓库（插件端）源码停止迭代；② 已安装副本照常可用；③ 未来将整体删除，桌面端（`packages/desktop`）已承接角色知识与相位编排。**被拒**：架构段前注记——顶部更醒目，且「废弃」是仓库级事实非架构细节 | proposal D1 / D5 + AC-9 |
| D4 | 14 个 md 逐文件正文定稿口径（待决 1） | **设计侧只钉「迁移映射表 + 结构骨架 + 桌面适配规则」，正文由 implement 阶段逐文件「搬迁 + 改写」并留痕**（逐条对照插件源可 diff）。每文件三段式：`## 角色自述` / `## 阶段要求`（输入·过程·输出·约束）/ `## 静态清单`（仅 8 evaluator，表 `ID | 检查项 | 判断依据`）；无 frontmatter；迁移 = 去 `__AGENT:__` / `__MODEL_*__` / `__MCP:*` / `__DEV_TEAM_ROOT__` / `__INCLUDE:*` 片段、`<change-name>` →「目标 change 名（由上下文头给出）」、去 phase_log 落账指令、补「静态脚本由桌面端执行，agent 内不重复执行」一句 | R1 / R2 / AC-1 / AC-7；省 token 是显式约束（D8） |
| D5 | `PhaseAgentSpec` 静态化与路由产出类型分离 | **`PhaseAgentSpec { prompt: &'static str, model_level }` 保持为静态表条目类型；新增 `ResolvedPhaseSpec { prompt: String, model_level }` 作为 `PhaseNextOutcome.executor/evaluator` 的产出类型**（静态主体 + 上下文头 + 回溯原因已组装）。`PhaseAgentSpec` 仍 `pub`（phase_next 组装 + 同 crate 测试消费）；跨 crate 消费方（orchestration）改用 `ResolvedPhaseSpec`。**被拒**：让 `PhaseAgentSpec.prompt` 保持 `String` 承载组装结果——与 AC-4「`PhaseAgentSpec` 仅含 `prompt: &'static str` + `model_level`」冲突，且静态表驻 `'static` 零分配语义丢失 | AC-4 / spec Module Contract |
| D6 | 相位表 `OnceLock<Vec>` 收敛形态 | **`static REQUIREMENT_TABLE: &[PhaseDefinition] = &[...]`**（去 `OnceLock` / 去 `requirement_table()` 构造函数 / 去 `spec` 闭包）。`include_str!` 是 `&'static str` const 表达式、`ModelLevel` 是字段less enum（const 可构造），全表 const 化；`phase_table()` 直接 `Some(REQUIREMENT_TABLE)`。**被拒**：保留 `OnceLock`——首见初始化本为 `proposal_explore_handoff()` 动态面而设，动态面删除后即死重 | D5 同源；编译期静态风格 |
| D7 | md 文件硬编码 `openspec` 域根目录名 vs layout 双禁令 | **md 文件以字面量 `openspec/changes/<目标 change 名>/…` 表述产物路径**（与插件源同式）；`proposal_explore_handoff()` 退役。依据：双禁令扫描只扫 `.rs` 且排除 `*_test.rs`，md 内容文件不在扫描面；phase_table.rs 源码只含 `include_str!("../prompts/…")` 不含 `openspec` 字面量，`cargo test` 绿。**被拒**：经占位符 / 变量间接供给域根目录名——重引入动态面，违背零占位不变量 | AC-2「零占位」；D1 上下文头只供给 change 名，不供给域根目录名 |
| D8 | evaluator 协议瘦身与 `EvaluatorChecklist` 字段删除 | **`evaluator_protocol` 去掉 phase / attempt / skipped 回声要求，保留 MCP 写通道红线 + `{verdict, report, checklist}` 形状；`EvaluatorChecklist` 删 `phase` / `attempt` / `skipped` 三字段**。盖戳单点不变：phase = walker 形参、skipped = walker 硬编码 false、attempt = phase_log 写面推导（`step_verdict_phase_log` 本就不消费三字段）。serde 无 `deny_unknown_fields`，evaluator 若仍回声多余字段则静默忽略（walker 权威）。**被拒**：保留字段并做一致性校验——evaluator 回声与 walker 权威双源冲突，且字段本无消费点 | R4 / AC-6 / spec MODIFIED「verdict 解析与 phase-log 代写」 |
| D9 | 版本交付与守线 | `packages/desktop` 0.4.31 → **0.4.32**（`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 零改动（版本 2.10.44 与三类交付产物）；守线为静态检查（fmt / clippy / 全仓零残留 grep / md 零占位符扫描 / 插件零 diff），全管线测试与 AC-1 ~ AC-4 人工核验归 test-execution 阶段 | proposal AC-9 / AC-4 |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写；公共函数仅列模块级导出函数 / 公开方法，私有函数不列入。
  测试文件（既有用例的机械保编译随动除外）由 test-design / test-gen / test-execution 阶段承接，不入本清单。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md` | executor：读项目上下文产出 proposal.md 与 specs 能力基线增量（迁移自 `plugins/dev-team/agents/proposal-planner.md`，D4 / D8 改写） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md` | executor：基于定稿 proposal 产出 design.md / tasks.md（迁移自 `agents/dev-design-planner.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-design-planner.md` | executor：从 design 推导测试范围与公共 API 签名，产出 test-design.md（迁移自 `agents/test-design-planner.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md` | executor：按 design.md / tasks.md 写实现代码（迁移自 `agents/implementation-generator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-gen-generator.md` | executor：按 test-design.md 写测试文件（迁移自 `agents/test-gen-generator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-execution-executor.md` | executor：测试执行与诊断（机械步骤由桌面端执行，agent 内不重复执行）（迁移自 `agents/test-execution-executor.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-evaluator.md` | evaluator：静态二项清单评估 proposal（迁移自 `agents/proposal-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-evaluator.md` | evaluator：静态清单评估 design（迁移自 `agents/dev-design-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-design-evaluator.md` | evaluator：静态清单评估 test-design（迁移自 `agents/test-design-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-evaluator.md` | evaluator：静态清单评估实现（迁移自 `agents/implementation-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-gen-evaluator.md` | evaluator：静态清单评估测试代码（迁移自 `agents/test-gen-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-execution-evaluator.md` | evaluator：校验测试执行报告完整性（迁移自 `agents/test-execution-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-review-evaluator.md` | evaluator：安全 / 测试覆盖 / 错误处理静态清单评审（evaluator-only，无 executor）（迁移自 `agents/code-review-evaluator.md`） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/acceptance-evaluator.md` | evaluator：按 proposal 验收标准静态二项清单评估（evaluator-only，无 executor）（迁移自 `agents/acceptance-evaluator.md`） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | `PhaseAgentSpec` 去 `agent_type`、`prompt: String` → `prompt: &'static str`；`requirement_table()` → `static REQUIREMENT_TABLE: &[PhaseDefinition]`，表条目 `include_str!("../prompts/<role>.md")` 显式字面路径（14 条）；删除 `interpolate` 与 `proposal_explore_handoff`；删除 `use foundation::layout::domain_dir_name` | D5 / D6 / D7；AC-3 / AC-4 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | 删除 `interpolate` 导入与调用；新增 `ResolvedPhaseSpec { prompt: String, model_level: ModelLevel }`；`PhaseNextOutcome.executor/evaluator` 类型 `Option<PhaseAgentSpec>` → `Option<ResolvedPhaseSpec>`；`build_phase_response` 改为静态主体 + 上下文头 `change: <name>\n\n` + 回溯原因后缀组装（`resolve` 闭包产出 `ResolvedPhaseSpec`）；补 `use agent::ModelLevel` | D1 / D5；AC-5 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 导出面：`phase_table` 导出去掉 `interpolate`；`phase_next` 导出增 `ResolvedPhaseSpec` | D5 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` | 删除 `strip_call_agent` / `role_brief` / `executor_prompt`；`evaluator_protocol` 瘦身为 `{verdict, report, checklist}`（去 phase/attempt/skipped 回声要求，保留 MCP 写通道红线）；`evaluator_prompt` / `decision_prompt` 保留 | D2 / D8；AC-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs` | `EvaluatorChecklist` 删 `phase` / `attempt` / `skipped` 字段（`parse_verdict` 逻辑与 report 长度门不变） | D8；AC-6 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | 导入面 `{decision_prompt, evaluator_prompt, executor_prompt}` → `{decision_prompt, evaluator_prompt}`；executor 分支 `executor_prompt(&executor.agent_type, &executor.prompt)` → `executor.prompt.clone()`（直出静态主体）；evaluator 分支 `evaluator_prompt(&evaluator.prompt)` 不变 | D2 / D5；AC-4 / AC-7 |
| `AGENT.md`（仓库根） | 顶部（`# AGENT.md` 标题后）加 DEPRECATED 注记（D3 三要素） | D3；AC-9 |
| `packages/desktop/package.json` | `version` 0.4.31 → 0.4.32 | D9；AC-9 |

<!-- 删除文件：无（删除的是函数 / 字段 / 令牌机制，非整文件；插件功能代码不删） -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `interpolate` | `crates/core/workflow/src/write/phase_table.rs` | 删除 | `pub fn interpolate(template: &str, change: &str, phase: Option<&str>) -> String` | 全桌面唯一模板替换退役（零残留） |
| `executor_prompt` | `crates/core/orchestration/src/prompt.rs` | 删除 | `pub fn executor_prompt(agent_type: &str, phase_prompt: &str) -> String` | 角色前导退役（角色自述溶解进静态 md） |
| `strip_call_agent` | `crates/core/orchestration/src/prompt.rs` | 删除 | `pub(crate) fn strip_call_agent(agent_type: &str) -> &str` | `__CALL_AGENT` 令牌解析退役 |
| `evaluator_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改 | `pub fn evaluator_prompt(phase_prompt: &str) -> String` | 追加瘦身协议附录（签名不变，协议内容瘦身） |
| `build_phase_response` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | 私有 | 改为静态主体 + 上下文头 + 回溯原因 append 组装 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `PhaseAgentSpec` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | `{ prompt: &'static str, model_level: ModelLevel }`（去 `agent_type`，prompt 静态化） |
| `ResolvedPhaseSpec` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | `{ prompt: String, model_level: ModelLevel }`（静态主体 + 上下文头 + 回溯原因已组装） |
| `PhaseNextOutcome` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | `executor` / `evaluator` 字段 `Option<PhaseAgentSpec>` → `Option<ResolvedPhaseSpec>` |
| `EvaluatorChecklist` | `crates/core/orchestration/src/verdict.rs` | 修改 | `{ verdict: Verdict, report: String, checklist: Vec<ChecklistItem> }`（去 `phase` / `attempt` / `skipped`） |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `0.4.31` → `0.4.32` | 桌面端版本号随变更递增 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `PhaseAgentSpec`（修改） | `prompt: &'static str`、`model_level: ModelLevel` | `PhaseDefinition.executor / evaluator` 引用 | 无（编译期静态文本，不落库） |
| `ResolvedPhaseSpec`（新增） | `prompt: String`、`model_level: ModelLevel` | `PhaseNextOutcome.executor / evaluator` | 无（进程内路由产出） |
| `PhaseNextOutcome`（修改） | `done` / `next_phase` / `round` / `executor` / `evaluator`（`ResolvedPhaseSpec`）/ `allowed_backtrack_phases` / `last_result` / `error` | — | 无（写面只读路由） |
| `EvaluatorChecklist`（修改） | `verdict: Verdict`、`report: String`、`checklist: Vec<ChecklistItem>` | walker 盖戳 phase / attempt / skipped 后经 `PhaseLogInput` 落账 | 经 `phase_log` 写面落 `PhaseRecord` + `ChecklistItemRecord`（原子落库） |

**持久化零改动**：store schema / `AgentRunRecordV3` / `SessionEventRecord` 转录 / golden wire contract 全部不触（proposal「引用沿用零 delta」与「不要修改」）。

---

## 依赖

### 运行时依赖

- 零新增——`include_str!` 为 std 宏，无新 crate 引入。

### 构建/测试依赖

- 零新增。

---

## 待决问题

- 无遗留——proposal 四项待决问题已在 D1–D4 定稿；14 个 md 的正文定稿与 checklist 条目逐条 diff 留痕归 implement 阶段执行（D4）。
