# 任务: desktop-prompt-centralize

> **变更**: desktop-prompt-centralize
> **依据**: proposal.md + design.md（D1–D9 决策编号见 design）

任务边界：本列表只含实现任务；**新增测试锚**（上下文头 `change: <name>` 在场断言、evaluator 协议瘦身形状断言、`EvaluatorChecklist` 瘦身回环、`PhaseAgentSpec` 无 `agent_type` 结构断言、md 零占位符扫描）由 test-design / test-gen / test-execution 阶段承接——但类型 / 字段 / 函数删除演进触及既有测试文件编译面，本列表阶段内含**机械随动**（改签名 / 改基设 / 改 fixture，不写新断言）。AC-1 ~ AC-4 的 grep / read 人工核验归 test-execution 阶段承载，不在本列表写自动化断言（D6 用户拍板「不建通配符守门测试」）。任务内文件均在 design.md 变更清单内。

只读红线：`plugins/dev-team` 全部功能代码 / MCP 工具 / hooks / CLI 工作流 / 版本 2.10.44 / 三类交付产物（仅根 `AGENT.md` 加注记）；`crates/infra/store` schema / `SessionEventRecord` 转录；`crates/infra/checks`（static-check / test-execution 门禁实现）；`crates/infra/agent` 的 `WorkerTurnRequest` / provenance / cwd 通道；相位机操作语义（相位序 / model_level / 白名单 / 前置依赖 / 重试上限 / attempt 计时 / eval 落账 / stale 传播）；`openspec/specs/**` 既有基线 spec。

## 阶段一：角色知识集中地 + 相位表 + 路由组装（core/workflow）

- [x] 新建 `packages/desktop/src-tauri/crates/core/workflow/src/prompts/` 目录，写入 14 个 md（6 executor + 8 evaluator）：`proposal-planner.md` / `dev-design-planner.md` / `test-design-planner.md` / `implementation-generator.md` / `test-gen-generator.md` / `test-execution-executor.md` / `proposal-evaluator.md` / `dev-design-evaluator.md` / `test-design-evaluator.md` / `implementation-evaluator.md` / `test-gen-evaluator.md` / `test-execution-evaluator.md` / `code-review-evaluator.md` / `acceptance-evaluator.md`（D4 迁移映射 + D8 结构骨架：`## 角色自述` / `## 阶段要求`（输入·过程·输出·约束）/ `## 静态清单`（仅 8 evaluator））
- [x] 14 个 md 桌面适配改写：去 `__AGENT:__` / `__MODEL_*__` / `__MCP:*` / `__DEV_TEAM_ROOT__` / `__INCLUDE:*` 片段；`<change-name>` →「目标 change 名（由上下文头给出）」；去 phase_log / phase_next / phase_start / backtrack 落账路由指令（evaluator 零落账、零 eval.json / workflow.json 写入指令）；每个 md 补「静态脚本（static-check / test-execution 等机械步骤）由桌面端执行，agent 内不重复执行」；只引用桌面七工具 read / grep / glob / ls / write / edit / bash 与 cwd=workspace root（AC-1 / AC-2 / AC-7）
- [x] `crates/core/workflow/src/write/phase_table.rs`：`PhaseAgentSpec` 去 `agent_type`、`prompt: String` → `prompt: &'static str`；`requirement_table()` 收敛为 `static REQUIREMENT_TABLE: &[PhaseDefinition] = &[...]`（去 `OnceLock` / `spec` 闭包），14 表条目 `include_str!("../prompts/<role>.md")` 显式字面路径；删除 `interpolate` 函数；删除 `proposal_explore_handoff()` 及 `use foundation::layout::domain_dir_name`；`phase_table()` 直接返回 `Some(REQUIREMENT_TABLE)`（D5 / D6 / D7；AC-3 / AC-4）
- [x] `crates/core/workflow/src/write/phase_next.rs`：删除 `interpolate` 导入与调用；新增 `ResolvedPhaseSpec { prompt: String, model_level: ModelLevel }`（`Debug, Clone, PartialEq, Eq`）与 `use agent::ModelLevel`；`PhaseNextOutcome.executor/evaluator` 类型 `Option<PhaseAgentSpec>` → `Option<ResolvedPhaseSpec>`；`build_phase_response` 组装改为 `change: <name>\n\n` 头部 + 静态主体 + 回溯原因后缀（`resolve` 闭包产出 `ResolvedPhaseSpec`，不再 clone `agent_type` / 不调 `interpolate`）（D1 / D5；AC-5）
- [x] `crates/core/workflow/src/write/mod.rs`：`phase_table` 导出去掉 `interpolate`；`phase_next` 导出增 `ResolvedPhaseSpec`（D5）
- [x] 编译面机械随动：`phase_table_test.rs` 删除 `interpolate` 三用例与 `__CALL_AGENT` 断言（改断言 prompt 为非空静态文本 + `PhaseAgentSpec` 无 `agent_type` 结构，新锚留测试相位）；`phase_next_test.rs` 插值断言改上下文头在场（`prompt` 含 `change: demo-change`、无 `<change>` / `<phase>` / CHANGE_ID，回溯原因后缀断言保留）；`cargo check -p workflow` 过

## 阶段二：orchestration——prompt / verdict / walker 消费面

- [x] `crates/core/orchestration/src/prompt.rs`：删除 `strip_call_agent` / `role_brief` / `executor_prompt`；`evaluator_protocol` 瘦身为 `{verdict, report, checklist}`（去 phase / attempt / skipped 回声要求，保留 MCP 写通道四件红线与 report ≤2000、checklist 至少一项、pass 布尔、evidence 事实依据）；`evaluator_prompt` / `decision_prompt` 保留（D2 / D8；AC-6）
- [x] `crates/core/orchestration/src/verdict.rs`：`EvaluatorChecklist` 删 `phase` / `attempt` / `skipped` 字段（`parse_verdict` 与 report 长度门逻辑不变；无 `deny_unknown_fields`，多余字段静默忽略——walker 权威）（D8；AC-6）
- [x] `crates/core/orchestration/src/walker.rs`：导入面 `use crate::prompt::{decision_prompt, evaluator_prompt, executor_prompt}` → `{decision_prompt, evaluator_prompt}`；executor 分支 `executor_prompt(&executor.agent_type, &executor.prompt)` → `executor.prompt.clone()`；evaluator 分支 `evaluator_prompt(&evaluator.prompt)` 不变；`model_level` 读取点（含 test-execution 分支 executor 档位回退）随 `ResolvedPhaseSpec` 字段名不变零改动（D2 / D5；AC-4 / AC-7）
- [x] 编译面机械随动：`prompt_test.rs` 删除 `executor_prompt` 三段组装 / 角色要点 / 兜底用例，`evaluator_prompt` 协议断言改瘦身形状（无 phase / attempt / skipped 回声要求，含 verdict / report / checklist 与 MCP 红线），`prompt确定性` 用例去掉 executor 恒等断言；`verdict_test.rs` fixture 与逐字段断言改瘦身形状（去 phase / attempt / skipped；attempt 缺席 / skipped 三形态用例删除或改走多余字段忽略面，新锚留测试相位）；`walker_test.rs` `route_with_round` 构造 `workflow::write::PhaseAgentSpec` → `ResolvedPhaseSpec`（去 `agent_type`）；`cargo check -p orchestration` 过

## 阶段三：废弃注记 + 版本 + 守线收口（静态，不含测试执行）

- [x] 根 `AGENT.md`：`# AGENT.md` 标题后加 DEPRECATED 注记（D3 三要素：源码停更 / 已安装副本照常可用 / 未来删除，桌面端已承接）（AC-9）
- [x] `packages/desktop/package.json`：`version` 0.4.31 → 0.4.32（`src-tauri/tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）（D9 / AC-9）
- [x] 守线自查：`cargo fmt` + `cargo clippy` 零错误零新告警；`cargo test`（workflow + orchestration）编译绿（新增断言留测试相位）；全仓 grep 零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`（desktop core 源码面）；14 个 md 零占位符 / 零 `__MCP:` / 零 phase_log 落账指令扫描；`plugins/dev-team` 零 diff（版本 2.10.44 + 三类交付产物）；`openspec/specs/**` 零改动；相位机语义与 store schema 零 diff（AC-4 / AC-7 / AC-8 / AC-9）
- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（14 新增 md + 8 修改文件，无清单外改动、无清单内遗漏）；AC-9 版本交付 0.4.32 已执行
