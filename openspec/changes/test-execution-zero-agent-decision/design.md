# 设计: test-execution-zero-agent-decision

> **变更**: test-execution-zero-agent-decision
> **日期**: 2026-10-18
> **基于**: proposal.md、`explore.md`（探索定论 12 条）与六份能力 delta（desktop-test-execution / desktop-change-orchestration / desktop-mutation-testing / desktop-workflow-write-face / desktop-change-queries / desktop-change-flow-view）

---

## 提案与规格同步状态

`proposal.md`「变更范围 - 实现文件」八文件与六份 capability delta 已定稿，本设计不重复其论证，只做三件事：逐文件落定实现形态、补 explore 未定稿的设计面口径（report 截断余量 / mutation 报告面取值 / 软拒环代码形状 / 拒绝文案单点）、按验收标准逐条对齐。

起点核实（工作树实况，锚点按当前文件核对）：

- **两个反馈循环确在相位内**：`step_test_execution`（`walker.rs:1029-1121`）与 `step_mutation_test`（`walker.rs:1271-1382`）各为 `loop { run_tool; run_worker(Executor, Continue, Feedback) }`，`fix_session` 槽位随循环累积；升格落账分居 `step_test_execution_upgraded_phase_log`（:1875）与 `step_mutation_test_upgraded_phase_log`（:1938），两处 `executor_session_id: fix_session...`。
- **`OnFail` 两变体**（`workflow/src/state.rs:390-404`）：`FeedbackLoop { limit }` / `Terminal`；`TEST_EXECUTION_STEPS` 两步为 `FeedbackLoop { FEEDBACK_LOOP_LIMIT }`（`phase_table.rs:326-339`）。
- **`allowed_backtrack_phases` 无自排除**（`phase_table.rs:808-820`）：`table[..=idx]` 过滤 `backtrackable` 即返回——test-execution 自身在列。
- **白名单门在 `resolve_deadlock`**（`walker.rs:3063-3098`）：`ensure_backtrack_allowed` Err 即 `Terminal::failed("决策越权被拒…")`，一次打死；`DecisionAction::Retry` 分支（:3160-3162）兑现靠 `fail_decided` 放行同相位重跑。
- **决策会话无漂移计数**（`walker.rs:3182-3246`）：`parse_decision` Err 直接 `Terminal::failed("决策解析失败…")`；ask 续问文案硬编码「四选一封闭集」。
- **决策输入无失败报告**（`walker.rs:3249-3304`）：`DecisionInput` 只有 fail checklist / allowed / candidates / `eval_only`；`record.report` 未进 prompt；`fail_report` 缺席。
- **升格落账 report 现为叙述句**（`walker.rs:1906-1910` / `:1969-1971`），`findings_detail` 只喂反馈边 fix prompt（`walker.rs:1101`）——反馈边退役后该字段失去唯一消费者，可原样转喂 report。
- **`MAX_REPORT_CHARS = 2000` 双处单源**：`workflow/write/phase_table.rs:41`（写面校验 `phase_log.rs:56` 超长即 Err）与 `orchestration/verdict.rs:6`（prompt 协议口径）——截断必须在 walker 侧完成且**留余量**（见 D2）。
- **边缘事实**：`MutationTestOutcome` 无 `report_dir` 字段（只有 `report_paths: Vec<String>`，`port.rs:286-302`）——突变证据的「报告目录」由 `report_paths` 原文承载（D3）；`RunEdgeKind` 四值已在 `detail.rs` 导入（:10），屏蔽单点零新 import；`OnFail` 不上 IPC（`bindings.ts` 不含），`DecisionInput` / `DecisionAction` 亦不上 IPC。

**只读红线零触点**：`crates/infra/store`（库格式 v3）、`crates/infra/checks`（testexec / mutation runner 执行面与报告面）、`port.rs` 字段面、`bindings.ts`、`src/views/changes/flow/graph.ts` / `change-flow-graph.tsx`、`plugins/**` 及全部插件交付产物、`openspec/specs/**` 基线。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 步失败语义三值 | `OnFail` 增 `Escalate`（无载荷）：零会话、落账、截断余步、回决策；`Terminal` / `FeedbackLoop` 语义与消费点零改动 | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | crate 内 serde / specta | 变体面单点；线词 `escalate`（camelCase 派生） |
| 步声明与白名单推导 | `TEST_EXECUTION_STEPS` 两步 `on_fail → Escalate`；`allowed_backtrack_phases` 增 `redoable` 自排除（`steps` 含 `role().is_some()` 的 agent 步 ⇒ 可重做） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | `crate::state::{OnFail, StepDefinition}` | 表内自描述，零相位名判断；`PhaseDefinition` 零新字段 |
| 路由幂等闸 | `fail_decided` 保留、注释改写为「该 fail 条目已进入过决策，不再重复分叉」；`eval_only` 派生删除；`PhaseNextError` 两变体去字段；`MaxRetriesExceeded` 文案/Doc 改「本相位已连续失败 N 次」 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | `phase_table`（`MAX_RETRY_TIMES` / `MAX_ROUNDS`） | 语义改写，非行为改写（判据字段集收缩） |
| 读面边屏蔽 | edges 投影前 `.filter(\|record\| record.edge_kind != RunEdgeKind::Retry)`（唯一屏蔽点），其余边按 `edge_seq` 原序出线 | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | `crate::state::RunEdgeKind`（既有 import） | 查询层纯读，DTO / bindings 零演进 |
| 门禁步执行体（test） | 三分派：`Terminal` → 既有 `step_test_execution_terminal`（零改动）；`Escalate` → 新单发执行体（单节点 running → spawn → pass 收口 / fail 或 error 升格）；其余声明 → 计划图漂移 Err。删 `loop` / `fix_session` / `feedback_limit` 消费 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（`step_test_execution` / `step_test_execution_escalate`） | `ToolStepPort`（既有）、`diagnose_brief` | `run_tool` Err 恒走 `?` → `Terminal`（环境问题不进决策） |
| 门禁步执行体（mutation） | `Escalate` → 单发执行体（runner `Err → Terminal` 分支原样保留、fail / error 两时点同式升格）；其余声明 → 漂移 Err。删 `loop` / `fix_session` | `walker.rs`（`step_mutation_test` / `step_mutation_test_escalate`） | 同上 | 绿后才跑突变的时序由「升格截断 + 决策回溯真重跑」保证（步序零改） |
| 升格落账 | 两处 upgraded phase_log 去 `fix_session` 参数；report = 完整明细（截断 ≤2000）、checklist 恰一条 `pass=false`（evidence = brief + 报告面）、三会话槽位 `None`；文案改「首败升格」 | `walker.rs`（`step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`）、新增私有 `truncate_report` | `phase_log` 写面（2000 上限）、`MAX_REPORT_CHARS` | 单一三段动作；写面拒绝超长 ⇒ 截断留 1 字符余量（D2） |
| 决策软拒环 | 白名单门自 `resolve_deadlock` 迁入 `decision_session`：每轮 run_worker(Decision, High) → decision-log → parse → 门；parse Err / 门 Err 计漂移、同会话续注；`drift > DECISION_DRIFT_LIMIT (2)` 才 Terminal；ask 不计漂移；每轮 parsed 动作 emit 一个 `WhitelistGate` 实例 | `walker.rs`（`decision_session`）、新增 `DECISION_DRIFT_LIMIT` | `parse_decision` / `ensure_backtrack_allowed` / `decision_fix_prompt` | 循环内闭环，零新 port、零新节点词 |
| 决议兑现 | `resolve_deadlock` 删门节点与门校验、删 `DecisionAction::Retry` 分支；`backtrack` 落账后 `trace.incoming = Backtrack`；`Deadlock` 注释去 retry | `walker.rs`（`resolve_deadlock`） | `ToolStepPort::Backtrack` | 门禁判定只剩决策会话一处 |
| 决策输入面 | `DecisionInput` 删 `eval_only`、增 `fail_report: String`；`build_decision_input` 去参、report 取失败相位最新 attempt 原文；prompt 三动作 + 白名单自注记 + 失败报告段 + 读报告指引 | `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` / `prompt.rs` / `walker.rs`（`build_decision_input`） | `ChangeDetail` 快照（既有） | 有界输入（≤2000 + checklist + candidates），零新字段进 IPC |
| 词面注释随动 | `ChangeStepKind::RetryGate` 注释改「决策动作兑现门」、`MutationTest` 注释去「独立反馈边②」；`RunEdgeKind::Retry` 注释改「存量解码专用」 | `orchestration/src/state.rs`、`workflow/src/state.rs` | — | 变体名与线词冻结（IPC 词面），只改 doc |

---

## 关键设计决策

| # | 议题（proposal / explore 定稿项之外的设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | `Escalate` 的代码形状 | `OnFail` 增单元变体 `Escalate`；`step_test_execution` 改 `match step.on_fail` 三分派（`Terminal` → 既有单发执行体、`Escalate` → 新单发执行体、其余 → 漂移 `Err`）；`step_mutation_test` 为二分派（`Escalate` → 新单发执行体、其余 → 漂移 `Err`，该步无语义上合法的 `Terminal`）。两处漂移文案形如 `Terminal::failed("计划图漂移：… 缺 OnFail::Terminal / Escalate 声明")`。`feedback_limit` 保留签名并增 `Some(OnFail::Escalate) => Err(...)` arm（漂移兜底，运行时不达——该函数只被 static-check / verdict-gate 调用）。**不引入 `limit: 0`**，相位内零 `loop` / 零 `fix_prompt` 构造 / 零 `feedback += 1` | 「移除」不等于「禁用」（explore 否决项）；`match` 全枚举穷尽，`FeedbackLoop` 声明误落到这两步时立刻显式失败而非静默跑反馈边；`feedback_limit` 的 Escalate arm 保证门禁步声明单点读取面无 `_ =>` 兜底（沿既有「误声明即漂移」哲学） |
| D2 | 升格 `report` 的截断口径 | 私有 `fn truncate_report(text: &str) -> String`：`chars().count() <= MAX_REPORT_CHARS` 原样；否则取前 `MAX_REPORT_CHARS - 1` 字符 + `…`（总长恰 ≤2000）。空明细（理论上 fail 态非空）兜底为 `"conclusion={}；报告目录 {}"`（mutation 为 `"conclusion={}；报告路径 {}"`） | 写面 `phase_log`（`phase_log.rs:56`）对 `report` 硬校验 `> MAX_REPORT_CHARS` 即 `Err`——「截 2000 再加省略号」会变 2001 被拒；留 1 字符余量后单点保证可落账。兜底避免空 report 让决策面丢失结论行 |
| D3 | 升格 evidence 的报告面与 checklist evidence 形态 | 单一 `evidence` 文本 = `format!("{}；{}", diagnose_brief(<明细>), <报告面>)`，其中 **test 步报告面 = `outcome.report_dir`**（`reports/test` 树，直接可读 summary.json 与逐 suite 报告）；**mutation 步报告面 = `outcome.report_paths.join("、")` 原文（零改写，空清单写「（无报告产物）」）**——`MutationTestOutcome` 无 `report_dir` 字段，不新增字段（proposal 的 port 零改动红线），全路径比「推导共同父目录」更精确且可直读 | 报告面「可回查」由路径本身保证；避免为一条 evidence 在 port 上新增字段（越过本变更边界）；`diagnose_brief`（200 字符、折叠空白）保证 evidence 可读，全量明细另在 `report` 字段 |
| D4 | 白名单自注记的数据来源 | `decision_prompt` 渲染白名单时，仅对 `phase == input.phase` 的条目追加「（= 重做本相位）」——**零新字段**：`allowed` 含失败相位自身 ⟺ 该相位可重做（即「步声明与白名单推导」组件的 `redoable` 判定的直接推论） | 白名单自含即重做的单一载体（explore §4.2 / §4.3）；若再加 `redoable: bool` 字段，等于把同一事实存两份，迟早漂移 |
| D5 | 自环判定与拒绝文案的落点 | 拒绝文案单点收在 `decision.rs::ensure_backtrack_allowed`，签名增 `failed_phase: &str`：非 backtrack → `Ok`；`to` 在白名单 → `Ok`；`to == failed_phase`（且不在白名单）→ 自环文案「本相位为确定性门禁相位（零 agent 步），不支持重做；请从白名单其他相位中选择，或 stop / ask」；其他越权 → 既有携 allowed 列表文案。判定条件与 spec 逐字一致（`to == 失败相位 && 该相位不在白名单`） | 文案与判定同处，杜绝 walker 与 decision.rs 两份自环判据；写面 `backtrack` 的二次校验（`write/backtrack.rs`）继续用同一 `allowed` 判越权——**零改动**（自环在白名单内即正常受理的重做语义已由白名单推导承载） |
| D6 | 软拒环的循环形状与漂移计数 | `decision_session` 内 `loop`：`drift: usize = 0`。分支——① parse `Ok(Ask)`：emit Ask → `wait_answer` → 应答续问（**不计漂移**）→ continue；② parse `Ok(action)`：`begin_node(WhitelistGate)` + running → 门 `Ok` 则 emit passed 并 `return Ok(action)`；门 `Err` 则 emit failed（detail = `diagnose_brief(message)`）、`drift += 1`、`drift > DECISION_DRIFT_LIMIT` 即 `Err(Terminal::failed("决策漂移：连续 N 次输出被驳回（{message}）"))`、否则 `prompt = decision_fix_prompt(...)` + continue；③ parse `Err`：`drift += 1`、超限同款 Terminal（reason 携解析错误）、未超限 `decision_fix_prompt` + continue。**每条 continue 前 `continue_session = Some(outcome.session_id)`** | 上限语义「超过 2 才死」⇒ 首两次驳回可纠正（spec scenario 2 次自环仍在纠正期、第 3 次死）；parse Err 不 emit 门节点（无动作可校验，AC-4 明示「无 WhitelistGate 节点」）；ask 停在 `WaitingAsk` 属用户驱动停等，计入会误判模型漂移。会话 id 每轮更新（含 ask 续问）保证「同一决策会话」断言成立 |
| D7 | 决策输入面的 report 取值 | `build_decision_input(detail, fail_phase, allowed)`：`fail_report` 取失败相位 `station.attempts.last().report` 原文（即升格落账写入的完整明细；空则空串，prompt 侧渲染「（无报告文本）」）。删除 `eval_only` 后 `resolve_deadlock` 的 `PhaseNextError` 解构改 `{ phase, .. }` | 升格落账与决策输入**同源同锚**（同一 PhaseRecord.report），不二次拼装、不重读磁盘；`attempts` 已按 attempt 序出线，`last()` 即本轮失败条目 |
| D8 | 存量 `retry` 边处置 | `RunEdgeKind::Retry` **变体与线词保留**（解码契约），注释改「存量解码专用：新写面零产出、查询面投影前屏蔽」；唯一屏蔽点在 `queries/detail.rs` edges 投影。`walker.rs:290-294` 的「同相位上一轮 fail → `EdgeKind::Retry` 入口边」分支**代码零改动**——`DecisionAction::Retry` 退役后该分支在新写面不可达（失败相位只经决策回溯重入，`trace.incoming = Backtrack` 优先级更高），保留即存量语义的兜底，删改需另立清理变更 | 删变体 ⇒ 旧行 `serde_json` decode 失败 ⇒ `list_run_edges` 整份 `Err` ⇒ 历史流程图整份打不开（explore §4.7）；库格式 `WORKSPACE_STORE_FORMAT_VERSION = 3` 不变、不建 from 链 |
| D9 | `fail_decided` 与 `MaxRetriesExceeded` 的措辞面 | `fail_decided` 判据零改动，注释改写「该 fail 条目已进入过决策，不再重复分叉」（删「retry 决议兑现」字样）；`resolve_deadlock` 头注释与两处内联注释同步去 retry 表述。`PhaseNextError::MaxRetriesExceeded` 变体名保留，**新增 doc 注释**「本相位已连续失败 N 次（窗口 fail 数达 `MAX_RETRY_TIMES`）——决策分叉标注变体之一」（该变体无字符串载荷，文案面即 doc） | 幂等闸是「每个 fail 条目只触发一次决策」的承重墙（explore §4.6 反证链）；变体名属 IPC / 线词冻结面，本变更不改名 |
| D10 | 变更边界（不做的事） | 零 IPC / 零 bindings 再生成（`OnFail` / `DecisionInput` / `DecisionAction` 均不上 IPC，`RunEdgeKind` 值域仍四值）；零前端改动（`graph.ts` 对端点缺失边本就 `continue`）；零插件改动与版本 bump；零 store 迁移；零新 crate 依赖；`decide_on_fail` 表字段留存（explore §6 显式不做） | 逐条对应 proposal「不要修改」清单与风险缓解；任何一项破边都会把变更从「编排链语义替换」扩成跨面演进 |

### 决策分叉新时序（替代现状 parse → 门 → 一次即死）

```
phase_next: VerdictFail / MaxRetriesExceeded（最新 fail 条目未挂决策）
  └▶ resolve_deadlock
       ├─ snapshot.detail → build_decision_input（fail checklist + allowed + candidates + fail_report）
       └▶ decision_session（ModelLevel::High）
            round 1..: run_worker(Decision, continue_session) → decision-log(幂等挂账)
              ├ parse Err ─────────────▶ drift+1 → 超限 Terminal / 否则纠正续注（无节点）
              ├ Ok(ask) ──────────────▶ Ask 停等 → 应答续注（不算 drift）
              └ Ok(action) ─▶ WhitelistGate 节点（每轮一实例）
                               ├ passed ─▶ return action
                               └ failed ─▶ drift+1 → 超限 Terminal / 否则纠正续注
  └▶ 兑现：Backtrack → 写面 backtrack → trace.incoming = Backtrack → 回 phase_next 重路由
          Stop → Terminal(stopped)   Ask 泄漏 → Terminal(漂移)
```

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。清单 = proposal「变更范围 - 实现文件」八项 + 四个测试文件的**编译面机械随动**（类型 / 签名 / 变体演进使既有测试模块编译不过，必须同批适配字段、实参与 match 臂）；proposal「测试文件」表所列**断言改写、期望值更新与新增用例**仍归测试相位（test-design / test-gen / test-execution），不在本清单。`crates/core/orchestration/src/port.rs` 字段面零改动（`findings_detail` / `feedback_text` 仅换消费者），不入修改清单；`workflow/src/state_test.rs` / `workflow/src/write/phase_table_test.rs` / `orchestration/src/run_history_test.rs` / `src/views/changes/flow/graph.test.ts` 只失效于断言层（无编译断点），同样归测试相位。 -->

### 新增文件

<!-- 本变更不新增任何文件：Escalate 落既有 OnFail、软拒环落既有 decision_session、截断/纠正文案落既有 walker.rs / prompt.rs。 -->

无。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | ① `OnFail` 增单元变体 `Escalate` + doc（零会话、落 fail 条目、截断余步回决策；消费点 TestExecution / MutationTest；升格动作固定三段；无载荷）；② `OnFail` enum doc 补「三变体 + 缺省 `None`」对照；③ `RunEdgeKind::Retry` doc 改「存量解码专用：新写面零产出、查询面投影前屏蔽」 | AC-2（零会话升格的声明面）/ AC-10（既有 `FeedbackLoop` / `Terminal` 声明零改动）；`OnFail` 派生与 serde `rename_all` 零改（线词 `escalate`）；`PhaseStateRecord.decision_session_id` 字段面不动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | ① `TEST_EXECUTION_STEPS` 的 TestExecution / MutationTest 两步 `on_fail: FeedbackLoop { FEEDBACK_LOOP_LIMIT } → OnFail::Escalate`（`FEEDBACK_LOOP_LIMIT` 仍被 static-check / verdict-gate 步消费，常量保留）；② `allowed_backtrack_phases` 增 `redoable = table[idx].steps.iter().any(\|step\| step.node.role().is_some())`，过滤链追加 `.filter(\|phase\| redoable \|\| phase.id != current_phase)`；③ 函数 doc 补「自身在列 ⟺ 可重做；零 agent 步相位自排除，判断 MUST NOT 以相位名硬编码」 | AC-1 / AC-10（initialize 两步 `Terminal`、archive 11 步、其余反馈边声明零改动）；`PhaseDefinition` 零新字段（可重做自步序列派生，explore 定论 4） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | ① `fail_decided` 注释改写为「该 fail 条目已进入过决策，不再重复分叉」；② 删 `eval_only` 派生（`next.executor.is_none() && next.evaluator.is_some()`）与两处构造实参；③ `PhaseNextError` 两变体去 `eval_only` 字段；④ `MaxRetriesExceeded` 增 doc「本相位已连续失败 N 次（窗口 fail 数达 `MAX_RETRY_TIMES`）」；⑤ 决策分叉两处块注释去「retry 决议兑现」表述，改为「决策 backtrack 重入为唯一合法重入路径」 | AC-7（幂等闸语义不变、注释与 spec 对齐）；判据逻辑零改（`window_fails` / `latest_fail` / `fail_decided` 三元关系原样）；`build_phase_response` 的 `retry_feedback` 入参保留（字段面与下发面零改动） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | edges 投影链 `.filter(\|record\| record.edge_kind != RunEdgeKind::Retry)` 插在 `list_run_edges(...)?.iter()` 与 `.map(ChangeRunEdgeRecord::from)` 之间；该处补注释「`retry` 为存量解码契约，读面唯一屏蔽点，边序不变」 | AC-8；`ChangeRunEdgeRecord` 值域 / DTO / golden 零演进；nodes 投影零改动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | ① `ChangeStepKind::RetryGate` doc 改「决策动作兑现门（`retry_gate` 为历史命名，IPC 词面冻结）」；② `ChangeStepKind::MutationTest` doc 去「独立反馈边②」，改「确定性突变执行链（失败升格交决策）」 | 词面零改（变体名 / `as_str` 线词 / 二十一值封闭集不动）；纯注释面 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` | ① `DecisionAction` 删 `Retry` 变体，enum doc 改「三动作封闭集」；② `DecisionInput` 删 `eval_only`、增 `pub fail_report: String`（doc：取失败条目 `report` 原文，有界 ≤2000）；③ `parse_decision` doc 去「四动作」、补「已退役 `retry` 与拼写漂移同走结构漂移 Err（软拒路径）」；④ `ensure_backtrack_allowed` 增参 `failed_phase: &str` 并加自环文案分支（D5） | AC-4 / AC-5 / AC-9；`MAX_REASON_CHARS` 与 reason 长度透传口径零改（≤500 仍由写面 `backtrack` 单点拒绝） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` | ① `decision_prompt` 重写：删 `retry_note` 两分支；「失败相位报告」段渲染 `input.fail_report`（空 → 「（无报告文本）」）；白名单条目对 `phase == input.phase` 自注记「（= 重做本相位）」；「报告文件」段（`read` 读 `reports/test/summary.json` 与突变段级 / 归并报告）；决策协议改三动作（backtrack / stop / ask），红线句保留；② 新增 `decision_fix_prompt(input: &DecisionInput, error: &str) -> String`：驳回原因 + 三动作重申 + 白名单一行（`allowed.join(" / ")`）——软拒环每轮纠正文案单点 | AC-4 / AC-5 / AC-9；`evaluator_protocol` / `verdict_fix_prompt` / 归档 prompt 零改动 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | ① `step_test_execution` 改 `match step.on_fail` 三分派（D1），新增 `step_test_execution_escalate`（单发：running → `run_tool(TestExecution{slot: Test})?` → pass 收口挂 `cursor.test_outcome` / fail 或 error → `StepSettle::failed(diagnose_brief)` + 升格落账 + `cursor.failed = true` + `Upgraded`）；② `step_mutation_test` 改 `match step.on_fail`（`Escalate` → 新 `step_mutation_test_escalate`，runner `Err → Terminal` 分支原样内嵌；其余 → 漂移 `Err`）；③ `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` 去 `fix_session` 参数、report 改 `truncate_report(<完整明细>)`、evidence 改 brief + 报告面、三会话槽位恒 `None`、doc 文案改「首败升格（零会话）」；④ 新增私有 `truncate_report(text: &str) -> String`（D2）与 `mutation_report_refs(outcome: &MutationTestOutcome) -> String`（D3）；⑤ `feedback_limit` 增 `Some(OnFail::Escalate) => Err(Terminal::failed("计划图漂移：{label} 步不得声明 OnFail::Escalate"))` arm；⑥ `decision_session` 重写为软拒环（D6，新增 `DECISION_DRIFT_LIMIT` 常量与 `WhitelistGate` 逐轮 emit）；⑦ `resolve_deadlock` 删门节点 / 门校验 / `DecisionAction::Retry` 分支、`PhaseNextError` 解构去 `eval_only`、`build_decision_input` 去 `eval_only` 实参、头注释与 `Deadlock` doc 去 retry 表述 | AC-2 / AC-3 / AC-4 / AC-5 / AC-6 / AC-9；`step_test_execution_terminal` / `step_static_check` / `step_verdict_gate` / archive 11 步 / `drive` 主循环（`StepFlow::Upgraded` 处理面）零改动；`walker.rs:290-294` 入口边分支按 D8 零改动（注释留痕不可达）；三处受签名 / 变体演进影响的既有调用点（`:1082` 升格落账、`:1342` 升格落账、`:3080` `ensure_backtrack_allowed` 门校验）随各自契约任务同批适配以保持逐项编译可过（前两处于分派改判任务删除、最后一处于门删除任务消失） |

### 修改文件（测试模块 —— 编译面机械随动）

<!-- 类型 / 签名 / 变体演进使既有测试模块编译不过（枚举穷尽 match、结构体字面量、函数实参），故这四个文件必须随实现同批适配。本表只记编译面随动（补删字段、补参、补 match 臂、删失效变体引用），**不含任何断言改动**；断言改写见 proposal「测试文件」表，归测试相位。 -->

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs` | ① `ensure_backtrack_allowed` 四处调用（:85 / :100 / :106 / :121）补第二实参 `failed_phase`；② 删 `DecisionAction::Retry` 引用（:34 / :66 / :73 / :105）；③ `DecisionInput` 两处字面量（`eval_only` 在 :218 / :245）删 `eval_only`、补 `fail_report: String::new()` | `ensure_backtrack_allowed` 增参 + `DecisionAction::Retry` 删除 + `DecisionInput` 字段面收缩的编译随动；`retry` 解析拒绝、自环文案、占比断言的改写归测试相位 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs` | 五处 `DecisionInput` 字面量（`eval_only` 在 :86 / :131 / :155 / :172 / :194）删 `eval_only`、补 `fail_report`；`eval_only_input`（:130）随字段删除只去字段、保留结构，其指引断言改写归测试相位 | `DecisionInput` 字段面收缩的编译随动；失败报告段 / 白名单自注记 / 三动作协议的断言改写归测试相位 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` | ① `max_retries_outcome` / `verdict_fail_outcome` 助手（`eval_only` 在 :896 / :908 / :928）删参数与字段实参；② `OnFail` 穷尽 match（:1757-1759）补 `OnFail::Escalate => None` 臂（该 match 取反馈上限，`Escalate` 无上限可返） | `PhaseNextError` / `DecisionInput` 字段收缩 + `OnFail` 增变体的编译随动；反馈边用例（原 4908 / 4976 / 5021 / 5204 / 5120 / 8614 / 8695 / 8748 / 8804）、`{"action":"retry"}` fixture（23 处）、首败升格与软拒漂移用例改写归测试相位 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs` | 六处 `PhaseNextError::{VerdictFail, MaxRetriesExceeded}` 构造（`eval_only` 在 :722 / :732 / :756 / :799 / :819 / :878）删 `eval_only` 字段 | `PhaseNextError` 两变体去字段的编译随动；`fail_decided` 已决 / 新条目两例（AC-7）与文案断言由测试相位承接 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `allowed_backtrack_phases` | `workflow/src/write/phase_table.rs` | 修改 | `pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String>`（签名不变） | 过滤链增「可重做」自排除；`current_phase` 缺省 / 表头早退空语义不变 |
| `ensure_backtrack_allowed` | `orchestration/src/decision.rs` | 修改 | `pub fn ensure_backtrack_allowed(action: &DecisionAction, failed_phase: &str, allowed: &[String]) -> Result<(), String>` | 增第二参；自环与通用越权两类文案单点；非 backtrack 恒 `Ok` |
| `parse_decision` | `orchestration/src/decision.rs` | 修改（行为，签名不变） | `pub fn parse_decision(text: &str) -> Result<DecisionAction, String>` | `DecisionAction` 收缩后 `{"action":"retry"}` 与未知名同走 serde 结构漂移 `Err` |
| `decision_prompt` | `orchestration/src/prompt.rs` | 修改（签名不变） | `pub fn decision_prompt(input: &DecisionInput) -> String` | 三动作协议 + 失败报告段 + 白名单自注记 + 读报告指引 |
| `decision_fix_prompt` | `orchestration/src/prompt.rs` | 新增 | `pub(crate) fn decision_fix_prompt(input: &DecisionInput, error: &str) -> String` | 软拒环纠正文案（驳回原因 + 三动作 + 白名单一行） |

> 模块私有函数不入本表（形态与职责见「修改文件」）：`walker.rs` 的 `step_test_execution` / `step_test_execution_escalate` / `step_mutation_test` / `step_mutation_test_escalate` / `step_*_upgraded_phase_log` / `feedback_limit` / `decision_session`（签名不变：`async fn decision_session(worker: &Arc<dyn WorkerAgentPort>, tools: &Arc<dyn ToolStepPort>, guard: &RunGuard, request: &RunRequest, input: &DecisionInput, trace: &mut EdgeTrace) -> Result<DecisionAction, Terminal>`）/ `build_decision_input` / `truncate_report(&str) -> String` / `mutation_report_refs(&MutationTestOutcome) -> String`。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `OnFail` | `workflow/src/state.rs` | 修改 | 三变体 `FeedbackLoop { limit: u32 }` / `Terminal` / `Escalate`（单元变体，线词 `escalate`）；缺省 `None` 语义不变 |
| `DecisionAction` | `orchestration/src/decision.rs` | 修改 | 三变体 `Backtrack { to: String, reason: String }` / `Stop { reason: String }` / `Ask { question: String, options: Vec<String> }`；`Retry` 删除；serde 标签 `action`（camelCase）不变 |
| `DecisionInput` | `orchestration/src/decision.rs` | 修改 | 字段：`phase: String` / `attempt: u32` / `fail_checklist: Vec<ChecklistItem>` / `allowed: Vec<String>` / `candidates: Vec<CandidateReport>` / **`fail_report: String`（新增）**；**`eval_only: bool` 删除** |
| `PhaseNextError` | `workflow/src/write/phase_next.rs` | 修改 | `MaxRetriesExceeded { phase: String, round: u32 }` / `VerdictFail { phase: String, round: u32 }`（两变体均去 `eval_only: bool`） |
| `RunEdgeKind` | `workflow/src/state.rs` | 名义不变（仅注释） | 值域仍四值 `Forward` / `Feedback` / `Retry` / `Backtrack`——`Retry` 为存量解码契约，不进绑定面变更 |
| `ChangeStepKind` | `orchestration/src/state.rs` | 名义不变（仅注释） | 二十一值封闭集与 `as_str` 线词不变（`retry_gate` 词面冻结） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `DECISION_DRIFT_LIMIT` | `orchestration/src/walker.rs` | 新增 | `usize`（编译期常量） | `2` | 决策软拒环漂移上限：`drift > LIMIT`（第 3 次驳回）才 `Terminal::failed`；ask 轮次不计。非运行期可调配置，与 `MAX_ROUNDS` / `MAX_RETRY_TIMES` 同类（常量单源） |
| `MAX_REPORT_CHARS` | `orchestration/src/verdict.rs`（既有）/ `workflow/src/write/phase_table.rs`（既有） | 不修改（本变更引用） | `usize` | `2000` | 升格 `report` 截断与写面校验同源；截断留 1 字符余量（D2） |

---

## 数据模型

本变更**零 schema 演进**（store 格式 v3 不变、无迁移、无新表 / 新列），只改既有字段的**写入内容**与**读面投影**。

### 失败升格条目的字段分布

`PhaseStateRecord`（fail 条目，无字段增删）：

| 字段 | 升格写入值（test-execution 相位两步） | 语义 |
|------|--------------------------------------|------|
| `phase` | 当前相位 id（`test-execution`） | 不变 |
| `verdict` | `Fail`（由 checklist 全含 `pass=false` 推导，写面口径不变） | 触发决策分叉 |
| `report` | `truncate_report(findings_detail)` / `truncate_report(feedback_text)`（≤2000，超长留 `…`） | **决策 prompt「失败相位报告」段唯一来源**（`DecisionInput.fail_report`），取代原 feedback 边 fix prompt |
| `checklist` | 恰一条：test = `{ item: "测试执行", pass: false, evidence: "<diagnose_brief>；<report_dir>" }`；mutation = `{ item: "突变测试", pass: false, evidence: "<diagnose_brief>；<report_paths 原文>" }` | 决策输入 `fail_checklist` 的 fail 项面 |
| `executor_session_id` / `evaluator_session_id` / `decision_session_id` | 三槽位恒 `None`（本相位零 agent 会话；决策会话由 `decision-log` 另行定点挂账） | 升格条目不带会话凭证 |
| `backtrack_to` / `backtrack_reason` | `None`（决策 backtrack 时由写面 `backtrack` 定点改写） | 重入路由面 |

### `decision_session_id` 幂等闸（语义不变、注释改写）

- 语义：某相位最新 fail 条目已挂 `decision_session_id` ⇒ `fail_decided = true` ⇒ 不再触发决策分叉。
- 唯一合法重入路径：决策 `backtrack` → 写面 `backtrack` 改写最新条目 `backtrack_to` → 下一轮 `phase_next` 回溯分支先手接管 → 目标相位真执行 → **新** fail 条目（`decision_session_id = None`）照常分叉。
- 本变更不改判据，只把注释从「retry 决议兑现面」改为「该 fail 条目已进入过决策，不再重复分叉」（AC-7）。

### 执行图边（读面投影）

| 边 kind | 写面产出 | 读面（`queries/detail.rs`） | 前端 |
|---------|----------|------------------------------|------|
| `forward` | 计划序推进 | 原样出线 | 现有样式 |
| `feedback` | static-check / verdict 解析失败修复 | 原样出线 | 现有样式 |
| `backtrack` | 决策回溯（含回溯到自身 = 重做本相位） | 原样出线 | 虚线 + `backtrackReason` |
| `retry` | **新写面零产出**（分支保留但不可达，D8） | **投影前屏蔽**（唯一屏蔽点） | 输入若含则落默认样式 / 端点缺失跳过（存量行为，零改动） |

`RunEdgeRecord` 词汇与库内旧行原样保留（`retry` 行仍在库、仅在投影面被滤掉），`WORKSPACE_STORE_FORMAT_VERSION` 保持 `3`。

---

## 路由 / API 设计

<!-- 本变更不新增 / 修改任何 HTTP 端点或 IPC 命令：OnFail / DecisionInput / DecisionAction 均不上 IPC，RunEdgeKind 值域仍四值，bindings.ts 零再生成；决策与升格的输入输出全在进程内（ToolStepPort / WorkerAgentPort）。 -->

不涉及。

---

## 验收标准对齐

| ID | 设计落点 |
|----|----------|
| AC-1 | `allowed_backtrack_phases` 的 `redoable` 自排除（`phase_table.rs` 修改项 ②）：test-execution `steps` 全为确定性步 ⇒ 自身不入列；implement `steps` 含 Executor / Evaluator ⇒ 自身在列 |
| AC-2 | `step_test_execution` 的 `Escalate` 单发执行体（D1）：pass 才挂 `cursor.test_outcome` 并 Proceed；fail / error 走升格落账后 `StepFlow::Upgraded` 截断 MutationTest 与 PhaseLog；全程零 `run_worker` 调用；升格条目的三会话槽位恒 `None`（见「数据模型 - 失败升格条目的字段分布」） |
| AC-3 | `resolve_deadlock` 不再 emit `WhitelistGate`（门已迁走）；`decision_session` 首轮 `run_worker(Decision, ModelLevel::High)` 是决策前唯一 agent 请求 |
| AC-4 | `parse_decision` 对 `{"action":"retry"}` 返回 `Err` ⇒ 软拒环 `drift = 1`（≤2）、不 emit 门节点、`continue_session = Some(上轮 session_id)`、`prompt = decision_fix_prompt(...)`；第二轮合法 `backtrack` 经 `WhitelistGate` passed 返回 → 写面 `backtrack` 落账 |
| AC-5 | 自环 → 门 `Err`（D5 文案）→ 门节点 failed + `drift += 1`；第 1/2 次（drift = 1/2，不 `> 2`）同会话续注，第 3 次（drift = 3 > `DECISION_DRIFT_LIMIT`）→ `Terminal::failed("决策漂移…")` |
| AC-6 | 决策 `backtrack` 后 `trace.incoming = Some(EdgeKind::Backtrack)`，下一站入口边优先取 backtrack（`drive` 现有优先级零改动）；test-execution 复用门因输入 mtime 变化失效即真重跑（`reports/test` 树复用门语义零改动） |
| AC-7 | `fail_decided` 判据零改动、注释改写（D9）；写面 `backtrack` 改写最新条目 `backtrack_to` 的既有路径不动 |
| AC-8 | `queries/detail.rs` edges 投影单点 `.filter(edge_kind != Retry)`：retry 行不出线，其余边 `edge_seq` 序不变，`nodes` 集不变，查询不报错 |
| AC-9 | `report = truncate_report(完整明细)`（≤2000，D2）；`DecisionInput.fail_report` 取其原文（D7）；`decision_prompt` 增「失败相位报告」段 + 读报告文件指引（prompt.rs 修改项 ①） |
| AC-10 | 回归锚零触点：`initialize` 两步 `on_fail=Terminal`、implement / test-gen 的 static-check `FeedbackLoop`、verdict 解析 `FeedbackLoop`、archive 11 步、`RunEdgeKind` 四值往返——本变更只改 `TEST_EXECUTION_STEPS` 两处 `on_fail`（D1 / phase_table 修改项 ①） |

---

## 依赖

### 运行时依赖

无新增。两个门禁步由「依赖 `WorkerAgentPort`（Executor 修复会话）」降为「只依赖 `ToolStepPort`」；软拒环只用既有 `WorkerAgentPort` / `ToolStepPort` / `RunGuard`；截断用标准库 `char` 迭代；无新 crate 依赖、无新 feature。

### 构建 / 测试依赖

无新增。既有测试设施（`walker_test` 假引擎 + 假写面、`phase_table_test`、`phase_next_test`、`detail` 查询假 store、前端 vitest）继续承载验证；`cargo check` / `static-check` / `test-execution` 由桌面端管线执行。零 bindings 再生成（`bindings:check` 期望零 diff）。

---

## 待决问题

| # | 事项 | 状态 / 处置建议 |
|---|------|----------------|
| Q1 | 与未归档的 `desktop-mutation-filter-observability` 同改突变失败面（`ProcessOutput` 错误摘要装配 vs「谁来修」） | 设计面已留接口：`feedback_text` 原样进升格 `report`，生产者侧摘要口径变化对消费者零感；归档顺序冲突留待归档时人工逐句核对（proposal 风险表既有条款，本设计不另裁定） |
| Q2 | `walker.rs:290-294`「同相位上一轮 fail → `EdgeKind::Retry` 入口边」分支在新写面不可达 | 本变更 **零改动保留**（D8）；是否删该分支与其 `EdgeTrace.failed_phase` 语义一并留作后续清理变更，删除需另证「无存量路径会命中」 |
| Q3 | `decide_on_fail` 表字段（prompt 文本零消费点） | 按 explore §6 显式不做，留待后续清理变更（本设计不引入新消费者、不删字段） |
| Q4 | `MaxRetriesExceeded` 的「本相位已连续失败 N 次」文案载体 | 该变体无字符串载荷，落为变体 doc 注释（D9）；若后续需要在 UI 暴露文案，属 IPC 面演进，须另立变更（本变更不触 `bindings.ts`） |
