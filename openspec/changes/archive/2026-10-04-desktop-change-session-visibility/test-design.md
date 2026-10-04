# 测试设计: desktop-change-session-visibility

> **日期**: 2026-10-03

---

## 验收范围

<!-- 逐条映射 proposal.md 验收标准；`被测文件或模块` = 承载用例的测试文件（单值）。
     跨模块组合用例挂靠链路入口模块 per-file 章节（2.10.44 模板无集成测试层）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | core `SessionQuery` trait 增单查方法；实现按 session id 返回会话行 + 聚合统计 + 轮行，运行状态可自轮行推导（有 running 轮行即 running）；不存在 id 显式 `Err`、blank root 空结果；port 测试与 store_port 测试覆盖（含跨 workspace 库隔离） | `packages/desktop/src-tauri/crates/core/agent/src/port_test.rs` |
| AC-1 | （同上——单查实现半边：D2 两段式（`find_session` miss-fast 锚定 + 清单面收敛）、running 轮行推导、跨 workspace 库隔离） | `packages/desktop/src-tauri/crates/infra/agent/src/store_port_test.rs` |
| AC-2 | `session_detail(root, session_id)` 经 builder 注册、bindings 再生成；命令体三件事纪律（参数转换 → 调用 → 错误映射）；mod_test 覆盖 | `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` |
| AC-2 | （同上——bindings 出线半边：`sessionDetail` / `session_detail` 出线断言清单补录，重导幂等不破） | `packages/desktop/src-tauri/src/bindings/mod_test.rs` |
| AC-3 | Agent 调试页历史区提供 debug / change / 全部三态筛选，默认 debug；切换后按 `agentSessions(root, source或null, null)` 重查，显式刷新模式不变（无轮询无订阅）；hook 测试覆盖 | `packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts` |
| AC-3 | （同上——三态按钮组 UI 半边：`history-source-filter` + `data-source` + aria-pressed 高亮） | `packages/desktop/src/views/agent/components/agent-run-history.test.tsx` |
| AC-3 | （同上——筛选状态页面持有与下传半边：`useState<AgentHistorySource>('debug')` → hook 取数链路） | `packages/desktop/src/views/agent/agent-debug-view.test.tsx` |
| AC-4 | `PhaseLog` 模型增 executor / evaluator / decision 三槽位可选字段；`phase_log` 写面随 `PhaseLogInput` 落账（显式在位才写）；旧 workflow.json（无槽位字段）读解析不报错；serde 写出仍可被插件解析面读取（fixture 对照绿） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs` |
| AC-5 | `step_verdict_phase_log` / `step_fail_phase_log` 从 `WorkerTurnOutcome.session_id` 取值随输入落账；decision 会话收口后其 id 记录在案（写挂时机按 design 定稿实现）；walker_test 覆盖 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-5 | （同上——decision 挂账写面操作半边：`decision_log` 最新条目定点改写 / 幂等覆写 / 无条目显式 `Err`） | `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log_test.rs` |
| AC-6 | `AttemptRecord` 暴露三槽位会话 id；无槽位的旧条目三值均 null 不报错；bindings 再生成一致（diff 守卫绿）；detail_test 覆盖 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs` |
| AC-7 | eval 节点抽屉呈 executor / evaluator / decision 三转录 tab；decision 槽位无记录时该 tab 呈空态，MUST NOT 虚构会话或误挂他 attempt 会话；组件测试覆盖 | `packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx` |
| AC-7 | （同上——`selectionRoleRefs` 补 decision 第三 ref 半边：槽位直读 + decision `sourceRef` 恒 null） | `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` |
| AC-8 | 有槽位 id 时转录面板按 session id 直查（`session_detail` + `agent_session_transcript`），无槽位回退 sourceRef 反查（旧数据兜底）；WorkerAgent 节点提供显式「查看会话」入口且打开同一抽屉转录联动；hook / 组件测试覆盖 | `packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts` |
| AC-8 | （同上——抽屉联动接线半边：runtime 节点 sessionId 直查键 + 旧条目反查兜底 + decision 双 null 空态） | `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` |
| AC-8 | （同上——「查看会话」按钮半边：workerAgent 节点渲染 + stopPropagation 上抛） | `packages/desktop/src/views/changes/flow/run-step-node.test.tsx` |
| AC-8 | （同上——`onOpenSession` 图层注入半边：仅 workerAgent 节点注入、与节点点击同一 `DrawerSelection`） | `packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx` |
| AC-9 | fixtures 增至少一个含会话槽位字段的 workflow.json 样本；golden 快照经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成后复核一致，diff 人工确认仅限新增槽位键；既有样本文件内容零改写；corpus 测试绿 | `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` |
| AC-10 | `pnpm -C packages/desktop run client:check`（fmt / lint / knip）与 `vp test`、`cargo test --workspace` 全绿，无新增豁免条目 | —（见不可测试项 1） |
| AC-11 | 归档提交将 `packages/desktop/package.json` version 0.4.3 → 0.4.4（用户可见变更）；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动 | —（见不可测试项 2） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test]、前端 vite-plus 纯函数/组件 mock 测试）。
     本变更 8 个既有 Rust 测试文件扩展 + 新建 decision_log_test.rs + 6 个既有前端测试文件扩展；
     fixtures / golden / bindings.ts / 纯类型等不建共置测试的面统一落「不可测试项」声明（含路由去向）。
     跨模块组合用例挂靠链路入口模块（发起方 / 最上层调用方）的 `#### 用例` 表，无独立集成测试章节。 -->

### packages/desktop/src-tauri/crates/core/agent/src/port.rs -> packages/desktop/src-tauri/crates/core/agent/src/port_test.rs

#### 待测功能

- SessionQuery::find_session_detail(session_id: &str) -> Result&lt;SessionSummary, String&gt;: trait 单查方法（查无此 id 显式 `Err`；返回 row + stats + turns 三件套，运行状态自轮行推导——D1）；`list_sessions` / `transcript` / `reconcile_stats` 零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 假 query 增 find_session_detail 适配 | 正向 | `FakeQuery` 增单查固定形态实现后，`Arc<dyn SessionQuery>` trait object 上调用可达：返回预置 `SessionSummary`（row/stats/turns 三件套逐字段），返回形态编译锚定（`SessionSummary` 复用零新 DTO——D1） | 新增 |
| 假 query 增 find_session_detail 适配 | 异常 | 假实现返回 `Err(String)`（查无此 id 记因）→ `Err` 原样传播不静默（单查严格语义的契约面——与清单空态区分，proposal 决策表拍板） | 新增 |
| 假 query 增 find_session_detail 适配 | 边界 | `assert_send_sync::<dyn SessionQuery>` 编译行随新方法继续成立（Send + Sync 边界不破）；既有三方法假实现签名零改动、既有用例族全绿（契约零收窄——proposal「不要修改」口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| SessionQuery 假实现（FakeQuery 增单查臂） | 入参例外（trait 本身即注入面，object safety 编译期锚定——既有装置沿用）：find_session_detail 返回预置 SessionSummary / 可编程 Err(String) | 全部行 |

### packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs -> packages/desktop/src-tauri/crates/infra/agent/src/store_port_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未单列该文件行（find_session_detail 的 infra 实现侧，D2）；
     待测面为 `impl SessionQuery for StoreQuery` 两段式单查（design 修改文件行声明）。 -->

- StoreQuery（impl SessionQuery::find_session_detail）: `find_session` 主键直查锚定存在性（miss → 显式 `Err`，不触全表）→ `list_sessions(None, None)` 收敛取单条聚合形状；第二段 find 未命中同词 `Err`（D2）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| StoreQuery 单查两段式 | 正向 | create_session + begin/finish 轮行 + append 密封转录后 `find_session_detail("ses-x")` 命中：row（id / provenance / 双时间戳）+ stats（轮数 / 墙钟 / token 现算）+ turns 全量随行，与 `list_sessions(None, None)` 对应条目逐字段全等（清单面是聚合形状的唯一既有路径——D2） | 新增 |
| StoreQuery 单查两段式 | 正向 | 含未收轮（begin 后未 finish）的会话单查：turns 含 status=Running 轮行——有 running 轮行即 running，应答无顶层状态字段（轮行清单为唯一状态事实源，与清单面 / 调试页同一推导式） | 新增 |
| StoreQuery 单查两段式 | 异常 | 查无此 id（空库 / 有库未见 id 两形态）：显式 `Err` 非空态——第一段 `find_session` miss-fast 即 `Err`，第二段类型完备性兜底同词 `Err`（不返回空 Vec、不吞错） | 新增 |
| StoreQuery 单查两段式 | 边界 | 空 session_id 入参（""）：`find_session` miss → 显式 `Err`（不 panic、不降级空态——单查语义对调用方错误早暴露） | 新增 |
| StoreQuery 单查两段式 | 边界 | 跨 workspace 库隔离：两个 Env 各自独立 redb 库文件，A 库会话经 A query 命中、经 B query 显式 `Err`（按 root 隔离的库边界——AC-1「含跨 workspace 库隔离」） | 新增 |
| StoreQuery 单查两段式 | 边界 | store schema 零触达回归：单查仅消费 `find_session` / `list_sessions` 既有公开方法，既有 store_port 用例族全绿（`crates/infra/store/**` 零改动——proposal「不要修改」） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| redb workspace 库（数据文件） | 进程边界：tempdir 真库 `Store::open_workspace` 装置（Env）真实组合，零 store mock；时间戳以显式 i64 实参注入（既有装置沿用） | 全部行 |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- session_detail(stores, root, session_id) -&gt; Result&lt;Option&lt;SessionSummary&gt;, String&gt;: tauri command + `#[specta::specta]`；blank root → `Ok(None)`（空结果）；查无 id → core `Err` 透传；`Some` 包裹经 `.map(Some)`（D3，三件事纪律）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| session_detail 命令三件事 | 正向 | seed 会话（已收轮 + 密封转录）后直调命令：`Ok(Some(SessionSummary))`——row / stats / turns 逐字段（参数转换 → `agent_runtime::session_query(store).find_session_detail` 调用 → 错误映射三件事齐备） | 新增 |
| session_detail 命令三件事 | 正向 | 运行中会话（begin 未收轮）单查：`Some` 应答 turns 含 running 轮行——命令面不加状态字段（消费方自轮行推导，AC-1 状态推导半边的命令面投影） | 新增 |
| session_detail 命令三件事 | 异常 | 查无此 id：`Err` 透传（core `Err` 原样——不吞成 `Ok(None)`，悬挂 id 不伪装空态，D3 拍板） | 新增 |
| session_detail 命令三件事 | 边界 | blank root（空串 / 空白串）：`Ok(None)` 空结果，`is_blank_root` 守卫先行于库寻址（与 `agent_sessions` / `agent_session_transcript` 同口径——AC-1 blank root 空结果半边） | 新增 |
| session_detail 命令三件事 | 边界 | 跨 workspace 库隔离：会话落 root A 库，经 root B 单查 → `Err`（`for_root` 按 root 寻址所属库，与既有查询命令一致） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri MockRuntime app | 运行环境边界：`tauri::test::mock_app()` manage 真实 WorkspaceStores 后直调命令函数（既有装置沿用，零 mock） | 全部行 |
| WorkspaceStores 数据文件 | 进程边界：tempdir Env 双根（data_dir + ws_root）+ `seed_sdk_session` / `append_session_events` 真实落库 fixture | 全部行 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未声明该源文件公共 API（`all_commands!` 为声明式宏登记单点）。
     本章节承载 design.md 待决问题指派 test-design 轮核实的结论：读 `src/bindings/mod_test.rs` 现状——
     ① `DTO_TYPES` 已含 `AttemptRecord`（无需补录类型名）；② `COMMAND_WRAPPERS` / `COMMAND_NAMES`
     尚未收录 `session_detail`——按该文件现行维护口径（命令面扩张由当轮变更补录自身命令，
     `agent_sessions` 等在册先例）补录两条；`change_flow_*` / `create_change` 缺录为既有滞后债，
     不在本变更范围（清单断言为 contains 语义，滞后不致红）。 -->

- all_commands! 宏 session_detail 条目: 登记单点追加（置于 `agent_session_transcript` 之后）；`invoke_handler` 与 specta builder 经同宏消费同源出线（design 修改文件行）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings 出线断言清单补录 | 正向 | `COMMAND_WRAPPERS` 增 `"sessionDetail"`、`COMMAND_NAMES` 增 `"session_detail"` 后，既有覆盖性用例（产物含全部命令包装名 / invoke 名 / DTO 类型名）自动涵盖新命令：产物含 `sessionDetail` 包装条目与 `"session_detail"` invoke 名（bindings 重导后断言绿——AC-2 bindings 再生成半边） | 新增 |
| bindings 出线断言清单补录 | 边界 | `AttemptRecord` 类型名已在 `DTO_TYPES` 在册（核实结论，无需补录）：三槽位字段出线由生成物幂等重导 + golden diff 守卫承载（AC-6「bindings 再生成一致（diff 守卫绿）」半边） | 新增 |
| bindings 出线断言清单补录 | 边界 | 补录后既有守线用例族全绿：连续两次导出逐字节一致、篡改产物重导恢复、产物无 `Result` 包装（Throw 模式）——`sessionDetail` 为同步直查命令，出线形态与 `codeStats` / `workspaceConfig` 同族直返 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| bindings.ts 产物文件 | 进程边界（文件系统，真实组合不 mock）：`export_bindings()` 真实重导 + `generated_path()` 真实读取 + 产物互斥锁串行（既有装置沿用） | 全部行 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log_test.rs

#### 待测功能

- workflow::write::decision_log(layout, change, phase, session_id) -&gt; Result&lt;DecisionLogOutcome, String&gt;: 决策槽位挂账——定位该相位最新 eval 条目（同 backtrack 的 `latest_entry_index` 锚定），raw 定点改写 `decision_session_id` 键；幂等覆写；无条目显式 `Err`；不做表位校验、不新增条目（D6）
- DecisionLogOutcome { phase }: 产出形状（沿 `PhaseLogOutcome` / `BacktrackOutcome` 惯例）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| decision_log 定点挂账 | 正向 | 同相位多条目（fail 1 + fail 2）挂账：最新条目 `decision_session_id` == 传入值、更早条目逐字节不动（`latest_entry_index` 锚定语义） | 新增 |
| decision_log 定点挂账 | 正向 | 返回 `DecisionLogOutcome { phase }` 逐字段（phase == 传入相位） | 新增 |
| decision_log 定点挂账 | 正向 | 挂账幂等：同值二连写后 workflow.json 字节逐位不变（ask 续轮同会话同值重挂的写面底座——D6） | 新增 |
| decision_log 定点挂账 | 边界 | 最新条目为 stale 条目仍被锚定改写（同 backtrack 锚定——stale 不出局）；覆写旧值（先挂占位值再挂新值）以新值生效 | 新增 |
| decision_log 定点挂账 | 边界 | 其余内容保形：既有 `executor_session_id` / `evaluator_session_id` 槽位、`backtrack_to` / `backtrack_reason`、checklist、未知键逐字节不动；pretty 写回可再读（2 空格缩进 + 尾换行）；file_log 零触碰 | 新增 |
| decision_log 定点挂账 | 异常 | 该相位无 eval 条目：显式 `Err` 且 workflow.json 字节零变化（不新增条目、不做表位校验——D6 纪律） | 新增 |
| decision_log 定点挂账 | 异常 | change 不存在（无 workflow.json）：显式 `Err` 零写入 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：TempWs tempdir 真盘 fixture 真实读写（`foundation::layout::resolve` 真实组合，零 mock）；「零写入」以调用前后字节比对断言（phase_log_test 装置先例） | 全部行 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs

#### 待测功能

<!-- 既有 PhaseLogInput struct 字面量需补三槽位 None 字段（编译机械适配，断言不变——非废弃断言）。 -->

- workflow::write::phase_log(layout, change, input): 签名不变；随 `PhaseLogInput` 三槽位扩展显式在位落账——仅 `Some` 槽位以 raw snake_case 键 insert（`executor_session_id` / `evaluator_session_id` / `decision_session_id`），缺省槽位不产生键（D5）；attempt / verdict 推导、report 长度门、开相前置零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_log 会话槽位显式在位落账 | 正向 | `PhaseLogInput` 携 executor + evaluator 双槽位（decision None）落账：条目落 `executor_session_id` / `evaluator_session_id` 两 raw 键、值逐字透传（写面不解释不改写——desktop-workflow-write-face spec 文字）；`decision_session_id` 键不在场 | 新增 |
| phase_log 会话槽位显式在位落账 | 正向 | 仅 executor 槽位（static-check 升格 fail 形态）：仅 `executor_session_id` 单键在场，其余两键不产生 | 新增 |
| phase_log 会话槽位显式在位落账 | 边界 | 三槽位全 None：条目形状与既有形态逐字段一致（零新键——显式在位纪律使「缺省槽位条目与既有形态一致」静态保证，AC-4）；既有 verdict / attempt / start_at / skipped 推导与约束零改动全绿 | 新增 |
| phase_log 会话槽位显式在位落账 | 边界 | 旧 workflow.json（eval 条目无槽位键）在场时落新条目：全文件经宽松解析不报错（`PhaseLog` `#[serde(default)]` 读兼容——旧条目槽位字段 None，AC-4 旧文件读解析半边）；未知字段保形 / file_log 零触碰既有口径不回归 | 新增 |
| phase_log 会话槽位显式在位落账 | 正向 | 落账产物经宽松解析面（`workflow::parse::parse_workflow_file`）读回：三槽位值逐字还原（alias 承接 snake_case 磁盘键——「serde 写出仍可被插件解析面读取」的 desktop 侧机械证明，AC-4 fixture 对照半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：TempWs tempdir 真盘 fixture 真实读写（既有装置沿用，零 mock）；「拒绝零写入」以字节比对断言 | 全部行 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表显式不列 walker 私有步执行体（walk_run / run_worker /
     decision_session / step_verdict_phase_log / step_fail_phase_log）；待测面取自 design 修改文件表
     walker.rs 行与 D5 / D6 定稿。既有 FakeWorker / FakeTools / TempRoot 装置沿用；
     PhaseLogInput 字面量机械适配三槽位字段（非废弃断言）。 -->

- step_verdict_phase_log / step_fail_phase_log（私有）: `WorkerTurnOutcome.session_id` → `PhaseLogInput` 槽位随行（verdict 条目携 executor + evaluator 双槽位；static-check 升格 fail 条目仅携 executor 槽位）；executor 会话 id 携出 `if let Some(executor)` 块作用域
- decision_session（私有）: 每轮 `run_worker` completed 收口后、`parse_decision` 之前经 `run_tool::<DecisionLogOutcome>` 挂账（`ToolCommand::DecisionLog`）；ask 续轮同会话同值幂等重挂；挂账失败显式 `Err` → run 失败；不新增步状态词汇

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walk_run → phase_log 会话槽位落账 | 正向 | 真实组合（LocalToolSteps + TempRoot 真盘）：verdict 条目落账后 workflow.json 条目含 `executor_session_id` / `evaluator_session_id` 两键，值与假引擎 sessions 序的 executor / evaluator 会话 id 逐一对应（取值自 `WorkerTurnOutcome.session_id`——AC-5）；`decision_session_id` 键不在场（verdict 条目 decision 恒 None，D5） | 新增 |
| walk_run → phase_log 会话槽位落账 | 正向 | static-check 反馈边超限升格（step_fail_phase_log）：fail 条目仅携 `executor_session_id` 单键（evaluator 未跑）；反馈修复轮复用同 executor 会话——槽位值续注不漂移 | 新增 |
| walk_run → phase_log 会话槽位落账 | 边界 | emit_step 词汇零新增：挂账全程更新流无新步状态信封（不 emit 步状态、`RunUpdate` / `ChangeStepKind` 无新词汇——run-state.ts 零触点的结构证明，proposal「不要修改」） | 新增 |
| walk_run → decision 挂账 → workflow.json | 正向 | 决策会话首轮 completed 收口：`parse_decision` 消费之前 workflow.json 最新 verdict 条目 `decision_session_id` == decision 会话 id（真实组合链 walk_run → LocalToolSteps DecisionLog 臂 → decision_log 写面——AC-5「收口后记录在案」；挂账先于决策解析，parse 失败路径同样留痕） | 新增 |
| walk_run → decision 挂账 → workflow.json | 正向 | ask 续轮：同会话（continue_session）收口后重挂同值幂等（DecisionLog 调用序两次、workflow.json 值不变——D6） | 新增 |
| walk_run → decision 挂账 → workflow.json | 异常 | 挂账失败（假写面 `fail_on("decision-log")` 注入 Err）：run 显式收敛 failed（写面严格语义——挂账失败不静默吞）；决策动作未达 `parse_decision` | 新增 |
| FakeTools 假写面 DecisionLog 臂适配 | 正向 | 假写面增臂：捕获 `ToolCommand::DecisionLog { change, phase, session_id }` 载荷逐字段、预录应答 `ToolStepOutput::DecisionLog(DecisionLogOutcome)`、可编程 Err（假件适配行——test-gen 装置扩展点） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorkerAgentPort 假引擎（FakeWorker） | 入参例外（port 注入依赖，既有装置沿用）：decision 角色预录产出 + sessions 序登记（(role, session_id) 捕获——槽位值对应断言的事实源）；decision 收口即 completed（挂账前提） | 「walk_run → decision 挂账」全部行与槽位对应断言 |
| ToolStepPort 假写面（FakeTools） | 入参例外（注入依赖）：增 DecisionLog 臂（捕获 + 预录 + fail_on 可编程 Err）；正向链路行不接假写面——LocalToolSteps 进程内直调真盘写面（写通道唯一） | 假件适配行与挂账失败注入行 |
| workflow.json（数据文件） | 进程边界：TempRoot tempdir 真盘（真实写面组合——决策挂账链零 CLI 子进程） | 「walk_run → phase_log / decision 挂账」全部正向与边界行 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- AttemptRecord（From&lt;&amp;PhaseLog&gt; 直读透出）: 追加 executor_session_id / evaluator_session_id / decision_session_id 三 `Option<String>` 字段；wire 键 `executorSessionId` / `evaluatorSessionId` / `decisionSessionId` 恒在场、无槽位 `null`（纯 derive 零字段属性口径不变——design 类型定义行 / D7）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AttemptRecord 三槽位透出 | 正向 | workflow.json 条目携三槽位键 → `detail.pipeline[].attempts[]` 记录三字段逐字直读（`From` 直读不派生不回填——值与条目记录一致） | 新增 |
| AttemptRecord 三槽位透出 | 正向 | 部分槽位形态（仅 `executor_session_id` 在场）：executor 字段承接、evaluator / decision 两字段 null（缺字段降级 null 不报错） | 新增 |
| AttemptRecord 三槽位透出 | 边界 | 旧条目（无槽位键）投影：三字段均 None，且 serde 序列化 wire 三键恒在场（值 null 不省略——golden 线面「缺省字段 null」契约；纯 derive 零字段属性口径不破，无 phases 分裂伴生类型） | 新增 |
| AttemptRecord 三槽位透出 | 边界 | 9 站流水线聚合零改动回归：既有同 phase 多 attempt 折叠 / 升序 / 全量字段用例族全绿（透出即排序归并既有路径——change_detail 聚合逻辑零变更） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workflow.json（数据文件） | 进程边界：TempWs tempdir 真盘 fixture（既有装置沿用，零 mock） | 全部行 |

### packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a（语料增量 + tests/golden/ 快照对账物） -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表未声明该面公共 API（fixtures / golden 为入仓只读语料与快照对账物）；
     待测面取自 design 修改文件行与 D10。 -->

- CHANGE_FIXTURES 清单: 补 `"v3-a"` 条目（语料一致性断言同步生效——fixtures 目录 ↔ 清单表 ↔ golden 文件集三方一致）
- golden_v3_a 投影对账: v3-a 全链路投影（判定 → 解析 → 详情 → 产物清单）与 `tests/golden/v3-a.json` 键排序规范化逐字节对比

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 语料清单与 golden 对账（AC-9） | 正向 | `CHANGE_FIXTURES` 补 `"v3-a"` 后语料完整性断言绿：fixtures 目录集 == 清单集、golden 文件集 == 语料集 + layout-\* + README（`v3-a.json` 新增在册） | 新增 |
| 语料清单与 golden 对账（AC-9） | 正向 | golden_v3_a 用例：v3-a 投影与 `tests/golden/v3-a.json` 一致——eval 三条目覆盖三形态（三槽位齐全 pass 条目 / 仅 executor 槽位 fail 升格形态 / 无槽位键旧形态对照），非 null 槽位值逐字等于 workflow.json 记录值（`detail.pipeline[].attempts[]` 投影面，AC-9） | 新增 |
| 语料清单与 golden 对账（AC-9） | 边界 | 既有 12 份快照经 `DESKTOP_GOLDEN_REWRITE` 显式重写流程覆写后复核全绿：diff 人工确认仅限「attempts 非空条目新增三 null 键」（design 数据模型节预期 diff 清单——v1-b / v1-c / v2-a / v2-b 及含可解析 eval 条目的 corrupt 样本；v0-\* / v1-a / corrupt-invalid-json 预期零 diff）；既有 fixtures 样本文件内容零改写 | 新增 |
| 语料清单与 golden 对账（AC-9） | 边界 | layout-\* 三份 list 投影与 `golden/README.md` 零变化；parse 摘要段零变化（本变更不新增投影统计字段——design 数据模型节） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| fixtures / golden 语料文件 | 进程边界（数据文件，真实组合不 mock）：TempWs 拷贝真实语料 → 全链路投影 → 规范化 JSON 与入仓 golden 全量对比；`DESKTOP_GOLDEN_REWRITE` 为运行环境重写开关（复核轮不带开关运行） | 全部行 |

### packages/desktop/src/views/agent/hooks/use-agent-run-history.ts -> packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts

#### 待测功能

- useAgentRunHistory(root: string \| null, source: AgentHistorySource): AgentRunHistoryState——第二参显式必填（默认值由调用方 `AgentDebugView` 持 `'debug'`），`'all'` → `null` 不过滤（D9）
- AgentHistorySource: `'debug' \| 'change' \| 'all'`（新导出类型——取数层映射 null）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentRunHistory source 参数化 | 正向 | `(ROOT, 'change')` 挂载 → invoke agent_sessions 携 `{ root, source: 'change', sourceRef: null }`（AC-3 切换重查命令面）；sessions 承接清单 | 新增 |
| useAgentRunHistory source 参数化 | 正向 | `(ROOT, 'all')` 挂载 → agent_sessions 携 `source: null`（不过滤——映射 null，debug + change 混合清单原样承接渲染） | 新增 |
| useAgentRunHistory source 参数化 | 边界 | `(ROOT, 'debug')` 挂载 → agent_sessions 携 `source: 'debug'`（默认档形态与既有断言逐字一致——现状不变） | 新增 |
| useAgentRunHistory source 参数化 | 正向 | source 变化（rerender `'debug'` → `'change'`）→ effect 重查：agent_sessions 按新 source 重发（effect 依赖增 source——D9） | 新增 |
| useAgentRunHistory source 参数化 | 边界 | refresh() 重取携当前 source（显式刷新模式不变）；挂载取数稳定后静置无新增 agent_sessions 调用（无轮询无订阅回归——AC-3） | 新增 |
| useAgentRunHistory source 参数化 | 废弃 | 单参调用形态 `useAgentRunHistory(root)` 废弃——第二参显式必填；既有用例调用点适配为 `(root, 'debug')`（断言不变：非死断言，仅调用形收敛） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：vi.hoisted + vi.mock 的 invokeMock 按命令名分发、记录入参（既有装置沿用） | 全部行 |
| bindings commands 模块 | 真实组合（不 mock）：`commands.agentSessions` 真实生成物参与——source 映射 null 的布线经真实生成物到达 invoke 载荷 | 全部行 |

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无 agent-debug-view.tsx 行（页面组件签名零变化）；
     待测面取自 design 修改文件行与 D9。 -->

- 筛选状态 useState&lt;AgentHistorySource&gt;('debug'): 页面持有并下传 `useAgentRunHistory(root, source)` 与 `<AgentRunHistory state={history} source={source} onSourceChange={setSource} />`（D9 落点定稿——状态提升到页面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentDebugView 来源筛选接线 | 正向 | 页面挂载（真实 useAgentRunHistory 组合 + mock invoke）：`history-source-filter` 在场且 debug 枚高亮；agent_sessions 以 `source: 'debug'` 取数（默认现状不变——AC-3） | 新增 |
| AgentDebugView 来源筛选接线 | 正向 | 点击 change 枚：onSourceChange 状态提升生效 → agent_sessions 按新 source 重查（筛选状态 → hook 取数链路，页面级真实组合） | 新增 |
| AgentDebugView 来源筛选接线 | 边界 | 点击全部枚：agent_sessions 携 `source: null`，混合来源清单渲染不炸（列表承接零新概念——D9） | 新增 |
| AgentDebugView 来源筛选接线 | 边界 | 表单 / 流视图切换 / 聊天会话面零改动回归：既有页面用例族装置补筛选接线后全绿 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke + Channel） | 进程边界（IPC）：invokeMock 按命令名分发 + ChannelMock class 捕获 onmessage（既有页面装置沿用） | 全部行 |
| useAgentChat / useAgentRunHistory 内部 hook | 真实组合（不 mock——最小 mock 原则既有修订口径）：fixture 经 mock invoke 流入真实 hook，页面断言渲染输出与 invoke 入参而非 hook 内部 | 全部行 |

### packages/desktop/src/views/agent/components/agent-run-history.tsx -> packages/desktop/src/views/agent/components/agent-run-history.test.tsx

#### 待测功能

- AgentRunHistory(props: AgentRunHistoryProps): props 增 `source: AgentHistorySource` / `onSourceChange: (source: AgentHistorySource) => void`；头行三态按钮组（debug / change / 全部，`data-testid="history-source-filter"` + 每枚 `data-source`，aria-pressed 高亮沿 StreamToggle 先例）（D9）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunHistory 来源筛选按钮组 | 正向 | `source='debug'` 挂载：`history-source-filter` 内三枚按钮 `data-source` 为 debug / change / all，debug 枚 aria-pressed 高亮（AC-3 data-testid 挂钩） | 新增 |
| AgentRunHistory 来源筛选按钮组 | 正向 | 点击 change 枚 → `onSourceChange('change')` 恰调用一次（组件无状态——状态提升断言面，D9 落点定稿） | 新增 |
| AgentRunHistory 来源筛选按钮组 | 边界 | `source='all'` 挂载：all 枚 aria-pressed（高亮随 props 切换，组件不自持状态） | 新增 |
| AgentRunHistory 来源筛选按钮组 | 边界 | 列表 / 重放区 / 空态 / 错误态零改动回归：既有用例族 `mount()` 装置补 source / onSourceChange 两 prop 后全绿（不引入 ui 新组件件——design 修改文件行） | 新增 |
| AgentRunHistory 来源筛选按钮组 | 废弃 | 无 source / onSourceChange 的旧 props 形态废弃——`AgentRunHistoryProps` 扩展后既有用例构造点适配（断言不变，仅装置补参） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| AgentRunHistoryState fixture 直传 | 入参例外（state 为组件显式 prop 注入依赖，既有装置沿用）：sessions / events / refresh / openSession 可编程；无进程边界 | 全部行 |
| onSourceChange spy | 入参例外（注入回调）：vi.fn 断言调用次数与实参 | 「来源筛选按钮组」正向行 |
| @testing-library/react | 真实 DOM 渲染：fireEvent.click 驱动按钮组（ui 层 Button 真实参与，不 mock 内部组件） | 全部行 |

### packages/desktop/src/views/changes/hooks/use-session-transcript.ts -> packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts

#### 待测功能

- useSessionTranscript(params: { root: string \| null; sourceRef: string \| null; sessionId: string \| null; liveEvents: AgentEvent[] }): UseSessionTranscriptResult——直查优先（`sessionDetail` → running 推导 → `agentSessionTranscript` 重放）/ 反查兜底（槽位缺席旧数据，行为与升级前一致）/ 双 null 清空态不查询 / 直查 `Err` 错误态不回退（D8）；返回面 `{ messages, running, error }` 不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useSessionTranscript 直查优先 | 正向 | sessionId 在场：invoke session_detail 携 `{ root, sessionId }` → 应答 turns 推导 running → invoke agent_session_transcript 携 `{ root, sessionId }` 重放装配 messages；agent_sessions 恰零调用（直查优先——有 id 不走反查，杜绝「有 id 仍走反查」漂移，D8） | 新增 |
| useSessionTranscript 直查优先 | 正向 | 直查应答轮行含 running：`running` 置真（面板头部流式状态与反查路径同一推导式） | 新增 |
| useSessionTranscript 直查优先 | 正向 | sessionId 变化（rerender）→ 直查新会话重放（选中联动，与既有 sourceRef 变化重查同型） | 新增 |
| useSessionTranscript 直查优先 | 正向 | 直查模式下 liveEvents seq 归并：以直查重放为底去重并入（assembleTranscript 语义与反查路径一致——返回面零改动） | 新增 |
| useSessionTranscript 直查优先 | 异常 | session_detail reject（悬挂 id——槽位 id 指向已不存在会话）：error 呈现 + messages 空，agent_sessions 恰零调用（直查 `Err` 不静默回退反查——回退仅限槽位缺席，防同 ref 多会话歧义回流，D8） | 新增 |
| useSessionTranscript 直查优先 | 边界 | sessionDetail 应答 null（blank root 空结果透传）：空态呈现，不发起 agent_session_transcript（D8） | 新增 |
| useSessionTranscript 反查兜底与空态 | 正向 | sessionId null + sourceRef 在场（executor / evaluator 槽位缺席旧数据）：回退既有反查链——agent_sessions(source='change', sourceRef) → transcript 重放（行为与升级前一致，既有反查用例族全绿） | 新增 |
| useSessionTranscript 反查兜底与空态 | 边界 | sessionId 与 sourceRef 双 null（decision 槽位缺席）：清空态零 invoke（不发起任何查询——MUST NOT 误挂他 attempt 会话的结构保证，AC-7 同源纪律） | 新增 |
| useSessionTranscript 反查兜底与空态 | 边界 | root null：零 invoke 空态（既有守卫先行于双键判断） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：invokeMock 按命令名分发（session_detail / agent_sessions / agent_session_transcript，可切 resolve / reject / 挂起并记录入参——既有 mockIpc 装置扩展 session_detail 臂） | 全部行 |
| agent-adapter | 真实组合（不 mock，内部模块）：eventsToUIMessages 真实参与折叠装配 | 全部行 |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无 detail-drawer.tsx 行（selectionRoleRefs 为模块内私有函数）；
     待测面取自 design 修改文件行与 D8。 -->

- selectionRoleRefs（私有）: eval 节点 ref 组补 decision 第三项（`sessionId` 取 `record.decisionSessionId`、`sourceRef` 恒 null），executor / evaluator 增槽位 `sessionId` 键；runtime 节点 ref 增 `sessionId = node.sessionId`（D8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| selectionRoleRefs 决策槽位联动 | 正向 | eval 节点选中（record 三槽位在场 fixture）：转录区呈三 tab（执行 / 评估 / 决策）；executor / evaluator 两路 invoke session_detail 直查（槽位 id 透传——AC-7 / AC-8） | 新增 |
| selectionRoleRefs 决策槽位联动 | 正向 | runtime 节点选中（workerAgent，node.sessionId 在场）：ref 携 sessionId 直查（运行步实时 id）；liveEvents 仍按该 sessionId 过滤下传（既有分桶语义不混入） | 新增 |
| selectionRoleRefs 决策槽位联动 | 边界 | 旧条目（record 三槽位均 null）：executor / evaluator 回退 agent_sessions sourceRef 反查；decision ref 双 null → 决策 tab 空态且零查询（不误挂他 attempt 会话——AC-7 降级面） | 新增 |
| selectionRoleRefs 决策槽位联动 | 边界 | eval 节点 attempt 为 null / runtime 节点 role 为 null：ref 组为空、无转录区（既有退化语义不回归） | 新增 |
| selectionRoleRefs 决策槽位联动 | 废弃 | RoleSessionRef 单键 `{ role, sourceRef }` fixture 形态废弃——双键可空化后既有用例 fixture 构造点适配（既有断言经 fixture 补 `sessionId: null` 后全绿，非死断言） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：invokeMock 按 sourceRef / sessionId 寻址的 fixture 注册表（既有装置沿用——反查与直查双寻址面扩展） | 全部行 |
| buildFlowGraph / mountMaterials / ArtifactView / FileLogTable | 真实组合（不 mock，内部模块）：图与素材面真实组装 | 全部行 |
| useSessionTranscript | 真实组合（不 mock——内部协作 hook 经 mock invoke 流入，抽屉断言渲染输出与 invoke 入参而非 hook 内部） | 转录区各行 |

### packages/desktop/src/views/changes/flow/session-transcript-panel.tsx -> packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无 session-transcript-panel.tsx 行（props / data 面改动）；
     待测面取自 design 修改文件行与 D8。 -->

- 三会话 tab: 执行 / 评估 / 决策（沿既有 `transcript-role-tab` 锚点，decision 标签为既有 `ROLE_LABEL` 词汇）；tab key 改 `ref.sessionId ?? ref.sourceRef ?? ref.role`；`RoleTranscript` 入参改双键透传（D8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| SessionTranscriptPanel 三会话 tab | 正向 | refs 携三 role（executor / evaluator / decision 槽位均在场）：三枚 transcript-role-tab（执行 / 评估 / 决策），默认选中执行；点击决策 tab → session_detail 直查其转录（sessionId 透传——AC-7） | 新增 |
| SessionTranscriptPanel 三会话 tab | 边界 | decision 双 null（槽位缺席）：决策 tab 呈 transcript-empty 空态、零查询、面板不渲染他 attempt 会话内容（MUST NOT 虚构 / 误挂——AC-7） | 新增 |
| SessionTranscriptPanel 三会话 tab | 边界 | tab key 回退链：executor 反查 ref（sessionId null + sourceRef 在场）与 decision 直查 ref（sessionId 在场）混合分页切换互不串页（key 唯一性——`sessionId ?? sourceRef ?? role`） | 新增 |
| SessionTranscriptPanel 三会话 tab | 异常 | 直查 reject → transcript-error 呈现（与反查 reject 同错误态锚点） | 新增 |
| SessionTranscriptPanel 三会话 tab | 边界 | AgentTimeline 复用 / liveEvents 透传 / timeline-running 标记既有用例族：fixture 补双键后全绿（无第二套时间线回归——proposal「不要修改」） | 新增 |
| SessionTranscriptPanel 三会话 tab | 废弃 | ① decision 经 sourceRef 反查的既有用例形态废弃（D8 decision `sourceRef` 恒 null——反查查无此 ref，槽位在场直查 / 缺席空态二态收口）；② RoleSessionRef 单键 fixture 形态废弃（双键化构造点适配） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke） | 进程边界（IPC）：invokeMock 按 sourceRef / sessionId 双寻址注册表（既有装置扩展 session_detail 臂） | 全部行 |
| useSessionTranscript / AgentTimeline | 真实组合（不 mock，内部模块）：fixture 经 mock IPC 流入真实 hook 与时间线 | 全部行 |

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

- RunStepNodeData（design 类型定义行）: 增 `onOpenSession?: () => void`（仅 workerAgent 节点由图层注入）；`group === 'workerAgent'` 节点渲染「查看会话」按钮（`data-testid="view-session"`，onClick stopPropagation + 上抛）（D8 / proposal 决策 Q1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunStepNode 查看会话入口 | 正向 | workerAgent 节点 + onOpenSession 注入：view-session 按钮渲染；点击恰一次上抛（stopPropagation——不冒泡至节点点击面，AC-8） | 新增 |
| RunStepNode 查看会话入口 | 边界 | onOpenSession 缺省（undefined）：不渲染按钮（三分类九步词汇既有渲染零回归） | 新增 |
| RunStepNode 查看会话入口 | 边界 | toolStep / gate 节点：无 view-session 按钮（沿用右侧抽屉步骤结果呈现——ToolStep / Gate 无入口，design 修改文件行） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| onOpenSession spy | 入参例外（注入回调）：vi.fn 断言调用次数 | 「查看会话入口」正向行 |
| ReactFlowProvider | 真实组合（运行环境——Handle 依赖 ReactFlow store context，沿 flow-event-node 先例外包 Provider，不 mock） | 全部行 |

### packages/desktop/src/views/changes/flow/change-flow-graph.tsx -> packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无 change-flow-graph.tsx 行（toChartNodes 为模块内装配函数）；
     待测面取自 design 修改文件行与 D8。 -->

- toChartNodes 注入: 仅对 `kind === 'runtime' && group === 'workerAgent'` 节点注入 `data.onOpenSession = () => onSelect({ scope: 'node', nodeId: node.id })`（与节点点击同一 `DrawerSelection`——D8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeFlowGraph 查看会话入口注入 | 正向 | workerAgent 运行节点渲染 view-session 按钮；点击 → `onSelect({ scope: 'node', nodeId: node.id })` 与节点点击同参（打开同一抽屉转录联动——AC-8「打开同一抽屉」，无独立会话 route） | 新增 |
| ChangeFlowGraph 查看会话入口注入 | 边界 | toolStep / gate 节点 data 无 onOpenSession 键（仅 workerAgent 注入——与 run-step-node 装置一致不渲染按钮） | 新增 |
| ChangeFlowGraph 查看会话入口注入 | 边界 | 注入后图结构逐项不变：节点集 / 边集 / `PIPELINE_PHASES` 布局与既有断言一致（图结构零触达——proposal「不要修改」） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ResizeObserver / DOMMatrixReadOnly / getBBox | 运行环境垫片（jsdom 缺口 stub 而非业务 mock，既有装置沿用）：节点度量真实发生 | 全部行 |
| onSelect spy | 入参例外（注入回调）：vi.fn 断言 DrawerSelection 载荷 | 「入口注入」正向行 |
| buildFlowGraph / mountMaterials | 真实组合（不 mock）：图数据一律真实产出 | 全部行 |

---

## 不可测试项

- AC-10 全管线回归（client:check 的 fmt / lint / knip 零新增豁免 + 前端 `vp test` 与 `cargo test --workspace` 全绿） — **原因**: 执行聚合面——由本文件各章节用例全集在 Rust / 前端自动化套件运行时综合承载（test-execution 阶段承接），无独立被测单元与新增用例；design.md 验收标准对齐同口径（阶段九守线为静态面）。
- AC-11 版本交付（归档轨道） — **原因**: 归档提交将 `packages/desktop/package.json` version 0.4.3 → 0.4.4 为人工归档动作（`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动），非可执行代码单元，无自动化断言面（auto-confirm AC-6 同型先例）。
- `packages/desktop/src/types/generated/bindings.ts` 生成物 — **原因**: specta 重导生成物非手写可测单元，不建共置 `bindings.test.ts`（防空套件红灯，test_resolve_paths 解析路径不采纳）；出线断言由 `src/bindings/mod_test.rs` 补录承载（见「commands/mod.rs -> bindings/mod_test.rs」章节），字段级线面由 detail_test.rs 序列化断言与 golden 快照守卫。
- `packages/desktop/src/views/changes/flow/types.ts` 纯类型（`RoleSessionRef` 双键化） — **原因**: 纯类型模块无独立运行时行为，不建 `types.test.ts`（test_resolve_paths 解析路径不采纳）；双键寻址语义（直查优先 / 反查兜底 / decision 双 null 空态 / tab key 回退链）由 use-session-transcript / session-transcript-panel / detail-drawer 用例经真实组合覆盖。
- `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs`（`PhaseLog` 三槽位 serde 字段） — **原因**: test_resolve_paths 解析出 `model/workflow_test.rs` 但该文件不存在且本变更不建（proposal 测试文件清单亦未列）；serde 读兼容（`#[serde(default)]` + alias 承接 snake_case 磁盘键）的行为证明由 phase_log_test.rs 解析读回用例与 corpus v3-a golden 承载。
- `packages/desktop/src-tauri/src/commands/mod.rs`（`all_commands!` 宏登记） — **原因**: test_resolve_paths 解析出 `commands/mod_test.rs` 但该文件不存在且本变更不建（仓库无命令宏直测先例）；登记出线由 `bindings/mod_test.rs` 补录断言承载（宏经同源消费的 specta builder 出线）。
- `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`（`decision_log` 模块挂载 + doc 措辞演进） — **原因**: test_resolve_paths 解析出 `write/mod_test.rs` 但该文件不存在且本变更不建；挂载正确性由 decision_log_test.rs 经 `workflow::write::decision_log` 出口路径可达性证明；doc 措辞为文档约定非运行时行为。
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` / `steps.rs`（`ToolCommand::DecisionLog` 变体 + LocalToolSteps DecisionLog 臂） — **原因**: 二者解析出的 `port_test.rs` / `steps_test.rs` 均为既有文件且不在本变更测试文件面（design.md 测试文件面未列）；变体与命令臂经 walker_test.rs 真实组合链（walk_run → LocalToolSteps → decision_log → workflow.json）端到端覆盖，不另设章节。
- golden 既有 12 份快照的「diff 人工逐份确认留痕」（AC-9 人工确认半边） — **原因**: 人工评审行为不可自动化；自动化半边（显式重写后复核全绿 + 预期 diff 清单范围断言）由 corpus_golden_test.rs 章节承载。

