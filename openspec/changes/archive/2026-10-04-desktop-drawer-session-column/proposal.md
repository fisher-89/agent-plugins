# 提案: desktop-drawer-session-column

> **变更**: desktop-drawer-session-column
> **日期**: 2026-10-04
> **状态**: proposed

---

## 问题

change 详情抽屉的会话转录联动存在覆盖缺口，且单列布局压制了转录可读性：

1. **active（运行中）节点无会话联动**：`selectionRoleRefs`（detail-drawer.tsx）只覆盖 eval 与 runtime WorkerAgent 两类节点。CLI 在外部跑的 run、desktop 发起后刷新页面重挂、run 中途 crash 残留 active_phase——这些场景图上无 runtime overlay 节点，点 active 节点看不到正在干活的会话。而链路本身已就绪：会话建档即落 provenance（`source` 恒 `"change"`、`sourceRef = <change>/<phase>/<role>/<attempt>`）、`create_session` 在 session start 就写行、sealed 事件流式 append，`use-session-transcript.ts` 的 `loadBySourceRef` 反查路径现成（旧数据兜底同款），active 节点接上即可见进行中会话的已流出转录——零后端改动。
2. **抽屉单列 560px，转录垫底**：现有三分节【本站文档 | eval report+checklist | 文件表】纵向排列，会话转录区垫底且限高 `max-h-480px`，前面内容多时转录难看到。
3. **interrupted 节点信息价值低**：与 active 的 UI 区分度本就低（都是"没走到 eval"），独立 dashed 节点维护成本（types / graph / 渲染 / 挂载优先级 / Rust 读模型 / golden 全链）大于其留档价值。

---

## 提案

三项收敛（探索五决策已拍板，见 openspec/changes/desktop-drawer-session-column/explore.md）：

1. **抽屉两列化**：抽屉从单列 560px 加宽至约 960px（保留 `max-w-[85vw]`），左列（约 60% 宽）为会话信息区——上方会话元信息（session id、运行状态徽章等，具体清单 design 定夺），下方转录 tab 与 `AgentTimeline` 拉满左列高度滚动；右列为既有三分节，各自在列内滚动。无会话联动的选中（列头 / ToolStep / Gate）左列呈空态占位，双列结构恒定不跳变。
2. **active 节点三会话联动**：`selectionRoleRefs` 补 `kind === 'active'` 分支，与 eval 同款三 role tab（executor / evaluator / decision）；active_phase 无会话槽位，`sessionId` 恒 null、走 sourceRef 反查（`ActivePhase.attempt` 非空保证定式可组装）；未开跑角色落现成「（暂无该会话转录）」空态。左列元信息取自查询返回的 `SessionSummary`（row + stats + turns），`use-session-transcript.ts` 需暴露 summary 面（现只返回 transcript + running）。
3. **interrupted 节点整体下线**：读模型停解析 `interrupted` 字段（Rust `Workflow.interrupted` / `ChangeDetail.interrupted` 删除，不只是前端隐藏），前端节点模型收敛两分类（`FlowNodeKind = 'eval' | 'active'`），UI 信息面彻底移除。写面零触点：desktop 从不写 interrupted（写入方只有插件 CLI），磁盘上存量 `interrupted[]` 经 serde 未知字段忽略原样保留、解析不炸，零数据丢失。**明示后果（有意识的简化）**：中断留档时间在 UI 完全不可达，残留 active_phase 节点成为唯一"未收口"痕迹。
4. **「查看会话」按钮全下线**：active 不加，WorkerAgent 节点现有显式入口（run-step-node.tsx）一并移除——两列布局后点节点即见左列转录，按钮冗余。

能力演进落点：`desktop-change-flow-view` 修改（节点模型三分类 → 两分类、抽屉双列、active 联动、查看会话入口下线）；DTO 字段删除沿 golden 显式重写既定流程（golden 重写、bindings 再生）。

---

## 能力

### 新增能力

- 无（本变更全部为既有能力演进）。

### 修改的能力

- **desktop-change-flow-view** — 节点模型三分类收敛两分类（interrupted 下线）；右侧抽屉单列改双列（左列会话信息 + 右列三分节）；节点会话转录联动补 active 节点三 role 反查、下线「查看会话」显式入口；时间序边推导 / 三代际降级 / 运行态并存描述 / 转换层直测边界随动。

---

## 变更范围

### 实现文件

- `packages/desktop/src/views/changes/flow/detail-drawer.tsx` — 两列布局（`w-[560px]` → 约 960px、左列 60%、双列各自滚动）；`selectionRoleRefs` 补 `kind === 'active'` 三 role 反查分支
- `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` — 左列元信息呈现（session id、运行状态徽章等，字段清单 design 定夺）；三 role tab 结构不变
- `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` — 返回值暴露 `SessionSummary` 面（transcript / running 之外）
- `packages/desktop/src/views/changes/flow/run-step-node.tsx` — 「查看会话」按钮与 `onOpenSession` prop 删
- `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` — `onOpenSession` 接线删
- `packages/desktop/src/views/changes/flow/types.ts` — `InterruptedFlowNode` 删；`FlowNodeKind` 收敛 `'eval' | 'active'`
- `packages/desktop/src/views/changes/flow/graph.ts` — `collectInterrupted` 删；`mergeByStartAt` 归并只剩 active 单节点
- `packages/desktop/src/views/changes/flow/flow-event-node.tsx` — `InterruptedBody` / dashed 分支删
- `packages/desktop/src/views/changes/flow/attachments.ts` — `NODE_PRECEDENCE` 三段收敛 `['eval', 'active']` 两段
- `packages/desktop/src/types/generated/bindings.ts` — 再生成（`ChangeDetail.interrupted` / `InterruptedEntry` 消失，`dto.ts` 的 `export type *` 自动跟随）
- `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` — `InterruptedEntry` + `Workflow.interrupted` 字段删；`ActivePhase` 注释中「InterruptedEntry 加 end_at 的扩展」自述联动清理
- `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file.rs` — `interrupted` 提取与单条容错分支删（宽松解析语义不变，字段回落未知字段忽略）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `ChangeDetail.interrupted` 与线面 `InterruptedEntry` 删
- `packages/desktop/src-tauri/crates/core/workflow/tests/golden/*.json` — detail 线面 golden 显式重写（v2-b 含实数据；沿 golden 契约冻结的显式重写流程）
- `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` — `interruptedCount` 投影删，golden 随动再生
- `packages/desktop/package.json` — `version` 0.4.6 → 0.4.7（**归档时执行**，非实现项）

### 测试文件

- `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` — 双列布局断言、active 三 role 反查新用例；`interrupted: []` fixture 字段与 interrupted 选中用例删
- `packages/desktop/src/views/changes/flow/graph.test.ts`、`layout.test.ts`、`attachments.test.ts`、`flow-event-node.test.tsx`、`change-flow-graph.test.tsx`、`change-detail-view.test.tsx`、`app.test.tsx` — interrupted 用例随动删除
- `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx` — 元信息呈现用例
- `packages/desktop/src-tauri/crates/core/workflow/src/parse/workflow_file_test.rs` — interrupted 条目用例删；补含 `interrupted[]` 存量文件照常解析（未知字段忽略）用例
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs` — `detail.interrupted` 断言删
- `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs` — `detail.interrupted` 断言随动

### 删除文件

- 无（均为文件内删减，无整文件退役）。

### 不要修改

- 磁盘形状与写入方：`workflow.json` 的 `interrupted[]` 字段原样保留（serde 未知字段忽略）；TS schema（`plugins/dev-team/bin/src/schemas/workflow.schema.ts`）不动；插件 CLI 写入方不动；`write/persist.rs` 五类写触点不动（interrupted 零写触点）
- 图结构不变量：`PIPELINE_PHASES` 9 列布局、时间序边推导公式、attempt 缺号兜底、空列恒定、stale 淡化
- 既有会话联动语义：id 直查优先、sourceRef 反查兜底、seq 归并、running 推导、`AgentTimeline` 复用
- 取数模型：显式刷新 + 运行状态 Channel 例外口径不变；`get_change_detail` 契约除 interrupted 字段外不动
- golden 线面契约其余字段：=null + ISO 串口径、DTO 纯 derive 零字段属性不变（仅 interrupted 字段按显式重写流程移除）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 抽屉两列布局（detail-drawer.tsx） | 打开任意节点/列头抽屉：左列为会话信息区（约 60% 宽，元信息 + 转录拉满滚动），右列为【本站文档 / 评估记录 / 文件清单】三分节列内滚动；抽屉宽约 960px 且 `max-w-[85vw]`；列头与 ToolStep/Gate 选中左列呈空态占位，双列结构不跳变（detail-drawer.test.tsx 覆盖） |
| AC-2 | active 节点三会话联动（selectionRoleRefs 补分支） | 点击 active 节点：呈 executor / evaluator / decision 三转录 tab，sessionId=null、sourceRef=`<change>/<phase>/<role>/<attempt>` 反查；已开跑角色重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不发起实时事件流订阅（detail-drawer.test.tsx 覆盖） |
| AC-3 | 左列会话元信息（use-session-transcript summary 面） | `useSessionTranscript` 暴露 `SessionSummary`（row + stats + turns）；左列呈现 session id 与运行状态徽章等元信息（具体字段清单 design 定夺）（session-transcript-panel.test.tsx 覆盖） |
| AC-4 | 「查看会话」按钮全下线（run-step-node.tsx / change-flow-graph.tsx） | WorkerAgent 节点（运行中或已收口）上无「查看会话」按钮，`onOpenSession` prop 与接线删除；点击节点本体即打开抽屉（change-flow-graph.test.tsx / run-step-node.test.tsx 覆盖） |
| AC-5 | 前端 interrupted 词汇清除（types / graph / flow-event-node / attachments） | `FlowNodeKind = 'eval' | 'active'`；`collectInterrupted` / `InterruptedBody` / dashed 分支删；`NODE_PRECEDENCE` 收敛两段；含 `interrupted[]` 留档的存量 change 详情图上无 interrupted 节点、无编译残留（graph.test.ts 等随动全绿） |
| AC-6 | Rust 读模型停解析（model / parse / detail） | `Workflow.interrupted`、`ChangeDetail.interrupted`、线面 `InterruptedEntry` 删除；含 `interrupted[]` 的存量 workflow.json 照常解析不报错（未知字段忽略）；`queries::detail` 与 parse 测试随动全绿 |
| AC-7 | golden 显式重写与 bindings 再生 | `tests/golden/*.json` detail 线面按显式重写流程更新（v2-b 无 interrupted 字段）；`corpus_golden_test` 的 `interruptedCount` 投影删且 golden 再生；`bindings.ts` 再生成后 `ChangeDetail.interrupted` / `InterruptedEntry` 消失，tsc 全量类型检查拦截前端消费漂移 |
| AC-8 | 全管线通过 | `pnpm -C packages/desktop run client:check`（fmt / lint / knip）与 `pnpm -C packages/desktop run test` 全绿；`cargo test --workspace`（src-tauri 根）全绿；无新增豁免条目 |
| AC-9 | desktop 版本升级（归档时执行） | `packages/desktop/package.json` `version` 0.4.6 → 0.4.7；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| interrupted 留档信息（start/end 时间）在 UI 完全不可达 | 中断留档检索能力丧失；UI 区分"中断"与"未收口"仅剩残留 active_phase 节点 | 确定（有意识的简化） | 已在探索阶段拍板接受；磁盘数据原样保留，如需恢复走读模型加字段的反向变更即可 |
| DTO 字段删除触 golden 线面契约（冻结口径） | detail 线面消费方漂移 | 低 | 沿既定 golden 显式重写流程（golden 重写 + bindings 再生）；tsc 全量类型检查拦截前端漂移；字段演进之外形态仍冻结 |
| active 节点转录为查询时快照：进行中会话只含已流出部分、运行徽章不自动翻绿 | 用户看到的是"截至打开抽屉"的转录，需重开抽屉/切 tab 才见新内容 | 中 | design 阶段定夺（可接受 / 做刷新）；实时流本就由 runtime overlay 节点承担，CLI/外部 run 场景反查重放已覆盖主要诉求 |
| 存量 fixture 与真实数据含 `interrupted[]`，读模型删字段后解析路径变化 | 解析降级、golden 失配 | 低 | serde 未知字段忽略是既有语义（desktop-change-parse 已约束）；parse 测试补存量文件照常解析用例 |
| 两列布局窄屏挤压（85vw 截断时列宽下限） | 小窗口下左列/右列过窄不可读 | 中 | design 阶段定夺列宽下限或堆叠降级策略 |
| 删按钮损失 WorkerAgent 显式入口的既定交互（上一变更刚交付） | 用户习惯回退 | 低 | 两列布局后点击节点即见转录，功能等价且少一步；explore 决策 5 已拍板 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 无会话选中时左列形态 | 空态占位，双列结构恒定不跳变 | 布局稳定性优先，选中切换无结构跳变 | 左列整体隐藏（结构跳变） |
| interrupted 处置深度 | 读模型停解析、UI 信息面整体下线（深于"仅前端隐藏"） | 与 active 区分度本就低；留读模型徒保全链维护成本；写面零触点（desktop 从不写 interrupted）保证零数据丢失 | 仅前端隐藏（读模型保留） |
| 抽屉尺寸与列配比 | 加宽约 960px，左列（会话）占 60%，保留 `max-w-[85vw]` | 转录为主要信息增量，左列拿大头；85vw 兜底窄屏 | 维持 560px 单列（转录垫底问题不解决） |
| 左列内容构成 | 会话元信息（id、状态徽章等）+ 转录拉满高度滚动 | 元信息回答"这是谁的会话"，转录拉满解决 480px 限高问题 | 仅转录（无元信息） |
| 「查看会话」按钮 | 全下线：active 不加、WorkerAgent 现有入口一并删 | 两列后点节点即见左列转录，按钮冗余 | 保留 WorkerAgent 入口（按钮与左列并存冗余） |
| active 节点会话寻址 | 复用 sourceRef 反查（sessionId 恒 null），eval 同款三 role | `ActivePhase.attempt` 非空保证定式可组装；建档即落库 + sealed 流式 append 使进行中内容反查可见；零后端改动 | 扩展 active_phase 携带会话槽位（需写面改动，不值） |

### 待决问题

- running 徽章时效：左列元信息来自查询时快照，会话收口后徽章不自动翻绿（直到重开抽屉 / 切 tab / run 终态刷新）——可接受还是要做刷新，design 定。
- 元信息字段清单：id 截断展示与否、轮数 / token 统计（`SessionStats`）是否呈现，design 定。
- 左列空态文案措辞，design 定。
- 窄屏保护：85vw 截断时左/右列宽下限（或堆叠降级），design 定。
