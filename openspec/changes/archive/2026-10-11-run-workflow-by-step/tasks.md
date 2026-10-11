# 任务: run-workflow-by-step

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖实现文件。顺序按依赖排列：计划图类型 → 计划图单源与下发 → 执行图类型与落库 → 输出记录与端口字段（infra/exec + orchestration/port）→ 检查域输出消费 → 编排走查 → artifact 清单驱动发现 → 详情出线 → 命令面 → 前端 → 交付。
>
> 依赖序要点（D4）：walker.rs 的 `emit_step` 消费 `StaticCheckOutcome.output_ref` / `TestExecutionOutcome.exec_id`，故 `orchestration/port.rs` 的字段扩展与 `infra/exec` 输出记录先行于 walker；`detail.rs` 改传 `manifest` 依赖 `registry.rs` 的 `discover_artifacts` 新签名，故 artifact 清单驱动发现先行于详情聚合。

## 阶段一：core/workflow 计划图类型与单源

- [x] 修改 `crates/core/workflow/src/state.rs`：新增 `StepGroup`（Exec/Agent/Gate）、`AgentRole`（Executor/Evaluator/Decision）、`StepNode`（10 值全词汇 + `group()` / `role()` 派生）、`OnFail`（`FeedbackLoop { limit: u32 }` / `Terminal`）、`StepDefinition { node, prompt_ref, on_fail }`、`ArtifactSpec { kind, path, title }`
- [x] 修改 `crates/core/workflow/src/state.rs`：`RunStepKind` 5 值 → 10 值全词汇（增 `PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate`）；`RunStepStateRecord` / `RunStepEntry` 退役，增 `RunNodeStateRecord` / `RunEdgeStateRecord` / `RunNodeEntry` / `RunEdgeEntry`；`RunFinishCommand.steps` → `nodes: Vec<RunNodeEntry>` + `edges: Vec<RunEdgeEntry>`；`ChangeStateStore` 增 `list_run_nodes` / `list_run_edges`、删 `list_run_steps`
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：`PhaseDefinition` 增 `steps: [StepDefinition]` + `artifact_manifest: [ArtifactSpec]`；逐相位声明步序列（implement/test-gen 含 `StaticCheck(FeedbackLoop{5})`、test-execution 含 `TestExecution(FeedbackLoop{5})`、verdict 步 `Terminal`）与产物清单（proposal.md / specs/** → proposal、design.md / tasks.md → dev-design、test-reports/** → test-execution）；`include_str!` + const 表装配不变，`prerequisites` / `dependents` / `allowed_backtrack_phases` / `MAX_RETRY_TIMES` 保留
- [x] 修改 `crates/core/workflow/src/write/mod.rs`：再导出 `StepDefinition` / `StepNode` / `StepGroup` / `AgentRole` / `OnFail` / `ArtifactSpec` 与 `RunNodeStateRecord` / `RunEdgeStateRecord` / `RunNodeEntry` / `RunEdgeEntry`

## 阶段二：计划步序列下发

- [x] 修改 `crates/core/workflow/src/write/phase_next.rs`：`PhaseNextOutcome` 增 `steps: Vec<StepDefinition>` + `artifact_manifest: Vec<ArtifactSpec>`；`build_phase_response` 随 next 相位下发其 `steps` 克隆与 `artifact_manifest` 克隆；`done` / `error` 分支两字段置空（`Vec::new()`）；路由 / 白名单 / 重试上限 / 锚点语义不变

## 阶段三：执行图类型与落库（infra/store）

- [x] 修改 `crates/infra/store/src/model.rs`：新增 `RunNodeRecord`（`node_key` 主键 = `hash64(run_id)<<64 | node_seq`、`run_id` / `change_id`、`phase` / `attempt` / `step: RunStepKind` / `instance_no`、`status` / `session_id` / `detail` / `output_ref` / `timestamp`，`node_id()` 身份串）；新增 `RunEdgeRecord`（`edge_key` 主键 = `hash64(run_id)<<64 | edge_seq`、`run_id` 二级索引、`from_node_key` String 二级索引、`to_node_key`、`edge_kind`、`timestamp`）；删除 `RunStepRecord` 及其 `pack_run_step_key`；`WORKSPACE_STORE_FORMAT_VERSION` 2→3；新增节点/边两模型 native_model id + 版本段
- [x] 修改 `crates/infra/store/src/store.rs`：`workspace_models` 注册 `RunNodeRecord` / `RunEdgeRecord`、摘 `RunStepRecord`；`finish_change_run` 改「终态 + 节点整包 + 边整包 + active_phase 清位」单事务（`RunNodeEntry` / `RunEdgeEntry` 逐条 `new` 落行）；`list_run_steps` → `list_run_nodes`（`run_id` 过滤、node_seq 升序）+ `list_run_edges`（`run_id` 全量 / `from_node_key` 后继两查）；启动标定 `calibrate_interrupted_runs` 不变
- [x] 修改 `crates/infra/store/src/change_port.rs`：`ChangeStateStore` 实现随 trait 演进（`list_run_steps` → `list_run_nodes` / `list_run_edges`，`run_finish` 载荷换 nodes + edges）

## 阶段四：输出记录单点与端口字段（infra/exec → orchestration/port）

- [x] 修改 `crates/infra/exec/src/lib.rs`：新增 `ExecOutput` 枚举（`Transient` / `Recorded { exec_id }`）；新增 `record_exec_output(root, change, stdout, stderr) -> Result<String, String>`（exec_id 铸造 + 全量落盘到 change 报告目录树）与 `read_exec_output(root, change, exec_id) -> Option<Vec<u8>>`（回查）；`SystemCommand` 不变
- [x] 修改 `crates/core/orchestration/src/port.rs`：`StaticCheckOutcome` 增 `output_ref: Option<String>`、`TestExecutionOutcome` 增 `exec_id: String`（与既有 `report_dir` 统一口径）；`ToolStepOutput` 随动（不变体结构，仅内载荷字段扩展）

## 阶段五：检查域输出消费（infra/checks）

- [x] 修改 `crates/infra/checks/src/static_check.rs`：输出捕获改经 `ExecOutput`（默认 `Transient`、可选 `Recorded` 落盘并回填 `exec_id`），构造 `StaticCheckOutcome` 时填入 `output_ref`；checks 层零私有 stdout 捕获 / 落盘分支
- [x] 修改 `crates/infra/checks/src/testexec/`：`TestExecutionOutcome` 构造填 `exec_id`（与既有 `report_dir` 统一口径，`report_dir` 保留）；报告全量仍落 change 报告目录；checks 层零私有 stdout 捕获 / 落盘分支

## 阶段六：core/orchestration 走查与累积

- [x] 修改 `crates/core/orchestration/src/state.rs`：`ChangeStepState` 增 `instance_no: u32` + `output_ref: Option<String>`；新增 `EdgeKind`（`Forward`/`Feedback`/`Retry`/`Backtrack`，camelCase 线格式）与 `ChangeStepEdge { from, to, kind }`；`ChangeRunSnapshot` 增 `edges: Vec<ChangeStepEdge>`；`RunUpdate` / `RunNotice` 增 `Edge { edge: ChangeStepEdge }` 变体（`kind()` 增 `"edge"` 判别、`From` 增 `Edge` 投影）
- [x] 修改 `crates/core/orchestration/src/control.rs`：`RunEntry.steps` → `nodes: Vec<ChangeStepState>` + `edges: Vec<ChangeStepEdge>`；`publish` 处理 `RunUpdate::Edge`（边累积）与 `RunUpdate::Step`（节点累积 + `instance_no` 分配）；`RunGuard::steps()` → `nodes()` / `edges()`；快照含 nodes + edges；头注释口径同步（进程内零 IO 不变量保持）
- [x] 修改 `crates/core/orchestration/src/run_history.rs`：删除 `persisted_step`（10→5 过滤退役）；`finish_command` 改组装 `RunNodeEntry`（`ChangeStepKind` → `RunStepKind` 1:1 全映射）+ `RunEdgeEntry`（`ChangeStepEdge` → 边端点身份串）整包；`StoreRunHistory` 保持
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`drive()` 改迭代 `next.steps`（按 `StepNode.group()` 分派 exec/agent/gate 步执行）；`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` 常量删除；`STATIC_CHECK_FEEDBACK_LIMIT` / `TEST_EXECUTION_FEEDBACK_LIMIT` 下沉为 `OnFail::FeedbackLoop { limit }`；feedback 重跑 = `instance_no` 递增新节点 + `feedback` 边（emit 规则见 design）；verdict 解析失败 = `OnFail::Terminal`；`emit_step` 增 `instance_no` / `output_ref` 参数（static-check / test-execution 步消费 `StaticCheckOutcome.output_ref` / `TestExecutionOutcome.exec_id`）；决策分叉（decision / retry-gate / whitelist-gate / backtrack）保留为 phase-next 错误触发的共享子序列；`walk_run` 终态出口改传 `guard.nodes()` / `guard.edges()`
- [x] 修改 `crates/core/orchestration/src/lib.rs`：摘除 `persisted_step` / `STATIC_CHECK_PHASES` 再导出、挂载 `EdgeKind` / `ChangeStepEdge` 再导出

## 阶段七：artifact 清单驱动发现（workflow/artifacts）

- [x] 修改 `crates/core/workflow/src/artifacts/envelope.rs`：`ArtifactDescriptor` 增 `phase: String`；`ArtifactCandidate` 注释口径同步（文件候选来自清单、eval 候选来自 PhaseRecord 投影）
- [x] 修改 `crates/core/workflow/src/artifacts/registry.rs`：`discover_artifacts(change_dir, phases, manifest)` 改「读 phase 清单 → 按 `ArtifactSpec.path` 相对路径枚举匹配（精确 / `**` 前缀 glob）→ 插件 parse → `phase` 随清单」；`enumerate_candidates` / `collect_files` 全树扫描退役；`read_artifact` 按 source 重建候选后仍经 `is_valid_source` / canonical 包含性校验；`ArtifactInput` 增清单归属上下文
- [x] 修改 `crates/core/workflow/src/artifacts/markdown_doc.rs`：`matches` 改「清单命中的 markdown-doc 候选项」而非「扩展名 == md」；title/rank 名单逻辑保留
- [x] 修改 `crates/core/workflow/src/artifacts/tasks_progress.rs`：`matches` 改「清单命中的 tasks.md 候选项」；勾选计数 parser 保留
- [x] 修改 `crates/core/workflow/src/artifacts/mod.rs`：再导出随 `ArtifactDescriptor.phase` / `discover_artifacts` 签名演进

## 阶段八：详情聚合 nodes / edges 出线（workflow/queries/detail）

- [x] 修改 `crates/core/workflow/src/queries/detail.rs`：`ChangeRunStepRecord` 退役、新增 `ChangeRunNodeRecord` / `ChangeRunEdgeRecord`；`ChangeRunEntry.steps` → `nodes` + `edges`；`change_detail` 读 `list_run_nodes` / `list_run_edges` 组装（节点按 node_seq 升序、边按 edge_seq 升序）；产物发现改传 manifest（`phase_table(record.workflow_type)` 取相位表后调用 `discover_artifacts` 新签名）

## 阶段九：命令面与通知桥

- [x] 修改 `src/commands/change_flow/mod.rs`：`change_flow_start` 组合根随 `ExecOutput` 记录器装配（注入 checks 执行器）；`ChangeFlowSink` 随 `RunUpdate::Edge` 透传；其余命令签名与语义不变
- [x] 修改 `src/commands/changes/mod.rs`：`ActiveRunView` 增 `edges`；`get_change_detail` 活面投影随 `ChangeRunSnapshot.edges` 出线；nodes / edges DTO 随 shape 演进

## 阶段十：前端随动

- [x] 修改 `src/views/changes/flow/types.ts`：`FlowEdgeKind` 增 `feedback`；`RuntimeFlowNode` 增 `instanceNo` / `outputRef`；`FlowEdge` 改读边表；`RunStepGroup` / `FlowRoleLabel` 词汇保持
- [x] 修改 `src/views/changes/flow/run-state.ts`：`unifiedRunSteps` 改读 `runs[].nodes`（`stepFromRecord` 增 `instanceNo` / `outputRef`）；`runStepNodes` 槽位配对 + `:seq` 堆叠退役、节点按后端 `(phase, attempt, step, instanceNo)` 直渲染（同键多实例以 `instanceNo` 区分不串扰）
- [x] 修改 `src/views/changes/flow/graph.ts`：`deriveEdges` Δ 派生退役、改读 `runs[].edges`（四 kind）；边端点按节点稳定身份键寻址；`buildFlowGraph` 边来自边表
- [x] 修改 `src/views/changes/flow/attachments.ts`：`docColumn` 静态映射退役、改读 `detail.artifacts[].phase` 清单归属（文档挂列）；eval-checklist 走 PhaseRecord 投影分支（`payload.phase + attempt` 挂节点）

## 阶段十一：绑定与版本交付

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `src/types/generated/bindings.ts`（nodes / edges / edgeKind / instanceNo / outputRef / ArtifactDescriptor.phase，非手改）
- [x] 修改 `packages/desktop/package.json`：`version` 0.4.34 → 0.4.35
- [x] 静态自查：`STATIC_CHECK_PHASES` / `TEST_EXECUTION_PHASES` / `persisted_step` / `RunStepRecord` / `enumerate_candidates` / `collect_files` / `deriveEdges` / `docColumn` / 槽位配对零残留引用；`plugins/dev-team` 零改动；`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份
