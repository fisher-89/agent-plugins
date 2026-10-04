# 任务: desktop-change-session-visibility

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件。`port_test.rs` / `store_port_test.rs` / `commands/exec/mod_test.rs` / `phase_log_test.rs` / `decision_log_test.rs` / `corpus_golden_test.rs`（`CHANGE_FIXTURES` 清单与语料一致性断言补 `v3-a`）/ `detail_test.rs` / `walker_test.rs` 假件适配与前端 hook / 组件测试均归测试轨道。顺序按依赖排列：core 单查契约 → 命令面 → workflow 槽位写面 → walker 传递 → 详情 DTO + bindings 重导 → 语料与 golden 对账 → 调试页筛选 → 流程图联动 → 守线。版本 bump 归档轨道（AC-11 / design D11），随归档提交执行，不在本列表。

## 阶段一：core 会话单查面（AC-1）

- [x] 修改 `packages/desktop/src-tauri/crates/core/agent/src/port.rs`：`SessionQuery` trait 增方法 `fn find_session_detail(&self, session_id: &str) -> Result<SessionSummary, String>`，方法 doc 钉死单查语义（查无此 id 显式 `Err`、返回 row + stats + turns 复用 `SessionSummary` 零新 DTO、运行状态自轮行推导——有 running 轮行即 running，design D1）；`list_sessions` / `transcript` / `reconcile_stats` 零改动
- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs`：`impl SessionQuery for StoreQuery` 增单查实现（design D2）——`store.find_session(session_id)` miss-fast 锚定存在性（`None` → `Err("会话不存在: id=…")`，不触全表）→ `store.list_sessions(None, None)` 收敛取 `row.id` 匹配单条（第二段未命中同词 `Err` 兜底）；仅调用 store 既有公开方法，`crates/infra/store/**` 零改动

## 阶段二：命令面与登记（AC-2）

- [x] 修改 `packages/desktop/src-tauri/src/commands/exec/mod.rs`：增 `session_detail` 命令（`#[tauri::command]` + `#[specta::specta]`，签名 `pub fn session_detail(stores: State<'_, WorkspaceStores>, root: String, session_id: String) -> Result<Option<SessionSummary>, String>`，design D3）——blank root 经 `is_blank_root` → `Ok(None)`；`stores.for_root(&root)` 寻址后 `agent_runtime::session_query(store).find_session_detail(&session_id).map(Some)`；命令体三件事纪律（参数转换 → 调用 → 错误映射），查无 id 的 core `Err` 透传不吞；既有四命令零改动
- [x] 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：`all_commands!` 宏追加 `$crate::commands::exec::session_detail` 条目（置于 `agent_session_transcript` 之后）；`main.rs` / `lib.rs` 本体零改动（builder 经同宏消费，design 修改文件表说明）

## 阶段三：workflow 会话槽位写面（AC-4）

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs`：`PhaseLog` 追加 `pub executor_session_id: Option<String>` / `pub evaluator_session_id: Option<String>` / `pub decision_session_id: Option<String>` 三字段，各带 `#[serde(default, alias = "executor_session_id")]` 等同型属性（`backtrack_to` 先例逐字同型，design D4）；字段 doc 钉死磁盘键名与显式在位语义；既有字段与宽松解析口径零改动
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs`：`PhaseLogInput` 追加三 `Option<String>` 槽位字段；`phase_log` 落账条目组装处对 `Some` 槽位以 raw snake_case 键 insert（`executor_session_id` / `evaluator_session_id` / `decision_session_id`），缺省槽位不产生键（沿 skipped / start_at 扩展字段先例，design D5）；attempt 推导 / verdict 推导 / 长度门 / 开相前置 / 落账清 `active_phase` 零改动
- [x] 新增 `packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs`：`pub fn decision_log(layout: &Layout, change: &str, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String>` + `pub struct DecisionLogOutcome { pub phase: String }`（design D6）——`load_doc` 后以 `latest_entry_index` + `entry_phase` 锚定该相位最新 eval 条目（无条目显式 `Err`，不做表位校验），`as_object_mut` raw 定点改写 `decision_session_id` 键（幂等覆写），`save` 保形写回；模块 doc 钉死锚定 / 幂等 / 「仅写面不改写槽位值」纪律，措辞零双禁令字面量
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`：挂载 `mod decision_log;` + `pub use decision_log::{decision_log, DecisionLogOutcome};` + 测试模块声明；模块 doc「schema 形状零新字段（W3）」措辞演进为「唯一例外 = eval 条目会话槽位字段（desktop-change-session-visibility 裁定）」

## 阶段四：walker 槽位传递与 decision 挂账（AC-5）

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`：`ToolCommand` 增 `DecisionLog { change: String, phase: String, session_id: String }` 变体、`ToolStepOutput` 增 `DecisionLog(DecisionLogOutcome)` 变体（design D6）；既有变体与注释零改动
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`：`execute` 增 `DecisionLog` 臂——layout 解析后 `workflow::write::decision_log(&layout, &change, &phase, &session_id)` 进程内直调 → `ToolStepOutput::DecisionLog`（写通道唯一不变，design D6）；其余臂零改动
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：drive() 将 executor 会话 id 以 `Option<String>` 携出 `if let Some(executor)` 块作用域（static-check 反馈边续注同会话，id 稳定）；`step_verdict_phase_log` 增 executor 槽位入参，`PhaseLogInput` 携 executor + evaluator 双槽位（evaluator 取 `outcome.session_id`，decision 恒 `None`）；`step_fail_phase_log` 增 executor 槽位入参（`FeedbackLoop.executor_session` 透传），fail 条目仅携 executor 槽位（design D5）；`decision_session` 增 `tools: &Arc<dyn ToolStepPort>` 参，每轮 `run_worker` completed 收口后、`parse_decision` 之前经 `run_tool::<DecisionLogOutcome>(…, ToolCommand::DecisionLog { … }, "decision-log")` 挂账（parse 失败路径同样留痕；ask 续轮同会话同值幂等，design D6）；ask 应答循环 / 取消观测 / 白名单与重试门 / 终态收口零改动，零新 `ChangeStepKind` / `RunUpdate` 词汇（`run-state.ts` 零触点）；新注释措辞零双禁令字面量

## 阶段五：详情 DTO 与 bindings 重导（AC-6）

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：`AttemptRecord` 追加三 `Option<String>` 字段 + `From<&PhaseLog>` 逐字段直读透出（纯 derive 零字段属性口径不变，wire 键 `executorSessionId` / `evaluatorSessionId` / `decisionSessionId` 恒在场、无槽位 `null`，design D7）；`change_detail` 聚合逻辑零改动
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 重导 `packages/desktop/src/types/generated/bindings.ts`（design D7）：预期 diff 仅两处——Commands 段新增 `sessionDetail` 条目（入参 root / sessionId，返回 `SessionSummary | null`）、Types 段 `AttemptRecord` 增三字段；其余命令条目与 DTO 零变化

## 阶段六：语料样本与 golden 显式对账（AC-9）

- [x] 新增 `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/workflow.json`：requirement 形状合成样本，eval 三条目分别覆盖三槽位齐全（pass）/ 仅 executor 槽位（static-check 升格 fail 形态）/ 无槽位键（旧形态对照），磁盘键用 snake_case（design D10）
- [x] 新增 `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/proposal.md` 与 `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/v3-a/tasks.md`：最小文档集（沿 v2-b 合成样本形态）
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/README.md`：清单表补 `v3-a` 行（来源：合成样本（非归档快照）；覆盖点：eval 条目会话槽位三形态）；既有行零改写
- [x] 触发 golden 显式重写流程：以 `DESKTOP_GOLDEN_REWRITE=1` 环境变量启用重写开关（入口与流程见 `corpus_golden_test.rs` 头注）覆写 `packages/desktop/src-tauri/crates/core/workflow/tests/golden/` 既有 12 份快照并生成 `golden/v3-a.json`；随即关闭开关人工逐份 diff 确认符合 design「golden 预期 diff 清单」（仅 attempts 非空条目新增三 null 键 + `v3-a.json` 新增 + layout-\* / README 零变化 + parse 摘要零变化），确认记录留痕于本任务完成备注；既有 fixtures 样本文件内容零改写；重写后的复核对比与全量语料回归归 test-execution 阶段承接
  - 留痕（重写前快照全量 diff 人工确认）：被覆写 7 份 = v1-b（15 条目）/ v1-c（12）/ v2-a（5）/ v2-b（4）/ corrupt-bad-eval-entry（2）/ corrupt-bad-timestamp（1）/ corrupt-bad-verdict（1），diff 逐条目仅新增 `executorSessionId` / `evaluatorSessionId` / `decisionSessionId` 三 null 键，其余投影逐字节不变；v0-a / v0-b / v1-a / corrupt-invalid-json / layout-\* 三份 / golden README 零 diff（无 attempts 条目投影，与 design 预期清单一致）；parse 摘要段零变化。`golden/v3-a.json` 新增，三形态槽位值与 workflow.json 记录逐字一致（条目 1 三槽位齐全 / 条目 2 仅 executor / 条目 3 无槽位）。关开关复核 `cargo test -p workflow --test corpus_golden_test` 18/18 绿。注册配套：`corpus_golden_test.rs` 补 `CHANGE_FIXTURES` `v3-a` 条目 + golden 测试宏一行（语料一致性断言目录=清单的机械前提，与测试轨道补挂账用例不冲突）

## 阶段七：调试页来源筛选（AC-3）

- [x] 修改 `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts`：导出 `type AgentHistorySource = 'debug' | 'change' | 'all'`；`useAgentRunHistory` 增第二参 `source: AgentHistorySource` 下传 `useSessions`；取数改 `commands.agentSessions(root, source === 'all' ? null : source, null)`，effect 依赖增 `source`（design D9）；`AgentRunHistoryState` 返回面、`useReplay` 重放链路、显式刷新（tick）零改动，无轮询无订阅
- [x] 修改 `packages/desktop/src/views/agent/agent-debug-view.tsx`：`useState<AgentHistorySource>('debug')` 持有筛选状态，下传 `useAgentRunHistory(root, source)` 与 `<AgentRunHistory state={history} source={source} onSourceChange={setSource} />`（design D9）；表单 / 流视图切换 / 聊天会话面零改动
- [x] 修改 `packages/desktop/src/views/agent/components/agent-run-history.tsx`：`AgentRunHistoryProps` 增 `source: AgentHistorySource` 与 `onSourceChange: (source: AgentHistorySource) => void`；头行「刷新历史」左侧增三态按钮组（debug / change / 全部，容器 `data-testid="history-source-filter"` + 每枚 `data-source`，aria-pressed 高亮沿 `StreamToggle` 先例，design D9）；列表 / 重放区 / 空态 / 错误态零改动，不引入 ui 新组件件

## 阶段八：变更流程图会话联动升级（AC-7 / AC-8）

- [x] 修改 `packages/desktop/src/views/changes/flow/types.ts`：`RoleSessionRef` 改 `{ role: FlowRoleLabel; sourceRef: string | null; sessionId: string | null }`，字段 doc 钉死三态寻址语义（sessionId 在场直查优先 / sourceRef 兜底反查 / decision 槽位缺席双 null → 空态不查询，design D8）；其余视图模型类型零改动
- [x] 修改 `packages/desktop/src/views/changes/hooks/use-session-transcript.ts`：入参增 `sessionId: string | null`——sessionId 在场走直查分支（`commands.sessionDetail(root, sessionId)` → `null`（blank root）呈空态、turns 有 running 轮行即 `running` 置真 → `commands.agentSessionTranscript(root, sessionId)` 重放；`Err` 呈错误态**不回退反查**）；sessionId 为 null 且 sourceRef 在场走既有 `agentSessions` 反查兜底（旧数据，行为与升级前一致）；双 null 清空态不发起查询（design D8）；`assembleTranscript` seq 归并、replayRef 合并底座、返回面零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/detail-drawer.tsx`：`selectionRoleRefs`——eval 节点 ref 组补 decision 第三项（`sessionId` 取 `node.record.decisionSessionId ?? null`、`sourceRef` 恒 null），executor / evaluator 增 `sessionId` 取 `record` 槽位；runtime 节点 ref 增 `sessionId: node.sessionId`（design D8）；三分节 / 遮罩关闭 / liveEvents 过滤零改动，单一交互入口不变（不建会话 route）
- [x] 修改 `packages/desktop/src/views/changes/flow/session-transcript-panel.tsx`：三会话 tab（执行 / 评估 / 决策，沿既有 `transcript-role-tab` 锚点与 `ROLE_LABEL` 词汇）；`RoleTranscript` 入参改双键透传（`sessionId` / `sourceRef`）；tab key 改 `ref.sessionId ?? ref.sourceRef ?? ref.role`（design D8）；`AgentTimeline` 复用、空态 / 错误态呈现零改动，不建第二套时间线
- [x] 修改 `packages/desktop/src/views/changes/flow/run-step-node.tsx`：`RunStepNodeData` 增 `onOpenSession?: () => void`；`group === 'workerAgent'` 节点渲染「查看会话」按钮（`data-testid="view-session"`，`onClick` stopPropagation 后上抛，design D8）；ToolStep / Gate 节点无入口；步状态视觉 / 徽章 / Handle 零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/change-flow-graph.tsx`：`toChartNodes` 对 `kind === 'runtime' && group === 'workerAgent'` 节点注入 `data: { node, onOpenSession: () => onSelect({ scope: 'node', nodeId: node.id }) }`（与节点点击同一 `DrawerSelection`，design D8）；`onNodeClick` / 边锚定 / translateExtent 零改动

## 阶段九：守线与静态自查（静态，不含测试执行）

- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy；cfg(test) 测试代码不在默认编译面，测试构造点适配由测试轨道承接）与 `pnpm -C packages/desktop run client:check`（vp check + knip，零新增豁免条目）；前端与 Rust 两套自动化套件的全绿验证由 test-execution 阶段承接（AC-10 执行面），不入本任务面
  - 留痕：既有前端共置测试文件 9 份做了类型级最小适配（attempt 假件构造器补三 null 字段 / `useAgentRunHistory` 调用补 `'debug'` 第二参 / `AgentRunHistory` 渲染补 props / `RoleSessionRef` 字面量补 `sessionId: null` / `use-session-transcript.test.ts` Params 补 sessionId），使 vp check 类型面恢复编译绿；其中 detail-drawer / change-detail-view 共 4 个用例断言升级前行为（runtime 节点 sourceRef 反查 / eval 双 tab），属被本变更有意取代的语义，改写归测试轨道（test-design 已覆盖直查优先 / 三 tab 用例）
- [x] bindings 重导幂等自查：再跑 `pnpm -C packages/desktop run bindings:export` 零新增 diff，`bindings.ts` 变化仅 `sessionDetail` 条目 + `AttemptRecord` 三字段（design D7）
- [x] 静态自查：禁改面零 diff——`crates/infra/store/**`（store schema 零变更）、前端 `flow/run-state.ts`、`lib/agent-adapter.ts`、`hooks/use-agent-chat.ts`、`components/agent`（`AgentTimeline`）、既有 fixtures 样本文件内容（v0-a … v2-b 及 corrupt-\*）、`plugins/dev-team/**`（插件侧 TS 写路径冻结）、`PIPELINE_PHASES` / 9 列布局 / 边推导、既有命令签名（`agent_sessions` / `agent_session_transcript` 零收窄）；新增 `.rs` 注释零双禁令字面量命中（磁盘域根目录名与配置文件名，foundation layout 命名隔离扫描口径）
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏）；`packages/desktop/package.json` 实现期零触点（version 0.4.3 → 0.4.4 归档轨道执行，AC-11 / design D11）
  - 留痕：清单外触达仅两类——共置前端测试文件类型适配（上条留痕）与 `corpus_golden_test.rs` v3-a 注册（阶段六留痕），均为静态检查 / golden 对账的机械前提，非功能实现面扩张
