# desktop-change-flow-view Delta

## RENAMED Requirements

- FROM: `### Requirement: 流程图节点模型与三分类`
- TO: `### Requirement: 流程图节点模型与两分类`

## MODIFIED Requirements

### Requirement: 流程图节点模型与两分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点粒度 SHALL 为 attempt 级。转换层 SHALL 将 `ChangeDetail` 的事件归并为两类节点：

- **eval 节点**：来自 `pipeline` 各站的 `attempts[]`（verdict / checklist / report / skipped / stale），实心呈现——pass 绿、fail 红；`stale: true` 的节点 SHALL 半透明淡化（废弃支线）；
- **active 节点**：来自 `activePhase`（运行中，无 verdict），pulse 运行中视觉，位于所在列的接流末端。

`interrupted[]` 留档 SHALL 不再上图：读模型（Rust `Workflow` / `ChangeDetail`）MUST NOT 解析该字段，前端节点模型 MUST NOT 保留 interrupted 词汇（`FlowNodeKind` 收敛 `'eval' | 'active'`）；磁盘上既有 `workflow.json` 的 `interrupted[]` 数据 SHALL 原样保留（serde 未知字段忽略），MUST NOT 迁移或清除，写入方（插件 CLI）MUST NOT 因本变更改动。9 站顺序 SHALL 以 `PIPELINE_PHASES` 固定序列为准（proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze），不依赖 eval 排列。

#### Scenario: 两分类节点呈现

- **WHEN** 打开一个含多 attempt 与 backtrack 的 v2 change 详情
- **THEN** 图上呈现 eval 节点（pass 绿 / fail 红）与 pulse 的 active 节点，无 dashed 灰显的 interrupted 节点

#### Scenario: interrupted 留档不出图且数据保留

- **WHEN** 打开 workflow.json 含 `interrupted[]` 留档的存量 v2 change（v2-b 夹具形态：`interrupted` 的 test-gen#1 与 `active_phase` 的 implement#2 均不在 eval 序列内）
- **THEN** test-gen#1 不以任何节点形态出现在图上（"未收口"语义由 implement#2 的 active 节点承担），磁盘 workflow.json 的 `interrupted[]` 数据原样保留、详情查询不报错

#### Scenario: stale 淡化

- **WHEN** 某站的最新 pass eval 被 backtrack 标记 `stale: true`
- **THEN** 该节点呈半透明淡化，表示废弃支线，但仍保留在图上

### Requirement: 时间序边推导

边 SHALL 由 O(n) 扫描推导：eval 事件按追加序（即时间序）串联，active 事件按 startAt 归并插入；每个事件的入边 SHALL 指向时间上紧邻的前一事件。边的 Δx 符号 SHALL 表达类型：向右=前进、同列（Δx=0）=重试、向左=回跳；回跳边 SHALL 为虚线并以 `backtrackReason` 作边标签。attempt 缺号 SHALL 按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款），startAt 缺失的事件 SHALL 按列表序归并、不丢节点。

#### Scenario: 前进 / 重试 / 回跳三类边

- **WHEN** 执行史为 proposal#1 → dev-design#1 fail → dev-design#2 pass（回跳重做）
- **THEN** proposal#1→dev-design#1 为向右前进边，dev-design#1→dev-design#2 为同列重试边；若失败边源自跨站 backtrack（如 test-execution#1 → test-gen#2），该边呈虚线且 `backtrackReason` 可读

#### Scenario: 缺号与缺时间戳兜底

- **WHEN** 某事件 `attempt` 为 null 或 startAt 为 null
- **THEN** 节点不丢失：attempt 以 0 兜底参与列内排序，startAt 缺失者按列表序归并，转换不报错

### Requirement: 右侧抽屉单一交互入口

phase 列头与 attempt 节点 SHALL 提供同一交互入口：点击打开右侧抽屉。抽屉 SHALL 为双列布局：左列（约 60% 宽）为会话信息区——上方呈现选中节点关联会话的元信息（session id、运行状态徽章等，具体字段清单由 design 定夺，数据取自会话查询的 `SessionSummary`），下方转录 tab 与时间线拉满左列高度滚动；右列为既有三分节【本站文档 | eval report+checklist | 文件表】，各节在右列内滚动。抽屉整体宽度 SHALL 由 560px 加宽至约 960px 并保留 `max-w-[85vw]`（窄屏列宽保护由 design 定夺）；遮罩点击关闭与右贴边定位不变。无会话联动的选中（列头 / ToolStep / Gate / 无 role 运行步）左列 SHALL 呈空态占位，双列结构 MUST NOT 因选中类型跳变。本站文档节 SHALL 复用 `renderers/registry`（经 `ArtifactView` 渲染，新增产物 kind 仍只需在 registry 追加一行）；eval 节展示该 attempt 的 report 与 checklist 条目；文件表节展示挂靠该节点的 file_log 条目（op / path / at）。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

#### Scenario: 节点点击抽屉双列内容

- **WHEN** 点击 implement#2（fail → retry pass 的第二个 attempt，会话记录在场）节点
- **THEN** 左列呈该 attempt 的会话元信息与 executor / evaluator / decision 转录 tab，右列自上而下为该站文档、attempt#2 的 report 与 checklist、attempt#2 名下的 file_log 文件表

#### Scenario: 列头点击同入口左列空态

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，右列本站文档节含 design.md / tasks.md，左列呈空态占位而非整列消失

#### Scenario: 双列各自滚动

- **WHEN** 某 attempt 的会话转录、report 与文件表内容均超出可视高度
- **THEN** 左列时间线在左列内滚动、右列三分节在右列内滚动，抽屉本体 MUST NOT 退回整列单滚

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id（desktop-change-queries）在场时，转录面板 SHALL 按 session id 直查（`session_detail` + `agent_session_transcript`）；槽位缺席（旧数据）时 SHALL 回退既有 sourceRef 定式反查（`<change>/<phase>/<role>/<attempt>`，同 ref 多会话取最近一条）。eval 节点与 active 节点的抽屉联动 SHALL 均覆盖三会话：executor / evaluator / decision 三转录 tab（形态由 design 定稿）；decision 槽位缺席时该 tab SHALL 呈空态，MUST NOT 虚构会话或误挂他 attempt 的会话。active 节点 SHALL 经 sourceRef 反查联动（`active_phase` 无会话槽位，sessionId 恒 null；`ActivePhase.attempt` 非空保证定式可组装）：会话建档即落库、sealed 事件流式 append，进行中会话的已流出转录经反查重放可见；未开跑角色的 tab SHALL 呈「（暂无该会话转录）」空态；active 节点联动 MUST NOT 依赖实时事件流（实时流由 runtime WorkerAgent overlay 节点承担，本 app run 场景图上并存）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。节点 MUST NOT 提供显式「查看会话」按钮（两列布局后点击节点即见左列转录，既有 run-step-node 入口随本变更移除，active 节点不新增）。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（左列空态占位、右列展示步骤输出摘要）。

#### Scenario: 运行中节点实时转录

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 直查优先

- **WHEN** 某 attempt 记录携带会话槽位 id，且同 sourceRef 定式下存在多条会话
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义

#### Scenario: 旧数据回退反查

- **WHEN** 打开不含会话槽位的历史 attempt（旧代际数据）
- **THEN** 转录面板回退 sourceRef 定式反查并照常重放，行为与升级前一致

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 eval 节点打开抽屉，该 attempt 的 decision 槽位在场（或缺席）两种形态分别驱动
- **THEN** 抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位在场时按 id 直查其转录，缺席时 decision tab 呈空态，executor / evaluator 两 tab 不受影响

#### Scenario: active 节点三会话反查联动

- **WHEN** 打开一个残留 active_phase 的 change（如 CLI 外部跑的 run、刷新重挂或 crash 残留），点击 active 节点
- **THEN** 抽屉左列呈 executor / evaluator / decision 三转录 tab，已开跑角色经 sourceRef 反查重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不订阅实时事件流，亦不虚构未开跑角色的会话

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看左列转录；无独立会话 route 引入

### Requirement: 三代际降级

视图 SHALL 按 inventory 降级：

- **v0**（无 workflow.json）：SHALL 呈现空图占位，产物区照常呈现（图区 MUST NOT 挤掉产物区）；
- **v1**（无 file_log，`fileLog === null`）：SHALL 正常绘制流程图（eval / active 节点照常），节点文件区为空、抽屉文件表节呈空态；
- **v2**：完整图（含 file_log 挂载与 workflow 独立面板）。

`unparsable`（workflow.json 损坏）警示与 error / loading / 未找到降级页 SHALL 沿用现状。

#### Scenario: v0 纯文档形态

- **WHEN** 打开 inventory 为 v0 的 change 详情
- **THEN** 图区呈现空图占位，产物区照常展示，页面不报错

#### Scenario: v1 无文件区

- **WHEN** 打开 inventory 为 v1 的 change 详情（fileLog 为 null）
- **THEN** 流程图正常绘制各节点与边，任一节点抽屉的文件表节为空态，无 file_log 报错

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的节点 SHALL 呈现运行态（pulse 视觉，与既有 active 节点同款），ToolStep 节点（phase-start / static-check / phase-log / backtrack 相位机步）与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（类型可区分）。运行态呈现 SHALL 与既有节点模型并存：eval（实心 pass 绿 / fail 红）、stale 淡化规则不变；`PIPELINE_PHASES` 列布局、时间序边推导规则与空列恒定 MUST NOT 因运行态呈现改变。运行收口后，图 SHALL 回落到既有派生规则（重新经 `ChangeDetail` 派生），运行态视觉完全褪去。

#### Scenario: 运行态与两分类并存

- **WHEN** implement#2 相位运行中（executor 会话活跃）且历史上有 fail→retry 与 active_phase 残留
- **THEN** 图上同时呈现：历史 eval 节点（实心三色）、active_phase 的 active 节点（pulse）与当前 executor 节点（pulse 运行态）及同相位的 ToolStep / Gate 节点（类型可辨、状态如实）

#### Scenario: 收口后回落派生规则

- **WHEN** run 以全部相位 pass 收口
- **THEN** 图经 `get_change_detail` 重新派生，与插件 CLI 直跑产出的执行史图形态一致，无残留运行态视觉

### Requirement: 纯函数转换层与渲染薄层

`detail → { nodes, edges }` 转换 SHALL 为独立纯函数模块（`views/changes/` 下，目录与命名由 design 定夺），不依赖 react 宿主组件状态、不发起任何 invoke，vitest SHALL 直测其边界：前进 / 重试 / 回跳 / stale 淡化 / active 接流末端 / attempt 缺号 / 时间戳缺失 / 文档归属映射 / workflow scope 剔除。react-flow SHALL 仅作渲染薄层，其库内布局与交互语义 MUST NOT 做逐项断言（符合「不逐项验证库语义」约定）。

#### Scenario: 转换层直测

- **WHEN** 以 v2-a / v2-b 夹具形状的 `ChangeDetail` 数据（含 fail→retry、backtrack、stale）调用转换函数
- **THEN** 输出的节点坐标符合列索引 / 执行序公式，边集合与方向符合时间序推导，无需挂载 react 组件即可断言

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 双列布局改造 | 左列会话区约 60%（元信息 + 转录拉满滚动）+ 右列三分节列内滚动；约 960px + `max-w-[85vw]`；`selectionRoleRefs` 补 active 三 role 反查（sessionId 恒 null）；无会话选中左列空态占位 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 左列转录 tab 与元信息 | executor / evaluator / decision 三 tab 不变；新增会话元信息呈现（清单 design 定夺）；复用 `AgentTimeline` 无第二套时间线 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | summary 面暴露 | transcript / running 之外返回 `SessionSummary`（row + stats + turns）；id 直查优先 + sourceRef 反查兜底、seq 归并逻辑不变 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` + `change-flow-graph.tsx` | 查看会话入口下线 | `onOpenSession` prop 与按钮删除；节点点击即抽屉联动，无独立会话 route |
| `packages/desktop/src/views/changes/flow/types.ts` + `graph.ts` + `flow-event-node.tsx` + `attachments.ts` | interrupted 前端词汇清除 | `FlowNodeKind` 收敛 `'eval' | 'active'`；`collectInterrupted` / `InterruptedBody` / dashed 分支删；`NODE_PRECEDENCE` 收敛 `['eval', 'active']` |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` + `parse/workflow_file.rs` | 磁盘读模型停解析 | `InterruptedEntry` + `Workflow.interrupted` 字段删（含解析提取与单条容错分支）；serde 未知字段忽略保证存量 `interrupted[]` 照常解析；写面 `write/persist.rs` 零触点 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 详情线面收敛 | `ChangeDetail.interrupted` 与线面 `InterruptedEntry` 删；其余字段线面口径（=null + ISO 串、纯 derive 零字段属性）不变 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型再生跟随 | `ChangeDetail.interrupted` / `InterruptedEntry` 再生后消失；tsc 全量类型检查拦截前端消费漂移 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/*.json` + `tests/corpus_golden_test.rs` | golden 显式重写 | detail 线面 golden 按冻结契约的显式重写流程更新（v2-b 含实数据）；`interruptedCount` 投影随动删除 |
| 磁盘形状与写入方（不改） | 零触点 | `workflow.json` 的 `interrupted[]` 原样保留；TS `workflow.schema.ts` 与插件 CLI 写入方不动 |
| 图结构与取数模型（不改） | 零触点 | `PIPELINE_PHASES` 9 列布局、边推导公式、attempt 缺号兜底、显式刷新 + Channel 例外口径不变 |
