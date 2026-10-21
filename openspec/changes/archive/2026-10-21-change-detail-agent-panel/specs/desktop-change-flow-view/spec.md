# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 右侧抽屉单一交互入口

phase 列头与 runtime step 节点 SHALL 提供同一交互入口：点击打开右侧抽屉。抽屉 SHALL 为**单列布局**（宽度约 960px 并保留 `max-w-[85vw]`，右贴边定位不变），内容按选中类型分派：

- **列头**：该站过程文档 + eval-checklist 素材（`columnDocs`，经 `ArtifactTabs` 渲染）；
- **WorkerAgent step 节点**（`role !== null`）：单列会话转录（复用 `SessionTranscriptPanel`，按节点 `sessionId` 直查；槽位缺席时呈「此步无会话记录」空态）；
- **带 `outputRef` 的 step 节点**（static-check / test-execution）：单列 stdout 面板（新组件，见「节点点击判据与 stdout 输出面板」）。

抽屉 SHALL 删除 `DrawerEvalSection` + `InlineChecklist` 与 eval/active 分支；评估 report（`node.record.report`）MUST NOT 展示（eval 节点已退役）。点击判据 SHALL 前置：`role` 与 `outputRef` 皆 null 的 runtime 节点不可点击（不 `onSelect`）；`role !== null` 而 `sessionId === null` 的节点仍可点击，抽屉呈「此步无会话记录」空态占位（不再回退 sourceRef 反查）。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

抽屉 SHALL 为非模态形态（与左栏通用 agent 面板并读的前提，见 desktop-change-chat）：

- **无遮罩**：抽屉 SHALL 以非模态方式打开（`modal={false}`——遮罩仅在模态时渲染、非模态时容器不吃指针事件），MUST NOT 遮蔽页面其余区域；抽屉打开期间左栏通用 agent 面板的输入与发送 SHALL 保持可交互。
- **关闭入口**：非模态形态下无「点遮罩关闭」——关闭 SHALL 经页脚「关闭」按钮与 Esc；关闭仍等价于上游把选中置 null（`selection = null`），MUST NOT 引入第二套展开 / 折叠交互。
- **缺省不回归**：承载抽屉的标准组件 SHALL 以 `modal` 缺省 `true` 保持既有其它调用方行为（模态 + 遮罩）不变，仅 change 详情页的抽屉传 `modal={false}`。
- **上游不动**：抽屉仍由选中态驱动（`selection !== null`）、仍是列头与节点的唯一交互入口，内容分派（会话转录 / stdout / 列头素材）与宽度不变。
- **窄窗口取舍**：960px + `max-w-[85vw]` 在窄窗口仍可能盖住左栏 chat——此为既定取舍（需要并读时收起抽屉），MUST NOT 以「抽屉改为内嵌分栏」解决。

#### Scenario: 节点点击抽屉双列内容

- **WHEN** 点击 implement#2（fail → retry pass 的第二个 attempt，会话记录在场）节点
- **THEN** 左列呈该 attempt 的会话元信息与 executor / evaluator / decision 转录 tab，右列自上而下为该站文档、attempt#2 的 report 与 checklist，无文件表节

#### Scenario: 列头点击同入口左列空态

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，右列本站文档节含 design.md / tasks.md，左列呈空态占位而非整列消失

#### Scenario: 双列各自滚动

- **WHEN** 某 attempt 的会话转录、report 与 checklist 内容均超出可视高度
- **THEN** 左列时间线在左列内滚动、右列二分节在右列内滚动，抽屉本体 MUST NOT 退回整列单滚

#### Scenario: WorkerAgent 节点点击单列转录

- **WHEN** 点击 executor / evaluator / decision / specSync / resolveConflicts step 节点（`role !== null`）
- **THEN** 抽屉单列呈该会话转录（`SessionTranscriptPanel`），无「本站文档」节、无「评估记录」节

#### Scenario: 带输出引用节点点击单列 stdout

- **WHEN** 点击 static-check 或 test-execution step 节点（`outputRef !== null`）
- **THEN** 抽屉单列呈 stdout 面板（经 `read_exec_output` 渲染脚本输出），无会话转录区、无文档节

#### Scenario: 列头点击同入口

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，单列呈该站过程文档（design.md / tasks.md）与 eval-checklist 素材，无会话转录区

#### Scenario: 双列结构已退役

- **WHEN** 打开任一节点 / 列头抽屉
- **THEN** 抽屉为单列（无左列会话区 + 右列文档区的双列切分），内容随选中类型分派，抽屉本体 MUST NOT 退回双列布局

#### Scenario: 非模态抽屉与左栏 chat 并读

- **WHEN** 打开某 WorkerAgent 节点抽屉（左侧通用 agent 面板正处于可发送态）后，在左栏输入并发送一条消息
- **THEN** 页面无遮罩节点（抽屉容器非模态）、左栏输入与发送全程可交互并正常发出；抽屉内容随选中类型照常呈现

#### Scenario: 非模态关闭入口

- **WHEN** 非模态抽屉打开时按 Esc 或点击页脚「关闭」按钮
- **THEN** 抽屉关闭（选中置 null），页面回到仅有分栏的形态；MUST NOT 需要点击遮罩（非模态下不存在遮罩）

#### Scenario: 其它调用方仍模态

- **WHEN** 审查标准抽屉组件的其它使用方（未显式传 `modal`）
- **THEN** 其行为保持模态（渲染遮罩、点遮罩关闭），不因 change 详情页的改动而变更

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/components/standard/drawer.tsx` | 模态开关透传 | 增 `modal?: boolean`（缺省 `true`，透传至底层 `Drawer`）；既有调用方零回归 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 非模态打开 | 传 `modal={false}`（`data-modal="false"`、无遮罩节点）；选中驱动、内容分派、单列宽度 `w-[960px] max-w-[85vw]` 与既有断言保持 |
| `packages/desktop/src/components/ui/drawer.tsx` | 零改动（能力已在位） | `modal === true` 才渲染 `DrawerOverlay`；非模态时 Viewport `pointer-events-none`、Popup 自身 `pointer-events-auto`（左侧 chat 可交互的机制来源） |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 分栏宿主席位 | 左栏通用 agent 面板（见 desktop-change-chat）+ 右栏详情；`DetailContent` 的抽屉装配与 `selection` 选中态语义不变 |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 零改动 | 只读会话形态与 `refreshKey`（会话状态重评估）语义不变；运行中仍经会话订阅面流式 |
