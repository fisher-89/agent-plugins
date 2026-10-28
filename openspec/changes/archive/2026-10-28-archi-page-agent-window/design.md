# 设计: archi-page-agent-window

> **变更**: archi-page-agent-window
> **日期**: 2026-10-27
> **基于**: `proposal.md` 与 `specs/{desktop-archi-chat,desktop-archi-agent,desktop-agent-execution,desktop-archi-model,desktop-archi-canvas,desktop-change-chat,desktop-write-protection,workspace-layout-resolution}/spec.md`

---

## 提案与规格同步状态

proposal 的范围裁决（写权 = B、知识 = 静态骨架 + 代码同源派生、面板抽骨架且 change chat 本次即换、手动刷新必需、清单命令对称）与八份 spec delta 均已定稿。本设计不重复其论证，只做四件事：把「来源臂 + 知识拼装 + 单条 System」落成逐函数可实现的结构；把 `SessionPanel` 骨架的入参面收窄到可验证的公共子集；给出架构页分栏与折叠真相的具体形态；按 AC-1…AC-15 逐条对齐落点。

起点核实（路径前缀：Rust 侧 `packages/desktop/src-tauri/`，前端侧 `packages/desktop/src/`；锚点为改动前行号）：

- **装配单点**：`crates/infra/agent/src/compose.rs:28` `fn assemble_injections(provenance: &SessionProvenance) -> Result<agent::SessionInjections, String>`（**私有**）；臂序 explore（`:29-46`，六工具 + `Explore`，`preamble: None`）→ 非 change 短路（`:47-49`）→ change 相位解析（`:50-61`，`change_phase_role` `:67`、`archi_matrix` `:83`）；调用点 `:262`；来源常量 `EXPLORE_SOURCE` `:16` / `CHANGE_SOURCE` `:20`。
- **系统消息消费点**：`crates/infra/agent/src/sdk/loop.rs:193-198`——`if let Some(preamble) = preamble::load(&turn.cwd).await { chat_history.push(Message::system(preamble)); }`（每轮重读，`LoopTurn.cwd` = 会话执行根）；`LoopTurn` `:85-118` 已携 `injections: agent::SessionInjections`（`:105`）与 `archi: Option<ArchiContext>`（`:111`）。
- **archi 工具执行根**：`crates/infra/agent/src/sdk/runner.rs:357` `mint_archi_context(injections.toolset.as_ref(), &open.ctx.workspace_root)`（`:315-320` 只认 `Some(Archi { .. })`）；`SessionCtx.workspace_root` 由命令层 `resolve_exec_root(cwd, root)` 决定（`src/commands/exec/mod.rs:41-52`、`:122-135`）⇒ **archi 会话 `cwd = null` 即回落主仓 root，工具观察根天然正确，零改动**（proposal 前提成立）。
- **词表**：`crates/core/archi/src/dto.rs`——`ElementKind::ALL` `:52`（四值）、`GateTier::ALL` `:86`（三值）+ `#[default] Off` `:67`、`ArchiDiagnosticKind` `:96-115` + `as_str` `:120-135`（九值，**无 `ALL`**）；`pub use ArchiDiagnosticKind as DiagnosticKind` `:118`。
- **路径派生**：`crates/core/foundation/src/layout/mod.rs`——常量组 `:4-34`（含 `ARCHITECTURE_DIR_NAME` `:28` / `MODELS_DIR_NAME` `:31` / `DECISIONS_DIR_NAME` `:34`）；绝对族 `archi_dir` `:126` / `archi_models_dir` `:132` / `archi_decisions_dir` `:137`；`is_archi_rel_path` `:145`；**无 repo 相对族**。
- **命令面**：`src/commands/exec/mod.rs:304-326` `list_change_chat_sessions(stores, root, id, offset, limit) -> Result<SessionPage, String>`（`CHANGE_CHAT_SOURCE` `:26`、`is_blank_root` `:29`、空页信封 `:313-318`）；`agent_sessions`（`:241-262`）为通用清单。唯一登记面 `src/commands/mod.rs:24-76` `all_commands!`（`:47` 有 `list_change_chat_sessions`）。
- **前端会话基建**：`hooks/use-agent-session.ts`——`UseAgentSessionParams { root, sessionId, source, sourceRef?, cwd?, refreshKey? }`（`sessionId = null` ⇒ 零重放取数零订阅）；`components/agent/agent-session-view.tsx` 注入形态（`session` 入参）+ `AgentSessionActions`；`components/agent/agent-input.tsx` 缺省档 `bypassPermissions`。
- **面板现状**：`views/changes/agent/change-chat-panel.tsx`（216 行：`ChatHeaderActions` `:34`、`StatusNote` `:74`、`HistoryItem` `:83`、`HistoryList` `:108`、`CollapsedRail` `:148`、`ExpandedBody` `:168`（note 派生 `:170`）、`ChangeChatPanel` `:202`）；`views/changes/agent/hooks/use-change-chat.ts:239-277` `useChangeChat`（判据 `:258-260`）。
- **架构页现状**：`views/architecture/architecture-view.tsx`:14-28（标题 + `archi-refresh` + `<ArchitectureCanvas key={refreshTick} root mode="full" />`），单列无分栏。
- **分栏先例**：`views/changes/change-detail-view.tsx:34-39`（`30 / 22 / 3rem / 48px` 常量）、`:358-429`（整宽 header + 横向 `ResizablePanelGroup`）、`:478-500` `useChatPanelState`（`panelRef` + `onResize` 折叠真相）。
- **版本面**：`packages/desktop/package.json:3` = `0.6.0`。

**四处事实校准（不影响范围裁决，但决定实现形态）**：

1. **proposal 实现文件表漏列一处机械挂载**：`packages/desktop/src-tauri/src/commands/mod.rs`——`all_commands!` 是命令的唯一登记面（`src/main.rs` 的 `generate_handler` 与 `src/bindings/mod.rs` 的 `collect_commands` 同源消费）。漏登记则新命令既不进 IPC 也不进 `bindings.ts`，AC-11 / AC-15 同时失守。本设计补入变更清单。
2. **archi 工具执行根无需改动**（见起点核实第三条）：`mint_archi_context` 已按 `SessionToolset::Archi` 铸造且 root 取 `SessionCtx.workspace_root`；`cwd = null` 回落主仓即得「观察根 = 当前工作根」。toolset 下发由本变更的 archi 臂新增，执行面零改动。
3. **四面词汇对拍的知识文本面住不进 `archi`**：`crates/core/archi/tests/vocabulary_test.rs` 是 `archi` crate 的集成测试；知识文本由 `agent-runtime`（infra）拼装，而依赖方向是单向 infra → core（`archi` 反向依赖 `agent-runtime` 会成环，含 dev-dependency 也不成立为结构）。故知识文本面的断言落在生产者 crate 一侧（`crates/infra/agent/src/archi_knowledge_test.rs`），以 `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL` 逐值 `as_str()` 在知识文本内**在场**为断言，与 `archi` 侧三面 + `ALL`-vs-`as_str` 对拍合成「四面词汇集相等」（传递闭合）。见待决问题 1。
4. **`crates/core/agent/src/runner.rs:63-70` 的 doc 注释将失真**：现写「`preamble` 位维持不消费」。proposal 把该文件列入「不要修改」，本设计按零改动执行（词汇与 serde 形态确无变化），措辞失真留待决问题 2。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | archi 臂的位置与判定 | `assemble_injections` 内 explore 臂之后、`source != CHANGE_SOURCE` 短路**之前**插入 `if provenance.source == ARCHI_SOURCE { .. }`；臂内只构造 `SessionInjections { preamble: knowledge(), tools: None, toolset: Some(Archi { module: true, search: true }) }`，**不读 `source_ref`** | 以 `source` 短路命中即天然「不碰相位解析」；排在 change 臂之前才能避免 `source_ref` 畸形误入 fail-closed。explore / change 两臂逐字不动（零回归由既有断言兜住） |
| D2 | 知识拼装形态 | `knowledge() -> Option<String>` 唯一入口 = `build(SKELETON).ok()`；`build(skeleton: &str) -> Result<String, String>` 承担两处真实校验：骨架 trim 后为空 → `Err`、骨架含 `<>` 占位符 → `Err`，通过后追加派生段 | 降级语义（`Option`）与「可触发的失败面」同时成立：`build` 有真实 `Err` 分支（不是不可能路径），compose 只把 `Option` 放进 `preamble` 位——工具面与 run 完全不受影响（D15） |
| D3 | 静态骨架 / 派生段的切分 | `archi-knowledge.md` **只写代码里没有单点的内容**（键形、内联 `metadata`、层级推导、`gate` 缺省语义、契约边界、自检、写面与可见性、历史会话续接）+ 一段示例 TOML；`kind` / `gate` / 结构诊断词 / 模型与决策目录相对路径**全由 `build` 追加**，md 内零词表枚举、零目录字面量（唯一例外：示例 TOML 为演示字段面必然写出个别 `kind` / `gate` 取值——它不是词表，正确性由 D4 的解析自证锁住，枚举仍只由 `build` 追加） | spec「用词 SHALL 从 `ALL` 常量派生、MUST NOT 手抄」+「目录路径 MUST NOT 在知识文本手写」。切分点即 drift 面：md 里出现**词表枚举**就是手抄；单个示例值不是枚举，且被自证测试挡住 |
| D4 | 示例 TOML 的自证取法 | 示例以**唯一一个** ```toml 围栏块给出；自证路径 = `archi::parse::TomlParser::parse(fence)`（`SourceParser` 公开，零 IO）+ `archi::graph::ModelGraph::build(model).diagnostics()`，两者要求皆空；示例内的 `docs` 指向虚构路径不产生诊断（`docNotFound` 属 **warning**，只在 `dto::module_detail` 投影出现，不进自证面） | 双重零诊断才能同时锁住「字段面正确」与「引用/层级自洽」（只跑 parse 会漏 `unknownReference` / `orphanHierarchy`）。避开 tempdir：不必写出模型目录即可断言 |
| D5 | 单条 System 的合成落点 | `loop.rs` 内新增 `pub(crate) fn system_prompt(knowledge: Option<&str>, agent_md: Option<String>) -> Option<String>`（纯函数），`:193-198` 改为一次调用 + 一次 `push` | 合并规则（顺序 / 空行分隔 / 单侧在场 / 皆无即不注入）收在一个可断言函数里；`preamble::load` 的 32KB 截断与每轮重读语义零改动（截断作用在 `agent_md` 自身，知识不截） |
| D6 | `DiagnosticKind::ALL` 形态 | `impl ArchiDiagnosticKind { pub const ALL: [Self; 9] = [..] }`，与 `as_str` 同文件相邻 | 与既有 `ElementKind::ALL` `[Self; 4]` / `GateTier::ALL` `[Self; 3]` 同形；`[Self; 9]` 的长度即「加变体忘补」的编译期/测试期红灯。枚举名与线值零变化（`specta` 注册名不动） |
| D7 | 相对路径族的返回形态 | 三函数返回 `String`：`archi_rel_dir()` = `format!("{DOMAIN_DIR_NAME}/{ARCHITECTURE_DIR_NAME}")`；另两者 = `format!("{}/{常量}", archi_rel_dir())` | `/` 分隔、零前导 `./`、零 IO 三项由构造天然成立；`String` 直接进 `format!` 拼接（`&'static str` 无法从常量组拼出，spec 允许形态自定）。子目录从 `archi_rel_dir` 派生 ⇒ 架构域段名不二次出现 |
| D8 | 骨架入参面（`SessionPanelState`） | 只收「会话面 + 清单面 + 唯一动作面判据 + 状态行」：`session` / `sessions` / `total` / `listOpen` / `listLoading` / `listError` / `toggleList` / `loadMore` / `newSession` / `selectSession` / `canStart` / `note`。**剔除** `canSend` / `disabledReason` / `send` / `stop` | AC-10 明写「`canSend` / `disabledReason` / 归档 / run 期判定不在骨架内」；骨架实际只消费 `canStart`（「新会话」禁用态）与 `note`。`send` / `stop` 由 `AgentSessionView` 的 `AgentSessionActions` 经 composer 插槽直供，骨架再持一份就是第二来源（哑参数）。两处消费者的状态类型以 `extends SessionPanelState` 声明（`ChangeChatState` / `ArchiChatState`），骨架契约一旦增删字段，两处 hook 编译期即红（契约可验，不靠口头纪律）；`SessionPanelState` 因此是真正被跨文件消费的类型，而非摆设导出 |
| D9 | 折叠分支与可选入参 | `const isCollapsed = collapsed === true && onToggleCollapsed !== undefined`；`isCollapsed` 为真 → 只渲染窄条（竖向 `title` + 展开按钮 `${prefix}-expand`，`title="展开{title}"`）；否则渲染展开体。不传 `onToggleCollapsed` 时**永不**呈窄条与展开入口 | spec「不传时 MUST NOT 呈折叠窄条与展开入口」；反向条件避免「收起态但没有展开入口」的死锁面板（explore 详情等非消费面零变化） |
| D10 | 两侧 `note` 与 composer 禁用口径 | change：`disabledReason ?? (currentSessionId === null ? '发送首条消息即开启新会话' : null)`（逐字平移 `ExpandedBody:170` 的派生）；archi：`disabledReason ?? (currentSessionId === null ? 草案文案 : '模型改动不会自动刷新，改完点「刷新」')`（**恒非 null**，两态 + 禁用理由优先） | change 侧行为零变化（既有「有会话且无理由时不占位」断言继续成立）；archi 侧按 spec「状态行两态」三条优先级实现，`note: string \| null` 的 `null` 分支对 archi 仅作类型对称保留（见待决问题 3）。composer 侧：change 传 `disabled={!canSend}`（业务只读态）与 `running`（运行态）两参；archi **只有一条判据**，按 spec「composer SHALL 呈 disabled 态（输入框与发送同时禁用）」传 `disabled={!canSend}` = 运行中即整面板禁用（随之 `AgentInput` 不呈停止入口，见待决问题 6） |
| D11 | 架构页折叠真相 | 页面 state `collapsed`（默认 `true`）+ `panelRef`：`toggle()` 依 state 分派 `panel.expand()` / `panel.collapse()` 并同步 state；挂载用 `useLayoutEffect` 收紧一次；**不接 `onResize`** | spec 明令「折叠态由页面开关 state 持有、MUST NOT 依赖 `onResize` 像素派生」；jsdom 无布局引擎，`onResize` 派生的折叠态不可测（R-6）。代价：用户拖拽收起后 state 不跟随（点击开关一次即重新同步），此为 spec 认可的刻意差异 |
| D12 | 架构页面头的落位 | 页面头保持**整宽、分栏之上**（现结构），内容次序 = 标题 → 面板开关（新 `archi-chat-toggle`）→ `archi-refresh`；其下才是横向分栏（左栏面板 / 右栏画布） | 现页面头本已整宽（`architecture-view.tsx:18-23`），最小改动即满足 proposal「页面头增面板显隐开关」；且左栏收起时开关仍必须可达（放进右栏会让「控制左栏的开关」住进右栏）。spec Module Contract 的「右栏（标题 + 面板开关 + 刷新 + 画布）」按「可观察次序不变 + 画布居右」兑现（见待决问题 4） |
| D13 | 命令面薄包装与来源常量 | `list_archi_chat_sessions(stores, root, offset, limit)` 与 `list_change_chat_sessions` 同式：blank root → 空页信封；`list_sessions(Some(ARCHI_CHAT_SOURCE), None, offset, limit)`（`source_ref` **不过滤**）。`ARCHI_CHAT_SOURCE = "archi"` 收命令层 | 「前端零来源词汇」纪律；`source` 字面量三处同值（`compose::ARCHI_SOURCE` / 命令层 / 前端 `use-archi-chat`），三处注释互指（沿 `EXPLORE_SOURCE` 三处先例） |
| D14 | `useArchiChat` 的复位键 | 面板本地态（`sessionId` / `listOpen`）挂 `root`：`root` 变化即复位 + 作废在途清单请求；挂载首拍为同值复位（零 invoke） | 归属是「一个 workspace 一份历史」，唯一身份源就是 `root`（change chat 的对应物是 `changeId`）。切根后不得把旧根的会话 id 带进新库 |
| D15 | 知识失效的降级范围 | 降级只作用 `preamble` 位：archi 臂永远返回 `tools = None` + `toolset = Some(Archi { module: true, search: true })`，`preamble` 单独取 `knowledge()`；臂内零 `?`、零 `Err` | spec「尽力增强、不炸 run」；把降级限制在单个字段，避免「知识拼不出 ⇒ 工具面也没了」的连带失效 |
| D16 | 已知边界（不实现） | 调试页来源筛选仍三值（不扩 `archi`）；会话收口不自动刷新画布；无 `archi_check` 工具；无 diff 工具；`AGENT.md` 之外的仓内文档通道不引入 | proposal 不做清单逐条对齐；R-7 / R-8 的可发现性由架构页自带清单与知识文本的「先取现状」条款兜住 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 注入装配（archi 臂） | `source == "archi"` 短路命中：`tools = None` + `Archi { module: true, search: true }` + `preamble = knowledge()`；不碰 `source_ref` 相位解析 | `crates/infra/agent/src/compose.rs`（改） | `agent::{SessionProvenance, SessionInjections, SessionToolset}`、`crate::archi_knowledge` | 既有组合根（纯函数臂） |
| 知识拼装 | 静态骨架 md + `archi::dto` 三个 `ALL` 词表 + `foundation::layout` 相对路径族 → 单条文本；空骨架 / 含 `<>` → `Err` → 降级 `None` | `crates/infra/agent/src/archi_knowledge.rs`（新） | `archi::dto`、`foundation::layout` | 纯函数、零 IO、`include_str!` |
| 知识静态骨架 | 键形 / 字段面 / 内联 `metadata` / 层级推导 / `gate` 缺省 / 契约边界 / 自检 / 写面与可见性 / 历史会话续接 + 一段可解析示例 TOML；零 `<>`、零目录字面量、零词表 | `crates/infra/agent/src/archi-knowledge.md`（新） | — | markdown（`include_str!` 编译期内嵌） |
| 系统消息合成 | `knowledge`（产品知识，前）+ `AGENT.md`（逐字，后）→ 恰一条 `Message::system`；单侧在场取其一；皆无则不注入 | `crates/infra/agent/src/sdk/loop.rs`（改） | 既有 `sdk::preamble` | rig `Message::system` |
| 词汇单点 | `ArchiDiagnosticKind::ALL: [Self; 9]`（与 `as_str` 同源）；既有 `ElementKind::ALL` / `GateTier::ALL` / `GateTier::default()` 复用 | `crates/core/archi/src/dto.rs`（改） | — | 关联常量 |
| 路径派生族 | `archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()`：`/` 分隔、零 IO、全由目录名常量组派生 | `crates/core/foundation/src/layout/mod.rs`（改） | `std` | 既有布局模块 |
| 命令面 | `list_archi_chat_sessions` 薄包装（三件事纪律：参数转换 → 内核分页查询 → 错误映射） | `src-tauri/src/commands/exec/mod.rs`（改）+ `src-tauri/src/commands/mod.rs`（改：登记） | `agent_runtime::session_query`、`WorkspaceStores` | `#[tauri::command] #[specta::specta]` |
| 面板骨架 | 头区动作（标题 + 新会话 + 历史 ▾）/ 状态行（`note`）/ 历史清单（分页 + 载入 / 空 / 错误）/ `AgentSessionView` 注入形态 + composer 插槽 / 折叠窄条；全部查询挂钩由 `testIdPrefix` 生成；无业务判据、不建会话流 | `packages/desktop/src/components/agent/session-panel.tsx`（新） | `AgentSessionView`、`AgentSessionActions`、`SessionSummary` | React 19 + 既有 `Button` |
| 骨架出口 | 桶导出 `SessionPanel`（组件）与 `SessionPanelState`（类型，供两处 hook `extends`） | `packages/desktop/src/components/agent/index.tsx`（改） | — | 既有 barrel |
| change 侧接入 | 改为骨架消费者（`testIdPrefix="change-chat"`），文案与 composer 插槽留在本地文件 | `packages/desktop/src/views/changes/agent/change-chat-panel.tsx`（改）+ `hooks/use-change-chat.ts`（改：供 `note`） | 骨架、`useChangeChat` | React |
| 架构页会话面 | `useArchiChat`（会话面 + 清单 + 新会话 / 历史 + 运行中禁发 + `note` 两态）→ `ArchiChatPanel`（`SessionPanel` + 归属参数）→ `ArchiChatComposer`（`AgentInput` 薄适配） | `packages/desktop/src/views/architecture/agent/{archi-chat-panel.tsx,archi-chat-composer.tsx,hooks/use-archi-chat.ts}`（新） | 骨架、`useAgentSession`、生成绑定 `listArchiChatSessions` | React + 生成绑定 |
| 架构页布局 | 整宽页面头（标题 + 面板开关 + `archi-refresh`）+ 横向分栏：左栏 `ArchiChatPanel`（`collapsible`、默认收起、留窄条）+ 右栏画布（`key={refreshTick}`） | `packages/desktop/src/views/architecture/architecture-view.tsx`（改） | `ui/resizable`、`ArchiChatPanel`、`ArchitectureCanvas` | `react-resizable-panels`（既有） |

依赖方向零新增边：`agent-runtime → archi` / `agent-runtime → foundation` 均在案；`archi` 仍不依赖任何 infra crate；前端不新增 npm 依赖。

---

## 变更清单 (Change Inventory)

> 路径口径：Rust 侧 `crates/…` / `src/…` 省略 `packages/desktop/src-tauri/` 前缀，前端侧写全 `packages/desktop/src/…`。
> 测试文件（`*_test.rs` / `*.test.tsx` / `*.test.ts`）不在本清单射程——测试由独立相位承接。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `crates/infra/agent/src/archi_knowledge.rs` | 知识拼装唯一实现：`include_str!("archi-knowledge.md")` + 派生段追加；`knowledge()` / `build()` 两函数 | 函数表 `knowledge` / `build` 行；D2 / D3 / D15 |
| `crates/infra/agent/src/archi-knowledge.md` | 静态骨架（含唯一 ```toml 示例块）；零 `<>` 占位符、零目录字面量、零词表 | D3 / D4；`desktop-archi-agent`「架构会话的系统知识注入」 |
| `packages/desktop/src/components/agent/session-panel.tsx` | 会话面板骨架唯一实现（头区动作 / 状态行 / 历史清单 / 折叠窄条 / `AgentSessionView` 注入 + composer 插槽） | 函数表 `SessionPanel` 行；类型表 `SessionPanelProps` / `SessionPanelState` 行；D8 / D9 |
| `packages/desktop/src/views/architecture/agent/archi-chat-panel.tsx` | 架构页面板薄封装：`useArchiChat` + `SessionPanel`（`testIdPrefix="archi-chat"`）+ composer 插槽 | 函数表 `ArchiChatPanel` 行；类型表 `ArchiChatPanelProps` 行 |
| `packages/desktop/src/views/architecture/agent/archi-chat-composer.tsx` | `AgentInput` 薄适配（`idPrefix="archi-chat"`），label / placeholder 文案留本文件 | 函数表 `ArchiChatComposer` 行；类型表 `ArchiChatComposerProps` 行 |
| `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts` | 归属参数（`source="archi"` / `sourceRef=null` / `cwd=null`）+ 清单取数（`listArchiChatSessions` 分页 + 两处事实驱动重取）+ 新会话 / 历史切换 + 运行中禁发 + `note` 两态 | 函数表 `useArchiChat` 行；类型表 `ArchiChatParams` / `ArchiChatState` 行；D10 / D13 / D14 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/agent/src/lib.rs` | 模块清单（`:1-4`）末尾增 `mod archi_knowledge;`；文件尾 `#[cfg(test)] mod lib_test;` 旁增 `#[cfg(test)] mod archi_knowledge_test;`（测试模块本体归测试相位，本相位只需声明可解析） | 新模块的挂载点（`:1-4` 现为 `compose` / `sdk` / `store_port` / `worker`）；测试声明缺失会让 `cargo fmt` / `cargo check --all-targets` 报 `failed to resolve mod` |
| `crates/infra/agent/src/compose.rs` | ① 常量区（`CHANGE_SOURCE` `:20` 之后）增 `const ARCHI_SOURCE: &str = "archi";`（doc：与命令层 `ARCHI_CHAT_SOURCE` / 前端 `use-archi-chat` 三处同字面量，改动需同步）；② `assemble_injections`（`:28`）在 explore 臂之后插 archi 臂（explore 之后、`:47` 短路之前）；③ 函数 doc（`:22-27`）由「两臂」改为「三臂」并补 archi 臂一行；④ `use` 区无需新增（`agent::SessionToolset` 已全路径引用） | 臂内：`preamble: crate::archi_knowledge::knowledge()`、`tools: None`、`toolset: Some(agent::SessionToolset::Archi { module: true, search: true })`；`source_ref` 零读取。explore 臂与 change 臂（`change_phase_role` / `archi_matrix`）逐字不动 |
| `crates/infra/agent/src/sdk/loop.rs` | ① 新增 `pub(crate) fn system_prompt(knowledge: Option<&str>, agent_md: Option<String>) -> Option<String>`（放在 `LoopTurn` 定义之后、`run` 之前）；② `:193-198` 改为 `if let Some(system) = system_prompt(turn.injections.preamble.as_deref(), preamble::load(&turn.cwd).await) { chat_history.push(Message::system(system)); }`；③ 注释「系统提示词每轮重读」补一句合并规则 | 顺序：知识在前、`AGENT.md` 逐字在后、单空行分隔；单侧在场即该条；皆无 → 零 System。`preamble::load` 调用与截断口径零改动 |
| `crates/core/archi/src/dto.rs` | `impl ArchiDiagnosticKind`（`:120-135`）内、`as_str` 之后增 `pub const ALL: [Self; 9] = [..]`（九值按 `as_str` 同序），doc 指向「全量常量 = 派生源，禁手抄词表」 | `ElementKind::ALL` `:52` / `GateTier::ALL` `:86` 同形先例；枚举名、线值、`specta` 注册零变化 |
| `crates/core/foundation/src/layout/mod.rs` | 文件尾（`is_archi_rel_path` `:145-150` 之后、`#[cfg(test)]` 之前）增三个函数 `archi_rel_dir` / `archi_models_rel_dir` / `archi_decisions_rel_dir`；既有四函数与 `Layout` / `resolve` / `config_path` 零改动 | 全从常量组派生（`DOMAIN_DIR_NAME` / `ARCHITECTURE_DIR_NAME` / `MODELS_DIR_NAME` / `DECISIONS_DIR_NAME`），`/` 分隔、零 IO、零行内字面量 |
| `src-tauri/src/commands/exec/mod.rs` | ① 常量区（`CHANGE_CHAT_SOURCE` `:26` 之后）增 `const ARCHI_CHAT_SOURCE: &str = "archi";`；② 文件尾（`list_change_chat_sessions` `:326` 之后）增 `list_archi_chat_sessions`（与 change 版同式：blank root → 空页信封；`list_sessions(Some(ARCHI_CHAT_SOURCE), None, offset, limit)`） | `source_ref` 不过滤（`None`）——一个 workspace 一份历史；doc 写明「与 `agent_sessions` 的隐式过滤区分」 |
| `src-tauri/src/commands/mod.rs` | `all_commands!`（`:24-76`）在 `list_change_chat_sessions`（`:47`）之后**尾部追加** `$crate::commands::exec::list_archi_chat_sessions` | 唯一登记面（proposal 漏列，本设计补入——见事实校准 1）；既有条目零位移 |
| `packages/desktop/src/components/agent/index.tsx` | 增 `export { SessionPanel } from './session-panel';` 与 `export type { SessionPanelState } from './session-panel';`（`SessionPanelProps` 留文件内 interface、不出桶：无跨文件消费者，导出只会成为 knip 报告里的可疑导出） | 既有 `AgentInput` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` 导出零变化 |
| `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | 删去本地 `ChatHeaderActions` / `StatusNote` / `HistoryItem` / `HistoryList` / `CollapsedRail` / `ExpandedBody` 六个实现，改为 `<SessionPanel state={state} title="通用 Agent" testIdPrefix="change-chat" composer={..} collapsed={collapsed} onToggleCollapsed={onToggleCollapsed} />`；`ChangeChatComposer` 经 composer 插槽注入（`running` / `disabled={!state.canSend}` / `disabledReason` / `onSend` / `onStop`） | props 面零变化；`change-chat-*` testid 集合与文案逐字由骨架按前缀生成；`DRAFT_NOTE` 常量随之移入 hook |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | ① `ChangeChatState`（`:39-64`）改为 `extends SessionPanelState`（从 `@/components/agent` 引入）并增 `note: string | null`（其余专有字段逐字保留）；② 文件内增 `const DRAFT_NOTE = '发送首条消息即开启新会话';`；③ `useChangeChat` 返回面（`:262-276`）补 `note: disabledReason ?? (currentSessionId === null ? DRAFT_NOTE : null)` | 判据矩阵（`:258-260`）、清单取数、新会话 / 历史切换、会话面零改动；`note` 派生逐字平移自 `ExpandedBody:170` |
| `packages/desktop/src/views/architecture/architecture-view.tsx` | 改为横向分栏：整宽页面头（`架构` + 面板开关 `archi-chat-toggle` + `archi-refresh`）+ `ResizablePanelGroup orientation="horizontal"`：左 `ResizablePanel`（`defaultSize="30"` / `minSize="22"` / `collapsible` / `collapsedSize="3rem"` / `panelRef`）包 `ArchiChatPanel`，右 `ResizablePanel minSize="40"` 包画布（`key={refreshTick}`）；折叠真相 = 页面 `useState(true)` + `panelRef` 命令面（`useLayoutEffect` 挂载收紧一次） | `root` 入参与 `mode="full"` 装配不变；`archi-refresh` 语义（`key` 重挂）零变化；零 watch 零轮询 |
| `packages/desktop/src/types/generated/bindings.ts` | 由 `pnpm run bindings:export` **重导出**（禁手改）：新增 `listArchiChatSessions` 一条 | `bindings:check` 零 diff 为交付门（AC-15） |
| `packages/desktop/package.json` | `version` `"0.6.0"` → `"0.6.1"`（单行） | 用户可见变更（架构页新增会话面板 + 新知识注入）；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 的 crate version 行**不随动** |

### 公共函数 / API

> 口径：Rust 侧列 `pub` / `pub(crate)` 模块级函数；前端列跨文件导出函数。私有函数（`compose::assemble_injections` 本体、`change_phase_role`、`archi_matrix`、`use-archi-chat` 内的 `useArchiChatList` / `useListRefetch` / `useChatActions`、`architecture-view` 内的面板态 hook）不入表。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `archi_knowledge::knowledge` | `crates/infra/agent/src/archi_knowledge.rs` | 新增 | `pub(crate) fn knowledge() -> Option<String>` | 唯一入口：`build(SKELETON).ok()`；`None` = 降级（compose 只丢 `preamble` 位）。纯函数、零 IO |
| `archi_knowledge::build` | 同上 | 新增 | `pub(crate) fn build(skeleton: &str) -> Result<String, String>` | 骨架 trim 空 → `Err("知识骨架为空")`；含 `<>` → `Err("知识骨架含占位符")`；否则 `skeleton.trim_end()` + `"\n\n"` + 派生段（`kind` 四值 / `gate` 三值 + `default()` / 诊断九词 / 模型与决策目录相对路径），`Err` 文案带触发点 |
| `loop::system_prompt` | `crates/infra/agent/src/sdk/loop.rs` | 新增 | `pub(crate) fn system_prompt(knowledge: Option<&str>, agent_md: Option<String>) -> Option<String>` | `(Some,Some)` → `"{knowledge}\n\n{agent_md}"`；`(Some,None)` → `knowledge.to_owned()`；`(None,Some)` → `agent_md`（**逐字**）；`(None,None)` → `None` |
| `layout::archi_rel_dir` | `crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn archi_rel_dir() -> String` | `<域根段>/<架构域段>`（形如 `openspec/architecture`），`/` 分隔、零前导 `./`、零 IO、不接 root 入参 |
| `layout::archi_models_rel_dir` | 同上 | 新增 | `pub fn archi_models_rel_dir() -> String` | `format!("{}/{}", archi_rel_dir(), MODELS_DIR_NAME)` |
| `layout::archi_decisions_rel_dir` | 同上 | 新增 | `pub fn archi_decisions_rel_dir() -> String` | `format!("{}/{}", archi_rel_dir(), DECISIONS_DIR_NAME)` |
| `commands::exec::list_archi_chat_sessions` | `src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn list_archi_chat_sessions(stores: State<'_, WorkspaceStores>, root: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | `#[tauri::command] #[specta::specta]`；blank root → `SessionPage { sessions: vec![], total: 0 }`；否则 `session_query(store).list_sessions(Some(ARCHI_CHAT_SOURCE), None, offset, limit)` |
| `SessionPanel` | `packages/desktop/src/components/agent/session-panel.tsx` | 新增 | `function SessionPanel(props: SessionPanelProps): React.JSX.Element` | 骨架唯一实现；`testIdPrefix` 生成全部查询挂钩（`${prefix}-panel` / `-actions` / `-new` / `-history` / `-note` / `-history-list` / `-history-item` / `-history-more` / `-history-empty` / `-history-error` / `-rail` / `-expand` / `-session`） |
| `ArchiChatPanel` | `packages/desktop/src/views/architecture/agent/archi-chat-panel.tsx` | 新增 | `function ArchiChatPanel(props: ArchiChatPanelProps): React.JSX.Element` | `useArchiChat({ root })` → `SessionPanel`（`title="架构 Agent"` / `testIdPrefix="archi-chat"`）+ `ArchiChatComposer` 插槽（`running` / `disabled={!state.canSend}` / `disabledReason` / `onSend` / `onStop`） |
| `ArchiChatComposer` | `packages/desktop/src/views/architecture/agent/archi-chat-composer.tsx` | 新增 | `function ArchiChatComposer(props: ArchiChatComposerProps): React.JSX.Element` | `AgentInput`（`idPrefix="archi-chat"` / `label="架构 Agent"` / `placeholder="问这个 workspace 的架构、让它补模型或修文档…"`）；档位 / 发送 / 停止收口在组件族 |
| `useArchiChat` | `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts` | 新增 | `function useArchiChat(params: ArchiChatParams): ArchiChatState` | 会话面（`source: 'archi'` / `sourceRef: null` / `cwd: null`）+ 清单（`listArchiChatSessions(root, offset, 20)`，会话 id 确立 / 轮收口 / 展开历史时重取）+ 新会话（`setSessionId(null)` + `reset()`）+ 历史点选（`setSessionId`）+ 判据（`canSend = !running`、`canStart = !running`、`disabledReason = running ? '本会话运行中，请等待收口' : null`）+ `note` 两态（见待决问题 6：运行中 composer 整面板禁用，`AgentInput` 的「整面板禁用不呈停止入口」规则随之生效） |
| `ArchitectureView` | `packages/desktop/src/views/architecture/architecture-view.tsx` | 修改 | `function ArchitectureView({ root }: { root: string | null }): React.JSX.Element` | props 面零变化；新增分栏与页面头开关（`archi-chat-toggle`）；`archi-refresh` / `refreshTick` / `key` 重挂语义零变化 |
| `ChangeChatPanel` | `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | 修改 | `function ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element` | props 面零变化；内部改骨架消费者（`testIdPrefix="change-chat"`） |
| `useChangeChat` | `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | 修改 | `function useChangeChat(params: ChangeChatParams): ChangeChatState` | 返回面增 `note`（其余逐字不变） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `SessionPanelState` | `packages/desktop/src/components/agent/session-panel.tsx` | 新增 | `{ session: UseAgentSessionState; sessions: SessionSummary[]; total: number; listOpen: boolean; listLoading: boolean; listError: string \| null; toggleList: () => void; loadMore: () => void; newSession: () => void; selectSession: (sessionId: string) => void; canStart: boolean; note: string \| null }`（D8：业务判据与 `send` / `stop` 不入） |
| `SessionPanelProps` | 同上 | 新增 | 文件内 interface（**不出桶**）： `{ state: SessionPanelState; title: string; testIdPrefix: string; composer: (actions: AgentSessionActions) => React.ReactNode; collapsed?: boolean; onToggleCollapsed?: () => void }` |
| `ArchiChatParams` | `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts` | 新增 | `{ root: string \| null }`（`null` ⇒ 零 invoke） |
| `ArchiChatState` | 同上 | 新增 | `SessionPanelState` 的超集：另含 `sessionId: string \| null`（面板本地寻址输入）、`currentSessionId: string \| null`（会话镜像真相）、`send: (input: { text: string; permissionMode: AgentPermissionMode }) => void`、`stop: () => void`、`canSend: boolean`、`disabledReason: string \| null`；声明为 `interface ArchiChatState extends SessionPanelState` |
| `ArchiChatPanelProps` | `packages/desktop/src/views/architecture/agent/archi-chat-panel.tsx` | 新增 | `{ root: string \| null; collapsed: boolean; onToggleCollapsed: () => void }` |
| `ArchiChatComposerProps` | `packages/desktop/src/views/architecture/agent/archi-chat-composer.tsx` | 新增 | `{ running: boolean; disabled: boolean; disabledReason: string \| null; onSend: (input: { text: string; permissionMode: AgentPermissionMode }) => void; onStop: () => void }` |
| `ChangeChatState` | `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | 修改 | 改为 `interface ChangeChatState extends SessionPanelState`（显式承载骨架契约）+ 既有专有字段（`sessionId` / `currentSessionId` / `cwd` / `send` / `stop` / `canSend` / `disabledReason`）逐字不变；inherited `note` 由本 hook 提供 |
| `ArchiDiagnosticKind`（别名 `DiagnosticKind`） | `crates/core/archi/src/dto.rs` | 修改 | 增 `pub const ALL: [Self; 9]`；枚举变体、线值（camelCase）、`specta` 注册零变化 |

<!-- 配置子表：本变更零配置键变更（不开新 config 键、不改 KNOWN_KEYS、不改 config.schema.json；不消费 context / rules），故整段省略。 -->

---

## 验收标准对齐 (Acceptance Mapping)

> 验证手段一栏为承接方（测试相位 / 既有门禁），本设计只给落点，不给用例内容。

| AC | 设计落点 | 承接 |
|----|----------|------|
| AC-1 注入臂 | `compose.rs` 新增 `ARCHI_SOURCE` 常量 + archi 臂（D1）：`preamble = knowledge()` / `tools = None` / `toolset = Some(Archi { module: true, search: true })`；臂内**零读取** `source_ref` ⇒ `null` / 非空 / 畸形四段同臂 | 装配面断言 |
| AC-2 零回归 | explore 臂（`:29-46`）与 change 臂（`change_phase_role` / `archi_matrix`）逐字不动；archi 臂只做 `source` 短路插入（D1） | 既有断言不改 |
| AC-3 知识同源派生 | `build()` 的派生段只从 `ElementKind::ALL` / `GateTier::ALL` + `default()` / `ArchiDiagnosticKind::ALL` / `layout::archi_*_rel_dir` 取词（D2 / D3 / D6 / D7）；`ALL` 补齐后 `as_str` 逐值对拍成立 | 词表断言（生产者 crate，见待决问题 1） |
| AC-4 知识自证 | 骨架含唯一 ```toml 示例块；自证 = `TomlParser::parse` + `ModelGraph::build(..).diagnostics()` 双零（D4）；`build()` 自带 `<>` / 空骨架校验（D2）；体量上限由「骨架 ≤ 3KB + 派生 ≤ 1KB」切分保障（待决问题 5） | 知识面断言 |
| AC-5 单条 System | `loop::system_prompt`（D5）四形态：双在场 → `"{知识}\n\n{AGENT.md}"`；仅 `AGENT.md` → 逐字；仅知识 → 单条；皆无 → 零 System；`preamble::load` 每轮重读位置与截断口径零改动 | 合成面断言 + 既有 `AGENT.md` 回归 |
| AC-6 工具面与档位 | 零代码改动：`tools::definitions` 已按 `SessionToolset::Archi { .. }` 追加两定义，`policy::READONLY_TOOLS` 已含两 archi 工具；本变更只补 archi 臂的 toolset 位 | 既有断言 |
| AC-7 直改主仓通路 | `cwd = null` → `resolve_exec_root` 回落 `root` → `SessionCtx.workspace_root` → `mint_archi_context` 的 root（事实校准 2）；`tools = None` 即七工具全量（含 `write` / `edit`）；`protect` 内置锁面不含架构域（既有裁定，零改动） | 既有链路面 |
| AC-8 写坏可自修 | 写盘路径零校验插口（不新增）；坏模型 → `archi_module` 返回结构化 `Err`（含 `file:line`，既有 `dto` 行为）；画布刷新后 `StructureErrorPanel` 同源诊断（既有）；知识文本要求「改完调 `archi_module` 复查」 | 既有面 + 知识条款 |
| AC-9 画布零放宽 | `architecture-view.tsx` 只加页面头开关与分栏，画布 props（`root` / `mode="full"` / `key`）零变化；零写命令、零 `onResize` 驱动的取数 | 既有断言 |
| AC-10 骨架归一 | `SessionPanel` 为唯一实现（D8 / D9）：骨架内零 `canSend` / `disabledReason` / 归档 / run 期判据、零 `useAgentSession` 调用、零 `send` / `stop` 入参；change 面板原地换骨架（testid 与文案逐字由前缀生成）；`useChangeChat` 只补 `note`（D10） | 既有 675 行行为级用例 + 骨架面用例 |
| AC-11 归属与清单 | `useArchiChat` 固定 `source='archi'` / `sourceRef=null` / `cwd=null`（D14）；`list_archi_chat_sessions` 只按 `source` 过滤（D13），change chat（两段）/ 流程会话（四段）不混；blank root → 空页信封 | 命令面 + hook 面 |
| AC-12 面板交互 | 空态可发（`sessionId: null` → `New` 铸行）、新会话回空态零落库、历史以 `sessionId` 直查、运行中禁发（`canSend = canStart = !running`，历史恒可用）、`note` 两态优先级（D10） | hook 面 + 视图面 |
| AC-13 布局 | 页面头开关（`archi-chat-toggle`）+ 左栏 `ResizablePanel collapsible`（默认 `collapsed = true`，D11）+ 窄条 `-rail` / `-expand`；收起态零会话类 invoke（`sessionId = null` ⇒ 零重放；清单首取只在展开历史 / id 确立 / 轮收口）；`archi-refresh` 的 `key` 重挂零变化 | 视图面 |
| AC-14 字面量单点 | 知识目录路径全由 `layout::archi_*_rel_dir` 注入，骨架 md 零目录字面量（D3）；`ARCHI_SOURCE` / `ARCHI_CHAT_SOURCE` 与前端 `ARCHI_CHAT_SOURCE` 三处同值且互指注释（D13） | 既有命名隔离扫描 |
| AC-15 交付面 | `commands/mod.rs` 登记（事实校准 1）→ `bindings:export` 出 `listArchiChatSessions`；`package.json` 0.6.1（`Cargo.toml` 版本行不随动）；`vp check` / `knip` / `cargo clippy -D warnings` 为机械门 | 机械门禁 |

---

## 数据模型 (Data Model)

### 持久化：零变更

| 面 | 现状 | 本变更 |
|---|---|---|
| 会话归属 | `SessionRecord`（store native model，`crates/infra/store/src/model.rs:418-437`）既有两列 `source: String`（自由字符串）/ `source_ref: Option<String>` | 新来源词 `"archi"` + `source_ref = NULL` 直接落既有两列；**无新模型版本、无新字段、无迁移** |
| 面板「当前会话」 | change 侧为前端内存态 | archi 侧同为前端内存态（`useArchiChat` 的 `sessionId`），零落库 |
| 架构模型 | `<archi_dir>/models/*.toml` | 零格式变更（知识文本描述的字段面与既有 `parse::toml` 严格面一致） |
| 工作区配置 | `config.json` | 零变更（不消费 `context` / `rules`） |

### 进程内数据模型

**知识文本（`String`，一次性拼装，随轮携带）**

```
SKELETON（include_str!，静态）
  ├ §模型位置与聚合（措辞：架构域 models 目录、按文件名字典序聚合；目录路径不在骨架内）
  ├ §格式（一元素一表头 / 带引号完整 FQN 单键 / 字段面 / [[relation]] from·to·desc / metadata 内联表）
  ├ §层级与缺省（FQN 最长前缀、祖先先声明 / 新元素 gate 缺省 off）
  ├ §边界（不写接口与导出符号；契约权威在 openspec/specs，只经 docs 引用）
  ├ §自检（改完调 archi_module 无参复查，按 file:line 修）
  ├ §写面与可见性（直改主仓、随 git diff 评审、只改模型与 docs 关联文档、写盘后请用户点刷新）
  ├ §历史会话续接（先取当下模型再回答）
  └ §示例（唯一 ```toml 围栏块）
DERIVED（build 追加，全部代码派生）
  ├ 元素 kind：ElementKind::ALL → as_str 四值
  ├ gate 档：GateTier::ALL → 三值 + GateTier::default().as_str()（缺省）
  ├ 结构诊断：ArchiDiagnosticKind::ALL → 九词
  └ 目录：foundation::layout::archi_models_rel_dir() / archi_decisions_rel_dir()
```

示例 TOML（骨架内，自证面）：两个元素（祖先 `App` + 子元素 `App.Auth`，子元素先声明祖先成立）+ 一条 `App → App.Auth` 关系（祖先指向后代，规避 `dependsOnAncestor` 与 `cycle`）+ `metadata = { owner = "auth-team" }` 内联表 + `path = ["packages/desktop/src/auth"]` 数组 + `docs = ["docs/auth.md"]` 显式文档条目。四条形态各锁一个易错点：内联 `metadata`（写成子表会被读成另一个元素）、引号键形（点分写法落 `unknownField`）、`gate` 词面、关系两端已声明。

**前端面板状态（内存）**

| 状态 | 归属 | 语义 |
|---|---|---|
| `sessionId` | `useArchiChat` 本地态 | `null` = 空态草案态（可发送、零重放取数、零订阅）；非 null = 以 id 直查重放 |
| `currentSessionId` | `useAgentSession` 镜像派生的真相 | 首发后由后端 early-resolve 轮行回写；驱动清单重取与 `note` 两态 |
| `sessions` / `total` / `listLoading` / `listError` | `useArchiChat` 清单面 | `updated_at` 降序由查询面保证；`sessions.length < total` 决定「加载更多」 |
| `listOpen` | `useArchiChat` 本地态 | 展开即重取首屏；收起不清空（再次展开无陈旧窗口） |
| `collapsed` | `ArchitectureView` 页面 state | 默认 `true`；唯一真源（不取 `onResize`） |

---

## 路由 / API 设计 (Route / API Design)

### IPC 命令（新增一条）

| 方法 | 路径 | 说明 | 输入 | 输出 | auth |
|------|------|------|------|------|------|
| IPC | `list_archi_chat_sessions` | 架构页会话清单（该 workspace 的 `source = "archi"` 会话，`source_ref` 不过滤） | `root: String`（blank ⇒ 空页信封）、`offset: u32`、`limit: u32` | `Result<SessionPage, String>`（`{ sessions: SessionSummary[], total: number }`） | 无（本机 IPC，与既有命令面同款） |

- 前端经生成绑定 `commands.listArchiChatSessions(root, offset, limit)` 消费；`source` 词常量不出命令层。
- 发起沿用既有 `agent_start`（`source = "archi"` / `source_ref = null` / `cwd = null`），零新增发起命令。
- 清单之外的重放面（`session_detail` / `agent_session_transcript` / `agent_session_watch`）全用既有命令。

### 路由

| 方法 | 路径 | 说明 | 输入 | 输出 | auth |
|------|------|------|------|------|------|
| route | `/architecture` | **既有路由，零变化**（`root` 取壳态当前根，URL 不携根段） | — | `ArchitectureView` | 无 |

- 无新增路由、无新增页签、侧栏 `nav-architecture` 零变化（`desktop-page-routing` 零改动）。

---

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 归属 | 用途 |
|------|------|------|
| `archi`（既有 workspace 依赖） | `agent-runtime` | 知识拼装的**词表单点**：`ElementKind::ALL` / `GateTier::ALL` / `GateTier::default()` / `ArchiDiagnosticKind::ALL`（`archi::dto`）；无新增边 |
| `foundation`（既有） | `agent-runtime` | 知识拼装的相对路径族（`archi_rel_dir` / `archi_models_rel_dir` / `archi_decisions_rel_dir`）；无新增边 |
| `agent`（既有） | `agent-runtime` | `SessionInjections` / `SessionToolset` 词汇（字段与变体零变化） |
| `react-resizable-panels`（既有，经 `ui/resizable`） | 前端 | 架构页横向分栏（`ResizablePanelGroup` / `Panel` / `Handle` / `usePanelRef`） |
| `lucide-react`（既有） | 前端 | 骨架头区动作图标（`PlusCircleIcon` / `HistoryIcon`，与 change 面板同一对） |

**零新增包**：Cargo 依赖集与 npm 依赖集均不变（知识文本走 `include_str!`，词表走既有 crate）。

### 构建 / 测试依赖

| 依赖 | 归属 | 用途 |
|------|------|------|
| `vite-plus`（`vp test`）/ `@testing-library/react`（既有） | 前端 | 骨架与架构页组件测试、change chat 既有行为级用例回归（测试相位承接） |
| `cargo test`（既有，无新 dev-dependency） | Rust | 知识文本面断言直接用 `archi::parse::TomlParser` + `ModelGraph::build`（公开面，无需 tempdir） |

---

## 待决问题 (Open Questions)

1. **知识文本面的对拍落点**（事实校准 3）：proposal 测试表把「四面词汇集对拍」的扩展指向 `crates/core/archi/tests/vocabulary_test.rs`，但知识文本由 `agent-runtime` 拼装，`archi` 反向依赖属方向违规。本设计的落法 = 知识文本面断言住 `crates/infra/agent/src/archi_knowledge_test.rs`（逐值 `as_str()` 在场 + 零 `<>` + 体量上限 + 无手抄词表），`archi` 侧三面与 `ALL`-vs-`as_str` 对拍保留原样，两者传递闭合出「四面相等」。若评审坚持单文件四面，则需 `archi` 增加对 `agent-runtime` 的 dev-dependency（workspace 依赖环，代价明显）——请裁决。不阻塞实现。
2. **`crates/core/agent/src/runner.rs` 的 doc 失真**（事实校准 4）：`SessionInjections` 的 doc 仍写「`preamble` 位维持不消费」。proposal 明列该文件「不要修改」，本设计按零改动执行；若要纠正可只改 doc 注释（3 行，零 API 变化），需 proposal 放行。
3. **archi 状态行的 `null` 分支**：spec 要求「空态 → 草案文案；已开会话且无理由 → 刷新提醒；禁用理由优先」，三档覆盖即恒非 `null`；同节 `#### Scenario: 状态行两态与优先级` 末尾又写「无理由且非空态时不占位」（与 change 侧口径同句）。本设计取三档覆盖（`note` 对 archi 恒非 null，类型保留 `string \| null` 以对齐骨架契约）。若评审要求 archi 亦有不占位态，删掉刷新提醒一档即可（单点改动）。
4. **页面头位置**（D12）：spec Module Contract 写「右栏（标题 + 面板开关 + `archi-refresh` + 画布）」，本设计把三者留在**分栏之上的整宽页面头**（可观察次序不变、画布仍居右、左栏收起时开关必可达、与 change 详情 header 同构、且是对现页面头的最小改动）。若评审要求严格「住进右栏」，改动局限 `architecture-view.tsx` 一个文件的 header 位置。
5. **知识体量上限的取值锚**：AC-4 定「≤ 4KB」。本设计按「骨架 ≤ 3KB + 派生段 ≤ 1KB」内部切分以实现良好余量；若后续新增必静态条款逼近上限，优先精简「示例」段（示例的形态锁靠自证测试兜住，不靠篇幅）。
6. **运行中不呈停止入口**：spec 要求「运行中 composer 呈 `disabled` 态（输入框与发送同时禁用）」，而 `AgentInput` 现有规则是「整面板禁用态不呈停止入口」（`agent-input.tsx:94-105`）。两者叠加 ⇒ 架构面板在运行中无法从面板停止（change 面板因 `disabled` 与 `running` 分参而保留停止）。本设计按 spec 字面执行。若评审认为「不可停止」不可接受，两条低成本出路：① 运行中只传 `running`（不传 `disabled`）——但违反「MUST NOT 只禁发送按钮而放过输入」；② 给 `AgentInput` 增「禁用但保留停止」形态——属组件族改动（`desktop-agent-chat-infra` 面），不在本变更范围。请裁决。
