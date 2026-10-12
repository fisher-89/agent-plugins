# 变更详情流程图：仅 step 节点 + 抽屉单列 + stdout 展示

日期:2026-10-11 · 状态:探索完成，待开 change

## 目标

packages/desktop 变更详情流程图（`src/views/changes/flow/`）本轮改造：

1. **移除 phase 节点**（eval + active），流程图仅显示 step（runtime）节点，运行中由 running 态 runtime 节点承载。
2. **抽屉单列**：step 节点点击后单列展示 agent 会话转录（sessionId）或 stdout（outputRef）。
3. **无点击判据**：无增量信息源的节点不可点击。
4. **static-check 启用 recorded**：stdout 落盘可回查。
5. **test-execution record 改脚本执行输出**（原为 summary JSON 重序列化）。

## 现状数据流

```
ChangeDetail（统一视图）
├─ pipeline[].attempts[]  ─collectEvals─►  EvalFlowNode('eval')      ← 移除
├─ activePhase            ─collectActive─► ActiveFlowNode('active')  ← 移除
└─ runs[].nodes           ─unifiedRunNodes→runStepNodes─► RuntimeFlowNode('runtime')  ← 保留
   runs[].edges           ─unifiedRunEdges→tableEdges─► FlowEdge（端点按 runtime 身份键）
```

`buildFlowGraph(detail, runNodes, runEdges)`（graph.ts）组装序：

```
columns  = 10 站容器（col:<phase>）
sequence = [...runNodes]          # 原 eval + active 归并链删除
edges    = tableEdges(runEdges, sequence, pipeline)   # pipeline 参数保留
```

## 最终决策记录

- eval + active 都去掉；运行中由 step 承载
- eval-checklist 素材挂列头；评估 report 不展示
- 抽屉单列按信息源分派（sessionId → 会话转录 / outputRef → stdout）
- 无点击 = sessionId 与 outputRef 皆 null
- static-check 接受 name 参数改造成本，启用 recorded
- test-execution record = 脚本真实输出（stdout+stderr 合并），非 summary JSON
- 削减流转类 step（phase_start/phase_log）为**后续 change**，本轮不做

---

## 前端改动清单

| # | 文件 | 改动 |
|---|------|------|
| 1 | `flow/graph.ts` | 删 `collectEvals`/`collectActive`/`mergeByStartAt`/`anchorMs`；`sequence = [...runNodes]`；**`pipeline` 参数保留**（`tableEdges` 的 backtrack 边 label 仍读它） |
| 2 | `flow/types.ts` | 删 `FlowNodeKind`/`EvalFlowNode`/`ActiveFlowNode`；`FlowNode = RuntimeFlowNode`；`FlowMaterials` 删 `nodeChecklists` |
| 3 | `flow/change-flow-graph.tsx` | 删 event 通道（`nodeTypes.event`/`toChartNodes` 非 runtime 分支/`onNodeClick` 'event' 判）；`onNodeClick` 加可点击判据 |
| 4 | `flow/flow-event-node.tsx` + 测试 | 整文件退役删除 |
| 5 | `flow/attachments.ts` | 全量改挂 `columnDocs`；删 `NODE_PRECEDENCE`/`locateNode`/`nodeChecklists`；`mountMaterials` 去 `graph` 参数（连带 `useFlowGraphData` 的 materials memo 依赖收窄 `[detail, artifacts]`） |
| 6 | `flow/detail-drawer.tsx` | 单列重构：删 `DrawerEvalSection`+`InlineChecklist`；按 sessionId/outputRef 分派；删 eval/active 分支；新增 `name` prop（read_exec_output 用 change name 非 id） |
| 7 | 新增 `flow/stdout-panel.tsx` + `hooks/use-exec-output.ts` | outputRef → read_exec_output 渲染 |
| 8 | 测试 | 删 flow-event-node.test；改 graph.test / change-flow-graph.test / detail-drawer.test / attachments.test / change-detail-view.test；新增 stdout 面板测试 |

### 关键事实链

- **eval-checklist 挂列头零成本**：payload 自带 `phase`（`eval-checklist-renderer.tsx` 的 `EvalChecklistPayload = { phase, attempt, verdict, items }`），`artifactColumn` 已有 pipeline 外相位校验；`EvalChecklistRenderer` 已能渲染全套元数据，列头抽屉走 `ArtifactTabs` 即达，渲染组件零改动。
- **运行中承载无缺口**：header Badge 来自 `detail.activePhase`（保留）；图上 running 态 runtime 节点已有 pulse（run-step-node `statusClass`）。

### 无点击判据

```
runtime 节点点击 →
  role !== null      → 单列会话转录（复用 SessionTranscriptPanel）
  outputRef !== null → 单列 stdout 面板（新组件）
  两者皆 null        → 无点击（不 onSelect）
```

覆盖流转两词（phaseStart/phaseLog）+ gate 三门 + git 四步 + preflight/seal（均无 sessionId/outputRef）。detail 短文本已在节点内显示，抽屉无增量信息。

---

## 后端改动清单

| # | 面 | 改动 |
|---|----|------|
| 9 | IPC | 新增 `read_exec_output(root, change_name, exec_id) -> Option<String>`，暴露 `infra/exec::read_exec_output`（UTF-8 lossy；miss=None） |
| 10 | static-check | 启用 recorded + `2>&1` 交错（见下） |
| 11 | test-execution | record 改脚本执行输出（见下） |

### stdout/stderr 结合：spawn 层 `2>&1` 交错合并（仅 static-check）

**关键洞察**：交错顺序在分离收集时就已丢失，无法在 record 层恢复。分文件 `.log`/`.err` 只是分离快照，与终端交错输出不一致。

正确做法：shell 在 OS 层 `2>&1` 把 stderr 并入 stdout 管道，两 fd 指向同一管道，写入顺序即真实顺序。

```rust
// static-check 现状（static_check.rs）：分离收集，diagnostics = stdout + stderr（顺序失真）
let mut command = exec::SystemCommand::script(&command).into_tokio();
command.stdout(piped).stderr(piped);
let output = command.output().await?;

// 改为：2>&1 单一交错流
let script = format!("{command} 2>&1");
let mut command = exec::SystemCommand::script(&script).into_tokio();
command.stdout(piped);                  // stderr 已并入 stdout
let output = command.output().await?;   // output.stdout = 交错流，stderr 空
// diagnostics = output.stdout（交错，更还原）；record = output.stdout（单一流）
```

落盘面：`record_exec_output` 收敛为单一 `.log`（`.err` 退役），`read_exec_output` 读同一 `.log` 即完整还原终端；后端命令无需单独读 stderr。

### static-check recorded 成本

```
port.rs          StaticCheckRunner::run(root) → run(root, name)（对齐 TestExecutionRunner）
static_check.rs  ProcessStaticCheck 去 record_anchor 构造；execute 加 name；
                 spawn 2>&1；record = 交错 .log；diagnostics = 交错 stdout
steps.rs         StaticCheck 分支解析 name（抄 TestExecution 分支 get_change(id).map(name)）
change_flow/mod  ProcessStaticCheck 构造器简化
```

### test-execution record：脚本执行输出（不加 2>&1）

**不加 `2>&1`**：rust 模板 `cargo test > "{results_file}"` 已把 stdout 重定向进结果工件，`2>&1` 会让 stderr（编译错误）混入污染 `parse_cargo_test`。

record 内容 = runner 捕获的 `stdout_bytes + stderr_bytes` 合并流（脚本真实输出）：

| 框架 | 结果去向 | 脚本 stdout 管道形态 |
|------|---------|---------------------|
| jest | `--outputFile` + `--silent` | 部分（被抑制） |
| vitest / vite-plus | `--reporter=json --outputFile.json` | 部分（banner） |
| rust | `cargo test > results.txt` 重定向 | 基本空（主输出进文件） |
| node-test | `--test-reporter-destination` | 部分 |

- 不 serde summary JSON；全量结构化报告仍在 report_dir 子树（各框架 report 工件各自为政）
- 各框架输出形态天然不同 = 「不同框架 report 可按需调整」，零统一格式负担
- rust stdout 重定向进 results.txt → record 主要是 stderr（编译错误）；results.txt 在报告树可见，不强行 tee

实现要点：`runner.rs` 的 `PlanExecution` 增 `output` 字段（当前 stdout_bytes/stderr_bytes 是局部变量只进 error 尾部）；`mod.rs` 聚合各 suite output 按 plan 序（字典序稳定）拼接落单一 `.log`。

---

## 边界（本轮不做）

- **削减流转类 step**（phase_start/phase_log）——后续 change。涉及后端 walker 源头不 emit（`step_phase_start`/`step_phase_log`×3 变体去 begin_node+emit_node，保留落账/开相副作用）+ `entry_kind` 迁移（executor/preflight 消费）+ 前端 `run-state.ts` 过滤兜底（已落库旧数据）。边链正确性需 walker 层 tail 不移动，非落库过滤能天然达成。
- 词汇枚举 RunStepKind / StepNode / ChangeStepKind 保留（历史数据兼容 + 前端 bindings 不变）。
