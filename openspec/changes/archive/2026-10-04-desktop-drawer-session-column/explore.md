# 抽屉两列化 + active 节点会话联动探索（含 interrupted 下线）

> 状态:探索收敛（五决策全部拍板），待 `phase-proposal` 收敛成 change
> 日期:2026-10-04
> 关联 change:无（拟议名 `desktop-drawer-session-column`）
> 探索触发:「运行中的变更节点也能查看会话；将详情抽屉分成左右两列，左边放置会话信息」

## 现状与会话联动覆盖矩阵

`selectionRoleRefs`（detail-drawer.tsx）只覆盖两类节点，active / interrupted 点开抽屉无转录区：

| 节点类型 | 来源 | 会话联动 | 寻址方式 |
|---|---|---|---|
| eval | `pipeline` attempts[]（已收口） | ✓ 三 role tab | 槽位 id 直查优先 + sourceRef 反查兜底 |
| **active（运行中 pulse）** | `workflow.json` active_phase | **✗ 缺口** | — |
| interrupted | `workflow.json` interrupted[] | ✗ | — |
| runtime WorkerAgent | 本 app run 步流 overlay | ✓ 单 role | `node.sessionId` 直查 |
| runtime ToolStep / Gate | 同上 | ✗（本来无会话） | — |

缺口场景：CLI 在外部跑的 run、desktop 发起后刷新页面重挂、run 中途 crash 残留 active_phase——图上无 runtime overlay 节点，点 active 节点看不到正在干活的会话。

## 关键验证（全部经代码实测，非记忆）

1. **建档即落库**：walker.rs:861 会话 provenance（source 恒 "change"，sourceRef = `<change>/<phase>/<role>/<attempt>`）；store_port.rs:28 `create_session` 在 session start 就写行（不必等收口），sealed 事件流式 append → **进行中会话经 sourceRef 反查即可见，转录含已流出的部分**。
2. **反查路径现成**：use-session-transcript.ts:43 `loadBySourceRef`（`agentSessions(root, 'change', sourceRef)`）即旧数据兜底同款路径，active 节点直接复用，零后端改动。
3. **ActivePhase.attempt 非空**（bindings.ts:197，`number`）→ sourceRef 组装安全。
4. **写面双面策略（interrupted 下线无数据丢失的根据）**：write/persist.rs `ChangeDoc { raw: Value, typed: Workflow }`——写回永远序列化 `raw`（原文载入定点改写，未知/legacy 字段原样保留），`typed` 从不回写磁盘；写触点五类（active_phase 置/清、eval 追加、stale 翻转、backtrack 标记），**`interrupted` 零触点，desktop 从不写它**（写入方只有插件 CLI）。从 typed 模型删字段，磁盘 `interrupted[]` 原样保留，serde 默认忽略未知字段，存量文件解析不受影响。
5. **active 节点无 sessionId 的实时流限制影响小**：本 app run 场景图上有 runtime 节点（带 sessionId）承担实时流；CLI/外部 run 本无实时流，反查重放已覆盖进行中内容。

## 决策落定（五项）

| # | 决策 |
|---|---|
| 1 | 无会话选中（列头 / ToolStep / Gate）→ 左列**空态占位**，双列结构恒定不跳变 |
| 2 | **下线 interrupted 节点**：读模型停解析 `interrupted` 字段（不只是前端隐藏），UI 信息面彻底移除 |
| 3 | 抽屉加宽 ~960px，左列（会话）占 60%，保留 `max-w-[85vw]` |
| 4 | 左列放**会话元信息**（session id、运行状态徽章等，具体清单 design 定夺） |
| 5 | 「查看会话」按钮**都不加**：active 不加、WorkerAgent 现有显式入口一并下线（两列布局后点节点即见左列转录，按钮冗余） |

## 目标形态

```
┌─────────────────────────────────────────────┐
│ 标题                                [关闭]   │
├──────────────────────┬──────────────────────┤
│ 会话信息（左列 60%）   │ ① 本站文档            │
│ [执行][评估][决策]     │ ② 评估记录            │
│ 元信息（id/状态徽章）   │ ③ 文件清单            │
│ ┌──────────────────┐ │ （列内滚动）           │
│ │ AgentTimeline    │ │                      │
│ │ 拉满高度滚动       │ │                      │
│ └──────────────────┘ │                      │
└──────────────────────┴──────────────────────┘
```

- **布局**：单列 560px 纵向四节（转录垫底、前面内容多时难看到）→ 双列；转录 `max-h-480px` 顺势拉满左列；抽屉整体从单滚改两列各自滚；遮罩关闭、右贴边不变。
- **active 节点联动**：`selectionRoleRefs` 补 `kind === 'active'` 分支，eval 同款三 role（sessionId=null、sourceRef 反查）；未开跑角色落现成「（暂无该会话转录）」空态，语义自洽。
- **元信息来源**：`sessionDetail` / `agentSessions` 返回的 `SessionSummary`（row + stats + turns）；现 hook 只取 transcript + running，需暴露 summary 面。

## interrupted 下线触面

```
Rust  core/workflow
  model/workflow.rs        InterruptedEntry + interrupted 字段删
                           （ActivePhase 注释自述"InterruptedEntry 加 end_at 的扩展"，
                             定义联动一并清理）
  queries/detail.rs        ChangeDetail.interrupted 删
  tests/golden/*.json      detail 线面 golden 显式重写（v2-b 有实数据；
                           corpus_golden_test 投影 interruptedCount 随动）
  orchestration/snapshot_test 等断言随动

TS    packages/desktop
  bindings.ts              ChangeDetail.interrupted / InterruptedEntry 再生消失
  flow/types.ts            InterruptedFlowNode 删；FlowNodeKind 收敛 'eval' | 'active'
  flow/graph.ts            collectInterrupted 删；mergeByStartAt 只剩 active 单节点
  flow/flow-event-node.tsx InterruptedBody / dashed 分支删
  flow/attachments.ts      checklist 定位降级 eval → active（三段收敛两段）
  flow/run-step-node.tsx   查看会话按钮删（决策 5）
  flow/change-flow-graph.tsx onOpenSession 接线删
  flow/detail-drawer.tsx   两列布局 + selectionRoleRefs 补 active 三 role 反查

Spec  desktop-change-flow-view
  「三分类节点」→ 两分类 + runtime overlay；「interrupted 独立成节点」requirement 删；
  「时间序边推导」interrupted 归并子句删；「查看会话显式入口」requirement 删；
  「右侧抽屉单一交互入口」分节描述改两列
```

**明示后果（有意识的简化）**：中断留档信息（start/end 时间）在 UI 完全不可达；残留 active_phase 节点（下线后唯一"未收口"痕迹）承担该语义。interrupted 与 active 的 UI 区分度本就低（都是"没走到 eval"）。

## design 层备注（design 阶段定夺）

1. **running 徽章时效**：左列元信息来自查询时快照（轮行推导），会话收口后徽章不自动翻绿，直到重开抽屉/切 tab 或 run 终态刷新换节点。可接受还是要做刷新，design 定。
2. **golden 显式重写**：DTO 字段删除走既定流程（golden 重写、bindings 再生、v3 代际夹具若涉及则随动）。
3. 左列空态文案、元信息具体字段清单（id 截断展示？轮数/token 统计要不要）、窄屏保护（85vw 截断时列宽下限）均 design 定。

## 对话纪要

1. 触发:运行中变更节点查看会话 + 抽屉两列化。摸清联动覆盖矩阵：缺口 = active 节点（interrupted 顺带提出）。
2. 实测后端链路:建档即写 provenance（walker.rs:861）、create_session 起始落库、sealed 事件流式 append → active 节点接 sourceRef 反查零后端改动可行。
3. 五决策拍板:空态占位（A）/ interrupted 整体下线（读模型停解析，深于最初"顺带覆盖"的提议）/ 960px 左 60% / 左列放元信息 / 查看会话按钮全下线。
4. 关键风险排除:写面双面策略（raw 定点改写、typed 从不回写、interrupted 零写触点）→ 读侧删字段零数据丢失；CLI 写入方不动。
5. 落盘本 explore；后续 `phase-proposal` 以此为输入收敛正式提案（spec: desktop-change-flow-view 演进）。
