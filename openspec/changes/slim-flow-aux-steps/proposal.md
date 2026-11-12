# 提案: slim-flow-aux-steps

> **变更**: slim-flow-aux-steps
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/slim-flow-aux-steps/explore.md`（结论稿：0 一句话范围 / 1 起点数据 / 2 关键事实 / 3 定稿决策六条 / 4 落地后的相位机 / 5 实施清单 A1–A6 + B1–B3 / 6 三处必须遵守的约束 / 7 兼容面词汇冻结 / 8 spec 影响清单 / 9 实施顺序与验收 / 10 交付清单 / 11 明确不做）
> **范围接缝**: 本变更只做**相位表辅助步（`PhaseStart` / `VerdictGate` / `PhaseLog`）退场与其内化动作的落位**：开相 → 相位入口的固定工程动作（`enter_phase`），verdict 裁决 → evaluator 步内读取 `finalized`，落账 → 相位收口动作（`settle_phase`）。MUST NOT 做：`StepSpec` 静态表 / 14 处「passed + 跳过文案」数据化 / `PhaseCursor` 拆分 / `run_step` 表驱动 / `StepGroup::Gate` 与三张词汇表的死变体清理 / `feedback_limit(step, label)` 的 label 分支改造 / 相位表 `VerdictGate` 步 `on_fail` 声明清理（该字段已成惰性字段，见 explore §11）。
> **前序**: 基线已同步至 `agent-finalize-tool-for-flow-steps`（2026-11-11）落地后的代码（`finish_step` 工具协议；`parse_verdict` / 反馈环 / 漂移计数均已退役）。该变更 proposal 记录本课题为「A3 与之重叠，待其完成后再单独更新」，本文即该更新。

---

## 问题

### 现象一：56 步里 27 步不携带任何信息

requirement 相位表实测（`crates/core/workflow/src/write/phase_table.rs`，10 相位）：

```
initialize       5   PhaseStart Clean StaticCheck TestExecution PhaseLog
proposal         5   PhaseStart Executor Evaluator VerdictGate PhaseLog
dev-design       5   PhaseStart Executor Evaluator VerdictGate PhaseLog
test-design      5   PhaseStart Executor Evaluator VerdictGate PhaseLog
implement        7   PhaseStart Executor StaticCheck ArchiCheck Evaluator VerdictGate PhaseLog
test-gen         6   PhaseStart Executor StaticCheck Evaluator VerdictGate PhaseLog
test-execution   4   PhaseStart TestExecution MutationTest PhaseLog
code-review      4   PhaseStart Evaluator VerdictGate PhaseLog
acceptance       4   PhaseStart Evaluator VerdictGate PhaseLog
archive         11   PhaseStart Preflight Commit Rebase ResolveConflicts Merge
                     SpecSync Cleanup Seal Finalize PhaseLog
                 ────────────────────────────────────────────────────────
                 56 步，其中 PhaseStart×10 + PhaseLog×10 + VerdictGate×7 = 27
```

test-only 表 7 相位 41 步、辅助 18（复用同一批 static，另加 `CODE_ANALYZE_STEPS`）。源码中辅助步声明合计 30 处（`PhaseStart`×11 / `PhaseLog`×11 / `VerdictGate`×8）。

> **口径校正（对 explore §1 的修订）**：explore 记「10 相位 55 步 / 辅助 26 / 目标 28」，按相位表现盘实测为 **56 步 / 辅助 27 / 目标 29**（`VerdictGate` 在 requirement 表出现 7 次而非 6 次）。逐相位目标步数见下表，全部以代码复核为准。

### 现象二：三个辅助步的外设极少，图面价值接近零

| 辅助步 | 实质性外设 | 失败语义 | 图面价值 |
|---|---|---|---|
| `PhaseStart` | `store.start_phase`（active_phase 写 + attempt 推导） | Terminal | attempt / 进入时刻（相位列头可承载） |
| `VerdictGate` | 零外设（纯读 `finalized` 分支） | Terminal | 「解析失败」证据（工具化后该形态不再产生） |
| `PhaseLog` | `store.log_phase`（PhaseRecord + active_phase 清位） | Terminal | 「已落账」（`pipeline.attempts` 本已可见） |

三者在执行图上均渲染为**静默节点**（`role` 与 `outputRef` 皆 null，点击不触发抽屉，`views/changes/flow/change-flow-graph.tsx`），审计行（`StepRecord`）前端**零消费**，`ChangeDetail`（`queries/detail.rs`）不含 `steps`——计划图与审计行都不出线。即：删掉三个辅助步，不损失任何被消费的信息，只损失「这三次动作发生过」的旁证。

### 现象三：每加一个相位要记着补两份声明，漏写即运行期计划图漂移

`PhaseStart` / `PhaseLog` 是相位表的**结构约定**而非业务步：`PhaseCursor.attempt` 由 `step_phase_start` 产出（`PhaseCursor::new` 里 `attempt` 恒 0），`PhaseLog` 是唯一落账入口。新相位漏写 `PhaseStart` 会让该相位全程 `attempt = 0`（节点身份键撞号），漏写 `PhaseLog` 则相位永不落账、`phase_next` 反复重路由；`VerdictGate` 缺产物时 `step_verdict_gate` 直接 `Terminal::failed`（「计划步漂移」）。这三种错法都不是业务错误，却都以运行期失败呈现。

### 现象四：verdict 裁决被拆成两个节点、两次状态搬运

evaluator 步跑完后，收口载荷已在 `WorkerTurnOutcome.finalized`（`Some(FinishPayload::Evaluator(checklist))` 或 `None`）——判据完全自足。但现盘面把它拆成 `cursor.eval_outcome`（`Option<WorkerTurnOutcome>`）+ `cursor.verdict`（`Option<EvaluatorChecklist>`）两个槽，中间由 `VerdictGate` 节点搬运，于是产生一个**非法可达态**：有 `eval_outcome` 而 `verdict` 为 `None`（gate 未跑 / 中途失败），下游 `step_phase_log` 只能靠 `let Some(..) else` 兜成「空 checklist 落账」。同一份事实被表达成两个可能不同步的槽位。

---

## 提案

把三类辅助步从执行图与计划图中移除，其职责内化为**相位机的固定动作**与**真实步的内部逻辑**：

```
相位机（walker）
  ① phase_next ─► done → Completed
               ├► error(VerdictFail / MaxRetries) → resolve_deadlock() → continue
               └► next_phase 缺位 / steps 空 → Terminal::failed（计划图漂移）
  ② stop_before 停止门（archive 前）：ConfirmWait + archive 预检载荷；未确认 → stopped
  ③ enter_phase()   写 active_phase + start_at + attempt；零 emit / 零审计
  ④ for step in &next.steps { run_step() }      ← 只剩真实步（含 ArchiCheck）
  ⑤ settle_phase()  单入口三形态（Pass 三分流 / Fail 升格与失败 / Skipped 短路）
                    → log_phase → PhaseRecord + 清位
  ⑥ 相位间停等（auto_next_phase=false 且非 stop_gate）→ ConfirmWait
```

目标相位表（辅助步全部退场，`ArchiCheck` 保留）：

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
                                                      29 步（−27）
```

### 决策表（六条，逐条自足）

| # | 决策 | 被拒备选与理由 |
|---|---|---|
| D1 | `start_phase` 保留为相位入口的固定工程动作 `enter_phase()`：**写 active_phase（含 attempt / start_at），零 emit、零审计、零计划步** | **被拒：把 `start_phase` 也删掉，attempt 改由 walker 推导**——attempt 语义依赖 store 内该 `(change_id, phase)` 的 PhaseRecord 全量条数（含 stale），walker 不持 store 句柄，自推导要么多读一次库、要么与写面口径分叉。保留既有写面能力、只去掉节点面，是零新语义的最小改法 |
| D2 | verdict 裁决内化进 `step_evaluator`：读 `outcome.finalized` 的 `FinishPayload::Evaluator` 臂；**缺席即 `failed` + `Terminal`**（截断 / 未收口两文案）。walker 无解析分支、无重试环 | **被拒：保留独立门步但只降为「零节点」**——门步的全部价值由 evaluator 步内的一个 `match` 覆盖，留一个不出节点的空步只会让相位表继续背结构约定 |
| D3 | `PhaseLog` 退出图面，改为相位收口动作 `settle_phase()`：**落账失败 → run failed + reason 前缀「相位 X 全部步通过但落账失败」**，节点保持全绿 | **被拒：落账失败仍 emit 一个 failed 节点**——要为它单独保留一个「收口节点」词汇与身份键，等于把刚删的东西换名留下；D3 的代价（全绿但失败）用 reason 文案消解 |
| D4 | 驳回 / 回跳的可见性由**边的走向 + 节点 detail** 承载，不靠独立 gate 节点 | **被拒：保留 `WhitelistGate` 节点承载越权证据**——白名单门已由「多轮软拒」降为「一次即 `Terminal`」，不存在「驳回后纠正」的两个节点实例；一次判定写进 decision 节点 detail 即可 |
| D5 | 「相位内所有步 passed 照常展示 pass」；**step 内部遇系统异常仍标 `failed`** | **被拒：相位级统一染色**——相位内系统异常（worker `Err` / 会话 `Failed`）若也走「全绿 + run failed」，用户将失去定位到具体步的唯一入口 |
| D6 | `ArchiCheck` 保留，与 static-check 同类（检测型门禁），计数独立 | **被拒：顺手把 ArchiCheck 并入 static-check**——两者 runner、报告槽位、反馈计数上限（3 vs 5）皆独立，合并属另一课题 |

### 双失败面（D3 + D5 的边界，必须可区分）

```
相位内某 step 抛系统异常（worker Err / 会话 Failed）
   → 该 step 节点 failed + run failed                 ← 图上可见红节点
相位内所有 step 都 passed，只有 settle_phase 落账失败
   → 节点全绿 + run failed + reason「相位 X 全部步通过但落账失败: …」  ← D3 的代价
```

两者在 reason 文案上 MUST 可区分，否则「全绿但失败」会被误判为缺陷。

### 实施清单

**A 档（删代码时顺手形成的形状）**

| # | 内容 | 收益 |
|---|---|---|
| A1 | `enter_phase()` 吸收 `step_phase_start` | 消 1 个 `run_step` 臂 + 相位表 11 处声明 |
| A2 | `settle_phase()` 吸收 `step_phase_log` + `step_initialize_skipped_phase_log` | 消 1 个臂 + 11 处声明；两处落账入口合一 |
| A3 | verdict 裁决并入 `step_evaluator`，`eval_outcome` + `verdict` 合为 `EvaluatorTurn` | 消 1 个臂 + 8 处声明 + **消 1 个非法可达态** |
| A4 | `run_step` 的分派面收窄：`StepNode` 冻结为 22 变体（`match` 必须穷尽）⇒ **删 `PhaseStart` / `VerdictGate` / `PhaseLog` 三臂**（三变体并入「不可达」收口臂），并把原 `Decision \| RetryGate \| WhitelistGate => Proceed` 空臂改为同一「不可达」收口臂 | 分派臂 20 → 17；不可达臂对**六变体**（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision`）统一返回 `Terminal::failed`（计划图漂移：这六词不出现在任何 `steps` 内），把原来的「静默 Proceed」升级为显式失败——防计划图漂移被无声吞掉 |
| A5 | 四处 `*_phase_log` 收敛（`gate_fail` 保留为载荷构造辅助，`test_exec_upgraded` / `mutation_upgraded` / `initialize_skipped` / `phase_log` 合入 `settle_phase` 的 `Fail` / `Skipped` / `Pass` 形态） | 四处落账面收成一处 |
| A6 | `run_worker` 增 `SettleMode { Immediate, Deferred }`：`Deferred` 只发 running 拍、终态由调用方决定（evaluator 与 decision 用 `Deferred`——终态 emit 须在 `finalized` 判定之后） | 消「先 emit passed 再发现无收口载荷」的自相矛盾拍；其余调用点零改动 |

`enter_phase` / `settle_phase` 形态：

```rust
/// 相位入口：写 active_phase + start_at + attempt（单事务，写面既有能力）。
/// 零 emit、零审计、零 StepRecord。失败 = store 坏 / 编程错误 → Terminal::failed。
async fn enter_phase(
    tools: &Arc<dyn ToolStepPort>,
    request: &RunRequest,
    phase: &str,
) -> Result<u32, Terminal>

/// 相位收口：单入口三形态（Pass 三分流 / Fail 门禁升格与归档步失败 / Skipped 短路）
/// → 落账 → active_phase 清位。零 emit、零审计。
/// 落账失败 → Terminal，reason 携「相位 X 全部步通过但落账失败」。
enum PhaseSettle<'a> {
    Pass,                   // 三分流取自 PhaseCursor（机械双槽 / evaluator checklist / 空 checklist）
    Fail(PhaseFail<'a>),    // item / report / evidence / executor_session
    Skipped(&'a str),       // legacy initialize 整相位短路
}

async fn settle_phase(
    tools: &Arc<dyn ToolStepPort>,
    request: &RunRequest,
    cursor: &PhaseCursor,
    phase: &str,
    settle: PhaseSettle<'_>,
) -> Result<u32, Terminal>

/// evaluator 步收口产物：会话结果与收口载荷同源。
struct EvaluatorTurn {
    outcome: WorkerTurnOutcome,
    checklist: EvaluatorChecklist,
}
```

`PhaseSettle::Pass` 的三分流原样搬入（语义不变）：`cursor.test_outcome` 在场 → 机械 checklist（含 mutation 双槽）；`cursor.evaluator_turn` 在场 → 该 turn 的 checklist；两者皆无（archive）→ 空 checklist（`derive_verdict([]) = Pass`）。门禁升格与归档步失败经 `PhaseSettle::Fail(PhaseFail { item, report, evidence, executor_session })`，legacy initialize 短路经 `PhaseSettle::Skipped(跳过因)`——三形态共用一个落账单点（`run_phase_log` + `active_phase` 清位，单事务）。

**B 档（改动面小、与本次修改高度契合）**

| # | 内容 |
|---|---|
| B1 | `steps.rs` 的四个门禁臂各写一遍 `store.get_change(&change_id)?.map(|r| r.name)` → 合流为 `resolve_change_name(store, change_id, what)` |
| B2 | `begin_archive_node` / `settle_archive_node` 合为 `open_step` / `close_step`（只差「是否先发 running」） |
| B3 | 决策会话的 `WhitelistGate` / `RetryGate` 节点退场（约 100 行），动作裁决写 decision 节点 detail；`ensure_backtrack_allowed` 纯函数保留（越权一次即 `Terminal`）；`ToolCommand::DecisionLog` 保留（业务数据，非节点） |

### 兼容面：三张词汇表全部冻结

```
RunNodeRecord.step 落库 = bincode 枚举下标（infra/store/src/model.rs 明确钉死）
  → 变体 MUST NOT 删除或重排

RunStepKind（22 值，workflow/state.rs）        ← 落库编码，保持
ChangeStepKind（22 值，orchestration/state.rs） ← 出线 + 节点身份键，保持
StepNode（22 值，workflow/state.rs）            ← 计划图词汇，辅助步成 decode-only

StepGroup::Gate（workflow/state.rs）
  · 非出线（bindings.ts 无 StepGroup）
  · 辅助步退场后无 StepNode 归属 → 保留枚举值 + group() 的 Gate 分支
前端 run-state.ts 的 gate 分组与 STEP_LABEL 保留三词（存量解码必需）
```

结论：**三张表一律「冻结 + 新写面零产出 + 前端保留解码」，零 db 迁移**。

代价分两类，均须注释标注 `decode-only`：

- `PhaseStart` / `PhaseLog` / `VerdictGate` 三词：变体保留（旧库行解码必需），计划图与执行图的**新写面零产出**；
- `RetryGate` / `WhitelistGate` 两词：变体早已冻结（沿 `RetryGate` 已确立的先例），本次把它们的**节点写面也归零**（决策动作裁决改由 decision 节点 detail 承载）。

### attempt 推导口径（必须遵守）

```
attempt = store 内该 (change_id, phase) 的 PhaseRecord 全量条数（含 stale）+ 1
```

**不能**取 `detail.pipeline[phase].attempts.len() + 1`：`queries/detail.rs` 装配前会过滤 stale 条目，而 backtrack 把下游 pass 条目标 stale 但**保留行**——用 pipeline 条数推导会让回溯导致 attempt 回退，节点身份键 `(phase, attempt, step, instance_no)` 与历史行撞号。该式子即 `store.rs` 的 `start_change_phase` 与 `log_change_phase` 现在用的同一式（两处同源、零新语义）。`active_phase.attempt` 字段**保留写入**（落账即清位，事后无法推导；`queries/detail.rs` / `list.rs` 直接读该字段）——`start_phase` 写面保持原样，`infra/store` 零改动。

同时：`phase_next` 的 `round`（窗口口径，`entries.len() - anchor + 1`）与 attempt 是**两个不同口径**，MUST NOT 合并。

### 明确不做（知情边界，非遗漏）

- `StepSpec` 静态表（exec / check / skip / fail 四项）、14 处「passed + 跳过文案」数据化为 `SkipRule`、`PhaseCursor` 拆分（14 字段 → 窄 struct）、`run_step` 表驱动 / trait 化——属 `step-abstraction-model` 课题。
- `StepGroup::Gate` 与三张词汇表的死变体清理：需要 db 迁移或一次性数据重写，另立变更。
- `feedback_limit(step, label)` 的 label 分支改造（基线已收缩为 static-check / archi-check 两个调用点，收益进一步变小）。
- 相位表 `VerdictGate` 步的 `on_fail` 声明清理——辅助步声明整体退场后该字段随之消失，无需单独清理（与上一变更「留给独立变更」的表述在此一次性了结）。
- 前端除 `run-state.ts` 解码保留外的任何渲染改动（辅助步退场后新 run 不再产出这些节点，既有渲染零改动即可）。
- 架构模型改动：零新 crate、零新元素、零新依赖边。

---

## 能力

### 新增的能力

**无。** 本变更全部落在既有能力面内——相位机形态（desktop-change-orchestration）、计划图静态表（desktop-workflow-write-face）、归档链（desktop-change-archive）、流程视图（desktop-change-flow-view）、状态库（desktop-change-state-store）、检查域两步（desktop-test-execution / desktop-mutation-testing）、db 种子语料（desktop-corpus-regression）。不引入新能力边界：辅助步退场是既有能力内部的步序列与节点面收窄。

### 修改的能力

- **desktop-change-orchestration** —
  - **RENAMED**「verdict 解析与 phase-log 代写」→「evaluator 收口裁决内化与相位收口落账」（`parse_verdict` 早已退役，标题里的「解析」与「phase-log 代写」两头都不再成立）。
  - **MODIFIED**（新标题）「evaluator 收口裁决内化与相位收口落账」：裁决并入 `step_evaluator`（读 `finalized`，缺席即 `Terminal::failed`，截断 / 未收口两文案），`VerdictGate` 节点零产出；落账改称 `settle_phase` 相位收口动作（三分流语义逐字保留，archive 空 checklist 分支不变）；`step_phase_log` / `step_initialize_skipped_phase_log` 退场。
  - **MODIFIED**「薄图 walker 相位循环与路由红线」：archive 11 步 → 9 步；`phase step 序列` 只含真实步；**verdict 解析失败** bullet 整体退役（承接 `finish_step` 落地后的事实）；计划图漂移判据保留。
  - **MODIFIED**「四类节点统一 walker 与可观测均一」：ToolStep 节点词表去 `phase-start` / `phase-log`；Gate 节点三条（verdict / whitelist / retry）转为**存量解码词汇**（新写面零产出）；initialize 5 步 → 3 步、archive 11 步 → 9 步的示例随动。
  - **MODIFIED**「决策协议」：动作裁决与驳回写 decision 节点 detail（`WhitelistGate` / `RetryGate` 节点零产出）；白名单越权一次即 `Terminal::failed`；`ask` 停等在会话之外闭合（每轮实例递增于 decision 节点）。
  - **MODIFIED**「相位 prompt 组装面静态化」：动态面去掉「verdict 解析失败修复注入」；段序列表随动（协议附录仍最后）。
  - **MODIFIED**「evaluator 输出协议瘦身」：改述为「调 `finish_step` 收口」形态（不再要求最终消息输出 JSON 形状），保留禁调写通道红线与 `report` 上限。
  - **MODIFIED**「确定性相位空 checklist 落账」：`step_phase_log` 三分流改称 `settle_phase` 三分流，语义逐字保留。
  - **MODIFIED**「MutationTest 步接线与失败升格」：步序列 `PhaseStart → TestExecution → MutationTest → PhaseLog` → `TestExecution → MutationTest`；反馈边注入对象句收为「仅 static-check 失败注入 executor」。
  - **MODIFIED**「initialize 基线检查相位执行体」：步序 `PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog` → `Clean → StaticCheck → TestExecution` + 相位收口；legacy 整相位短路改述为「enter → `settle_phase(Skipped)`」。
  - **MODIFIED**「步级时间戳与 token 消耗随行」：单 emit 瞬步例随辅助步退场而无生产例，合并语义（`started_at` 保首见）保留并由直构断言承载。
  - **MODIFIED**「计划图与执行图对齐与步粒度回退」：归档步数口径校正为 9 步。
  - **MODIFIED**「ArchiCheck 步接线与反馈边」：**`ArchiCheck` 步 SHALL NOT 在退场范围内**——implement 相位仍为 `Executor → StaticCheck → ArchiCheck → Evaluator` 四步，本步座位（static-check 之后、evaluator 之前）、runner / 报告槽位 / 反馈上限（`FeedbackLoop{3}`，与 static-check 的 5 次独立计数）、「违例注入同一 executor 会话」语义逐字不变；唯一改动是「超限升格」的落账入口改由 `settle_phase(PhaseSettle::Fail(..))` 承载（item「架构检查」，`report` / `evidence` / `executor_session` 装配逐字不变）。
  - **REMOVED**「决策会话软拒环与漂移上限」（Reason：漂移环已随 `finish_step` 工具化退役，本变更进一步删除该 requirement 描述的门节点；Migration：越权一次即 `Terminal::failed` + decision 节点 detail 留痕）。
  - **ADDED**「相位入口与收口内化（计划图仅含真实步）」：`enter_phase` 零 emit / 零审计、`settle_phase` 三分流与落账失败终态，`steps` 空判据保留。
  - **ADDED**「相位收口失败可辨与相位内系统异常仍标 failed」（单条 requirement 承载 D3 + D5 第二半的面）：全部步 passed 但落账失败 → 节点全绿 + run failed + reason 前缀可辨；`run_worker` `Err` / 会话 `Failed` → 该步节点 failed + `Terminal`——两型 reason 文案 MUST 可区分。
  - **ADDED**「驳回与动作裁决经节点 detail 可见」：decision 节点实例递增承载动作与越权文案（D4）。
  - **ADDED**「attempt 推导口径与 round 口径分离」：全量含 stale + 1；与 `round` 属两个口径，MUST NOT 合并。
- **desktop-workflow-write-face** —
  - **MODIFIED**「相位定义有序 step 序列与 artifact 清单」：`INITIALIZE_STEPS` 3 步 / `TEST_EXECUTION_STEPS` 2 步 / `IMPLEMENT_STEPS` 4 步 / P→E 相位 2 步 / `CODE_ANALYZE_STEPS` 2 步 / `ARCHIVE_STEPS` 9 步；`StepNode` 二十二值**冻结**（`phase_start` / `phase_log` / `verdict_gate` 转 decode-only，新表零引用）；`StepGroup::Gate` 分支保留（无 StepNode 归属亦不删）。
  - **MODIFIED**「test-only 相位表与前置依赖」：test-only 的 `steps` 复用表随动（test-execution 两步、code-analyze 两步）。
- **desktop-change-archive** —
  - **MODIFIED**「归档编排链与阶段幂等」：11 步 → 9 步（开相归 `enter_phase`、落账归 `settle_phase`），scenario 与 Module Contract 步数口径同步。
  - **MODIFIED**「归档作为 run 相位与手动门」：**stop-before 停等面语义 SHALL 逐字不变**——停等触发时机、确认载荷（完成度警告 / delta specs 清单 / merge 目标）、「未确认不入相位」「确认后才 `enter_phase` + 9 步计划步」「停止 run 自然停在 archive 前」五条均不改；改动点仅为「开相与落账移出 `steps`、零节点零审计」（该 requirement 内的归档 10 步节点口径随动为 9 步）。
- **desktop-change-flow-view** —
  - **MODIFIED**「运行态节点与实时执行视图」：archive 列 11 步 → 9 步；二十二值词汇**冻结**（辅助步与三门 gate 为存量解码词，新 run 零产出，解码映射保留）。
  - **MODIFIED**「节点点击判据与 stdout 输出面板」：判据本身零改动；双 null 节点例改述为「**存量** run 的 `phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate`，或任一新 run 的 git / exec 双 null 步」（新 run 不再产出辅助步节点，但仍须可解码、仍落「不可点击」臂）。
  - **MODIFIED**「步级耗时与 token 节点呈现」：瞬步例换为「无生产瞬步」的表述（合并语义仍由直构断言承载）。
  - **MODIFIED**「initialize 相位列与 clean 步词呈现」：四节点 → 三节点。
- **desktop-change-state-store** —
  - **MODIFIED**「run 执行图落库」：二十二值词汇**冻结**；`phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` **五词**新写面零产出（`retry_gate` 早已是存量解码词，本次把 `whitelist_gate` 的节点写面一并归零；存量行照常解码）；archive 相位 9 步节点（无 `PhaseStart` / `PhaseLog`）。
  - **MODIFIED**「步骤审计落库可查」：`StepRecord` 的 `phase_start` / `phase_log` 审计行新写面零产出（词面保留解码），审计查询面形状不变。
- **desktop-test-execution** —
  - **MODIFIED**「MutationTest 确定性门禁步与时序」：步序列改 `TestExecution → MutationTest`（两端辅助步退场）。
  - **MODIFIED**「失败即升格交决策（零 agent 修复）」：「余下步（MutationTest 与主分支 PhaseLog）不执行」→「余下步（MutationTest）不执行」。
  - **MODIFIED**「机械 checklist 双槽与 evidence 三态」：落账点表述由 `PhaseLog` 步改为相位收口动作。
- **desktop-mutation-testing** —
  - **MODIFIED**「失败升格交决策（报告指引式信息面）」：「机械 checklist 落账进入 PhaseLog」→「机械 checklist 由相位收口落账」。
- **desktop-corpus-regression** —
  - **MODIFIED**「run 运行史语料维度」：initialize 相位样本 5 步 → 3 步、archive 相位样本 11 步 → 9 步（真实相位形态随辅助步退场而变）；**新增双口径**——`phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` 五词转存量解码专用但仍 MUST 各有语料样本（旧库行解码回归面），样本集内与「真实相位形态样本」并列标注、互不替换；golden 随显式重写流程更新。

### 其余能力零改动

- **desktop-change-queries / desktop-workspace-store**：`ChangeDetail` 不含 `steps`、`PhaseRecord` / `ChecklistItemRecord` 形状与落账形态零变更。
- **desktop-checks-domain / desktop-archi-gate**：四个 runner port 与报告槽位零改动（`ArchiCheck` 与 static-check 的反馈边语义逐字保留）。
- **desktop-phase-prompts**：14 份 md 零改动（辅助步从不出现在角色知识里）。
- **desktop-ipc-type-bindings**：`ChangeStepKind` 值集不变 ⇒ `bindings:export` 再生成后零 diff。
- **desktop-crate-layout / repo-archi-model**：零新 crate、零新元素、零新依赖边。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/orchestration/src/walker.rs` | 新 `enter_phase`（吸收 `step_phase_start`，零 emit / 零审计）与 `settle_phase` / `PhaseSettle` / `PhaseFail`（吸收 `step_phase_log` + `step_initialize_skipped_phase_log`，单入口三形态 `Pass` / `Fail` / `Skipped`，三分流原样搬入，落账失败 → `Terminal::failed`「相位 X 全部步通过但落账失败」）；`step_evaluator` 内化 verdict 裁决（读 `finalized`，缺席按截断 / 未收口两文案 `Terminal::failed`，写入 `cursor.evaluator_turn`）；删 `step_phase_start` / `step_phase_log` / `step_initialize_skipped_phase_log` / `step_verdict_gate`；`run_step` 把 `PhaseStart` / `VerdictGate` / `PhaseLog` 三臂与原 `Decision \| RetryGate \| WhitelistGate => Proceed` 空臂合并为单一不可达臂（`StepNode` 冻结 22 变体，`match` 须穷尽；六变体返回 `Terminal::failed` 而非静默 `Proceed`）；`drive` 改为 `enter_phase` → 迭代 `next.steps` → `settle_phase`（legacy initialize 短路改走同一对动作）；`PhaseCursor` 的 `eval_outcome` + `verdict` 合为 `evaluator_turn: Option<EvaluatorTurn>`；`step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` 合入 `settle_phase(PhaseSettle::Fail(..))`；`run_worker` 增 `SettleMode { Immediate, Deferred }` 形参（evaluator / decision 传 `Deferred`，终态由调用方在 `finalized` 判定后 emit）；`begin_archive_node` / `settle_archive_node` 合为 `open_step` / `close_step`；`decision_session` 删 `WhitelistGate` 节点（动作裁决写 decision 节点 detail），`resolve_deadlock` 删 `RetryGate` 三次 emit；注释同步（相位机形态 / 瞬步口径 / `attempt` 推导） |
| `crates/core/orchestration/src/steps.rs` | 四个门禁臂的 `store.get_change(..).map(\|r\| r.name)` 合流为 `resolve_change_name(store, change_id, what)`；`ToolCommand::PhaseStart` / `PhaseLog` 保留（写面能力未变），审计行产出随相位机步退场而移除（实现面：`step_phase_start` / `step_phase_log` 调用点消失即不再经 `audit_outcome`） |
| `crates/core/workflow/src/write/phase_table.rs` | 11 张 `*_STEPS` 删全部 `PhaseStart` / `VerdictGate` / `PhaseLog` 项（共 30 处声明）；`INITIALIZE_STEPS` 3 步、`TEST_EXECUTION_STEPS` 2 步、`IMPLEMENT_STEPS` 4 步、`ARCHIVE_STEPS` 9 步、P→E 相位 2 步、`CODE_ANALYZE_STEPS` 2 步；各相位 doc 注释的步序串同步 |
| `crates/core/workflow/src/state.rs` | **仅加注释、零语义改动**：`StepNode` 二十二值**保持**（`PhaseStart` / `PhaseLog` / `VerdictGate` 标 `decode-only`：计划图新表零引用、存量节点解码必需），`RunStepKind` 同款 `decode-only` 注释（bincode 下标钉死，MUST NOT 删 / 重排）；`StepGroup::Gate` 分支保留 + 注释说明「辅助步退场后无 StepNode 归属，词汇面仍冻结」 |
| `crates/core/orchestration/src/state.rs` | **仅加注释、零语义改动**：`ChangeStepKind` 二十二值保持 + 三个辅助步变体的 `decode-only` 注释（不删不重排：出线词与节点身份键的存量行依赖） |
| `packages/desktop/package.json` | `version` 相对**落地时 `HEAD` 现值 +1 patch**（当前盘面 `0.6.35` → 落地时以 `git diff HEAD -- packages/desktop/package.json` 的版本行判「恰好 +1」为准；MUST NOT 写死字面号）；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `enter_phase`：写 active_phase + attempt（全量含 stale + 1）、零 emit、零 StepRecord；写失败 → `Terminal::failed` | `walker_test.rs` / `steps_test.rs` | 新增（原 `phase_start` 节点断言改为「无节点 + active_phase 在案」） |
| `settle_phase` 三分流：test_outcome（含 mutation 双槽）/ evaluator_turn checklist / 空 checklist（archive）；落账失败 → 节点全绿 + reason 前缀 | `walker_test.rs` | 新增（原 `node.step == PhaseLog` 的「已落账」断言改读 `phase_log_input` 载荷与 pipeline attempts） |
| evaluator 步内化裁决：`finalized = Some(Evaluator)` → 同实例 passed + detail `verdict=…`；缺席（Completed 未收口 / 截断）→ 同实例 failed + `Terminal`，两文案可辨；零 `feedback` 边 | `walker_test.rs` | 改写（锚点由 `node_of(VerdictGate)` 改为「该 attempt 的 evaluator 节点末实例」） |
| 决策会话：动作裁决写 decision 节点 detail（通过 / 越权一次即停）；`WhitelistGate` / `RetryGate` 节点零产出；ask 应答后新开一轮为 `instance_no` 递增的 decision 节点 | `walker_test.rs` | 改写（`step_instances(RunStepKind::WhitelistGate)` → `step_instances(RunStepKind::Decision)`） |
| 相位表新序：`INITIALIZE_STEP_ORDER` 3 / `TEST_EXECUTION_STEP_ORDER` 2 / `IMPLEMENT_STEP_ORDER` 4 / `ARCHIVE_STEP_ORDER` 9 / P→E 2 / `CODE_ANALYZE_STEP_ORDER` 2；辅助步在全部 `steps` 上零引用 | `crates/core/workflow/src/write/phase_table_test.rs` | 改写（`ARCHIVE_STEP_ORDER` 11 → 9 等六处序表 + 步数断言） |
| `run_worker` 的 `SettleMode`：`Deferred` 只发 running 拍（返回后同实例无终态）、`Immediate` 语义与既有逐字一致；evaluator / decision 收口判定后终态恰一拍 | `walker_test.rs` | 新增 |
| 词汇冻结守卫：`StepNode` / `RunStepKind` / `ChangeStepKind` 仍二十二值且序不变；`StepGroup::Gate` 分支在案（`VerdictGate` / `RetryGate` / `WhitelistGate` 仍归 Gate） | `crates/core/workflow/src/state_test.rs` / `crates/core/orchestration/src/state_test.rs` | 新增（防回退：删变体或重排即红） |
| 6 处 `walk_run(...)` 整链测试（节点序列与总数断言） | `walker_test.rs` | 逐条重定期望值（辅助步退场后节点数减少，边数相应减少）——**在代码段内完成，不留到测试段** |
| golden 快照（含 `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` 节点与串行边） | `crates/core/workflow/tests/golden/*.json` | `DESKTOP_GOLDEN_REWRITE=1` 重写 → 不带环境变量复跑确认零 diff（语料种子仍构造全词汇样本，故词汇不减少；步数与边序按重写结果核对） |

> **本表无前端行**：`packages/desktop/src/**`（含 `src/views/changes/flow/*.test.tsx`）在本变更内零改动——辅助步退场只改后端写面，前端既有渲染与既有用例照常通过（见 AC-10 与「不要修改」）。

### 不要修改

- **`crates/infra/store/**`**：`PhaseRecord` / `StepRecord` / `RunNodeRecord` 形状与 `StepKind` 词面零改动（词汇冻结即零迁移；`start_change_phase` / `log_change_phase` 的 attempt 推导式子逐字不动）。
- **`crates/core/orchestration/src/{port.rs,control.rs,snapshot.rs,run_history.rs}`**：四个 runner port、`ToolCommand` / `ToolStepOutput` 封闭集、`accumulate_node` 合并语义、快照装配与 `ChangeStepKind → RunStepKind` 映射零改动。
- **`crates/core/orchestration/src/{finish,verdict,decision,prompt,transcript}.rs`**：`finish_step` 协议面（校验器 / 动态 schema / 载荷提取）零改动；`ensure_backtrack_allowed` 逐字不动。
- **`crates/core/workflow/src/prompts/*.md`（14 份）**：零改动（辅助步从不出现在角色知识里）。
- **`crates/infra/agent/**`**：工具挂载面 / 守卫链 / 会话泵零改动（本变更不触碰协议）。
- **`packages/desktop/src/**`**：零改动——`run-state.ts` 的 `STEP_LABEL` / `normalizeKind` / gate 分组是**存量解码必需**，删键即旧库行渲染崩；`change-flow-graph.tsx` 的静默节点判据对「不存在的节点」天然无害。
- **`crates/core/workflow/tests/corpus_golden_test.rs` 的种子构造词汇面**：全词汇样本 MUST 保留（含三个辅助步与三门 gate），只随序号 / 步数断言校正——删样本词汇会与 `desktop-corpus-regression`「二十二类词汇在场」冲突。
- **`openspec/changes/slim-flow-aux-steps/explore.md`**：只读。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | **辅助步全退场**：相位表 11 张 `*_STEPS` 内 `PhaseStart` / `VerdictGate` / `PhaseLog` 出现次数为 **0**（现状 30 处）；requirement 表 10 相位步数 = 3/2/2/2/4/3/2/1/1/9（合计 29）；test-only 表 7 相位步数 = 3/2/2/2/3/2/9（合计 23） | `phase_table_test` 序表断言 + 源码 `grep -c 'StepNode::PhaseStart\|StepNode::VerdictGate\|StepNode::PhaseLog' phase_table.rs` = 0 |
| AC-2 | **相位入口内化**：`enter_phase` 为唯一开相路径，写 active_phase（含 attempt = PhaseRecord 全量条数 + 1，含 stale）且**零 `RunUpdate::Step` / 零 `StepRecord`**；legacy initialize 短路亦经 `enter_phase` + `settle_phase(Skipped)` | `walker_test`（节点数与审计行数断言）+ `steps_test`（`PhaseStart` 命令不再产审计行） |
| AC-3 | **相位收口内化**：`settle_phase` 三分流逐分支可达（test_outcome / evaluator checklist / 空 checklist）；archive 相位空 checklist 落 `verdict=pass` 且 `active_phase` 清位；落账失败时**节点全绿**、run `failed`、reason 前缀「相位 X 全部步通过但落账失败」 | `walker_test`（三分流 + 落账失败注入）+ 与「步骤系统异常 failed」用例的 reason 文案对拍 |
| AC-4 | **verdict 裁决内化**：`step_evaluator` 读 `outcome.finalized`；`Some(Evaluator(..))` → 同实例 passed + detail `verdict=…` + 交 `settle_phase`；缺席 → 同实例 failed + `Terminal::failed`，「输出被截断」与「未调用 finish_step」两文案可辨；零 `feedback` 边、`VerdictGate` 节点零产出 | `walker_test`（三态：合规 / 未收口 / 截断） |
| AC-5 | **决策节点承载裁决**：一次决策分叉只产 decision 节点（`instance_no` 按轮递增），detail 携动作（`backtrack → <phase>` / `stop`）或越权文案（含 `allowed`）；`WhitelistGate` / `RetryGate` 节点零产出；`ensure_backtrack_allowed` 仍为纯函数、越权一次即 `Terminal::failed` | `walker_test`（通过 / 越权 / ask 应答三形态）+ 源码复核两 gate 的 `begin_node` 调用点为零 |
| AC-6 | **词汇冻结**：`StepNode` / `RunStepKind` / `ChangeStepKind` 三枚举仍各 22 值且变体序不变（bincode 下标稳定）；`StepGroup::Gate` 分支在案；三处辅助步 / 三门 gate 变体带 `decode-only` 注释 | `state_test`（两 crate）逐值 + 序断言；`bindings:export` 后 `src/types/generated/**` 零 diff |
| AC-7 | **归档链 9 步**：`ARCHIVE_STEPS` 序 = Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize；开相与落账不在 `steps` 内；`ARCHIVE_STEP_ORDER` 断言 9 | `phase_table_test` + `walker_test`（archive 相位节点数 = 9，无 `phase_start` / `phase_log` 节点） |
| AC-8 | **计划图漂移升级为显式失败**：`run_step` 的不可达臂对六词（`PhaseStart` / `PhaseLog` / `VerdictGate` / `RetryGate` / `WhitelistGate` / `Decision`）返回 `Terminal::failed`（原 `Proceed` 静默吞掉）；`match` 仍穷尽（`StepNode` 22 变体） | `walker_test` 直构含 `PhaseStart` 步的 `steps` 序列断言 `Terminal`；`cargo check` 保证穷尽性 |
| AC-9 | **行为面零回归**：`ToolCommand` 载荷断言（audit / 落账载荷）逐条不变；四处机械 checklist 条目文案（静态检查 / 架构检查 / 测试执行 / 突变测试 / suite 结论一致 / 聚合计数一致 / mutation null 自动通过）全部在案且**不经**任何门步 | `cargo test -p orchestration` + `grep` 复核 7 个条目文案零删除 |
| AC-10 | **前端零改动可解存量**：`run-state.ts` 的 `STEP_LABEL` 穷尽 Record 仍含 `phaseStart` / `phaseLog` / `verdictGate` / `retryGate` / `whitelistGate` 五键，gate 分组仍归 `gate`；`vp test` 既有渲染用例**零改写、零新增**（「新 run initialize 列恰三节点」由既有渲染路径天然承载，不需新用例） | `pnpm -C packages/desktop exec vp test run src/views/changes/flow` + `git diff HEAD -- packages/desktop/src` 为空 |
| AC-11 | **golden 零 diff**：`DESKTOP_GOLDEN_REWRITE=1` 重写后不带环境变量复跑为绿（`cargo test -p workflow --test corpus_golden_test`）；真实相位形态样本 initialize 3 节点 / archive 9 节点按序 forward 串行 | 段 3 的两条命令 |
| AC-12 | **语料双口径**：`phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` 五词各至少一个节点样本仍在校（标注存量解码形态），与退场后的真实相位形态样本并列存在、互不替换；二十二值全词汇断言不变 | `corpus_golden_test` 全词汇断言 + `tests/fixtures/README.md` 两条目复核 |

> **验收表口径**：AC-1…AC-12 是本变更的**能力验收面**。静态检查链、全量测试与版本递增是全仓既有交付门（主基线承载），不进验收表。

### 验证命令

**段 1（代码）**

```powershell
cargo check --manifest-path packages/desktop/src-tauri/Cargo.toml
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p orchestration
cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p workflow
```

**段 3（golden）**

```powershell
$env:DESKTOP_GOLDEN_REWRITE=1; cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p workflow --test corpus_golden_test
Remove-Item Env:DESKTOP_GOLDEN_REWRITE; cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p workflow --test corpus_golden_test
```

**交付门**

```powershell
cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml --all-targets
pnpm -C packages/desktop run bindings:export
pnpm -C packages/desktop run check      # server:check → bindings:export → api:check → client:check
pnpm -C packages/desktop test
git diff HEAD -- packages/desktop/package.json   # version 行恰 +1 patch
```

### 实施顺序（按段，不按模块）

```
段 1（代码）      A1–A6 + B1–B3；编译通过 + 行为面断言全绿
                  形状面断言此时必然红（预期），用 #[ignore] 或临时注释隔离
段 2（形状面测试） ~40 处断言改写 + 6 处 walk_run 整链期望值重定，恢复全绿
段 3（golden）    DESKTOP_GOLDEN_REWRITE=1 重写 → 不带环境变量复跑确认零 diff
```

**为什么不先改测试**：形状面断言的新形态依赖 A3 内化后「同一实例 passed、缺席即 failed」的最终语义；先写测试会写出中间态，代码落定后还得再改一次。

**改写原则：改断言对象，不改断言意图。**

```
node_of(PhaseStart, 0) 找相位入口锚点          → 该相位首个真实步节点
step_instances(RunStepKind::WhitelistGate)    → step_instances(RunStepKind::Decision)
node.step == PhaseLog 的「已落账」断言         → pipeline attempts / phase_log_input 载荷
```

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 测试改写才是工作量重心**（`walker_test.rs` 16816 行 / 186 个用例；`PhaseLog` 66 行、`PhaseStart` 25 行、`VerdictGate` 16 行、`WhitelistGate` 12 行命中） | 形状面断言大面积变红，误改断言意图会掩盖真回归 | 高 | 三段切分（代码 → 形状面 → golden），每段有独立绿线；改写原则「改断言对象不改意图」逐条落到测试文件表；段 1 内先逐条确认 6 处 `walk_run` 整链期望值 |
| **R2 双失败面文案混淆**（全绿 + failed vs 红节点 + failed） | 用户把落账失败读成缺陷 | 中 | AC-3 / AC-4 两条文案对拍；reason 前缀写死「相位 X 全部步通过但落账失败」，系统异常路径沿既有「会话执行失败 / 会话失败收敛」文案 |
| **R3 attempt 推导口径漂移**（改用 `detail.pipeline` 条数） | 回溯导致 attempt 回退 → 节点身份键撞号、图面节点覆盖 | 中 | 口径写进 spec 的 ADDED requirement（全量含 stale + 1，与 `round` 分离）；`enter_phase` 复用写面既有式子，`infra/store` 零改动即不可能引入第二份推导 |
| **R4 词汇删变体或重排**（把三个辅助步从枚举里「顺手清掉」） | 存量 `RunNodeRecord.step` 按 bincode 下标解码 → 全库步词错位 | 低但致命 | 三张表**冻结**写进 spec 与 proposal 范围；AC-6 逐值与序断言；变体注释标 `decode-only`；「不要修改」明确列 `infra/store` |
| **R5 golden 重写夹带非预期变化** | golden 变成「实现写成什么样就长什么样」 | 中 | 段 3 必须「重写 → 不带环境变量复跑零 diff」两步；语料种子词汇面不改（仅步数 / 序断言随动），重写差异可按文件逐一审阅 |
| **R6 基线 spec 滞后于代码**（`agent-finalize-tool-for-flow-steps` 的 delta 未合并进 `openspec/specs/`：主基线仍写「最终消息输出 checklist JSON」「决策会话软拒环与漂移上限」） | MODIFIED 段落以旧文为拷贝底稿，合入后可能留下与代码不符的残句 | 高 | 本变更 README 式地在 `desktop-change-orchestration` delta 内单列「基线漂移与取代关系」表，逐条指明被取代的主基线片段；MODIFIED 正文直接写**目标态**（`finish_step` + 无 gate + 无软拒环），即对该滞后一并收口；`grep -F` 旧片段（`parse_verdict` / `DECISION_DRIFT_LIMIT` / `WhitelistGate` 每轮一节点）零命中为合入判据 |
| **R7 计划图漂移判据软化**（`steps.is_empty()` 与 evaluator 缺位检查） | 相位定义写错时会静默空转而非失败 | 中 | 保留 `next.steps.is_empty()` → `Terminal::failed`（相位定义恒含至少一个真实步）；`step_evaluator` 的 `None` 检查保留；新增守卫用例：任意相位 `steps` 含辅助步即红（AC-1） |
| **R8 前端存量解码被误删**（认为「不再产出」就把 `STEP_LABEL` / gate 分组摘掉） | 旧库行渲染出未知步词 / 分类缺失 | 中 | AC-10 明确五键在场 + `git diff HEAD -- packages/desktop/src` 为空；「不要修改」列出 `run-state.ts` 理由 |
| **R9 `StepGroup::Gate` 死分支引发「清理冲动」** | 越界改动把 `StepNode` 的 Gate 分支删掉 → 存量行 `group()` 落空 | 低 | spec（workflow-write-face）明写「分支保留、无 StepNode 归属亦不删」；explore §11 的「死变体清理」列为明确不做 |
| **R10 版本与绑定纪律** | 交付门红 | 低 | `package.json` `version` 按「落地时 HEAD 现值 +1 patch」相对递增（MUST NOT 写死字面号）；`ChangeStepKind` 值集不变 ⇒ `bindings:export` 零 diff；零新 crate / 元素 ⇒ 架构模型零改动。**该条是交付门而非能力验收**，故不进 AC 表 |
| **R11 `SettleMode` 引入的终态拍序风险**（`Deferred` 下若调用方漏发终态，节点永停 running） | 图面残留 running 节点、前端走秒不止 | 中 | `Deferred` 仅两个调用点（evaluator / decision），两处都以「判定 → 单次终态 emit」成对书写；AC-4 / AC-5 断言「同实例恰一拍终态」；`accumulate_node` 合并语义零改动（不新增「终态后仍可覆写」的宽容分支） |
