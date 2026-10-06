# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 三代际降级

视图 SHALL 按建档状态两态降级（inventory 代际词汇退役）：

- **已建档**（db `ChangeRecord` 在案）：SHALL 呈现完整状态面——流程图（eval / active 节点照常）、抽屉与产物区照常；
- **文档形态**（db 缺记录，存量 CLI change）：SHALL 呈现空图占位，产物区照常呈现（图区 MUST NOT 挤掉产物区），无状态面区块、无任何「无法解析」警示。

inventory 代际徽章（V0 / V1 / V2）与 `unparsable`（workflow.json 损坏）警示 SHALL 退役：清单与详情的前端词汇清零（组件、DTO 消费与测试断言一并移除）；error / loading / 未找到降级页 SHALL 沿用现状。

#### Scenario: 建档 change 完整状态面

- **WHEN** 打开一个 db 已建档、含多 attempt 与 backtrack 的 change 详情
- **THEN** 流程图正常绘制 eval / active 节点，无代际徽章出现，抽屉与产物区照常

#### Scenario: 文档形态空图占位

- **WHEN** 打开一个无 db 记录的存量 CLI change 详情
- **THEN** 图区呈现空图占位，产物区照常展示，无「无法解析」警示，页面不报错

#### Scenario: 代际与警示词汇清零

- **WHEN** 扫描清单 / 详情视图源码与测试对 `inventory` / `unparsable` / 「无法解析」的消费
- **THEN** 零消费残留；knip 无新增豁免，全量类型检查绿

### Requirement: 过程素材挂载规则

素材挂载分两层：**文档挂列、记录挂节点**。

- 过程文档（`artifacts` 中 `markdown-doc` / `tasks-progress` 类）SHALL 经前端静态映射表归属到 phase 列（列头徽章，每站一份、不随 attempt 重复）：`proposal.md` 与 `specs/**` → proposal、`design.md` 与 `tasks.md` → dev-design、`test-reports/**` → test-execution；映射表外的文档 SHALL 落入「未归属」处理（design 定夺展示位置），MUST NOT 误挂到错误站。`explore.md` MUST NOT 入图（探索在流程之外，不属于任何站）。
- eval-checklist 信封（payload 自带 `phase` + `attempt`）SHALL 挂到对应 attempt 节点。

file_log 挂载 SHALL 退役：file_log 条目挂节点规则与 `scope='workflow'` 图外独立面板随载体退役一并删除（前端 file-log 面板组件与挂载分支移除）。

归属 SHALL 全部在前端派生，后端 `ChangeDetail` DTO 与 `PIPELINE_PHASES` MUST NOT 因本能力新增 file_log 相关字段。

#### Scenario: 文档按映射表归列

- **WHEN** 某 change 的产物含 `proposal.md`、`design.md`、`tasks.md` 与 `test-reports/xxx.md`
- **THEN** proposal 列头徽章含 proposal.md，dev-design 列头徽章含 design.md 与 tasks.md，test-execution 列头徽章含该 test-reports 文档

#### Scenario: explore.md 隐身与文件面板退役

- **WHEN** 打开任一 change 详情，产物清单含 `explore.md`
- **THEN** explore.md 不出现在图中，图上无文件徽章挂靠（节点文件区概念已退役），无 workflow 独立面板渲染

### Requirement: 右侧抽屉单一交互入口

phase 列头与 attempt 节点 SHALL 提供同一交互入口：点击打开右侧抽屉。抽屉 SHALL 为双列布局：左列（约 60% 宽）为会话信息区——上方呈现选中节点关联会话的元信息（session id、运行状态徽章等，具体字段清单由 design 定夺，数据取自会话查询的 `SessionSummary`），下方转录 tab 与时间线拉满左列高度滚动；右列为二分节【本站文档 | eval report+checklist】（原第三分节「文件表」随 file_log 退役删除），各节在右列内滚动。抽屉整体宽度 SHALL 保持约 960px 并保留 `max-w-[85vw]`；遮罩点击关闭与右贴边定位不变。无会话联动的选中（列头 / ToolStep / Gate / 无 role 运行步）左列 SHALL 呈空态占位，双列结构 MUST NOT 因选中类型跳变。本站文档节 SHALL 复用 `renderers/registry`（经 `ArtifactView` 渲染，新增产物 kind 仍只需在 registry 追加一行）；eval 节展示该 attempt 的 report 与 checklist 条目。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

#### Scenario: 节点点击抽屉双列内容

- **WHEN** 点击 implement#2（fail → retry pass 的第二个 attempt，会话记录在场）节点
- **THEN** 左列呈该 attempt 的会话元信息与 executor / evaluator / decision 转录 tab，右列自上而下为该站文档、attempt#2 的 report 与 checklist，无文件表节

#### Scenario: 列头点击同入口左列空态

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，右列本站文档节含 design.md / tasks.md，左列呈空态占位而非整列消失

#### Scenario: 双列各自滚动

- **WHEN** 某 attempt 的会话转录、report 与 checklist 内容均超出可视高度
- **THEN** 左列时间线在左列内滚动、右列二分节在右列内滚动，抽屉本体 MUST NOT 退回整列单滚

### Requirement: 节点会话转录联动

WorkerAgent 节点（executor / evaluator / decision）SHALL 提供到对应会话转录的联动：点击运行中的 WorkerAgent 节点 SHALL 打开该会话的实时时间线（delta 流、可停止）；点击已收口的 WorkerAgent 节点 SHALL 打开该会话的转录重放（不要求运行进程存活）。会话寻址 SHALL 记录 id 直查优先：attempt 记录暴露的会话槽位 id（desktop-change-queries，自 PhaseRecord 三槽位列直读）在场时，转录面板 SHALL 按 session id 直查（`session_detail` + `agent_session_transcript`）；槽位缺席（缺省落账，槽位列 None）时 SHALL 回退既有 sourceRef 定式反查（`<change>/<phase>/<role>/<attempt>`，同 ref 多会话取最近一条）。eval 节点与 active 节点的抽屉联动 SHALL 均覆盖三会话：executor / evaluator / decision 三转录 tab（形态由 design 定稿）；decision 槽位缺席时该 tab SHALL 呈空态，MUST NOT 虚构会话或误挂他 attempt 的会话。active 节点 SHALL 经 sourceRef 反查联动（`active_phase` 无会话槽位，sessionId 恒 null；`ActivePhase.attempt` 非空保证定式可组装）：会话建档即落库、sealed 事件流式 append，进行中会话的已流出转录经反查重放可见；未开跑角色的 tab SHALL 呈「（暂无该会话转录）」空态；active 节点联动 MUST NOT 依赖实时事件流（实时流由 runtime WorkerAgent overlay 节点承担，本 app run 场景图上并存）。转录呈现 SHALL 复用既有会话基建（`AgentTimeline` / 会话重放，desktop-agent-chat-infra），MUST NOT 为 change 场景另建第二套时间线组件。节点 MUST NOT 提供显式「查看会话」按钮（两列布局后点击节点即见左列转录，active 节点不新增）。ToolStep / Gate 节点 SHALL 沿用右侧抽屉单交互入口呈现步骤结果（左列空态占位、右列展示步骤输出摘要；审计全量见 desktop-change-state-store 步骤审计查询）。

#### Scenario: 运行中节点实时转录

- **WHEN** executor 会话运行中点击其 WorkerAgent 节点
- **THEN** 打开该会话实时时间线（token 级流式、tool_use / tool_result 成对、可停止），与 Agent 调试页同款组件

#### Scenario: 收口节点重放一致

- **WHEN** run 收口后点击历史某 attempt 的 evaluator 节点
- **THEN** 经会话查询重放完整转录，与运行时实时呈现一致，不依赖原进程存活

#### Scenario: 记录 id 直查优先

- **WHEN** 某 attempt 记录携带会话槽位 id，且同 sourceRef 定式下存在多条会话
- **THEN** 转录面板按槽位 id 精确直查该会话，MUST NOT 落入「同 ref 多会话取最近一条」的反查歧义

#### Scenario: 槽位缺席回退反查

- **WHEN** 打开槽位列 None 的 attempt 记录（缺省落账）
- **THEN** 转录面板回退 sourceRef 定式反查并照常重放，行为与直查路径一致可靠

#### Scenario: decision 第三 tab 与空态

- **WHEN** 点击某 eval 节点打开抽屉，该 attempt 的 decision 槽位在场（或缺席）两种形态分别驱动
- **THEN** 抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位在场时按 id 直查其转录，缺席时 decision tab 呈空态，executor / evaluator 两 tab 不受影响

#### Scenario: active 节点三会话反查联动

- **WHEN** 打开一个残留 active_phase 的 change（如重启后遗留），点击 active 节点
- **THEN** 抽屉左列呈 executor / evaluator / decision 三转录 tab，已开跑角色经 sourceRef 反查重放已落库转录（含进行中会话已流出部分），未开跑角色呈「（暂无该会话转录）」空态；全程不订阅实时事件流，亦不虚构未开跑角色的会话

#### Scenario: 查看会话按钮不呈现

- **WHEN** WorkerAgent 节点渲染（运行中或已收口）
- **THEN** 节点上无「查看会话」按钮，点击节点本体即打开抽屉查看左列转录；无独立会话 route 引入

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/change-list-view.tsx` | 清单降级两态与词汇清零 | `InventoryBadge` 与 unparsable 标注删除；文档形态条目照常入列 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 详情退役面 | 代际徽章、损坏警示条、`WorkflowPanel`（file_log 图外面板）删除；建档两态分流 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 右列三分节 → 二分节 | 文件表节删除；左列会话区、滚动与单一交互入口不变 |
| `packages/desktop/src/views/changes/flow/file-log-table.tsx` | **（删除）** | file_log 文件表组件及测试随载体退役删除 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 挂载分支收缩 | file_log 挂节点与 scope='workflow' 分支删除；文档挂列 + checklist 挂节点保持 |
| `packages/desktop/src/views/changes/flow/graph.ts` + 节点组件（不改语义） | 转换层与渲染 | 节点两分类、9 列布局、边推导、纯函数直测全部不变（消费的 `ChangeDetail` 字段面随 queries delta 演进） |
| `packages/desktop/src/types/generated/bindings.ts`（重导） | 类型跟随 | `inventory` / `fileLog` / `unparsable` 类型消失；tsc 全量拦截前端消费漂移 |
