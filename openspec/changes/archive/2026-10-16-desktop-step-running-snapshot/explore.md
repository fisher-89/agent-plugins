# 步级起止时间与 token 消耗上流程图节点

workflow_type: requirement

## 主题与背景

用户诉求（桌面端 change 流程）：
1. step 记录开始时间与结束时间；
2. 流程图节点上显示该步耗时；
3. agent step（WorkerAgent 步）额外显示关联 session 的 token 消耗。

现状结论（一句话）：**步级时间戳在 Rust 后端 → 落库 → 前端的全链路上完全缺席**；token 消耗在 session 域已有完整数据面（`SessionStats`），但流程图节点渲染层（`RunStepNode`）拿不到它。

## 数据流全景（现状）

```
Rust walker (步执行)                Rust control (RunGuard)            前端
─────────────────────              ─────────────────────────          ─────────────────────
emit_node(running)  ──RunUpdate::Step──▶ accumulate_node               ActiveRunView.nodes
     ...执行步...                       （同身份键就地合并：            （活面投影，含 sessionId）
emit_node(passed/failed/stopped)       status/sessionId/detail 覆写）   │
     .with_session(outcome.session_id)        │                        ▼
run 收口: finish_command              snapshot() 快照面          runStepNodes() → RuntimeFlowNode
  nodes 整包 ──▶ store RunNodeRecord  （进程内，不落库）          → RunStepNode 渲染
```

关键文件锚点：

| 层 | 文件 | 说明 |
|---|---|---|
| 步状态行（活面契约） | `src-tauri/crates/core/orchestration/src/state.rs:131` | `ChangeStepState` —— **无任何时间字段** |
| emit 单点 | `orchestration/src/walker.rs:440` `emit_node` + `StepSettle`（380-460） | 双 emit 模式的产生源 |
| 节点累积 | `orchestration/src/control.rs:403` `accumulate_node` | 同身份键就地合并（status/sessionId/detail/outputRef 覆写） |
| 落库命令 | `orchestration/src/run_history.rs:63` `finish_command` | `RunNodeEntry` 整包映射 |
| 落库行 | `src-tauri/crates/infra/store/src/model.rs:934` `RunNodeRecord` | 仅有 `timestamp`，且 = finish 的 `finished_at` **整包同刻**（所有节点同一值，非步级时间） |
| 生成绑定 | `packages/desktop/src/types/generated/bindings.ts:679` | `ChangeStepState` TS 面 |
| 前端视图合成 | `src/views/changes/flow/run-state.ts` | `stepFromRecord` / `UnifiedRunStep` / `runStepNodes` |
| 节点类型 | `src/views/changes/flow/types.ts:48` `RuntimeFlowNode` | **无时间字段** |
| 节点渲染 | `src/views/changes/flow/run-step-node.tsx` | `RunStepHeader`：徽章 + 步文案 + attempt + 状态缀语——耗时/token 的落点 |

## 线索一：步级时间戳的天然锚点 —— 双 emit 模式

每个步的生命周期在 walker 中恒为两次 `emit_node`（同一稳定身份键 `(phase, attempt, step, instance_no)`）：

```
begin_node(建边) ─▶ emit_node(StepSettle::running())   ← 开始时刻锚点（此刻无 sessionId）
        │
        │  执行步（run_tool / run_worker）
        ▼
emit_node(StepSettle::passed/failed/stopped             ← 结束时刻锚点
          .with_session(...) / .with_output_ref(...))
```

- 工具步样例：`walker.rs:756-793`（static-check 的 running → passed/failed + feedback 循环）
- agent 步样例：`walker.rs:2993-3068`（run_worker：running emit → `worker.run()` → 终态 emit 携 `outcome.session_id`）
- 注意：phaseStart 是例外——直接 emit passed（瞬步，起止同刻）；gate 步、归档步各有同款模式（emit_node 共 40+ 调用点，见 grep）。

改法推演（最小侵入）：在 `ChangeStepState` 增 `started_at: i64` + `ended_at: Option<i64>`（UTC unix 毫秒，与域内既有 `now_millis()` 口径一致——`agent/src/event.rs:5`）：
1. walker：running emit 铸 `started_at`（ended_at = None）；终态 emit 补 `ended_at`；
2. `accumulate_node`（control.rs:403）：合并时保留首见 `started_at`、覆写 `ended_at`（与 sessionId 同款"some 才覆写"语义）；
3. `finish_command` / `RunNodeEntry` / `RunNodeRecord` 透传落库（store 表加列，`#[serde(default)]` 先例在 model.rs:956）；
4. bindings 重新生成 → 前端 `stepFromRecord` / `RuntimeFlowNode` 透传 → `RunStepNode` 渲染耗时。

落库面可保留现有 `timestamp`（整包同刻）不动，步级时间独立成列。

## 线索二：token 消耗 —— 数据已存在，缺的是"节点 → session stats"的通道

session 域已有完整统计面：

- `SessionStats`（`agent/src/session.rs:91`）：`turn_count` / `total_duration_ms` / `input_tokens` / `output_tokens`（自 TurnDone usage 鸭子类型求和，`infra/store/src/store.rs:164` `sum_usage_tokens`）；
- `sessionDetail` 命令：按 id 单查三件套（row + stats + turns）；
- 前端已有消费先例：抽屉 `SessionTranscriptPanel` → `useSessionTranscript`（`views/changes/hooks/use-session-transcript.ts:38`）→ `sessionDetail` 单查，`SessionMeta` 已渲染 `tokens in / out`（session-transcript-panel.tsx:40-42）。

因此"agent 节点显示 token"存在两条实现路线：

```
路线 A（后端随行）：run_worker 收口处从 outcome.transcript 的
    TurnDone.usage 求和 → 写入 ChangeStepState 新字段（如 token_usage）
    → 落库 + 活面同源，前端直读直渲染（与 sessionId 同款路径）
    优点：图史（重挂/重启后读库）与实时面一致，零额外查询
    缺点：ChangeStepState 契约 + 落库表加列，改动面大

路线 B（前端 join）：节点渲染层按 sessionId 批量查 session stats
    优点：后端零改动
    缺点：N 个节点 N 次查询（或需新批量命令）；库读史节点在
          session 清理后会缺数；running 节点 sessionId 尚未回写（见坑）
```

## 线索三：语义歧义点 —— "关联 session 的 token"是全量还是本步增量？

关键发现：**feedback 续注机制下同一 session 承载多个步实例**。static-check / test-execution 失败后走 `run_worker(continue_session = executor_session)`（walker.rs:818-833），修复轮跑在同一 executor 会话里——session 是跨步共享的。

由此 token 显示有两种口径：

```
口径 1（session 全量累计，SessionStats 现成）：
  同 session 的多个 executor 步节点会显示相同的 token 数
  —— 用户看到"3 个节点都是 120k"可能困惑，但语义是"该会话总消耗"

口径 2（本步 turn 增量，需现算）：
  run_worker 的 outcome.transcript 就是本 turn 的事件全集，
  其中 TurnDone.usage 求和即本步增量——最干净的自包含口径
  （TurnSummary 轮行已有 duration_ms / cost_usd 先例，但无 token 字段）
```

## 坑与边界（探索中识别）

1. **running 节点无 sessionId**：`session_id` 仅在终态 emit 携带（walker.rs:3067）；运行中节点上 sessionId = null（命令层 sink 桥只更新 control 注册表槽位，不回写节点行）。路线 B 对 running agent 节点拿不到 session → token 无法显示；路线 A 不受影响（token 也只在终态落值，running 节点显示"—"即可）。
2. **running 节点的耗时是"活值"**：耗时 = ended_at - started_at，但 running 时 ended_at 为 null → 前端需以 `Date.now() - started_at` 实时走秒渲染（或节点上只显示已收口步的静态耗时 + running 步动态计时——需要定时器/重渲染策略，xyflow 节点内自持 interval 可行）。
3. **stopped / 崩溃面**：进程被杀时终态 emit 不会发生 → 节点留 running + started_at、ended_at 恒 null。库读史里这类节点（`RunStepStatus::Running` 收口在途）耗时只能显示"—"或以 run finished_at 兜底。
4. **corpus 确定性测试**：run_history / golden 测试对时间戳敏感（`started_at` 命令层铸造、`finished_at` 整包同刻——注释多处强调 corpus 确定性）。步级时间需同样走"命令层/emit 处单点铸造 + 测试注入固定时钟"的模式，否则 golden 测试全红。
5. **phaseStart 等瞬步**：单 emit 直接 passed——started_at = ended_at（耗时 0ms），渲染层可对 0 耗时不显示。
6. **时长格式化无现成工具**：前端搜不到 formatDuration 类工具，节点内需新写毫秒 → 人读格式（如 `3m20s` / `1.2s`）；run 级已有 startedAt 显示先例可参考（`ActiveRunView.startedAt`，run-control-panel）。

## 待与用户对齐的决策点

1. token 口径选**本步增量**（推荐，路线 A 顺带解决）还是 **session 全量累计**（路线 B 即可，或 A/B 混合：节点显示增量、抽屉已显示全量）？
2. running 节点是否需要动态走秒耗时，还是只在步收口后显示静态耗时（实现复杂度差异明显）？
3. 耗时显示粒度：节点正文一行小字 vs HoverCard 内（detail 现有 HoverCard 先例，run-step-node.tsx:113-124）？
4. 库读史（旧 run）无步级时间——迁移语义：显示"—"即可，还是要从 turn 行（TurnSummary.startedAt/finishedAt）回填近似值？

## 相关探索

- `openspec/explores/consolidate-agent-session.md`（session 域既有调研）

---

## 决策记录（用户拍板，探索第二轮）

1. **token 口径 = A（后端随行，本步增量）**；会话结束后若大模型返回累计结果（TurnDone.usage），刷新为该值。
2. **接口返回起止时间**（startedAt / endedAt），前端计算静态耗时；running 步前端走秒。
3. **显示位置 = 图内节点标题下常驻**（非 HoverCard）。
4. **旧库读史无步级时间 → 显示 "-"**（不从 turn 行回填）。

## 决策后的方案细化

### 契约变更（单点：`ChangeStepState`）

```
ChangeStepState {
    ...既有字段...
    started_at: i64,          // running emit 时铸造；phaseStart 等瞬步 = ended_at 同刻
    ended_at: Option<i64>,    // 终态 emit 时铸造；running 恒 None
    token_input: Option<u64>, // 本步 turn 增量（TurnDone.usage 求和）；缺席合法缺省
    token_output: Option<u64>,// 同上；仅 WorkerAgent 步有值
}
```

透传链（全直线复制，无逻辑分支）：
`ChangeStepState` → `accumulate_node`（合并语义见下）→ 活面 `ActiveRunView.nodes` / `ChangeRunSnapshot.nodes` → `finish_command` → `RunNodeEntry` → `RunNodeRecord`（store 加列，`#[serde(default)]` 先例 model.rs:956）→ bindings → `stepFromRecord` / `UnifiedRunStep` / `RuntimeFlowNode` → `RunStepNode`。

### 时间铸造点（复用既有模式）

`walker.rs:3125` 已有 `now_millis()` 收口铸造先例——注释明确「corpus 确定性由测试注入 started_at / 假 history 承载，本时钟只在生产路径消费」。步级时间同款：`emit_node` 内按 `settle.status` 铸造（running → started；终态 → ended），无需新增时钟注入缝。

### 合并语义（`accumulate_node`，control.rs:403）

与 sessionId 同款「some 才覆写」：
- `started_at`：**保留首见值**（running emit 落值后不被终态 emit 顶替——终态行需带同值或 accumulate 侧保首见，二选一，倾向后者与 session_id 对称：首见即定）
- `ended_at` / `token_input` / `token_output`：终态覆写
- 注意：终态 emit 构造 `ChangeStepState` 时这四个字段如何填——若 running/终态两次 emit 都从 `StepSettle` 构造，`StepSettle` 需扩四字段（running() 全 None/零，终态构造时补）

### token 求和函数归属（依赖方向约束）

`sum_usage_tokens` 现居 `infra/store/src/store.rs:199`，但 orchestration 在 core 域、依赖方向 infra → core 不可反向。方案：**上移 core/agent**（`AgentEventKind::TurnDone` 定义所在 crate，orchestration 已依赖 agent），infra/store 改引 core 实现（或留私有副本——函数 12 行，上移更干净）。`run_worker` 终态 emit 前对 `outcome.transcript` 求和落值。

### 「会话结束后再刷新一次」的落点澄清

`run_worker` 是 await 到收口才构造终态（`walker.rs:3021` `worker.run(turn).await` → outcome.transcript 已含 TurnDone）——**终态 emit 单次落值即会话结束后的累计结果**，无需二次刷新通道。usage 缺席（引擎未返回）= Option None，前端显示 "—"。running 期间 token 无值，同样 "—"。

### 前端渲染（`RunStepNode`）

```
┌────────────────────────────┐
│ [WorkerAgent] 执行 attempt 2 │  ← 既有头行
│ ⏱ 3m20s   ▸ tokens 12k/3k  │  ← 新增常驻行（标题下）
│ （detail 短文本既有行）       │
└────────────────────────────┘
```

- 静态耗时：`ended_at - started_at`（前端计算，后端不落 duration）
- running 走秒：节点组件内 `setInterval(1s)` 自持 tick，仅 `status === 'running'` 时挂载（**前端全库无 setInterval 先例**，全新模式；xyflow 节点独立渲染，局部 state 更新不惊动画布）
- token 行：仅 `group === 'workerAgent'` 步渲染；in/out 两个值，缺席 "—"
- 格式化工具：新写（毫秒 → `1.2s` / `3m20s` 类），无既有先例
- 旧数据（库读史无新列值）：耗时与 token 均 "—"（决策 4）

### 测试面影响预估

- Rust：`ChangeStepState` 构造点 ~20 处（state_test / control_test / run_history_test / corpus_golden_test / walker_test / mod_test）需补字段——编译器驱动逐处补
- corpus golden：注释口径「本时钟只在生产路径消费」，golden 断言不含 wall-clock 具体值的话零影响；若 golden 对 nodes 序列化全量断言则需注入定值（实施时确认 corpus_golden_test 断言粒度）
- 前端：`run-step-node.test.tsx` 增耗时/token 行断言；`change-detail-view.test.tsx` 的 `stepRow()` 构造器补字段

### 实施切面建议（后续提案的 task 切分参考）

1. core：`ChangeStepState` + `StepSettle` + `emit_node` 铸造 + `accumulate_node` 合并语义 + token 求和上移
2. store：`RunNodeEntry` / `RunNodeRecord` 加列透传
3. bindings 重生成 + 前端 `run-state.ts` / `types.ts` 透传
4. 前端：`RunStepNode` 渲染（静态耗时 + 走秒 + token）+ 格式化工具
