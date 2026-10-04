# 设计: desktop-change-session-visibility

> **变更**: desktop-change-session-visibility
> **日期**: 2026-10-03

---

## 提案与规格同步状态

`proposal.md` 与五个能力 delta（`specs/desktop-agent-execution/spec.md`、`specs/desktop-workflow-write-face/spec.md`、`specs/desktop-change-queries/spec.md`、`specs/desktop-change-flow-view/spec.md`、`specs/desktop-corpus-regression/spec.md`，路径相对域根）已由提案阶段写入，属已完成产物，不在本变更清单与任务列表内。测试文件面（`port_test.rs` / `store_port_test.rs` / `commands/exec/mod_test.rs` / `phase_log_test.rs` / 新 `decision_log_test.rs` 挂账用例 / `corpus_golden_test.rs` 的 `CHANGE_FIXTURES` 清单与语料一致性断言补 `v3-a` 条目 / `detail_test.rs` / `walker_test.rs` 假件适配 / 前端 hook 与组件测试）由 test-design / test-gen / test-execution 阶段承接，不在本清单。

---

## 关键设计决策（design 定稿点）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | core 单查契约形状与命名 | `SessionQuery` trait 增方法 `find_session_detail(&self, session_id: &str) -> Result<SessionSummary, String>`（沿提案「`find_session_detail` 形状」命名定稿）；返回类型复用既有聚合形状 `SessionSummary`（row + stats + turns），**零新 DTO**；查无此 id 显式 `Err`；不设顶层运行状态字段——轮行清单为唯一状态事实源（有 running 轮行即 running），与清单面 / 调试页同一推导式 | 单查语义与清单空态区分（proposal 决策表拍板）；`SessionSummary` 三件套恰为「会话行 + 聚合统计 + 轮行」既定形状，新造 DTO 只会分裂线格式 |
| D2 | infra 单查实现路径 | 两段式：① `store.find_session(session_id)` 主键直查锚定存在性（miss → 显式 `Err`，不触全表）；② `store.list_sessions(None, None)` 收敛取单条聚合形状。store 无「单会话轮行读面」，零 store 变更约束下清单面是聚合形状的唯一既有路径（全表读 + 内存过滤为 store 既有哲学）；第二段 find 未命中为类型完备性兜底（会话无删除路径），同词 `Err` | 轮行（`AgentRunRecord`）只经 `list_sessions` 可达；`find_session` miss-fast 使「查无此 id」路径免全表扫描。proposal 所指「`find_session` 底座」在本实现中承担存在性锚定，store schema 零变更承诺不破 |
| D3 | 命令面形状 | 同步命令 `session_detail(stores, root, session_id) -> Result<Option<SessionSummary>, String>`（与 `agent_sessions` 同 sync 直查形态）；blank root → `Ok(None)`（空结果，与既有两查询命令同口径）；查无 id → core `Err` 透传为命令 `Err`；命令体三件事纪律（参数转换 → `agent_runtime::session_query(store).find_session_detail(...)` 调用 → 错误映射）；`Some` 包裹经 `.map(Some)` | 单对象「空结果」只能是 `null`（`Option`）；吞掉 `Err` 会把悬挂 id 伪装成空态，违背单查拍板 |
| D4 | 会话槽位字段形态 | **扁平三字段**。磁盘键（snake_case，写面 raw 写入）：`executor_session_id` / `evaluator_session_id` / `decision_session_id`；`PhaseLog` 模型字段 `pub executor_session_id: Option<String>` 等三个，各带 `#[serde(default, alias = "executor_session_id")]`（容器 `rename_all = "camelCase"` 下 alias 承接磁盘 snake_case 键）——与 `backtrack_to` / `backtrack_reason` 演进先例逐字同型 | 三个槽位独立落账、无整体传递的消费方，结构体聚合只引入嵌套默认值与旧文件兼容的额外分支；扁平字段使宽松解析口径、显式在位纪律、golden diff 面全部沿既有先例零新概念 |
| D5 | 落账随行与显式在位纪律 | `PhaseLogInput` 增 `executor_session_id` / `evaluator_session_id` / `decision_session_id` 三个 `Option<String>` 槽位；`phase_log` 写面仅对 `Some` 槽位以 raw 键 insert 进条目（沿 skipped / start_at 扩展字段先例），缺省槽位不产生键。walker 侧：verdict 条目携 executor + evaluator 双槽位（decision 恒 None）；static-check 升格 fail 条目仅携 executor 槽位（evaluator 未跑）；executor 会话 id 经 static-check 反馈边续注同会话保持稳定 | 显式在位才写使「缺省槽位条目形状与既有形态一致」可静态保证（AC-4）；槽位值全部来自 `WorkerTurnOutcome.session_id` 透传，写面不解释不改写（desktop-workflow-write-face spec 文字） |
| D6 | decision 槽位写入时机 | **显式 amend 写面操作**：新写面函数 `decision_log(layout, change, phase, session_id) -> Result<DecisionLogOutcome, String>`（新文件 `write/decision_log.rs`）——定位该相位最新 eval 条目（同 backtrack 的 `latest_entry_index` 锚定），raw 定点改写 `decision_session_id` 键，幂等覆写；无条目显式 `Err`（不做表位校验——不新增条目、不触相位路由语义）。时机：`decision_session` 内每轮 `run_worker` completed 收口后、`parse_decision` 之前挂账（parse 失败路径同样留痕）；ask 续轮同会话同值幂等重挂。载体：`ToolCommand::DecisionLog { change, phase, session_id }` + `ToolStepOutput::DecisionLog(DecisionLogOutcome)`，经 `LocalToolSteps` 进程内直调（写通道唯一不变）。**不新增 `ChangeStepKind` / `RunUpdate` 词汇**（挂账不 emit 步状态，`run-state.ts` 零触点）；挂账失败显式 `Err` → run 失败（写面严格语义） | 拒绝 backtrack 写挂：只覆盖 Backtrack 动作，Retry / Stop 决策会话仍不可达——恰是本变更要关闭的缺口；拒绝随下一 attempt 条目：backtrack 跨相位后无同相位宿主、Stop 终局无下一 attempt。首轮收口即挂账使 AC-5「收口后记录在案」对 parse 失败路径也成立；失败 / 停止的 decision 会话不入槽位（库内转录仍在，经调试页 change 筛选可查，AC-3 兜底） |
| D7 | 详情 DTO 暴露与 bindings 面 | `AttemptRecord` 追加三 `Option<String>` 字段（纯 derive 零字段属性口径不变），`From<&PhaseLog>` 直读透出；wire 键 `executorSessionId` / `evaluatorSessionId` / `decisionSessionId` **恒在场**、无槽位降级 `null`（golden 线面「缺省字段 null」既定契约）。bindings 经 `bindings:export` 一次重导，预期 diff 仅两处：Commands 段新增 `sessionDetail` 条目 + Types 段 `AttemptRecord` 增三字段 | 纯 derive 是 golden 契约的机械前提（手动序列化属性会被 specta 判相位差）；查询层直读不派生、不回填（desktop-change-queries spec 文字） |
| D8 | 前端转录寻址升级 | `RoleSessionRef` 改双键可空 `{ role, sourceRef: string \| null, sessionId: string \| null }`：runtime 节点 `sessionId = node.sessionId`（运行步实时 id）、eval 节点取 record 槽位，槽位在场 → `sessionDetail` + `agentSessionTranscript` 按 id 直查；executor / evaluator 槽位缺席 → 回退既有 sourceRef 定式反查（旧数据兜底，行为与升级前一致）。**decision 特例：`sourceRef` 恒 null**——槽位缺席即双 null → 空态不发起任何查询（MUST NOT 误挂他 attempt 会话）；槽位在场直查。直查 `Err`（悬挂 id）呈 `transcript-error` 错误态，**不静默回退反查**（回退仅限槽位缺席，防同 ref 多会话歧义回流）；blank root 的 `null` 应答呈空态。tab key 改 `ref.sessionId ?? ref.sourceRef ?? ref.role` | desktop-change-flow-view spec 语义逐字落地：「缺席时 decision tab 呈空态」即不查询；双键可空用类型面表达寻址模式，杜绝「有 id 仍走反查」的实现漂移 |
| D9 | 调试页来源筛选 | `useAgentRunHistory(root, source)` 参数化：新导出类型 `AgentHistorySource = 'debug' \| 'change' \| 'all'`，`'all'` 映射 `null` 不过滤，取数改 `commands.agentSessions(root, source === 'all' ? null : source, null)`，effect 依赖增 `source`；显式刷新（tick 重取）与「无轮询无订阅」不变；默认 `'debug'` 现状不变。筛选状态落 `AgentDebugView`（`useState<AgentHistorySource>('debug')`），三态按钮组 UI 落 `AgentRunHistory` 头行（`data-testid="history-source-filter"` + 每枚 `data-source`，aria-pressed 高亮沿 `StreamToggle` 先例）；重放链路复用 `AgentRunHistory` 整套零新概念 | 落点定稿（proposal 的「历史区组件落点」二选一）：筛选 UI 与列表同区、状态提升到页面——组件保持无状态取数展示，测试锚点单一；默认值保住调试页既有用户不被惊扰（proposal 决策 Q2） |
| D10 | 语料样本与 golden 对账 | 新样本**手工构造最小合成 fixture `v3-a`**（非归档快照）：v2 形状 workflow.json，eval 条目覆盖三形态——三槽位齐全（pass 条目）、仅 executor 槽位（static-check 升格 fail 形态）、无槽位键（旧形态对照）；配套 proposal.md / tasks.md 最小文档集（沿 v2-b 合成样本形态）。golden 沿显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` 触发重写开关，入口见 `corpus_golden_test.rs` 头注）再生成，**不静默自动重写**；预期 diff 形态见「数据模型」后附清单，人工逐份确认留痕；既有样本文件内容零改写 | 等首个含槽位归档 change 收录会让 AC-9 无限期阻塞且本变更自产落账实例不可复用（golden 输入是入仓只读快照语料）；合成样本使三形态可确定性覆盖，来源与覆盖点记入 fixtures README 清单表（语料完整性测试要求同步维护） |
| D11 | 版本 | `packages/desktop/package.json` version 0.4.3 → 0.4.4 **随归档提交执行**（归档轨道，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动） | 用户可见变更（单查命令 / 清单筛选 / 节点会话联动）才提升（AC-11 / 仓库既有 bump 口径） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 会话单查契约（core） | `SessionQuery` trait 增 `find_session_detail` 单查方法（D1）；`list_sessions` / `transcript` / `reconcile_stats` 零改动 | `packages/desktop/src-tauri/crates/core/agent/src/port.rs` | `crate::session`（SessionSummary / SessionRow / SessionStats） | Rust trait（core 定契约、infra 实现） |
| 单查 store 适配（infra） | `StoreQuery` 实现单查：`find_session` 锚定 + 清单面收敛（D2）；store schema 零触达 | `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs` | `store`（find_session / list_sessions）、`agent`（SessionQuery） | Rust（redb workspace 库只读） |
| `session_detail` 命令 | 薄包装三件事：blank root → `Ok(None)`、库寻址、单查调用与错误映射（D3） | `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `agent_runtime`（session_query 组合根）、`store::WorkspaceStores`、tauri / specta | Rust（tauri command + specta，sync） |
| 命令登记单面 | `all_commands!` 宏追加 `exec::session_detail` 条目（proposal 所指「lib.rs builder 注册处」的实际单点：`main.rs` 的 `invoke_handler` 与 specta builder 经同宏消费，`lib.rs` / `main.rs` 本体零改动） | `packages/desktop/src-tauri/src/commands/mod.rs` | `commands::exec` | Rust declarative macro |
| workflow 磁盘模型槽位 | `PhaseLog` 增三槽位可选字段 + serde alias（D4）；宽松解析口径不变 | `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | serde / time | Rust（serde 磁盘模型，不出线） |
| 评估落账槽位写面 | `PhaseLogInput` 增三槽位随行 + `phase_log` 显式在位才写 raw 键（D5）；attempt / verdict 推导零改动 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | `crate::write::persist`（load_doc / eval_entries_mut / save）、`crate::write::phase_table` | Rust（sync 写面，raw Value 保形定点改写） |
| 决策槽位挂账写操作（新） | `decision_log`：相位最新 eval 条目 raw 定点改写 `decision_session_id`，幂等覆写、无条目显式 `Err`（D6） | `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs` | `crate::write::persist`（load_doc / eval_entries_mut / latest_entry_index / entry_phase / save） | Rust（sync 写面，新文件） |
| 写面出口登记 | `write/mod.rs` 挂载 `decision_log` 模块与 `pub use` 出口；模块 doc「schema 形状零新字段」措辞演进为「唯一例外 = eval 条目会话槽位」 | `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 子模块声明 | Rust（mod 出口） |
| 工具步命令封闭集 | `ToolCommand` 增 `DecisionLog` 变体、`ToolStepOutput` 增 `DecisionLog(DecisionLogOutcome)` 变体（D6） | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `workflow::write`（DecisionLogOutcome） | Rust enum（封闭集） |
| 工具步本地适配 | `LocalToolSteps` 增 `DecisionLog` 臂：layout 解析后进程内直调 `workflow::write::decision_log`（写通道唯一不变） | `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `workflow::write`、`foundation::layout` | Rust（进程内直调，零 CLI 子进程） |
| 相位循环 walker（槽位生效层） | drive() 将 executor 会话 id 携出 if-let 块作用域；`step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id` 取值随 `PhaseLogInput` 落账（D5）；`decision_session` 增 tools 参，收口即 `DecisionLog` 挂账（D6）；ask / 取消 / 终态 / static-check 反馈边逻辑零改动，零新步状态词汇 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `crate::port`（ToolCommand / WorkerTurnOutcome）、`workflow::write`（PhaseLogInput） | Rust（async 相位循环） |
| 详情 DTO 槽位透出 | `AttemptRecord` 增三槽位字段 + `From` 直读透出（D7）；纯 derive 零字段属性口径不变 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `crate::model`（PhaseLog） | Rust（出线 DTO， specta Type） |
| bindings 生成物 | 经 `bindings:export` 重导：`sessionDetail` 命令条目 + `AttemptRecord` 三字段（D7），非手写 | `packages/desktop/src/types/generated/bindings.ts` | `export-bindings` crate（specta builder 经 `all_commands!` 同源） | TypeScript（生成物） |
| 调试页取数 hook | `useAgentRunHistory(root, source)` 参数化 + `AgentHistorySource` 导出（D9）；取数 / 重放 / 显式刷新模型零新概念 | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | `bindings`（commands.agentSessions） | React hook（TypeScript） |
| 调试页筛选接线 | 筛选状态 `useState<AgentHistorySource>('debug')` 持有与下传（D9） | `packages/desktop/src/views/agent/agent-debug-view.tsx` | `useAgentRunHistory`、`AgentRunHistory` | React 组件（TSX） |
| 历史区筛选 UI | 头行三态按钮组（debug / change / 全部，默认 debug、aria-pressed 高亮、`data-testid="history-source-filter"`）（D9）；列表 / 重放 / 刷新零改动 | `packages/desktop/src/views/agent/components/agent-run-history.tsx` | `@/components/ui`（Button）、`AgentHistorySource` | React 组件（TSX + tailwind） |
| 会话寻址 hook | `useSessionTranscript` 增 `sessionId` 入参：直查优先（sessionDetail → transcript）、sourceRef 反查兜底、decision 双 null 空态、直查 Err 错误态（D8）；seq 归并与 running 推导不变 | `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | `bindings`（commands.sessionDetail / agentSessionTranscript / agentSessions）、`agent-adapter` | React hook（TypeScript） |
| 抽屉联动扩展 | `selectionRoleRefs` 补 decision 第三 ref（槽位直读）+ runtime 节点 sessionId 直查键（D8）；「查看会话」入口即既有节点选中联动，零新交互面 | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `flow/types`（RoleSessionRef）、`SessionTranscriptPanel` | React 组件（TSX） |
| 三会话转录面板 | 三 tab（执行 / 评估 / 决策）+ decision 空态呈现 + tab key 改双键回退链（D8）；`AgentTimeline` 复用不变 | `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | `useSessionTranscript`、`AgentTimeline`、`flow/types` | React 组件（TSX） |
| WorkerAgent 节点显式入口 | `group === 'workerAgent'` 节点渲染「查看会话」按钮（`data-testid="view-session"`），点击上抛与节点点击同一 `DrawerSelection`（D8 / proposal 决策 Q1） | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `flow/types`（RunStepNodeData） | React 组件（TSX，react-flow 自定义节点） |
| 图层入口接线 | `RunStepNodeData` 增 `onOpenSession`，`toChartNodes` 仅对 workerAgent 节点注入（`onSelect({ scope: 'node', nodeId })` 同节点点击）（D8） | `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | `flow/types`（DrawerSelection） | React 组件（TSX，react-flow 薄层） |
| 联动键组类型 | `RoleSessionRef` 双键可空化 + 字段 doc 钉死寻址语义（D8） | `packages/desktop/src/views/changes/flow/types.ts` | `types/dto`（AttemptRecord） | TypeScript type |
| 语料增量样本（新） | `v3-a` 合成样本：三槽位齐全 / 仅 executor / 无槽位三形态条目（D10） | `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/workflow.json` | — | JSON fixture（入仓只读语料） |
| 语料清单 | README 清单表补 `v3-a` 行（来源：合成样本；覆盖点：会话槽位三形态）（D10） | `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/README.md` | — | Markdown（语料完整性测试要求同步维护） |
| golden 快照对账物 | 既有 12 份快照按显式重写流程覆写 + 新增 `v3-a.json`（D10，预期 diff 清单见数据模型节） | `packages/desktop/src-tauri/crates/core/workflow/tests/golden/v3-a.json` | `corpus_golden_test.rs` 投影（重写开关 `DESKTOP_GOLDEN_REWRITE`） | JSON（golden 快照） |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件归 test-design / test-gen / test-execution 阶段，不在本清单。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs` | 决策会话槽位挂账写操作（`decision_log` + `DecisionLogOutcome`，D6）；模块 doc 钉死锚定语义与幂等覆写纪律，措辞零双禁令字面量 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/workflow.json` | 含会话槽位的新代际合成样本：requirement 形状 workflow.json，eval 三条目分别覆盖三槽位齐全 / 仅 executor 槽位（fail 升格形态）/ 无槽位键（旧形态对照）（D10） |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/proposal.md` | 样本配套文档（最小文档集，沿 v2-b 合成样本形态） |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/tasks.md` | 样本配套文档（同上） |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/v3-a.json` | 新样本 golden 快照（随显式重写流程生成） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/port.rs` | `SessionQuery` trait 增 `find_session_detail` 方法（签名见公共函数表），方法 doc 钉死单查语义（查无此 id 显式 `Err`、运行状态自轮行推导）；`list_sessions` / `transcript` / `reconcile_stats` 零改动（AC-1 / D1） | core 契约单点扩展；`SessionSummary` 复用零新 DTO |
| `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs` | `impl SessionQuery for StoreQuery` 增单查实现：`find_session` miss-fast 锚定 → `list_sessions(None, None)` 收敛取单条（D2） | store 触点仅既有公开方法（`find_session` / `list_sessions`），`crates/infra/store/**` 零改动 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 增 `session_detail` 命令（tauri command + `#[specta::specta]`，D3）；复用 `is_blank_root` 守卫与 `WorkspaceStores::for_root` 寻址 | 命令体三件事纪律；既有 `agent_start` / `agent_stop` / `agent_sessions` / `agent_session_transcript` 签名与语义零收窄（proposal「不要修改」条目） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 宏追加 `$crate::commands::exec::session_detail` 条目（置于 `agent_session_transcript` 之后）（D3） | proposal 所指「`packages/desktop/src-tauri/src/lib.rs`（builder 注册处）」经核实的实际登记单点即本宏——`main.rs` 的 `invoke_handler` 与 `src/bindings/mod.rs` 的 specta builder 经同宏消费，`lib.rs` 仅 `pub mod` 声明，`lib.rs` / `main.rs` 本体零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs` | `PhaseLog` 追加三槽位字段 `executor_session_id` / `evaluator_session_id` / `decision_session_id: Option<String>`，各带 `#[serde(default, alias = "<同名字段名>")]`（AC-4 / D4） | `backtrack_to` 先例同型；旧 workflow.json 读解析不报错；宽松解析口径与「未知字段 serde 忽略」不变 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | `PhaseLogInput` 增三 `Option<String>` 槽位字段；`phase_log` 落账处对 `Some` 槽位以 raw 键 insert（`executor_session_id` 等 snake_case 键），缺省不产键（AC-4 / D5） | 沿 skipped / start_at 扩展字段先例；attempt 推导、verdict 推导、report 长度门、开相前置、落账清 `active_phase` 零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 挂载 `mod decision_log;` + `pub use decision_log::{decision_log, DecisionLogOutcome};` + 挂账 / 槽位测试模块声明；模块 doc「schema 形状零新字段（W3）」措辞演进为「唯一例外 = eval 条目会话槽位字段（desktop-change-session-visibility 裁定）」 | 写面出口单点；sync 零 tokio 边界声明随 doc 保持 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ToolCommand` 增 `DecisionLog { change: String, phase: String, session_id: String }` 变体；`ToolStepOutput` 增 `DecisionLog(DecisionLogOutcome)` 变体（AC-5 / D6） | 封闭集扩展；既有变体与注释零改动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `execute` 增 `DecisionLog` 臂：`workflow::write::decision_log(&layout, &change, &phase, &session_id)` 直调 → `ToolStepOutput::DecisionLog`（AC-5 / D6） | 写通道唯一（进程内直调）不变；static-check 委托臂零改动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | drive() 中 executor 会话 id 携出 `if let Some(executor)` 块作用域（`Option<String>`）；`step_verdict_phase_log` 增 executor 槽位入参、`PhaseLogInput` 携 executor + evaluator 双槽位；`step_fail_phase_log` 增 executor 槽位入参（`FeedbackLoop.executor_session` 透传）、fail 条目仅携 executor 槽位；`decision_session` 增 `tools` 参，每轮 `run_worker` 收口后经 `run_tool::<DecisionLogOutcome>` 挂账（`ToolCommand::DecisionLog`），置于 `parse_decision` 之前（AC-5 / D5 / D6） | ask 应答循环 / 取消观测 / 白名单与重试门 / 终态收口 / 步状态 emit 词汇零改动（`run-state.ts` 零触点）；`emit_step` 签名不变 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `AttemptRecord` 追加三 `Option<String>` 字段 + `From<&PhaseLog>` 逐字段直读透出（AC-6 / D7） | 纯 derive 零字段属性口径不变（golden 契约）；`change_detail` 聚合逻辑零改动（透出即排序 / 归并既有路径） |
| `packages/desktop/src/types/generated/bindings.ts` | 生成物重导（`pnpm -C packages/desktop run bindings:export`）：Commands 段新增 `sessionDetail` 条目、Types 段 `AttemptRecord` 增三字段（D7），其余零 diff | 非手写文件；重导幂等 + 最小 diff 是守线验收面 |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 导出 `AgentHistorySource` 类型；`useAgentRunHistory` 增第二参 `source` 并下传 `useSessions`；取数改 `commands.agentSessions(root, source === 'all' ? null : source, null)`，effect 依赖增 `source`（AC-3 / D9） | 去硬编码 `'debug'`；`AgentRunHistoryState` 返回面、`useReplay` 重放链路、显式刷新（tick）零改动，无轮询无订阅 |
| `packages/desktop/src/views/agent/agent-debug-view.tsx` | `useState<AgentHistorySource>('debug')` 持有筛选状态，下传 `useAgentRunHistory(root, source)` 与 `<AgentRunHistory state={history} source={source} onSourceChange={setSource} />`（AC-3 / D9） | 表单 / 流视图切换 / 聊天会话面零改动 |
| `packages/desktop/src/views/agent/components/agent-run-history.tsx` | `AgentRunHistoryProps` 增 `source` / `onSourceChange`；头行「刷新历史」左侧增三态按钮组（debug / change / 全部，`data-testid="history-source-filter"` + 每枚 `data-source`，aria-pressed 高亮）（AC-3 / D9） | 列表 / 重放区 / 空态 / 错误态零改动；不引入 ui 新组件件 |
| `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 入参增 `sessionId: string \| null`；直查分支（`sessionDetail` → running 推导 → `agentSessionTranscript` 重放）+ 反查兜底分支（sessionId 与 sourceRef 双 null → 清空态不查询）；直查 `Err` 呈错误态不回退（AC-8 / D8） | `assembleTranscript` seq 归并、replayRef 合并底座、返回面 `{ messages, running, error }` 零改动；`agent-adapter` / `use-agent-chat` 零触达 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | `selectionRoleRefs`：eval 节点 ref 组补 decision 第三项（`sessionId` 取 `record.decisionSessionId`、`sourceRef` 恒 null），executor / evaluator 增槽位 `sessionId` 键；runtime 节点 ref 增 `sessionId = node.sessionId`（AC-7 / AC-8 / D8） | 三分节（文档 / 评估 / 文件表）、遮罩关闭、liveEvents 过滤零改动；单一交互入口不变（不建会话 route） |
| `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx` | 三会话 tab（执行 / 评估 / 决策，沿既有 `transcript-role-tab` 锚点）；`RoleTranscript` 入参改双键透传；tab key 改 `ref.sessionId ?? ref.sourceRef ?? ref.role`（AC-7 / D8） | `AgentTimeline` 复用、空态 / 错误态呈现、`ROLE_LABEL` 词汇零改动（decision 标签既有）；不建第二套时间线 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `RunStepNodeData` 增 `onOpenSession?: () => void`；`group === 'workerAgent'` 节点渲染「查看会话」按钮（`data-testid="view-session"`，`onClick` stopPropagation + 上抛）（AC-8 / D8） | ToolStep / Gate 节点无入口（沿用右侧抽屉步骤结果呈现）；步状态视觉 / 徽章 / Handle 零改动 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | `toChartNodes` 对 `kind === 'runtime' && group === 'workerAgent'` 节点注入 `data: { node, onOpenSession: () => onSelect({ scope: 'node', nodeId: node.id }) }`（AC-8 / D8） | `onNodeClick`、边锚定、translateExtent 零改动；图结构（`PIPELINE_PHASES` 布局 / 边推导）零触达 |
| `packages/desktop/src/views/changes/flow/types.ts` | `RoleSessionRef` 改 `{ role: FlowRoleLabel; sourceRef: string \| null; sessionId: string \| null }`，字段 doc 钉死直查优先 / 兜底 / decision 空态三态语义（AC-8 / D8） | 其余视图模型类型零改动 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/README.md` | 清单表补 `v3-a` 行（来源：合成样本（非归档快照）；覆盖点：eval 条目会话槽位三形态）（AC-9 / D10） | 既有行零改写；语料完整性测试要求 README 与目录同步维护 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/golden/`（既有 12 份 `.json` 快照） | 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程覆写（AC-9 / D10） | 预期 diff 仅「attempts 非空条目新增三 null 键」（清单见数据模型节）；人工逐份确认留痕；既有 fixtures 样本文件内容零改写 |
| `packages/desktop/package.json` | version 0.4.3 → 0.4.4 | **归档轨道**（AC-11 / D11），随归档提交执行，不在实现任务列表 |

### 公共函数 / API

Rust 签名为 Rust 语法；TS 签名为 TS 语法。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `SessionQuery::find_session_detail` | `crates/core/agent/src/port.rs` | 新增 | `fn find_session_detail(&self, session_id: &str) -> Result<SessionSummary, String>` | trait 方法（core 定契约 / infra 实现）；查无此 id 显式 `Err`；返回 row + stats + turns，运行状态自轮行推导（D1） |
| `session_detail` | `src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn session_detail(stores: State<'_, WorkspaceStores>, root: String, session_id: String) -> Result<Option<SessionSummary>, String>` | tauri command + `#[specta::specta]`；blank root → `Ok(None)`；查无 id → `Err` 透传（D3） |
| `workflow::write::decision_log` | `crates/core/workflow/src/write/decision_log.rs` | 新增 | `pub fn decision_log(layout: &Layout, change: &str, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String>` | 决策槽位挂账：相位最新 eval 条目 raw 定点改写，幂等覆写，无条目显式 `Err`（D6） |
| `workflow::write::phase_log` | `crates/core/workflow/src/write/phase_log.rs` | 修改 | `pub fn phase_log(layout: &Layout, change: &str, input: &PhaseLogInput) -> Result<PhaseLogOutcome, String>` | 签名不变；随 `PhaseLogInput` 槽位扩展显式在位落账（D5） |
| `useAgentRunHistory` | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 修改 | `function useAgentRunHistory(root: string \| null, source: AgentHistorySource): AgentRunHistoryState` | 第二参显式必填（默认值由调用方 `AgentDebugView` 持 `'debug'`），`'all'` → `null` 不过滤（D9） |
| `AgentRunHistory` | `packages/desktop/src/views/agent/components/agent-run-history.tsx` | 修改 | `function AgentRunHistory(props: AgentRunHistoryProps): React.JSX.Element` | props 增 `source` / `onSourceChange`；筛选按钮组渲染（D9） |
| `useSessionTranscript` | `packages/desktop/src/views/changes/hooks/use-session-transcript.ts` | 修改 | `function useSessionTranscript(params: { root: string \| null; sourceRef: string \| null; sessionId: string \| null; liveEvents: AgentEvent[] }): UseSessionTranscriptResult` | 直查优先 / 反查兜底 / 双 null 空态三态寻址（D8）；返回面不变 |

<!-- `walk_run` / `run_worker` / `decision_session` / `step_verdict_phase_log` / `step_fail_phase_log`（私有步执行体，签名变化在 walker 修改行说明）、`session_query` 工厂、`agent_sessions` / `agent_session_transcript`（签名零变化，proposal「不要修改」）、`DetailDrawer` / `SessionTranscriptPanel` / `RunStepNode` / `ChangeFlowGraph`（签名零变化，改动在 props / data 面，见类型定义）不列。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `SessionQuery` | `crates/core/agent/src/port.rs` | 修改 | trait 增 `find_session_detail`（方法签名见公共函数表）；其余方法零改动 |
| `PhaseLog` | `crates/core/workflow/src/model/workflow.rs` | 修改 | 追加 `executor_session_id` / `evaluator_session_id` / `decision_session_id: Option<String>` 三字段（`#[serde(default, alias = ...)]`）；磁盘不出线（`Type` 不 derive 口径不变） |
| `PhaseLogInput` | `crates/core/workflow/src/write/phase_log.rs` | 修改 | 追加三 `Option<String>` 槽位；walker verdict 条目携 executor + evaluator、fail 条目仅 executor、decision 恒 None（decision 走 `decision_log` 单点） |
| `DecisionLogOutcome` | `crates/core/workflow/src/write/decision_log.rs` | 新增 | `pub struct DecisionLogOutcome { pub phase: String }`（沿 `PhaseLogOutcome` / `BacktrackOutcome` 产出形状惯例） |
| `ToolCommand` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `DecisionLog { change: String, phase: String, session_id: String }` 变体 |
| `ToolStepOutput` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `DecisionLog(DecisionLogOutcome)` 变体 |
| `AttemptRecord` | `crates/core/workflow/src/queries/detail.rs` | 修改 | 追加三 `Option<String>` 字段；wire 键 `executorSessionId` / `evaluatorSessionId` / `decisionSessionId` 恒在场、无槽位 `null`（纯 derive 零字段属性） |
| `AgentHistorySource` | `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 新增 | `type AgentHistorySource = 'debug' \| 'change' \| 'all'`；`'all'` 在取数层映射 `null`（不过滤） |
| `AgentRunHistoryProps` | `packages/desktop/src/views/agent/components/agent-run-history.tsx` | 修改 | 增 `source: AgentHistorySource` 与 `onSourceChange: (source: AgentHistorySource) => void` |
| `RoleSessionRef` | `packages/desktop/src/views/changes/flow/types.ts` | 修改 | `sourceRef` / `sessionId` 双键可空：sessionId 在场直查优先；sourceRef 兜底反查；decision 槽位缺席双 null → 空态不查询 |
| `RunStepNodeData` | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 修改 | 增 `onOpenSession?: () => void`（仅 workerAgent 节点由图层注入） |

<!-- bindings 侧 `commands.sessionDetail` 与 `AttemptRecord` TS 类型为生成物，随 D7 重导体现，不单列；`SessionSummary` / `SessionRow` / `TurnSummary` / `SessionStats` 复用零变化不列。 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string（0.4.3 → 0.4.4） | 用户可见变更版本提升；**归档轨道**（AC-11 / D11），随归档提交执行，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `workflow.json` eval 条目（`PhaseLog` 磁盘形状，修改） | 既有字段全集 + 新增可选键 `executor_session_id` / `evaluator_session_id` / `decision_session_id`（snake_case 磁盘键；**显式在位才写**，缺省槽位不产生键） | 条目经 `workflow::write` 写面落账 / 挂账（`phase_log` 随行写、`decision_log` 定点改写）；读面经宽松解析收敛同一类型 | 磁盘 `workflow.json`（raw Value 保形定点改写，既有键序与形状不破坏；`#[serde(default)]` 读兼容无槽位旧文件，历史文件零迁移） |
| `AttemptRecord` 详情线面（修改） | 既有字段全集 + `executorSessionId` / `evaluatorSessionId` / `decisionSessionId: string \| null`（wire 恒在场） | eval 条目 → 线面投影直读透出（一一条目一一投影，无派生改写、无回填）；前端经 bindings 类型消费 | golden 快照钉死（`tests/golden/*.json`，显式重写流程维护） |
| workspace 库会话域（`SessionRecord` / `AgentRunRecord` / `SessionEventRecord`，零变更） | 既有 schema 全集 | `find_session_detail` 以 `find_session` + `list_sessions` 既有公开方法组装单查应答 | redb workspace 库（按 root 隔离；store schema 零变更承诺不破） |
| `SessionSummary` 单查应答（复用，零变更） | `row: SessionRow` + `stats: SessionStats` + `turns: Vec<TurnSummary>` | `session_detail` IPC 应答体（`Option` 包裹：blank root → null）；运行状态由消费方自 `turns` 推导（有 running 轮行即 running） | 不持久化（IPC 瞬时面） |

**golden 预期 diff 清单（D10 人工确认的验收形态）**：

- 既有 12 份 change fixture 快照中，投影 `detail.pipeline[].attempts[]` 非空的条目对象各新增三键（`executorSessionId` / `evaluatorSessionId` / `decisionSessionId`，值恒 `null`——既有样本无槽位字段）；`attempts` 为空的投影与其余区块（detect / parse 摘要 / activePhase / interrupted / fileLog / artifacts）零变化。涉及面以「attempts 非空」为准（v1-b / v1-c / v2-a / v2-b 及含可解析 eval 条目的 corrupt 样本；v0-\* / v1-a / corrupt-invalid-json 无条目投影，预期零 diff）。
- `v3-a.json` 为新增文件（样本条目携非 null 槽位值，逐字等于 workflow.json 记录值）。
- layout-\* 三份为 list 投影（不经 `AttemptRecord`），零变化；`golden/README.md` 零变化。
- parse 摘要段零变化（本变更不新增投影统计字段）。

---

## 路由/API 设计

本变更不涉及 HTTP API；IPC 命令面（Tauri invoke 契约）单点扩展，既有命令零收窄：

| IPC 命令 | 输入 | 输出 | 变化 |
|----------|------|------|------|
| `session_detail` | `root: string, sessionId: string` | `SessionSummary \| null`（查无此 id → `Err(string)`） | 新增。语义：按 id 单查会话（行 + 聚合统计 + 轮行）；blank root → `null`（空结果，与 `agentSessions` / `agentSessionTranscript` 同口径）；跨 workspace 库隔离与既有查询命令一致（按 root 寻址所属库） |
| `agent_sessions` / `agent_session_transcript` / `agent_start` / `agent_stop` | 各自既有入参 | 各自既有出参 | 零变化（proposal「不要修改」：既有命令签名不收窄） |

认证：本机 IPC，无鉴权面（与既有命令同口径）。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 阶段一 D1 trait 方法 + D2 infra 两段式实现；`Err` / blank root / 跨库隔离 / running 推导的证明（port 测试 + store_port 测试含跨 workspace 库隔离）归 test-design / test-gen / test-execution 阶段承接 |
| AC-2 | 阶段二 D3 命令薄包装（三件事纪律）+ `all_commands!` 登记 + D7 bindings 重导（`sessionDetail`）；mod_test 覆盖归测试轨道 |
| AC-3 | 阶段七 D9：`AgentHistorySource` 三态参数化（默认 `'debug'`、`'all'` → null）、`agentSessions(root, source或null, null)` 重查、显式刷新模式不变（无轮询无订阅）；筛选 UI 锚点 `history-source-filter`；hook 测试归测试轨道 |
| AC-4 | 阶段三 D4 三槽位字段（`#[serde(default)]` 读兼容）+ D5 `PhaseLogInput` 显式在位落账（缺省不产键）；旧文件读解析与「serde 写出可被插件解析面读取」的 fixture 对照归测试轨道（插件侧 TS 写路径冻结不改，风险坐实则按 proposal 风险表回退另立变更） |
| AC-5 | 阶段四 D5 walker 取值传入（verdict 双槽位 / fail 仅 executor）+ D6 decision 挂账（`decision_log` amend 操作、收口即挂账、幂等覆写、不新增步状态词汇）；walker_test 覆盖归测试轨道 |
| AC-6 | 阶段五 D7 `AttemptRecord` 三槽位透出（纯 derive 零字段属性）+ bindings 再生成一致（diff 守卫绿）；旧条目三值 null 不报错由直读透出结构保证；detail_test 归测试轨道 |
| AC-7 | 阶段八 D8：eval 节点三转录 tab（沿 `transcript-role-tab` 锚点），decision 槽位缺席双 null → 空态不查询（MUST NOT 虚构 / 误挂的结构保证：类型面杜绝查询路径）；组件测试归测试轨道 |
| AC-8 | 阶段八 D8 直查优先 + sourceRef 兜底 + 直查 `Err` 错误态；`view-session` 显式入口与节点点击同一 `DrawerSelection`（无独立会话 route）；hook / 组件测试归测试轨道 |
| AC-9 | 阶段六 D10：`v3-a` 合成样本（三形态覆盖）+ fixtures README 清单行 + golden 显式重写流程覆写与 `v3-a.json` 新增；预期 diff 清单（数据模型节）人工逐份确认留痕；既有样本零改写；语料全量回归归 test-execution 阶段承接（测试轨道同步 `CHANGE_FIXTURES` 清单与目录一致性断言） |
| AC-10 | 阶段九守线为静态面（`server:check` 的 cargo fmt + clippy、`client:check` 的 vp check + knip 零新增豁免、bindings 重导幂等）；前端与 Rust 两套自动化套件的全绿验证由 test-execution 阶段承接，不入本设计任务面 |
| AC-11 | D11：version 0.4.3 → 0.4.4 随归档提交执行（归档轨道，配置表已标注，不在实现任务列表）；`tauri.conf.json` 自动跟随、`Cargo.toml` 不随动 |

---

## 依赖

### 运行时依赖

- 无新增 — 单查面复用 `store` 既有公开方法；槽位为纯语言内 `Option<String>`；前端复用 `bindings` 生成命令面与既有 React / tailwind 组件件，零新包

### 构建/测试依赖

- 无新增 — 沿 `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）、`client:check`（vp check + knip）、`bindings:export` / `bindings:check` 既有工具链；specta 出线与 export-bindings 重导流程既有；golden 重写开关 `DESKTOP_GOLDEN_REWRITE` 既有（corpus-regression 轨道建立）；cargo workspace 套件注册（src-tauri 根）零触达

---

## 待决问题

- `src/bindings/mod_test.rs` 的 `COMMAND_NAMES` / `DTO_TYPES` 断言清单现状未收录 `change_flow_*` 系列与 `create_change`（疑滞后于命令面扩张）——新命令 `session_detail` 与 `AttemptRecord` 类型名是否补录、按该文件现行维护口径处理，由 test-design 轮核实后定（不影响实现轨道）。
- 留痕（非待决）：decision 挂账以「会话 completed 收口」为前提——失败 / 停止收口的 decision 会话不入槽位，其转录仍在库内（`source="change"`），经 AC-3 的调试页 change 筛选可查，属已知可达边界而非缺口。
- 留痕（非待决）：`session_detail` 的悬挂 id（槽位 id 指向已不存在会话，如跨 root / 库重置）呈转录错误态而非静默回退反查（D8）——回退仅限槽位缺席的旧数据，防「同 ref 多会话取最近一条」歧义回流。
