# 设计: desktop-drawer-session-column

> **变更**: desktop-drawer-session-column
> **日期**: 2026-10-04

---

## 提案与规格同步状态

`proposal.md` 与能力 delta `specs/desktop-change-flow-view/spec.md`（路径相对域根）已由提案阶段写入，属已完成产物，不在本变更清单与任务列表内。测试文件面（proposal「测试文件」清单 12 项，另含 `use-change-detail.test.ts` / `use-session-transcript.test.ts` / `use-session-transcript` summary 面新用例、Rust cfg(test) 构造点 `artifacts/eval_checklist_test.rs` 的 `Workflow` 字面量适配）由 test-design / test-gen / test-execution 阶段承接，不在本清单；前端共置测试文件因 DTO 字段删除产生的类型级最小适配在守线阶段以留痕方式处理（见阶段七）。

---

## 关键设计决策（design 定稿点）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 抽屉双列布局结构 | `detail-drawer.tsx` 的 `aside` 从 `w-[560px]` 加宽至 `w-[960px]`（保留 `max-w-[85vw]`），并**去除整列 `overflow-y-auto` 单滚**：header 之下改内容行 `flex min-h-0 flex-1`，左列 `w-[60%] min-w-0 flex-col`（会话信息区），右列 `min-w-0 flex-1 overflow-y-auto`（既有三分节原样搬入，节内结构零改动）；列宽保护**不做堆叠降级**——85vw 截断时两列按 60/40 等比收窄（各带 `min-w-0` 防溢出），极窄窗口可读性由两列各自的列内滚动兜底 | 结构恒定不跳变是 spec 硬约束（「双列结构 MUST NOT 因选中类型跳变」），堆叠降级引入断点行为、收益低；等比收窄 + 列内滚动是最小实现 |
| D2 | 左列恒渲染与空态占位 | `detail-drawer.tsx` 删除 `roleRefs.length > 0` 条件渲染，`SessionTranscriptPanel` 在左列**恒渲染**；面板对空 `roleRefs`（列头 / ToolStep / Gate / role=null 运行步）从返回 `null` 改为返回空态占位 `<div data-testid="drawer-session-empty">（当前选中无关联会话）</div>` | 双列结构恒定的承接半边：占位由面板内部承担，抽屉壳层零分支；文案一行中性描述，不虚构会话 |
| D3 | active 节点三会话反查 | `selectionRoleRefs` 补 `kind === 'active'` 分支：executor / evaluator / decision 三 ref，`sessionId` 恒 `null`、`sourceRef` 恒为定式 `` `${change}/${node.phase}/${role}/${node.attempt}` ``（`ActivePhase.attempt` 非空保证可组装）。**decision 与 eval 节点不同：active 的 decision 走反查**——eval 的 decision `sourceRef` 恒 null 是「槽位缺席防误挂」的旧数据语义（既有语义不动），active 无槽位可言，反查是该 phase/attempt 下 decision 会话的唯一正确寻址 | 会话建档即落 provenance（source 恒 change、sourceRef 定式）、`create_session` 起始落库、sealed 事件流式 append——进行中会话的已流出部分反查即可见；零后端改动。`liveEvents` 按 sessionId 过滤、active 无 sessionId → 实时事件恒空数组，结构上保证 AC-2「不发起实时事件流订阅」 |
| D4 | 左列元信息字段清单与快照时效 | `SessionTranscriptPanel` 新增 `SessionMeta` 区（置于转录 tab 之下、时间线之上，`data-testid="session-meta"`）：session id（等宽字体 CSS truncate + `title` 全量）、运行状态徽章（`running` → 「运行中」/ 否则「已收口」，沿 `Badge` 既有 `active` / `inv2` 变体）、轮数 `stats.turnCount`、token 合计 `stats.inputTokens` / `stats.outputTokens`（null → 「—」）；`summary` 为 null（查无会话 / 未开跑角色）时元信息区整体不渲染，空态由既有「（暂无该会话转录）」承担。**快照时效：接受查询时快照，不做自动刷新/轮询/订阅**——重开抽屉、切 tab（`RoleTranscript` 经 key 重挂重查）、run 终态刷新即重查 | id 回答「这是谁的会话」、徽章回答「是否在跑」、轮数/tokens 回答「干了多少」；createdAt / durationMs 不上（时间轴在转录内已有，简洁优先）。实时流由 runtime WorkerAgent overlay 节点承担（explore 验证 5），CLI/外部 run 本无实时流；刷新属订阅面改动，收益不抵复杂度 |
| D5 | 转录容器拉满左列 | `RoleTranscript` 的时间线容器从 `max-h-[480px] overflow-y-auto` 改为 `min-h-0 flex-1 overflow-y-auto`；`SessionTranscriptPanel` 根节改 `flex h-full min-h-0 flex-col`（header 行 shrink-0），高度语义从「自持限高」改「填充宿主列」 | `SessionTranscriptPanel` 唯一消费方即抽屉左列（全库 grep 证实），无其他调用面受高度语义变化影响；AC-1「转录拉满左列高度滚动」的直接落点 |
| D6 | interrupted 前端词汇清除触面 | 四文件收敛：`types.ts` 删 `InterruptedFlowNode`、`FlowNodeKind` 收敛 `'eval' | 'active'`、`FlowNode` union 收敛；`graph.ts` 删 `collectInterrupted`、`mergeByStartAt` 第二参收窄 `ActiveFlowNode[]`、`buildFlowGraph` 归并列表只余 `collectActive(detail)`（注释「先 interrupted 后 active」措辞随动）；`flow-event-node.tsx` 删 `InterruptedBody` 与 dashed 回退分支、`nodeClass` 收敛 eval / active 两分支；`attachments.ts` `NODE_PRECEDENCE` 收敛 `['eval', 'active']`（checklist 定位降级链 eval → active） | 存量 `interrupted[]` 留档不出图由双重结构保证：读模型停解析（D7）使 detail 不再携带该字段 + 转换层无收集路径；无编译残留由 tsc 全量拦截（D8） |
| D7 | Rust 读模型停解析（磁盘形状零触点） | `model/workflow.rs` 删 `InterruptedEntry`（含「ActivePhase 加 `end_at` 的扩展」自述 doc）与 `Workflow.interrupted` 字段；`model/mod.rs` re-export 随动；`parse/workflow_file.rs` 删 `interrupted` 提取 match 块与 `Workflow` 构造字段（两段式宽松解析语义不变——`interrupted` 键回落 serde 未知字段忽略，模块 doc「单条 eval / file_log / interrupted 条目」措辞随动）；`queries/detail.rs` 删线面 `InterruptedEntry` + `From` impl、`ChangeDetail.interrupted` 字段与聚合组装、model import 随动。写面 `write/persist.rs` **零触点**（interrupted 零写触点，`ChangeDoc` 双面策略下 typed 从不回写磁盘） | explore 验证 4：写面 raw 定点改写、desktop 从不写 interrupted（写入方只有插件 CLI），读侧删字段零数据丢失；serde 未知字段忽略是既有语义（v2-b fixture 即存量形态实证），含 `interrupted[]` 的存量 workflow.json 照常解析 |
| D8 | golden 显式重写与 bindings 再生 | ① `corpus_golden_test.rs` 的 parse 摘要投影删 `"interruptedCount"` 行；② 以 `DESKTOP_GOLDEN_REWRITE=1` 重写开关（入口见 `corpus_golden_test.rs` 头注）覆写 13 份 change 语料 golden，关开关人工逐份 diff 对照预期清单（见数据模型节）；③ `pnpm -C packages/desktop run bindings:export` 再生 `bindings.ts`（预期 diff 仅 `getChangeDetail` 返回面去 `interrupted` 键 + `InterruptedEntry` 类型消失，`dto.ts` 的 `export type *` 自动跟随）；④ `src-tauri/src/bindings/mod_test.rs` 的 `DTO_TYPES` 断言清单同步移除 `"InterruptedEntry"`（断言清单随动，非新用例） | DTO 字段删除沿 golden 契约冻结的显式重写既定流程（desktop-change-session-visibility 先例）；tsc 全量类型检查拦截前端消费漂移；mod_test 清单与生成物强耦合（「产物缺出线类型」panic），不随动必挂 |
| D9 | 版本 | `packages/desktop/package.json` version 0.4.6 → 0.4.7 **随归档提交执行**（归档轨道，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 引用自动跟随、零改动，`src-tauri/Cargo.toml` 不随动） | 用户可见变更（双列抽屉 / active 联动 / interrupted 下线 / 按钮退役）才提升（仓库既有 bump 口径） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| workflow 磁盘读模型 | `InterruptedEntry` + `Workflow.interrupted` 字段删除（D7）；其余字段与宽松时间戳口径零改动 | `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | serde / time / specta | Rust（serde 磁盘模型，不出线） |
| 模型出口登记 | `pub use` 去 `InterruptedEntry`（D7 随动） | `packages/desktop/src-tauri/crates/core/workflow/src/model/mod.rs` | `model::workflow` | Rust（mod 出口） |
| 宽松解析 | `interrupted` 提取 match 块删除，键回落未知字段忽略；两段式解析骨架不变（D7） | `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs` | `crate::model`、serde_json | Rust（逐字段逐条目宽松解析） |
| 详情线面 | 线面 `InterruptedEntry` + `ChangeDetail.interrupted` 删除；其余字段线面口径（=null + ISO 串、纯 derive 零字段属性）不变（D7） | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `crate::model`、specta | Rust（出线 DTO） |
| bindings 生成物 | 再生后 `ChangeDetail.interrupted` / `InterruptedEntry` 消失，非手写（D8） | `packages/desktop/src/types/generated/bindings.ts` | `export-bindings` crate（specta builder） | TypeScript（生成物） |
| bindings 覆盖清单 | `DTO_TYPES` 断言清单移除 `"InterruptedEntry"`（D8 随动，cfg(test)） | `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 生成物文本断言 | Rust（cfg(test) 断言） |
| 语料投影与 golden | `interruptedCount` 投影删 + 13 份 change 语料 golden 显式重写（D8） | `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` | `workflow::parse` / `workflow::queries`、`DESKTOP_GOLDEN_REWRITE` 开关 | Rust 集成测试投影 + JSON golden |
| 节点视图模型 | `InterruptedFlowNode` 删、`FlowNodeKind` 收敛两分类、`FlowNode` union 收敛（D6） | `packages/desktop/src/views/changes/flow/types.ts` | `types/dto` | TypeScript type |
| 转换纯函数 | `collectInterrupted` 删、归并只剩 active 单节点；边推导 / 缺号兜底 / 插入规则不变（D6） | `packages/desktop/src/views/changes/flow/graph.ts` | `flow/types`、`flow/layout` | TypeScript 纯函数 |
| 素材挂载 | `NODE_PRECEDENCE` 收敛 `['eval', 'active']`（D6） | `packages/desktop/src/views/changes/flow/attachments.ts` | `flow/types` | TypeScript 纯函数 |
| 事件节点渲染 | `InterruptedBody` / dashed 分支删，`nodeClass` 收敛两分支（D6） | `packages/desktop/src/views/changes/flow/flow-event-node.tsx` | `@xyflow/react`、`flow/types` | React 组件（react-flow 自定义节点） |
| 运行步节点渲染 | 「查看会话」按钮与 `onOpenSession` data 键删除（AC-4） | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `@xyflow/react`、`flow/types` | React 组件（react-flow 自定义节点） |
| 图渲染薄层 | `toChartNodes` 的 `onOpenSession` 注入删除；节点点击上抛、边锚定零改动（AC-4） | `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | `@xyflow/react`、`flow/types` | React 组件（react-flow 薄层） |
| 会话寻址 hook | 返回值暴露 `SessionSummary` 面（`summary`），寻址 / 归并 / running 推导不变（D4） | `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | `bindings`（commands.sessionDetail / agentSessions / agentSessionTranscript）、`agent-adapter` | React hook（TypeScript） |
| 左列转录面板 | 会话元信息区（`SessionMeta`）+ 空 refs 空态占位 + 时间线拉满宿主列；三 role tab 不变（D2 / D4 / D5） | `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | `useSessionTranscript`、`AgentTimeline`、`flow/types` | React 组件（TSX） |
| 抽屉双列壳 | 双列布局（D1）+ `selectionRoleRefs` 补 active 三 role 反查分支（D3） | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `SessionTranscriptPanel`、`flow/types`、`renderers` | React 组件（TSX） |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件归 test-design / test-gen / test-execution 阶段，不在本清单（mod_test.rs 的 DTO_TYPES 清单与 golden 投影因与生成物/语料强耦合，按实现清单随动）。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | 删 `InterruptedEntry` 结构体（含「ActivePhase 加 `end_at` 的扩展」自述 doc）与 `Workflow.interrupted` 字段（AC-6 / D7） | 磁盘形状零触点：`interrupted[]` 键经 serde 未知字段忽略原样保留；其余字段、宽松时间戳、camelCase alias 口径零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/mod.rs` | `pub use workflow::{...}` 去 `InterruptedEntry`（AC-6 / D7 随动） | 模型出口单点 |
| `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs` | 删 `interrupted` 提取 match 块、`Workflow` 构造去字段、`InterruptedEntry` import 删；模块 doc「单条 eval / file_log / interrupted 条目损坏时跳过该条」措辞随动（AC-6 / D7） | 两段式宽松解析骨架不变；`interrupted` 键回落第一段 Value 的未知字段忽略；新增措辞零 layout 命名隔离双禁令字面量 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 删线面 `InterruptedEntry` + `From<&DiskInterruptedEntry>` impl、`ChangeDetail.interrupted` 字段、`change_detail` 聚合处 interrupted 组装、`DiskInterruptedEntry` import（AC-6 / D7） | 纯 derive 零字段属性口径不变；`PIPELINE_PHASES` / pipeline 聚合 / active_phase / file_log / artifacts 投影零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` | `project_change_fixture` 的 parse 摘要投影删 `"interruptedCount": workflow.interrupted.len()` 行（AC-7 / D8） | proposal 实现文件清单条目；投影函数属 golden 生成机械，非用例增删 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/*.json`（13 份 change 语料） | 以 `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程覆写（AC-7 / D8；预期 diff 清单见数据模型节） | fixtures 样本文件内容零改写（v2-b 的 `interrupted[]` 原样保留，即存量解析路径实证）；`layout-*` 三份与 `golden/README.md` 零变化 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | `DTO_TYPES` 断言清单移除 `"InterruptedEntry"` 条目（AC-7 / D8 随动） | 与生成物强耦合的覆盖清单，删类型不随动必触发「产物缺出线类型」panic；`COMMAND_NAMES` 零变化 |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm -C packages/desktop run bindings:export` 再生：`getChangeDetail` 返回面去 `interrupted` 键、`InterruptedEntry` 类型消失（AC-7 / D8） | 非手写文件；`dto.ts` 纯 `export type *` re-export 自动跟随、零手改；重导幂等是守线验收面 |
| `packages/desktop/src/views/changes/flow/types.ts` | 删 `InterruptedFlowNode`；`FlowNodeKind` 收敛 `'eval' | 'active'`；`FlowNode` union 收敛；相关 doc 措辞随动（AC-5 / D6） | `RoleSessionRef` / `RuntimeFlowNode` / `DrawerSelection` / `FlowMaterials` 等零改动 |
| `packages/desktop/src/views/changes/flow/graph.ts` | 删 `collectInterrupted`；`mergeByStartAt` 第二参收窄 `ActiveFlowNode[]`；`buildFlowGraph` 归并列表只余 `collectActive(detail)`；模块 doc 三步描述中 interrupted 子句随动（AC-5 / D6） | `PIPELINE_PHASES` 9 列布局、时间序边推导公式、attempt 缺号兜底、插入规则、run overlay 追加语义零改动 |
| `packages/desktop/src/views/changes/flow/flow-event-node.tsx` | 删 `InterruptedBody` 组件与 `kind === 'interrupted'` 渲染分支；`nodeClass` 的 dashed 回退分支删（收敛 eval / active 两分支）；import 随动（AC-5 / D6） | Handle 四侧锚点、eval 三色 / stale 淡化 / active pulse 视觉零改动 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | `NODE_PRECEDENCE` 收敛 `['eval', 'active']`；模块 doc「eval → active → interrupted」措辞随动（AC-5 / D6） | 挂载规则（文档挂列 / checklist 挂节点 / file_log 挂节点 / outsideFiles 兜底）零改动 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `RunStepNodeData` 去 `onOpenSession` 键；`RunStepNode` 删「查看会话」按钮块（AC-4） | 分组徽章 / 步词汇 / 状态视觉 / Handle 零改动；ToolStep / Gate 呈现不变 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | `toChartNodes` 的 runtime 分支注入回 `data: { node }`（`onOpenSession` 注入与注释删）（AC-4） | `onNodeClick`（事件 / 运行步节点共用抽屉入口）、边锚定、translateExtent 零改动 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | `TranscriptLoad` 增 `summary: SessionSummary | null`；`loadBySessionId` 返回 `sessionDetail` 三件套（row + stats + turns）；`loadBySourceRef` 返回反查 latest 的三件套；`UseSessionTranscriptResult` 增 `summary`，双 null 清空态置 `null`（AC-3 / D4） | 直查优先 / 反查兜底 / seq 归并 / running 推导 / liveEvents 并入零改动（proposal「不要修改」条目） |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 增 `SessionMeta` 元信息区（session id 截断 + title 全量、运行状态徽章、轮数、token 合计，`summary` null 时不渲染）；空 `roleRefs` 返回 `drawer-session-empty` 空态占位（原 `null`）；根节改 `flex h-full min-h-0 flex-col`、时间线容器 `max-h-[480px]` 改 `min-h-0 flex-1`（AC-1 / AC-3 / D2 / D4 / D5） | 三 role tab 结构与 `ROLE_LABEL` 词汇不变；`AgentTimeline` 复用、空态 / 错误态呈现不变；不建第二套时间线 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `aside` 改双列布局（`w-[960px]` + `max-w-[85vw]`、去整列单滚、内容行左 60% / 右 40% 各自滚动）；`selectionRoleRefs` 补 `kind === 'active'` 三 ref 反查分支（`sessionId` 恒 null、sourceRef 定式组装）；删 `roleRefs.length > 0` 条件渲染（左列恒渲染面板）；模块 doc active / interrupted 措辞随动（AC-1 / AC-2 / D1 / D3） | 右列三分节（本站文档 / 评估记录 / 文件表）结构与 `data-testid` 锚点零改动；遮罩关闭、右贴边、liveEvents 过滤、selectionTitle 零改动 |
| `packages/desktop/package.json` | version 0.4.6 → 0.4.7 | **归档轨道**（AC-9 / D9），随归档提交执行，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 引用零改动，`src-tauri/Cargo.toml` 不随动 |

### 公共函数 / API

Rust 签名为 Rust 语法；TS 签名为 TS 语法。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `parse_workflow_file` | `crates/core/workflow/src/parse/workflow_file.rs` | 修改 | `pub fn parse_workflow_file(path: &Path) -> WorkflowFileParse` | 签名不变；停提取 `interrupted`（键回落未知字段忽略），宽松解析降级语义不变（D7） |
| `change_detail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | `pub fn change_detail(layout: &Layout, name: &str) -> Option<ChangeDetail>` | 签名不变；`ChangeDetail` 线面减 `interrupted` 字段（D7） |
| `buildFlowGraph` | `packages/desktop/src/views/changes/flow/graph.ts` | 修改 | `function buildFlowGraph(detail: ChangeDetail, runNodes?: RuntimeFlowNode[]): FlowGraph` | 签名不变；interrupted 收集路径删除，图输出收敛两分类事件节点（D6） |
| `useSessionTranscript` | `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 修改 | `function useSessionTranscript(params: { root: string \| null; sourceRef: string \| null; sessionId: string \| null; liveEvents: AgentEvent[] }): UseSessionTranscriptResult` | 入参不变；返回面增 `summary: SessionSummary \| null`（AC-3 / D4） |
| `SessionTranscriptPanel` | `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 修改 | `function SessionTranscriptPanel(props: SessionTranscriptPanelProps): React.JSX.Element` | props 签名不变；空 refs 返回空态占位（原 `null`）、新增元信息区、高度改填充宿主列（D2 / D4 / D5） |
| `DetailDrawer` | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 修改 | `function DetailDrawer(props: DetailDrawerProps): React.JSX.Element \| null` | props 签名不变；双列布局 + active 三 role 反查分支（D1 / D3） |
| `RunStepNode` | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 修改 | `function RunStepNode(props: NodeProps<RunStepFlowNode>): React.JSX.Element` | data 面 `onOpenSession` 删除，「查看会话」按钮退役（AC-4） |

<!-- `mountMaterials` / `mountMaterials` 内部 `locateNode`（签名零变化，`NODE_PRECEDENCE` 为模块私有常量）、`FlowEventNode` / `ChangeFlowGraph` / `PhaseColumnNode`（签名零变化）、`selectionRoleRefs` / `assembleTranscript` / `loadBySessionId` / `loadBySourceRef` / `SessionMeta` / `RoleTranscript`（模块私有，改动在修改文件表与类型定义表说明）不列。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `InterruptedEntry`（磁盘模型） | `crates/core/workflow/src/model/workflow.rs` | 删除 | `Workflow.interrupted` 字段随删；磁盘 `interrupted[]` 键回落 serde 未知字段忽略（D7） |
| `Workflow` | `crates/core/workflow/src/model/workflow.rs` | 修改 | 字段面收敛为 `workflow_type` / `created` / `eval` / `file_log` / `active_phase`（D7） |
| `InterruptedEntry`（线面） | `crates/core/workflow/src/queries/detail.rs` | 删除 | `ChangeDetail.interrupted` 字段随删；bindings 再生后 TS 侧同名类型消失（D7 / D8） |
| `ChangeDetail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | 字段面减 `interrupted`；其余线面口径（=null + ISO 串、纯 derive 零字段属性）不变（D7） |
| `FlowNodeKind` | `packages/desktop/src/views/changes/flow/types.ts` | 修改 | 收敛 `'eval' \| 'active'`（AC-5 / D6） |
| `InterruptedFlowNode` | `packages/desktop/src/views/changes/flow/types.ts` | 删除 | `FlowNode` union 随收敛（AC-5 / D6） |
| `FlowNode` | `packages/desktop/src/views/changes/flow/types.ts` | 修改 | union 收敛为 `EvalFlowNode \| ActiveFlowNode \| RuntimeFlowNode`（AC-5 / D6） |
| `UseSessionTranscriptResult` | `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 修改 | 增 `summary: SessionSummary \| null`（三件套：row + stats + turns）；`messages` / `running` / `error` 面不变（AC-3 / D4） |
| `RunStepNodeData`（`RunStepFlowNode` 载荷） | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 修改 | 删 `onOpenSession?: () => void`，收敛为 `{ node: RuntimeFlowNode }`（AC-4） |

<!-- `SessionSummary` / `SessionRow` / `SessionStats` / `TurnSummary` 为 bindings 既有类型，复用零变化不列；`dto.ts` 纯 re-export 零手写类型不列；`SessionMetaProps` 等面板内部私有 props 不列。 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string（0.4.6 → 0.4.7） | **归档轨道**（AC-9 / D9），随归档提交执行，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 引用自动跟随、零改动，`src-tauri/Cargo.toml` 不随动 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `Workflow` 磁盘读模型（修改） | `workflow_type` / `created` / `eval[]` / `file_log[]` / `active_phase`（`interrupted` 删除） | 经两段式宽松解析收敛；写面 `write/persist.rs` raw 定点改写零触点（typed 从不回写磁盘） | 磁盘 `workflow.json`——**磁盘形状不变**，存量 `interrupted[]` 经 serde 未知字段忽略原样保留，MUST NOT 迁移或清除，写入方（插件 CLI）MUST NOT 因本变更改动 |
| `ChangeDetail` 详情线面（修改） | 既有字段全集减 `interrupted`（`name` / `source` / `inventory` / `created` / `unparsable` / `pipeline[]` / `activePhase` / `fileLog` / `artifacts[]`） | `change_detail` 聚合投影；前端经 bindings 类型消费，tsc 全量拦截漂移 | 不持久化（IPC 瞬时面）；`tests/golden/*.json` 逐字节钉死（显式重写流程维护） |
| `FlowNode` 视图模型（修改） | 两分类：`eval`（record 携带）/ `active`（startAt）+ `runtime` overlay 三词汇 | `buildFlowGraph` 自 `ChangeDetail` 派生；`NODE_PRECEDENCE` 两段定位挂载 | 不持久化（前端运行时派生面） |
| `SessionSummary`（复用，零变更） | `row: SessionRow` + `stats: SessionStats` + `turns: TurnSummary[]` | `session_detail` 直查 / `agentSessions` 反查均返回三件套，`useSessionTranscript` 首次整块暴露为 `summary` | workspace 库（按 root 隔离，只读消费）；IPC 瞬时面 |

**golden 预期 diff 清单（D8 人工确认的验收形态）**：

- 全 13 份 change 语料 golden（v0-a / v0-b / v1-a / v1-b / v1-c / v2-a / v2-b / v3-a / corrupt-bad-eval-entry / corrupt-bad-filelog-entry / corrupt-bad-timestamp / corrupt-bad-verdict / corrupt-invalid-json）的 `detail` 段 `interrupted` 键删除（v2-b 为含一条实数据数组的整键删除，其余 12 份为空数组键删除）。
- 含 parse 摘要段的 10 份 golden（v1-a / v1-b / v1-c / v2-a / v2-b / v3-a / corrupt-bad-eval-entry / corrupt-bad-filelog-entry / corrupt-bad-timestamp / corrupt-bad-verdict）的 `parse.interruptedCount` 键删除（随投影删除）。
- `layout-*` 三份（list 投影）与 `golden/README.md` 零变化；detect 段零变化；fixtures 样本文件内容零改写（v2-b 的 `workflow.json` 含 `interrupted[]` 原样保留——重写后其 detail 段不再投影该键，恰为「存量文件照常解析」的实证路径）。

---

## 路由/API 设计

本变更不涉及 HTTP API；IPC 命令面（Tauri invoke 契约）单键收敛，既有命令零收窄：

| IPC 命令 | 输入 | 输出 | 变化 |
|----------|------|------|------|
| `get_change_detail` | `root: string, change: string` | `ChangeDetail \| null` | 输出面减 `interrupted` 键（bindings 再生）；其余字段与语义零变化 |
| `session_detail` / `agent_sessions` / `agent_session_transcript` | 各自既有入参 | 各自既有出参 | 零变化（active 反查与元信息面纯前端复用，零后端改动） |

认证：本机 IPC，无鉴权面（与既有命令同口径）。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 阶段六 D1 双列布局（`w-[960px]` + `max-w-[85vw]`、左 60% 会话区 / 右 40% 三分节、两列各自滚动、抽屉本体去单滚）+ D2 空态占位（列头 / ToolStep / Gate 左列占位、结构不跳变由「面板恒渲染」结构保证）+ D5 转录拉满左列；布局断言归 test-design / test-gen / test-execution 阶段承接 |
| AC-2 | 阶段六 D3：`selectionRoleRefs` 补 `kind === 'active'` 三 ref 反查分支（`sessionId` 恒 null、sourceRef 定式组装）；未开跑角色落既有「（暂无该会话转录）」空态；「不发起实时事件流订阅」由 sessionId 恒 null → liveEvents 过滤恒空的结构保证；用例归测试轨道 |
| AC-3 | 阶段五 `UseSessionTranscriptResult` 增 `summary`（三件套直查 / 反查两路同源）+ 阶段六 D4 `SessionMeta` 元信息区（id 截断、运行状态徽章、轮数、token 合计；清单定夺见 D4）；hook / 组件用例归测试轨道 |
| AC-4 | 阶段四：`RunStepNodeData` 去 `onOpenSession`、按钮块删、图层注入删；点击节点本体打开抽屉的既有 `onNodeClick` 入口不变；用例归测试轨道 |
| AC-5 | 阶段三 D6 前端词汇清除（types / graph / flow-event-node / attachments 四触点）+ 阶段一读模型停解析双重保证「含 `interrupted[]` 留档的存量 change 详情图上无 interrupted 节点」；无编译残留由 tsc 全量拦截；用例归测试轨道 |
| AC-6 | 阶段一 D7：磁盘模型 / 解析 / 详情线面三触点删除；存量 workflow.json 照常解析由 serde 未知字段忽略既有语义保证（v2-b fixture 为实证语料）；parse / detail / snapshot 断言随动归测试轨道 |
| AC-7 | 阶段二 D8：`interruptedCount` 投影删 + golden 显式重写（预期 diff 清单见数据模型节，关开关人工逐份确认留痕）+ `bindings:export` 再生 + mod_test `DTO_TYPES` 清单随动；tsc 全量类型检查拦截前端消费漂移；语料全量复核归 test-execution 阶段承接 |
| AC-8 | 阶段七守线为静态面（`server:check` 的 cargo fmt + clippy、`client:check` 的 vp check + knip 零新增豁免、bindings 重导幂等）；前端与 Rust 两套自动化套件的全绿验证由 test-execution 阶段承接，不入本设计任务面 |
| AC-9 | D9：version 0.4.6 → 0.4.7 随归档提交执行（归档轨道，配置表已标注，不在实现任务列表）；`tauri.conf.json` 经 `../package.json` 引用零改动、`src-tauri/Cargo.toml` 不随动 |

---

## 依赖

### 运行时依赖

- 无新增 — 双列布局为 flex + tailwind 既有能力；会话联动复用 `bindings` 既有命令面（`sessionDetail` / `agentSessions` / `agentSessionTranscript`）与 `AgentTimeline` 既有组件；Rust 侧为纯字段删除，零新 crate / 零新 trait

### 构建/测试依赖

- 无新增 — 沿 `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）、`client:check`（vp check + knip）、`bindings:export` 既有工具链；golden 重写开关 `DESKTOP_GOLDEN_REWRITE` 既有（corpus-regression 轨道建立）；cargo workspace 套件注册（src-tauri 根）零触达

---

## 待决问题

- 无——proposal 四项待决问题已全部在 design 定夺：running 徽章时效（D4，接受查询时快照）、元信息字段清单（D4）、左列空态文案（D2，「（当前选中无关联会话）」）、窄屏保护（D1，等比收窄不做堆叠降级）。
- 留痕（非待决）：active 节点转录为查询时快照——进行中会话只含已流出部分、徽章不自动翻绿，重开抽屉 / 切 tab / run 终态刷新即重查（D4 拍板口径，与 proposal 风险表的缓解措施一致）。
- 留痕（非待决）：interrupted 留档信息（start/end 时间）在 UI 完全不可达为有意识的简化（proposal 已拍板）；磁盘数据原样保留，如需恢复走读模型加字段的反向变更即可。
- 留痕（非待决）：Rust cfg(test) 构造点（`artifacts/eval_checklist_test.rs` 的 `Workflow` 字面量、`detail_test.rs` / `snapshot_test.rs` / `workflow_file_test.rs` 断言）不在默认编译面，适配由测试轨道承接；前端共置测试文件的类型级最小适配在守线阶段留痕处理（阶段七）。
