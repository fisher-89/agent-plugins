# 设计: desktop-change-flow

> **变更**: desktop-change-flow
> **日期**: 2026-10-01（回溯重收敛后重写：写通道由「CLI 子进程」翻转为「core/workflow Rust 写面」）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-workflow-write-face/`、`specs/desktop-change-orchestration/`、`specs/desktop-change-flow-view/` 已由提案阶段（回溯重收敛）写入并通过评估，属已完成产物，不在本变更清单与任务列表内。本设计基于其定稿文本展开，仅覆盖「变更范围 - 实现文件」。

工作树实况：前一版 implement 已落盘大量未提交代码（含将回退的插件侧 walker-* 与将删除的 `crates/infra/devteam`）。本设计描述**本变更完成后的目标形态**（相对 master 基线）；与上一版 design 的翻转差异见下表，被回退/删除的落盘代码在变更清单以删除条目如实列示。

## 回溯修订差异表（上一版 → 本版）

| # | 主题 | 上一版（CLI 子进程方案） | 本版（core/workflow 写面） |
|---|------|--------------------------|----------------------------|
| 1 | 写通道 | walker-* CLI 子进程（`node cli.cjs walker-*`），workflow.json 零直写 | core/workflow Rust 写面**进程内直调**，零 CLI 子进程写通道；写面为命令层可达独立能力 |
| 2 | 插件侧 | 新增 walker-* 七文件 + `cli.ts` 注册 + 2.10.45 bump + dist rebuild | **零新增**：七文件、注册、2.10.45 bump 全部回退（含 dist rebuild 去痕）；MCP / hooks 冻结 |
| 3 | file_log | transcript 提取 Write/Edit 路径 → `walker-change-files` 补录（D6 recordFileOps 通道） | desktop run **不记录 file_log**；变更文件上下文降级 **git diff**（prompt 面）；补录通道整体出局 |
| 4 | transcript.rs | 双导出（`extract_write_paths` + `final_assistant_text`） | 砍半：`extract_write_paths` 出局，`final_assistant_text` 保留 |
| 5 | views.rs | CLI JSON 封闭视图 + `parse_view`（zod schema 镜像） | **删除**（views.rs + views_test.rs）：写面原生类型直载，无跨进程序列化面 |
| 6 | infra/devteam crate | `discover.rs` / `runner.rs`（CLI 发现与子进程执行） | **整体删除**（含 Cargo.toml、workspace 注册、根包依赖） |
| 7 | static-check | `walker-static-check` CLI 步（插件内 spawn） | spawn 留 infra：`StaticCheckRunner` port（orchestration）+ `ProcessStaticCheck`（infra/agent）；`ToolCommand::StaticCheck` 变体保留 |
| 8 | snapshot.rs | FsSnapshot（前置校验含「CLI 可发现」一道） | 保留缩水：纯读不变；`change_flow_start` 前置校验去除 CLI 可发现项，换 workflow_type=requirement 校验 |
| 9 | sessionAnchors | CLI one-shot 进程隔离、锚点必重置，mid-phase interruption 不可达（V1 留痕） | **进程内复活**：`SessionAnchors` 随写面，中断相位由 walker 重入检查标定，分支可达 |
| 10 | phase_log 已知缺陷 | 插件侧现状带病（backtrack 后丢重评条目） | Rust 写面**顺手修复**（纯追加落账）；插件侧冻结不修 |
| 11 | 等价性口径 | 写通道即插件同一批命令实现，行为天然一致 | **对照功能验收**（无差分 oracle）；serde 写出经插件 zod 兼容 fixture 对照 |
| 12 | schema 权威 | zod（插件）保持唯一权威 | zod → serde **移交**（时点见 W3）；schema 形状零新字段不变 |
| 13 | 版本 | desktop 0.3.16；dev-team 2.10.45 + rebuild | desktop **0.4.0**（已就位保持）；dev-team **零 bump**（回退保持 2.10.44） |
| 14 | 配置 | 新增 `DEV_TEAM_PLUGIN_ROOT` 环境变量（discover.rs） | 随 discover.rs 删除出局，无环境变量 |
| 15 | verdict/decision 类型 | 自有镜像视图类型（`ChecklistItemView` 等） | 解析目标直用 `workflow::model` 域类型（`Verdict` / `ChecklistItem`），削减镜像层 |

**保持不变（上一版已定稿、本版延续，不得回退）**：

- walker 循环骨架（相位 ①→⑦，本版随补录步出局由 ⑧ 收敛为 ⑦）与三类节点模型（WorkerAgent / ToolStep / Gate）；ToolStep 做成图节点（可观测性均一）。
- port 缝四契约中的 `WorkerAgentPort` / `WorkflowSnapshotPort` / `RunEventSink` 及 `WorkerTurnRequest` / `WorkerTurnOutcome` 中性类型；std-only Future 别名手法。
- `control.rs`（begin/subscribe/request_stop/answer/confirm/snapshot + `RunGuard`）与 `state.rs` IPC 类型面（`RunUpdate` / `ChangeRunStatus` 等，`ChangeStepKind` 收缩一个变体）。
- verdict / decision 解析器形态（容忍围栏代码块、四动作封闭集、`ensure_backtrack_allowed` 预校验）、决策协议触发点（`max_retries_exceeded` 唤起、retry 预算内 walker 自走）。
- prompt 组装三入口（executor 角色要点前导 / evaluator 输出协议附录 / 决策有界输入）与 `STATIC_CHECK_FEEDBACK_LIMIT` / `STATIC_CHECK_PHASES` 常量。
- 停等节奏（phase 内自动、phase 间确认）、run 收口不触发归档、`bypassPermissions` 恒档、claude code CLI 引擎恒选。
- 前端 flow 运行态全套（run-state / graph overlay / run-step-node / run-control-panel / session-transcript-panel / 双 hook / detail-drawer / change-detail-view 接线）；执行图 = 展示图同源；时间序边推导规则不变。
- 数据模型四件：workflow.json schema 零新字段、`agentSessions` provenance（`source="change"` + `<change>/<phase>/<role>/<attempt>`）、`ChangeFlowControl` 进程内不持久化、前端 `ChangeFlowRunState` 内存态。
- `PIPELINE_PHASES` 与 `detail.rs` 相位列纯布局身份；红线「图不持有转移规则」（语义升级：权威与 walker 同进程仍每步问写面 `phase_next`）。

## 关键设计决策（proposal 待决问题定稿）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| W1 | 写面模块划分与命名 | `crates/core/workflow/src/write/` 子模块七文件：`mod.rs`（声明与再导出）、`phase_table.rs`（requirement 相位表 + prompt 模板 + 插值 + 白名单计算 + `MAX_RETRY_TIMES`）、`phase_next.rs`（路由状态机 + `SessionAnchors`）、`phase_start.rs`、`phase_log.rs`、`backtrack.rs`、`persist.rs`（crate 内私有持久层）；`lib.rs` 挂载 `pub mod write` | 与插件 `commands/phase-{next,start,log}.ts` / `backtrack.ts` 一比一对应，移植面可逐项对照；persist 为实现细节不外露，公共面收敛在四个操作函数 |
| W2 | workflow.json 持久化策略 | **raw Value 保形定点改写**：`persist.rs` 以 `serde_json::Value` 载入磁盘文档 + 既有宽松解析出 typed `Workflow` 作逻辑面；写触点仅限定点键（`active_phase` 置/清、`eval` 追加、`eval[i].stale` 翻转、`eval[i].backtrack_to/reason` 标记、`interrupted` 追加），其余字段（含未知/legacy 字段）原样保留，pretty JSON 写回 | 读面「未知字段宽松忽略」现状不改；双写并存窗口下零字段丢失是 serde↔zod 兼容的最强形态；`serde_json::Value` 即 serde 写出，AC-9 直接满足 |
| W3 | schema 权威移交时点 | **本变更合入即移交**：workflow.json 的校验/写出实现权威自 zod（插件）易手 serde（core/workflow 写面）；插件 zod schema 冻结不删，转为兼容 fixture 基准。fixture 形态：以插件直跑生成的多生命周期 workflow.json（初始 / active_phase 挂起 / backtrack 后 / 中断）为固定样本，serde 写出后断言 zod 可读键集与字段形状不变 | proposal 已拍板 #7「移交时点在 design 显式声明」；schema 形状零新字段为硬约束，fixture 对照即可锁 |
| W4 | static-check spawn 缝形态 | **独立 port trait**：`StaticCheckRunner`（orchestration/port.rs，`run(root) -> BoxToolFuture`）+ `ProcessStaticCheck`（infra/agent/src/static_check.rs，`core::config` 读 static_analysis 命令 → `tokio::process` spawn cwd=root → 诊断捕获；无配置/空命令 = passed 直接过）；不并入 `WorkerAgentPort`（static-check 非 agent 会话）。「spawn 不进 core」红线保持：orchestration 全 crate 零进程 spawn | proposal 已拍板 static-check 留 infra；`ToolCommand::StaticCheck` 变体保留使 walker 流程与步词汇不变，仅实现端换血 |
| W5 | git diff 变更文件上下文 | **独立 port trait** `DiffContextPort`（`diff_context(root) -> BoxDiffFuture`）+ `GitDiffSource`（infra/agent/src/git_diff.rs：`git status --porcelain` 文件清单 + `git diff HEAD` 补丁体拼接，`DIFF_CONTEXT_LIMIT` 字符截断带省略标记）；walker 在每次 WorkerAgent prompt 组装前取一次 | git diff 是进程 spawn，必须落 infra（同 W4 红线）；`git status --porcelain` 补齐未跟踪新文件（`git diff HEAD` 不含 untracked）；每次组装取新 diff 使 attempt 递进可见 |
| W6 | snapshot.rs 去留 | **保留缩水**：FsSnapshot 与 `WorkflowSnapshotPort::detail` 原样（前置校验 + 决策输入的 `ChangeDetail` 只读装配），仅 doc 注释语义换血（去「写触点唯一经 CLI 子进程」措辞） | 读缝与写缝是两个关注点；决策输入（fail checklist / 候选 eval report）仍走 detail 只读，删除合并只会让 walker 直依赖 queries 层、丧失假件缝 |
| W7 | 写面句柄与锚点归属 | `SessionAnchors` 为轻量值对象（内部 `Mutex<HashMap<(change, run_id), usize>>`），**每 run 一个**：`change_flow_start` 组合根创建后注入 `LocalToolSteps`；未来 UI 手动 backtrack/重试直调写面时自建实例即可（run_id 键隔离，无需跨 run 共享） | 锚点只在 phase_next 消费、且以 (change, run_id) 为键；per-run 实例避免进程级全局可变状态，测试面天然隔离 |
| W8 | 发起前置校验重定 | `change_flow_start` 前置校验三道换血：~~CLI 可发现~~ → **workflow_type=requirement**（写面 `phase_table` 返回 None 即显式 Err，V1 只 requirement）；change 存在且 workflow.json 可解析、无并行 run 两道保持 | CLI 已不在依赖面；V1 范围显式拒绝 bug-fix / test-only，优于相位机半途报错 |
| W9 | phase_log 丢条目缺陷修复点 | 落账为**纯追加**：attempt 推导只数该相位既有条目（含 stale/pass 历史）+ 1，**不因「相位已存在 pass 条目」短路跳过**（插件 `computeAttempt` 缺陷根源）；backtrack 回跳后同相位重评条目逐条 append | proposal 已拍板 #「Rust 写面顺手修复」；AC-9 回归项的直接落点 |
| W10 | run_id 粒度与停止寻址 | run_id 每 run 一个（`run-<millis>`，walker 铸造），作 `SessionAnchors` 键分量与会话窗口标识；停止寻址键 = change 名（`ChangeFlowControl` cancel + 当前会话 `StopRegistry::request_stop`），与 walker 会话/调试页会话同源内核寻址不冲突 | 沿上一版 D4 结论；进程内锚点复活后 run_id 有了写面侧消费点，语义闭环 |

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| requirement 相位表单源 | `PhaseDefinition` / `PhaseAgentSpec`（executor / evaluator 的 agent_type + prompt 模板）、`phase_table`（V1 仅 requirement）、`interpolate`（`<change>`/`<phase>` 插值）、`allowed_backtrack_phases`（表序前置相位）、`MAX_RETRY_TIMES=5` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（新） | serde | Rust（sync，模板静态表） |
| 相位路由状态机 | `phase_next` 只读路由：初始 / 推进 / fail 重试（≤`MAX_RETRY_TIMES`）/ 重试上限判定、`SessionAnchors` 进程内锚点与 mid-phase interruption 检测、白名单下发、prompt 插值、`last_result` 快照 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs`（新） | phase_table、model、persist（读） | Rust（sync） |
| 开相位写操作 | `phase_start`：`active_phase` 定点写入（phase / attempt / start_at），attempt 自既有值递增 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs`（新） | persist、model | Rust（sync） |
| 评估落账写操作 | `phase_log`：verdict 推导（checklist 全 pass）+ skipped 约束 + report ≤500 校验、**纯追加落账（W9 缺陷修复）**、start_at 自 active_phase 继承、落账后清 active_phase | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs`（新） | persist、model、phase_table | Rust（sync） |
| 回溯写操作 | `backtrack`：白名单二次校验（越权 Err 不写）、reason ≤500、最新条目标记 backtrack_to/reason、stale 标记 + 按相位表依赖向后传播 | `packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs`（新） | persist、model、phase_table | Rust（sync） |
| 写面持久层 | raw Value 载入 / 定点改写 / pretty 写回（W2）；crate 内私有，无 crate 外导出 | `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs`（新） | serde_json、model、foundation | Rust（sync） |
| 写面入口 | `write/mod.rs` 模块声明与公共再导出；`lib.rs` 增 `pub mod write` | `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`（新） | 同子模块 | Rust |
| walker 相位循环 | 硬编码单图相位循环 ①→⑦：三类节点调度、feedback 边计数、停等点（phase 间确认 / ask）、fail 重试与决策分叉、停止收敛、run_id 铸造；本版换血：补录步删除、每 WorkerAgent prompt 组装前经 `DiffContextPort` 取 diff 上下文、相位机步经 `LocalToolSteps` 进程内直调 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（修改） | port.rs、state.rs、control.rs、verdict.rs、decision.rs、transcript.rs、prompt.rs、core::agent | Rust、tokio async |
| 编排 port 契约 | `WorkerAgentPort` / `ToolStepPort`（语义换血：进程内相位机 + 工具步缝）/ `WorkflowSnapshotPort` / `RunEventSink` 保持，新增 `StaticCheckRunner` / `DiffContextPort`；`ToolCommand` 封闭集收缩（ChangeFiles 出局）、`ToolStepOutput` 载荷换 workflow 写面原生类型、`ToolStepError` 出局（统一 `Result<_, String>`） | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`（修改） | core::workflow（write 原生类型） | Rust trait、`Pin<Box<dyn Future>>` 别名 |
| 进程内工具步适配 | `LocalToolSteps` 实现 `ToolStepPort`：相位机四步（phase-next / phase-start / phase-log / backtrack）直调 `workflow::write::*`；StaticCheck 步委托注入的 `StaticCheckRunner` | `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`（新） | workflow::write、port.rs、foundation layout | Rust、tokio async |
| verdict 解析器 | evaluator 最终消息 → checklist（容忍围栏代码块），解析目标直用 `workflow::model::{Verdict, ChecklistItem}`；结构漂移显式失败停给用户 | `packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs`（修改） | serde、workflow::model | Rust |
| 决策解析与白名单预校验 | 决策 agent 最终消息 → 四动作封闭集；backtrack 越权预校验（写面 backtrack 二次校验兜底）；白名单类型换 `Vec<String>` | `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs`（修改） | serde、workflow::model | Rust |
| 转录提取器（砍半） | `final_assistant_text` 保留（verdict/决策 JSON 载体）；`extract_write_paths` 半边删除 | `packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs`（修改） | core::agent | Rust |
| prompt 组装 | executor 角色要点前导 + evaluator 输出协议附录 + 决策有界输入；本版增 git diff 变更文件上下文段（executor / evaluator 双入口） | `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs`（修改） | decision.rs | Rust |
| run 状态类型 | `RunUpdate` / `ChangeRunStatus` / `ChangeStepKind`（收缩：ChangeFiles 变体出局）/ `ChangeStepStatus` / `ChangeStepState` / `AskPayload` / `ChangeRunSnapshot` / `ChangeRunSummary` | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`（修改） | serde、specta | Rust |
| run 控制注册表 | 进程内 per-change run 控制（begin / cancel / 会话槽 / ask·confirm 通道 / broadcast / 快照查询）；本版仅注释措辞换血 | `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs`（修改） | state.rs、tokio sync | Rust、Mutex + broadcast + oneshot |
| 快照 port 实现 | `FsSnapshot`（foundation Layout + `workflow::change_detail` 只读装配）；本版缩水为纯读 + 注释换血 | `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs`（修改） | core::workflow、core::foundation | Rust |
| 编排 crate 清单 | 模块声明与再导出（views.rs 摘除、steps.rs 挂载） | `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs`（修改） | 同 crate 内模块 | Rust |
| WorkerAgentPort 内核适配 | compose_turn 装配 + `begin_turn` + 泵驱动收集密封转录与终态；事件经 `RunEventSink` 透传；provenance / permission 组装（既有工作树文件继续，port 面随动核对） | `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs`（修改） | core::agent、core::orchestration、本 crate compose.rs | Rust、tokio |
| static-check spawn 实现 | `ProcessStaticCheck` 实现 `StaticCheckRunner`：`core::config` 读 static_analysis 命令 → `tokio::process` spawn（cwd=workspace root）→ 诊断捕获 + 退出码；无配置/空命令 = passed 直接过 | `packages/desktop/src-tauri/crates/infra/agent/src/static_check.rs`（新） | core::orchestration（StaticCheckRunner）、core::config、core::foundation、tokio::process | Rust |
| git diff 上下文源 | `GitDiffSource` 实现 `DiffContextPort`：`git status --porcelain` 清单 + `git diff HEAD` 补丁拼接，`DIFF_CONTEXT_LIMIT` 截断 | `packages/desktop/src-tauri/crates/infra/agent/src/git_diff.rs`（新） | core::orchestration（DiffContextPort）、tokio::process | Rust |
| agent crate 清单 | `mod static_check` / `mod git_diff` 挂载与再导出；Cargo.toml 增 config / foundation 依赖 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（修改） | 同 crate 内模块 | Rust |
| change-flow 命令组 | 六命令薄包装；`change_flow_start` 前置校验换血（去 CLI 可发现项、增 requirement 校验）与组合根装配换血（LocalToolSteps / ProcessStaticCheck / GitDiffSource / FsSnapshot + run 级锚点） | `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（修改） | core::orchestration、agent-runtime、workflow、tauri | Rust、tauri IPC |
| 前端 run 状态纯 reducer | `RunUpdate` 流 → 前端 run 视图模型纯归并；运行步节点推导（图 overlay 输入）；零 react 依赖直测 | `packages/desktop/src/views/changes/flow/run-state.ts`（新） | ../types/generated/bindings、flow/types | TypeScript |
| run 控制 hook | invoke 六命令 + Channel 订阅生命周期 + 重挂快照恢复 | `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts`（新） | run-state.ts、bindings | React hook |
| 会话转录 hook | 按 `source="change"` + sourceRef 反查会话 → 密封转录重放 → `AgentUIMessage` 适配 → 实时事件按 seq 去重并入 | `packages/desktop/src/views/changes/hooks/use-session-transcript.ts`（新） | lib/agent-adapter.ts、bindings | React hook |
| 运行步节点 | react-flow 自定义节点 `runStep`：WorkerAgent / ToolStep / Gate 可辨 + pulse 运行态 + 失败态 | `packages/desktop/src/views/changes/flow/run-step-node.tsx`（新） | @xyflow/react、flow/types | React、Tailwind |
| 运行控制面板 | 发起 / 停止 / phase 间确认 / ask 应答卡片 | `packages/desktop/src/views/changes/flow/run-control-panel.tsx`（新） | use-change-flow-run | React |
| 会话转录面板 | role 分页 + `AgentTimeline` 渲染（运行中实时 / 收口重放一致） | `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`（新） | AgentTimeline、use-session-transcript | React |
| 流程图转换层 | `buildFlowGraph` 可选运行步节点 overlay；时间序边推导规则、9 列布局、attempt 缺号兜底不变 | `packages/desktop/src/views/changes/flow/graph.ts`（修改） | flow/types、layout | TypeScript |
| 流程图模型类型 | `RuntimeFlowNode` / `RunStepNodeData` / `RoleSessionRef`；三分类与边推导类型不变 | `packages/desktop/src/views/changes/flow/types.ts`（修改） | dto 类型 | TypeScript |
| 流程图渲染层 | `runStep` nodeType 注册、ChartNode 联合扩展 | `packages/desktop/src/views/changes/flow/change-flow-graph.tsx`（修改） | run-step-node、graph | React |
| 详情抽屉 | WorkerAgent 运行节点与历史 eval 节点的会话转录联动区 | `packages/desktop/src/views/changes/flow/detail-drawer.tsx`（修改） | session-transcript-panel | React |
| change 详情视图 | useChangeFlowRun + RunControlPanel 组装、run overlay 并入、转录 props 下传 | `packages/desktop/src/views/changes/change-detail-view.tsx`（修改） | 上述 flow 新件 | React |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界（相对 master 基线的目标形态）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 写面子模块声明与公共再导出（无独立函数） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | requirement 相位表 + executor/evaluator prompt 模板单源 + 插值 + 白名单计算 + `MAX_RETRY_TIMES` |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | 相位路由状态机（只读）+ `SessionAnchors` 进程内锚点 + 重试上限判定 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs` | 开相位：`active_phase` 定点写入与 attempt 计时 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | 评估落账：verdict 推导、report ≤500、纯追加（丢条目缺陷修复点）、清 active_phase |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs` | 回溯：白名单二次校验、reason ≤500、stale 标记 + 传播 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs` | workflow.json 持久层：raw Value 保形定点改写（crate 内私有，无 crate 外导出） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `LocalToolSteps`：ToolStepPort 进程内实现（相位机四步直调写面 + StaticCheck 委托） |
| `packages/desktop/src-tauri/crates/infra/agent/src/static_check.rs` | `ProcessStaticCheck`：static-check spawn 实现（spawn 留 infra 红线落点） |
| `packages/desktop/src-tauri/crates/infra/agent/src/git_diff.rs` | `GitDiffSource`：git diff 变更文件上下文源（spawn 落 infra） |
| `packages/desktop/src/views/changes/flow/run-state.ts` | run 状态纯 reducer 与运行步节点推导 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | run 控制 hook（invoke + Channel 生命周期） |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 会话反查 + 重放 + 实时并入 hook |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `runStep` 自定义节点组件 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | 运行控制面板（发起/停止/确认/ask 卡片） |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | role 分页会话转录面板（复用 AgentTimeline） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/lib.rs` | 增 `pub mod write;` 与写面公共类型再导出 | 写面挂载（crate 根注释「纯读库」措辞同步修订） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ToolCommand` 收缩（ChangeFiles 出局）、`ToolStepOutput` 载荷换 `workflow::write` 原生类型、`ToolStepError` 出局（统一 `Result<_, String>`）、新增 `StaticCheckRunner` / `DiffContextPort` / `BoxDiffFuture` / `StaticCheckOutcome`；`WorkerAgentPort` 等三契约保持 | port 语义换血：「CLI 工具步」→「相位机 + 工具步」进程内缝 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | 相位循环 ①→⑦：补录步删除、diff 上下文接入（`walk_run` 增 `diff: Arc<dyn DiffContextPort>` 参数）、决策输入来源换写面类型、run 级 `SessionAnchors` 装配随组合根；反馈边 / 停等 / 决策分叉 / 停止续走骨架保持 | 循环主体语义保留；步序随补录步出局由 ①→⑧ 收敛为 ①→⑦ |
| `packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs` | 删 `extract_write_paths` 半边（含其测试模块对应半边，仅删除失效代码） | 砍半：`final_assistant_text` 保留 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs` | `EvaluatorChecklist.checklist` 载荷类型换 `workflow::model::ChecklistItem`（自有 `ChecklistItemView` 出局）；`parse_verdict` 签名不变 | 解析目标直用域类型，削减镜像层 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` | `DecisionInput.allowed` / `ensure_backtrack_allowed` 白名单类型换 `Vec<String>`；verdict 引用换 `workflow::model::Verdict` | 写面白名单即相位 id 串，无视图包装 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` | `executor_prompt` / `evaluator_prompt` 增 diff 上下文段参数；协议附录「禁调 MCP phase-log」措辞保留 | git diff 变更文件上下文的组装落点 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ChangeStepKind` 收缩（`ChangeFiles` 变体出局）；步词汇 doc 措辞「CLI 步」→「相位机/工具步」 | 三分类可辨维度不变 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` | 注释与错误消息中 CLI 通道措辞清理 | 逻辑零改动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` | doc 注释语义换血（去「写触点唯一经 CLI 子进程」措辞） | 保留缩水（W6） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs` | 摘除 `mod views` 声明、挂载 `mod steps` | 模块清单随删增同步 |
| `packages/desktop/src-tauri/crates/core/orchestration/Cargo.toml` | crate 注释措辞换血；依赖已含 workflow 保持 | 零新增外部依赖 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 增 `mod static_check;` / `mod git_diff;` 与再导出 | 新 spawn 缝挂载 |
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml` | 增 `config`、`foundation` workspace 依赖（static_analysis 命令读取与 workspace 根解析）；crate 注释去 devteam 措辞 | 新增依赖均为既有 workspace 内 crate |
| `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` | 既有实现继续；`WorkerAgentPort` 契约未变，仅随 port 中性类型调整核对编译面 | WorkerAgent 会话执行面（proposal：既有工作树内文件继续） |
| `packages/desktop/src-tauri/Cargo.toml` | workspace members / dependencies 增 `crates/core/orchestration`；**不注册** `crates/infra/devteam`（工作树内曾注册，随回退移除）；根包 dependencies 同步（增 orchestration、去 devteam） | 新 crate 入树 + 回退清理一体；cargo 套件在 src-tauri 根自动覆盖 |
| `packages/desktop/src-tauri/Cargo.lock` | 随 crate 删增再生成 | devteam 入锁条目出清 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `change_flow_start`：删 `discover_cli` 前置校验、增 workflow_type=requirement 校验（`phase_table` None → 显式 Err）、组合根装配换血（`LocalToolSteps` + `ProcessStaticCheck` + `GitDiffSource` + `FsSnapshot` + run 级 `SessionAnchors`）；其余五命令保持 | 运行控制命令组适配（proposal：发起前置校验去除 CLI 可发现项） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `pub mod change_flow;` + `all_commands!` 宏清单六命令 | 单一登记面（已落盘保持） |
| `packages/desktop/src-tauri/src/main.rs` | `app.manage(Arc::new(ChangeFlowControl::new()))` | run 控制注册表托管（已落盘保持） |
| `packages/desktop/src-tauri/src/bindings/mod.rs` | 无手改点：新命令/类型经 `all_commands!` 宏自动纳入；以 `bindings:check` 零 diff 验证再生成链路 | proposal「bindings 经 export-bindings 再生成，非手改」；预期零 diff |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成（非手改） | 写面/port 类型变化（`ChangeStepKind` 收缩等）出线 |
| `packages/desktop/src/views/changes/flow/types.ts` | `FlowNode` 联合增 `RuntimeFlowNode`；`RunStepNodeData` / `RoleSessionRef` 与 sourceRef 定式（已落盘保持） | 运行态维度；三分类与边推导类型不变 |
| `packages/desktop/src/views/changes/flow/graph.ts` | `buildFlowGraph(detail, runNodes?)` 可选 overlay（已落盘保持） | 时间序边推导规则不变 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | `runStep` nodeType 注册（已落盘保持） | 运行步节点上图 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 转录联动区（已落盘保持） | 节点 ↔ 会话转录联动 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 运行控制入口组装（已落盘保持） | 发起 / 停止 / 确认 / ask 卡片挂载 |
| `plugins/dev-team/bin/src/cli.ts` | 回退 walker-* 注册（恢复 2.10.44 已发布形态） | 插件零新增回退；MCP 注册面与相位表零触碰 |
| `plugins/dev-team/bin/src/cli.test.ts` | 回退 walker-* 注册断言 | 同上 |
| `plugins/dev-team/package.json` | `version` 2.10.45 回退为 2.10.44 | 插件零 bump（proposal 已拍板 #8） |
| `dist/claude-plugins/dev-team/`、`dist/cursor-plugins/dev-team/`、`dist/cursor-home-image/dev-team/` | 回退后 `pnpm -C plugins/dev-team run build` 刷新（CLAUDE.md 项目规则） | AC-10：产物中零 walker-* 痕迹；净效果与 2.10.44 基线一致（预期零净 diff） |
| `packages/desktop/package.json` | `version` 0.3.15 → 0.4.0（已就位，保持） | `tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/devteam/` | 目录级整体删除：`Cargo.toml`、`src/lib.rs`、`src/discover.rs`、`src/runner.rs` 及同名测试（CLI 发现与子进程执行面随写通道转向出局） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/views.rs` | CLI JSON 封闭视图 + `parse_view`（跨进程序列化面随写通道出局） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/views_test.rs` | views 测试随之删除 |
| `plugins/dev-team/bin/src/commands/walker-io.ts`（含同名测试） | walker 子命令 JSON 信封 + zod 校验 + stdin 读取 |
| `plugins/dev-team/bin/src/commands/walker-phase-next.ts`（含同名测试） | `walker-phase-next` 子命令 |
| `plugins/dev-team/bin/src/commands/walker-phase-start.ts`（含同名测试） | `walker-phase-start` 子命令 |
| `plugins/dev-team/bin/src/commands/walker-phase-log.ts`（含同名测试） | `walker-phase-log` 子命令 |
| `plugins/dev-team/bin/src/commands/walker-backtrack.ts`（含同名测试） | `walker-backtrack` 子命令 |
| `plugins/dev-team/bin/src/commands/walker-change-files.ts`（含同名测试） | `walker-change-files` 子命令（补录通道出局） |
| `plugins/dev-team/bin/src/commands/walker-static-check.ts`（含同名测试） | `walker-static-check` 子命令 |

### 公共函数 / API

Rust 签名为 Rust 语法；TS 为 TypeScript 语法。`mod.rs` / `lib.rs` / `Cargo.toml` 为声明、再导出与清单面，不设独立条目。命令组各命令配 `pub(crate) async fn *_with<R: tauri::Runtime>(...)` 泛型测试缝（沿 `agent_start_with` 先例），不逐条列出。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `PhaseAgentSpec` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | `pub struct PhaseAgentSpec { pub agent_type: String, pub prompt: String }` | 相位角色的 agent 引用与 prompt 模板（`__CALL_AGENT:<role>__` 约定原样保留） |
| `PhaseDefinition` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | `pub struct PhaseDefinition { pub id: &'static str, pub description: &'static str, pub executor: PhaseAgentSpec, pub evaluator: PhaseAgentSpec }` | 单相位定义（requirement 表静态项） |
| `MAX_RETRY_TIMES` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | `pub const MAX_RETRY_TIMES: u32 = 5;` | 重试上限（与插件同名常量一致） |
| `phase_table` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` | 按工作流类型取相位表；V1 仅 `requirement` 返回 Some，其余 None（W8 校验依据） |
| `interpolate` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | `pub fn interpolate(template: &str, change: &str, phase: Option<&str>) -> String` | `<change>` / `<phase>` 占位符插值（与插件 `interpolatePrompt` 语义一致） |
| `allowed_backtrack_phases` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | `pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String>` | 当前相位在表中的全部前置相位（白名单） |
| `SessionAnchors` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | `pub struct SessionAnchors { /* Mutex<HashMap<(String, String), usize>> */ }`，`SessionAnchors::new() -> Self` | 进程内会话锚点：(change, run_id) → 首见时 eval 条目数基线（每 run 一个实例，W7） |
| `PhaseNextOutcome` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | `pub struct PhaseNextOutcome { pub done: bool, pub next_phase: Option<String>, pub round: u32, pub executor: Option<PhaseAgentSpec>, pub evaluator: Option<PhaseAgentSpec>, pub allowed_backtrack_phases: Vec<String>, pub last_result: Option<LastResult>, pub error: Option<PhaseNextError> }` | 路由产出（executor / evaluator prompt 已插值；白名单随行下发） |
| `PhaseNextError` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | `pub enum PhaseNextError { MaxRetriesExceeded { phase: String, round: u32 } }` | 重试上限（决策 agent 唤起触发点，沿上一版 D7） |
| `LastResult` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | `pub struct LastResult { pub phase: String, pub verdict: Verdict, pub report: String, pub timestamp: Option<OffsetDateTime> }` | 最近一次 eval 条目快照（决策输入面） |
| `phase_next` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | `pub fn phase_next(layout: &Layout, change: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | 只读路由状态机：初始 / 推进 / fail 重试 / 上限 / mid-phase interruption（锚点比对）；不改 eval store |
| `PhaseStartOutcome` | `crates/core/workflow/src/write/phase_start.rs` | 新增 | `pub struct PhaseStartOutcome { pub phase: String, pub attempt: u32, pub start_at: OffsetDateTime }` | 开相位产出 |
| `phase_start` | `crates/core/workflow/src/write/phase_start.rs` | 新增 | `pub fn phase_start(layout: &Layout, change: &str, phase: &str) -> Result<PhaseStartOutcome, String>` | `active_phase` 定点写入（attempt 自既有值递增，重入即新一轮计时） |
| `PhaseLogInput` | `crates/core/workflow/src/write/phase_log.rs` | 新增 | `pub struct PhaseLogInput { pub phase: String, pub report: String, pub checklist: Vec<ChecklistItem>, pub skipped: bool }` | 落账输入（checklist 用 `workflow::model::ChecklistItem`） |
| `PhaseLogOutcome` | `crates/core/workflow/src/write/phase_log.rs` | 新增 | `pub struct PhaseLogOutcome { pub phase: String, pub attempt: u32 }` | 落账产出 |
| `phase_log` | `crates/core/workflow/src/write/phase_log.rs` | 新增 | `pub fn phase_log(layout: &Layout, change: &str, input: &PhaseLogInput) -> Result<PhaseLogOutcome, String>` | verdict 推导（checklist 全 pass；skipped 约束）+ report ≤500 校验 + **纯追加落账（W9）** + 清 active_phase |
| `BacktrackInput` | `crates/core/workflow/src/write/backtrack.rs` | 新增 | `pub struct BacktrackInput { pub phase: String, pub to: String, pub reason: String, pub allowed: Vec<String> }` | 回溯输入（allowed 随行走带，写面二次校验） |
| `BacktrackOutcome` | `crates/core/workflow/src/write/backtrack.rs` | 新增 | `pub struct BacktrackOutcome { pub phase: String, pub target: String }` | 回溯产出 |
| `backtrack` | `crates/core/workflow/src/write/backtrack.rs` | 新增 | `pub fn backtrack(layout: &Layout, change: &str, input: &BacktrackInput) -> Result<BacktrackOutcome, String>` | 白名单二次校验（越权 Err 不写）+ reason ≤500 + 最新条目标记 + stale 标记与相位表依赖传播 |
| `ToolCommand` | `crates/core/orchestration/src/port.rs` | 修改 | `pub enum ToolCommand { PhaseNext { change: String, run_id: String }, PhaseStart { change: String, phase: String }, PhaseLog { change: String, phase: String, input: PhaseLogInput }, Backtrack { change: String, phase: String, input: BacktrackInput }, StaticCheck }` | 封闭集收缩（ChangeFiles 出局）；载荷直载写面输入类型 |
| `ToolStepOutput` | `crates/core/orchestration/src/port.rs` | 修改 | `pub enum ToolStepOutput { PhaseNext(Box<PhaseNextOutcome>), PhaseStart(PhaseStartOutcome), PhaseLog(PhaseLogOutcome), Backtrack(BacktrackOutcome), StaticCheck(StaticCheckOutcome) }` | 载荷换写面原生类型（PhaseNext 大变体 Box 收敛尺寸差手法保持） |
| `StaticCheckOutcome` | `crates/core/orchestration/src/port.rs` | 新增 | `pub struct StaticCheckOutcome { pub passed: bool, pub diagnostics: String }` | 门禁产出（passed=false 走定向反馈边） |
| `StaticCheckRunner` | `crates/core/orchestration/src/port.rs` | 新增 | `pub trait StaticCheckRunner: Send + Sync { fn run(&self, root: &str) -> BoxToolFuture; }` | static-check spawn 缝（infra 实现；W4） |
| `DiffContextPort` | `crates/core/orchestration/src/port.rs` | 新增 | `pub trait DiffContextPort: Send + Sync { fn diff_context(&self, root: &str) -> BoxDiffFuture; }` | git diff 变更文件上下文缝（infra 实现；W5） |
| `BoxDiffFuture` | `crates/core/orchestration/src/port.rs` | 新增 | `pub type BoxDiffFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;` | std-only Future 别名（不引 futures 依赖） |
| `LocalToolSteps::new` | `crates/core/orchestration/src/steps.rs` | 新增 | `pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>) -> Self` | 进程内 `ToolStepPort` 实现：相位机四步直调 `workflow::write::*`（`foundation::layout::resolve(root)` 解析路径），StaticCheck 委托注入 runner |
| `walk_run` | `crates/core/orchestration/src/walker.rs` | 修改 | `pub async fn walk_run(worker: Arc<dyn WorkerAgentPort>, tools: Arc<dyn ToolStepPort>, diff: Arc<dyn DiffContextPort>, snapshot: Arc<dyn WorkflowSnapshotPort>, control: Arc<ChangeFlowControl>, guard: RunGuard, request: RunRequest) -> ChangeRunStatus` | 相位循环主入口（增 diff 参数）；补录步删除后循环收敛为 ①→⑦ |
| `STATIC_CHECK_FEEDBACK_LIMIT` | `crates/core/orchestration/src/walker.rs` | 修改 | `pub const STATIC_CHECK_FEEDBACK_LIMIT: u32 = 5;` | 定向反馈边独立上限（保持，沿 hook `loop_limit` 语义） |
| `STATIC_CHECK_PHASES` | `crates/core/orchestration/src/walker.rs` | 修改 | `pub const STATIC_CHECK_PHASES: [&str; 2]` | 步门控布局常量 implement / test-gen（保持，非路由权威） |
| `parse_verdict` | `crates/core/orchestration/src/verdict.rs` | 修改 | `pub fn parse_verdict(text: &str) -> Result<EvaluatorChecklist, String>` | 签名不变；checklist 载荷换域类型 |
| `ensure_backtrack_allowed` | `crates/core/orchestration/src/decision.rs` | 修改 | `pub fn ensure_backtrack_allowed(action: &DecisionAction, allowed: &[String]) -> Result<(), String>` | 白名单类型换 `Vec<String>`（walker 侧第一道；写面 backtrack 二次校验兜底） |
| `executor_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改 | `pub fn executor_prompt(agent_type: &str, phase_prompt: &str, diff_context: &str) -> String` | 角色要点前导 + 已插值 phase prompt + git diff 上下文段 |
| `evaluator_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改 | `pub fn evaluator_prompt(phase_prompt: &str, diff_context: &str) -> String` | 输出协议附录（禁调 MCP phase-log、最终消息 checklist JSON）+ git diff 上下文段（change/phase 已由写面插值，参数出局） |
| `decision_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改 | `pub fn decision_prompt(input: &DecisionInput) -> String` | 有界输入组装（保持） |
| `ProcessStaticCheck::new` | `crates/infra/agent/src/static_check.rs` | 新增 | `pub fn new() -> Self` | `StaticCheckRunner` 实现：core::config 读 static_analysis 命令 → `tokio::process` spawn（cwd=root）→ 诊断捕获；无配置/空命令 passed 直接过 |
| `GitDiffSource::new` | `crates/infra/agent/src/git_diff.rs` | 新增 | `pub fn new() -> Self` | `DiffContextPort` 实现：`git status --porcelain` + `git diff HEAD` 拼接 + `DIFF_CONTEXT_LIMIT` 截断 |
| `change_flow_start` | `src-tauri/src/commands/change_flow/mod.rs` | 修改 | `pub async fn change_flow_start(app: AppHandle, on_event: Channel<RunUpdate>, root: String, change: String) -> Result<ChangeRunSummary, String>` | 签名不变；前置校验与组合根装配换血（W8） |
| `change_flow_stop` / `change_flow_answer` / `change_flow_confirm` / `change_flow_state` / `change_flow_watch` | `src-tauri/src/commands/change_flow/mod.rs` | 修改 | 签名不变（五命令保持） | 逻辑零改动；随装配面编译核对 |
| `initialRunState` | `src/views/changes/flow/run-state.ts` | 新增 | `export function initialRunState(snapshot: ChangeRunSnapshot \| null): ChangeFlowRunState \| null` | 快照 → 前端 run 视图模型初值 |
| `applyRunUpdate` | `src/views/changes/flow/run-state.ts` | 新增 | `export function applyRunUpdate(state: ChangeFlowRunState \| null, update: RunUpdate): ChangeFlowRunState \| null` | 纯归并：步状态 / SessionEvent 缓存 / ask / 确认等待 / 终态 |
| `runStepNodes` | `src/views/changes/flow/run-state.ts` | 新增 | `export function runStepNodes(state: ChangeFlowRunState): RuntimeFlowNode[]` | run 视图模型 → 图 overlay 运行步节点（id `run:<phase>:<attempt>:<step>[:<seq>]`） |
| `useChangeFlowRun` | `src/views/changes/hooks/use-change-flow-run.ts` | 新增 | `export function useChangeFlowRun(params: { root: string \| null; change: string \| null }): UseChangeFlowRunResult` | invoke 六命令 + Channel 生命周期 + 重挂快照恢复 |
| `UseChangeFlowRunResult` | `src/views/changes/hooks/use-change-flow-run.ts` | 新增 | `export interface UseChangeFlowRunResult { state: ChangeFlowRunState \| null; start(): Promise<void>; stop(): Promise<void>; confirm(proceed: boolean): Promise<void>; answer(text: string): Promise<void>; error: string \| null }` | 控制面板与视图消费面 |
| `useSessionTranscript` | `src/views/changes/hooks/use-session-transcript.ts` | 新增 | `export function useSessionTranscript(params: { root: string \| null; sourceRef: string \| null; liveEvents: AgentEvent[] }): { messages: AgentUIMessage[]; running: boolean; error: string \| null }` | 会话反查 → 重放 → 适配 → 实时并入 |
| `RunStepNode` | `src/views/changes/flow/run-step-node.tsx` | 新增 | `export function RunStepNode(props: NodeProps<RunStepFlowNode>): React.JSX.Element` | 类型徽章 + pulse 运行态 + 失败红态 |
| `RunStepFlowNode` | `src/views/changes/flow/run-step-node.tsx` | 新增 | `export type RunStepFlowNode = Node<RunStepNodeData, 'runStep'>` | react-flow 节点类型 |
| `RunControlPanel` | `src/views/changes/flow/run-control-panel.tsx` | 新增 | `export function RunControlPanel(props: { change: string; run: UseChangeFlowRunResult }): React.JSX.Element` | 发起/停止/确认/ask 卡片 |
| `SessionTranscriptPanel` | `src/views/changes/flow/session-transcript-panel.tsx` | 新增 | `export function SessionTranscriptPanel(props: { root: string \| null; roleRefs: RoleSessionRef[]; liveEvents: AgentEvent[] }): React.JSX.Element` | role 分页 + `AgentTimeline`（无第二套时间线组件） |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `PhaseAgentSpec` / `PhaseDefinition` | `crates/core/workflow/src/write/phase_table.rs` | 新增 | 相位表静态类型（模板单源） |
| `SessionAnchors` / `PhaseNextOutcome` / `PhaseNextError` / `LastResult` | `crates/core/workflow/src/write/phase_next.rs` | 新增 | 路由状态机产出与进程内锚点 |
| `PhaseStartOutcome` / `PhaseLogInput` / `PhaseLogOutcome` / `BacktrackInput` / `BacktrackOutcome` | `crates/core/workflow/src/write/{phase_start,phase_log,backtrack}.rs` | 新增 | 写操作输入/产出封闭类型（载荷复用 `workflow::model::{Verdict, ChecklistItem}`） |
| `ToolCommand` / `ToolStepOutput` | `crates/core/orchestration/src/port.rs` | 修改 | 封闭集收缩 + 载荷换血（`ToolStepError` / `ChangeFilesView` 等视图类型出局） |
| `StaticCheckOutcome` / `StaticCheckRunner` / `DiffContextPort` / `BoxDiffFuture` | `crates/core/orchestration/src/port.rs` | 新增 | spawn 缝与 diff 缝契约 |
| `EvaluatorChecklist` | `crates/core/orchestration/src/verdict.rs` | 修改 | checklist 载荷换 `workflow::model::ChecklistItem`（自有 `ChecklistItemView` 出局） |
| `DecisionInput` / `CandidateReport` | `crates/core/orchestration/src/decision.rs` | 修改 | 白名单 `Vec<String>`；verdict 引用换域类型（封闭集 `DecisionAction` 形状不变） |
| `ChangeStepKind` | `crates/core/orchestration/src/state.rs` | 修改 | 收缩：`ChangeFiles` 变体出局（executor/evaluator/decision/phaseStart/staticCheck/phaseLog/verdictGate/retryGate/whitelistGate 九变体） |
| `RunUpdate` / `ChangeRunStatus` / `ChangeStepStatus` / `ChangeStepState` / `AskPayload` / `ChangeRunSnapshot` / `ChangeRunSummary` | `crates/core/orchestration/src/state.rs` | 修改 | 既有类型保持（serde camelCase + specta Type；仅注释措辞随语义换血） |
| `RuntimeFlowNode` / `RunStepNodeData` / `RoleSessionRef` | `src/views/changes/flow/types.ts` | 新增 | 运行步图节点与转录反查键（并入 `FlowNode` 联合） |
| `ChangeFlowRunState` | `src/views/changes/flow/run-state.ts` | 新增 | 前端 run 视图模型（不持久化） |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `"0.4.0"` | 0.3.15 → 0.4.0（已就位保持）；tauri.conf.json 自动跟随，src-tauri/Cargo.toml 不随动 |
| `version` | `plugins/dev-team/package.json` | 修改 | string | `"2.10.44"` | 2.10.45 **回退**为 2.10.44（插件零 bump）；rebuild 刷新 dist/ 三类交付产物 |

<!-- 上一版新增的 DEV_TEAM_PLUGIN_ROOT 环境变量随 discover.rs 删除出局（见差异表 #14）。workflow.json schema 零新字段，无配置键变更。 -->

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `workflow.json`（复用，schema 零新字段） | `eval[]`（phase/attempt/verdict/report/checklist/skipped/stale/backtrack_to/backtrack_reason/start_at/timestamp）、`file_log[]`、`active_phase`（phase/attempt/start_at）、`interrupted[]`、`workflow_type`/`created` | **core/workflow 写面为唯一写通道**（读 + 写单一权威，AC-6）；桌面 run 全程 `file_log` 零新增（AC-3）；派生节点状态的输入面 | 磁盘（W2 raw Value 保形定点改写；serde 写出兼容插件 zod 读取，AC-9） |
| schema 权威移交 | 校验/写出实现 zod → serde；zod schema 冻结为兼容 fixture 基准（W3） | 插件直跑 fixture（初始 / 挂起 / backtrack 后 / 中断）为 serde 写出的兼容对照面 | 双写并存窗口的兼容硬约束 |
| `agentSessions`（复用 store schema） | `SessionRow` + `provenance { source: "change", source_ref: "<change>/<phase>/<role>/<attempt>" }` | WorkerAgent 节点状态派生的会话集反查键；转录可观测与重放数据面 | redb（既有表，不加列不建表） |
| `ChangeFlowControl`（进程内） | per-change：run_id、`ChangeRunStatus`、当前步、当前 session_id 槽、cancel 标志、ask/confirm 单次通道、broadcast sender | 键 = change 名；命令层读写、walker 持 `RunGuard` 写；run 终态即除名 | 不持久化（桌面重启后自 active_phase 续走，AC-7） |
| `ChangeFlowRunState`（前端内存） | 状态机镜像 + 步状态表 + 最近会话事件缓存 | 由 `RunUpdate` 流纯归并；收口后释放订阅，图回落 `ChangeDetail` 派生 | 不持久化 |

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `walker.rs` 相位循环 ①→⑦（`walk_run`）；相位机步经 `steps.rs` 进程内直调写面、WorkerAgent 经 port 双缝（假引擎 + 假写面可驱动）；pass 自动推进、fail 重试 ≤`MAX_RETRY_TIMES`、上限判定均为写面 `phase_next` 权威；workflow.json 演进与插件直跑形态**对照功能验收**一致（AC-9 口径） |
| AC-2 | `decision.rs` 四动作封闭集 + `ensure_backtrack_allowed` 预校验；决策 agent 由 `PhaseNextError::MaxRetriesExceeded` 唤起（retry 预算内 walker 自走）；输入有界（fail checklist + 写面下发白名单 + 候选 eval report）；白名单内 backtrack 经写面 `backtrack` 执行且二次校验兜底、越权被拒；`ask` → control 通道 + `RunControlPanel` 中断卡片应答回流 |
| AC-3 | 写面持久层不触 `file_log` 键（desktop run 零 file_log 条目）；`prompt.rs` executor/evaluator 双入口组装 `DiffContextPort` 提供的 git diff 上下文（`GitDiffSource`：porcelain 清单 + diff HEAD + 截断）；既有 skill 路径 file_log 数据的流程图挂载（`attachments.ts`）与图外面板显示零改动 |
| AC-4 | `STATIC_CHECK_PHASES` 门控 implement/test-gen；`ToolCommand::StaticCheck` 必经步（`ProcessStaticCheck` spawn，infra 层）；失败诊断 Continue 注入同一 executor 会话、`STATIC_CHECK_FEEDBACK_LIMIT=5` 独立计数；超限按桌面代写 fail `phase_log` 升格（进重试/决策分叉）；static-check 节点状态经 `RunUpdate::Step` 上图可观测 |
| AC-5 | `run-state.ts` + `graph.ts` overlay（边推导规则不变）；`run-step-node.tsx` 三类可辨 + pulse；`use-session-transcript` + `session-transcript-panel.tsx` + `detail-drawer.tsx` 节点 ↔ 转录联动（运行中 `RunUpdate::SessionEvent` 实时流、收口重放一致）；run 终态释放订阅并显式 refresh，图回落既有派生规则（desktop-change-flow-view 既有场景回归不受影响） |
| AC-6 | 写通道唯一：workflow.json 全部变更仅经 `workflow::write` 进程内直调（零 CLI 子进程写代码路径）；`crates/infra/devteam` 不存在（删除清单）；`orchestration → workflow` 依赖方向成立（Cargo.toml）；static-check 与 git diff spawn 为仅有的进程 spawn 且均落 infra 层（spawn 不进 core） |
| AC-7 | `worker.rs` provenance `source="change"` + sourceRef 定式（保持）；`agentSessions` 反查派生节点状态；停止经 `StopRegistry` 收敛；重启后重新发起：写面 `phase_next` 按 eval 史路由自 active_phase 续走、已 pass 相位不重跑；`SessionAnchors` 进程内复活使 mid-phase interruption 重入标定可达 |
| AC-8 | 版本交付：desktop 0.4.0（已就位保持，Cargo.toml 不随动）、dev-team 零 bump（2.10.44）+ dist rebuild 无 walker-* 痕迹；守线为静态检查（fmt / clippy / client:check 含 knip / bindings:check 零 diff），自动化套件的执行验证由 test-execution 阶段承接 |
| AC-9 | 写面逐项对照插件直跑行为语义验收（`phase_next` 路由/上限/白名单、`phase_start` attempt 计时、`phase_log` 落账、`backtrack` stale 标记+传播），不建差分 oracle；`phase_log` 纯追加落账（W9）使 backtrack 回跳后同相位重评条目全落账（插件缺陷不复发）；`persist.rs` 写出经插件 zod 兼容 fixture 对照可读（W2/W3） |
| AC-10 | walker-* 七文件、`cli.ts`/`cli.test.ts` 注册、`package.json` 2.10.45 bump 全部回退（删除/修改清单）；三类 dist 交付产物 rebuild 刷新且产物中零 walker-* 痕迹 |

---

## 依赖

### 运行时依赖

- 无新增 npm / cargo 外部依赖 — 写面 sync（`workflow` crate 既有 serde/serde_json/time/foundation 全覆盖）；orchestration 异步面沿用 std `Pin<Box<dyn Future>>` 别名与既有 tokio；spawn 面用既有 `tokio::process`
- `git`（PATH）— 变更文件上下文源（`GitDiffSource` spawn；缺失时 diff 上下文降级为错误提示，不阻断 run 启动）
- claude code CLI 引擎 — WorkerAgent 会话执行引擎（既有 `crates/infra/agent/src/cli` 发现与运行面复用，V1 恒 CLI，SDK 不承载）
- `node` / dev-team CLI 子进程 — **不再是运行时依赖**（随写通道转向出局）

### 构建/测试依赖

- 无新增 — 沿 `vp`（check/test/build）、`knip`、cargo workspace（src-tauri 根，crate 删增随套件自动收敛）、`export-bindings`（bindings 再生成）既有工具链

---

## 待决问题

- evaluator 无视协议附录仍直调 MCP phase_log 的双写防御：插件 MCP 冻结在场，V1 依赖 prompt 附录约束 + 桌面代写 phase-log 的最终消息解析；不一致时以先落账者为准的语义实测后定是否加运行时拦截
- `interrupted[]` 条目的桌面侧写入时机：walker 停止/中断时 V1 只留 `active_phase` 残留（写面锚点比对可检测），是否补写 interrupted 条目以保全流程图 interrupted 节点精度，实测后评估（不阻塞本期）
- 决策会话应答回流轮次上限：answer → Continue 重出封闭集若连续 `ask`，V1 未设上限（实测后定是否计次封顶）
- run 控制注册表以 change 名为单键：跨 workspace 同名 change 并行 run 的键冲突形态（出现再扩复合键，V1 不做）
- bug-fix / test-only 相位表的写面移植与编排支持（后续独立 change；W8 前置校验已显式拒绝）
- 全自动推进开关（phase 间免确认）：V1 不做（proposal 已拍板停等节奏）
