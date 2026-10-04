# 任务: desktop-drawer-session-column

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件。`workflow_file_test.rs` / `detail_test.rs` / `snapshot_test.rs` / `artifacts/eval_checklist_test.rs`（cfg(test) 构造点适配）与前端共置测试文件的 interrupted 用例删除、双列布局 / active 三 role 反查 / 元信息呈现新用例均归测试轨道。顺序按依赖排列：Rust 读模型停解析 → golden 重写与 bindings 再生 → 前端节点模型收敛 → 查看会话入口下线 → hook summary 面 → 抽屉双列与 active 联动 → 守线。版本 bump 归档轨道（AC-9 / design D9），随归档提交执行，不在本列表。

## 阶段一：Rust 读模型停解析（AC-6）

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs`：删 `InterruptedEntry` 结构体（含「ActivePhase 加 `end_at` 的扩展」自述 doc）与 `Workflow.interrupted` 字段（design D7）；其余字段、宽松时间戳、camelCase alias 口径零改动，新措辞零 layout 命名隔离双禁令字面量
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/model/mod.rs`：`pub use workflow::{...}` 去 `InterruptedEntry`（design D7 随动）
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs`：删 `interrupted` 提取 match 块与 `Workflow` 构造字段、`InterruptedEntry` import 删；模块 doc「单条 eval / file_log / interrupted 条目损坏时跳过该条」措辞随动（design D7）——两段式宽松解析骨架不变，`interrupted` 键回落第一段 Value 的未知字段忽略
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：删线面 `InterruptedEntry` + `From<&DiskInterruptedEntry>` impl、`ChangeDetail.interrupted` 字段、`change_detail` 聚合处 interrupted 组装、`DiskInterruptedEntry` import（design D7）；纯 derive 零字段属性口径与其余投影零改动

## 阶段二：golden 显式重写与 bindings 再生（AC-7）

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs`：`project_change_fixture` 的 parse 摘要投影删 `"interruptedCount": workflow.interrupted.len()` 行（design D8；投影函数属 golden 生成机械，非用例增删）
- [x] 触发 golden 显式重写流程：以 `DESKTOP_GOLDEN_REWRITE=1` 环境变量启用重写开关（运行入口与流程见 `corpus_golden_test.rs` 头注）覆写 `packages/desktop/src-tauri/crates/core/workflow/tests/golden/` 下 13 份 change 语料快照；随即关开关人工逐份 diff 确认符合 design「golden 预期 diff 清单」（全 13 份 detail 段 `interrupted` 键删、10 份 parse 摘要 `interruptedCount` 键删、`layout-*` 三份与 `golden/README.md` 零变化），确认记录留痕于本任务完成备注；fixtures 样本文件内容零改写（v2-b 的 `workflow.json` 含 `interrupted[]` 原样保留）；关开关后的等价复核与全量语料回归归 test-execution 阶段承接
  - 留痕（重写后人工逐份 diff 确认）：13 份 golden 共 31 行删除、0 行新增——全 13 份 detail 段 `interrupted` 键删除（v2-b 为含一条实数据对象的整键删除，其余 12 份为空数组键删除）；10 份含 parse 摘要段 golden 的 `interruptedCount` 键删除（v1-a / v1-b / v1-c / v2-a / v2-b / v3-a / corrupt-bad-eval-entry / corrupt-bad-filelog-entry / corrupt-bad-timestamp / corrupt-bad-verdict；corrupt-invalid-json / v0-a / v0-b 无 parse 摘要段，仅 detail 键删）；`layout-*` 三份与 `golden/README.md` 零变化；fixtures 样本文件零触达；关开关复核 `cargo test -p workflow --test corpus_golden_test` 18 用例全绿
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生 `packages/desktop/src/types/generated/bindings.ts`（design D8）：预期 diff 仅两处——`getChangeDetail` 返回面去 `interrupted` 键、`InterruptedEntry` 类型消失；`dto.ts` 纯 `export type *` re-export 自动跟随、零手改
- [x] 修改 `packages/desktop/src-tauri/src/bindings/mod_test.rs`：`DTO_TYPES` 断言清单移除 `"InterruptedEntry"` 条目（design D8 随动；与生成物强耦合的覆盖清单，非新用例）；`COMMAND_NAMES` 零变化

## 阶段三：前端节点模型两分类收敛（AC-5）

- [x] 修改 `packages/desktop/src/views/changes/flow/types.ts`：删 `InterruptedFlowNode` 接口；`FlowNodeKind` 收敛 `'eval' | 'active'`；`FlowNode` union 收敛为 `EvalFlowNode | ActiveFlowNode | RuntimeFlowNode`；「事件节点三分类」等 doc 措辞随动（design D6）；`RoleSessionRef` / `RuntimeFlowNode` / `DrawerSelection` / `FlowMaterials` 零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/graph.ts`：删 `collectInterrupted` 函数；`mergeByStartAt` 第二参收窄 `ActiveFlowNode[]`；`buildFlowGraph` 归并列表只余 `collectActive(detail)`；模块 doc 三步描述中 interrupted 子句随动（design D6）；`PIPELINE_PHASES` 9 列布局、边推导公式、attempt 缺号兜底、插入规则、run overlay 追加语义零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/flow-event-node.tsx`：删 `InterruptedBody` 组件与 `kind === 'interrupted'` 渲染分支；`nodeClass` dashed 回退分支删（收敛 eval / active 两分支）；import 随动（design D6）；Handle 锚点与 eval 三色 / stale 淡化 / active pulse 视觉零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/attachments.ts`：`NODE_PRECEDENCE` 收敛 `['eval', 'active']`；模块 doc「eval → active → interrupted」措辞随动（design D6）；挂载规则与 outsideFiles 兜底零改动

## 阶段四：「查看会话」入口全下线（AC-4）

- [x] 修改 `packages/desktop/src/views/changes/flow/run-step-node.tsx`：`RunStepNodeData` 去 `onOpenSession?: () => void` 键（收敛为 `{ node: RuntimeFlowNode }`）；`RunStepNode` 删「查看会话」按钮块与 `data.onOpenSession` 解构（design AC-4）；分组徽章 / 步词汇 / 状态视觉 / Handle 零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/change-flow-graph.tsx`：`toChartNodes` 的 runtime 分支注入回 `data: { node }`（`onOpenSession` 注入与注释删）（design AC-4）；`onNodeClick`、边锚定、translateExtent 零改动

## 阶段五：会话 summary 面暴露（AC-3）

- [x] 修改 `packages/desktop/src/views/changes/hooks/use-session-transcript.ts`：`TranscriptLoad` 增 `summary: SessionSummary | null`；`loadBySessionId` 返回 `sessionDetail` 三件套（row + stats + turns，`running` 推导不变）；`loadBySourceRef` 返回反查 latest 的三件套；`UseSessionTranscriptResult` 增 `summary`，双 null 清空态置 `null`（design D4）；直查优先 / 反查兜底 / seq 归并 / liveEvents 并入零改动

## 阶段六：抽屉双列布局与左列会话信息区（AC-1 / AC-2 / AC-3）

- [x] 修改 `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`：根节改 `flex h-full min-h-0 flex-col`（header 行 shrink-0）；`RoleTranscript` 增 `SessionMeta` 元信息区（session id 等宽 CSS truncate + `title` 全量、运行状态徽章 `Badge` `active`/`inv2` 变体、轮数 `stats.turnCount`、token 合计 `stats.inputTokens`/`stats.outputTokens` 缺席「—」，`data-testid="session-meta"`，`summary` null 不渲染，design D4）；空 `roleRefs` 返回 `（当前选中无关联会话）` 占位（`data-testid="drawer-session-empty"`，原 `null`，design D2）；时间线容器 `max-h-[480px]` 改 `min-h-0 flex-1 overflow-y-auto`（design D5）；三 role tab 结构与 `ROLE_LABEL`、`AgentTimeline` 复用、空态 / 错误态零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：`aside` 改 `w-[960px] max-w-[85vw]` 并去整列 `overflow-y-auto`；header 之下改内容行 `flex min-h-0 flex-1`——左列 `w-[60%] min-w-0 flex-col` 恒渲染 `SessionTranscriptPanel`（删 `roleRefs.length > 0` 条件渲染），右列 `min-w-0 flex-1 overflow-y-auto` 承载既有三分节（节结构与 `data-testid` 锚点零改动）（design D1）；`selectionRoleRefs` 补 `kind === 'active'` 分支——executor / evaluator / decision 三 ref，`sessionId` 恒 `null`、`sourceRef` 定式 `` `${change}/${node.phase}/${role}/${node.attempt}` ``（design D3）；模块 doc active / interrupted 措辞随动；遮罩关闭、右贴边、selectionTitle、liveEvents 过滤零改动

## 阶段七：守线与静态自查（静态，不含测试执行）

- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy；cfg(test) 测试代码不在默认编译面，Rust 构造点适配由测试轨道承接）与 `pnpm -C packages/desktop run client:check`（vp check + knip，零新增豁免条目）；前端与 Rust 两套自动化套件的全绿验证由 test-execution 阶段承接（AC-8 执行面），不入本任务面
- [x] 前端共置测试文件类型级最小适配（使 vp check 类型面恢复编译绿）：`app.test.tsx` / `use-change-detail.test.ts` / `change-detail-view.test.tsx` / `graph.test.ts` / `layout.test.ts` / `attachments.test.ts` / `flow-event-node.test.tsx` / `change-flow-graph.test.tsx` / `detail-drawer.test.tsx` 等夹具对象去 `interrupted` 键、`RunStepNodeData` 字面量去 `onOpenSession`——仅字段面删除，被本变更有意取代语义的用例改写（interrupted 节点用例、view-session 按钮用例、active 无联动用例）归测试轨道（test-design 派生新用例面）
  - 留痕：适配仅限编译级字段删除与调用点收参，不改写任何断言语义；清单外零触达。具体：① 夹具对象 `interrupted` 键删除（app / use-change-detail / change-detail-view / graph / layout / attachments / change-flow-graph / detail-drawer 八文件）；② 被本变更有意取代语义、且去键后仍无法编译或断言对象已消亡的用例整块删除（interrupted 节点用例、view-session 按钮用例、active 无联动用例——其新语义用例由 test-design 派生）；③ graph.test / change-flow-graph.test 中时间序边推导、插入规则、回跳 label 等「保留不变量」用例的夹具载体由 interrupted 单条目适配为同参 activePhase（断言语义零改写，仅节点 id 前缀与用例标题随载体随动）；④ run-step-node.test.tsx 零触达（`as unknown as` 转译下无编译错误，view-session 用例归测试轨道）
- [x] bindings 重导幂等自查：再跑 `pnpm -C packages/desktop run bindings:export` 零新增 diff，`bindings.ts` 变化仅 `getChangeDetail` 返回面减键 + `InterruptedEntry` 类型消失（design D8）
- [x] 静态自查：禁改面零 diff——`plugins/dev-team/bin/src/schemas/workflow.schema.ts`（TS schema 不动）、插件 CLI 写入方、`write/persist.rs` 五类写触点（interrupted 零写触点）、fixtures 样本文件内容（v0-a … v3-a 及 corrupt-\*，v2-b 的 `interrupted[]` 原样保留）、`PIPELINE_PHASES` 9 列布局 / 时间序边推导 / attempt 缺号兜底 / 空列恒定 / stale 淡化、`useSessionTranscript` 既有寻址语义（直查优先 / 反查兜底 / seq 归并 / running 推导）、`AgentTimeline` 与会话重放基建、取数模型（显式刷新 + Channel 例外）、`tauri.conf.json` 与 `src-tauri/Cargo.toml` 版本引用；新增 / 改写 `.rs` 注释零 layout 命名隔离双禁令字面量命中（磁盘域根目录名与配置文件名，foundation layout 扫描口径）
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏）；`packages/desktop/package.json` 实现期零触点（version 0.4.6 → 0.4.7 归档轨道执行，AC-9 / design D9）
