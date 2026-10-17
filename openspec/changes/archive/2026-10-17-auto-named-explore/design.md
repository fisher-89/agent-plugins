# 设计: auto-named-explore

> **变更**: auto-named-explore
> **日期**: 2026-10-17
> **基于**: proposal.md、openspec/explores/auto-named-explore.md（决策记录一至四）、specs/ 四能力 delta

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 四份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」，并把 proposal 风险 9 遗留给 design 的四个定稿面收敛：**create 执行体上下文缝**（组合根铸造、`LoopTurn` 携带，D3）、**枚举命名**（`SessionToolset::Explore`，D1）、**stance 两形文案**（D11）、**占位名时间戳格式**（`draft-<epoch-millis>` + UTC 临时标题，D7）。

工作树实况（本变更起点，`packages/desktop` 0.5.13 / `plugins/dev-team` 2.10.44）：`SessionInjections` 双位（`preamble` / `tools`）为切片③预留接口位、生产装配恒 `default`（`compose.rs:157`）；loop 每轮 `tools::definitions()` 全量七工具组装、`RunStarted.tools` 报 `TOOL_NAMES` 全量常量、分发链 `policy → sandbox → execute` 无成员校验；`tools::execute(root, name, input)` 纯函数；store `create_explore_record(root, name: &str)` 显式名建档；`rename_explore_record`（store + 命令 + bindings）UI 零接线休眠；前端 `TopicEntry` 为 kebab 单行输入（`isKebabCase` 本地校验）、无 auto-send、stance 前导按 `record.name` 钉死路径插值。本设计描述**本变更完成后的目标形态**：占位名建档（`name: Option`）+ create 工具原子定名 + desktop 感知改名 + explore 会话工具面硬约束（write 移除 + 成员校验）。零 db 迁移（`ExploreRecord` 零结构变化）、零事件信封 schema 变化、`rename_explore_record` 维持休眠。

---

## 关键设计决策（proposal / spec 遗留形态面定稿）

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | `SessionToolset` 枚举命名与落位 | `crates/core/agent/src/runner.rs` 新增 `pub enum SessionToolset { Explore }`（derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize` + `#[serde(rename_all = "camelCase")]`，线值 `"explore"`）；`SessionInjections` 增 `pub toolset: Option<SessionToolset>`（`#[serde(default)]`）并为结构体补挂 `Serialize, Deserialize` derive（既有结构原本零 serde——「additive」即从零挂起到可序列化且缺省读兼容旧 JSON） | core 只承载词汇、解释归 infra（`preamble` 位同纪律先例）；受控枚举沿「工具面封闭清单」哲学——自由字符串工具名不进执行面；camelCase 与 `AgentPermissionMode` / `ModelLevel` 同款 | `tools: Vec<String>` 放宽承载专用工具名（封闭集失守，spec 点名否决）；枚举驻 infra（compose 门面已 pub 但词汇跨 core 契约面流转，驻 core 与 `SessionInjections` 同居） |
| D2 | 注入装配判别与非法名 Err | `compose.rs` 新私有 `assemble_injections(source: &str) -> Result<SessionInjections, String>`：`source == "explore"` → `tools = Some([read, grep, glob, ls, edit, bash])`（**write 移除**，序同 `TOOL_NAMES`）+ `toolset = Some(Explore)`；装配前对六名逐个校验 ∈ `tools::TOOL_NAMES`（非法名显式 `Err`——spec 装配层校验条款）；其余来源恒 `SessionInjections::default()`。explore 来源字面量 `const EXPLORE_SOURCE: &str = "explore"` 驻 compose（与 store `EXPLORE_RUN_SOURCE`、前端 `EXPLORE_SOURCE` 三处同字面量，注释互链） | 生产路径首次非 default 装配收在组合根单点；`begin` 的 `provenance.source` 是既有判别源（命令层缺省 `debug`、change-flow 恒 `change-*`，天然走 default 臂零变化） | loop 侧按 session 记录反查来源（引擎读库——越层）；命令层显式传注入参数（扩 IPC 面，spec 无此要求） |
| D3 | create 执行体上下文缝（`LoopTurn` 形态） | 组合根铸造链：`compose_turn` 经门面新 builder `EngineFacade::with_explore_note_store(Arc<Store>)` 携 workspace 库句柄 → `runner_for` 传入 `SdkRunner`（新字段 `explore_store: Option<Arc<Store>>`）→ `session_pump` 以 **per-turn** 形态铸造 `sdk::explore_note::ExploreNoteContext { store: Arc<Store>, root: PathBuf, session_id: String }`（`toolset == Explore` 且句柄在场才铸；`root = open.ctx.workspace_root`，`session_id = Continue id`——即记录 `session_id` 指向的 core 会话 id，`continue_session_id` 既有助手提取）→ `LoopTurn` 增 `injections: SessionInjections` + `explore_note: Option<ExploreNoteContext>` 两字段携带。`tools::execute` 对七通用工具保持 `(root, name, input)` 纯函数签名，MUST NOT 塞 store 句柄 | `compose_turn` 每轮调用（`agent_start` 命令体），门面 builder 与 `with_resume_transcript` / `with_context_window` 同型先例；explore 会话恒 `Continue`（建档预建空会话），core 会话 id 即记录指针——执行体按调用时以 `record.session_id == ctx.session_id` 圈定记录，天然 per-call 解析（改名后续轮自动指向新名，无快照漂移） | `tools::execute` 增第四参（违背纯函数红线）；执行体经系统通知回传改名（事件面多一路消费，proposal 已倾向 id 桥） |
| D4 | definitions 组装参数化与下发面单源 | `sdk/tools.rs`：`pub fn definitions()` 改签 `pub fn definitions(injections: &SessionInjections) -> Vec<ToolDefinition>`——七通用工具按 `injections.tools` 过滤（`None` = 全七；`Some` = 子集，按 `TOOL_NAMES` 序保序过滤），`toolset == Some(Explore)` 时追加 `create_explore_note` 定义（`pub(crate) const EXPLORE_NOTE_TOOL: &str = "create_explore_note"`）；七定义体从 vec 字面量重构为私有 `generic_definition(name: &str) -> ToolDefinition` match 臂（内容逐字不变）。loop 侧 `dispatched` 名单**从 definitions 本体派生**（`definitions.iter().map(|d| d.name.clone())`），`RunStarted.tools` 与成员校验同源消费——零第二份名单 | 下发面单源防漂移（定义与上报与校验三者天然一致）；`TOOL_NAMES` 常量保留（七通用工具封闭集——policy 表与子集校验的基准面） | 独立 `dispatched_names()` 函数（第二份名单，漂移面）；`TOOL_NAMES` 退役（policy 与校验失去基准） |
| D5 | 成员校验形态与次序 | loop 工具分发循环顶部（**先于 policy**）增成员校验：`!dispatched.contains(&name)` → `deny(&sender, "tool_unavailable", …)`（`SystemNotice{subtype:"tool_unavailable"}` + is_error=true ToolResult 与 tool_use 同 id 回灌，run 不中断，不触执行体）+ `history.push(Message::tool_result(…))`，`continue`。默认会话 `dispatched` = 七名——七工具全放行、行为与上线前一致；**未知工具名的拒绝形态自 `permission_denied` 迁移为 `tool_unavailable`**（「不在下发面」比「档位不允许」语义更准，spec ADDED 条款覆盖全域调用名） | 「从 definitions 移除 = 硬禁用」缺口收口（幻觉 `write` 调用不再直通执行）；与 `permission_denied` / `sandbox_denied` 同构 deny 助手复用 | 校验驻 policy（纯决策表需感知注入面——签名扩参破坏纯函数性）；校验驻 `tools::execute`（已过 sandbox，半执行语义） |
| D6 | `create_explore_note` 执行体 | 新文件 `sdk/explore_note.rs`（`ExploreNoteContext::execute(&self, input: &Value) -> Result<String, String>`，async）：① 按调用时圈定记录（`store.list_explore_records(root)` 过滤 `session_id` 指针；miss → 显式 `Err`）；② 占位记录（`draft-` 前缀判别）→ 前置校验（本地 kebab 校验 + 长度 ≤128 / 目标名无既有记录 / 目标文件不存在）→ ① `tokio::fs::OpenOptions::create_new(true)` 落盘 `Layout.explores_root/<name>.md`（create-only 硬语义，缺失父目录补建）→ ② `store.rename_explore_record` in-place 改名（用记录自身 `root` 字段寻址）→ 返回 repo 相对路径（`strip_prefix(root)` + `\`→`/` 归一）；③ 已定名记录 → `name == record.name` 且文件缺失时补建（孤儿重建），否则 `Err` 附现有名与笔记路径（错误文案即恢复通道）；④ ①后②失败 → `Err` 附已落盘路径与恢复指引（重跑 create 命中「文件已存在」附路径 → 会话外对齐或删除残留后重试）。kebab 校验为本文件私有复制（与 store `is_kebab_case` / 命令层 `is_kebab_case` 同口径三处互链——前置校验必须先于落盘，防半边残留） | 会话指针圈定 = per-call 现查（改名后续轮零漂移）；`create_new` 比前置 exists 检查更硬（并发窗口归零）；步序 ①落盘②改名按 spec delta 定稿（见 D12 评估） | 执行体驻 commands 层（引擎工具执行不过 IPC，无命令面）；步序翻转（spec 已定序，见 D12） |
| D7 | 占位名与临时标题铸造 | store `create_explore_record` 签名改 `name: Option<&str>`：`Some` 走既有路径零变化（校验 / 查重 / `title = name` 均照旧）；`None` 分支在**写事务内**铸造——基值 `now_millis()` 一次调用，占位名 `draft-<millis>`（kebab 天然合法：首段字母、次段数字），撞名（同毫秒或既有同名记录）以事务内扫描判重 + **单调 +1** 重试（不依赖事务外时钟推进）；临时标题 `format!("{}-分析中", format_utc_seconds(millis))`（同一基值派生；`format_utc_seconds` 私有助手经 workspace 既有 `time` crate 格式化 `yyyy-MM-dd HH:mm:ss`，**UTC 口径**——store 时间戳纪律同源、本地化归展示层，纯装饰字段可接受；格式化失败退化为毫秒串不 panic）；`ExploreRecord::new` 后覆盖 `title`。前缀常量 `pub const EXPLORE_PLACEHOLDER_PREFIX: &str = "draft-"` 驻 store.rs（`lib.rs` 再导出；promote 门禁与 agent-runtime 占位判别消费，前端 stance 分形侧字面量双源互链） | 同事务铸造（含预建空会话既有两行同事务）保证「无半边」；单调 +1 兜底比时钟重试确定性强（事务内时间可能不推进）；`draft-<millis>` 19 字符 < 128 上限且 kebab 合法免二次校验 | 前端生成占位名（两连 invoke 复活 + 撞名错误逃逸到 UI）；`draft-<yyyyMMdd-HHmmss>`（同秒撞名仍需兜底，字符更多无收益） |
| D8 | promote 占位门禁步序 | `promote_explore_with` 在参数格式校验（blank root / kebab / workflow_type）之后、**读笔记之前**增 `name.starts_with(store::EXPLORE_PLACEHOLDER_PREFIX)` → 显式 `Err`（文案引导先在会话中完成定名：`create_explore_note`）；步序注释随动（① 占位门禁 → ② 读笔记 → ③ 记录 → ④ 写面 → ⑤ 打标） | 占位名永不落盘——「笔记未落盘」错误形态对占位态不足以引导（spec 明令先于笔记读取）；前缀比对一个常量消费，零成本 | 不拦（`draft-*` change 名难看且 stance 命名指令未完成，proposal 倾向拦） |
| D9 | auto-send 闸门 | `useExploreSession` 增第三参 `autoPrompt: string | null`（缺省 null）；effect 闸门四条件：`autoPrompt` 在场 && `!session.loading` && `session.chain.length === 0` && `record/sessionId` 就绪 → 一次性消费 ref（**per-mount** 语义：ref 驻 hook 实例，页面卸载即重置——back/forward 复现 state 时新 mount 重新武装，首轮失败 `chain` 仍 0 即构成 proposal 留痕的免费重试；发出后 `chain > 0` 闸门自然拒发）后 `send({ prompt: autoPrompt, permissionMode: 'bypassPermissions' })`（与 composer 缺省档同源常量）。`state.autoPrompt` 读数驻 `explore-detail-view.tsx`（`useLocation().state` 宽容解析） | send 已内建 loading / 未就绪守卫，auto-send 只是新增一个 effect 触发点；per-mount ref 同时满足「同页不重发」与「复现即重试」两个 proposal 裁定 | sessionStorage 标记（跨 mount 持久——免费重试被杀，违背留痕）；派发后清 location state（react-router 无此 API 形态） |
| D10 | 改名感知（id 桥）与快照兜底 | `explore-view.tsx` 记录派生改三级链：按 URL 名 find → miss 时按 `lastRecordRef`（最近非空派生记录快照，渲染期缓存维护）的 **id** 在清单重寻 → 仍 miss 以快照本体兜底；`explore-detail-view.tsx` 增两名变 effect：① run 终态（`running` true→false 既有 effect）在定点重读文档外增 `list.refresh()`；② `record.name !== routeName`（`useParams` 名与派生记录名分叉——id 桥已寻得新名）→ `navigate(/explores/<新名>, { replace: true })`，`useExploreDoc` 以 name 为依赖自动重订阅（既有机制零改） | in-place 改名保 id——id 是跨名稳定锚（changes 页「一切寻址以 id 为准」哲学呼应）；刷新与导航之间的旧名 miss 由 id 级 / 快照级兜底，MUST NOT 闪现 MissingRecord；replace 不携带 state（autoPrompt 已消费或被 chain 闸门拦截） | SystemNotice `explore_renamed` 事件驱动（实时性更好但事件面多一路消费，proposal 已倾向 id 桥）；刷新后前端本地 `rename` invoke 对齐（写面越权——前端不提供定名入口） |
| D11 | stance 两形文案定稿 | `lib/explore-stance.ts`：`buildExplorePrompt(userInput, recordName)` 签名不变、内部按 `recordName.startsWith('draft-')` 分形（`DRAFT_NAME_PREFIX` 常量与后端前缀双源互链）；公共段抽私有 `modeAndStance()`（模式说明 + 立场五条）与 `commonNoteRules()`（两行约定 + 追加纪律），两形各自拼「笔记」段——**命名指令形**（占位）：生成 kebab 最终名、经 `create_explore_note(name, content)` 原子定名、返回路径供 `edit` 续用、优先专用工具而非 bash 造文件，**MUST NOT 插值任何文件路径**；**笔记定位形**（已定名）：插值 `record.name` 与 `openspec/explores/<name>.md`、指引 `edit` 修改、文件缺失时同名补建。全文见下节定稿文案 | 判别在前端（记录名在手）；占位形零路径插值（路径知识归工具描述与返回值——决策三）；两形共享两行约定与类型推算判断标准（AC「stance 注入类型推算约定」既有断言继续成立） | 单形 + 条件段拼接（文案耦合难审计）；后端注入口（`SessionInjections.preamble` 位维持不消费——spec 明令） |
| D12 | create 两步步序评估 | 维持 spec 定稿序 **① 落盘 ② 改名**；①后②失败的残留窗口以 `Err` 附残留路径与恢复指引收口（前置校验把窗口收窄到并发 DB 错误级——promote 先例）。步序翻转（先改名后落盘）的自愈性留痕评估：翻转后写失败 = 记录已定名 + 文件缺席 → 恰好落进「已定名同名补建」自愈臂，理论上更优；但 spec delta 已显式定序（①②）且翻转把「文件在而记录未改」残留换成「记录改而文件未建」的预览空态窗口（与中间窗口同形），收益边际——本设计从 spec，翻转留待真实残留案例触发再议 | spec 是模块契约唯一真源；两形态的失败面都已有显式恢复通道 | 步序翻转（需 spec 修订先行，超出本 change 范围） |

### D11 附：stance 两形定稿文案

命名指令形（占位记录，`<占位名>.startsWith('draft-')`）：

```
你在探索模式下工作。这是思考与调研时间，不是实现时间：可以读文件、搜代码、调查仓库，但绝不写应用代码，也绝不为了存放笔记而创建 change 目录（用户明确要求产出 OpenSpec 工件时除外）。

## 立场
- 好奇而非说教：问题从对话中自然生长，不套固定清单
- 多开线索而非审问：呈现多个有趣方向，让用户选择跟进哪条
- 善用 ASCII 图解：结构、状态机、数据流，能画就画
- 随线索转向：新信息出现时调整方向，不急于收敛结论
- 落地调研：探索真实代码库，不凭空推演

## 笔记定名与落盘（本次探索尚未定名）
- 调研成型后尽快为本次探索起一个 kebab-case 最终名（小写字母/数字、`-` 连接，如 `api-retry-strategy`）
- 笔记初稿经专用工具 `create_explore_note(name, content)` 落盘：`name` 即最终名，服务端原子完成文件创建与记录定名，返回笔记路径（repo 相对路径）；此后用 `edit` 修改该路径
- 优先使用 `create_explore_note` 而非 bash 造文件（bash 造出的笔记不会与探索记录绑定）
- 笔记首行写 `# <标题>`（人类可读标题，用于探索清单展示；写中文标题亦可）
- 笔记第二行写 `workflow_type: <requirement|test-only>`（推算变更类型：目标是补测试 / 生成测试用例 → test-only，否则 requirement）
- 默认追加；仅用户明确要求时才整文件重写

## 其他
- 回答与提问一律用中文
```

笔记定位形（已定名记录，`<name>` = `record.name` 插值）：

```
你在探索模式下工作。这是思考与调研时间，不是实现时间：可以读文件、搜代码、调查仓库，但绝不写应用代码，也绝不为了存放笔记而创建 change 目录（用户明确要求产出 OpenSpec 工件时除外）。

## 立场
（同上五条，公共段单函数复用）

## 笔记定位（本次探索已定名）
- 本次探索的笔记在 openspec/explores/<name>.md（记录名 = <name>）：修改笔记用 `edit` 编辑该文件
- 文件缺失（如被会话外删除）时，先以 `create_explore_note(name="<name>", content=…)` 同名补建，再继续用 `edit`
- 笔记首行写 `# <标题>`（人类可读标题，用于探索清单展示；写中文标题亦可）
- 笔记第二行写 `workflow_type: <requirement|test-only>`（推算变更类型：目标是补测试 / 生成测试用例 → test-only，否则 requirement）
- 默认追加；仅用户明确要求时才整文件重写

## 其他
- 回答与提问一律用中文
```

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| core 契约词汇（toolset 位） | `SessionToolset` 受控枚举（首值 `Explore`）+ `SessionInjections.toolset` 位 + serde additive 挂起激活；core 只承载词汇不解释 | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs`（+ `lib.rs` 再导出） | serde | Rust（中性契约类型） |
| store 占位建档 | `create_explore_record` 的 `None` 分支：写事务内占位名铸造（单调 +1 兜底）+ 临时标题 + 前缀常量导出 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（+ `lib.rs` 再导出、`Cargo.toml` 增 time） | native_db、native_model、time（workspace 既有） | Rust（sync 写事务） |
| 引擎门面与泵装配缝 | `EngineFacade` builder 携 store 句柄；`SdkRunner` 持有；`session_pump` per-turn 铸造 `ExploreNoteContext` 并随 `LoopTurn` 下发 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` + `sdk/runner.rs` | store、tokio | Rust |
| 工具面组装与成员校验 | `definitions(&injections)` 参数化组装（收窄 + 专用工具追加）；loop 下发面单源、成员校验（`tool_unavailable`）、专用工具路由 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` + `sdk/loop.rs` | rig（ToolDefinition）、agent（SessionInjections） | Rust、tokio async |
| create 执行体（新） | 会话指针圈定记录 + 占位两步定名 / 已定名补建与拦截 + 失败面恢复指引 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note.rs`（新；`sdk/mod.rs` 声明） | store、foundation::layout、tokio::fs、serde_json | Rust、tokio async |
| policy 决策表 | `create_explore_note` 归写面档位（Default 拒绝 / 两写档放行） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` | agent（AgentPermissionMode） | Rust（纯函数） |
| compose 注入装配 | `assemble_injections`（explore 六工具 + Explore 枚举 / 其余 default）+ facade store 句柄注入 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | agent、store、sdk::tools | Rust |
| 命令层 | `create_explore_record` 命令 `name: Option<String>` 透传；`promote_explore_with` 占位前缀门禁 | `packages/desktop/src-tauri/src/commands/explores/mod.rs` | store、workflow、tauri、specta | Rust（IPC 薄包装） |
| bindings 再生 | `createExploreRecord(root, name: string \| null)` TS 面跟随（specta 自动，非手改） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | TypeScript（生成物） |
| 前端新建入口 | TopicEntry 改需求 textarea（`name = null` invoke、空白禁提交、kebab 校验退役）；ImportEntry 显式名；`onCreated` 携占位名 + 需求 | `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` | bindings | TypeScript、React |
| 前端详情与会话 | autoPrompt 读数与传递、run 终态清单刷新、名变 navigate replace；auto-send 闸门（三条件 + per-mount ref） | `packages/desktop/src/views/explores/explore-detail-view.tsx` + `hooks/use-explore-session.ts` | bindings、react-router | TypeScript、React hooks |
| 前端清单视图与 stance | `onCreated` 导航携 `state.autoPrompt`；记录派生三级链（name → id → 快照）；stance 前导两形 | `packages/desktop/src/views/explores/explore-view.tsx` + `packages/desktop/src/lib/explore-stance.ts` | dto、react-router | TypeScript、React |
| 双源同步与插件产物 | SKILL.md 笔记落盘约定随工具化改写；插件版本 bump + 三面产物重建 | `plugins/dev-team/skills/openspec-explore/SKILL.md` + `plugins/dev-team/package.json` | — | Markdown、vite-plus build |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note.rs` | `create_explore_note` 执行体：`ExploreNoteContext`（store 句柄 + root + 会话指针）与 `execute`（占位两步定名 / 已定名补建与拦截 / 失败面恢复指引）；crate 内公共面见「公共函数 / API」表下注（配套 `explore_note_test.rs` 归测试相位） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 `pub enum SessionToolset { Explore }`（derive 与 serde 形态见 D1）；`SessionInjections` 增 `pub toolset: Option<SessionToolset>`（`#[serde(default)]`）并补挂 `Serialize, Deserialize` derive；结构体 doc 由「切片③预留接口位，MVP 引擎忽略」改注「tools / toolset 位自本 change 起按来源装配消费（preamble 位维持不消费）」 | core 契约词汇扩展（additive：旧 JSON 无 toolset 字段读出 `None`） |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | `pub use runner::{…}` 再导出清单增 `SessionToolset` | 词汇出口 |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | `[dependencies]` 增 `time = { workspace = true }`（workspace 既有条目：parsing / formatting / serde 特性足够 `format_description::parse` + UTC 格式化） | 临时标题时间格式化唯一新增依赖面 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | ① 新增 `pub const EXPLORE_PLACEHOLDER_PREFIX: &str = "draft-"`（doc：占位名前缀「未定名」判别口径——生成侧 / promote 门禁 / 引擎占位判别 / 前端 stance 分形消费，前端侧字面量双源互链）；② 新增私有 `format_utc_seconds(millis: i64) -> String`（`yyyy-MM-dd HH:mm:ss` UTC；格式化 / 时间域失败退化为毫秒串，不 panic）；③ `create_explore_record` 签名 `name: Option<&str>`：`Some` 分支既有校验 / 查重 / 建档逐行为零变化；`None` 分支写事务内铸造占位名（基值 `now_millis()` 一次，`draft-<millis>`，事务内扫描同 root 同名判重、命中单调 +1 重试）+ 临时标题（同基值派生 `<yyyy-MM-dd HH:mm:ss>-分析中`，覆盖 `ExploreRecord::new` 的 `title = name` 缺省）；预建空会话两行同事务照旧；doc 注释随动（占位建档零磁盘操作不变） | 占位建档单点；`Some` 路径回归逐字段一致（AC 8） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | `pub use store::{…}` 再导出清单增 `EXPLORE_PLACEHOLDER_PREFIX` | 前缀常量出口（命令层与 agent-runtime 消费） |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | `EngineFacade` 增 `explore_note_store: Option<Arc<Store>>` 字段与 builder `pub fn with_explore_note_store(self, store: Arc<Store>) -> Self`（与 `with_context_window` 同型链式）；`runner_for` 的 Sdk 臂把句柄传入 `SdkRunner::new`（签名增第四参） | 组合根 store 句柄注入缝（builder 单点） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `SdkRunner` 增字段 `explore_store: Option<Arc<Store>>`（`new` 签名增参）；`open_session` 把句柄传入 `session_pump`；pump 内 `LoopTurn` 字面量增 `injections: open.injections.clone()` 与 `explore_note`（`toolset == Some(Explore)` 且句柄在场时以 `ExploreNoteContext::new(store, open.ctx.workspace_root.clone(), continue_session_id(&open))` 铸造——Continue id 缺席则 None，生产不可达） | per-turn 上下文铸造点（D3） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 增 `pub(crate) mod explore_note;`（+ `#[cfg(test)] mod explore_note_test;` 占位声明随测试相位） | 模块声明 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | ① 新增 `pub(crate) const EXPLORE_NOTE_TOOL: &str = "create_explore_note"`；② `definitions` 改签 `pub fn definitions(injections: &agent::SessionInjections) -> Vec<ToolDefinition>`（七通用工具按 `injections.tools` 保序过滤 + `Explore` 枚举追加专用工具定义——入参 `name: string`（kebab 最终名）/ `content: string`（笔记初稿全文，原样落盘），description 注明无路径参数、服务端按当前会话解析、返回 repo 相对路径）；③ 七定义体重构为私有 `generic_definition(name: &str) -> ToolDefinition` match 臂（内容逐字不变）；④ `TOOL_NAMES` doc 随动（基准面口径——RunStarted 与成员校验自 definitions 派生） | 定义面参数化（D4）；`execute` 分发与七执行体零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` | `WRITE_TOOLS` 扩为三值 `["write", "edit", "create_explore_note"]`；模块 doc 注释随动（create_explore_note 归写面：Default 拒绝、AcceptEdits / BypassPermissions 放行——拒绝形态与 write/edit 同构） | 决策表纯函数扩展（AC 6 Default 档拒绝） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | ① `LoopTurn` 增 `pub injections: agent::SessionInjections` 与 `pub explore_note: Option<crate::sdk::explore_note::ExploreNoteContext>` 字段；② `run` 顶部 `let definitions = tools::definitions(&turn.injections);` + `let dispatched: Vec<String> = definitions.iter().map(\|d\| d.name.clone()).collect();`，`RunStarted.tools` 改报 `dispatched.clone()`（doc 口径注释随动：本轮实际下发面）；③ 工具分发循环顶部增成员校验（D5：`tool_unavailable` 拒绝、run 不中断、不触执行体）；④ 执行臂路由：`name == tools::EXPLORE_NOTE_TOOL` → `turn.explore_note` 上下文执行（`ctx.execute(&input).await`；上下文缺席 → is_error 回灌「执行上下文未装配」，生产不可达），其余走 `tools::execute(&turn.cwd, &name, &sandboxed)` 纯函数（签名零改动） | 下发面单源 + 硬禁用收口 + 专用工具路由（AC 5 / AC 7） |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | ① 新增 `const EXPLORE_SOURCE: &str = "explore"`（与 store `EXPLORE_RUN_SOURCE` / 前端 `EXPLORE_SOURCE` 三处同字面量注释互链）与私有 `fn assemble_injections(source: &str) -> Result<agent::SessionInjections, String>`（D2：六工具子集校验 ∈ `crate::sdk::tools::TOOL_NAMES`，非法名显式 `Err`；`Explore` 枚举注入；其余来源 `default`）；② `begin` 的 `TurnRequest.injections` 由恒 `default` 改 `self` 无关的 `assemble_injections(&provenance.source)?`；③ `compose_turn` 门面构造链增 `.with_explore_note_store(Arc::clone(&store))` | 生产路径首次非 default 装配（AC 5 / AC 7） |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | ① `create_explore_record` 命令签名 `name: Option<String>`（`name.as_deref()` 透传 store；blank root `Err` 照旧；doc 注释随动：`None` = 占位建档 / `Some` = 导入绑定显式名）；② `promote_explore_with` 增占位前缀门禁（D8：workflow_type 校验后、笔记读取前，`name.starts_with(store::EXPLORE_PLACEHOLDER_PREFIX)` → 显式 `Err` 引导先完成会话定名；步序 doc 注释随动重编号） | 命令薄包装三件事纪律不变（AC 2 / AC 9） |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成：`createExploreRecord: (root: string, name: string \| null) => …`（非手改，`bindings:check` 守卫拦截漂移） | 前端类型跟随 |
| `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` | ① `ExploreCreateDialogProps.onCreated` 签名改 `(name: string, autoPrompt: string \| null) => void`；② `TopicEntry` 改需求多行 textarea（`data-testid="explore-topic-requirement"`，label「探索需求（自然语言）」；`createExploreRecord(root, null)` 成功后 `onCreated(record.name, requirement.trim())`——占位名取返回记录；空白输入禁用提交）；③ `isKebabCase` 函数与 `explore-topic-invalid` 错误分支退役；④ `ImportItem` invoke 成功回调改 `onCreated(entry.name, null)`（显式名路径零语义变化） | 免命名建档 + autoPrompt 上抛（AC 1） |
| `packages/desktop/src/views/explores/explore-view.tsx` | ① `onCreated(created, autoPrompt)`：`list.refresh()` 后 `navigate(\`/explores/${created}\`, autoPrompt === null ? undefined : { state: { autoPrompt } })`；② 记录派生改三级链（D10：URL 名 find → `lastRecordRef` 快照 id 在清单重寻 → 快照本体兜底；`lastRecordRef` 在派生非空时渲染期刷新维护） | autoPrompt 导航 + id 桥派生（AC 1 / AC 4） |
| `packages/desktop/src/views/explores/explore-detail-view.tsx` | ① `useLocation().state` 宽容解析 `autoPrompt`（非 string 一律 null）传入 `useExploreSession(root, record, autoPrompt)`；② run 终态 effect 增 `list.refresh()`（与既有 `doc.refresh()` 同点）；③ 新增名变收敛 effect：`record !== null && record.name !== routeName`（`useParams<'name'>()`）→ `navigate(\`/explores/${record.name}\`, { replace: true })` | id 桥收敛导航（AC 4；中间窗口停留占位名——AC「终态前中间窗口」留痕接受） |
| `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | ① 签名增第三参 `autoPrompt: string \| null = null`；② 新增 auto-send effect（D9：per-mount 一次性 ref `autoSentRef` + 四条件闸门，`AUTO_SEND_PERMISSION_MODE: AgentPermissionMode = 'bypassPermissions'` 常量与 composer 缺省档同源注释）；③ `send` 内首轮 stance 调用不变（`buildExplorePrompt(input.prompt, record.name)`——分形由 stance 模块内判别） | 自动首轮 + stance 分形调用点（AC 1 / AC 8） |
| `packages/desktop/src/lib/explore-stance.ts` | `stancePreamble(topic)` 退役，改为 `DRAFT_NAME_PREFIX` 常量 + 私有 `modeAndStance()` / `commonNoteRules()` / `namingStance()` / `locatingStance(recordName)` 四片段 + `buildExplorePrompt` 按 `recordName.startsWith(DRAFT_NAME_PREFIX)` 分形组装（D11 定稿文案）；文件头双源互链注释更新（与 SKILL.md 双源同步面随两形扩大） | stance 两形（AC 1 / AC 8） |
| `plugins/dev-team/skills/openspec-explore/SKILL.md` | 「Explore draft inbox」段笔记落盘约定随工具化改写：自由格式叙述调整为「自由格式 + 两条约定行」——首行 `# <标题>`、次行 `workflow_type: <requirement\|test-only>`（与 desktop 探索会话 stance 前导双源同步；desktop 会话笔记创建经专用工具 `create_explore_note` 原子定名——插件 CLI 会话无该工具，仅同步结构约定与命名叙事） | 双源同步（proposal 风险 8 缓解）；属插件源变更走版本 bump + 重建 |
| `plugins/dev-team/package.json` | `version` 2.10.44 → 2.10.45 | 插件源变更版本交付（项目规则） |
| `packages/desktop/package.json` | `version` 0.5.13 → 0.5.14 | 版本交付（`tauri.conf.json` 经 `../package.json` 引用自动跟随） |

> **零改动沿用**：`rename_explore_record` 命令与 store 操作（语义不变、命令面维持不接线——执行体复用 store 操作）；`read_explore` / `scan_explores` / `explore_doc_path` 读面与查询层纪律；`SessionInjections.preamble` 位（系统提示词唯一来源 `AGENT.md`）；事件信封 schema（`RunStarted.tools` 既有字段仅值面分化，`AgentEvent` golden wire contract 不动）；`sdk/bash.rs` 执行体与 `sandbox.rs`（`create_explore_note` 无 path 字段天然穿透）；`sdk/context.rs` / `compact.rs` L1-L3 防线（专用工具返回体恒短，不进 L1 收口）；`worker.rs` 与 change-flow 装配（非 explore 来源恒 default）；`use-explore-list.ts`（`create`/`rename` 动作签名 `string` 对 `string \| null` 参数兼容，零改动）；`AgentMessages` / `tool-cards` 渲染（未注册工具名回退通用工具对卡，`create_explore_note` 零注册可达）；`use-explore-doc.ts`（name 依赖重订阅既有机制）；`desktop-change-*` 全域；`openspec/explores/auto-named-explore.md`（只读）。测试文件（`runner_test.rs` / sdk 各 `_test` / `store_test.rs` / `mod_test.rs` / 前端五测试文件——含 `loop_test.rs` 七工具断言随默认面语义保持、未知工具拒绝形态 `permission_denied → tool_unavailable` 随迁）由测试相位承接。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `SessionToolset`（枚举） | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | `pub enum SessionToolset { Explore }`（derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`；`#[serde(rename_all = "camelCase")]`，线值 `"explore"`） | 会话级专用工具面受控枚举（首值 Explore = explore 笔记工具面）；core 只承载词汇，解释归 infra |
| `SessionInjections`（结构） | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 修改 | `pub struct SessionInjections { pub preamble: Option<String>, pub tools: Option<Vec<String>>, pub toolset: Option<SessionToolset> }`（derive 增 `Serialize, Deserialize`；`toolset` 标 `#[serde(default)]`） | 契约位激活：`tools` = 七工具子集收窄、`toolset` = 受控追加；`preamble` 维持不消费；serde additive（旧 JSON 无 `toolset` 读出 `None`） |
| `EXPLORE_PLACEHOLDER_PREFIX` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（经 `lib.rs` 再导出） | 新增（pub const） | `pub const EXPLORE_PLACEHOLDER_PREFIX: &str = "draft-"` | 占位名前缀「未定名」判别口径单点：store 生成侧、promote 门禁（命令层）、引擎执行体占位判别（agent-runtime 私有复制消费 store 导出）、前端 stance 分形（字面量双源互链） |
| `Store.create_explore_record` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 修改（签名） | `pub fn create_explore_record(&self, root: &str, name: Option<&str>) -> Result<ExploreRecord, StoreError>` | `Some` = 既有显式名建档（逐行为零变化）；`None` = 同事务占位名 + 临时标题 + 预建空会话（撞名写事务内单调 +1 兜底）；store 层零磁盘操作不变 |
| `Store.rename_explore_record` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 不变 | `pub fn rename_explore_record(&self, root: &str, name: &str, new_name: &str) -> Result<ExploreRecord, StoreError>` | 语义不变（in-place 保主键保会话链）；新增消费方为引擎 `create_explore_note` 执行体，命令面仍不接线 |
| `EngineFacade.with_explore_note_store` | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_explore_note_store(self, store: Arc<Store>) -> Self` | 组合根 store 句柄注入 builder（与 `with_context_window` 同型链式；`runner_for` 内传 `SdkRunner`，pump per-turn 铸造执行上下文） |
| `compose_turn` / `ComposedTurn.begin` | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | 不变（签名） | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>` / `pub fn begin(&self, session: SessionRef, question: String, ctx: SessionCtx, provenance: SessionProvenance) -> Result<RunningTurn, String>` | 签名零改动；`begin` 内部按 `provenance.source` 装配注入（explore 六工具 + Explore，非法名 `Err`；其余 default） |
| `create_explore_record`（IPC） | `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 修改（签名） | `#[tauri::command] pub fn create_explore_record(stores: State<'_, WorkspaceStores>, root: String, name: Option<String>) -> Result<ExploreRecord, String>` | `None` = 占位建档（新话题需求直入）；`Some` = 导入绑定显式名；薄包装三件事纪律不变 |
| `promote_explore`（IPC） | `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 不变（签名，行为门禁） | `#[tauri::command] pub async fn promote_explore(app: AppHandle, root: String, name: String, workflow_type: String) -> Result<PromoteOutcome, String>` | 签名零改动；`promote_explore_with` 步序前置占位前缀门禁（先于笔记读取） |
| `buildExplorePrompt` | `packages/desktop/src/lib/explore-stance.ts` | 修改（分形） | `export function buildExplorePrompt(userInput: string, recordName: string): string` | 按 `recordName` 的 `draft-` 前缀分两形（命名指令形 / 笔记定位形，D11 文案）；调用方（`use-explore-session`）零改动 |
| `useExploreSession` | `packages/desktop/src/views/explores/hooks/use-explore-session.ts` | 修改（签名） | `export function useExploreSession(root: string | null, record: ExploreRecord | null, autoPrompt: string | null = null): { … 既有返回面 … }` | 增 auto-send 闸门（三条件 + per-mount 一次性 ref）；既有返回面（messages / chain / events / loading / running / error / send / stop）零变化 |
| `ExploreCreateDialogProps.onCreated` | `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` | 修改 | `onCreated: (name: string, autoPrompt: string | null) => void` | 建档成功回调携需求原文（导入路径传 `null`）；父层据此导航携 `state.autoPrompt` |

> crate 内公共面（`sdk` 为 `pub(crate)` 模块，不入上表）：`sdk::tools::definitions(injections: &agent::SessionInjections) -> Vec<ToolDefinition>`（改签）、`sdk::tools::EXPLORE_NOTE_TOOL: &str`（新增 pub(crate) const）、`sdk::explore_note::ExploreNoteContext::new(store: Arc<Store>, root: PathBuf, session_id: String) -> Self` 与 `ExploreNoteContext::execute(&self, input: &Value) -> Result<String, String>`（async，新增）、`sdk::loop::LoopTurn` 增 `injections` / `explore_note` 两字段。私有件：`compose::assemble_injections`、`store::format_utc_seconds`、`tools::generic_definition`、`explore_note.rs` 内 kebab 校验与各失败臂文案助手、`explore-stance.ts` 四私有片段函数、`use-explore-session.ts` 的 `AUTO_SEND_PERMISSION_MODE` 常量与 `autoSentRef`。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `SessionToolset` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 新增 | 受控枚举首值 `Explore`（serde camelCase 线值 `"explore"`；零 specta——不进 IPC） |
| `SessionInjections` | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 修改 | 增 `toolset: Option<SessionToolset>`（`#[serde(default)]`）；derive 增 `Serialize, Deserialize`（serde 挂起激活，additive） |
| `LoopTurn` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | 修改（crate 内） | 增 `injections: agent::SessionInjections`（definitions 组装源）与 `explore_note: Option<ExploreNoteContext>`（专用工具执行上下文）两字段 |
| `ExploreNoteContext` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note.rs` | 新增（crate 内） | `{ store: Arc<Store>, root: PathBuf, session_id: String }`——per-turn 铸造的专用工具执行上下文（组合根句柄 + 轮 root + 会话指针） |
| `EngineFacade` | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 修改 | 增 `explore_note_store: Option<Arc<Store>>` 私有字段（builder 注入） |
| `SdkRunner` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | 修改（crate 内） | 增 `explore_store: Option<Arc<Store>>` 私有字段（`new` 签名增第四参） |
| `ExploreCreateDialogProps` | `packages/desktop/src/views/explores/components/explore-create-dialog.tsx` | 修改 | `onCreated` 回调签名扩 `autoPrompt: string \| null` |

> `ExploreRecord`（Rust / TS 双面）零结构变化——占位名与临时标题均落在既有 `name` / `title` 值面，零迁移零版本 bump；bindings 的 `ExploreRecord` TS 类型不变，仅 `createExploreRecord` 参数类型随签名再生。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.5.14` | 0.5.13 → 0.5.14（`tauri.conf.json` 经 `../package.json` 引用自动跟随；`src-tauri/Cargo.toml` 版本不随动） |
| `version` | `plugins/dev-team/package.json` | 修改 | string | `2.10.45` | 2.10.44 → 2.10.45（SKILL.md 插件源变更；`pnpm -C plugins/dev-team run build` 重建 `dist/claude-plugins/` / `dist/cursor-plugins/` / `dist/cursor-home-image/` 三面产物） |

> 探索六工具清单 / 占位前缀 / stance 文案均驻源码常量，不进 `config.json`（与 `PRUNED_DIR_NAMES` 等封闭集同纪律）；config schema 零新增键。

---

## 数据模型

### ExploreRecord（workspace 库，零结构变化）

占位建档只动值面：`name = "draft-<epoch-millis>"`（写事务内单调 +1 兜底互异）、`title = "<yyyy-MM-dd HH:mm:ss>-分析中"`（UTC，同基值派生，后续经 `set_explore_title` 回填覆盖）、`session_id` 预建回写照旧、`workflow_type = "requirement"` 缺省照旧。native_model 版本链（v4）零触碰——占位名 kebab 合法，`check_explore_name` 校验面无感知。占位生命周期状态机：

```
[新话题] create(None) ──同事务──▶ draft-<millis>（记录在库；磁盘无文件；清单渲染「<时间戳>-分析中」）
      │ agent 会话内 create_explore_note(name, content)
      ▼ ① explores_root/<name>.md 落盘（create-only） ② rename in-place（保 id / session_id）
已定名 <name>（文件与记录自诞生即最终名；title / workflow_type 回填照常触发）
      │ 前置校验拦截：非法 kebab / 撞记录名 / 文件已存在（附路径）→ 显式 Err 零半边
      │ ①后②失败：文件在而记录未改 → Err 附残留与恢复指引（D12 留痕窗口）
      ▼ promote（门禁：draft- 前缀显式 Err）→ mark_explore_promoted（既有）
```

### SessionInjections / SessionToolset（core 契约，进程内 + serde 线形态）

```json
// explore 会话（compose 装配产物；serde 线形态示例）
{ "preamble": null, "tools": ["read","grep","glob","ls","edit","bash"], "toolset": "explore" }
// 其余来源（default）与旧线 JSON：toolset 缺省读出 null
```

消费链：`compose::begin`（装配 + 子集校验）→ `TurnRequest.injections` → `SessionOpen.injections` → `session_pump`（per-turn 铸 `ExploreNoteContext`）→ `LoopTurn.injections`（loop definitions 组装唯一源）。`preamble` 位全程不消费。

### 探索会话工具面（值面矩阵）

| 会话来源 | definitions | RunStarted.tools | 成员校验 |
|----------|-------------|------------------|----------|
| `source = "explore"` | read / grep / glob / ls / edit / bash / **create_explore_note**（无 write） | 同左（七名，逐名一致） | 调用名 ∉ 左列即 `tool_unavailable` 拒绝（含幻觉 `write`） |
| debug / change / 缺省 | 七通用工具（含 write） | 七名（与上线前一致） | 七工具全放行；未知名 `tool_unavailable` 拒绝（自 `permission_denied` 形态迁移） |

policy 档位 × 工具（`create_explore_note` 新行）：Default 拒 / AcceptEdits 放 / BypassPermissions 放（与 write / edit / bash 同构 `permission_denied` 拒绝形态）。

### 事件与转录（信封零变化）

`RunStarted.tools` 为既有字段，仅值面随来源分化（上表）；`tool_unavailable` 走既有开放枚举 `SystemNotice{subtype, payload}`（与 `api_retry` / `permission_denied` 同通道），payload 形状 `{"tool", "reason"}` 与 `deny` 助手既有口径一致；`create_explore_note` 的 ToolUse / ToolResult 成对入转录（渲染按名回退通用卡，零前端注册）。

<!-- 路由 / API 设计：本变更零 HTTP 端点；IPC 面仅 `create_explore_record` 签名扩 `name: Option<String>`（见公共函数表），`promote_explore` 签名不变仅行为门禁——详情不另立节。 -->

---

## 依赖

### 运行时依赖

workspace 依赖零新增；`crates/infra/store` 增 `time = { workspace = true }`（workspace 既有条目 0.3 + parsing / formatting / serde）——占位临时标题的 epoch-millis → `yyyy-MM-dd HH:mm:ss` UTC 格式化唯一消费点。既有依赖复用：`foundation::layout`（`explores_root` 派生）、`store`（记录圈定 / 改名 / 前缀常量）、`rig::completion::ToolDefinition`（定义面）、`tokio::fs`（create-only 落盘）、serde / serde_json（契约与入参解析）。

### 构建 / 测试依赖

零新增。bindings 再生走既有 `pnpm -C packages/desktop run bindings:export` 管线（`bindings:check` 守卫拦截漂移）；插件三面产物重建走既有 `pnpm -C plugins/dev-team run build`（项目规则：插件源变更后 bump + 重建）。

---

## 待决问题

无——proposal 风险 9 所列四个 design 定稿面已全部收敛（执行体上下文缝 → D3；枚举命名 → D1；stance 两形文案 → D11；占位名时间戳格式 → D7），步序翻转评估留痕于 D12（按 spec 定稿序，真实残留案例触发再议）。接受类风险照 proposal 留痕不重开：autoPrompt 复现的免费重试 / 部分成功意外重发一次（D9 per-mount ref 语义）、定名到 run 终态的中间窗口可见性（D10 一次收敛）、占位前缀撞用户刻意自建的 `draft-*` 名（promote 门禁拦截，文档留痕）、stance / SKILL.md 双源漂移（同步纪律照旧，双侧注释互链）。
