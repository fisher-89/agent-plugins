# 提案: test-execution-zero-agent-decision

> **变更**: test-execution-zero-agent-decision
> **日期**: 2026-10-18
> **状态**: 草稿

---

## 问题

`openspec/todo.md:73` 要求「移除 test-execution 阶段的 agent 执行，测试不通过时走决策回溯」。现状与该目标五处相悖：

1. **相位内仍在跑 agent**：`TEST_EXECUTION_STEPS` 的 TestExecution / MutationTest 两步声明 `OnFail::FeedbackLoop { limit: 5 }`，失败即注入本相位 executor 会话（`EdgeKind::Feedback` 续注）修复后重跑——`step_test_execution`（`walker.rs:1015`）与 `step_mutation_test`（`walker.rs:1262`）各有一个 `loop { run_tool; run_worker }`。测试红 / 覆盖率不达标 / 突变未达标三种失败都在相位内被 agent 修复吸收，裁决权不在决策面。

2. **缺第三种失败语义**：`OnFail` 只有两变体（`workflow/src/state.rs:389`）——`FeedbackLoop`（有会话、超限才落账、回路由）与 `Terminal`（零会话、**不落账**、failed 停车）。目标态需要的「零会话 + 落 fail 条目 + 回 `phase_next` 交决策」无法表达；用 `FeedbackLoop { limit: 0 }` 顶替是「禁用」而非「移除」（`loop {}`、`fix_prompt` 构造、`feedback += 1` 仍留在代码里，只是运行时不达）。

3. **重做语义两个载体**：决策协议里 `{"action":"retry"}` 与「回溯白名单包含自身」表达同一件事；且对零 agent 步的相位（test-execution 两步、archive）retry 只会重跑确定性步序列、产出同一结论——白烧一轮。一个概念应只有一个载体。

4. **决策信息面不足**：`build_decision_input`（`walker.rs:3249`）只取 fail checklist，失败条目的 report 文本不进决策 prompt，evidence 是 `diagnose_brief` 的 200 字硬截断，且 prompt 未告知可读报告文件。决策 agent 只凭约 200 字判断根因，而完整 findings / 突变明细就在报告树里。

5. **一次漂移即打死 run**：决策输出 parse 失败或 backtrack 越权现状直接 `Terminal::failed`（`walker.rs:3063`）——同会话内纠正机会为零；图上只见一个失败的白名单门节点，看不到第几轮被驳回。

## 提案

只改桌面端编排链（`packages/desktop/src-tauri`），插件与 CLI 零改动，五面：

1. **新增 `OnFail::Escalate`（无载荷）**：语义 = 「本相位本轮判负，落账后交给决策面」——零 agent 会话、落 fail checklist、`StepFlow::Upgraded` 截断余下步回 `phase_next`。`TEST_EXECUTION_STEPS` 两步（TestExecution / MutationTest）`on_fail` 由 `FeedbackLoop { 5 }` 改 `Escalate`；同一步词在不同表项语义不同（initialize 的 TestExecution 仍 `Terminal`），由表内自描述承载，零相位名判断。升格动作固定三段：代写 fail checklist（一条，evidence = 诊断摘要 + 报告目录）→ `phase_log` 落账 → `StepFlow::Upgraded`。`phase_table_test` 加归属守卫：`Escalate` 只允许出现在 TestExecution / MutationTest 步上（四个执行体只是否读 `on_fail`，误声明会静默失效）。

2. **retry 从决策协议退役，重做由白名单自含表达**：`allowed_backtrack_phases` 中「自身在列」⟺ 该相位可重做（步序列含 agent 步，`role().is_some()`）。零 agent 步的相位（test-execution / archive）自身不入白名单——决策 agent 上下文里不存在「重做本相位」选项；可重做相位（proposal / dev-design / test-design / implement / test-gen / code-analyze / code-review / acceptance）自身照旧在列，prompt 里自注记「（= 重做本相位）」。`DecisionAction::Retry` 变体删除——`{"action":"retry"}` 落入未知名软拒一次。

3. **白名单门迁入决策会话，成软拒环**：现状 parse → 白名单门 → 越权即 `Terminal`，改为 `decision_session` 内闭环：每轮 `run_worker(Decision, High)` → `decision-log` 挂账 → `parse_decision` → 白名单门；parse Err 或门 Err = 「模型漂移」，`drift += 1`（上限 `DECISION_DRIFT_LIMIT = 2`），同会话续注纠正文案重出一轮；超限才 `Terminal::failed("决策漂移")`。`ask` 轮次不计数（用户驱动的停等）。每轮 emit 一个 `WhitelistGate` 节点实例（passed / failed），驳回在图面可见。拒绝文案两类：自环（`to == 失败相位` 且该相位不在白名单）→「本相位为确定性门禁相位（零 agent 步），不支持重做；请从白名单其他相位中选择，或 stop / ask」；其他越权沿用携 allowed 列表的现状文案。

4. **决策信息面同批补强**：升格落账的 `report` 字段放**完整**失败明细（test = `findings_detail`、突变 = `feedback_text`，上限 `MAX_REPORT_CHARS = 2000`），evidence 放 `diagnose_brief` + `report_dir`；`DecisionInput` 删 `eval_only`、增 `fail_report`；决策 prompt 增「失败相位报告」段与一句「可用 `read` 读 `reports/test/summary.json` 与突变段级 / 归并报告」。`TestExecutionOutcome.findings_detail` 失去反馈边这一消费者后，正好转喂这里的 report 字段（字段面零改）。

5. **存量兼容取保守路线**：`WORKSPACE_STORE_FORMAT_VERSION` 保持 3（不 bump、不建 from 链）；`RunEdgeKind::Retry` 变体**保留**（记录信封走 serde_json，删变体 ⇒ 旧行 decode 失败 ⇒ 历史 run 流程图整份打不开），注释改「存量解码专用、新写面零产出」。查询面投影前单点屏蔽 `retry` 边（`queries/detail.rs`），前端因此少几条连线（已接受，`graph.ts` 对端点缺失的边本就 `continue`）。`fail_decided`（`phase_next.rs:184`）**保留**——它是「每个 fail 条目只触发一次决策」的幂等闸，删掉后任何 fail 过的相位都无法再被执行（回跳到 implement 重做 pass 后，该相位最新 fail 条目仍会再次触发决策，相位永不被执行，直烧到 `MAX_ROUNDS = 20`）。`ChangeStepKind::RetryGate` / `PhaseNextError::MaxRetriesExceeded` 词面保留（IPC 词面冻结），只改注释与文案。

## 能力

### 新增的能力

无（本变更在既有六项桌面能力内做语义替换与新增 requirement，不引入新能力）。

### 修改的能力

- **desktop-test-execution** — 「结论反馈边与独立预算」整条被「失败即升格交决策（零 agent 修复）」取代（REMOVED + ADDED）；确定性门禁步、MutationTest 时序、载荷双面、V1 边界留痕、baseline 树隔离五条随「反馈边退役 / `findings_detail` 转喂决策 report」修订。
- **desktop-change-orchestration** — 相位内 step 控制流新增「失败升格」态；决策协议四动作 → 三动作、白名单门迁入决策会话、`fail_decided` 语义改写；新增「决策会话软拒环与漂移上限」与「失败升格落账与决策信息面」两条 requirement；「MutationTest 步接线与独立反馈预算」被「MutationTest 步接线与失败升格」取代；四类节点 / 计划图与执行图对齐两条随 `retry` 边退役为存量解码专用修订。
- **desktop-mutation-testing** — 「双时点反馈边（报告指引式口径）」被「失败升格交决策（报告指引式信息面）」取代（REMOVED + ADDED）：两时点仍各自落账，但落账后交决策而非注入修复会话；「两口径反向并存」的红线随两反馈边一并失效。
- **desktop-workflow-write-face** — `OnFail` 三变体（增 `Escalate`）、`TEST_EXECUTION_STEPS` 两步 `on_fail`、`allowed_backtrack_phases` 自排除规则（白名单自含 = 可重做）、`backtrack` 目标为自身即重做本相位。
- **desktop-change-queries** — run 边投影面屏蔽存量 `retry` 边的读面契约（唯一屏蔽点；其余边序不变）。
- **desktop-change-flow-view** — 图面 `retry` 边恒缺席（投影面屏蔽）、`edgeKind` 四值词汇保留为解码契约的渲染口径。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/workflow/src/state.rs` | `OnFail` 增 `Escalate`（无载荷）；`RunEdgeKind::Retry` 保留、注释改「存量解码专用，新写面零产出，查询面投影前屏蔽」 |
| `crates/core/workflow/src/write/phase_table.rs` | `TEST_EXECUTION_STEPS` 两步 `on_fail: FeedbackLoop{5} → Escalate`；`allowed_backtrack_phases` 增 `redoable` 自排除 |
| `crates/core/workflow/src/write/phase_next.rs` | `fail_decided` 保留、注释改写为「该 fail 条目已进入过决策，不再重复分叉」；`eval_only` 派生删除，`PhaseNextError` 两变体去 `eval_only` 字段，`MaxRetriesExceeded` 文案改「本相位已连续失败 N 次」 |
| `crates/core/workflow/src/queries/detail.rs` | edges 投影前 `.filter(edge_kind != RunEdgeKind::Retry)` |
| `crates/core/orchestration/src/state.rs` | `ChangeStepKind::RetryGate` 注释改「决策动作兑现门」（变体名与线词不动）；`MutationTest` 注释去「独立反馈边②」 |
| `crates/core/orchestration/src/walker.rs` | `step_test_execution` / `step_mutation_test` 删反馈循环、加 `Escalate` 前置分支（各自保留 `Terminal` 分支与 runner `Err → Terminal`）；两处升格落账改文案、去 `fix_session` 槽位、report 携完整明细、evidence 携 `report_dir`；`feedback_limit` 增 `Escalate` arm（漂移 Err、不可达）；白名单门迁入 `decision_session`、`DecisionAction::Retry` 分支删；软拒环（drift ≤ 2）与 ask 续问文案三动作化 |
| `crates/core/orchestration/src/decision.rs` | 删 `Retry` 变体；`DecisionInput` 删 `eval_only`、增 `fail_report` |
| `crates/core/orchestration/src/prompt.rs` | 三动作协议、白名单自注记「（= 重做本相位）」、删 retry 指引二分支、增失败报告段与读报告文件提示 |

### 测试文件

| 用例 | 位置 | 处置 |
|---|---|---|
| 反馈边 fail / error 走反馈边 / 恰五次超限 / 修复重入 / 双门禁独立计数 | `orchestration/src/walker_test.rs`（原 4908 / 4976 / 5021 / 5204 / 5120） | 改为「首败即升格、零 Executor / Evaluator 请求、恰一条 fail 条目」；计数类用例退役 |
| mutation 四条反馈边用例 | `walker_test.rs`（原 8614 / 8695 / 8748 / 8804） | 改为「report 携 `feedback_text` → 首败升格」；两条计数用例退役 |
| `test_execution门控声明锚定` | `walker_test.rs`（原 5270） | `test_execution_feedback_limit` 断言改 `Escalate` 归属 |
| `{"action":"retry"}` fixture（23 处） | `walker_test.rs` | 改为软拒一次 + 合法动作，或直接改 stop / backtrack 序列 |
| `OnFail` 往返 | `orchestration/src/state_test.rs` | 补 `Escalate` 往返 + 非法词表 |
| 拆步 / `on_fail` 断言 | `workflow/src/write/phase_table_test.rs` | 两步 `on_fail` 期望值更新 + `Escalate` 归属守卫 |
| `eval_only` 断言 | `workflow/src/write/phase_next_test.rs` | 随字段删除一并清理；`decision_session_id` latch 用例保留（latch 仍在） |
| `RunEdgeKind` 四值往返 | `orchestration/src/run_history_test.rs` | 保留（测的是解码兼容） |
| `decision_test` / `prompt_test` | `orchestration/src/` | 删 retry 解析 / 自注记 / 失败报告段断言 |
| 前端 retry fixture | `src/views/changes/flow/graph.test.ts`（原 311 / 402） | 改写为「输入含 `retry` 边 → 输出图不含该边」的投影锚 |
| 新增 | — | `allowed_backtrack_phases` 自排除 / 可重做自含、查询面 `retry` 屏蔽、决策软拒漂移与上限、首败升格零会话 |

### 不要修改

- `src/types/generated/bindings.ts`：`OnFail` / `DecisionInput` 均不上 IPC，`RunEdgeKind` 值域仍四值——无需重生成。
- `src/views/changes/flow/graph.ts` / `change-flow-graph.tsx`：`retry` 本就落默认样式，零改动。
- `crates/infra/store`：库格式 v3 不变，`RunEdgeRecord` 词汇不变（`retry` 变体保留为解码契约）。
- `crates/infra/checks`（testexec / mutation runner）：执行面与报告面零改动（`findings_detail` / `feedback_text` 字段语义不变，只换消费者）。
- `plugins/**` 与插件交付产物（`claude-plugins` / `cursor-plugins` / `cursor-home-image`）：插件零改动、零版本 bump。
- `explore.md`：只读。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `allowed_backtrack_phases(TEST_ONLY_TABLE/REQUIREMENT_TABLE, "test-execution")` 不含自身；`(…, "implement")` 含自身 | `phase_table_test` 断言（两表各一例） |
| AC-2 | test-execution 首败（`conclusion=Fail`）：零 `WorkerRole::Executor` / `Evaluator` 请求、恰一条 `verdict=Fail` PhaseRecord、`TestExecution` 节点 failed、余下步不执行 | `walker_test` 假引擎 + 假写面直驱 |
| AC-3 | 首败后走决策分叉：决策会话恰一次、`ModelLevel::High`；决策前零 `WhitelistGate` 节点 | `walker_test` 请求序列断言 |
| AC-4 | 决策输出 `{"action":"retry"}` → 解析失败走软拒一次（无 `WhitelistGate` 节点、漂移 +1、同会话续注，`continue_session` 断言）→ 第二轮合法 `backtrack` 通过白名单门落账 | `walker_test` 决策脚本两轮 |
| AC-5 | 决策 `backtrack` 目标 = 失败相位且该相位零 agent 步 → `WhitelistGate` failed 节点 + 自环文案；连续 3 次自环（漂移计数超 `DECISION_DRIFT_LIMIT = 2`）→ `Terminal::failed("决策漂移")`；前两次驳回均同会话续注纠正 | `walker_test` 漂移上限用例 |
| AC-6 | 组合链：fail → 决策 backtrack → implement → test-execution 复用门因 mtime 变化失效 → 真重跑 → pass；重入的 test-execution 入口边为 `backtrack`（非 `retry`） | `walker_test` 端到端 + 边 kind 断言 |
| AC-7 | `fail_decided` 回归：backtrack 重入后新 fail 条目照常分叉；同一条目不重复分叉（不触发第二次决策） | `phase_next_test` 已决 / 新条目两例 |
| AC-8 | 查询面：库中含 `retry` 边的旧 run → detail 投影后 edges 不含该边，其余边序不变 | `detail` 查询用例（store 假实现直插 retry 行） |
| AC-9 | 升格落账 report 携完整明细（长 findings 用例 > 200 字仍落在 ≤2000 内），decision prompt 含「失败相位报告」段与读报告指引 | `walker_test` + `prompt_test` |
| AC-10 | 回归锚不变：initialize 基线两步 `on_fail=Terminal`、implement / test-gen 的 static-check 反馈边、verdict 解析失败反馈边、archive 11 步、`RunEdgeKind` 四值往返 | 既有用例全绿 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| 决策频次上升（每次 fail 必决策而非相位内自修） | token 消耗与用户等待增加 | 中 | `MAX_ROUNDS = 20`（按 `entries.len()` 计）硬兜底、每轮循环必落 ≥1 条目 ⇒ token 损失封顶；不做周期检测的裁定留痕于此 |
| 与未归档的 `desktop-mutation-filter-observability` 改同一段 spec / 同一处反馈面 | 归档顺序冲突、合并丢失一方意图 | 高 | 该变更改的是错误摘要装配（`ProcessOutput`）与保留产物路径，本变更改的是「谁来修」（升格交决策）；两者落在同一 requirement 时以本变更的 REMOVED + ADDED 为准，错误摘要口径需在合并时逐句承接（归档时人工核对） |
| `run_worker` 会话复用语义被误改（`continue_session`） | 软拒环变成每轮新会话，纠正失效 | 中 | 软拒环仅在第 2 轮起携 `continue_session = Some(上轮 session_id)`；用例 AC-4 显式断言 |
| 自环文案与「通用越权」文案混淆 | 决策 agent 反复试自环烧 drift | 低 | 自环判定以「`to == 失败相位` 且该相位不在白名单」为唯一条件；文案明示可选项与 `stop` / `ask` 退路 |
| `Escalate` 误声明到不读 `on_fail` 的步 | 静默失效（声明被忽略） | 低 | `phase_table_test` 归属守卫：`Escalate` 仅允许 TestExecution / MutationTest 步 |
| `retry` 边投影屏蔽使历史流程图缺线 | 用户可见的图面变化 | 确定（已接受） | `RunEdgeKind::Retry` 保留解码、查询面单点屏蔽；`graph.ts` 对端点缺失边本就 `continue`，不崩 |
| 23 处 retry fixture 改写掩盖真实回归 | 决策路径断言被削平 | 中 | 改写后须保留「决策会话被唤起恰一次」与「放行恰一次重跑」两类断言（可用 `backtrack` 到自身承载） |
| explore 未决项：`decide_on_fail` 表字段暂留（prompt 文本零消费点） | 死资产继续留存 | 低 | 本变更显式不做（`evaluator.is_some()` 标记保留），留待后续清理变更 |
