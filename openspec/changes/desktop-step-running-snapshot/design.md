# 设计: desktop-step-running-snapshot

> **变更**: desktop-step-running-snapshot
> **日期**: 2026-10-17
> **基于**: proposal.md、`specs/` 五能力 delta（desktop-change-orchestration / desktop-change-state-store / desktop-change-queries / desktop-change-flow-view / desktop-corpus-regression）

---

## 提案与规格同步状态

`proposal.md` 与五份能力 delta 已由提案阶段定稿（探索二轮四项决策用户拍板在案：token 口径路线 A 本步增量 / 接口返回起止时间前端算差 / 图内节点标题下常驻行 / 旧库读史显 "-" 不回填）；本设计不重复其内容，在 proposal 五项「待决问题」之上逐项定稿（D8 / D9 / D3 / D10 / D11），并补设计面决策（D1–D7 / D12）。

工作树实况（本变更起点，`packages/desktop` 0.5.10 基线，锚点已逐条核实）：

- **双 emit 单点结构成立**：walker 全部 44 个 `emit_node` 调用点均经 `StepSettle` 四构造器（`running()` / `passed()` / `failed()` / `stopped()`，`walker.rs:388-437`；终态构造器经 `..Self::running()` struct-update 展开）+ `with_session` / `with_output_ref` 两 builder；`phaseStart` 瞬步单 emit passed（`walker.rs:676`）；归档步经 `settle_archive_node` 包装同一 `emit_node`（`walker.rs:1771`）——铸造收 `emit_node` 单点即全 44 点零触点覆盖。
- **`now_millis()` 既有口径**：`walker.rs:3125`（收口时刻铸造点，注释明示「corpus 确定性由测试注入承载，本时钟只在生产路径消费」）；core/agent 侧同款 `agent::event::now_millis`（pub(crate)）。
- **corpus 确定性（proposal R6 / 待决 5）已核实解除**：`corpus_golden_test.rs::project_detail` 对 `ChangeDetail` 全量序列化（`serde_json::to_value`，runs[].nodes 在内）——新字段必进 golden（显式重写既定）；但种子路径经 `step` / `step_at` fixture **直构 `ChangeStepState` 字面量**（:1198-1232）喂 `finish_command` → store 操作面，**不经 `emit_node` / `now_millis`**——零 wall-clock 进 corpus，确定性成立。
- **落库加列先例**：`RunNodeRecord`（`model.rs:934`，native_model id=16 version=1）既有 `session_id` / `detail` / `output_ref` 三列即后期 additive 加列 + `#[serde(default)]` 同版本先例（:956-963）——四新列同款，零迁移。
- **token 求和现居**：`sum_usage_tokens`（`infra/store/src/store.rs:199`，12 行私有函数；消费点 :164-165 会话统计与 :870-871 对账纠偏）；`AgentEventKind::TurnDone.usage` 定义在 `core/agent/src/event.rs:96`；agent crate 依赖面仅 serde 系 + specta + tokio（serde_json 已在），函数仅吃 `&[AgentEvent]` 上移天然合规。
- **活面复用**：`ActiveRunView.nodes: Vec<orchestration::ChangeStepState>`（`src/commands/changes/mod.rs:20`）——字段自动跟随、命令面签名零改动；`RunNotice` kind-only 通知零载荷不变。
- **前端消费链**：`unifiedRunNodes(runs, liveRun)`（`run-state.ts:121`）双源合成 → `runStepNodes`（:186）→ `RuntimeFlowNode` → `RunStepNode`；`stepFromRecord`（:84）纯复制；`change-detail-view.tsx:233` 唯一调用点；前端全库无 `setInterval` 先例（走秒为首例）。

**关键既有红线（零触点核对结论）**：`StepRecord` 审计面与 `StepKind` 封闭集；`RunUpdate` 进程内 seam 与 IPC `RunNotice` kind-only 降位；`SessionStats` / `SessionSummary` / `sessionDetail` 契约；runs 级 `ChangeRunEntry.startedAt` / `finishedAt` ISO 串口径；`RunNodeRecord.timestamp` 列（落包时刻）语义；walker 路由语义 / 相位转移红线 / evaluator 输出协议 / 决策协议；检查域 spawn 与 vcs 步执行语义；crate 依赖方向（orchestration 零 infra 依赖、agent 禁 workspace 内依赖）；`plugins/dev-team` 全部（2.10.44）；`openspec/specs/**` 基线。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 契约单点扩展 | `ChangeStepState` 增四字段（`started_at: i64` / `ended_at: Option<i64>` / `token_input: Option<u64>` / `token_output: Option<u64>`，doc 注释携口径与合并语义指针）；活面 `ChangeRunSnapshot.nodes` / `ActiveRunView.nodes` 复用本类型自动跟随 | `crates/core/orchestration/src/state.rs` | crate 内 serde/specta | 活面契约单点（AC-1） |
| emit 铸造与 token 落值 | `StepSettle` 扩四字段（构造器中性，`with_tokens` builder 随行 token）；`emit_node` 时间铸造单点（running → `(now, None)`、终态 → `(now, Some(now))`，复用既有 `now_millis()` 零新时钟缝）；`run_worker` 终态 emit 前对 `outcome.transcript` 的 TurnDone.usage 求 in / out 两键之和单次落值 | `crates/core/orchestration/src/walker.rs` | `agent::sum_usage_tokens`（新增引用）、crate 内 | 44 个调用点零触点（铸造收单点；AC-2 / AC-3） |
| 合并语义 | `accumulate_node`：`started_at` 无条件保首见（终态行携带的同刻值被丢弃、瞬步单 emit 首见即真值）；`ended_at` / `token_input` / `token_output` 与 `session_id` 同款 some 才覆写；头注释措辞随动 | `crates/core/orchestration/src/control.rs` | crate 内 | 就地合并单点（AC-4） |
| token 求和单源 | `sum_usage_tokens` 自 infra/store 上移（函数体原样、签名 `(&[AgentEvent], &str) -> Option<u64>` 同款）；唯一实现居 `AgentEventKind::TurnDone` 定义所在 crate；orchestration / infra/store 两消费方同源引用 | `crates/core/agent/src/usage.rs`（新）+ `lib.rs` re-export | serde_json、crate::event | 依赖方向 infra → core 不可反向（AC-10） |
| store 消费方改引 | 删除私有 `sum_usage_tokens`，`use agent::{…, sum_usage_tokens}`（会话统计 :164 / 对账纠偏 :870 两消费点词面零变化，聚合行为零变化） | `crates/infra/store/src/store.rs` | core/agent（既有依赖） | 行为不变重构（AC-10） |
| 中性载荷与读面 | `RunNodeEntry`（finish 载荷内联条目）与 `RunNodeStateRecord`（port 读面）各增四字段——写面与读面同一份字段集 | `crates/core/workflow/src/state.rs` | crate 内 serde/specta | 全直线复制零逻辑分支（AC-1 / AC-5） |
| 收口整包透传 | `finish_command` 节点映射增四字段直复制（零过滤零改写）；doc 注释「透传」措辞随动 | `crates/core/orchestration/src/run_history.rs` | workflow（既有） | `RunNodeEntry` 组装单点（AC-1） |
| 落库加列 | `RunNodeRecord` 增四列均 `#[serde(default)]`（旧行解码 `started_at=0` / `ended_at=None` / token None 零迁移）；`RunNodeRecord::new` 直复制；`timestamp` 列语义不动；native_model id=16 version=1 不动（additive 先例） | `crates/infra/store/src/model.rs` | native_db/native_model | `#[serde(default)]` 旧行兼容（AC-5） |
| 读面投影 | `list_run_nodes` 四列逐字段透传 `RunNodeStateRecord`（node_seq 升序 = emit 序口径不变） | `crates/infra/store/src/store.rs` | model.rs | 读面与写面同字段集（AC-5） |
| 出线扩展 | `ChangeRunNodeRecord` 增四字段（毫秒数口径、与活面 `ChangeStepState` 同形、纯 derive）；`From<&RunNodeStateRecord>` 直读透出；runs 级 ISO 口径不动（两口径并存留痕注释） | `crates/core/workflow/src/queries/detail.rs` | state.rs | 查询层纯读零回填（AC-6） |
| bindings 再生 | `ChangeStepState` / `ChangeRunNodeRecord` TS 面四字段经 specta 自动跟随（非手改）；`dto.ts` 纯 re-export 零手改 | `packages/desktop/src/types/generated/bindings.ts` | export-bindings crate | `bindings:check` 守卫拦截漂移（AC-1） |
| 前端透传与活面标注 | `RuntimeFlowNode` 增四字段 + `live: boolean`（design 增补，D7）；`stepFromRecord` 纯复制、`UnifiedRunStep` 增 `live`、`unifiedRunNodes` 按源标注、`runStepNodes` 透传 | `packages/desktop/src/views/changes/flow/types.ts` + `run-state.ts` | bindings | `UnifiedRunStep` 单一形状（AC-1 / AC-8） |
| 节点呈现与走秒 | 标题下常驻行（静态耗时 + workerAgent token 段）；走秒 = 节点组件内自持 `setInterval(1s)` 局部重渲染（仅 `live && status === 'running'` 挂载、翻态 / 卸载即清 timer）；缺席与旧数据兜底 | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `metrics.ts` | xyflow 节点独立渲染不惊动画布（AC-7 / AC-8） |
| 度量格式化纯函数 | 毫秒 → 人读分级（`1.2s` / `3m20s` / `1h05m`）+ token 缩写（`12k/3k`）+ 「缺席 → "-" / 0ms 省显」判定单点 | `packages/desktop/src/views/changes/flow/metrics.ts`（新） | 零依赖纯函数 | vitest 直测边界（AC-7，D8） |
| corpus 种子与 golden | 种子构造器（`step` / `step_at`）补四字段定值注入（零 wall-clock）；时间 / token 有无两态样本构造；detail nodes 四字段投影入 golden 显式重写；fixtures README 矩阵随动 | `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/README.md` + `tests/golden/` | store run 域操作面 | `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程（AC-9，D11） |
| 版本交付 | `packages/desktop` 0.5.10 → 0.5.11 | `packages/desktop/package.json` | — | D12 |

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | `StepSettle` 扩四字段形态与铸造落位（设计面） | `StepSettle` 增 `started_at: i64` / `ended_at: Option<i64>` / `token_input: Option<u64>` / `token_output: Option<u64>`：构造器（`running()` 与经 struct-update 展开的三个终态构造器）四字段恒中性（`0` / `None` ×3，doc 注释「时间字段由 emit_node 单点铸造覆写」）；新增 builder `with_tokens(input: Option<u64>, output: Option<u64>)`。**铸造收 `emit_node` 单点**：`let now = now_millis(); settle.started_at = now; settle.ended_at = (status != Running).then_some(now);` 后组装完整 `ChangeStepState`——44 个调用点零触点（构造器 struct-update 自动继承中性值，无需逐点补） | proposal / orchestration delta 明文「emit_node 铸造单点」；两时间字段在 `StepSettle` 是**载荷镜像面**（`ChangeStepState` 全字段经其组装、缺字段不编译），铸造值不来自调用方；token 两值是**数据随行面**（run_worker 经 builder 落值、其余步不设即 None） |
| D2 | token 落值条件与失败路径（设计面） | `run_worker` 在 `worker.run(turn).await` 返回 **Ok** 后（终态 emit 前）对 `outcome.transcript` 求和（`sum_usage_tokens(&outcome.transcript, "input_tokens" / "output_tokens")`）随行 `with_tokens` 落终态 emit——**无论 `outcome.status` 为 Completed / Failed / Stopped / Running 漂移收敛**（transcript 在手即求和）；**Err 路径**（无 outcome、无 transcript）failed emit 恒 None。终态单次落值即「会话结束后累计结果」，零二次刷新通道 | `worker.run` await 收口后 transcript 已含 TurnDone（终态增量自包含）；失败收敛步同样消耗了 token、显值语义诚实；Err 路径无数据源，None 即「usage 缺席合法缺省」（AC-3） |
| D3 | `sum_usage_tokens` 上移落位与命名（待决 3） | **新模块 `crates/core/agent/src/usage.rs`**，`pub fn sum_usage_tokens(events: &[AgentEvent], key: &str) -> Option<u64>`（函数体自 store.rs 原样上移，含 saturating_add / any-then-some 语义）；`lib.rs` 增 `mod usage;` + `pub use usage::sum_usage_tokens;`；**命名保留原名**——infra/store 改引仅动 `use` 行（两消费点词面零变化），grep 单点可验 | `AgentEventKind::TurnDone` 定义所在 crate；函数吃 `&[AgentEvent]` + serde_json（agent 既有依赖），不违「agent 禁 workspace 内依赖」；session.rs 为会话域契约类型文件（仅 `new_session_id` 一函数），usage 求和是事件流概念非会话域概念，独立小模块职责干净；命名保留使两消费方调用面与既有测试断言零漂移（AC-10 / R7） |
| D4 | `accumulate_node` 合并实现形态（设计面） | `Some(node)` 命中臂：`status` 无条件覆写照旧；`session_id` / `detail` / `output_ref` 既有 some 才覆写照旧；新增 `ended_at` / `token_input` / `token_output` 三字段同款 some 才覆写；**`started_at` 不出现在命中臂**（零赋值 = 无条件保首见，终态行携带的同刻 started 值被丢弃）——`None` 臂 `entry.nodes.push(step)` 首见即落值。头注释「running → 终态 / 会话 id / 详情 / 输出引用覆写」扩为含「起止与 token：started 保首见、ended / token some 才覆写」 | 与 `session_id` 首见语义对称；瞬步单 emit（首见即终态行自身）started == ended 自然成立，无需区分「首 emit 是否终态」；不赋值比条件赋值更显式表达「终态值被丢弃」（AC-2 / AC-4） |
| D5 | 落库列形态与旧行兼容（设计面） | `RunNodeRecord` 四列（`timestamp` 前追加）均带 `#[serde(default)]`（含 `started_at: i64` 缺省 0）；**native_model id=16 version=1 不动**（`session_id` / `detail` / `output_ref` additive 同版本先例）；`RunNodeRecord::new` 增四行直复制 `RunNodeEntry`；`timestamp` 列（= finished_at 整包同刻）语义原样保留——步级时间独立成列不顶替不复用；doc 注释留痕「旧库行缺省 0 = 步级时间缺席，前端判定收格式化模块单点」 | additive 加列 + serde default = 存量库零迁移零报错（AC-5）；i64 非可选的 0 缺省语义负担收单点判定（R4） |
| D6 | 出线两口径并存留痕（设计面） | `ChangeRunNodeRecord` 四字段**毫秒数直出**（`startedAt: number` / `endedAt: number \| null` / `tokenInput: number \| null` / `tokenOutput: number \| null`；纯 derive，旧库行 `0` / `null` 原样出线、查询层零改写零回填）；runs 级 `ChangeRunEntry.startedAt` / `finishedAt` ISO 串口径**不变**——DTO doc 注释显式留痕「节点级毫秒数与活面同形、runs 级 ISO 为聚合时刻口径，两口径并存为有意裁定」 | `UnifiedRunStep` 单一形状（活面 / 库读史同型）、`stepFromRecord` 纯复制、走秒直减 `Date.now()` 零 parse；查询层纯读纪律（AC-6 / R5） |
| D7 | 前端活面标注（设计面，proposal 清单外字段增补） | `UnifiedRunStep` 与 `RuntimeFlowNode` 各增 `live: boolean`：`unifiedRunNodes` 按源标注（库读史步 false、`liveRun` 步 true）；走秒挂载条件 = `live && status === 'running'` | AC-8 要求库读史 status=running 残留节点（run 已死）不走秒——仅凭 `status === 'running'` 不可辨归属；xyflow 节点仅经 node data 取上下文，标注随图构建时落面最简（`unifiedRunNodes` 是两源合成单点，天然知道归属）；在飞 run 的库 start 行 nodes 恒空，两源无同键重复、标注无歧义 |
| D8 | 度量格式化模块命名、函数面与分级规则（待决 2） | **新文件 `flow/metrics.ts`**（节点度量：耗时分级 + token 缩写 + 缺席判定单点）。导出面三个（均获 run-step-node.tsx 跨文件生产消费）：`formatTokens(value: number \| null): string`、`settledDurationLabel(startedAt: number, endedAt: number \| null): string \| null`、`runningDurationLabel(startedAt: number, now: number): string`；**分级核心 `formatDurationMs(ms: number): string` 模块私有**（knip 纪律：防 test-only export；分级断言经两个标签函数全覆盖）。缺席判定收本模块单点（MUST NOT 散落 `0` 魔数比较，flow-view delta 条款）：`settledDurationLabel`——`startedAt <= 0 \|\| endedAt == null` → `"-"`；`endedAt <= startedAt` → `null`（0ms 瞬步省显，调用侧不渲染该段）；正值 → 分级串。`runningDurationLabel`——`startedAt <= 0` → `"-"`，否则 `now - startedAt` 分级。分级规则：`< 60_000ms` → 秒一位小数（`1.2s` / `59.9s` / 999ms → `1.0s`）；`< 3_600_000ms` → `3m20s`（分 + 整秒余量）；`≥ 3_600_000ms` → `1h05m`（时 + 两位补零分）。token 缩写：`null` → `"—"`；`< 1000` 原样十进制；`1000 ≤ n < 1_000_000` → `(n / 1000)` 一位小数四舍五入去尾零 + `k`（`12k` / `12.3k`，999_999 → `1000k` k 阶封顶不跨界）；`≥ 1_000_000` → 同式 + `M`（`1.5M` / `1M`） | proposal 点名「毫秒 → 人读格式化工具新写（纯函数独立可测）」；缺席 / 0ms / "—" 三态语义散落多组件会漂移，单点模块承载（R4）；999ms→`1.0s` 与 999_999→`1000k` 为边界留痕非缺陷（显示保真 vs 阶界互斥二选一，取阶界互斥） |
| D9 | 走秒实现形态（待决 1） | **节点组件内自持**（不提炼自定义 hook）：`RunStepNode` 内 `const [tickNow, setTickNow] = useState(() => Date.now())` + `useEffect`（deps = `[ticking]`，`ticking = node.live && node.status === 'running'`）：active 时 `setTickNow(Date.now())` 对齐 + `setInterval(() => setTickNow(Date.now()), 1000)`（模块常量 `TICK_MS = 1000`），cleanup `clearInterval`——翻态（status 离开 running）或组件卸载即清；局部 state 更新只重渲染本节点（xyflow 节点独立渲染） | running 同刻 ≤ 1（walker 串行），hook 复用度低（Simplicity first）；timer 生命周期绑定组件由 effect deps + cleanup 结构性承载（R1：卸载即清、收口回落）；不提炼 hook 免「单使用点抽象」 |
| D10 | 常驻行视觉形态与测试挂钩（待决 4） | 标题头行与 detail 短文本行之间插常驻行：`⏱ <耗时>` 段 + `tokens <in>/<out>` 段，**两段齐备时以 ` ▸ ` 分隔**（`⏱ 3m20s ▸ tokens 12k/3k`）；行渲染条件 = 至少一段在场（0ms 瞬步且非 workerAgent → 整行省显）；token 段仅 `group === 'workerAgent'` 渲染（running 亦渲染、缺席 `—/—`）；文案 `tokens` 与抽屉 `SessionMeta` 同名（增量 vs 全量语义差由常驻行 / 抽屉两位置语境承载，R2）。data-testid：`run-step-metrics`（行容器）/ `run-step-duration`（耗时段，含 "⏱" 前缀）/ `run-step-tokens`（token 段） | 常驻行（非 HoverCard）已拍板；`—`（em dash）用于 token 缺席、`-`（hyphen）用于耗时缺席，与 AC-7 / flow-view delta 词面逐字一致；既有锚点（`run-step-node` / `run-step-kind` / `run-step-group` / `run-step-detail`）零改号 |
| D11 | corpus 定值注入与两态样本（待决 5 + R6） | `step` / `step_at` fixture 增四字段**定值注入**（默认形态：`started_at` 取种子既有时间定值族基准、`ended_at` 按步状态派生定值、token None；两态样本经参数 / overrides 注入），全部经 `finish_command` → store run 域操作面落库（禁裸表插桩，delta 条款）；样本维度：时间两态（正常起止双值步 / 瞬步 `started == ended` / running 收口在途 `ended = None`）+ token 两态（WorkerAgent 步带值 / 工具步 None）各至少一个；golden 经 `DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test` 显式重写 + diff 人工确认留痕。断言粒度已核实（见「提案与规格同步状态」）：golden 为 detail 全量序列化、种子路径零生产时钟——确定性成立，待决解除 | AC-9；零 wall-clock 进 corpus 沿「corpus 确定性由测试注入承载」既有口径 |
| D12 | 版本与守线 | `packages/desktop` 0.5.10 → **0.5.11**（`src-tauri/tauri.conf.json` 经 `../package.json` 引用自动跟随、`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 2.10.44 零改动；bindings 全量再生成（`ChangeStepState` / `ChangeRunNodeRecord` 四字段）；`vp test` / `client:check`（含 50 行函数限 lint）/ knip 全绿归桌面端管线（AC-11） | 既有惯例；生成物零手改 |

### 透传链（全直线复制，无逻辑分支）

```
emit_node（铸造单点：started / ended 按 status；token 随 StepSettle）
  └▶ RunUpdate::Step { step: ChangeStepState(+4) }
       ├▶ control::publish → accumulate_node（started 保首见 / ended + token some 才覆写）
       │     ├▶ RunEntry.nodes（进程内累积器，emit 序）
       │     └▶ broadcast RunNotice（kind-only 零载荷——不变量保持）
       ├▶ snapshot() → ChangeRunSnapshot.nodes(+4) → ActiveRunView.nodes（复用自动跟随）
       └▶ walk_run 终态出口 guard.nodes() → run_history::finish_command
             └▶ RunNodeEntry(+4) → store.finish_change_run 单事务 → RunNodeRecord(+4 列)
                   └▶ list_run_nodes → RunNodeStateRecord(+4) → ChangeRunNodeRecord(+4 毫秒数)
                         └▶ bindings.ts 再生 → stepFromRecord 纯复制 / UnifiedRunStep(+live)
                               └▶ RuntimeFlowNode(+4+live) → RunStepNode 常驻行（静态耗时 / 走秒 / token）
```

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。proposal「测试文件」中纯断言编写归 test-design / test-gen / test-execution 阶段；与类型演进强耦合的机械面（既有构造点补字段）与 corpus 种子 / golden 重写按先例随实现清单随动（下表末两行 + 任务列表「编译面机械随动」项）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/usage.rs` | token 求和单源：`pub fn sum_usage_tokens`（自 infra/store 上移，D3；子表关联见「公共函数 / API」第 1 行、「类型定义」表上方注释不涉及） |
| `packages/desktop/src/views/changes/flow/metrics.ts` | 节点度量纯函数模块：耗时分级 + token 缩写 + 缺席判定单点（D8；子表关联见「公共函数 / API」第 5–7 行） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ChangeStepState` 增 `started_at: i64` / `ended_at: Option<i64>` / `token_input: Option<u64>` / `token_output: Option<u64>` 四字段（`output_ref` 后追加），doc 注释逐字段携口径（UTC unix 毫秒 / running 恒 None / TurnDone 增量缺席合法缺省）与合并语义指针（accumulate 保首见 / some 才覆写） | AC-1；活面 `ChangeRunSnapshot.nodes` / 命令层 `ActiveRunView.nodes` 复用本类型自动跟随、命令面签名零改动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `StepSettle` 扩四字段 + `running()` 中性初始化 + 新 builder `with_tokens`（D1）；`emit_node` 时间铸造单点（`now_millis()` 按 status 铸 started / ended，注释锚「收口时刻铸造只在生产路径消费、corpus 确定性由测试注入承载」沿 :3125 口径）；`run_worker` Ok 路径终态 emit 前 `agent::sum_usage_tokens` 对 `outcome.transcript` 求 in / out 落值（D2，`use agent::{…, sum_usage_tokens}` 随动） | AC-2 / AC-3；44 个 `emit_node` 调用点零触点；Err 路径恒 None；路由 / 相位转移零触点 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` | `accumulate_node` 合并语义扩展：命中臂增 `ended_at` / `token_input` / `token_output` 三字段 some 才覆写、`started_at` 零赋值保首见（D4）；头注释措辞随动 | AC-4；`publish` / 累积器 / Notice 广播结构零改动 |
| `packages/desktop/src-tauri/crates/core/agent/src/lib.rs` | 增 `mod usage;` + `pub use usage::sum_usage_tokens;`（D3 落位随动；proposal 实现文件「`core/agent/src/`（落位 design 定稿）」条目的 re-export 半边） | AC-10；agent 依赖面零新增（serde_json 既有） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 删除私有 `sum_usage_tokens`（:199-211），`use agent::{…}` 增 `sum_usage_tokens`（:164 / :870 两消费点词面零变化，D3 消费方）；`list_run_nodes` 投影增四字段透传 `RunNodeStateRecord` | AC-10（聚合行为零变化）+ AC-5（读面随行）；`finish_change_run` 事务结构零改动（`RunNodeRecord::new` 内部消化新列） |
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `RunNodeEntry` 增四字段（`output_ref` 后）；`RunNodeStateRecord` 增四字段（`timestamp` 前），doc 注释同款口径 | AC-1 / AC-5；写面载荷与读面同一份字段集 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | `finish_command` 节点映射增四字段直复制（零过滤零改写）；doc「session_id / detail / output_ref 透传」措辞随动含四字段 | AC-1；签名零变化（载荷字段随 `ChangeStepState` / `RunNodeEntry` 扩展） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `RunNodeRecord` 增四列（`timestamp` 前）均 `#[serde(default)]` + doc 注释（旧库行缺省 0 / None / None / None、`started_at=0` = 步级时间缺席）；`RunNodeRecord::new` 增四行直复制；native_model id=16 version=1 不动（D5） | AC-5；`timestamp` 列语义原样保留；加列先例 = 同结构 `session_id` / `detail` / `output_ref` |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `ChangeRunNodeRecord` 增四字段（毫秒数口径，`output_ref` 后）+ `From<&RunNodeStateRecord>` 直读透出；DTO doc 注释留痕两口径并存裁定（节点级毫秒 / runs 级 ISO，D6） | AC-6；纯 derive 零字段属性（旧库行 `0` / `null` 原样出线）；`change_detail` 聚合逻辑零改动 |
| `packages/desktop/src/types/generated/bindings.ts` | `pnpm -C packages/desktop run bindings:export` 再生（非手改）：`ChangeStepState` / `ChangeRunNodeRecord` 各增 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` | AC-1；`dto.ts` 纯 `export type *` re-export 自动跟随零手改 |
| `packages/desktop/src/views/changes/flow/types.ts` | `RuntimeFlowNode` 增 `startedAt: number` / `endedAt: number \| null` / `tokenInput: number \| null` / `tokenOutput: number \| null` / `live: boolean`（D7，doc 注释：旧库行 0 = 缺席、live = 活面来源标注） | AC-1 / AC-8；`FlowNode` 单分类与既有字段零改动 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | `stepFromRecord` 四字段纯复制；`UnifiedRunStep` 增 `live: boolean`；`unifiedRunNodes` 按源标注（persisted false / live true）；`runStepNodes` 透传四字段 + `live` 进节点 | AC-1 / AC-8；词汇归一 / 槽位配对 / 边锚定机制零改动 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 标题头行与 detail 行之间新增常驻行（模块私有 `StepMetricsRow`：`⏱ <耗时>` + `tokens <in>/<out>` 两段、齐备以 ` ▸ ` 分隔、行条件 ≥ 1 段在场、token 段仅 workerAgent 组，D10）；走秒 tick（组件内 `useState` + `useEffect` + `setInterval(1000)`，`ticking = live && status === 'running'`，cleanup 即清，D9）；data-testid `run-step-metrics` / `run-step-duration` / `run-step-tokens` | AC-7 / AC-8；头行 / detail HoverCard / 布局坐标 / 点击判据零改动；前端全库首例 interval 的生命周期收组件 effect 承载 |
| `packages/desktop/package.json` | `version` 0.5.10 → 0.5.11（D12） | AC-11；`src-tauri/tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs` | `step` / `step_at` fixture 补四字段定值注入 + 时间 / token 两态样本构造（D11；经 `finish_command` → store run 域操作面）；两态断言编写归测试相位 | AC-9；机械 / 种子面随实现清单，非断言编写 |
| `packages/desktop/src-tauri/crates/core/workflow/tests/fixtures/README.md` | 语料覆盖面矩阵增「步级时间与 token」维度行（两态样本说明） | AC-9 随动 |

### 公共函数 / API

Rust 签名为 Rust 语法；TS 签名为 TS 语法。签名零变化者标注「签名不变」。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `sum_usage_tokens` | `packages/desktop/src-tauri/crates/core/agent/src/usage.rs` | 新增（自 infra/store 上移） | `pub fn sum_usage_tokens(events: &[AgentEvent], key: &str) -> Option<u64>` | 转录 `TurnDone.usage` 求和单源（key = `"input_tokens"` / `"output_tokens"` 鸭子类型键；全缺席 → None、部分在场饱和求和）；orchestration `run_worker` 与 infra/store 会话统计两消费方同源引用；函数体与上移前逐行一致（AC-3 / AC-10） |
| `finish_command` | `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | 修改（签名不变） | `pub fn finish_command(request: &RunRequest, status: ChangeRunStatus, reason: Option<String>, finished_at: i64, nodes: &[ChangeStepState], edges: &[ChangeStepEdge]) -> RunFinishCommand` | 节点映射增四字段直复制进 `RunNodeEntry`（零过滤零改写，AC-1） |
| `RunNodeRecord::new` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 修改（签名不变） | `pub fn new(run_id: &str, change_id: &str, node_seq: u64, entry: &RunNodeEntry, timestamp: i64) -> Self` | 直复制 `entry` 四字段进新列（`timestamp` 参数语义不变，AC-5） |
| `Store::list_run_nodes` | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 修改（签名不变） | `pub fn list_run_nodes(&self, run_id: &str) -> Result<Vec<RunNodeStateRecord>, StoreError>` | 四列逐字段透传 `RunNodeStateRecord`（node_seq 升序 = emit 序口径不变，AC-5） |
| `formatTokens` | `packages/desktop/src/views/changes/flow/metrics.ts` | 新增 | `export function formatTokens(value: number \| null): string` | token 缩写（D8 分级：null → `—` / < 1000 原样 / k / M 一位小数去尾零） |
| `settledDurationLabel` | `packages/desktop/src/views/changes/flow/metrics.ts` | 新增 | `export function settledDurationLabel(startedAt: number, endedAt: number \| null): string \| null` | 已收口步耗时段：缺席 → `"-"`、0ms → `null`（省显）、正值 → 分级串——缺席判定收本模块单点（AC-7） |
| `runningDurationLabel` | `packages/desktop/src/views/changes/flow/metrics.ts` | 新增 | `export function runningDurationLabel(startedAt: number, now: number): string` | running 走秒段：`startedAt <= 0` → `"-"`，否则 `now - startedAt` 分级串（`now` 由组件 tick 注入，纯函数可测，AC-8） |
| `unifiedRunNodes` | `packages/desktop/src/views/changes/flow/run-state.ts` | 修改（签名不变） | `export function unifiedRunNodes(runs: ChangeRunEntry[], liveRun: LiveRunFace \| null): UnifiedRunStep[]` | 返回步行增 `live` 源标注（persisted false / live true，D7） |
| `runStepNodes` | `packages/desktop/src/views/changes/flow/run-state.ts` | 修改（签名不变） | `export function runStepNodes(steps: UnifiedRunStep[], phaseOrder: string[]): RuntimeFlowNode[]` | 节点透传 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` / `live` 五字段（AC-1 / AC-8） |
| `RunStepNode` | `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 修改（签名不变） | `export function RunStepNode({ data }: NodeProps<RunStepFlowNode>): React.JSX.Element` | 常驻行 + 走秒呈现（AC-7 / AC-8）；props / 节点类型注册零改动 |

<!-- `emit_node` / `accumulate_node` / `StepSettle::with_tokens` / `stepFromRecord` / `StepMetricsRow` / `formatDurationMs` 为模块私有函数不列（后者模块私有为 knip test-only export 纪律有意为之，分级断言经两个导出标签函数覆盖，D8）；`emit` / `publish` / `snapshot` / `finish_change_run` / `change_detail` / `get_change_detail` 签名与行为面零改动不列。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `ChangeStepState` | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | 修改 | 增 `pub started_at: i64` / `pub ended_at: Option<i64>` / `pub token_input: Option<u64>` / `pub token_output: Option<u64>`（serde camelCase 随 struct 既有属性；doc 注释携口径与合并语义指针，AC-1） |
| `RunNodeEntry` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | 增四字段（`i64` / `Option<i64>` / `Option<u64>` ×2；finish 载荷内联条目随行，AC-1 / AC-5） |
| `RunNodeStateRecord` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | 增四字段（port 读面随行，AC-1 / AC-5） |
| `RunNodeRecord` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 修改 | 增四列均 `#[serde(default)]`（旧行解码 `0` / `None` / `None` / `None`；native_model id=16 version=1 不动，D5 / AC-5） |
| `ChangeRunNodeRecord` | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 修改 | 增 `pub started_at: i64` / `pub ended_at: Option<i64>` / `pub token_input: Option<u64>` / `pub token_output: Option<u64>`（毫秒数口径、纯 derive，AC-6） |
| `RuntimeFlowNode` | `packages/desktop/src/views/changes/flow/types.ts` | 修改 | 增 `startedAt: number` / `endedAt: number \| null` / `tokenInput: number \| null` / `tokenOutput: number \| null` / `live: boolean`（D7，AC-1 / AC-8） |
| `UnifiedRunStep` | `packages/desktop/src/views/changes/flow/run-state.ts` | 修改 | interface 增 `live: boolean`（活面来源标注，D7） |
| `ChangeStepState`（TS 生成物镜像） | `packages/desktop/src/types/generated/bindings.ts` | 修改（specta 再生） | 增 `startedAt: number` / `endedAt: number \| null` / `tokenInput: number \| null` / `tokenOutput: number \| null` |
| `ChangeRunNodeRecord`（TS 生成物镜像） | `packages/desktop/src/types/generated/bindings.ts` | 修改（specta 再生） | 同上四字段（毫秒数 / null 口径） |

<!-- `StepSettle`（walker 模块私有 struct）扩四字段归修改文件表说明，不列公共类型；`LiveRunFace` / `RunStepFlowNode` / `RunStepNodeData` 零变化不列。 -->

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.5.11`（现行 `0.5.10`） | 版本交付（AC-11 / D12）：`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 引用自动跟随，`src-tauri/Cargo.toml` 不随动；`plugins/dev-team` 保持 2.10.44 零改动 |

<!-- 走秒周期 `TICK_MS = 1000` 为 run-step-node.tsx 模块内代码常量（非用户配置面），不列。 -->

---

## 数据模型

### 步级时间与 token 四字段（六载体同形）

| 载体 | 字段（逐字段同值透传） | 关系 | 持久化 |
|------|------------------------|------|--------|
| `ChangeStepState`（活面契约） | `started_at: i64`（UTC unix 毫秒）/ `ended_at: Option<i64>`（running 恒 None）/ `token_input: Option<u64>` / `token_output: Option<u64>`（TurnDone 增量，仅 WorkerAgent 步终态有值） | `emit_node` 铸造 / 落值单点；`accumulate_node` 合并（started 保首见 / ended + token some 才覆写）；`ActiveRunView.nodes` / `ChangeRunSnapshot.nodes` 复用 | 进程内（注册表累积器）；IPC 经 `get_change_detail` 出线 |
| `StepSettle`（emit 载荷收窄面） | 同四字段（构造器中性，`started_at` / `ended_at` 由 emit_node 铸造覆写、token 两值 `with_tokens` 随行） | `ChangeStepState` 的组装前置面 | 不持久化（walker 栈内瞬态） |
| `RunNodeEntry`（finish 载荷内联） | 同四字段 | `finish_command` 自 `ChangeStepState` 直复制 | 经 `RunFinishCommand.nodes` 进 store 单事务 |
| `RunNodeRecord`（落库行） | 同四列（`#[serde(default)]`） | `RunNodeRecord::new` 直复制；`node_key` 打包主键 / `run_id` 二级索引不变；`timestamp` 列（= finished_at 整包同刻）原语义保留 | native_db（workspace 库）additive 加列零迁移；旧库行解码 `started_at=0` / `ended_at=None` / token None |
| `RunNodeStateRecord`（port 读面） | 同四字段 | `list_run_nodes` 投影（node_seq 升序 = emit 序） | 不持久化（流量像） |
| `ChangeRunNodeRecord`（线面） | `startedAt: number` / `endedAt: number \| null` / `tokenInput: number \| null` / `tokenOutput: number \| null`（毫秒数 / null，纯 derive） | `From<&RunNodeStateRecord>` 直读；runs 级 ISO 串口径并存不变 | IPC 出线 + golden wire 契约 |
| `RuntimeFlowNode`（前端节点） | 同线面四字段 + `live: boolean` | `stepFromRecord` 纯复制 / `unifiedRunNodes` 源标注 / `runStepNodes` 透传 | 内存态（React）；走秒直减 `Date.now()` 零 parse |

### 呈现语义（常驻行三态）

| 形态 | 判定（收 `metrics.ts` 单点） | 呈现 |
|------|------------------------------|------|
| 已收口步 | `startedAt > 0 && endedAt != null && endedAt > startedAt` | `⏱ <分级串>`（如 `⏱ 3m20s`） |
| 活面 running | `live && status === 'running'`（startedAt > 0） | `⏱ <走秒串>`（每秒 tick） |
| 0ms 瞬步 | `startedAt > 0 && endedAt == startedAt`（或 ≤） | 耗时段省显（`settledDurationLabel → null`；无 token 段时整行省显） |
| 缺席 / 旧库行 / 库读史 running 残留 | `startedAt <= 0` 或 `endedAt == null`（非活面 running） | 耗时 `-`；token（workerAgent）`—/—` |

### token 口径（用户拍板留痕）

节点常驻行显**本步 turn 增量**（`outcome.transcript` 的 TurnDone.usage 求和、终态单次落值）；抽屉 `SessionMeta` 显 session 全量累计（`SessionStats` 零改动）——同 session 多步两数不同为有意语义（每节点增量自包含、可解释），文案两侧同名 `tokens in / out` 不叠加解释（R2）。

---

## 路由 / API 设计

本变更不涉及 HTTP API；IPC 命令面（Tauri invoke 契约）签名零改动，载荷字段自动扩展：

| IPC 面 | method / path | 说明 | 输入 | 输出 | auth |
|--------|---------------|------|------|------|------|
| `get_change_detail` | invoke `get_change_detail` | 统一视图（库读史 ∪ 活面） | `root: string, id: string`（不变） | `Option<ChangeDetailUnified>`（不变）；字段面扩展：`runs[].nodes[]`（`ChangeRunNodeRecord`）与 `activeRun.nodes[]`（`ChangeStepState`）各增四字段 | 本机 IPC 无鉴权（同既有命令口径） |
| `change_flow_watch` 通知 | Channel `RunNotice` | 步状态变更通知 | 订阅 `(root, id)`（不变） | kind-only 六变体零载荷（不变）——载荷不出进程，客户端收通知重查统一视图取含四字段的节点行 | 本机 IPC 无鉴权 |

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 契约四字段全链：`ChangeStepState`（state.rs）→ `RunNodeEntry` / `RunNodeStateRecord`（workflow/state.rs）→ `RunNodeRecord`（model.rs）→ `ChangeRunNodeRecord`（detail.rs）→ `RuntimeFlowNode`（types.ts）六载体逐字段同形（i64 / Option 口径一致）；bindings 再生（变更清单第 10 行）+ `bindings:check` 守卫；活面 `ActiveRunView.nodes` 复用自动跟随、命令签名零改动（路由 / API 节） |
| AC-2 | D1（emit_node 铸造：running `(now, None)`、终态 `(now, Some(now))`；44 调用点含 stopped 路径全覆盖）+ D4（accumulate started 保首见、终态同刻值被丢弃；瞬步单 emit 首见即真值 started == ended） |
| AC-3 | D2（run_worker Ok 路径 transcript TurnDone.usage 求 in / out 两键、终态单次落值；Err / 工具步 / Gate / Git 步恒 None；无二次刷新通道）+ D3（`sum_usage_tokens` 单源） |
| AC-4 | D4（accumulate 合并语义：started 零赋值保首见、ended / token 三字段 some 才覆写）——同键多 emit 后快照 / 落库值与合并规则一致由透传链全直线复制保证 |
| AC-5 | D5（四列 `#[serde(default)]` additive 加列、version=1 不动、零迁移；旧行解码 `0` / `None` / `None` / `None`）+ `timestamp` 列语义保留（model.rs 修改行）+ `list_run_nodes` 读面透传 |
| AC-6 | D6（`ChangeRunNodeRecord` 毫秒数纯 derive、旧行 `0` / `null` 原样出线零回填；runs 级 ISO 口径不变两口径留痕） |
| AC-7 | D8（`settledDurationLabel` 缺席 / 0ms / "-" 判定单点）+ D10（常驻行两段形态、token 段仅 workerAgent、缺席 `—`）——已收口显静态耗时、0ms 省显、旧数据 `-` |
| AC-8 | D7（`live` 源标注——库读史 running 残留不走秒）+ D9（组件内 interval、deps 翻态即清、卸载 cleanup、局部重渲染）+ `runningDurationLabel` |
| AC-9 | D11（种子 `step` / `step_at` 定值注入、时间 / token 两态样本、经 store 操作面；golden `DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工确认）+ 断言粒度已核实（种子路径零生产时钟） |
| AC-10 | D3（唯一实现居 core/agent/usage.rs；store 改引同源、函数体逐行一致 → 会话统计输出零变化；orchestration 依赖面零 infra 新增——仅新增 `agent::sum_usage_tokens` 引用） |
| AC-11 | D12 + 配置表（0.5.11 / Cargo.toml 不随动 / plugins 2.10.44）；`vp test` / `client:check` / knip 全绿归桌面端管线执行 |

### 风险对齐（proposal R1–R7 → 设计落点）

R1 走秒 timer → D9（active 绑定 effect + cleanup、局部 state 不惊动画布、同刻 ≤ 1 节点）；R2 两口径 token 并存文案 → D10 + 数据模型「token 口径留痕」节；R3 构造点 ~20 处波及 → 编译器驱动机械随动（任务阶段一 / 二 / 四）+ golden 显式重写流程；R4 `started_at=0` 魔数 → D8（缺席判定收 metrics 单点）+ D5（model.rs doc 留痕）；R5 两时间口径并存 → D6（detail.rs doc 留痕 + 前端单一形状消费）；R6 corpus 确定性 → D11（已核实：golden 全量序列化但种子直构零生产时钟）；R7 求和双实现 → D3（唯一实现 + 两消费方同源，grep 单点可验）。

---

## 依赖

### 运行时依赖

- 零新增 npm / crate 依赖：Rust 侧 `serde` / `serde_json` / `specta` / `native_db` / `native_model` 全部既有（`usage.rs` 仅用 serde_json——agent crate 既有依赖面）；前端 `setInterval` / `useState` / `useEffect` 为 React / DOM 既有面，xyflow 节点独立渲染既有。

### 构建 / 测试依赖

- 零新增；沿既有工具链：`pnpm -C packages/desktop run bindings:export`（specta 再生成）、`bindings:check`（一致性守卫）、`DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test`（golden 显式重写）。
- 断言编写与测试执行归 test-design / test-gen / test-execution 阶段（proposal「测试文件」清单）；静态脚本（static-check / test-execution 等机械步骤）由桌面端执行，agent 内不重复执行。

---

## 待决问题

- 无未决事项：proposal 五项待决已全部定稿（D8 格式细节 / D9 走秒形态 / D3 求和落位命名 / D10 常驻行视觉 / D11 corpus 断言粒度——后者经实况核实解除）；公共函数签名无 `(待确定)` 项。
- 流程性留痕（非待决）：golden 重写 diff 的人工确认（预期范围 = 既有 detail golden 全量 nodes 行增四键 + 新增两态样本相关键）随实现执行在案。
