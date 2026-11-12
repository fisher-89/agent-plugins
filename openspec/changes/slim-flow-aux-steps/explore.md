# 相位机精简：辅助步退场与 step 收口内化

workflow_type: requirement

> 本文是**结论稿**。探索过程中的方案比选、推翻与纠偏已剔除，只保留可执行的定稿、
> 代码锚点与验收口径。后续课题（step 抽象模型）见 `step-abstraction-model.md`。
>
> **基线**：已同步至 `agent-finalize-tool-for-flow-steps`（2026-11-11）落地后的代码——
> 流程 Agent 收口改为 `finish_step` 工具协议，`parse_verdict` / 反馈环 / 漂移计数均已退役。
> 该变更 proposal 记录本课题为「A3 与之重叠，待其完成后再单独更新」，本文即该更新。

---

## 0. 一句话范围

把相位表里的三类辅助步（`PhaseStart` / `VerdictGate` / `PhaseLog`）从执行图与计划图中移除：
开相改为相位入口的固定工程动作，verdict 解析内化进 evaluator 步，落账改为相位收口动作。

```
requirement 相位表：10 相位 55 步 → 28 步（辅助步 26 → 0）
一次全绿 run 的执行图：55 节点 / 54 边 → 28 节点 / 27 边
```

---

## 1. 起点数据

```
INITIALIZE_STEPS       5  PhaseStart Clean StaticCheck TestExecution PhaseLog
PROPOSAL_STEPS         5  PhaseStart Executor Evaluator VerdictGate PhaseLog
CODE_ANALYZE_STEPS     5  同上
DEV_DESIGN_STEPS       5  同上
TEST_DESIGN_STEPS      5  同上
IMPLEMENT_STEPS        7  PhaseStart Executor StaticCheck ArchiCheck Evaluator VerdictGate PhaseLog
TEST_GEN_STEPS         6  PhaseStart Executor StaticCheck Evaluator VerdictGate PhaseLog
TEST_EXECUTION_STEPS   4  PhaseStart TestExecution MutationTest PhaseLog
ARCHIVE_STEPS         11  PhaseStart Preflight Commit Rebase ResolveConflicts Merge
                          SpecSync Cleanup Seal Finalize PhaseLog
CODE_REVIEW_STEPS      4  PhaseStart Evaluator VerdictGate PhaseLog
ACCEPTANCE_STEPS       4  同上

requirement 表 10 相位 = 55 步，辅助 26（PhaseStart×10 + PhaseLog×10 + VerdictGate×6）
test-only  表  7 相位 = 41 步，辅助 18
```

**目标相位表**（辅助步全部退场，`ArchiCheck` 保留）：

```
initialize       [Clean, StaticCheck, TestExecution]                    3
proposal         [Executor, Evaluator]                                  2
dev-design       [Executor, Evaluator]                                  2
test-design      [Executor, Evaluator]                                  2
implement        [Executor, StaticCheck, ArchiCheck, Evaluator]         4
test-gen         [Executor, StaticCheck, Evaluator]                     3
test-execution   [TestExecution, MutationTest]                          2
code-review      [Evaluator]                                            1
acceptance       [Evaluator]                                            1
archive          [Preflight, Commit, Rebase, ResolveConflicts,
                  Merge, SpecSync, Cleanup, Seal, Finalize]             9
                                                    ─────────────────────
                                                      28 步
```

---

## 2. 关键事实（决定「删掉不损失信息」）

```
                      ┌───────────────── ① 计划图（源码）
                      │   PhaseDefinition.steps（&'static [StepDefinition]）
                      │   ← phase_next 每轮 clone 下发给 walker
                      ▼
  ② 执行图节点（进程内 + db）        ③ 审计行（db）            ④ 落账（db）
  begin_node / emit_node             store.append_step          store.log_phase
  → RunNodeRecord + RunEdgeRecord    → StepRecord               → PhaseRecord + checklist
  → 前端 flow 图渲染节点               → 无任何 UI 消费           → 前端 pipeline.attempts

  PhaseStart   ✓ 节点                 ✓ StepKind::PhaseStart    ✗
  VerdictGate  ✓ 节点                 ✗（StepKind 里没有该词）   ✗
  PhaseLog     ✓ 节点                 ✓ StepKind::PhaseLog      ✓（本体的职责）
```

- `ChangeDetail`（`workflow/queries/detail.rs:191`）**不含 steps**：计划图与审计行都不出线；
- `StepRecord` 有 port 读面 `list_steps`（`workflow/state.rs:784`），前端**零消费**；
- 前端把 PhaseStart / PhaseLog / Gate 三门渲染为**静默节点**：`role` 与 `outputRef` 皆 null，
  点击不触发抽屉（`views/changes/flow/change-flow-graph.tsx:84`）。

三个辅助步的实质性外设极少：

| 辅助步 | 外设 | 失败语义 | 图面价值 |
|---|---|---|---|
| PhaseStart | `store.start_phase`（active_phase 写 + attempt 推导） | Terminal | attempt / 进入时刻（列头可承载） |
| VerdictGate | 零外设（纯解析）；失败续注 evaluator | FeedbackLoop ≤5 / Terminal | 「解析失败」证据 |
| PhaseLog | `store.log_phase`（PhaseRecord + 清位） | Terminal | 「已落账」（pipeline 本已可见） |

---

## 3. 定稿决策（六条）

| # | 决策 |
|---|---|
| D1 | `start_phase` 保留为相位入口的固定工程动作：**写 active_phase（含 attempt / start_at），零 emit、零审计、零计划步** |
| D2 | verdict 裁决内化进 evaluator 步：读 `outcome.finalized` 的 `FinishPayload::Evaluator` 臂；**缺席即 `failed` + `Terminal`**（截断 / 未收口两文案）。协议纠错由 `finish_step` 工具在同一 turn 内闭合，walker 无解析分支、无重试环 |
| D3 | `PhaseLog` 退出图面，改为相位收口动作（B1 形态）：**落账失败 → run failed + reason 前缀，节点保持全绿** |
| D4 | 驳回 / 回跳的可见性由**边的走向 + 节点 detail** 承载，不靠独立 gate 节点 |
| D5 | 「相位内所有步 passed 照常展示 pass」；**step 内部遇系统异常仍标 `failed`** |
| D6 | `ArchiCheck` 保留，与 static-check 同类（检测型门禁），计数独立 |

### 双失败面（D3 + D5 的边界）

```
相位内某 step 抛系统异常（worker Err / 会话 Failed）
   → 该 step 节点 failed + run failed                  ← 图上可见红节点
相位内所有 step 都 passed，只有 settle_phase 落账失败
   → 节点全绿 + run failed + reason「相位 X 全部步通过但落账失败」  ← D3 的代价
```

两者在 reason 文案上必须可区分，否则用户看到「全绿但失败」会误判为缺陷。

---

## 4. 落地后的相位机

```
① phase_next ─► done → Completed
             ├► error(VerdictFail / MaxRetries) → resolve_deadlock() → continue
             └► next_phase 缺位 / steps 空 → Terminal::failed（计划图漂移）
                          ▼
② stop_before 停止门（archive 前）：ConfirmWait + archive 预检载荷；未确认 → stopped
                          ▼
③ enter_phase()   写 active_phase + start_at + attempt；零 emit / 零审计
                  写失败 → Terminal::failed（编程 / store 错误，重启由启动标定自愈）
                          ▼
④ for step in &next.steps { run_step() }    ← 只剩真实步（含 ArchiCheck）
                  Proceed → 下一步
                  Upgraded → 截断余下步，break（回 ①）
                  Err(Terminal) → 收口
                          ▼
⑤ settle_phase()  取 verdict（三分流按产物在否）→ log_phase → PhaseRecord + 清位
                  落账失败 → Terminal::failed「相位 X 全部步通过但落账失败: …」
                          ▼
⑥ 相位间停等（auto_next_phase=false 且非 stop_gate）→ ConfirmWait；未确认 → stopped ─► 回 ①
```

### 4.1 evaluator 步内部（内化的 verdict 门）

> **基线变更（2026-11-11 `agent-finalize-tool-for-flow-steps`）**：收口协议已从
> 「最终消息输出 JSON + walker 解析」改为 **`finish_step` 工具协议**——校验在工具内
> 闭合（is_error 当场回灌模型，同 turn 内纠正），`parse_verdict` 已删除，
> `verdict_fix_prompt` / 反馈环 / 漂移计数全部退役。故内化后**无解析分支**：

```
step_evaluator(ctx, trace, cursor, step)
  ├─ prompt = next.evaluator.prompt（含 change 头 + 失败反馈段 / 回溯原因）
  ├─ outcome = run_worker(Evaluator, prompt, None)          // 恒单轮
  ├─ match outcome.finalized {
  │    Some(FinishPayload::Evaluator(checklist)) →
  │       emit passed(session_id, tokens, detail="verdict={checklist.verdict}")
  │       cursor.evaluator_turn = EvaluatorTurn { outcome, checklist }
  │       return Proceed
  │    _ →                                                    // None / Evaluator 臂之外
  │       reason = truncated(transcript)
  │           ? "评估输出被截断（output_truncated），未产生 finish_step 收口产物"
  │           : "未调用 finish_step 收口（会话 Completed 但无收口载荷）"
  │       emit failed(detail=diagnose_brief(reason))
  │       return Err(Terminal::failed(reason))
  │  }
  └─ 系统异常（worker Err / AgentRunStatus::Failed / Stopped）
        → emit failed/stopped + Terminal                     // D5 第二半
```

**收口载荷单点**：`transcript::finish_payload`（取末条 `SystemNotice{finish_step}`，
末条损坏不回退前条）；walker 只读 `outcome.finalized`，零二次解析。

**与旧方案的差异**：

| 项 | 旧设想（文本协议） | 现状（finish_step） |
|---|---|---|
| 解析失败环 | 同会话续注 ≤5 次 + 超限升格 | **不存在**——工具 is_error 当场回灌 |
| `parse_fail` 声明 | 需新增 `StepDefinition` 字段 | **不需要**（无消费者） |
| 解析超限落 failed | D2 的一条 | **不需要**（无该路径） |
| 超限升格的 `*_upgraded_phase_log` | 需保留一份 | `step_verdict_gate_upgraded_phase_log` 已删 |
| 终态 emit 时机 | 必须延后到解析成功 | 仍延后到 `finalized` 判定（同理，语义不变） |


### 4.2 决策会话（基线已是单轮收口 + ask 外移）

> **基线变更**：软拒环与漂移计数（`DECISION_DRIFT_LIMIT`、`decision_fix_prompt`）
> 已整体删除；协议纠错在 `finish_step` 工具内闭合（is_error 当场回灌）。
> `ask` 停等已外移到 walker（应答后新开一轮决策会话，resume 全史）。

```
resolve_deadlock(..)
  └─ decision_session(..)
       loop {                                        // 仅 ask 会进入第二轮
         outcome = run_worker(Decision, prompt, continue_session)
         decision-log 挂账（幂等覆写，置于动作裁决之前）
         match outcome.finalized {
           Some(Decision(Ask{q,opts})) → emit RunUpdate::Ask → wait_answer
                                          ├ 终止 → Terminal::stopped
                                          └ 应答 → prompt = decision_answer_prompt(answer)
                                                    continue_session = 本会话 → continue
           Some(Decision(action))      → action          // 落出环
           Some(Evaluator(_))          → Terminal::failed（角色漂移）
           None                        → Terminal::failed（截断 / 未收口两文案）
         }
       }
  └─ 动作裁决（出环后）：
       白名单门 ensure_backtrack_allowed（纯函数）
       ├ 通过 → 返回动作
       └ 越权 → Terminal::failed（**一次即停**，文案携 allowed；无第二轮软拒）
  └─ Backtrack → run_tool(Backtrack) → trace.incoming = Backtrack → Continue
```

原 `RetryGate` 三次 emit 删除；「决策 backtrack → x」写 decision 节点 detail。


### 4.3 边的走向（D4 的可见性载体）

```
正常          Executor#0 ──forward──► Evaluator#0             （verdict=pass）
static-check  Executor#0 ──forward──► StaticCheck#0 ──feedback──► Executor#1 ──► StaticCheck#1
决策 ask      decision#0 ──feedback──► decision#1             （detail：应答后新开一轮）
决策驳回      decision#0 ──failed 节点──► （无后继）            ← 一次即 stop，无第二轮
决策回溯      decision#0 ──backtrack──► implement#2: Executor#0
相位重做      implement#1 ──backtrack──► implement#2: Executor#0（attempt 递增）
```

**verdict 解析失败已无对应边**：`finish_step` 把纠错收进同一个 turn 内（工具 is_error
回灌），不产生跨轮 feedback 边。evaluator 步只会有两种收场——`passed`（有收口载荷）
或 `failed`（截断 / 未收口，且随即 `Terminal`）。原先设计的
`Evaluator#0 ──feedback──► Evaluator#1` 自环**不再存在**。


---

## 5. 实施清单

### 5.1 A 档：删代码时顺手形成的形状

| # | 内容 | 收益 |
|---|---|---|
| A1 | `enter_phase()` 吸收 `step_phase_start` | −58 行 + 消 1 个 match 臂 |
| A2 | `settle_phase()` 吸收 `step_phase_log` | −106 行 + 消 1 个 match 臂 |
| A3 | verdict 裁决并入 `step_evaluator`（读 `finalized`），`eval_outcome` + `verdict` 合为 `EvaluatorTurn` | −67 行 + 消 1 个 match 臂 + **消 1 个非法可达态** |
| A4 | 删除 `Decision / RetryGate / WhitelistGate => Proceed` 三个空臂 | 臂数 20 → 16 |
| A5 | 三份 `*_phase_log` 收敛（`gate_fail` / `test_exec_upgraded` / `mutation_upgraded`） | −150 → 约 −100 行 |


#### A1 `enter_phase()`

```rust
/// 相位入口：写 active_phase + start_at + attempt（单事务，写面既有能力）。
/// 零 emit、零审计、零 StepRecord。
/// 失败 = store 坏 / 编程错误，收敛 Terminal::failed（重启由启动标定自愈）。
async fn enter_phase(
    tools: &Arc<dyn ToolStepPort>,
    request: &RunRequest,
    phase: &str,
) -> Result<u32, Terminal>
```

删除：`step_phase_start`（58 行）、`StepNode::PhaseStart` 分派臂、相位表 10 处声明。

#### A2 `settle_phase()`

```rust
/// 相位收口：取 verdict（三分流按产物在否）→ 落账 → active_phase 清位。
/// 零 emit、零审计。落账失败 = Terminal，reason 携「相位 X 全部步通过但落账失败」。
async fn settle_phase(
    tools: &Arc<dyn ToolStepPort>,
    request: &RunRequest,
    cursor: &PhaseCursor,
    phase: &str,
) -> Result<PhaseLogOutcome, Terminal>
```

三分流原样搬入（语义不变）：

```
cursor.test_outcome 在场        → test_execution_phase_log_input（含 mutation 双槽）
cursor.evaluator_turn 在场      → 该 turn 已解析的 checklist
两者皆无（archive）             → 空 checklist（derive_verdict([]) = Pass）
```

Fail 变体入参：

```rust
struct PhaseFail {
    item: &'static str,                     // 「静态检查」/「架构检查」/「测试执行」/「突变测试」
    report: String,
    evidence: String,
    executor_session: Option<String>,
}
```

删除：`step_phase_log`（113 行）、`step_initialize_skipped_phase_log`（52 行，
改为「enter → `settle_phase(Skipped)`」）、相位表 10 处声明。

#### A3 verdict 裁决内化 + `EvaluatorTurn`

```rust
/// evaluator 步收口产物：会话结果与收口载荷同源，消除「有 outcome 无 checklist」的非法态。
struct EvaluatorTurn {
    outcome: WorkerTurnOutcome,
    checklist: EvaluatorChecklist,   // 取自 outcome.finalized 的 Evaluator 臂
}
```

`step_evaluator` 主体见 §4.1。删除：`step_verdict_gate`（67 行）、
相位表 6 处 `VerdictGate` 声明、`feedback_limit(step, "verdict-gate")` 调用点
（该调用点已随反馈环退役而消失，此处仅确认零残留）。

**前置改动**：`run_worker` 需一次小改——把「emit 终态」从函数内移到调用方可控：

```rust
enum SettleMode { Immediate, Deferred }   // Deferred = 只发 running，终态由调用方决定
```

evaluator 与 decision 用 `Deferred`（终态 emit 须在 `finalized` 判定之后），其余调用点零改动。

#### A4 空臂删除

```rust
// 现状
StepNode::Decision | StepNode::RetryGate | StepNode::WhitelistGate => Ok(StepFlow::Proceed),
// 删除整行——三者不在任何相位表 steps 内
```

#### A5 落账函数收敛

```
保留：step_gate_fail_phase_log（static-check / archi-check 共用，参数化 item + reason）
改造：test_execution / mutation_test 两个 upgraded 落账 → 合入 settle_phase(PhaseFail)
```

> `step_verdict_gate_upgraded_phase_log` 已在基线变更中删除（随 verdict 反馈环退役），
> 本方案不再涉及。逐 grep 现状：现存的 `*_phase_log` 为 5 个
> （`initialize_skipped` / `phase_log` / `gate_fail` / `test_exec_upgraded` / `mutation_upgraded`）。

### 5.2 B 档：改动面小且与本次修改高度契合

#### B1 `steps.rs` id → name 去重

```rust
fn resolve_change_name(
    store: &dyn ChangeStateStore, change_id: &str, what: &str,
) -> Result<String, String>
```

四个门禁臂（StaticCheck / TestExecution / MutationTest / ArchiCheck）现各写一遍
`store.get_change(&change_id)?.map(|r| r.name)`，合流一次。

#### B2 归档 emit 辅助合一

```rust
// 现状 begin_archive_node / settle_archive_node 只差「是否先发 running」
fn open_step(ctx, trace, cursor, kind) -> u32                    // begin_node + running
fn close_step(ctx, cursor, kind, instance_no, settle)            // 单个终态 emit
```

#### B3 决策会话门节点退场

```
删除：WhitelistGate 的 begin/emit（约 50 行）
删除：RetryGate 三次 emit（约 50 行）
保留：白名单门纯函数校验 ensure_backtrack_allowed（越权一次即 Terminal，无第二轮软拒）
落点：回溯目标与驳回原因写 decision 节点 detail
      复用 run_worker 的 SettleMode::Deferred，在 decision 终态 emit 时一次性写
保留：ToolCommand::DecisionLog（业务数据，非节点）
```

### 5.3 从 B 档移除的项

| 原项 | 移除原因 |
|---|---|
| `parse_fail` 字段（原 B1） | `finish_step` 工具协议已在同一 turn 内闭合纠错，无可重试的「解析失败」路径；字段无消费者 |
| 解析超限 D2 | 同上，`Exhausted::Escalate/Terminal` 二分不再存在 |


---

## 6. 两处必须遵守的约束

### 6.1 attempt 推导口径

```
attempt = store 内该 (change_id, phase) 的 PhaseRecord 全量条数（含 stale）+ 1
```

**不能**取 `detail.pipeline[phase].attempts.len() + 1`：`queries/detail.rs` 装配前会过滤
stale 条目，而 backtrack 把下游 pass 条目标 stale 但**保留行**。用 pipeline 条数推导会让
回溯导致 attempt 回退，节点身份键 `(phase, attempt, step, instance_no)` 与历史行撞号。

该式子即 `start_change_phase`（store.rs:1549-1570）与 `log_change_phase`（store.rs:1619-1623）
现在用的同一式，**两处同源、零新语义**。

同时：`phase_next` 的 `round`（窗口口径，`entries.len() - anchor + 1`）与 attempt 是**两个
不同口径**，MUST NOT 合并。

### 6.2 `active_phase.attempt` 字段保留写入

`active_phase` 在相位落账时即清位（store.rs:1662），落账后无法推导「刚结束的是第几次
attempt」；而 `queries/detail.rs:309` / `list.rs:99` 直接读该字段。

结论：**`start_phase` 写面保持原样（含 attempt 推导），只是不再 emit 节点、不再写审计行。**
`infra/store` 零改动。

### 6.3 收口载荷的唯一来源

```
walker 只读 outcome.finalized（WorkerTurnOutcome 字段）
  ↑ 由 transcript::finish_payload 从末条 SystemNotice{finish_step} 反序列化
  ↑ 校验在工具内闭合（finish::validate → Err 即 is_error 回灌模型）

MUST NOT 在 walker 侧重新解析 final_message / 文本 JSON
MUST NOT 恢复任何「解析失败 → 续注 → 重试」分支（该路径已随反馈环退役）
```

`finalized` 为 `None` 的两种成因必须分文案（`truncated(transcript)` 判据）：

```
含 output_truncated notice → 「评估输出被截断（output_truncated），未产生 finish_step 收口产物」
否则                       → 「未调用 finish_step 收口（会话 Completed 但无收口载荷）」
```

---

## 7. 兼容面：三张词汇表全部冻结

```
RunNodeRecord.step 落库 = bincode 枚举下标（infra/store/src/model.rs:954 明确钉死）
  → 变体 MUST NOT 删除或重排

RunStepKind（21 值，workflow/state.rs:497）              ← 落库编码，保持
ChangeStepKind（22 值，orchestration/state.rs:48）       ← 出线 + 节点身份键，保持
StepNode（22 值，workflow/state.rs）                     ← 计划图词汇，辅助步成 decode-only

StepGroup::Gate（workflow/state.rs:221）
  · 非出线（bindings.ts 无 StepGroup）
  · 辅助步退场后无 StepNode 归属 → 保留枚举值 + group() 的 Gate 分支
前端 run-state.ts 的 gate 分组与 STEP_LABEL 保留三词（存量解码必需）
```

结论：**三张表一律「冻结 + 新写面零产出 + 前端保留解码」，零 db 迁移**。
代价是每表长期多 3 个死变体，需注释标注 `decode-only`。
走 `RetryGate` 已确立的先例（orchestration/state.rs:41 的冻结注释）。

---

## 8. spec 影响清单

### 8.1 需重述（22 处，跨 7 个 capability）

| spec 位置 | 现状措辞 | 定为 |
|---|---|---|
| orchestration L25 | 「含 archive 相位 11 步」 | 9 步 |
| orchestration L31 | 「末尾 PhaseLog 空 checklist 落账 pass 后推进 done」 | 「相位收口空 checklist 落账后推进 done」 |
| orchestration L36 | 「声明 Evaluator / VerdictGate 步而 evaluator=None…」 | 去 VerdictGate 词，**保留 ArchiCheck** |
| orchestration L205 | 「门步节点呈失败态，解析失败原因注入同一 evaluator 会话」 | 「evaluator 步读 `finalized`；缺席即 failed + Terminal（截断 / 未收口两文案），零反馈边」 |
| orchestration L494-504 | 「archive 相位在末尾 PhaseLog 步以空 checklist 落账…`step_phase_log` 三分流」 | 改称 `settle_phase`，三分流语义保留 |
| orchestration L522 | 「`PhaseStart → TestExecution → MutationTest → PhaseLog`」 | 「`TestExecution → MutationTest`」 |
| orchestration L546 / 559 | 「phaseStart 等单 emit 瞬步 `started_at == ended_at`」 | **换例**（瞬步已无；用 Cleanup / 幂等跳过步） |
| orchestration L584 | 「`PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`」 | 「`Clean → StaticCheck → TestExecution`」+ 相位收口 |
| orchestration L588 / 595 / 619 | 多处 PhaseLog 措辞 | 同上 |
| orchestration L717-735 | ArchiCheck 的 `on_fail` 描述 | 保留，与 static-check 同类（检测型，计数独立） |
| workflow-write-face L173 | 「`INITIALIZE_STEPS`：PhaseStart → … → PhaseLog」 | 去两端 |
| workflow-write-face L173 | 「`TEST_EXECUTION_STEPS` 四步」 | 两步 |
| workflow-write-face L173 | 「implement 7 步序列」 | 4 步（Executor / StaticCheck / ArchiCheck / Evaluator） |
| workflow-write-face L173 | 「`CODE_ANALYZE_STEPS`：PhaseStart → … → PhaseLog」 | 「Executor → Evaluator」 |
| archive L57 / L70 | 「11 步…PhaseStart（开相，attempt max+1 分配）…PhaseLog」 | 9 步 + 开相归 enter_phase |
| flow-view L278 | 「archive 列呈现 11 步节点…PhaseStart / PhaseLog 为 exec」 | 9 步 |
| flow-view L441 | 「点击 phaseStart / phaseLog / verdictGate / … 不可点击」 | 词表去三词 |
| flow-view L533 | 「`phaseStart` 等瞬步 0ms 不显耗时缀」 | 换例 |
| flow-view L566 / 571 | initialize「四节点（含 PhaseLog）」 | 三节点 |
| state-store L18 / L128 | 二十二值全词汇 | 词汇冻结（见 §7） |
| state-store L152 | 「归档 9 步与 PhaseStart / PhaseLog 全落 RunNodeRecord」 | 无 PhaseStart / PhaseLog |
| test-execution L177 / L191 | 「PhaseStart → … → PhaseLog」 | 两步 |
| mutation-testing L131 | 「机械 checklist 落账进入 PhaseLog」 | 「相位收口落账」 |

### 8.2 需删除（2 条）

- **「白名单门每轮一节点」整条 requirement**（含 scenario）——
  `finish_step` 落地后软拒环已删（越权一次即 `Terminal`），不存在「两轮驳回」；
  本方案进一步删除门节点本身，改为「decision 节点实例递增 + detail 承载动作裁决」。
- **「ToolStep / Git 失败同等可见」中的「写面相位机步」措辞**，例子改为「检查步 / git 步」。

### 8.3 需新增（4 条）

1. **相位收口失败可辨**：相位内全部步 passed 但 `log_phase` 失败 → run failed，
   reason 前缀「相位 X 全部步通过但落账失败」，节点全绿（D3 的可见性代价）。
2. **驳回 / 动作裁决经节点 detail 可见**：`finish_step` 落地后越权一次即 `Terminal`、
   无第二轮软拒，故不再用 `WhitelistGate` 实例计数表达；改为 decision 节点 detail 携
   动作与越权文案（D4）。
3. **attempt 推导口径**：全量含 stale + 1，与 `round` 窗口口径分离（§6.1）。
4. **相位内步骤系统异常仍标 failed**（D5 第二半）：`run_worker` Err → 节点 failed + Terminal，
   与「落账失败」区分。

> 原「解析超限 evaluator 节点落 failed」一条已删：`finish_step` 工具内闭合纠错后，
> 不存在「解析超限」这条路径——只有「未调用 / 被截断」这一种缺席形态。

---

## 9. 实施顺序与验收

```
段 1（代码）      A1–A5 + B1–B4；编译通过 + 行为面 87 处断言全绿
                  形状面断言此时必然红（预期），用 #[ignore] 或临时注释隔离
段 2（形状面测试） 约 40 处断言改写，恢复全绿
段 3（golden）    DESKTOP_GOLDEN_REWRITE=1 重写 → 不带环境变量复跑确认零 diff
```

验收命令：

```
段 1   cargo check + cargo test -p orchestration
段 3   DESKTOP_GOLDEN_REWRITE=1 cargo test -p workflow --test corpus_golden_test
       cargo test -p workflow --test corpus_golden_test          # 复核零 diff
```

### 9.1 工作量重心：测试改写，不是删代码

```
walker_test.rs 158 个测试
  行为面断言（ToolCommand 载荷 → 不受影响）       87 处
  形状面断言（RunStepKind 节点词 → 需改写）       ~40 处代码行
    PhaseLog       代码行 39（其中约 20 是 phase_log_input 载荷断言，实际不受影响）
    PhaseStart     代码行 19（多数是 node_of(PhaseStart, 0) 找锚点）
    WhitelistGate  代码行  8（→ decision 节点实例数断言）
    VerdictGate    代码行  2
```

**改写原则：改断言对象，不改断言意图。**

```
node_of(PhaseStart, 0) 找相位入口锚点          → 改为「该相位首个真实步节点」
step_instances(RunStepKind::WhitelistGate)    → 改为 step_instances(RunStepKind::Decision)
node.step == PhaseLog 的「已落账」断言         → 改为 pipeline attempts / phase_log_input 载荷
```

**tasks 切分：按「代码 → 形状面 → golden」三段，不按模块切**——每段有可独立验收的绿线。

**为什么不先改测试**：形状面断言的新形态依赖 A3 内化后「同一实例 passed、缺席即 failed」
的最终语义；先写测试会写出中间态，代码落定后还得再改一次。

**风险**：`walker_test.rs` 的 6 处 `walk_run(...)` 整链测试（L1531 / 1576 / 1624 / 1669 /
6363 等）断言节点序列与总数，辅助步退场后期望值全部变化——需在段 1 逐条确认，
不要留到段 2 被动修补。

---

## 10. 交付清单（供 proposal 引用）

```
代码   −430 行（A 档 ≈ −410 + B 档 ≈ −20；基线已删掉 verdict 反馈环与解析器，
             故比文本协议时代的估算少约 160 行）
表     26 个辅助步声明删除（PhaseStart×10 + PhaseLog×10 + VerdictGate×6）
       ★ 无新增字段（parse_fail 已随 finish_step 落地而取消）
接口   新增  enter_phase / settle_phase / resolve_change_name / open_step / close_step
       修改  run_worker（+ SettleMode）
       删除  step_phase_start / step_phase_log / step_verdict_gate 三个函数
测试   ★ 工作量重心（见 §9.1）
         形状面 ~40 处改写 + 6 处 walk_run 整链测试期望值重定
         新增 4 条（§8.3）
spec   22 处措辞 + 2 条删除 + 4 条新增，跨 7 个 capability
db     零迁移（三张词汇表冻结，各留 3 个 decode-only 变体）
```

---

## 11. 明确不做（另立 change 时再评估）

```
StepSpec 静态表（exec / check / skip / fail 四项）
14 处「passed + 跳过文案」数据化为 SkipRule
PhaseCursor 拆分（14 字段 → 窄 struct）
run_step 表驱动 / trait 化
StepGroup::Gate 与三张词汇表的死变体清理
feedback_limit(step, label) 的 label 分支改造
  （基线已收缩为 2 个调用点：static-check / archi-check，收益进一步变小）
相位表 VerdictGate 步 on_fail 声明的清理
  （finish_step 落地后两值语义合一，已成惰性字段；基线变更明确留给独立变更）
```

判断标准：**只做「本来就会因辅助步退场而必须重写」的改写；为抽象额外去改的，一律不做。**

详见 `step-abstraction-model.md`。
