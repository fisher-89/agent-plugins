# 提案: desktop-step-running-snapshot

> **变更**: desktop-step-running-snapshot
> **日期**: 2026-10-17
> **状态**: draft
> **探索**: `openspec/changes/desktop-step-running-snapshot/explore.md`（探索二轮四项决策已用户拍板在案）

---

## 问题

桌面端 change 流程图节点缺两块运行观测信息：

1. **步级时间戳全链缺席**：`ChangeStepState`（活面契约，`core/orchestration/src/state.rs:131`）无任何时间字段；落库行 `RunNodeRecord`（`infra/store/src/model.rs:934`）仅有 `timestamp`，且其值 = run 收口 `finish_command` 携带的 `finished_at` **整包同刻**——所有节点同一值，不承载步级起止。前端 `RuntimeFlowNode`（`flow/types.ts:48`）与 `RunStepNode` 渲染层随之无耗时可显。用户无法从图上看出「这一步跑了多久」。
2. **token 消耗数据已存在但到不了节点**：session 域已有完整统计面（`SessionStats`，`core/agent/src/session.rs:91`，自 `TurnDone.usage` 求和；抽屉 `SessionTranscriptPanel` → `sessionDetail` 单查已渲染 tokens in / out），但流程图节点渲染层（`RunStepNode`）拿不到：running 节点 `session_id` 为 null（仅终态 emit 携带），前端按 sessionId join 的路线（路线 B）对 running 节点与 session 清理后的库读史节点均缺数。
3. **双 emit 模式是天然锚点但未被利用**：每个步的生命周期在 walker 中恒为 running emit → 终态 emit 两次 `emit_node`（同一稳定身份键，`walker.rs` 共 44 个调用点）；running emit 即开始时刻锚点、终态 emit 即结束时刻锚点，`run_worker` 的 `outcome.transcript`（`WorkerTurnOutcome`，await 收口后已含 `TurnDone`）即本步 token 增量的自包含数据源。

**用户已拍板**（explore「决策记录」）：token 口径 = 路线 A（后端随行、**本步增量**，终态单次落值即会话结束后 TurnDone 的累计结果）；接口返回 startedAt / endedAt，前端计算静态耗时、running 步前端走秒；显示位置 = 图内节点标题下**常驻行**（非 HoverCard）；旧库读史无步级时间 → 显示 "-"（不从 turn 行回填）。

---

## 提案

单点契约扩展 + 全直线透传，四条线：

1. **契约单点（`ChangeStepState` 增四字段）**：`started_at: i64`（UTC unix 毫秒，与域内 `now_millis()` 口径一致）、`ended_at: Option<i64>`（running 恒 None）、`token_input: Option<u64>` / `token_output: Option<u64>`（本步 turn 增量，`TurnDone.usage` 求和；仅 WorkerAgent 步终态有值，缺席合法缺省）。`StepSettle` 同步扩四字段。
2. **时间铸造（`emit_node` 单点，复用既有 `now_millis()`，零新时钟缝）**：running emit 铸 `started_at = now`、`ended_at = None`；终态 emit（passed / failed / stopped）铸 `ended_at = now` 且随行 `started_at = now`（该值是否被采信由合并侧裁定）——`phaseStart` 等单 emit 瞬步由此自然得 started = ended 同刻（耗时 0ms，渲染层可零耗时省显）。收口时刻铸造只在生产路径消费，corpus 确定性由测试注入定值承载（沿 `walker.rs:3125` 既有注释口径）。
3. **合并语义（`accumulate_node`，control.rs:403）**：`started_at` **无条件保首见**（running emit 落值后不被终态 emit 顶替——终态行携带的同刻值被丢弃，瞬步首见即真值）；`ended_at` / `token_input` / `token_output` 与 `session_id` 同款「some 才覆写」。
4. **token 求和（`run_worker` 终态 emit 前单次落值）**：对 `outcome.transcript` 的 `AgentEventKind::TurnDone.usage` 求和（in / out 两键）写入终态 `StepSettle`——`worker.run(turn).await` 收口后 transcript 已含 TurnDone，**终态单次落值即「会话结束后累计结果」**，无需二次刷新通道；usage 缺席（引擎未返回）= None。求和函数上移：`sum_usage_tokens` 现居 `infra/store/src/store.rs:199`，而 orchestration 在 core 域（依赖方向 infra → core 不可反向）——上移至 `core/agent`（`AgentEventKind::TurnDone` 定义所在 crate，orchestration 已依赖 agent；agent crate 禁 workspace 内依赖，函数仅吃 `&[AgentEvent]` 天然合规），infra/store 改引 core 实现（聚合行为零变化）。
5. **落库与出线透传（全直线复制，无逻辑分支）**：`RunNodeEntry`（workflow/state.rs）→ `RunNodeRecord`（store model 加列，四字段均 `#[serde(default)]`——旧行解码 `started_at=0` / `ended_at=None` / token None，读史兼容零迁移）→ `RunNodeStateRecord` → `ChangeRunNodeRecord`（queries/detail.rs）→ bindings 再生成。出线口径：节点级 `startedAt: number` / `endedAt: number | null` / `tokenInput` / `tokenOutput`（毫秒数，与活面 `ChangeStepState` 同形——`UnifiedRunStep` 单一形状、`stepFromRecord` 纯复制、走秒直减 `Date.now()` 零 parse）；runs 级 `startedAt`/`finishedAt` ISO 串口径**不变**，两口径并存为有意裁定。既有 `timestamp` 列（落包时刻）语义保留不顶替。活面 `ActiveRunView.nodes` / `ChangeRunSnapshot.nodes` 复用 `ChangeStepState`，字段自动跟随、命令面签名零改动。
6. **前端呈现（`RunStepNode`）**：标题下新增常驻行 `⏱ 3m20s ▸ tokens 12k/3k`——静态耗时 = `ended_at - started_at`（前端计算，后端不落 duration）；token 行仅 `group === 'workerAgent'` 渲染（in / out 缩写，design 定稿格式），缺席与旧数据显示 "—"（`startedAt <= 0` 或 `endedAt == null`）；running 走秒 = 节点组件内自持 `setInterval(1s)` tick 局部重渲染（前端全库首例，xyflow 节点独立渲染不惊动画布），**仅活面（在飞 run）节点生效**——库读史 status=running 残留节点恒静态呈现（ended 缺席 → "—"）。毫秒 → 人读格式化工具新写（`1.2s` / `3m20s` 类，纯函数独立可测）。
7. **版本交付**：`packages/desktop` `0.5.10` → `0.5.11`；`plugins/dev-team` 零改动（2.10.44）。

---

## 能力

### 新增能力

- 无（两主题均落既有能力：契约扩展 = 编排域能力内加字段；呈现 = 流程图视图能力内加常驻行）。

### 修改的能力

- **desktop-change-orchestration** — 新增「步级时间戳与 token 消耗随行」requirement：`ChangeStepState` / `StepSettle` 四字段、`emit_node` 铸造语义、`accumulate_node` 合并语义（started 保首见 / ended + token some 才覆写）、`run_worker` 终态 token 求和落值、`sum_usage_tokens` 上移 core/agent（依赖方向约束）；`RunUpdate::Step` 载荷自动扩展但 IPC 通知 kind-only 降位不变。
- **desktop-change-state-store** — 新增「RunNodeRecord 步级时间与 token 列」requirement：四列加列透传（`#[serde(default)]` 旧行兼容）、`timestamp` 列语义保留、`RunNodeEntry` 随行扩展。
- **desktop-change-queries** — 新增「步级时间与 token 出线」requirement：`ChangeRunNodeRecord` 四字段出线（毫秒数口径与活面同形、纯 derive）、runs 级 ISO 口径不变的两口径并存留痕、golden 显式重写随动。
- **desktop-change-flow-view** — 新增「步级耗时与 token 节点呈现」requirement：`RuntimeFlowNode` 透传、标题下常驻行（静态耗时 + token）、running 走秒（仅活面、interval 局部重渲染）、缺席与旧数据 "—" 兜底、格式化工具纯函数。
- **desktop-corpus-regression** — 新增「步级时间与 token 语料维度」requirement：种子构造器补字段（定值注入保确定性）、时间 / token 有无两态样本、golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写。

### 引用沿用（零 delta）

- desktop-ipc-type-bindings — bindings 再生走既有管线（DTO 字段增经 specta 自动跟随、一致性守卫不变），零词面扩展。
- desktop-workspace-store — `sum_usage_tokens` 上移为行为不变重构（会话清单统计现算口径与输出零变化）。
- desktop-agent-execution — `SessionStats` / `SessionSummary` / `sessionDetail` 契约零改动（token 求和复用同一 TurnDone 数据源，session 域统计面不动）。
- desktop-app-shell — `get_change_detail` 命令签名零改动（`ActiveRunView.nodes` 复用 `ChangeStepState` 自动跟随）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — `ChangeStepState` 增 `started_at` / `ended_at` / `token_input` / `token_output` 四字段（含 doc 注释：口径与合并语义指针）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `StepSettle` 扩四字段（`running()` / 终态构造器随动）；`emit_node` 铸造单点；`run_worker` 终态 emit 前对 `outcome.transcript` 求和落 token（in / out）
- `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` — `accumulate_node` 合并语义：`started_at` 无条件保首见、`ended_at` / `token_input` / `token_output` some 才覆写
- `packages/desktop/src-tauri/crates/core/agent/src/`（落位 design 定稿，如 session.rs 或新 usage 模块）— `sum_usage_tokens` 上移承载（`pub fn`，签名 `(&[AgentEvent], &str) -> Option<u64>` 同款）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — 删除私有 `sum_usage_tokens`，改引 core/agent 实现（聚合行为零变化）
- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — `RunNodeEntry` 增四字段；`RunNodeStateRecord` 增四字段
- `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` — `finish_command` 透传四字段
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `RunNodeRecord` 加列（四字段 `#[serde(default)]`，先例 model.rs:956）；`RunNodeRecord::new` 随行
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — `list_run_nodes` 投影透传四字段
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `ChangeRunNodeRecord` 增四字段（毫秒数出线）+ `From` 投影
- `packages/desktop/src/types/generated/bindings.ts` — 再生成（`ChangeStepState` / `ChangeRunNodeRecord` 字段跟随）
- `packages/desktop/src/views/changes/flow/types.ts` — `RuntimeFlowNode` 增 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput`
- `packages/desktop/src/views/changes/flow/run-state.ts` — `stepFromRecord` / `runStepNodes` 透传四字段
- `packages/desktop/src/views/changes/flow/run-step-node.tsx` — 标题下常驻行（静态耗时 + token 行）+ running 走秒（`setInterval` 1s，仅活面 running 节点挂载）
- `packages/desktop/src/views/changes/flow/`（新文件，命名 design 定稿，如 `duration.ts`）— 毫秒 → 人读格式化纯函数
- `packages/desktop/package.json` — `version` 0.5.10 → 0.5.11

### 测试文件

- `crates/core/orchestration/src/state_test.rs` / `walker_test.rs` — `ChangeStepState` 构造补字段；emit 铸造语义（running started / 终态 ended / 瞬步同刻）+ `run_worker` token 落值（TurnDone.usage 在场求和 / 缺席 None / 仅 WorkerAgent 步有值）
- `crates/core/orchestration/src/control_test.rs` — `accumulate_node` 合并语义（started 保首见不被终态顶替、ended + token some 才覆写、瞬步单 emit 自然同刻）
- `crates/core/orchestration/src/run_history_test.rs` — `RunNodeEntry` 四字段透传
- `crates/core/workflow/src/queries/detail_test.rs`（或既有 detail 测试落位）— 线面四字段出线与旧行缺省值形态
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/fixtures/` — 种子 `ChangeStepState` 字面量补字段（定值注入）；detail nodes 投影 golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写；补时间 / token 有无两态样本断言
- `crates/infra/store/src/` 既有 run 节点读写回归 — 加列随动 + 旧行（无新列编码）解码缺省形态
- `packages/desktop/src/views/changes/flow/run-step-node.test.tsx` — 常驻行断言（静态耗时文本 / token 行仅 workerAgent / 缺席 "—" / 旧数据 "—"）+ 走秒渲染（fake timers）
- `packages/desktop/src/views/changes/flow/run-state.test.ts` — `stepFromRecord` 四字段透传
- `packages/desktop/src/views/changes/`（duration 测试）— 格式化边界（< 1s / 秒 / 分秒 / 时分级 / 0ms）
- `packages/desktop/src/views/changes/change-detail-view.test.tsx` — `stepRow()` 构造器补字段

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（MCP 工具、hooks、版本 2.10.44、三类交付产物）
- `StepRecord` 审计面与 `StepKind` 封闭集（与执行图节点史职责分立——步级时间不进审计行）
- `RunUpdate` 通知 IPC kind-only 降位形态（载荷不出进程；步状态变更经统一视图重查，通知零载荷不变）
- `SessionStats` / `SessionSummary` / `sessionDetail` / `agentSessionTranscript` 契约（session 域统计面零改动）
- runs 级 `ChangeRunEntry.startedAt` / `finishedAt` ISO 串出线口径（本变更只加节点级毫秒数口径，runs 级不动）
- `RunNodeRecord.timestamp` 列语义（落包时刻，= finished_at 整包同刻——步级时间独立成列，不顶替不复用）
- walker 路由语义 / 相位转移红线（每步过渡仍问写面 `phase_next`；本变更零路由触点）、evaluator 输出协议、决策协议
- 检查域 spawn（static-check / test-execution / mutation-test）与 vcs 步执行语义
- crate 依赖方向（orchestration 不新增 infra 依赖——token 求和上移动因；agent crate 保持禁 workspace 内依赖）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 契约四字段全链在场 | `ChangeStepState` / `RunNodeEntry` / `RunNodeRecord` / `RunNodeStateRecord` / `ChangeRunNodeRecord` / `RuntimeFlowNode` 六载体四字段齐备（i64 / Option 口径一致），bindings 再生成且守卫绿；活面 `ActiveRunView.nodes` / `ChangeRunSnapshot.nodes` 自动跟随、命令面签名零改动 |
| AC-2 | 时间铸造语义 | running emit：`started_at` 铸造、`ended_at=None`；终态 emit（passed / failed / stopped）：`ended_at` 铸造；合并后节点 `started_at` = running 首见值（不被终态 emit 顶替）；`phaseStart` 等瞬步（单 emit）`started_at == ended_at`；stopped 收口（用户停止）同有 `ended_at` |
| AC-3 | token 本步增量 | `run_worker` 终态节点携 in / out 两值 = 本步 `outcome.transcript` 的 TurnDone.usage 求和；usage 缺席 → None；非 WorkerAgent 步（工具步 / Gate / Git）恒 None；终态单次落值即会话结束后累计结果（无二次刷新通道） |
| AC-4 | 合并语义 | `accumulate_node`：`started_at` 无条件保首见；`ended_at` / `token_input` / `token_output` some 才覆写；同键多 emit 后快照 / 落库值与合并规则一致 |
| AC-5 | 落库与旧行兼容 | 新 run 落库行四列值与累积器逐字段一致；旧库行（无新列）解码 `started_at=0` / `ended_at=None` / token None 零报错零迁移；`timestamp` 列原语义原值保留 |
| AC-6 | 出线口径 | `ChangeRunNodeRecord` 出线 `startedAt: number` / `endedAt: number | null` / `tokenInput` / `tokenOutput`（毫秒数，与活面同形、纯 derive）；runs 级 `startedAt` / `finishedAt` 仍 ISO 串 |
| AC-7 | 节点呈现 | 标题下常驻行：已收口步显静态耗时（前端 `ended - started` 计算）；token 行仅 `workerAgent` 组渲染、缺席 "—"；`startedAt<=0` 或 `endedAt=null` 的非 running 态显 "-"（旧库读史含）；0ms 瞬步不显耗时缀 |
| AC-8 | running 走秒 | 活面（在飞 run）running 节点以 `Date.now() - startedAt` 每秒刷新呈现；节点收口（状态离开 running）后 timer 卸载、回落静态耗时；库读史 status=running 残留节点不走秒（静态 "-"） |
| AC-9 | corpus 与 golden | 种子构造器以定值注入四字段（零 wall-clock 进 golden）；时间 / token 有无两态样本在案；detail nodes 投影 golden diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕 |
| AC-10 | 求和上移 | `sum_usage_tokens` 唯一实现居 core/agent（orchestration / infra/store 两消费方同源引用）；store 会话统计输出与上移前逐字段一致；orchestration 依赖面零 infra 新增 |
| AC-11 | 版本与管线 | `packages/desktop` version 0.5.11（`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 仍 2.10.44；`vp test` / `client:check`（fmt / lint 含 50 行函数限）/ knip 全绿 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 走秒 `setInterval` 为前端全库首例：timer 泄漏 / 节点卸载后残留 / 画布级重渲染 | running 节点耗时卡死或全局性能劣化 | 中 | 走秒仅活面 running 节点挂载（同刻通常 ≤ 1 个——walker 串行）；timer 生命周期绑定节点组件（卸载即清）；xyflow 节点独立渲染、局部 state 不惊动画布；AC-8 断言收口回落 |
| R2 token 口径与抽屉并存：节点显本步增量、`SessionMeta` 显 session 全量，同 session 多步两数不同 | 用户对「同会话两节点 token 不等 / 之和 ≠ 抽屉值」困惑 | 中 | 用户已拍板（本步增量）；反馈轮（static-check 修复续注同 executor session）各节点增量自包含、语义可解释；文案以「tokens in/out」与抽屉同名不叠加解释（design 可复核措辞） |
| R3 golden / corpus 波及面：`ChangeStepState` 构造点 ~20 处（state_test / control_test / run_history_test / corpus_golden_test / walker_test / mod_test）+ detail nodes golden 全量字段变化 | 机械补字段遗漏 → 编译红 / golden 红 | 高（机械） | 编译器驱动逐处补；golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认）；AC-9 汇总断言 |
| R4 旧库行 `started_at` 缺省 0 的语义负担：i64 非可选 + serde default → 前端需识别 0 为缺席 | 口径泄漏进前端判断（`startedAt<=0` 魔数） | 低 | 合并判定收格式化工具单点（0 / 缺席 → "-"）；AC-7 断言旧行形态；显式 doc 注释留痕（决策 4） |
| R5 两套时间出线口径并存（runs 级 ISO 串 / 节点级毫秒数） | 后续消费方拿错口径 | 低 | queries delta 显式留痕两口径裁定（活面同形推论）；前端仅经 `UnifiedRunStep` 单一形状消费 |
| R6 emit 铸造碰 corpus 确定性：44 个 emit 点全走 `now_millis()` 生产时钟 | golden 注入 wall-clock 漂移 | 低 | corpus 种子直构 `ChangeStepState`（不经 emit_node）注入定值；golden 断言既有粒度不含 wall-clock（词面 / 计数）；实施时确认 corpus_golden_test 断言粒度（explore 坑 4 已留） |
| R7 `sum_usage_tokens` 上移动错方向（复制副本而非上移） | 双实现漂移 | 低 | AC-10 断言唯一实现 + 两消费方同源；函数 12 行、上移成本低 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| token 口径 | 路线 A：后端随行、本步 turn 增量 | 用户拍板（explore 决策 1）；`outcome.transcript` 自包含，图史与实时面同源，零额外查询；TurnDone 单次落值即「会话结束后累计」 | 路线 B 前端按 sessionId join（running 节点无 id、session 清理后缺数）；session 全量累计（同 session 多节点同值，困惑面更大） |
| 耗时计算归属 | 后端只存起止（i64 毫秒），前端算差值 | 用户拍板（决策 2）；落库面零派生列；running 走秒复用同一 `startedAt` | 后端落 duration 列（冗余派生、running 无值） |
| 显示位置 | 节点标题下常驻行 | 用户拍板（决策 3，非 HoverCard） | HoverCard（explore 已排除） |
| 旧库读史 | 显示 "-"，不从 turn 行回填 | 用户拍板（决策 4）；`#[serde(default)]` 旧行零迁移 | TurnSummary 时间回填近似值（复杂度不成比例） |
| `started_at` 合并语义 | accumulate 侧无条件保首见 | 与 sessionId 首见语义对称；终态 emit 携同刻值被丢弃，瞬步单 emit 首见即真值，无需区分「首 emit 是否终态」 | 终态行回带原 started 值（emit 间传值复杂化） |
| 节点级出线形状 | 毫秒数（与活面 `ChangeStepState` 同形） | `UnifiedRunStep` 单一形状、`stepFromRecord` 纯复制、走秒直减 `Date.now()` | ISO 串（活面 raw 序列化需命令层逐节点转换，多一层映射） |
| token 求和归属 | 上移 core/agent，store 改引 | 依赖方向 infra → core 不可反向；agent crate 禁 workspace 内依赖而函数仅吃 `&[AgentEvent]`；上移比留副本干净 | orchestration 留私有副本（双实现漂移） |
| running 走秒范围 | 仅活面（在飞 run）节点 | 库读史 running 残留节点 run 已死，走秒是假象；统一视图 runId 可辨归属 | 一律走秒（interrupted 残留永久走秒） |

### 待决问题

- 走秒实现形态：节点组件内自持 interval + 局部 state，vs 自定义 hook 提炼（多节点复用度低，倾向组件内）；卸载清理由组件生命周期承载——design 定稿。
- token 缩写格式（`12k/3k` vs 全量数字 vs `12.3k`）与耗时格式细节（`1.2s` / `3m20s` / `1h05m` 分级边界、0ms 省显规则）——随实现定稿。
- `sum_usage_tokens` 上移落位（`core/agent/src/session.rs` 内 vs 新 `usage.rs` 模块）与命名（保留原名 vs 更明确）——design 定稿。
- 常驻行视觉形态（图标 / 分隔符 / 与 detail 行的排布）——design 定稿，data-testid 挂钩在案。
- corpus_golden_test 断言粒度复核（若既有 golden 对 nodes 序列化全量断言，确认定值注入路径不受 emit 生产时钟影响）——实施时确认（explore 坑 4）。
