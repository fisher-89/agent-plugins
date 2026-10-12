# 提案: change-flow-show-step-only

> **变更**: change-flow-show-step-only
> **日期**: 2026-10-12
> **状态**: draft
> **探索**: `openspec/changes/change-flow-show-step-only/explore.md`（探索已收敛：仅 step 节点 + 抽屉单列 + stdout 展示）

---

## 问题

`packages/desktop` 的 change 详情流程图目前并存**三种节点**：eval（phase attempt，pass/fail 实心）、active（`activePhase`，运行中 pulse）、runtime（step 节点）。前两类相位节点与 step 节点信息重叠，且抽屉双列承载三块内容（本站文档 / eval report+checklist / 会话转录），静态检查与测试执行的**真实脚本输出不可回查**：

1. **相位节点冗余**：eval / active 节点承载的 attempt 信息已由 `runs[].nodes` 的 step 节点覆盖（running 态 runtime 节点已有 pulse 视觉、header Badge 已读 `detail.activePhase`），图上三分类并存既增加转换层 / 渲染 / 挂载优先级 / golden 全链维护成本，又未提供 step 节点之外的增量信息。
2. **抽屉无 stdout 回查**：static-check 默认 `ExecOutput::Transient`（全量即弃，`output_ref` 恒 None）、test-execution 的 `exec_id` 虽在案但落盘的是 summary JSON 重序列化而非脚本真实输出——用户点开 static-check / test-execution 节点看不到命令实际打出的 stdout/stderr。
3. **评估 report 与 checklist 展示位置失配**：eval 节点移除后，eval report 无承载（应不展示），eval-checklist 素材应上移列头（payload 自带 `phase`，列头抽屉经 `ArtifactTabs` 已可渲染全套元数据）。

explore 已把本轮改造收敛为五点（见下），削减流转类 step（phase_start / phase_log）明确留后续 change。

## 提案

变更详情流程图本轮改造，五点收敛：

1. **移除相位节点（eval + active）**：`buildFlowGraph` 删除 `collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs`，`sequence = [...runNodes]`（仅 runtime step 节点）。`pipeline` 参数保留（`tableEdges` 的 backtrack 边 label 仍读它）。运行中由 running 态 runtime 节点承载；header Badge 仍读 `detail.activePhase`（保留）。
2. **抽屉单列**：step 节点点击后单列展示 agent 会话转录（`sessionId` / `role`）或 stdout（`outputRef`）。抽屉删除 `DrawerEvalSection` + `InlineChecklist` 与 eval/active 分支；新增 `name` prop（`read_exec_output` 用 change name 非 id）。
3. **无点击判据**：runtime 节点 `role !== null` → 单列会话转录（复用 `SessionTranscriptPanel`）；`outputRef !== null` → 单列 stdout 面板（新组件）；两者皆 null → 不可点击（不 `onSelect`）。覆盖流转两词（phaseStart/phaseLog）+ gate 三门 + git 四步 + preflight/seal（均无 sessionId/outputRef，detail 短文本已在节点内显示，抽屉无增量信息）。
4. **static-check 启用 recorded**：`ProcessStaticCheck` 去 `record_anchor` 构造、`run(root, name)` 收 name；spawn 层 `2>&1` 把 stderr 并入 stdout 单一交错流；`diagnostics = output.stdout`（交错，更还原）、`record = output.stdout`（单一流）。`record_exec_output` 收敛为单一 `.log`（`.err` 退役），`read_exec_output` 读同一 `.log` 完整还原终端。
5. **test-execution record 改脚本执行输出**：`PlanExecution` 增 `output` 字段（runner 捕获的 stdout_bytes + stderr_bytes 合并流，**不加 `2>&1`**——rust 模板 `cargo test > results.txt` 已重定向 stdout，`2>&1` 会污染 `parse_cargo_test`）；`mod.rs` 聚合各 suite output 按 plan 序拼接落单一 `.log`，不再 serde summary JSON；全量结构化报告仍留 report_dir 子树。

前端过程素材随动：eval-checklist 素材改挂列头（`columnDocs`），`FlowMaterials.nodeChecklists` 退役；`mountMaterials` 去 `graph` 参数，连带 `useFlowGraphData` 的 materials memo 依赖收窄为 `[detail, artifacts]`。`flow-event-node.tsx` 整文件退役。

版本交付：`packages/desktop` 0.4.37 → 0.4.38（归档时执行）；`plugins/dev-team` 零改动。

## 关键决策

本轮五项收敛与两项边界决策均列明理由与被拒绝的备选方案：

| # | 决策 | 理由 | 被拒绝的备选方案 |
|---|------|------|----------------|
| D1 | 移除相位节点（eval + active），仅保留 runtime step 节点 | 相位节点与 step 节点信息重叠：attempt verdict 已由 step 状态（running / passed / failed / stopped）承载，运行中由 running 态 runtime 节点 pulse + header Badge（`detail.activePhase`）承载；减转换层 / 渲染 / 挂载优先级 / golden 全链维护成本 | ① 仅移除 eval、保留 active 节点承载运行中（否决：双分类仍在，active 与 running 态 step 重叠、交互入口两套）；② 三分类全保留、仅追加 stdout 回查（否决：未消除冗余，抽屉仍需三节） |
| D2 | 抽屉单列按信息源分派（`sessionId` / `role` → 会话转录，`outputRef` → stdout） | 单列更聚焦；eval report 随 eval 节点退役无承载（可经报告目录回查），eval-checklist 素材上移列头 | ① 保留双列并增第三列 stdout（否决：宽度与心智负担超载）；② eval report 继续在抽屉展示（否决：eval 节点退役后无承载，report 权威在报告树） |
| D3 | 无点击判据：`sessionId` 与 `outputRef` 皆 null 的节点不可点击（不 `onSelect`） | 流转两词（phaseStart / phaseLog）/ gate 三门 / git 四步 / preflight / seal 的 detail 短文本已在节点内显示，抽屉无增量信息 | ① 所有节点皆可点击、空抽屉占位（否决：空抽屉无信息量、误导交互）；② 为 ToolStep / Git / Gate 单独造 summary 面板（否决：无统一数据源，与 stdout / 转录面板重复） |
| D4 | static-check 启用 recorded + spawn 层 `2>&1` 交错合并 | 交错顺序在分离收集时即丢失、record 层无法还原；OS 层 `2>&1` 使两 fd 指向同一管道、写入顺序即真实顺序，单一 `.log` 落盘可完整回查终端 | ① 分离 `.log` / `.err` 双文件保留（否决：交错顺序失真、与终端不一致）；② record 层事后拼接 stdout+stderr（否决：顺序已丢失，非真实交错）；③ 继续 `Transient` 不回填 `output_ref`（否决：stdout 不可回查） |
| D5 | test-execution record 改脚本真实输出（stdout+stderr 合并，不加 `2>&1`） | summary JSON 重序列化非脚本真实输出，人工回查无价值；rust 模板 `cargo test > results.txt` 已重定向 stdout，`2>&1` 会把 stderr 混入污染 `parse_cargo_test` | ① 保留 summary JSON 重序列化（否决：非真实输出、回查失真）；② test-execution 同加 `2>&1`（否决：污染 rust `parse_cargo_test`）；③ 强行统一各框架脚本输出格式（否决：各框架输出形态天然不同，零统一格式负担更稳） |
| D6 | eval-checklist 素材挂列头、eval report 不展示 | payload 自带 `phase` + `attempt` + `verdict` + `items`，`artifactColumn` 已有 pipeline 外相位校验、`EvalChecklistRenderer` 已能渲染全套元数据，列头经 `ArtifactTabs` 即达（零成本） | ① eval-checklist 继续挂节点（否决：节点模型已单分类、无 eval 节点承载）；② eval report 继续上图（否决：eval 节点退役无承载，report 权威在报告目录） |
| D7 | 削减流转类 step（phase_start / phase_log）留后续 change | 涉及后端 walker 源头不 emit（×3 变体去 begin_node + emit_node）+ `entry_kind` 迁移（executor / preflight 消费）+ 前端 `run-state.ts` 过滤兜底，blast radius 大且边链正确性需 walker 层 tail 不动 | ① 本轮一并削减（否决：扩大 blast radius，混淆本轮五点收敛）；② 仅前端过滤不改后端（否决：落账 / 开相副作用与边链正确性未处理，留旧数据隐患） |

## 能力

### 新增能力

- 无（本变更修改四个既有能力）。

### 修改的能力

- **desktop-change-flow-view** — 节点模型两分类（eval / active）+ runtime overlay 收敛为**单一 runtime step 节点分类**；抽屉双列改单列并按 sessionId / outputRef 分派；新增 stdout 输出面板与 `use-exec-output` 取数；eval-checklist 素材挂列头、`nodeChecklists` 退役、`flow-event-node.tsx` 退役；运行中由 running 态 runtime 节点承载（header Badge 保留读 `activePhase`）。
- **desktop-checks-domain** — static-check spawn 改 `2>&1` 交错合并（stderr 并入 stdout 单一流）；static-check 默认启用 `ExecOutput::Recorded`（`output_ref` 回填）；`StaticCheckRunner::run` port 契约改 `(root, name)`；`record_exec_output` 收敛单一 `.log`（`.err` 退役）、`read_exec_output` 读同一 `.log`。
- **desktop-test-execution** — test-execution 输出记录由 summary JSON 重序列化改为脚本真实输出（stdout+stderr 合并流，按 plan 序聚合落单一 `.log`）；`PlanExecution` 增 `output` 字段。
- **desktop-change-orchestration** — `steps.rs` 的 StaticCheck 分支解析 name（id → 记录 → name，与 TestExecution 分支同式）；`change_flow` 组合根 `ProcessStaticCheck` 构造器简化；版本交付 0.4.37 → 0.4.38。

### 引用沿用（零 delta）

- **desktop-ipc-type-bindings** — 新增 `read_exec_output` 命令经 tauri-specta 再生与一致性守卫既有纪律承载，机制零改动（返回 `Option<String>`，无新 DTO）。
- **desktop-change-queries** — `runs[].nodes[].outputRef` 线面既有，零 delta。
- **desktop-change-state-store** — `RunNodeRecord.output_ref` 落库引用既有，零 delta。
- **desktop-corpus-regression** — 无 detail DTO 形状变更（`output_ref` 仍为 exec_id 字符串），golden 零 delta。

---

## 变更范围

### 实现文件

- `packages/desktop/src/views/changes/flow/graph.ts` — 删 `collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs`；`sequence = [...runNodes]`；`pipeline` 参数保留（`tableEdges` backtrack label）
- `packages/desktop/src/views/changes/flow/types.ts` — 删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode`；`FlowNode = RuntimeFlowNode`；`FlowMaterials` 删 `nodeChecklists`
- `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` — 删 event 通道（`nodeTypes.event` / `toChartNodes` 非 runtime 分支 / `onNodeClick` 'event' 判）；`onNodeClick` 加可点击判据
- `packages/desktop/src/views/changes/flow/attachments.ts` — 全量改挂 `columnDocs`（eval-checklist 挂列头）；删 `NODE_PRECEDENCE` / `locateNode` / `nodeChecklists`；`mountMaterials` 去 `graph` 参数
- `packages/desktop/src/views/changes/flow/detail-drawer.tsx` — 单列重构：删 `DrawerEvalSection` + `InlineChecklist`；按 sessionId/outputRef 分派；删 eval/active 分支；新增 `name` prop（read_exec_output 用 change name 非 id）
- `packages/desktop/src/views/changes/flow/stdout-panel.tsx` + `hooks/use-exec-output.ts` — 新增：`outputRef` → `read_exec_output` 渲染
- `packages/desktop/src/views/changes/change-detail-view.tsx` — `useFlowGraphData` 的 materials memo 依赖收窄 `[detail, artifacts]`；`DetailDrawer` 传 `name`
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — `StaticCheckRunner::run(root)` → `run(root, name)`（对齐 `TestExecutionRunner`）
- `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` — StaticCheck 分支解析 name（抄 TestExecution 分支 `get_change(id).map(name)`）
- `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` — `ProcessStaticCheck` 去 `record_anchor` 构造；`execute` 加 name；spawn `2>&1`；record = 交错 `.log`；diagnostics = 交错 stdout
- `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` — `record_exec_output` 收敛单一 `.log`（`.err` 退役）；`read_exec_output` 读同一 `.log`
- `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` — `PlanExecution` 增 `output` 字段（stdout+stderr 合并流，仅进 record，不进 error 尾部）
- `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` — 聚合各 suite `output` 按 plan 序拼接落单一 `.log`；record 改脚本输出（去 serde summary JSON）
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — 新增 `read_exec_output(root, change_name, exec_id) -> Option<String>`（UTF-8 lossy；miss=None）
- `packages/desktop/src-tauri/src/commands/mod.rs` — `all_commands!` 注册 `read_exec_output`
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — `ProcessStaticCheck` 构造器简化（recorded 默认化后 `new()` 装配与注释随动）
- `packages/desktop/src/types/generated/bindings.ts` — 随 `read_exec_output` 命令再生
- `packages/desktop/package.json` — `version` 0.4.37 → 0.4.38（归档时执行）

### 测试文件

- `packages/desktop/src/views/changes/flow/flow-event-node.test.tsx` — **退役删除**
- `packages/desktop/src/views/changes/flow/graph.test.ts` — 仅 step 节点、无 eval/active 归并
- `packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx` — 可点击判据（无信息源节点不 `onSelect`）
- `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` — 单列分派（sessionId → 转录 / outputRef → stdout / 列头 → 文档）
- `packages/desktop/src/views/changes/flow/attachments.test.ts` — eval-checklist 挂列头、nodeChecklists 退役
- `packages/desktop/src/views/changes/change-detail-view.test.tsx` — materials memo 依赖与 `name` 透传
- `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx` + `hooks/use-exec-output.test.ts` — 新增 stdout 面板与取数
- `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs` — `2>&1` 交错 + recorded 回填 + name 落盘
- `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs` — `PlanExecution.output` 合并流
- `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs` — record = 脚本输出（去 summary JSON 断言）
- `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs` — `record_exec_output` 单一 `.log`
- `packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs` — StaticCheck name 解析
- `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` — `ProcessStaticCheck` 构造器简化随动
- `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` — `read_exec_output` 命令测试（UTF-8 lossy / miss=None）

### 删除文件

- `packages/desktop/src/views/changes/flow/flow-event-node.tsx`（+ `flow-event-node.test.tsx`）

### 不要修改

- **削减流转类 step**（phase_start / phase_log）——后续 change：后端 walker 源头不 emit（`step_phase_start` / `step_phase_log` ×3 变体去 begin_node+emit_node，保留落账/开相副作用）+ `entry_kind` 迁移（executor/preflight 消费）+ 前端 `run-state.ts` 过滤兜底（已落库旧数据）。本轮不做。
- **词汇枚举** `RunStepKind` / `StepNode` / `ChangeStepKind` 保留（历史数据兼容 + 前端 bindings 不变）。
- `detail.activePhase` 出线与 header Badge 保留（仅移除图上 active 节点，不删读模型字段）。
- `PIPELINE_PHASES` 10 列布局、边表读取规则与四 kind、attempt 缺号兜底（不变）。
- 会话转录联动机制（sessionId 直查优先 + sourceRef 反查兜底、`SessionTranscriptPanel` 复用）不变。
- `plugins/dev-team` 全部（零改动）。
- `desktop-change-queries` / `desktop-change-state-store` 的 `output_ref` 线面与落库（零 delta）。

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 流程图仅 step 节点 | 打开任一 change 详情：图上仅 runtime step 节点（running 态 pulse、终态样式），无 eval 节点、无 active 节点；header Badge 仍读 `detail.activePhase` 呈现「运行中 · phase · attempt」；`flow-event-node.tsx` 已删除、`nodeTypes.event` / `collectEvals` / `collectActive` / `mergeByStartAt` 零残留 |
| AC-2 | 抽屉单列分派 | 点击 WorkerAgent step 节点 → 单列会话转录；点击带 `outputRef` 的 step 节点 → 单列 stdout 面板；点击列头 → 单列该站文档（含 eval-checklist 素材）；抽屉无 eval/active 分支、无「评估记录」节、无 `DrawerEvalSection` / `InlineChecklist` |
| AC-3 | 无点击判据 | 点击 phaseStart / phaseLog / verdictGate / retryGate / whitelistGate / commit / rebase / merge / finalize / preflight / seal 节点（sessionId 与 outputRef 皆 null）→ 不触发 `onSelect`、抽屉不打开；有 role 或有 outputRef 的节点照常打开抽屉 |
| AC-4 | 素材挂列头 | eval-checklist 信封挂对应 phase 列头（payload `phase` 归属），`columnDocs` 含文档 + checklist；`FlowMaterials.nodeChecklists` 退役；`mountMaterials` 无 `graph` 参数、materials memo 依赖收窄 `[detail, artifacts]` |
| AC-5 | stdout 面板与 IPC | `use-exec-output` 经 `commands.readExecOutput(root, changeName, execId)` 取数；stdout 面板渲染 UTF-8 lossy 文本、miss 呈空态；`read_exec_output` 命令注册进 `all_commands!` 并出线 bindings（`bindings:check` 绿） |
| AC-6 | static-check recorded 与 2>&1 交错 | static-check spawn 经 `{command} 2>&1` 单一流，`diagnostics` 为交错 stdout（stderr 空）；`output_ref` 恒回填（非 None）；`record_exec_output` 仅写 `<exec_id>.log`（`.err` 退役）；`read_exec_output` 读回同一 `.log` |
| AC-7 | test-execution record 脚本输出 | `PlanExecution.output` = stdout+stderr 合并流（不加 2>&1）；record 内容为各 suite 脚本输出按 plan 序拼接，非 summary JSON；`read_exec_output` 回查内容与脚本真实输出一致，全量结构化报告仍留 report_dir 子树 |
| AC-8 | 全管线通过 | `pnpm -C packages/desktop run client:check`（fmt / lint / knip）与 `pnpm -C packages/desktop run test` 全绿；`cargo test --workspace`（src-tauri 根）全绿；无新增豁免条目 |
| AC-9 | desktop 版本升级（归档时执行） | `packages/desktop/package.json` `version` 0.4.37 → 0.4.38；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 零改动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 相位节点信息（pass/fail 相位评估）从图上移除 | 用户失去相位 attempt 的 verdict 一屏总览，只余 step 节点 | 确定（有意收敛） | 探索已拍板；step 节点 status（running / passed / failed / stopped）仍承载步级状态，phase verdict 历史可从库读史 / 报告回查；如需相位面后续变更可反向加回 |
| R2 2>&1 交错合并改变 diagnostics 顺序语义 | 依赖「stdout 在前 stderr 在后」的既有断言 / 消费方漂移 | 中 | 交错更还原终端真实顺序；`StaticCheckOutcome.diagnostics` 形态不变（仍 String），仅拼接顺序改变；static_check_test 显式断言交错序 |
| R3 `record_exec_output` 签名收敛（stdout+stderr → 单一流）牵动两调用点 | 误留 `.err` 写入或漏改 test-execution / static-check 调用 | 低 | infra/exec 单点收敛 + lib_test 覆盖单一 `.log`；两调用点（static_check / testexec mod）同改同测 |
| R4 rust 模板不 2>&1，record 主要为 stderr（编译错误面） | rust 套件的脚本 stdout 被重定向进 results.txt，record 不含主输出 | 中（框架形态固有） | 探索已接受；results.txt 在报告树可见，record 不强行 tee；`findings_detail` / 报告目录仍为全量权威 |
| R5 无信息源节点不可点击 | ToolStep / Gate / Git 节点的 detail 短文本已显示在节点内，点击无抽屉 | 确定（有意收敛） | 探索已拍板（流转两词 + gate 三门 + git 四步 + preflight/seal 均无增量信息源）；列头仍可点开文档 |
| R6 `name` 从页面传入抽屉（read_exec_output 用 change name 非 id） | 身份段 id / name 混用引入寻址漂移 | 低 | `DetailDrawer` 新增 `name` prop 取 `detail.name`（展示面已用 name）；`read_exec_output` 磁盘面按 name 定位（与 `record_exec_output` 同口径），id 仅作寻址键 |
| R7 削减流转类 step 的边界被误并入本轮 | phase_start/phase_log 源头不 emit 属后续 change，若被本轮顺带做会扩大 blast radius | 低 | 提案「不要修改」显式列明后续 change 边界；验收仅覆盖本轮五点，流转 step 仍照常 emit 与上图 |
