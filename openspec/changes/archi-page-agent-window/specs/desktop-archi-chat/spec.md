# desktop-archi-chat Specification

## Purpose

定义架构页（`/architecture`，挂载点 A）左栏**通用 Agent 会话窗口**的能力边界：面板骨架复用（共享 `SessionPanel`，见 `desktop-change-chat`「面板骨架归一」）与左栏分栏折叠形态、会话归属（`source = "archi"` / `source_ref = null` / `cwd = null` 回落主仓根）与专用清单薄命令、禁发矩阵（仅本会话运行态；架构页无归档态与 flow run 判据）与状态行两态、以及「写权在会话、画布仍只读」的落点纪律与手动刷新语义。工具面与知识面不在本能力内：注入装配见 `desktop-archi-agent`，系统消息合并在 `desktop-agent-execution`。

## ADDED Requirements

### Requirement: 架构页左栏通用 Agent 面板（骨架复用与布局）

`/architecture` SHALL 在**左栏**提供常驻的通用 Agent 面板，与右侧画布并排（`ResizablePanelGroup orientation="horizontal"`），MUST NOT 以右侧抽屉 / 浮层承载对话：

- **布局与折叠**：左栏 SHALL 默认收起（挂载即窄条形态，仍留可见展开入口），页面头 SHALL 提供面板显隐开关（收起 / 展开同一入口）；右栏 SHALL 保持「标题 + 刷新按钮 + 画布」既有次序，画布 SHALL 可在左栏收起时全宽使用。
- **折叠真相**：架构页的折叠态 SHALL 由页面开关 state 持有，MUST NOT 依赖分栏 `onResize` 的像素派生（与 change 详情左栏的折叠真相来源不同，属刻意差异：本页折叠只由显式开关驱动）。
- **骨架复用**：面板 SHALL 经共享骨架组件（`SessionPanel`，`testIdPrefix = "archi-chat"`）渲染头区动作（新会话 / 历史会话）、状态行、历史清单与折叠窄条，MUST NOT 复制第二套会话面板实现；对话体 SHALL 经 `AgentSessionView` 的**注入形态**渲染（来源侧建 `useAgentSession`），composer 经插槽注入，MUST NOT 在视图层自拼消息 / 轮行 / 事件流。
- **刷新链路**：页面刷新入口 SHALL 保留既有 `archi-refresh` 语义（按钮触发 `refreshTick` → 画布以 `key` 重挂 → 重新取数）；本能力 MUST NOT 引入自动刷新、目录监听或轮询。
- **零取数纪律**：面板收起（窄条）时 MUST NOT 发起任何会话 / 清单取数；面板挂载但处于空态时 MUST NOT 发起重放查询与订阅。

#### Scenario: 默认收起与展开

- **WHEN** 打开 `/architecture`
- **THEN** 左栏呈窄条（竖向标题 + 展开入口），右栏标题 / 刷新按钮 / 画布齐备且画布占满宽度；点击页面头开关或窄条展开入口后左栏展开为完整面板，再点可收起

#### Scenario: 骨架复用的可观测证据

- **WHEN** 检索架构页面板的实现
- **THEN** 头区动作 / 状态行 / 历史清单 / 折叠窄条经共享骨架组件渲染，`data-testid` 前缀为 `archi-chat`（`archi-chat-panel` / `-new` / `-history` / `-note` / `-history-list` / `-rail` / `-expand` / `-session` 等在案）；对话与轮行经 `AgentSessionView` 注入形态渲染，面板内无自有转录查询 hook

#### Scenario: 收起时不取数

- **WHEN** 页面以默认收起态挂载并保持无交互
- **THEN** 零会话类 invoke（无 `list_archi_chat_sessions`、无 `session_detail`、无 `agent_session_watch`）；展开面板后空态亦不发起重放查询

#### Scenario: 刷新链路零变化

- **WHEN** 点击 `archi-refresh`
- **THEN** 画布以同一 `root` 重发恰一次取数（`key` 重挂语义不变）；无自动刷新、无定时轮询介入

### Requirement: 会话归属与新会话 / 历史会话

面板的会话 SHALL 以 `source = "archi"`、`source_ref = null` 写入会话归属；清单读取 SHALL 经专用薄命令，选中后 SHALL 一律以 `sessionId` 直查重放：

- **写侧归属**：发起 SHALL 携带 `source = "archi"` 与 `source_ref = null`（一个 workspace 一份历史，不挂 change / explore 身份）；`cwd` SHALL 传 `null` 使后端回落 workspace root（主仓根），MUST NOT 传 change worktree 一类的替代根。
- **读侧清单**：命令面 SHALL 提供 `list_archi_chat_sessions(root, offset, limit) -> SessionPage` 薄包装（等价 `session_query(source = "archi")` 的分页清单，`source` 词常量收在命令层）；blank root SHALL 返回空页信封；面板 MUST NOT 自拼 `source` / `source_ref` 字符串，MUST NOT 直调通用清单命令（`agent_sessions`）以隐式过滤。
- **空态与可发送性**：打开面板 SHALL 为**空态**（`sessionId = null`：零重放取数、零订阅、零会话行），MUST NOT 打开即自动恢复最近一条历史会话；该空态 SHALL 是可发送的草案态（首发 `sessionId` 仍为 null → 后端 `New` 语义铸行）。
- **新会话与历史会话**：「新会话」SHALL 将 `sessionId` 置 null 并清空对话本地镜像（会话行不预建）；「历史会话」清单 SHALL 分页呈该归属会话（`updated_at` 降序由查询面保证），点选 SHALL 以 `sessionId` 直查 `session_detail` 与会话转录重放装载；面板 SHALL 在会话 id 确立或首轮收口时重取清单，使新铸会话可点选重开。
- **不新增模型字段**：架构页会话引用 MUST NOT 落库为 workspace / change 模型字段（零 native model 升级、零 store 迁移）；「当前会话」为前端面板态。

#### Scenario: 归属写入可查且互不相撞

- **WHEN** 在架构页面板发出首轮对话并收口
- **THEN** 该会话在库内 `source = "archi"`、`source_ref = null`；`list_archi_chat_sessions` 返回该会话，而同 workspace 的 change chat 会话（`source = "change"`、两段 `source_ref`）与 change 流程会话（四段 `source_ref`）一条都不出现

#### Scenario: 空态首发铸会话行

- **WHEN** 面板处于空态时输入并发送首条消息
- **THEN** 发起 invoke 的 `sessionId` 为 null（`New` 语义）、`source = "archi"`、`source_ref = null`、`cwd = null`；后端铸造会话行，轮行回写后该会话在历史清单中可点选重开

#### Scenario: 新会话不堆积空行

- **WHEN** 连续点击「新会话」三次且不发任何消息
- **THEN** 库内不产生任何会话行；已开过的旧会话仍留历史清单可重开

#### Scenario: 历史点选直查重放

- **WHEN** 在历史清单中点击一条既有会话
- **THEN** 面板以该 `sessionId` 直查 `session_detail` 与转录取数重放装载（含元信息与轮行）；选中与重放链路零 `source` / `source_ref` 反查调用

#### Scenario: 不自动恢复最近一条

- **WHEN** 某 workspace 已有若干 archi 会话时打开 `/architecture` 并展开面板
- **THEN** 面板呈空态（无活跃 `sessionId`、无重放查询、无订阅），历史会话只在用户显式展开清单并点选后装载

### Requirement: 禁发矩阵与状态行

面板 SHALL 以单一判据源决定「可发送 / 可开新会话 / 可切历史」，MUST NOT 由各组件各自判定：

| 本会话库态 `running` | 可发送 | 可开新会话 | 可切历史 |
| --- | --- | --- | --- |
| true | ✗ | ✗ | ✓ |
| false | ✓ | ✓ | ✓ |

- **运行中禁发**：判据 SHALL 为本会话库态 `running`；禁发 SHALL 连「新会话」一并禁，只留历史浏览（MUST NOT 出现「能开新会话但不能发」的半可用态）。
- **无归档态、无 flow run 判据**：架构页不承载 change（无 `detail.source = "archive"` 只读态、无在飞 change flow run），MUST NOT 引入这两条判据的占位参数。
- **空态不参与判据**：`sessionId === null` MUST NOT 作为禁用理由，也 MUST NOT 豁免上表任一行；可发送行下空态照常可发。
- **禁用面**：composer SHALL 呈 `disabled` 态（输入框与发送同时禁用）+ 禁用理由文案，MUST NOT 只禁发送按钮而放过输入。
- **状态行两态**（由来源侧 hook 供 `note`，骨架只渲染）：空态 SHALL 呈草案文案（发送首条消息即开启新会话）；已开会话且无可发送性理由时 SHALL 呈刷新提醒（模型改动不会自动刷新，改完点「刷新」）；禁用理由在场时 SHALL 优先于上述两态。

#### Scenario: 运行中禁发但可读史

- **WHEN** 当前会话库态为运行中
- **THEN** composer 禁用并呈运行中理由，「新会话」禁用，历史清单展开与点选可用

#### Scenario: 空闲可正常收发

- **WHEN** 无在飞 run、当前会话不在运行
- **THEN** 输入与发送可用，「新会话」可用，历史切换可用；空态同样可发

#### Scenario: 状态行两态与优先级

- **WHEN** 分别观察空态面板、已开会话（空闲）、运行中会话
- **THEN** 依次呈「发送首条消息即开启新会话」、「模型改动不会自动刷新，改完点「刷新」」、运行中理由；无理由且非空态时不占位

### Requirement: 写权落点与刷新纪律

架构页会话的工具面 SHALL 为七通用工具（含 `write` / `edit` / `bash`）叠加两 archi 只读工具（注入矩阵与工具面见 `desktop-archi-agent`）：**写权落在会话里，画布仍零写入口**。

- **画布不参与**：`mode = "full"` 的画布 MUST NOT 因本能力新增任何可提交的 `gate` 编辑入口或写命令（见 `desktop-archi-canvas`「能力差异」的消歧条款）；MUST NOT 以「会话能写」为理由放宽画布，也 MUST NOT 以「画布只读」为理由禁会话写。
- **直改主仓**：agent 写模型 SHALL 直改主仓 `<archi_dir>/models/*.toml`（无 worktree、无归档确认回合、无专用写工具，仍用 `write` / `edit`）；写入不校验、读取报错（既有纪律见 `desktop-archi-agent`「模型 = 源码」）。
- **可见性**：agent 写盘后画布 MUST NOT 自动更新（零 watch 零轮询的必然推论）；用户点 `archi-refresh` 后 SHALL 见到新图，模型被写坏时 SHALL 见到停摆态诊断（`file:line` 逐条，与 agent 读取面同一份 `archi::dto` 投影）。该语义 SHALL 在面板状态行提示，MUST NOT 被当作缺陷处理。
- **零自动提交**：会话 MUST NOT 在轮收口或写盘后自动提交 / 自动归档 / 自动 `git add`；改动一律经用户 `git diff` 评审后自行处置。
- **档位**：档位沿用会话档位选择器（缺省 `bypassPermissions`，与既有 `AgentInput` 一致）；`default` 档下 `write` / `edit` / `bash` 仍被静态权限表拒绝（三档语义零改动），用户可据此把面板当只读顾问使用。

#### Scenario: 直改主仓模型成功

- **WHEN** 在架构面板会话中要求 agent 新增一个元素并写入 `<archi_dir>/models/09-billing.toml`
- **THEN** 写入落在主仓该文件（工作区其他会话随即可读），会话行与事件转录正常收口；画布在用户点刷新前保持旧图

#### Scenario: 写后刷新才可见

- **WHEN** agent 已写盘但用户未点 `archi-refresh`
- **THEN** 画布不变化（无自动重取）；点刷新后呈现新图；若模型被写坏则呈 `StructureErrorPanel` 的逐条诊断

#### Scenario: 画布仍只读

- **WHEN** 在 `/architecture` 审查元素抽屉与页面源码
- **THEN** 无任何可提交的 `gate` 编辑控件、零写命令调用；面板会话的存在不改变该断言

#### Scenario: 零自动提交

- **WHEN** 会话收口（`running` 由 true 变 false）
- **THEN** 无自动 `git add` / `commit` / 归档动作；工作区改动保持未提交状态等待用户评审

## Module Contract

### Component: `packages/desktop/src/views/architecture/architecture-view.tsx`（改）

| 项 | 变化 | 描述 |
|---|---|---|
| `ArchitectureView` | 单列 → 横向分栏 | `ResizablePanelGroup orientation="horizontal"`：左栏 `ResizablePanel`（`collapsible`、默认收起、留窄条）+ 右栏（标题 + 面板开关 + `archi-refresh` + 画布 `key={refreshTick}`）；折叠真相为页面开关 state（不取 `onResize` 像素派生）；`root` 入参与 `mode = "full"` 装配不变 |

### Component: `packages/desktop/src/views/architecture/agent/`（新增）

| 组件 | Props | 描述 |
|------|-------|------|
| `ArchiChatPanel` | `{ root: string \| null; collapsed: boolean; onToggleCollapsed: () => void }` | 面板薄封装：`useArchiChat` 状态面 + 共享骨架 `SessionPanel`（`testIdPrefix = "archi-chat"`）+ composer 插槽；归属参数（`source = "archi"` / `sourceRef = null` / `cwd = null`）由 hook 承担 |
| `ArchiChatComposer` | `{ running, disabled, disabledReason, onSend, onStop }` | `AgentInput` 薄适配（`idPrefix = "archi-chat"`；label / placeholder 文案留本文件，档位 / 发送 / 停止收口在组件族） |

### Function: `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts`（新增）

| 项 | 签名 | 描述 |
|------|------|------|
| `useArchiChat` | `(params: { root: string \| null }) => ArchiChatState` | 会话面（`useAgentSession` 注入：`source = "archi"` / `sourceRef = null` / `cwd = null`）+ 清单取数（`list_archi_chat_sessions` 分页，会话 id 确立 / 轮收口后重取）+ 新会话（置 null + `reset()`）+ 历史点选（`setSessionId`）+ 运行中禁发判据（`canSend` / `canStart`）+ 状态行 `note`（草案文案 / 刷新提醒 / 禁用理由优先） |

### API: `packages/desktop/src-tauri/src/commands/exec/mod.rs`（命令面随动）

| 命令 | 签名 | 描述 |
|------|------|------|
| `list_archi_chat_sessions` | `(root: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | 薄包装：`session_query` 分页清单，`source` 常量 `"archi"`（`source_ref` 不过滤）；blank root → 空页 |
