# 测试设计: change-flow-show-step-only

> **变更**: change-flow-show-step-only
> **角色**: test-design-planner（executor）
> **输入**: proposal.md / design.md（本轮无既有 test-design.md，属首产）
> **范围**: 仅设计单元测试与公共 API 签名，不写测试命令细节；静态脚本（static-check / test-execution）由桌面端执行。

## 测试框架与测试区域识别

- **前端 `packages/desktop/src/`**：Vitest（`vite-plus/test` 提供 `describe` / `it` / `expect` / `vi`），React 组件用 `@testing-library/react` + jsdom。测试文件与源文件同目录同前缀，扩展名 `.test.ts`（纯函数 / hook）与 `.test.tsx`（React 组件）；断言风格 `expect(...).toMatchObject / toEqual / toHaveLength`，进程边界 invoke 经 `vi.hoisted` + `vi.mock('@tauri-apps/api/core')`。
- **后端 `packages/desktop/src-tauri/`**：Rust `cargo test --workspace`，测试为同目录 `*_test.rs`（经 `#[cfg(test)] mod xxx_test;` 内联挂载）。断言 `assert_eq!` / `assert!`，临时 workspace 用 `tempfile::TempDir` 或手写 RAII，进程边界（spawn / PATH / 环境变量）经 crate 级 `TEST_ENV_LOCK` / `TEST_PATH_LOCK` 串行化。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 流程图仅 runtime step 节点（running 态 pulse、终态样式），无 eval / active 节点；header Badge 仍读 `detail.activePhase`；`flow-event-node.tsx` 删除、`nodeTypes.event` / `collectEvals` / `collectActive` / `mergeByStartAt` 零残留 | `packages/desktop/src/views/changes/flow/graph.ts`、`packages/desktop/src/views/changes/flow/types.ts`、`packages/desktop/src/views/changes/flow/change-flow-graph.tsx` |
| AC-2 | 抽屉单列分派：WorkerAgent（role）→ 会话转录、outputRef → stdout 面板、列头 → 该站文档 + eval-checklist；无 eval/active 分支、无「评估记录」节、无 `DrawerEvalSection` / `InlineChecklist` | `packages/desktop/src/views/changes/flow/detail-drawer.tsx`、`packages/desktop/src/views/changes/flow/stdout-panel.tsx`、`packages/desktop/src/views/changes/hooks/use-exec-output.ts` |
| AC-3 | 无点击判据：sessionId 与 outputRef 皆 null 的节点不触发 `onSelect`、抽屉不打开；有 role 或有 outputRef 的节点照常打开 | `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` |
| AC-4 | eval-checklist 信封挂对应 phase 列头（`columnDocs` 含文档 + checklist）；`FlowMaterials.nodeChecklists` 退役；`mountMaterials` 无 `graph` 参数、materials memo 依赖收窄 `[detail, artifacts]` | `packages/desktop/src/views/changes/flow/attachments.ts`、`packages/desktop/src/views/changes/flow/types.ts`、`packages/desktop/src/views/changes/change-detail-view.tsx` |
| AC-5 | `use-exec-output` 经 `commands.readExecOutput(root, changeName, execId)` 取数；stdout 面板渲染 UTF-8 lossy、miss 呈空态；`read_exec_output` 命令注册进 `all_commands!` 并出线 bindings | `packages/desktop/src/views/changes/hooks/use-exec-output.ts`、`packages/desktop/src/views/changes/flow/stdout-panel.tsx`、`packages/desktop/src-tauri/src/commands/changes/mod.rs`、`packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src/types/generated/bindings.ts` |
| AC-6 | static-check spawn 经 `{command} 2>&1` 单一流、`diagnostics` 为交错 stdout；`output_ref` 恒回填；`record_exec_output` 仅写 `<exec_id>.log`（`.err` 退役）；`read_exec_output` 读回同一 `.log` | `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs`、`packages/desktop/src-tauri/crates/infra/exec/src/lib.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` |
| AC-7 | `PlanExecution.output` = stdout+stderr 合并流（不加 `2>&1`）；record 内容为各 suite 脚本输出按 plan 序拼接、非 summary JSON；全量结构化报告仍留 report_dir 子树 | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs`、`packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` |
| AC-8 | 全管线通过（client:check / test / cargo test --workspace），无新增豁免条目 | 全部被测文件（AC-1–AC-7 覆盖，桌面端执行门禁） |
| AC-9 | `packages/desktop/package.json` `version` 0.4.37 → 0.4.38；`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 零改动 | `packages/desktop/package.json` |

---

## 单元测试

> 说明：`路径类型` 取值——`正向` / `异常` / `边界` / `组合`（跨模块链路用例，按最上层调用方挂靠到入口模块章节）/ `幂等`（写路径重复调用与无副作用）；`迭代类型` 取值——`新增`（本轮新增测试文件/用例）、`修改`（既有测试文件随行为变化改写）、`保留`（既有用例不变）。每份 `#### 用例` 表对每个 `#### 待测功能` 条目至少覆盖正向 / 异常 / 边界三类，写路径另加幂等用例。

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能

- `buildFlowGraph(detail, runNodes?, runEdges?): FlowGraph` — 仅 runtime step 节点：删除 eval/active 归并，`sequence = [...runNodes]`；`pipeline` 参数保留供 `tableEdges` backtrack label。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `buildFlowGraph` | 正向 | `detail.pipeline` 含多 attempt 且 `detail.activePhase` 非空、`runNodes` 缺省 → `nodes` 为空（eval / active 不再上图），`columns` 恒 10、`edges` 空 | 修改 |
| `buildFlowGraph` | 正向 | 传入 `runNodes` → `nodes` 恰为该 `runNodes` 原序（`sequence = [...runNodes]`），`assignOrders` 按 `phase` 重编 `order`，节点 `kind` 恒 `runtime` | 修改 |
| `buildFlowGraph` | 正向 | `runNodes` 两节点 + `runEdges` 含 backtrack 边 + `pipeline` 携 `backtrackReason` → 边 `label` 取发起相位记因（`pipeline` 参数保留生效） | 修改 |
| `buildFlowGraph` | 正向 | 未供 `runEdges` → 零边；`runEdges` 端点悬空（ghost 身份串）→ 该边跳过不炸图 | 修改 |
| `buildFlowGraph` | 异常 | `runEdges` 端点悬空 / `runNodes` 为空数组或 `undefined` → 不抛错，`edges` / `nodes` 收口为空或跳过悬空边 | 修改 |
| `buildFlowGraph` | 边界 | `detail.pipeline` 为空数组（v0 形态）→ 返回空图 `{ columns: [], nodes: [], edges: [] }` | 保留 |
| `buildFlowGraph` | 边界 | `runNodes` 含同键多实例（`instanceNo` 递增）→ 节点身份键唯一不串扰；超长列表（全史叠加多 attempt）→ 顺序保持、不去重丢失 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 不 mock：真实组合 `layout`（`PIPELINE_PHASES`）与 `run-state`（`nodeIdentityKey`），纯函数直测 | 全部用例 |

---

### packages/desktop/src/views/changes/flow/change-flow-graph.tsx -> packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx

#### 待测功能

- `ChangeFlowGraph(props: ChangeFlowGraphProps): React.JSX.Element` — 删 event 通道（`nodeTypes.event` / 非 runtime 分支 / `onNodeClick` 'event' 判），`onNodeClick` 加 `role === null && outputRef === null` 不可点击判据。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ChangeFlowGraph` | 正向 | 含 `RuntimeFlowNode` 的图 → 渲染 `runStep` 组件可辨，10 列 + 节点计数与 `graph` 一致；整树无 `flow-event-node` 残留（`queryByTestId` 为空） | 修改 |
| `ChangeFlowGraph` | 正向 | 点击 `role !== null` 的 runtime 节点 → `onSelect` 收到 `{ scope: 'node', nodeId }` | 修改 |
| `ChangeFlowGraph` | 正向 | 点击 `outputRef !== null` 的 runtime 节点（static-check / test-execution）→ `onSelect` 照常上抛 | 新增 |
| `ChangeFlowGraph` | 正向 | 点击列头 → `onSelect` 收到 `{ scope: 'column', phase }`（列头不受判据约束） | 保留 |
| `ChangeFlowGraph` | 异常 | 点击 `role === null && outputRef === null` 节点（phaseStart / phaseLog / gate 三门 / git 四步 / preflight / seal）→ 不触发 `onSelect`（调用数不增长） | 新增 |
| `ChangeFlowGraph` | 边界 | 空 `graph`（columns / nodes / edges 为空）→ 渲染空画布不抛错；`graph` 变化重渲染 → 新节点集合无残留 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| jsdom 环境垫片 | `ResizeObserver` / `offsetWidth` / `offsetHeight` / `DOMMatrixReadOnly` / `getBBox` 垫片（环境垫片非业务 mock） | 全部渲染用例 |
| 内部模块 | 不 mock：真实组合 `buildFlowGraph` / `mountMaterials` / `RunStepNode` / `PhaseColumnNode` | 全部用例 |

---

### packages/desktop/src/views/changes/flow/attachments.ts -> packages/desktop/src/views/changes/flow/attachments.test.ts

#### 待测功能

- `mountMaterials(detail, envelopes): FlowMaterials` — 去 `graph` 参数；文档 + eval-checklist 均挂 `columnDocs`；`nodeChecklists` 退役。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `mountMaterials` | 正向 | `markdown-doc` / `tasks-progress` 信封按 `detail.artifacts[].phase` 清单归属挂 `columnDocs`（proposal / dev-design / test-execution） | 修改 |
| `mountMaterials` | 正向 | `eval-checklist` 信封 payload 自带 `phase` → 挂对应 phase 列头 `columnDocs`（`artifactColumn`），不再挂节点 | 修改 |
| `mountMaterials` | 正向 | 同列多文档 + 多 checklist → `columnDocs` 数组聚合保序；产物键集仅 `columnDocs`（无 `nodeChecklists`） | 修改 |
| `mountMaterials` | 异常 | `eval-checklist` payload 形状不合规（null / 缺 `phase` / `phase` 类型不符）→ 不挂载 | 保留 |
| `mountMaterials` | 异常 | 清单未声明文档 / `explore.md` / pipeline 外相位 → 不误挂任何列 | 保留 |
| `mountMaterials` | 边界 | `envelopes = []` → `FlowMaterials` 仅 `columnDocs: {}`，不抛错 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 不 mock：纯函数直测，真实组合 `layout`（`PIPELINE_PHASES`）与 `types` | 全部用例 |

---

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

- `DetailDrawer({ selection, graph, materials, root, changeId, name, transcriptRefreshKey, onClose }): React.JSX.Element | null` — 单列分派：列头 → `ArtifactTabs`；runtime `role !== null` → `SessionTranscriptPanel`；`outputRef !== null` → `StdoutPanel`；删 `DrawerEvalSection` / `InlineChecklist` / eval-active 分支；新增 `name` prop。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `DetailDrawer` | 正向 | `selection.scope === 'column'` → 单列 `ArtifactTabs`（该站文档 + eval-checklist），无会话转录区 | 修改 |
| `DetailDrawer` | 正向 | runtime 节点 `role !== null` → 单列 `SessionTranscriptPanel`（`selectionRoleRefs` 只处理 runtime），不发起 `read_exec_output` | 修改 |
| `DetailDrawer` | 正向 | runtime 节点 `outputRef !== null`（且 `role === null`）→ 单列 `StdoutPanel`，收到 `{ root, changeName: name, execId: outputRef }` | 新增 |
| `DetailDrawer` | 正向 | `name` prop 透传：断言 `StdoutPanel` 收到 `name`（`detail.name`，非 `changeId`） | 新增 |
| `DetailDrawer` | 异常 | `selection === null` → 返回 `null` 不渲染；悬空 `nodeId` → 标题中性占位不崩 | 保留 |
| `DetailDrawer` | 边界 | 双列结构退役：整树无 `drawer-docs-section` / `drawer-eval-section` / `InlineChecklist` / `DrawerEvalSection` 残留（`queryByTestId` 为空） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发：`agent_sessions` / `agent_session_transcript` / `session_detail` / `read_exec_output`，可切换 resolve / reject | 会话转录与 stdout 取数用例 |
| 内部模块 | 不 mock：`SessionTranscriptPanel` / `ArtifactTabs` / `StdoutPanel` 真实组合 | 全部用例 |

---

### packages/desktop/src/views/changes/flow/stdout-panel.tsx -> packages/desktop/src/views/changes/flow/stdout-panel.test.tsx

#### 待测功能

- `StdoutPanel(props: { root: string | null; changeName: string; execId: string }): React.JSX.Element` — 单列 `<pre>` 渲染脚本输出；miss 空态 / 错误态不炸图。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `StdoutPanel` | 正向 | `readExecOutput` 命中 → 单列 `<pre>` 渲染 UTF-8 lossy 文本（含 emoji / 特殊字符替换符） | 新增 |
| `StdoutPanel` | 正向 | `text === null`（miss）→ 空态提示 | 新增 |
| `StdoutPanel` | 异常 | invoke 拒绝 → 错误态提示，不炸图 | 新增 |
| `StdoutPanel` | 异常 | `root === null` → 空态（零 invoke，不崩溃） | 新增 |
| `StdoutPanel` | 边界 | 超长输出（>1000 chars）→ `<pre>` 完整渲染不截断；空串输出 → 渲染合法（空 `<pre>` 或空态，按 hook 契约） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 控制 `read_exec_output` 返回 `Some(文本)` / `None` / reject | 空态 / 错误态 / 命中用例 |
| 内部模块 | 不 mock：真实组合 `useExecOutput`（内部模块不 mock） | 全部用例 |

---

### packages/desktop/src/views/changes/hooks/use-exec-output.ts -> packages/desktop/src/views/changes/hooks/use-exec-output.test.ts

#### 待测功能

- `useExecOutput(params: { root: string | null; changeName: string | null; execId: string | null }): { text: string | null; loading: boolean; error: string | null }` — 经 `commands.readExecOutput(root, changeName, execId)` 取数；miss 空态、invoke 拒绝 → error。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `useExecOutput` | 正向 | 三参齐全 → invoke `read_exec_output { root, changeName, execId }` → `text` = 返回串、`loading` false、`error` null | 新增 |
| `useExecOutput` | 正向 | 返回 `None`（miss）→ `text === null`、`loading` false、`error` null（空态语义） | 新增 |
| `useExecOutput` | 正向 | `execId` 变化（rerender）→ 重查新 `execId` | 新增 |
| `useExecOutput` | 异常 | invoke 拒绝 → `error` 字符串、`text === null`（错误态，不炸） | 新增 |
| `useExecOutput` | 异常 | 竞态守卫：切走 `execId` 后旧查询后到不回写 `text` / `error`（disposed 清理生效） | 新增 |
| `useExecOutput` | 边界 | 任一入参 `null`（root / changeName / execId）→ 清空态零 invoke；空串 `changeName` / `execId`（非 null）→ 按非空参数照常发起 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 控制 `read_exec_output` resolve / reject，记录入参 | 全部用例 |
| 内部模块 | 不 mock：真实组合 `types/generated/bindings` 的 `commands.readExecOutput` 调用路径 | 全部用例 |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

- `ChangeDetailView({ root }): React.JSX.Element` — 数据源收窄与 name 透传：`useFlowGraphData` 的 materials memo 依赖收窄 `[detail, artifacts]`（去 `graph`）、`mountMaterials(detail, artifacts)` 随签名；`DetailDrawer` 传 `name={detail.name}`（`read_exec_output` 磁盘面按 change name 定位，非 id）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ChangeDetailView` | 正向 | materials memo 依赖收窄：`detail` / `artifacts` 不变而 `graph`（`runNodes` / `runEdges` / `activeRun`）变化 → `mountMaterials` 不重算（`vi.spyOn` 观测调用次数不增、materials 引用稳定） | 修改 |
| `ChangeDetailView` | 正向 | `mountMaterials(detail, artifacts)` 双参调用（无 `graph`）→ 文档归列与 eval-checklist 挂列头照常（真实 `mountMaterials` 组合） | 修改 |
| `ChangeDetailView` | 正向 | `id ≠ name` 建档详情渲染 → `DetailDrawer` 收到 `name === detail.name`（非 URL id），stdout 面板取数经 `readExecOutput(root, changeName=name, execId=outputRef)` | 修改 |
| `ChangeDetailView` | 异常 | `detail.name` 与 URL id 相异且选中节点携 `outputRef` → stdout 面板仍按 `detail.name` 取数，不因 id 寻址漂移（miss 空态不炸） | 新增 |
| `ChangeDetailView` | 边界 | `artifacts = []` / `runs = []`（零图零产物）→ materials 空 `columnDocs`、抽屉 `name` 仍透传，不抛错 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发：`get_change_detail` / `read_artifact` / `read_exec_output`（可切 resolve / reject） | 取数与 stdout 面板用例 |
| 内部模块 | 不 mock：真实组合 `useFlowGraphData` / `mountMaterials` / `buildFlowGraph` / `ChangeFlowGraph` / `DetailDrawer`（`mountMaterials` 可 `vi.spyOn` 观测调用次数，不替换实现） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- `StaticCheckRunner::run(root, name)`（trait）— 收 `name` 供 infra 侧 output 落盘，对齐 `TestExecutionRunner::run(root, name)`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `StaticCheckRunner.run` | 正向 | 假实现记录 `(root, name)` 两参 → 断言两参签名（`root` + `name`）与 `TestExecutionRunner` 同型 | 修改 |
| `StaticCheckRunner.run` | 正向 | 组合（链路方向：steps.rs StaticCheck 分支 → `StaticCheckRunner::run(root, name)`）：已建档 change → `store.get_change(id)` 解析 `name`，fake runner 收到该 `name`（非 id） | 组合 |
| `StaticCheckRunner.run` | 异常 | 组合：change 未建档（无 `ChangeRecord`）→ steps.rs 显式 `Err`，不调用 `run` | 组合 |
| `StaticCheckRunner.run` | 边界 | 组合：`id ≠ name` 形态 → fake runner 收到的 `name` 与命令载荷 `change_id` 相异，逐字等于记录 `name` | 组合 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `StaticCheckRunner` | 假实现（作为被测 API 显式入参 / trait object 注入），记录入参 + 可编程产出 | 签名锚定与 name 透传用例 |
| `ChangeStateStore` | 组合用例注入 FakeStore / 真实 tempfile store（进程边界 DB，可 mock 或真实组合） | name 解析 / 未建档 Err 用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

- `LocalToolSteps.run(ToolStepRequest)` 的 `ToolCommand::StaticCheck` 分支 — id → 记录 → name 解析（`store.get_change(id).map(|record| record.name)`，抄 TestExecution 分支）；未建档显式 `Err`；`static_check.run(&step.root, &name)` 传 name。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `LocalToolSteps.run`（StaticCheck 分支） | 正向 | 已建档（`id ≠ name`）→ 解析记录 `name` 并调 `static_check.run(root, name)`，fake runner 收到 `name`（非 id）；审计行落库 `StepKind::StaticCheck`（passed / failed 均落） | 修改 |
| `LocalToolSteps.run`（StaticCheck 分支） | 正向 | passed 与 failed 两分支产出均携 `output_ref`（`StaticCheckOutcome` 语义不变，recorded 恒回填） | 修改 |
| `LocalToolSteps.run`（StaticCheck 分支） | 异常 | change 未建档（无 `ChangeRecord`）→ 显式 `Err`（携 change id / 未建档文案），零 runner 调用（不 spawn） | 新增 |
| `LocalToolSteps.run`（StaticCheck 分支） | 异常 | `store.get_change` 读故障 → `Err` 传播不静默（审计 error 行落库，失败不阻断审计） | 修改 |
| `LocalToolSteps.run`（StaticCheck 分支） | 边界 | `id == name` 形态 → 解析 name 逐字等于 id（零漂移）；重复调用 → 每次独立解析、runner 调用计数递增（无副作用） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `StaticCheckRunner` | 假实现（被测 API 注入依赖）：记录 `(root, name)` 调用 + 可编程产出 | name 透传与零发起用例 |
| `TestExecutionRunner` | 假实现（同缝，非本分支观察面） | 组合根装配 |
| `ChangeStateStore` | `FakeStore` / 真实 tempfile store（进程边界 DB） | name 解析 / 未建档 Err / 读故障用例 |
| 内部模块 | 不 mock：`audit_outcome` / `clip_summary` 真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

#### 待测功能

- `ProcessStaticCheck.new()` — 去 `record_anchor` 字段与 `recorded()` 方法；默认即 recorded；`run(root, name)` 经 spawn `{command} 2>&1` 交错合并、`diagnostics` = 交错 stdout、`output_ref` 回填单一 `.log`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `ProcessStaticCheck.new` | 正向 | `new()` 零字段单位语义 + `run(root, name)` 默认 recorded → `output_ref` 回填 `Some(exec_id)`，`read_exec_output` 读回内容 = 交错 stdout | 修改 |
| `ProcessStaticCheck.new` | 正向 | spawn `{command} 2>&1`：命令向 stdout / stderr 交错写 → `diagnostics` 逐字等于真实交错序（非「stdout 全量 + stderr 全量」），`output.stderr` 空 | 新增 |
| `ProcessStaticCheck.new` | 正向 | `run(root, name)` 收 `name` 落盘 `<report_dir>/exec-outputs/<exec_id>.log`，无 `<exec_id>.err` 文件 | 新增 |
| `ProcessStaticCheck.new` | 异常 | 无配置 / 空命令 → passed 直过、零 spawn、`output_ref` None | 保留 |
| `ProcessStaticCheck.new` | 异常 | 命令不可达 → 显式 `Err`（spawn 前程序解析前置） | 保留 |
| `ProcessStaticCheck.new` | 异常 | 落盘失败（报告树路径被文件占位）→ `output_ref` 降级 None，不阻断检查主流程 | 修改 |
| `ProcessStaticCheck.new` | 边界 | 命令仅向 stderr 写 → 2>&1 后 `diagnostics` 含该文本（`stderr` 空）；空输出命令 → `output_ref` 落空 `.log` 可回查 | 新增 |
| `ProcessStaticCheck.new` | 幂等 | 重复 `run(root, name)` → 每次新铸互异 `exec_id`、仅 `<exec_id>.log` 落盘（无 `.err` 残留），前次 `.log` 不被覆写；重复回读各自 `exec_id` 内容一致（无副作用） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 进程 spawn（进程边界） | 不 mock：真实平台 shell 命令（临时 workspace + `cmd /C` / `sh -c`），`2>&1` 交错序为被测行为，必须真实 spawn | 交错 / recorded / 空输出用例 |
| `exec`（内部模块） | 不 mock：`record_exec_output` / `read_exec_output` 真实落盘回查（临时目录） | recorded 落盘 / 回查用例 |

---

### packages/desktop/src-tauri/crates/infra/exec/src/lib.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs

#### 待测功能

- `record_exec_output(root, change, output): Result<String, String>` — 收敛单一 `.log`（`.err` 退役），`output` 为单一交错 / 合并流。
- `read_exec_output(root, change, exec_id): Option<Vec<u8>>` — 读同一 `.log`（实现不变，注释随动）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `record_exec_output` | 正向 | 单一 `output` 落盘 → 仅 `<exec_id>.log`（无 `<exec_id>.err`），返回 `exec_id`；`read_exec_output` 回读逐字节一致 | 修改 |
| `record_exec_output` | 正向 | 两次落盘 `exec_id` 互异；不同 `change` 名目录隔离（跨 change 回查 → None） | 保留 |
| `read_exec_output` | 正向 | 读同一 `.log`（已知 `exec_id` → `Some`，内容与写入一致） | 保留 |
| `record_exec_output` | 异常 | 目录不可写（报告树路径被文件占位）→ `Err(String)` 串语义 | 保留 |
| `read_exec_output` | 异常 | 未知 / 非法 `exec_id`（空串 / `..` / 路径分隔符 / 盘符 / `.log` 后缀）→ `None`（miss 非错误，零路径逃逸） | 保留 |
| `record_exec_output` / `read_exec_output` | 边界 | 空输出 → 落空 `.log` 可回查 `Some(vec![])`；超长 + 特殊字节（`\n` / `\0` / emoji）全量保真不截断 | 修改 |
| `record_exec_output` / `read_exec_output` | 幂等 | 重复调用 `record_exec_output(root, change, output)` → 每次新铸互异 `exec_id`、各写独立 `.log`，互不覆写（无副作用）；重复 `read_exec_output` 同 `exec_id` 恒返回同字节，不迁移不清理既有文件 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 不 mock：真实临时 workspace 文件系统往返，真实组合 `foundation::layout` | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs

#### 待测功能

- `execute_plan(plan, report_dir): Result<PlanExecution, String>` — 签名不变；`PlanExecution` 增 `output`（stdout + stderr 合并流，不加 `2>&1`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `execute_plan` | 正向 | spawn 成功 → `PlanExecution.output` = `stdout_bytes` 前缀 + `stderr_bytes` 后缀的合并流（逐字），不加 `2>&1` | 新增 |
| `execute_plan` | 正向 | `output` 独立承载合并流：`error` 仍经 `output_text` 派生，`output` 字段不并入 error 尾部语义 | 新增 |
| `execute_plan` | 正向 | rust 档不加 `2>&1`：`cargo test > results.txt` 已重定向 stdout → `output` 主要为 stderr（编译错误面），`results.txt` 仍在报告树 | 新增 |
| `execute_plan` | 异常 | 程序不可达 → `Err`（spawn 前程序解析前置，不产 `PlanExecution`） | 保留 |
| `execute_plan` | 异常 | 非零退出 + 工件不可解析 → `Ok` 携 `output`（合并流）与 `error` 面（被测域失败走产出，不 Err） | 修改 |
| `execute_plan` | 边界 | 超时分支 → `output = Vec::new()`（空）、`exit_code = -1`、`error` 记超时；空 stdout + 空 stderr → `output` 空；大输出（>1000 bytes）→ `output` 全量承载不截断 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 进程 spawn（进程边界） | 不 mock：真实临时目录 + 平台 shim / `cp` / `cmd /C type` 命令 | 合并流 / 超时 / 错误面用例 |
| parser（内部模块） | 不 mock：`parse_cargo_test` / `parse_spec_report` / `parse_coverage` 真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

#### 待测功能

- `outcome(root, change, summary, sub_reports, reports_dir, merged_output): ToolStepOutput` — 增 `merged_output`，record 改脚本输出（去 summary JSON）。
- `record_summary()` —（退役删除）：serde summary JSON 重序列化退役，经 `outcome` 行为与编译期缺失验证，不设独立测试对象。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `outcome` | 正向 | `merged_output` 经 `exec::record_exec_output(root, change, merged_output)` 落单一 `.log` 并回填 `exec_id`；record 内容 = 脚本真实输出（非 summary JSON 重序列化） | 修改 |
| `outcome` | 正向 | 多 suite `output` 按 plan 序（`plans` 迭代序，字典序稳定）聚合 → `merged_output` 拼接序正确 | 新增 |
| `outcome` | 正向 | 全量结构化报告仍落 `report_dir`（`summary.json` + 逐 suite `report.json`） | 保留 |
| `outcome` | 异常 | 落盘失败 → `exec_id` 空串（record 不阻断门禁结论，报告目录仍为全量权威） | 保留 |
| `outcome` | 异常 | 复用分支（既有 summary 可复用）→ `outcome` 同样落 `merged_output`（record 单点面独立于复用门，`exec_id` 每次新铸） | 修改 |
| `outcome` | 边界 | 多 suite 空 `output` → `merged_output` 空，仍落空 `.log` 可回查；`read_exec_output` 回查内容 ≠ `serde_json::to_vec(summary)`（去 summary JSON 断言） | 新增 |
| `outcome` | 幂等 | 重复落盘（重跑 / 复用分支）→ 每次新铸 `exec_id`、单一 `.log` 独立落盘互不覆写，无 summary JSON 残留，report_dir 结构化报告不被 record 干扰（无副作用） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 进程 spawn（进程边界） | 不 mock：真实 tempfile workspace + 平台 shim 命令（沿 `ChainWs` 先例） | 全链组合用例 |
| `exec` / `checks`（内部模块） | 不 mock：`record_exec_output` / `read_exec_output` 与 `checks::aggregate` / `diagnose` 真实组合 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- `read_exec_output(root, change_name, exec_id): Option<String>`（IPC 新增）— UTF-8 lossy；miss = None；暴露 `infra/exec::read_exec_output`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `read_exec_output`（IPC） | 正向 | 已知 `exec_id` → `Some(UTF-8 lossy 字符串)`，内容 = 落盘 `.log` 字节 | 新增 |
| `read_exec_output`（IPC） | 正向 | 非 UTF-8 字节 → lossy 替换符（不 panic 不报错） | 新增 |
| `read_exec_output`（IPC） | 正向 | 磁盘面按 `change_name` 定位（与 `record_exec_output` 同口径），`change_name` 与 id 相异时仍命中 | 新增 |
| `read_exec_output`（IPC） | 异常 | 未知 / 非法 `exec_id` → `None`（miss 非错误） | 新增 |
| `read_exec_output`（IPC） | 异常 | blank `root` → `None` 早退（读命令空结果语义） | 新增 |
| `read_exec_output`（IPC） | 边界 | 空输出 → `Some("")`；超长输出（>1000 chars）→ 完整返回不截断 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| —（无进程边界依赖） | 不 mock：真实临时数据根 / workspace 根 + 真实 `exec::record_exec_output` 落盘，命令直调 `State` 直测（沿 `commands/changes/mod_test.rs` 先例） | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- `change_flow_start_with(app, on_event, root, id, auto_next_phase)`（组合根装配缝）— `ProcessStaticCheck::new()` 构造简化随动：去 `recorded()` 装配分支、默认 Recorded 注释随动；组合根零记录逻辑（record 单点驻 infra/exec）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| `change_flow_start_with` | 正向 | 组合根以 `ProcessStaticCheck::new()`（零参，无 `recorded()` 装配）装配 `LocalToolSteps` 并发起 run → 返回 running 摘要、`run_id` 前缀合法；装配面回归 | 修改 |
| `change_flow_start_with` | 正向 | 组合（链路方向：组合根 → `ProcessStaticCheck::new()` → static-check 步 → infra/exec 落盘）：static-check 步执行后 `output_ref` 回填 `Some(exec_id)`、单一 `.log` 落盘（recorded 默认化经组合根成立，非 `recorded(change)` 装配） | 新增 |
| `change_flow_start_with` | 异常 | blank root / blank id → 守卫 `Err`（构造器简化不破命令面守卫）；未建档 change → `Err` 携 change 名 | 保留 |
| `change_flow_start_with` | 边界 | 同 change 并行 run → 冲突 `Err`（装配幂等面零变更）；重复发起失败分支零登记 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 进程边界（PATH / spawn） | PATH 隔离窗口（`isolate_path` / `TEST_PATH_LOCK`）+ cli 默认实例 fixture：executor 以 CliMissing 合成收敛，不 spawn 真实 CLI | 装配回归 / 守卫 / 并行冲突用例 |
| 进程边界（static-check spawn） | 不 mock：组合用例经 `LocalToolSteps` StaticCheck 分支注入真实 `ProcessStaticCheck::new()` + 平台 shim 命令真实 spawn（临时 workspace），断言 `output_ref` 回填与单一 `.log` 落盘 | recorded 默认化经组合根成立用例 |
| `@tauri-apps` Channel / MockRuntime | MockRuntime app + 捕获型 Channel（沿既有 `mod_test` 先例） | 全部用例 |
| 内部模块 | 不 mock：真实组合 `ProcessStaticCheck::new()` / `LocalToolSteps::new()` / 真实 store（tempfile db） | 全部用例 |

---

## 不可测试项

| 被测文件或模块 | 原因 |
|----------------|------|
| `packages/desktop/src/views/changes/flow/types.ts` | 类型面纯声明（删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode`，`FlowNode = RuntimeFlowNode`，`FlowMaterials` 删 `nodeChecklists`），无运行时行为；由编译期 + 消费方测试（`graph.test.ts` / `attachments.test.ts` / `change-flow-graph.test.tsx`）间接校验，无独立测试路径 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 宏登记（新增 `read_exec_output`），无运行时逻辑；由编译期 + `bindings:check` 一致性守卫承接，无单元测试路径 |
| `packages/desktop/src/types/generated/bindings.ts` | codegen 生成物（`bindings:export` 再生 `readExecOutput` typed 包装，非手改）；由 `bindings:check`（`git diff --exit-code`）一致性守卫承接，无单元测试路径 |
| `packages/desktop/package.json` | `version` 字段 0.4.37 → 0.4.38 为归档时机械变更，无运行时逻辑；由 AC-9 归档步骤承接，无单元测试路径 |
