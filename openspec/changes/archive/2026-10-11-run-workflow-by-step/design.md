# 设计: run-workflow-by-step

> **变更**: run-workflow-by-step
> **日期**: 2026-10
> **基于**: proposal.md, explore.md（五轮收敛）、specs/ 八个能力 delta

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 八份 delta 已由提案阶段定稿，本设计基于其文本展开，仅覆盖「变更范围 - 实现文件」。explore 五轮已把目标模型收敛为四条决策（决策① 定义载体 = 源码 / 决策② 边表单独落 + feedback 重跑 = 新节点 / 决策③ 重设计运行记录 model / 决策④ 会话回放复用 + stdout 收敛 infra/exec），本设计把四条决策落到类型面、存储面、走查面与前端面的具体形态，并定稿 proposal 遗留的四个待决问题。

工作树实况：本变更起点为 `packages/desktop` 0.4.34 基线（run 运行史已落 `RunRecord` / `RunStepRecord` 两表、walk 循环硬编码 ①→⑦、artifact 为全树扫描 + 前端 `docColumn` 静态映射、static-check stdout 全量即丢）。本设计描述**本变更完成后的目标形态**。

---

## 关键设计决策（proposal 待决问题定稿）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `StepDefinition` 的 `on_fail` 语义形态 | `on_fail: Option<OnFail>`，`OnFail` 两变体：`FeedbackLoop { limit: u32 }`（定向反馈边——失败步诊断注入本相位 executor 会话 Continue 修复重跑，独立计数 ≤ limit，超限升格相位 fail）与 `Terminal`（终态失败停给用户——verdict 解析失败）；缺省 `None` = 直接按步失败收场。`kind` / `role` 由 `StepNode` 派生访问器供给（`group()` / `role()`），不落独立冗余字段 | 反馈边 / 终态两语义恰好覆盖 walker 三处特殊控制流里的两处（confirm 停等是 run 级 `auto_next_phase` 开关，不是 step 属性）；`StepNode` 10 值全词汇为单源，kind/role 派生避免三字段一致性风险 |
| D2 | feedback 边的 kind 命名 | 定为 `feedback`（四值 `forward / feedback / retry / backtrack`） | explore 五轮已定名；「phase 内回边」与「phase 级 retry」语义分立，命名自然可辨 |
| D3 | eval-checklist 与相对路径清单的统一表达 | **保留独立 PhaseRecord 投影分支**，不并入相对路径清单：文件类（markdown-doc / tasks-progress）走「清单相对路径枚举 → 插件 parse → `ArtifactDescriptor.phase` 随清单」；eval-checklist 走 `(phase, attempt)` 投影（payload 自带 phase + attempt）。两者在 `discover_artifacts` 出口汇合为同一 `Vec<ArtifactDescriptor>`，`ArtifactDescriptor` 增 `phase` 字段承载归属 | eval-checklist 天然以 PhaseRecord 落账为登记、无文件可枚举，硬并入清单只会制造一个伪「相对路径」；出口统一即可，来源分支是异构本质 |
| D4 | `RunNodeRecord` / `RunEdgeRecord` 键形态 | 见「数据模型 - 执行图落库」：节点主键 `node_key: u128 = hash64(run_id)<<64 \| node_seq`（node_seq = run 内 emit 序单调计数，打包先例 `SessionEventRecord.event_key` 同构）；节点身份串 `node_id() = "{phase}:{attempt}:{step}:{instance_no}"`（attempt 跨 run max+1 唯一 → node_id 全库唯一）作边端点；边主键 `edge_key = hash64(run_id)<<64 \| edge_seq`，`run_id`（按 run 全量读）与 `from_node_key`（String 二级索引，按 from_node 读后继）两索引 | 复合主键打包沿 `pack_session_event_key` / `pack_run_step_key` 既有先例；`from_node_key` 后继查询走 String 二级索引（`session_id` 先例同式） |
| D5 | `ExecOutput::Recorded { exec_id }` 的落库载体 | **落盘文件 + 引用（不落库 blob）**：`Recorded` 全量落盘到 change 报告目录树，`RunNodeRecord.output_ref` 存 `exec_id` 引用（非全文）；test-execution 复用既有 `report_dir`（`change_test_reports/<name>/`）并统一 `exec_id` 口径，static-check 可选 `Recorded` 时落 `<report_dir>/exec-outputs/<exec_id>.log` | R5 写放大缓解：默认 `Transient`、按需 `Recorded`；「MUST NOT 全文进 `RunNodeRecord.detail`」红线保持，引用式与截断式口径不变 |
| D6 | 计划图步的 prompt 装配 | `StepDefinition.prompt_ref: Option<&'static str>` 承载 `include_str!` 静态文本（红线可验）；运行时 agent prompt 经 `PhaseNextOutcome.executor` / `.evaluator`（`ResolvedPhaseSpec`，phase_next 已组装 `context_header + prompt + reason_suffix`）按 `role` 供给，walker 不再二次组装 | prompt 全静态零占位红线不翻案；组装单点仍驻 phase_next，walker 只消费已组装产物 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| requirement 计划图单源 | `PhaseDefinition` 增 `steps: [StepDefinition]` + `artifact_manifest: [ArtifactSpec]`；`StepNode` 10 值全词汇、`OnFail` 语义、`ArtifactSpec` 相对路径清单；`prerequisites` 依赖边保留；`include_str!` + const 表装配不变 | `crates/core/workflow/src/write/phase_table.rs` | agent（ModelLevel）、state（StepDefinition/ArtifactSpec） | Rust（sync，编译期 const） |
| 计划图侧类型 | `StepDefinition` / `StepNode` / `StepGroup` / `AgentRole` / `OnFail` / `ArtifactSpec` 全静态词汇；`RunStepKind` 5→10 全词汇、`RunNodeStateRecord` / `RunEdgeStateRecord` / `RunNodeEntry` / `RunEdgeEntry`、`RunFinishCommand` 换 nodes+edges | `crates/core/workflow/src/state.rs` | serde、specta、model | Rust（中性状态类型） |
| 相位路由下发 | `phase_next` 响应增 `steps` + `artifact_manifest`（next 相位计划步序列与产物清单随行）；路由 / 白名单 / 重试上限语义不变 | `crates/core/workflow/src/write/phase_next.rs` | phase_table、state、store port | Rust（sync 只读路由） |
| walker 计划步通用执行 | `drive()` 改「迭代相位 `steps` 序列 + 按 `on_fail` 表达 feedback / terminal / confirm」；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量退役（门禁由 step 定义声明）；决策分叉（decision / retry / whitelist / backtrack）为 phase-next 错误触发的共享子序列保留 | `crates/core/orchestration/src/walker.rs` | port、state、control、run_history、verdict、decision、transcript、prompt | Rust、tokio async |
| 编排步节点与边词汇 | `ChangeStepState` 增 `instance_no` / `output_ref`；新增 `EdgeKind`（四值）与 `ChangeStepEdge`；`RunUpdate` / `RunNotice` 增 `Edge` 变体；`ChangeRunSnapshot` 增 `edges` | `crates/core/orchestration/src/state.rs` | serde、specta、agent | Rust |
| 节点 + 边整包投影 | `persisted_step` 10→5 过滤退役；`finish_command` 改组装 `RunNodeEntry` + `RunEdgeEntry` 整包；`StoreRunHistory` 适配器不变 | `crates/core/orchestration/src/run_history.rs` | workflow::state、port | Rust（sync） |
| 节点 + 边累积器 | `RunEntry` 自持 `nodes`（emit 序、`instance_no` 分配）与 `edges`（四 kind 显式落边）；`publish` 步边双累积；快照含 nodes + edges；进程内零 IO 不变量保持 | `crates/core/orchestration/src/control.rs` | state、tokio sync | Rust、Mutex + broadcast + oneshot |
| 执行图落库模型 | `RunNodeRecord`（全词汇 + 稳定身份键 + `output_ref`）/ `RunEdgeRecord`（四 kind + 两二级索引）；`RunStepRecord` 退役；native_model 版本段演进 + 格式版本 bump | `crates/infra/store/src/model.rs` | native_db、native_model、workflow::state | Rust（serde） |
| run 域操作面 | `finish_change_run` 改节点 + 边整包单事务；`list_run_nodes` / `list_run_edges` 取代 `list_run_steps`（`run_id` / `from_node_key` 索引）；启动标定 sweep 不变 | `crates/infra/store/src/store.rs` | model、workflow::state | Rust（sync，redb 事务） |
| `ChangeStateStore` 适配器 | `impl ChangeStateStore for Store` 随 trait 演进（`list_run_steps` → `list_run_nodes` / `list_run_edges` 委托同名操作面、`run_finish` 载荷换 nodes + edges）；`StoreError` → `StoreFault` 映射单点不变 | `crates/infra/store/src/change_port.rs` | store、workflow::state | Rust（sync，委托胶水） |
| 详情聚合 nodes/edges 出线 | `ChangeDetail` 出线 `runs[].nodes`（全词汇 + 稳定身份键 + `outputRef`）+ `runs[].edges`（四 kind）取代 `runs[].steps`；产物发现改清单驱动 | `crates/core/workflow/src/queries/detail.rs` | state、artifacts、foundation::layout | Rust（sync） |
| 清单驱动产物发现 | `discover_artifacts` 改「读 phase 清单 → 相对路径枚举 → 插件 parse」；`enumerate_candidates` / `collect_files` 全树扫描退役；`ArtifactDescriptor` 增 `phase`；markdown-doc / tasks-progress 的 `matches` 改「清单类型判定」 | `crates/core/workflow/src/artifacts/registry.rs` | envelope、eval_checklist、markdown_doc、tasks_progress、write::phase_table | Rust（sync） |
| 输出记录单点 | `ExecOutput::Transient` / `Recorded { exec_id }`；spawn 捕获 + 落盘 / 回查接口 | `crates/infra/exec/src/lib.rs` | 无（std + tokio process） | Rust、tokio async |
| 检查域输出消费 | static-check 输出捕获改经 `ExecOutput`（默认 Transient / 可选 Recorded）；test-execution `report_dir` 与 `exec_id` 统一；checks 层零私有 stdout 捕获 | `crates/infra/checks/src/static_check.rs` + `testexec/` | infra/exec、orchestration::port | Rust、tokio async |
| 通知桥与 DTO 装配 | `get_change_detail` 的 `ActiveRunView` 增 `edges`；`change_flow_start` 组合根随 `ExecOutput` 注入；stop / confirm / answer 请求-应答不变 | `src/commands/change_flow/mod.rs` + `changes/mod.rs` | orchestration、workflow、tauri | Rust、tauri IPC |
| 前端边表直读 | `deriveEdges` Δ 派生退役、改读 `runs[].edges`（四 kind）；节点身份键 `(phase, attempt, step, instanceNo)` 直读 | `src/views/changes/flow/graph.ts` | types、layout | TypeScript |
| 前端清单归属 | `docColumn` 静态映射退役、改读后端 `ArtifactDescriptor.phase` 清单归属；eval-checklist 走 PhaseRecord 投影分支 | `src/views/changes/flow/attachments.ts` | types | TypeScript |
| 前端稳定身份渲染 | `runStepNodes` 槽位配对 + `:seq` 堆叠退役、节点按后端 `(phase, attempt, step, instanceNo)` 直渲染；feedback 多实例不串扰 | `src/views/changes/flow/run-state.ts` | types、layout | TypeScript |
| 前端类型跟随 | `FlowEdgeKind` 增 `feedback`；`RuntimeFlowNode` 增 `instanceNo` / `outputRef`；`FlowEdge` 读边表 | `src/views/changes/flow/types.ts` | dto 类型 | TypeScript |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。本变更零新增文件、零删除文件——全部为既有文件修改。 -->

### 新增文件

<!-- 不涉及：本变更全部落既有文件（proposal「删除文件」无整文件删除、「实现文件」20 条均为修改）。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/workflow/src/write/phase_table.rs` | `PhaseDefinition` 增 `steps: [StepDefinition]` + `artifact_manifest: [ArtifactSpec]`；const 表逐相位声明步序列与产物清单（proposal/design 相位 executor/evaluator 步 + verdict gate + phase-log；implement/test-gen 增 static-check 步携 `FeedbackLoop{5}`；test-execution 相位声明 test-execution 步 + phase-log；code-review/acceptance 无 executor 步）；`MAX_RETRY_TIMES` / `prerequisites` / `dependents` / `allowed_backtrack_phases` 保留 | 计划图单源；prompt 全静态零占位红线不翻案 |
| `crates/core/workflow/src/state.rs` | 新增 `StepDefinition` / `StepNode` / `StepGroup` / `AgentRole` / `OnFail` / `ArtifactSpec`；`RunStepKind` 5→10 全词汇；`RunStepStateRecord` 退役、增 `RunNodeStateRecord` / `RunEdgeStateRecord`；`RunStepEntry` 退役、增 `RunNodeEntry` / `RunEdgeEntry`；`RunFinishCommand.steps` → `nodes` + `edges`；`ChangeStateStore` 增 `list_run_nodes` / `list_run_edges`、删 `list_run_steps` | 计划图 + 执行图类型单源 |
| `crates/core/workflow/src/write/phase_next.rs` | `PhaseNextOutcome` 增 `steps: Vec<StepDefinition>` + `artifact_manifest: Vec<ArtifactSpec>`；`build_phase_response` 随 next 相位下发步序列与清单 | 计划步序列下发；路由权威不变 |
| `crates/core/workflow/src/write/mod.rs` | 再导出 `StepDefinition` / `StepNode` / `StepGroup` / `AgentRole` / `OnFail` / `ArtifactSpec` 与 `RunNodeStateRecord` / `RunEdgeStateRecord` / `RunNodeEntry` / `RunEdgeEntry` | 计划图 + 执行图类型出口面跟随 |
| `crates/core/orchestration/src/walker.rs` | `drive()` 改迭代 `phase.steps`（按 `StepNode.group()` 分派 exec/agent/gate）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量退役（删除）；`STATIC_CHECK_FEEDBACK_LIMIT` / `TEST_EXECUTION_FEEDBACK_LIMIT` 下沉为 `OnFail::FeedbackLoop { limit }`；feedback 重跑 = 新节点（`instance_no` 递增）+ `feedback` 边；verdict 解析失败 = `OnFail::Terminal`；决策分叉（decision / retry-gate / whitelist-gate / backtrack）保留为 phase-next 错误触发的共享子序列 | walker 零相位转移规则红线不变 |
| `crates/core/orchestration/src/state.rs` | `ChangeStepState` 增 `instance_no: u32` + `output_ref: Option<String>`；新增 `EdgeKind`（`Forward` / `Feedback` / `Retry` / `Backtrack`）+ `ChangeStepEdge`（`from` / `to` / `kind`）；`ChangeRunSnapshot` 增 `edges: Vec<ChangeStepEdge>`；`RunUpdate` / `RunNotice` 增 `Edge` 变体 | 步节点稳定身份键 + 边四值词汇 |
| `crates/core/orchestration/src/run_history.rs` | `persisted_step` 10→5 过滤整个退役（删除）；`finish_command` 改组装 `nodes` + `edges` 整包（`RunNodeEntry` 全词汇直映射 + `RunEdgeEntry` 四 kind）；`StoreRunHistory` 不变 | 节点 + 边整包投影；全词汇零过滤 |
| `crates/core/orchestration/src/control.rs` | `RunEntry.steps` → `nodes` + `edges`（节点 `instance_no` 分配、边四 kind 显式落边）；`publish` 处理 `RunUpdate::Edge`；`RunGuard::steps()` → `nodes()` / `edges()`；快照含 nodes + edges；头注释口径同步 | 节点 + 边累积器；进程内零 IO 不变 |
| `crates/core/orchestration/src/lib.rs` | 摘除 `persisted_step` / `STATIC_CHECK_PHASES` / `STATIC_CHECK_FEEDBACK_LIMIT` 再导出；挂载 `EdgeKind` / `ChangeStepEdge` 再导出 | 出口面跟随 |
| `crates/core/orchestration/src/port.rs` | `StaticCheckOutcome` 增 `output_ref: Option<String>`、`TestExecutionOutcome` 增 `exec_id: String`（与 `report_dir` 统一口径）；`ToolStepOutput` 随动 | 输出记录引用随 step 产出透传 |
| `crates/infra/store/src/model.rs` | 新增 `RunNodeRecord`（`node_key` 主键、`run_id` / `change_id`、`phase` / `attempt` / `step` / `instance_no`、`status` / `session_id` / `detail` / `output_ref` / `timestamp`）；新增 `RunEdgeRecord`（`edge_key` 主键、`run_id` / `from_node_key` / `to_node_key` / `edge_kind` / `timestamp`）；`RunStepRecord` 退役；`WORKSPACE_STORE_FORMAT_VERSION` 2→3；native_model id 版本段演进（14 退役、新增节点 / 边两模型 id） | 执行图 model 重设计，零迁移链 |
| `crates/infra/store/src/store.rs` | `workspace_models` 注册 `RunNodeRecord` / `RunEdgeRecord`、摘 `RunStepRecord`；`finish_change_run` 改节点 + 边整包单事务；`list_run_steps` → `list_run_nodes`（`run_id` 序）+ `list_run_edges`（`run_id` 全量 / `from_node_key` 后继两查）；启动标定 sweep 不变 | run 域操作面重设计 |
| `crates/infra/store/src/change_port.rs` | `impl ChangeStateStore for Store` 随 trait 演进：`list_run_steps` → `list_run_nodes` / `list_run_edges`（委托同名 store 操作面）、`run_finish` 载荷换 nodes + edges；`fault` 错误面映射单点不变 | ChangeStateStore 适配器随 trait 演进 |
| `crates/core/workflow/src/queries/detail.rs` | `ChangeRunEntry.steps` → `nodes` + `edges`；新增 `ChangeRunNodeRecord` / `ChangeRunEdgeRecord` DTO（`step` 全词汇、`instanceNo`、`outputRef`、边 `edgeKind` 四值）；`change_detail` 出线 nodes / edges（取代 seq / steps）；产物发现改清单驱动（读 `phase_table(record.workflow_type)` 取 manifest） | 详情聚合 nodes/edges 出线 |
| `crates/core/workflow/src/artifacts/registry.rs` | `discover_artifacts(change_dir, phases, manifest)` 改「读 phase 清单 → 相对路径枚举匹配 → 插件 parse」；`enumerate_candidates` / `collect_files` 全树扫描退役；`ArtifactDescriptor` 增 `phase`；`ArtifactInput` 增清单归属上下文；markdown-doc / tasks-progress 的 `matches` 改「清单类型 == kind」 | 清单驱动发现；白名单取代全量扫描 |
| `crates/core/workflow/src/artifacts/markdown_doc.rs` | `matches` 改「清单命中的 markdown-doc 候选项」而非「扩展名 == md」；title/rank 名单逻辑保留 | Docs 探测折叠进清单驱动 |
| `crates/core/workflow/src/artifacts/tasks_progress.rs` | `matches` 改「清单命中的 tasks.md 候选项」；勾选计数 parser 保留 | tasks.md 特化视图改清单来源 |
| `crates/core/workflow/src/artifacts/envelope.rs` | `ArtifactDescriptor` 增 `phase: String`；`ArtifactCandidate` 语义收缩（文件候选来自清单、eval 候选来自 PhaseRecord 投影） | 产物归属出线 |
| `crates/core/workflow/src/artifacts/mod.rs` | 再导出随 `ArtifactDescriptor.phase` / `discover_artifacts` 签名演进 | 出口面跟随 |
| `crates/infra/exec/src/lib.rs` | 新增 `ExecOutput` 枚举（`Transient` / `Recorded { exec_id }`）与落盘 / 回查接口（`record_exec_output` / `read_exec_output`）；`SystemCommand` 不变 | 输出记录单点 |
| `crates/infra/checks/src/static_check.rs` + `testexec/` | static-check 输出捕获改经 `ExecOutput`（默认 `Transient` / 可选 `Recorded`）；test-execution `report_dir` 与 `exec_id` 统一；checks 层零私有 stdout 捕获 / 落盘分支 | 输出记录收敛到 infra/exec |
| `src/commands/change_flow/mod.rs` + `changes/mod.rs` | `ActiveRunView` 增 `edges`；`change_flow_start` 组合根随 `ExecOutput` 记录器装配；`get_change_detail` 的 nodes/edges DTO 随 shape 演进；通知桥随 `RunNotice::Edge` 演进 | nodes/edges DTO 装配与通知桥 |
| `src/views/changes/flow/graph.ts` | `deriveEdges` Δ 派生退役、改读 `runs[].edges`（四 kind）；边端点按节点稳定身份键寻址 | 边表直读 |
| `src/views/changes/flow/attachments.ts` | `docColumn` 静态映射退役、改读 `detail.artifacts[].phase` 清单归属；eval-checklist 走 PhaseRecord 投影分支 | 清单归属 |
| `src/views/changes/flow/run-state.ts` | `runStepNodes` 槽位配对 + `:seq` 堆叠退役、节点按后端 `(phase, attempt, step, instanceNo)` 直渲染；`unifiedRunSteps` 改读 `runs[].nodes` | 稳定身份渲染 |
| `src/views/changes/flow/types.ts` | `FlowEdgeKind` 增 `feedback`；`RuntimeFlowNode` 增 `instanceNo` / `outputRef`；`FlowEdge` 改读边表 | 前端类型演进 |
| `src/types/generated/bindings.ts` | 经 `bindings:export` 再生成（nodes / edges / edgeKind / instanceNo / outputRef / ArtifactDescriptor.phase） | 非手改 |
| `packages/desktop/package.json` | `version` 0.4.34 → 0.4.35 | 版本交付 |

### 公共函数 / API

<!-- 仅列模块级导出函数 / CLI 子命令 / HTTP 端点；私有函数不列。 -->

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `phase_table` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` | 返回项 `PhaseDefinition` 增 steps / artifact_manifest；V1 仍仅 `requirement` |
| `allowed_backtrack_phases` | `crates/core/workflow/src/write/phase_table.rs` | 不变 | `pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String>` | 白名单语义不变 |
| `phase_next` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | `pub fn phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | `PhaseNextOutcome` 增 steps / artifact_manifest 下发 |
| `discover_artifacts` | `crates/core/workflow/src/artifacts/registry.rs` | 修改 | `pub fn discover_artifacts(change_dir: &Path, phases: &[PhaseStateRecord], manifest: &[PhaseDefinition]) -> Vec<ArtifactDescriptor>` | 清单驱动枚举；`enumerate_candidates` 全树扫描退役 |
| `read_artifact` | `crates/core/workflow/src/artifacts/registry.rs` | 修改 | `pub fn read_artifact(change_dir: &Path, phases: &[PhaseStateRecord], kind: &str, source: &str) -> Option<ArtifactEnvelope>` | 按 kind + source 解析单信封；source 校验口径不变 |
| `change_detail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | `pub fn change_detail(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Option<ChangeDetail>` | 出线 nodes/edges 取代 steps |
| `new_run_id` | `crates/core/orchestration/src/walker.rs` | 不变 | `pub fn new_run_id() -> String` | 铸造 `run-<millis>` |
| `walk_run` | `crates/core/orchestration/src/walker.rs` | 修改 | `pub async fn walk_run(worker: Arc<dyn WorkerAgentPort>, tools: Arc<dyn ToolStepPort>, snapshot: Arc<dyn WorkflowSnapshotPort>, history: Arc<dyn RunHistoryPort>, control: Arc<ChangeFlowControl>, guard: RunGuard, request: RunRequest) -> ChangeRunStatus` | 计划步通用执行；两写时序保留 |
| `persisted_step` | `crates/core/orchestration/src/run_history.rs` | 删除（退役） | — | 10→5 过滤整个退役；`ChangeStepKind` 与 `RunStepKind` 改 1:1 全映射 |
| `finish_command` | `crates/core/orchestration/src/run_history.rs` | 修改 | `pub fn finish_command(request: &RunRequest, status: ChangeRunStatus, reason: Option<String>, finished_at: i64, nodes: &[ChangeStepState], edges: &[ChangeStepEdge]) -> RunFinishCommand` | 组装节点 + 边整包 |
| `StoreRunHistory::new` | `crates/core/orchestration/src/run_history.rs` | 不变 | `pub fn new(store: Arc<dyn ChangeStateStore>) -> Self` | store 适配器 |
| `SystemCommand`（公开方法不变） | `crates/infra/exec/src/lib.rs` | 不变 | `bare` / `script` / `script_raw` / `script_with` / `arg` / `args` / `into_std` / `into_tokio` | spawn 封装；零改动 |
| `record_exec_output` | `crates/infra/exec/src/lib.rs` | 新增 | `pub fn record_exec_output(root: &Path, change: &str, stdout: &[u8], stderr: &[u8]) -> Result<String, String>`（`exec_id` 铸造 + 落盘；返回 `exec_id` 引用） | `ExecOutput::Recorded` 落盘半边 |
| `read_exec_output` | `crates/infra/exec/src/lib.rs` | 新增 | `pub fn read_exec_output(root: &Path, change: &str, exec_id: &str) -> Option<Vec<u8>>` | `ExecOutput::Recorded` 回查半边 |
| `start_change_run` | `crates/infra/store/src/store.rs` | 不变 | `pub fn start_change_run(&self, command: &RunStartCommand) -> Result<(), StoreError>` | run 起始第一写 |
| `finish_change_run` | `crates/infra/store/src/store.rs` | 修改 | `pub fn finish_change_run(&self, command: &RunFinishCommand) -> Result<(), StoreError>` | 终态 + 节点整包 + 边整包 + active_phase 清位单事务 |
| `list_change_runs` | `crates/infra/store/src/store.rs` | 不变 | `pub fn list_change_runs(&self, change_id: &str) -> Result<Vec<RunStateRecord>, StoreError>` | run 清单 |
| `list_run_nodes` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_run_nodes(&self, run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreError>` | 取代 `list_run_steps` |
| `list_run_edges` | `crates/infra/store/src/store.rs` | 新增 | `pub fn list_run_edges(&self, run_id: &str) -> Result<Vec<RunEdgeStateRecord>, StoreError>` | 按 run 全量读边（`from_node_key` 后继查询另经二级索引） |
| `calibrate_interrupted_runs` | `crates/infra/store/src/store.rs` | 不变 | `pub fn calibrate_interrupted_runs(&self, now: i64) -> Result<Vec<String>, StoreError>` | 启动标定 |
| `get_change_detail`（IPC） | `src/commands/changes/mod.rs` | 修改 | `#[tauri::command] pub fn get_change_detail(stores: State<'_, WorkspaceStores>, control: State<'_, Arc<ChangeFlowControl>>, root: String, id: String) -> Option<ChangeDetailUnified>` | DTO shape 演进（nodes/edges/edges 活面） |
| `change_flow_start` / `change_flow_stop` / `change_flow_answer` / `change_flow_confirm` / `change_flow_watch`（IPC） | `src/commands/change_flow/mod.rs` | 不变（签名） | 见源码 | 请求-应答语义不变；仅组合根随 ExecOutput 装配 |

### 类型定义

<!-- 含 interface / type alias / enum / 公共 API class。 -->

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `StepGroup` | `crates/core/workflow/src/state.rs` | 新增 | `enum StepGroup { Exec, Agent, Gate }`——三类节点分组 |
| `AgentRole` | `crates/core/workflow/src/state.rs` | 新增 | `enum AgentRole { Executor, Evaluator, Decision }`——agent 步角色 |
| `StepNode` | `crates/core/workflow/src/state.rs` | 新增 | `enum StepNode { Executor, Evaluator, Decision, PhaseStart, StaticCheck, TestExecution, PhaseLog, VerdictGate, RetryGate, WhitelistGate }`——10 值全词汇；`group()` / `role()` 派生 |
| `OnFail` | `crates/core/workflow/src/state.rs` | 新增 | `enum OnFail { FeedbackLoop { limit: u32 }, Terminal }`——步失败语义 |
| `StepDefinition` | `crates/core/workflow/src/state.rs` | 新增 | `struct StepDefinition { node: StepNode, prompt_ref: Option<&'static str>, on_fail: Option<OnFail> }`——计划图步节点 |
| `ArtifactSpec` | `crates/core/workflow/src/state.rs` | 新增 | `struct ArtifactSpec { kind: &'static str, path: &'static str, title: &'static str }`——产物清单项（相对路径模式） |
| `PhaseDefinition` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | 增 `steps: [StepDefinition]` 与 `artifact_manifest: [ArtifactSpec]` |
| `PhaseNextOutcome` | `crates/core/workflow/src/write/phase_next.rs` | 修改 | 增 `steps: Vec<StepDefinition>` + `artifact_manifest: Vec<ArtifactSpec>` |
| `RunStepKind` | `crates/core/workflow/src/state.rs` | 修改 | 5 值 → 10 值全词汇（增 `PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`） |
| `RunStepStateRecord` | `crates/core/workflow/src/state.rs` | 删除（退役） | 被 `RunNodeStateRecord` 取代 |
| `RunNodeStateRecord` | `crates/core/workflow/src/state.rs` | 新增 | `{ run_id, change_id, phase, attempt, step: RunStepKind, instance_no, status, session_id, detail, output_ref, timestamp }` |
| `RunEdgeStateRecord` | `crates/core/workflow/src/state.rs` | 新增 | `{ run_id, change_id, from_node_key, to_node_key, edge_kind, timestamp }` |
| `RunStepEntry` | `crates/core/workflow/src/state.rs` | 删除（退役） | 被 `RunNodeEntry` 取代 |
| `RunNodeEntry` | `crates/core/workflow/src/state.rs` | 新增 | `{ phase, attempt, step, instance_no, status, session_id, detail, output_ref }` |
| `RunEdgeEntry` | `crates/core/workflow/src/state.rs` | 新增 | `{ from_node_key, to_node_key, edge_kind }` |
| `RunFinishCommand` | `crates/core/workflow/src/state.rs` | 修改 | `steps: Vec<RunStepEntry>` → `nodes: Vec<RunNodeEntry>` + `edges: Vec<RunEdgeEntry>` |
| `ChangeStepState` | `crates/core/orchestration/src/state.rs` | 修改 | 增 `instance_no: u32` + `output_ref: Option<String>` |
| `EdgeKind` | `crates/core/orchestration/src/state.rs` | 新增 | `enum EdgeKind { Forward, Feedback, Retry, Backtrack }`（camelCase 线格式） |
| `StaticCheckOutcome` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `output_ref: Option<String>`（`ExecOutput::Recorded` 的 `exec_id` 引用） |
| `TestExecutionOutcome` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `exec_id: String`（与既有 `report_dir` 统一口径） |
| `ChangeStepEdge` | `crates/core/orchestration/src/state.rs` | 新增 | `{ from: NodeKey, to: NodeKey, kind: EdgeKind }`（`NodeKey = (String, u32, ChangeStepKind, u32)`） |
| `ChangeRunSnapshot` | `crates/core/orchestration/src/state.rs` | 修改 | 增 `edges: Vec<ChangeStepEdge>` |
| `RunUpdate` / `RunNotice` | `crates/core/orchestration/src/state.rs` | 修改 | 增 `Edge { edge: ChangeStepEdge }` 变体 |
| `RunNodeRecord` | `crates/infra/store/src/model.rs` | 新增 | 执行图节点行（native_model 新 id + 版本） |
| `RunEdgeRecord` | `crates/infra/store/src/model.rs` | 新增 | 执行图边行（native_model 新 id + 版本） |
| `RunStepRecord` | `crates/infra/store/src/model.rs` | 删除（退役） | 被 `RunNodeRecord` / `RunEdgeRecord` 取代 |
| `ChangeRunStepRecord` | `crates/core/workflow/src/queries/detail.rs` | 删除（退役） | 被 `ChangeRunNodeRecord` 取代 |
| `ChangeRunNodeRecord` | `crates/core/workflow/src/queries/detail.rs` | 新增 | `{ phase, attempt, step: RunStepKind, instance_no, status, session_id, detail, output_ref }` |
| `ChangeRunEdgeRecord` | `crates/core/workflow/src/queries/detail.rs` | 新增 | `{ from_node_key, to_node_key, edge_kind }` |
| `ChangeRunEntry` | `crates/core/workflow/src/queries/detail.rs` | 修改 | `steps` → `nodes: Vec<ChangeRunNodeRecord>` + `edges: Vec<ChangeRunEdgeRecord>` |
| `ArtifactDescriptor` | `crates/core/workflow/src/artifacts/envelope.rs` | 修改 | 增 `phase: String` |
| `ExecOutput` | `crates/infra/exec/src/lib.rs` | 新增 | `enum ExecOutput { Transient, Recorded { exec_id: String } }` |

### 配置

<!-- 不涉及除版本外的配置项变更；无新增配置键。`version` 为静态核对项，由验收阶段核对。 -->

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.4.35` | 0.4.34 → 0.4.35（`tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 版本不随动） |

---

## 数据模型

### 计划图（源码，编译期静态，不落库）

```
WorkflowDefinition（"requirement"，隐式 = REQUIREMENT_TABLE）
 ├─ PhaseDefinition（8 相位 const 表）
 │   ├─ id / description
 │   ├─ steps: [StepDefinition]                 ← 计划图节点
 │   │    ├─ node: StepNode（10 值全词汇）
 │   │    ├─ prompt_ref: Option<&'static str>（agent 步；include_str! 静态文本）
 │   │    └─ on_fail: Option<OnFail>（FeedbackLoop{limit} | Terminal）
 │   ├─ artifact_manifest: [ArtifactSpec]        ← phase 声明产物清单
 │   │    └─ kind · path（相对路径模式）· title
 │   ├─ executor / evaluator: Option<PhaseAgentSpec>（prompt + model_level，保持）
 │   └─ prerequisites（依赖边，plan 图形态，stale 传播源）
```

各相位 `steps` 声明（计划图线性体；`phase-next` / `confirm` 为 run 级循环骨架、决策分叉为 phase-next 错误触发的共享子序列，均不入相位 steps）：

| 相位 | steps 序列 | 备注 |
|------|-----------|------|
| proposal / dev-design / test-design | `PhaseStart` → `Executor(agent)` → `Evaluator(agent)` → `VerdictGate(on_fail=Terminal)` → `PhaseLog` | executor 步无 on_fail（会话失败统一 Terminal） |
| implement / test-gen | 上序列在 `Executor` 后插 `StaticCheck(exec, on_fail=FeedbackLoop{5})` | 门禁由 step 声明（原 `STATIC_CHECK_PHASES` 退役） |
| test-execution | `PhaseStart` → `TestExecution(exec, on_fail=FeedbackLoop{5})` → `PhaseLog` | 绿跑零 agent（原 `TEST_EXECUTION_PHASES` 退役） |
| code-review / acceptance | `PhaseStart` → `Evaluator(agent)` → `VerdictGate(on_fail=Terminal)` → `PhaseLog` | executor=None → 无 Executor 步 |

`artifact_manifest` 声明（相对路径 → phase 归属权威来源）：

| phase | ArtifactSpec（kind · path · title） |
|-------|-------------------------------------|
| proposal | `markdown-doc · proposal.md · 提案`、`markdown-doc · specs/** · 规格` |
| dev-design | `markdown-doc · design.md · 设计`、`tasks-progress · tasks.md · 任务进度` |
| test-execution | `markdown-doc · test-reports/** · 测试报告` |

### 执行图（db，仅执行记录）

```
RunRecord          run 主行（run_id 主键 / change_id 二级索引 / status / reason / started_at / finished_at）
RunNodeRecord      执行图节点（node_key 主键 = hash64(run_id)<<64 | node_seq）
                   ├─ run_id（二级索引）/ change_id
                   ├─ 稳定身份键 (phase, attempt, step, instance_no)
                   ├─ status / session_id / detail / output_ref / timestamp
RunEdgeRecord      执行图边（edge_key 主键 = hash64(run_id)<<64 | edge_seq）
                   ├─ run_id（二级索引，按 run 全量读边）
                   ├─ from_node_key（二级索引，String，按 from_node 读后继）
                   ├─ to_node_key（目标节点身份串）
                   └─ edge_kind ∈ { forward, feedback, retry, backtrack }
```

- **节点身份键** `(run_id, phase, attempt, step, instance_no)`：`instance_no` 每次执行递增（feedback 重跑 = 独立新节点）；`attempt` 经 `phase_start` max+1 分配跨 run 不撞号 → 节点身份串 `node_id() = "{phase}:{attempt}:{step}:{instance_no}"` 全库唯一，作边端点。
- **边 kind 四值**：`forward` 计划序推进 / `feedback` gate 失败回到同 phase agent 步（phase 内回边）/ `retry` phase fail 后同相位重跑 / `backtrack` 跨相位回跳。边端点均指向节点身份串。
- **每 run 两写**：run 发起 `start_change_run`（running 行）；run 收口 `finish_change_run` 单事务（终态 + 节点整包 + 边整包 + active_phase 清位）。无每步写事务；任一环节失败整体回滚（无半包节点 / 半包边）。
- **退役**：`RunStepRecord.seq` + `persisted_step` 10→5 过滤整体退役；`RunNodeRecord` 全词汇零过滤。

### 边 emit 规则（walker 侧单点）

| 边 | 触发 | 端点 |
|----|------|------|
| `forward` | 相位内按 steps 序推进、phase 间推进 | 上一节点 → 下一节点 |
| `feedback` | FeedbackLoop 步失败 → 重跑 executor 修复节点（新 instance_no） | 失败 gate/exec 节点 → executor 修复节点 |
| `retry` | 相位 fail 后同相位重跑（新 attempt） | 上一 attempt 末节点 → 新 attempt phase-start 首节点 |
| `backtrack` | 决策 backtrack 动作 | 决策/回溯节点 → 目标相位首节点 |

### artifact 关联与发现

- 关联键 = 相对 POSIX 路径（`encode_file_source` 口径不变）；归属 = 源码 phase 清单（`ArtifactDescriptor.phase`）。
- `discover_artifacts(change_dir, phases, manifest)`：文件候选 = 按 `manifest` 各 `ArtifactSpec.path` 枚举匹配（精确路径 / `**` 前缀 glob），eval 候选 = `phases`（PhaseRecord 投影，eval-checklist 独立分支）；matcher 仅在清单命中的候选项内判定 kind。
- `enumerate_candidates` / `collect_files` 全树扫描退役；`docColumn` 前端静态映射退役。

### stdout 输出记录

- `ExecOutput::Transient`：spawn 即得、反馈边消费、即弃（无 id，全量不进库 / 不落盘引用）。
- `ExecOutput::Recorded { exec_id }`：stdout 全量落盘到 change 报告目录树，`exec_id` 供 `RunNodeRecord.output_ref` 引用回查。
- static-check 默认 `Transient`（反馈边消费即够）、可选 `Recorded`；test-execution 复用既有 `report_dir` 并统一 `exec_id`；会话记录继续走 `SessionEventRecord`（`session_id` 重放唯一来源），不内嵌。

---

## 路由 / API 设计

<!-- 无 HTTP 端点；desktop 为 Tauri IPC 命令面。IPC 命令签名不变（stop / confirm / answer / watch 请求-应答语义零改动），仅 `get_change_detail` 出线 DTO 形状演进（wire contract 变更，走 golden 显式重写）。 -->

| method / path | 说明 | 输入 | 输出 | auth |
|---------------|------|------|------|------|
| `get_change_detail`（Tauri IPC） | 详情聚合统一视图 | `root: String, id: String` | `Option<ChangeDetailUnified>`：`detail.runs[].nodes`（`phase/attempt/step/instanceNo/status/sessionId/detail/outputRef`）与 `detail.runs[].edges`（`fromNodeKey/toNodeKey/edgeKind`）取代 `steps`；`activeRun.edges` 活面 | 无（本机桌面） |
| `change_flow_start`（Tauri IPC） | run 发起 | `root, id, auto_next_phase: bool` | `ChangeRunSummary`（不变） | 无 |

其余 IPC 命令（`change_flow_stop` / `change_flow_answer` / `change_flow_confirm` / `change_flow_watch`）签名与语义不变；通知信封 `RunNotice` 增 `Edge` kind-only 变体（查询结果权威，客户端收通知重查统一视图）。

---

## 依赖

### 运行时依赖

- 无新增外部 crate：执行图 model 走既有 `native_db` / `native_model`；artifact 清单 glob 匹配为 core 内手写（零 infra 红线）；`ExecOutput` 落盘走 std + 既有 `foundation::layout`。
- `crates/infra/checks` → `crates/infra/exec`（既有依赖，新增 `ExecOutput` 消费）。
- `crates/core/workflow/src/artifacts` → `crates/core/workflow/src/write::phase_table`（同 crate 内，清单读取）。

### 构建 / 测试依赖

- 无新增：golden 走既有 `DESKTOP_GOLDEN_REWRITE=1` 流程；bindings 走既有 `bindings:export` / `bindings:check`；语料走既有 `corpus_golden_test.rs` 种子构造器（扩展 run 维度节点 / 边样本）。

---

## 待决问题

- 无——proposal 四个待决问题已由 D1 / D3 / D4 / D5 定稿；无新增未决事项。
