# 设计: backtrack-window-context

> **变更**: backtrack-window-context
> **日期**: 2026-11-11
> **基于**: `proposal.md`（问题 / 提案 1–7 / 变更范围 / AC-1..AC-15 / R-1..R-11）、`explore.md`（只读，657 行：现状链路 / 代码锚点 / 定稿方案 4.1–4.12 / 18 条裁决 / 实现前必读 7.8）与两份 delta spec（`desktop-workflow-write-face` 2 MODIFIED + 2 ADDED / `desktop-change-orchestration` 3 MODIFIED）
> **性质**: 路由面语义补全——「回溯背景」从「目标相位的一次性 prompt 后缀」改为「回溯窗口内每个相位、每个 agent 步随行的背景段」。**零新表 / 零新列 / 零 schema 迁移 / 零前端改动 / 零 bindings 改动 / 零新 port / 零新 step 词**。
>
> **本篇边界**（改动 7 个文件 = 6 个产品源码 + 1 个版本行）：
> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs`、`.../write/phase_table.rs`、`.../write/mod.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs`、`.../orchestration/src/prompt.rs`、`.../orchestration/src/walker.rs`、`packages/desktop/package.json`（版本一行）。另加 4 个**测试文件**的结构体字面量补字段 9 处（编译闭合随动，不新增用例，见「变更清单 - 编译闭合随动」）。
> **零 diff 面**：`crates/core/workflow/src/prompts/**`（实测 14 份角色 md：6 executor + 8 evaluator，零改动；见事实校准 7）、`crates/core/workflow/src/write/backtrack.rs`、`crates/infra/store/**`、`crates/core/orchestration/src/{port.rs,steps.rs,control.rs,state.rs}`、`crates/core/orchestration/src/{verdict.rs,transcript.rs,run_history.rs,snapshot.rs}` 的既有文本、`packages/desktop/src/**`（含 `views/changes/flow/graph.ts`）、`src/types/generated/{bindings.ts,bindings.zod.ts}`、`openspec/specs/**`、`.github/workflows/**`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 版本行、本 change 的 `explore.md`。
> **零新增文件、零新增依赖、零新增 IPC 命令。**

---

## 提案与规格同步状态

提案的七条条款与 explore 的 18 条裁决已定稿，两份 delta spec 已定稿。本设计不重述论证，只做五件事：

1. 把 spec 已钉语义、未钉**函数形态**的三处收敛到可编译的具体签名：**推导 fn 与渲染 fn 的入参 / 可见性 / 名称**（D12 + 事实校准 2）、**两个响应生产者如何拿到 `entries` 与 `table`**（事实校准 1）、**决策输入如何拿到窗口集合**（D9 的透传形参）。
2. 把「渲染段与相邻段的**分隔符归属**」钉成逐字节契约（D8）——`reason_suffix` 删除后，空集分支必须与现状 prompt **逐字节相同**（否则 `decision_prompt` 的既有断言与 `phase_next_test` 的非窗口用例全部被无意义扰动）。
3. 把「上限双分支」在**实现层**收敛成一条等价表达式并给出等价链（D4），同时把「先裁剪后排序」的错序写成显式禁令（D5）。
4. 逐条 AC 给落点（「验收标准回应」表），**不写测试策略**（用例编写与门禁执行由独立相位承接）。
5. 把三处必须显式留痕的边界写成可实现形态：legacy 标记行缺 `reason` 的渲染形态（D14 / 待决问题 1）、原因原文含 `</backtrack>` 的不设防线（D13 / 待决问题 2）、深层嵌套不提示裁剪（D13 / 待决问题 3）。

### 起点实况核实（工作树实测；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）

- **写面路由**：`crates/core/workflow/src/write/phase_next.rs`（374 行）——`SessionAnchors`（`:13-35`）、`PhaseNextError`（`:39-51`）、`LastResult`（`:53-61`）、`ResolvedPhaseSpec`（`:63-70`，doc 现写「失败反馈段 / 回溯原因行（append）」）、`PhaseNextOutcome`（`:75-92`）、`phase_next`（`:97-235`）、backtrack 分支（`:134-156`，`backtrack_reason` 在此单点读取后传入 `build_phase_response`）、`round` 与 `MAX_ROUNDS` 判定（`:119-130`）、`window_fails`（`:181-184`）、`latest_fail` / `fail_decided` 幂等闸（`:190-192`）、两处决策分叉 → `error_outcome`（`:194` / `:215`）、正常路由 → `build_phase_response`（`:226-234`）、`error_outcome`（`:239-256`）、`has_phase_passed`（`:260-264`）、`latest_entry`（`:268-280`）、`latest_phase_entry`（`:284-299`）、`latest_result`（`:302-309`）、`fail_feedback_suffix`（`:314-334`，**段首恒 `\n\n`**）、`build_phase_response`（`:339-374`，doc 现写「回溯原因后缀」、`reason_suffix` 在 `:349-351`、拼接序在 `:353-357`）。三处 `PhaseNextOutcome` 字面量：`:164`（done）/ `:244`（error_outcome）/ `:360`（build_phase_response）。
- **写面常量区**：`crates/core/workflow/src/write/phase_table.rs`（852 行）——`MAX_RETRY_TIMES = 5`（`:41`）、`MAX_REPORT_CHARS = 2000`（`:44`）、`MAX_REASON_CHARS = 1000`（`:46`）、`MAX_ROUNDS = 20`（`:62`）、`FEEDBACK_LOOP_LIMIT` / `ARCHI_FEEDBACK_LIMIT`（`:66` / `:70`）；`PhaseDefinition` 以**表内下标**即泳道序（`table.iter().position(...)` 是 `backtrack_options`（`:811-832`）/ `backtrack`（`backtrack.rs:51-74`）共用的既有取序手法）。
- **出口面**：`crates/core/workflow/src/write/mod.rs`——`pub use phase_next::{phase_next, LastResult, PhaseNextError, PhaseNextOutcome, ResolvedPhaseSpec, SessionAnchors};`（`:21-23`）、`pub use phase_table::{…}`（`:25-28`）。
- **落库载体（零改动）**：`crates/infra/store/src/store.rs`——`apply_change_backtrack`（`:1770-1830`）在写事务内改写「发起相位最新条目（`max_by_key((timestamp, id))`）」的 `backtrack_to` / `backtrack_reason`（`:1814-1815`，**恒写 `Some(reason)`**）、目标相位最新 pass 与 `stale_dependents` 闭包置 stale；`log_change_phase` 的 `next_id = all.last().map_or(1, |last| last.id + 1)`（`:1743`）；`list_phase_records`（`:1914-1950`）按主键自然序（id 升序）返回。`PhaseStateRecord`（`crates/core/workflow/src/state.rs:100-133`）字段面含 `id: i64` / `phase` / `backtrack_to` / `backtrack_reason` / `timestamp`。
- **决策面**：`crates/core/orchestration/src/decision.rs`（91 行）——`MAX_REASON_CHARS = 500`（`:6`）、`DecisionAction`（`:9-21`）、`CandidateReport`（`:23-29`）、`DecisionInput`（`:31-49`，现有五字段 `phase` / `attempt` / `fail_checklist` / `fail_report` / `allowed` / `candidates`）、`parse_decision`（`:55-60`）、`ensure_backtrack_allowed`（`:67-90`）。
- **决策 prompt**：`crates/core/orchestration/src/prompt.rs`（211 行）——`use workflow::write::BacktrackOption;`（`:1`）、`evaluator_prompt`（`:57-59`）、`decision_prompt`（`:75-137`，段序由 `:120-136` 的单个 `format!` 串给定：开头句 `:121` → `## 失败相位报告` `:122` → `## 失败清单` → `## 可回溯白名单` → `## 候选相位最近一次评估报告` → `## 报告文件` → `## 决策协议`）、`decision_fix_prompt`（`:141-154`）、`spec_sync_prompt`（`:166-186`）/ `merge_conflict_prompt`（`:190-211`，archive 两步，**零改动**）。
- **walker**：`crates/core/orchestration/src/walker.rs`（3965 行）——`use workflow::write::{…}`（`:8-13`）、`drive` 的 `phase-next` 调用与 `resolve_deadlock` 分叉（`:237-253`）、`step_executor` doc（`:791-792`，现写「回溯原因后缀全在写面」）、`resolve_deadlock`（`:3302-3400`，`build_decision_input(&detail, fail_phase, allowed)` 在 `:3328`）、`build_decision_input`（`:3550-3606`，`DecisionInput { … }` 字面量在 `:3600-3606`）。
- **结构体字面量落点（编译器兜住的 9 处）**：生产 3 处（上列 `phase_next.rs` 三点）；测试 6 处 `PhaseNextOutcome`——`crates/core/orchestration/src/port_test.rs:27`（`sample_phase_next_outcome`）、`walker_test.rs:931`（`route_with_round`）/ `:952`（`done_outcome`）/ `:967`（`max_retries_outcome`）/ `:992`（`verdict_fail_outcome`）/ `:1770`（`archive_route_outcome`）。`DecisionInput` 完整构造器 3 处——`prompt_test.rs:124`（`base_input()` 权威 fixture）/ `:610`（确定性用例内联构造）、`decision_test.rs:361`（`decision_input_fail_report字段面` 的 `base`）；其余 19 处均以 `..base_input()` / `..base.clone()` spread 构造，自动随动。
- **S2 断言现状**：产品 `.rs` 内 `⚠️ 回溯原因` / `reason_suffix` 共 4 类落点——产品 3 处（`phase_next.rs:337` doc / `:349-351` 生成 / `walker.rs:792` doc）；测试断言 4 处（`phase_next_test.rs:718` / `:1035` / `:1084` / `:2098`，相邻说明 8 处）。`grep -rn "回溯原因"` 在 `packages/desktop/src/**` 零命中。
- **出线与生成物**：`packages/desktop/src/types/generated/bindings.ts` 对 `PhaseNextOutcome` / `DecisionInput` / `BacktrackOption` **零命中**（`crates/tools/export-bindings/src/main.rs` 不引用 `workflow::write`）⇒ 新增字段零 TS 联动、`bindings:export` 零 diff。
- **版本现状**：`packages/desktop/package.json:3` `version = "0.6.31"`（`HEAD` = `52f32a8`）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟。

### 事实校准（不改 spec 意图，只定实现形态）

1. **两个响应生产者需增形参（spec 只写「填值」，未写取数面）**：`error_outcome` 现有签名 `(round, last_result, error)` 手上既无 `entries` 也无 `table`，但窗口推导两者都要。裁定：`error_outcome` 增 `entries: &[PhaseStateRecord]` + `table: &'static [PhaseDefinition]` 两个形参；**失败相位不另开形参**——`PhaseNextError` 的 `MaxRetriesExceeded { phase, .. }` / `VerdictFail { phase, .. }` 已自带该值，函数体内以 `let PhaseNextError::MaxRetriesExceeded { phase, .. } | PhaseNextError::VerdictFail { phase, .. } = &error;` 单点取出（与 `resolve_deadlock` 既有的同款 match 手法一致）。`build_phase_response` 增 `entries` 形参、删 `backtrack_reason` 形参。`phase_next` 自身签名逐字不变。
2. **`open_backtrack_windows` 取 `pub(crate)`、只有渲染 fn 取 `pub`**：可见性是**纪律载体**而非语言限制——`write::phase_next_test` 是 `phase_next` 的兄弟模块（`write/mod.rs` 的 `mod phase_next_test;`），`pub(crate)` 已足够被测试直接调用；而 orchestration 跨 crate 唯一需要的只有渲染 fn（复用文案单点）。若把推导 fn 也 `pub` 并再导出，orchestration 就有了「自己算一遍窗口」的能力，D1 的单点纪律退化为 review 纪律。故：推导 fn `pub(crate)`、渲染 fn `pub`（经 `write/mod.rs` 再导出，spec Module Contract 已钉）。
3. **`MAX_BACKTRACK_WINDOWS` 保持 `pub(crate)`（proposal 原文如此）⇒ orchestration 侧（含 `prompt_test`）取不到该常量**：这是刻意的结构性执法——orchestration 无法在本侧引用上限，也就无法写出第二份裁剪分支（AC-10 的机械保障）。引用该常量的断言只允许出现在 `crates/core/workflow/**` 内（`phase_next_test` 经 `super::phase_table::MAX_BACKTRACK_WINDOWS` 可见）。
4. **`table` 形参不取 `&'static`**：推导 fn 取 `&[PhaseDefinition]`（与既有 `backtrack_options` / `allowed_backtrack_phases` / `backtrack` 的取法一致，生命周期不耦合）；`build_phase_response` / `error_outcome` 的 `table` 形参保持既有 `&'static [PhaseDefinition]`（`phase_table()` 返回 `Option<&'static …>`，逐字不动）。
5. **分隔符归属必须钉死**：现有 prompt 的三段拼接是 `{context_header}{spec.prompt}{reason_suffix}{feedback_suffix}`，`reason_suffix` 自带前导 `\n\n`、`fail_feedback_suffix` 也自带前导 `\n\n`（`:331`）。若把窗口段的**尾随**分隔符留在段内，非窗口用例与窗口用例的空白形态会分裂。故渲染 fn 的字节契约定为「前导 `\n\n` 归段内、尾随零换行」（D8），使空集分支产出 `""` 时与现状逐字节相等。
6. **proposal 写「生产 3 处字面量 + 测试 6 处」是对的**，但 `walker.rs:3877` 的 `impl TryFrom<ToolStepOutput> for PhaseNextOutcome` 是**拷贝解构**（`Ok(*outcome)`），不构造字面量，无需改动——登记防误改。
7. **`crates/core/workflow/src/prompts/**` 实为 14 份 md，非 16 份**：目录实测 14 个文件（executor 6：`proposal-planner` / `dev-design-planner` / `test-design-planner` / `implementation-generator` / `test-gen-generator` / `code-analyze-planner`；evaluator 8：`proposal` / `dev-design` / `test-design` / `implementation` / `test-gen` / `code-review` / `acceptance` / `code-analyze`），`phase_table.rs` 的 `include_str!` 恰 14 条装配点（`:72-85`）、`grep -c include_str!` 的 16 含 2 条 doc 内字面量。proposal「prompts/**（16 份角色 md，零改动）」的**结论面不变**（零改动），计数以实测为准（`code-review` / `acceptance` 为评估-only 相位，无 executor md；`initialize` / `test-execution` / `archive` 为零 agent 步相位，无任何 md）。
8. **实现相位交付态是「编译绿、`cargo test` 在 4 处必红」——这是刻意的相位切分，不是缺陷**：`phase_next_test.rs` 的 4 处 `⚠️ 回溯原因` 断言（`:718` / `:1035` / `:1084` / `:2098`）与相邻 8 处说明断言的是 S2 已删除的文案，落地后必然失败。proposal「测试文件」表把「S2 断言改写」明确划给测试相位，本相位**不改测试断言** ⇒ `cargo check --workspace --all-targets`（编译）绿、`cargo test` 在这 4 处红是**预期中间态**，由 test-gen / test-execution 相位收口。此条必须显式登记，否则实现相位会被误判为「门禁未过」；`tasks.md` 收尾自查据此只判「编译 + clippy 零新告警」，不判 `cargo test` 全绿。
9. **本变更不新增 / 不改动模块 API 契约（`.dev-team/architecture/api/**` 零 diff）**：新增两个 `pub` 项（`BacktrackWindowEntry` / `backtrack_window_section`）落在 `App.Backend.Core.Workflow` 元素下，该元素在 `INDEX.toml` 现状为 `status = "absent"`（`types = 0` / `ops = 0`）、且模型未标 `api`（缺省 `declared`，缺契约的 `apiMissing` 是**既有 warn 级基线**、不进 `status`）。`api:check` 的 `apiUndeclared` 档位为 `Off`（`drift/report.rs:231`）、`ApiUnexpected` 只查「`api = none` 却有契约文件」——故新增公开项**不引入任何新 finding**，`api:export` / `api:check` 均零 diff。若将来要把这两项纳入契约，是一次独立的契约补录（hand-written 契约 + `api:export`），**不在本变更范围**。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 窗口由谁派生、向谁下发 | **写面派生，随 `PhaseNextOutcome.backtrack_window` 下发**；orchestration 零二次推导（决策输入原样透传） | walker 不直读 store（路由权威唯一、经 `ToolStepPort`），故背景必须由写面随响应供给；一处派生 = 一处判据 = 一处裁剪（AC-6 / AC-9 / AC-10 的源码单点判据） |
| D2 | 判据三条的精确形态 | `col(P)` = `table.iter().position(\|def\| def.id == P)`（与 `detail.pipeline` 下标、前端 `colIndex` 同源，MUST NOT 另引第二份相位序）；**拼接** `to_index ≤ col(P) ≤ from_index`；**未闭环** = 发起相位无 `id > 标记行 id` 的条目；**堆叠** = 按标记行 `id`（落账序）升序；**裁剪** = 取末位 `MAX_BACKTRACK_WINDOWS`。标记行 `id` 只是推导中间元组的排序键，**不落 `BacktrackWindowEntry`** | 三处同源已核实（`queries/detail.rs` 的 `pipeline` 由 `phase_table` 生成）；`id` 即落账序（`log_change_phase` 写事务内 `max+1`），标记行恒为发起相位末次落账 ⇒ 创建序可用 `id` 表达，零新时钟列（explore 4.6） |
| D3 | 标记行的识别 | 遍历**全量** `entries` 中 `backtrack_to` 经 `trim` 后非空的条目为候选标记行；`to` 取 `trim` 后的相位 id（与 `phase_next` backtrack 分支 `:135-140` 同一读取口径）；`reason` 取 `backtrack_reason` 原文（`None` → `""`，**不 trim**）；标记行相位或目标相位不在表内 → 跳过该候选（防御式，生产不可达） | 历史标记行在发起相位重跑后**永久留在库中**（store 只增行不改旧行），窗口的关闭完全由「未闭环判据」承担——这正是零持久化载体可行的前提 |
| D4 | 上限双分支的实现形态 | 实现写成 `let start = hits.len().saturating_sub(MAX_BACKTRACK_WINDOWS); hits.into_iter().skip(start)` 一条表达式；**等价链**：`≤ 上限` ⇒ `saturating_sub` = 0 ⇒ 全量并集；`> 上限` ⇒ 取末位两条。**语义定义唯一表述** = 「保留创建序末位两层，裁掉**创建序最早**那条」≡「`take_last` 首先丢弃」≡「该组中 `from_index` **最大**者」 | spec 要求双分支必须写清（AC-4 / AC-15），但实现层不需要真写 `if len <= MAX {…} else {…}`——一条表达式对两个分支同真，且不可能出现「只实现一支」的漏写。等价链的方向固定为「被裁者 `from_index` **最大**」（结构性性质：同时开放窗口按创建序 `from_index` 严格递减），MUST NOT 读成「`from_index` 最小」 |
| D5 | 求值顺序 | **`filter(open) → filter(connect) → sort(创建序升序) → take_last(MAX)`**，四步不得换序 | 先裁剪后排序取到的是「输入末位两条」而非「创建序末位两条」；`entries` 的 id 升序是 store 的自然序但**不得据此省略显式排序**（AC-5 反例用例刻意构造 `输入条目序 ≠ 创建序`） |
| D6 | 自环回溯 | 零特判：`to == from` 时 `to_index == from_index`，区间退化为单点，与常规回溯走**同一条代码**；源码 MUST NOT 出现 `if to == from` 分支 | 区间判据天然覆盖单点；特判会成为第二份判据（AC-2 的源码审查项） |
| D7 | 跨 run 继承 | 推导 fn 入参**只有** `entries` / `table` / `phase_id`；**不入参** `SessionAnchors` / `run_id` / `anchor`，函数体零 `entries[anchor..]` 切片 | `round` / `window_fails` 按 run 窗口（预算层）、回溯窗口按全量事件序（背景层），两层口径有意不同；混用会把跨 run 未闭环窗口误判为已闭环（AC-3 / spec「口径分层」） |
| D8 | 渲染段的字节契约 | `backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String`：空集 → `""`；非空 → `"\n\n## 回溯重做窗口（背景信息）\n\n" + 说明段 + "\n\n" + 各块以 `"\n\n"` 连接`，块形如 `"<backtrack reason>\n{reason}\n</backtrack>"`；**前导 `\n\n` 归段内、尾随零换行** | 使两处消费点的拼接都只需一次插值：写面 `format!("{context_header}{}{section}{feedback}", spec.prompt)`（`feedback_suffix` 自带前导 `\n\n`）、决策面 `"…路由决策。{section}\n\n## 失败相位报告…"`。空集分支因此**逐字节**等于现状 prompt，既有非窗口断言零扰动（AC-7 / AC-10 的「其余段序与内容逐字不变」）。**evaluator 最终序**由外层决定：`ResolvedPhaseSpec.prompt`（含窗口段）→ `evaluator_prompt` 再 append 输出协议附录（`walker.rs:1542`）⇒ `上下文头 → 静态主体 → 窗口段 → 失败反馈段 → 协议附录`——walker 侧零改动即成立 |
| D9 | 决策输入的窗口来源 | `DecisionInput` 增 `pub backtrack_window: Vec<BacktrackWindowEntry>`；walker `build_decision_input` 增形参 `backtrack_window: &[BacktrackWindowEntry]`，`resolve_deadlock` 以 `&next.backtrack_window` 传入（**不传 `&PhaseNextOutcome`**，保持该 fn 的取数面窄） | 透传即「写面权威」；若传整个 `next`，`build_decision_input` 会顺带握有 `steps` / `executor` 等无关面，诱导后续二次推导（AC-10 的「零 store 读取、零相位表推导」源码判据） |
| D10 | 决策 prompt 的段位置与零副本 | `decision_prompt` 在开头句与 `## 失败相位报告` 之间插入 `{section}`；文案与渲染**只调写面 `backtrack_window_section`**，本侧零第二份文案、零裁剪分支 | 文案单点（AC-9）；位置钉在段序最前（背景比失败明细更宏观）；空集时 `{section}` = `""` ⇒ 既有五段与协议段逐字不变 |
| D11 | S2 的删除面 | 删 `reason_suffix`（`phase_next.rs:349-351`）与 `build_phase_response` 的 `backtrack_reason` 形参；backtrack 分支（`:146-154`）与正常分支（`:226-234`）的实参序列收敛为同一形（只差 `retry_feedback`）；`⚠️ 回溯原因` 字样在 `.rs` 内零命中 | 窗口段是原因行的**同位替换**而非新增；入参收敛使「回溯分支」不再有专属形参，也消除「两处渲染各自漂移」的可能（AC-7） |
| D12 | 命名 | `BacktrackWindowEntry { from_phase, from_index, to_phase, to_index, reason }`；推导 `open_backtrack_windows`、渲染 `backtrack_window_section`、常量 `MAX_BACKTRACK_WINDOWS` | 与渲染句「从 from 回溯至 to」同向；**MUST NOT** 启用 `window_start` / `window_end`（那是泳道位置的读法，与渲染方向相反，两套方向并存即埋反向映射坑，裁决 9）；`section` 后缀沿用既有「段 / 面」命名（`fail_feedback_suffix` / `evaluator_protocol` / `decision_prompt`） |
| D13 | 渲染面纪律（三条禁令） | ①只取 `reason`，标签行 `<backtrack reason>` **零属性**，prompt 正文零相位 id 字面量（`from_phase` / `to_phase` 值零命中）；②原因**不截断、不压平、不 trim**（含首尾换行与标点逐字保留），MUST NOT 套用 `diagnose_brief` 的 200 字口径；③命中 ≥ 上限时**零提示**（无「另有 N 条」/「仅显示最近 2 次」类元信息） | 结构面 / 渲染面分离（裁决 16）；转义等于改写原文（R-3 不设防线）；提示语会诱导 agent 追问或自行推测缺失内容（裁决 15）。三条的防御面是测试断言而非纪律（R-5） |
| D14 | legacy 缺 `reason` 标记行的处置 | 推导**不**因 `reason` 空白而剔除（判据三条不含 `reason`）；渲染对空 `reason` 产出空正文块（`<backtrack reason>\n\n</backtrack>`） | 生产路径**不可达**：`apply_change_backtrack` 恒写 `backtrack_reason = Some(command.reason)`（`store.rs:1815`），而 `store` 零改动；仅「旧 `phase_log` 机制遗留的 `backtrack_to` 兼容行」理论上可能缺 reason。若届时可达，正确改法是给判据加第四条过滤并同步改 spec 三条判据——一次裁决，不在实现期暗改（待决问题 1） |
| D15 | 文件规模与拆分 | **不拆**：`phase_next.rs` 374 → ~470 行（同 crate：`phase_table.rs` 852 / `create.rs` 459 / `phase_next_test.rs` 2156） | 关键理由是数据依赖而非行数：窗口判据需要 `entries` + `table` 同时在场，而它们正是 `phase_next` 的既有入参与 `build_phase_response` 的既有形参域；拆文件必须把二者跨模块传递，且多出 `mod` 声明 + `pub use` + 测试搬迁三笔账（R-10）。若后续坚持拆分，唯一可接受切面 = 判据 + 渲染整体成 `write/backtrack_window.rs`，MUST NOT 把判据与渲染分到两个文件 |
| D16 | 明确不做 | ①不改 `run_id` / `SessionAnchors` / `round` / `window_fails` 的既有预算层语义；②不改 `backtrack.rs` 的写面裁定（白名单 / 表位 / `reason ≤ 1000` / stale 传播）；③不给 `PhaseNextOutcome` 加 `Serialize`；④archive 两步（`spec_sync_prompt` / `merge_conflict_prompt`）零窗口段；⑤不动 `evaluator_protocol` / `verdict_fix_prompt` / `decision_fix_prompt` 文本；⑥不动图面 / 前端 / 生成物；⑦不给 store 加列（含决策时刻列 / 窗口号列）；⑧不因体积顾虑在窗口渲染路径新增 `truncate` / `chars().take(…)` | 逐条对应 proposal「不要修改」与 AC-11 / AC-13 / AC-14；越界项一律拒 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 窗口推导（**新增**） | `open_backtrack_windows(entries, table, phase_id)`：标记行枚举 → 区间判据 → 未闭环判据 → 创建序排序 → 上限裁剪；输出按创建序升序的 `Vec<BacktrackWindowEntry>` | `crates/core/workflow/src/write/phase_next.rs` | `crate::state::{PhaseStateRecord}`、`phase_table::{PhaseDefinition, MAX_BACKTRACK_WINDOWS}` | 既有纯函数（零 IO / 零 async / 零锁） |
| 响应填充（**修改**） | `PhaseNextOutcome` 增 `backtrack_window`；`build_phase_response` 填「覆盖 `def.id` 的开放窗口」、`error_outcome` 填「覆盖失败相位的开放窗口」、`done` 响应填 `Vec::new()`；三者共调同一推导 fn | 同上 | 上述推导 fn | 同上 |
| 窗口段渲染单点（**新增**） | `backtrack_window_section(windows) -> String`：标题 + 纯陈述说明 + 逐条 `<backtrack reason>` 定界的原因原文（只取 `reason`）；空集返回 `""` | 同上（经 `write/mod.rs` 再导出） | 无（`format!` + `join`） | 同上（零第二份文案） |
| S2 删除面（**修改**） | 删 `reason_suffix` 与 `build_phase_response` 的 `backtrack_reason` 形参；窗口段在原址拼接 | 同上 | — | — |
| 层级上限常量（**新增**） | `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2` | `crates/core/workflow/src/write/phase_table.rs`（常量区） | — | — |
| 出口面扩项（**修改**） | `pub use phase_next::{…, BacktrackWindowEntry, backtrack_window_section}` | `crates/core/workflow/src/write/mod.rs` | — | — |
| 决策有界输入（**修改**） | `DecisionInput` 增 `backtrack_window`（透传面，字段 doc 注明「walker 不自行推导」） | `crates/core/orchestration/src/decision.rs` | `workflow::write::{BacktrackOption, BacktrackWindowEntry}` | 既有纯数据结构 |
| 决策 prompt 组装（**修改**） | `decision_prompt` 在开头句与 `## 失败相位报告` 之间插入写面渲染单点的产物；空集整段省略 | `crates/core/orchestration/src/prompt.rs` | `workflow::write::backtrack_window_section` | 既有 `format!` 单串 |
| 决策输入透传（**修改**） | `build_decision_input` 增形参并原样透传；`resolve_deadlock` 调用点随行；`step_executor` doc 同步 | `crates/core/orchestration/src/walker.rs` | 上述类型 | 既有 step 循环（零新 port / 零新 step / 零新 route 规则） |
| 版本行（**修改**） | `version` `0.6.31 → 0.6.32` | `packages/desktop/package.json` | — | — |

<!-- 无新增 crate / 无新增模块目录 / 无新增文件：本变更全部落在既有文件的既有模块内（推导与渲染同驻 phase_next.rs，D15）。 -->

---

## 变更清单 (Change Inventory)

> 路径口径：仓库根相对全路径。**测试文件与用例编写归测试相位**；本清单只登记「结构体字面量补字段」这一类**编译闭合随动**（新增字段后不给全字段即 E0063，属实现相位的可编译义务，不新增任何用例）。

### 新增文件

**无。** 判据 + 渲染 + 结构体同驻 `phase_next.rs`（D15）：三者的输入是同两份数据（`entries` / `table`），新开文件只会把「谁负责裁剪」这个单点语义拆散，并把 `entries` / `table` 跨模块往返三次。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | ① import 区（`:8-11`）加 `MAX_BACKTRACK_WINDOWS`；② 新增 `pub struct BacktrackWindowEntry`（置于 `ResolvedPhaseSpec` 之后、`PhaseNextOutcome` 之前，五字段 + 逐字段 doc，derive `Debug, Clone, PartialEq, Eq`，**不 derive `Serialize`**）；③ `PhaseNextOutcome`（`:75-92`）在 `evaluator` 与 `allowed_backtrack_phases` 之间增 `pub backtrack_window: Vec<BacktrackWindowEntry>`（doc：正常响应对 `next_phase`、error 响应对失败相位，创建序升序，空集即无窗口）；④ 新增 `pub(crate) fn open_backtrack_windows`（置于 `latest_result` 之后、`fail_feedback_suffix` 之前）；⑤ 新增 `pub fn backtrack_window_section`（紧跟推导 fn）；⑥ `error_outcome`（`:239-256`）签名增 `entries: &[PhaseStateRecord]` / `table: &'static [PhaseDefinition]` 两形参（失败相位从 `error` 的 `phase` 字段单点取出，不另开形参），体内填 `backtrack_window: open_backtrack_windows(entries, table, phase)`；⑦ 两处调用点（`:194` / `:215`）传 `&entries` / `table`；⑧ `build_phase_response`（`:339-374`）签名删 `backtrack_reason: Option<&str>`、增 `entries: &[PhaseStateRecord]`，体内两行取代 `reason_suffix`（`:349-351` 整段删除）：`let backtrack_window = open_backtrack_windows(entries, table, def.id);` + `let window_section = backtrack_window_section(&backtrack_window);`（推导一次、渲染一次、响应字面量末尾 move 入结构体——零重复计算），拼接式（`:353-357`）改为 `format!("{context_header}{}{window_section}{feedback_suffix}", spec.prompt)`，响应字面量（`:360-373`）填 `backtrack_window`；⑨ 两处调用点（`:146-154` / `:226-234`）删实参 `entry.backtrack_reason.as_deref()` / `None`，补 `&entries`；⑩ `done` 响应（`:164-175`）补 `backtrack_window: Vec::new()`；⑪ doc 同步：`ResolvedPhaseSpec`（`:63-65`）「失败反馈段 / 回溯原因行」→「失败反馈段 / 回溯重做窗口段」、`build_phase_response`（`:336-338`）删「回溯原因后缀」改「回溯重做窗口段（静态主体之后、反馈段之前）」 | AC-1 / AC-2 / AC-3 / AC-4 / AC-5 / AC-6 / AC-7 / AC-8 / AC-9 / AC-13；`has_phase_passed` / `latest_entry` / `latest_phase_entry` / `latest_result` / `fail_feedback_suffix` / `SessionAnchors` / `phase_next` 的路由语义（`round` / `window_fails` / 幂等闸 / 两处决策分叉判据）**逐字不动** |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | 在 `MAX_REASON_CHARS`（`:46`）之后新增 `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2;` + doc（窗口层级上限：≤ 上限全量并集、超限取创建序末位两层；与 `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 同区；prompt 文案与测试断言引用本常量，零写死数字） | AC-4 / AC-9；`PhaseDefinition` / 两张相位表 / `backtrack_options` / `dependents` / 其余常量**逐字不动** |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | ① `pub use phase_next::{…}`（`:21-23`）追加 `BacktrackWindowEntry`；② 同处追加 `backtrack_window_section` 的再导出（供 orchestration 复用同一文案单点，零新模块声明） | AC-9；零新 `mod` 行、零新 `#[cfg(test)] mod` 行 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` | ① `use workflow::write::{BacktrackOption, BacktrackWindowEntry};`（`:3`）；② `DecisionInput`（`:31-49`）在 `candidates` 之后增 `pub backtrack_window: Vec<BacktrackWindowEntry>`（doc：透传自写面 error 响应；walker MUST NOT 自读 store、MUST NOT 自相位表推导；判据单源 = 写面「回溯重做窗口推导与 prompt 携带」） | AC-10；`DecisionAction` / `parse_decision` / `ensure_backtrack_allowed` / `MAX_REASON_CHARS = 500` / `CandidateReport` **逐字不动** |
| `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` | ① `use workflow::write::{backtrack_window_section, BacktrackOption};`（`:1`）；② `decision_prompt`（`:75-137`）体内 `let window_section = backtrack_window_section(&input.backtrack_window);`，`format!` 串（`:121-122`）在开头句与 `## 失败相位报告` 之间插 `{window_section}`；③ `decision_prompt` doc（`:72-74`）的有界输入清单补「回溯重做窗口」项 | AC-8 / AC-10；`evaluator_protocol` / `evaluator_prompt` / `verdict_fix_prompt` / `decision_fix_prompt` / `backtrack_option_artifacts` / `backtrack_option_head` / `backtrack_option_label` / `spec_sync_prompt` / `merge_conflict_prompt` **逐字不动** |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | ① import（`:8-13`）加 `BacktrackWindowEntry`；② `build_decision_input`（`:3550-3606`）增第四形参 `backtrack_window: &[BacktrackWindowEntry]`，`DecisionInput` 字面量（`:3600-3606`）填 `backtrack_window: backtrack_window.to_vec()`，doc 补该面来源；③ `resolve_deadlock` 调用点（`:3328`）传 `&next.backtrack_window`；④ `step_executor` doc（`:791-792`）「回溯原因后缀全在写面」→「回溯重做窗口段全在写面」 | AC-10 / AC-11；零新 port / 零新 step 词 / 零新 route 规则 / 零新通知类型；`drive` / `StepContext` / `PhaseCursor` / `run_worker` **逐字不动** |
| `packages/desktop/package.json` | `version` `0.6.31 → 0.6.32`（一次 patch）；`dependencies` / `devDependencies` / `scripts` **零变化** | AC-12；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零手改），`src-tauri/Cargo.toml` crate 版本行**不跟** |

### 编译闭合随动（测试文件，**不新增任何用例**）

> 依据：`PhaseNextOutcome` / `DecisionInput` 新增字段后，Rust **要求**结构体字面量给全字段（E0063）；这些点不给字段则 `cargo check --all-targets` 不通过（AC-12 的机械门面）。**只补字段、只求可编译**——不改断言值、不改用例名、不新增 / 删除用例（断言改写与新增用例归测试相位，见 proposal「测试文件」表）。

| 文件路径 | 落点 | 补什么 |
|----------|------|--------|
| `crates/core/orchestration/src/port_test.rs` | `sample_phase_next_outcome()`（`:27`） | `backtrack_window: Vec::new()` |
| `crates/core/orchestration/src/walker_test.rs` | `route_with_round`（`:931`）/ `done_outcome`（`:952`）/ `max_retries_outcome`（`:967`）/ `verdict_fail_outcome`（`:992`）/ `archive_route_outcome`（`:1770`） | 同上（5 处；`route_with_round` 建议留空集、决策两个产出装置亦留空集——装置语义保持「零窗口」基线，窗口透传用例归测试相位新增） |
| `crates/core/orchestration/src/prompt_test.rs` | `base_input()`（`:124`）/ 确定性用例内联构造（`:610`） | `backtrack_window: Vec::new()`（其余 19 处 `..base_input()` / `..base` spread 自动随动） |
| `crates/core/orchestration/src/decision_test.rs` | `decision_input_fail_report字段面` 的 `base`（`:361`） | 同上 |

### 公共函数 / API

> 只列模块级导出（`pub` / `pub(crate)`）。私有辅助函数（`build_phase_response` / `error_outcome` / `build_decision_input`）的签名变更与 `reason_suffix` 的退役落点已逐条写在「修改文件」表内，此处不重复登记。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `open_backtrack_windows` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | 新增（`pub(crate)`） | `fn open_backtrack_windows(entries: &[PhaseStateRecord], table: &[PhaseDefinition], phase_id: &str) -> Vec<BacktrackWindowEntry>` | 单点推导：候选标记行 = `backtrack_to` trim 后非空；区间判据 `to_index ≤ col(phase_id) ≤ from_index`；未闭环判据 = 发起相位无 `id > 标记行 id` 的条目；按标记行 `id` 升序；`skip(len.saturating_sub(MAX_BACKTRACK_WINDOWS))`（≤ 上限全量并集、超限取创建序末位两层）；标记行 `id` 是中间元组排序键，不落结构体。`phase_id` 不在表内 → 空集。`pub(crate)` 而非 `pub`：跨 crate 消费方只需渲染 fn（事实校准 2 / 待决问题 6） |
| `backtrack_window_section` | 同上（经 `write/mod.rs` 再导出） | 新增（`pub`） | `fn backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String` | 文案与渲染单点。空集 → `""`；非空 → 前导 `\n\n` + `## 回溯重做窗口（背景信息）` + 说明段 + 逐条 `<backtrack reason>\n{reason}\n</backtrack>`（块间 `\n\n`），尾随零换行。**只取 `reason`**，零属性、零相位 id、不截断不压平不 trim |
| `phase_next` | 同上 | 修改（体内；签名不变） | `fn phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | 三处 `backtrack_window` 填充（done 空集 / error 覆盖失败相位 / 正常覆盖 `next_phase`）；路由语义（round 上限、backtrack 分支、两处决策分叉、幂等闸、正常路由）逐字不变 |
| `decision_prompt` | `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` | 修改（体内；签名不变） | `fn decision_prompt(input: &DecisionInput) -> String` | 段序：开头句 → （窗口段）→ `## 失败相位报告` → 失败清单 → 白名单 → 候选报告 → 报告文件 → 决策协议；空集整段省略（零残留标题） |

<!-- 无 CLI 子命令、无 HTTP 端点、无 IPC 命令增删：本变更的「出线面」只有既有结构体字段（见「类型定义」）与既有函数体内变化。私有函数（build_phase_response / error_outcome / build_decision_input / 退役的 reason_suffix）按「私有函数不列」纪律不出本表，落点见「修改文件」表。 -->

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `BacktrackWindowEntry`（struct） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | 新增 | `pub from_phase: String`（发起相位，区间上界；**不进 prompt**）/ `pub from_index: usize`（其相位表下标）/ `pub to_phase: String`（目标相位，区间下界；**不进 prompt**）/ `pub to_index: usize` / `pub reason: String`（`backtrack_reason` 原文逐字）。derive `Debug, Clone, PartialEq, Eq`；**不 derive `Serialize`**（不出 bindings，零 TS 联动） |
| `PhaseNextOutcome`（struct） | 同上 | 修改 | 增 `pub backtrack_window: Vec<BacktrackWindowEntry>`（`Vec` 非 `Option`：空集即「无窗口」；`Default` 语义 = 整段省略）。其余 10 字段（`done` / `next_phase` / `round` / `steps` / `artifact_manifest` / `executor` / `evaluator` / `allowed_backtrack_phases` / `last_result` / `error`）类型与语义不变 |
| `DecisionInput`（struct） | `packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs` | 修改 | 增 `pub backtrack_window: Vec<workflow::write::BacktrackWindowEntry>`（第六面，置于 `candidates` 之后）。既有五字段（含 `fail_report` 有界 ≤2000）语义不变 |
| `PhaseNextError` / `LastResult` / `ResolvedPhaseSpec` / `BacktrackOption` / `CandidateReport` / `DecisionAction` | 同上 | **零改动**（登记防误改） | 封闭集三动作、白名单项三元、失败快照四字段、角色 spec 两字段全部不变（AC-11 的反向钉住面） |

### 配置

<!-- 无配置项：本变更零 KNOWN_KEYS 增删、零 TestFramework 变体、零 openspec/config.json 改动、零 crates/core/config/config.schema.json 镜像改动。package.json 的 version 是版本纪律项，已登记在「修改文件」表。 -->

---

## 数据模型 (Data Model)

### 回溯窗口（纯读面派生，**零持久化载体**）

| 字段 | 类型 | 取值来源 | 说明 |
|------|------|----------|------|
| `from_phase` | `String` | 标记行自身的 `PhaseStateRecord.phase` | 发起相位（泳道位置靠后）= 区间上界。**只服务判据 / 排序 / 裁剪 / 测试断言，不渲染** |
| `from_index` | `usize` | `table.iter().position(\|def\| def.id == from_phase)` | 上界的泳道下标（与 `detail.pipeline` 下标、前端 `colIndex` 同源） |
| `to_phase` | `String` | 标记行 `backtrack_to`（`trim` 后） | 目标相位（泳道位置靠前）= 区间下界。不渲染 |
| `to_index` | `usize` | 同上相位在表内的下标 | 下界的泳道下标；自环时等于 `from_index` |
| `reason` | `String` | 标记行 `backtrack_reason` 原文（`None` → `""`） | **prompt 唯一渲染字段**；逐字保留（不 trim / 不截断 / 不压平） |

### 派生判据（**可执行伪码**）

排序键 `order = m.id` **只活在推导过程中的中间元组里**（`BacktrackWindowEntry` 五字段不含 `order`——窗口记录形状的 `order` 由标记行 `id` 即时给出，不落结构体，见 spec「窗口记录零持久化载体」）：

```
候选标记行 M = { m ∈ entries | m.backtrack_to.trim() 非空 }
  to    = m.backtrack_to.trim()
  from  = m.phase
  order = m.id
  reason = m.backtrack_reason.clone().unwrap_or_default()
  （from 或 to 不在 table 内 → 跳过该候选，防御式）

filter(open)      : ¬∃ e ∈ entries · e.phase == from ∧ e.id > m.id
filter(connect)   : to_index ≤ col(phase_id) ≤ from_index        col(P) = table 内 position
sort(创建序升序)  : 按中间元组的 order（= m.id）升序（id = 落账序，写事务内 max+1；标记行互为不同 id，故排序稳定且无并列）
take_last(MAX)    : skip(len.saturating_sub(MAX_BACKTRACK_WINDOWS))   ← D4 的双分支等价式
投影              : 中间元组 → BacktrackWindowEntry { from_phase, from_index, to_phase, to_index, reason }
```

- **窗口关闭靠「未闭环判据」而非清理**：标记行不是新行，而是改写「发起相位最新条目」的 `backtrack_to` / `backtrack_reason`（`store.rs:1814-1815`）；发起相位重跑会新增一行（`backtrack_to = None`），旧标记行**永久留在库中**，被 `filter(open)` 关掉。这就是「零新表 / 零新列 / 零新时钟」的全部依据。
- **创建序单调性**：发起相位恒为「刚失败的相位」（`resolve_deadlock` 的 `fail_phase` 来自 `PhaseNextError`），其 fail 条目是决策前最后一次落账，决策先于任何其它相位运行 ⇒ 每次回溯的标记行 `id` 都是当时全库最新，多个标记天然按决策先后递增（explore 4.6）。
- **两层「本轮」口径（有意不同，MUST NOT 混用）**：预算层 `round = (entries.len() - anchor) + 1`、`window_fails = entries[anchor..]` 计数（按 run 窗口，重启换新预算，**逐字不动**）；背景层按 db 事件序**全量 `entries`**（跨 run 直到闭环）。把背景层写成读 `anchor` 会把跨 run 未闭环窗口误判为已闭环。
- **结构性性质（裁剪方向的可断言形式）**：同时开放的多窗口按创建序排列时 `from_index` **严格递减**（要创建 `from=F_j` 的新窗口，流程必须先跑到 `F_j`，而这要求更早的窗口都已闭环）⇒ 创建序最早那条恒为 `from_index` **最大**者，也正是 `take_last` 首先丢弃的那条。

### 渲染产物形状（prompt 片段，**非 JSON、非落库**）

非空时按下列顺序拼接（转义序列为真实字符；`reason` 逐字替换）：

```
"\n\n## 回溯重做窗口（背景信息）\n\n"
+ "本 change 发生过下列相位回退，且均尚未回到闭环。这些是此前回溯留下的、需要解决的问题——原因原文按发生先后排列（最上面最早），保留原始换行与标点，未做截断或改写。它们是背景，不构成额外验收条件。"
+ "\n\n<backtrack reason>\n" + reason_1 + "\n</backtrack>"
+ "\n\n<backtrack reason>\n" + reason_2 + "\n</backtrack>"
```

空集时为空串。**体积界**：`MAX_BACKTRACK_WINDOWS` ×（标签与说明行 + `MAX_REASON_CHARS`）= 2 × (≈60 + 1000) ≈ ≤2.1K 字符（相对现状「仅目标相位一行 ≤1K」的最坏增量 ≈ +1.1K/相位；实现 MUST NOT 再加截断）。

### 持久化（**本变更零落库语义**）

| 项 | 现状 | 本变更 |
|----|------|--------|
| `PhaseRecord` / `PhaseStateRecord` 字段面 | `id` / `phase` / `backtrack_to` / `backtrack_reason` / `timestamp` 等 | **零改动**：窗口四元 `{from, to, reason, order}` 全部从既有字段直接读出 |
| `apply_change_backtrack` 写事务 | 改写发起相位最新条目 + 目标 pass 置 stale + 闭包传播 | **零改动**（`store.rs` 零 diff） |
| schema / 迁移 / 索引 / 查询面 | — | **零改动**：无新表、无新列、无窗口号字段、无决策时刻列；`list_phase_records` 的 id 升序语义即创建序来源 |
| 内存态（`Vec<BacktrackWindowEntry>`） | — | **纯响应载荷**：随 `PhaseNextOutcome` 在进程内流转，不落库、不出 IPC、不出 bindings |

<!-- 路由 / API 设计：不涉及 —— 本变更零 IPC 命令增删、零 HTTP 端点、零 CLI 子命令、零 bindings 增量；唯一「界面」是既有 Rust 结构体字段（`PhaseNextOutcome.backtrack_window` / `DecisionInput.backtrack_window`），二者均不出 `src/types/generated/**`，`bindings:export` 应零 diff。 -->

---

## 依赖 (Dependencies)

### 运行时依赖（**零新增**）

| 依赖 | 版本（既有） | 用途 |
|------|--------------|------|
| `agent`（workspace crate） | 既有 | `ModelLevel`（`ResolvedPhaseSpec.model_level` 既有面，零改动） |
| `crate::state`（workflow 内部） | 既有 | `PhaseStateRecord`（窗口派生的输入：`id` / `phase` / `backtrack_to` / `backtrack_reason`） |
| `crate::write::phase_table` | 既有 | `PhaseDefinition`（泳道序单源）+ `MAX_BACKTRACK_WINDOWS`（上限单点） |
| `workflow`（orchestration 的 workspace 依赖） | 既有 | 决策面消费 `BacktrackWindowEntry` 与 `backtrack_window_section`（同一文案单点） |

### 构建 / 测试依赖（**零新增**）

| 依赖 / 工具 | 用途 |
|-------------|------|
| `cargo clippy --all-targets -D warnings`（经 `server:check`） | 新增私有 fn 的未用告警 / 签名变更的编译闭合（AC-12） |
| `cargo test` / nextest（既有基线 7429 条） | 既有断言零回归（S2 断言改写归测试相位） |
| `bindings:export`（`crates/tools/export-bindings`） | `PhaseNextOutcome` / `DecisionInput` 不出 bindings ⇒ 再生成应零 diff（AC-12） |

**无 `Cargo.lock` 变化**（零依赖增删）；**无前端依赖变化**（`src/**` 与 `src/types/generated/**` 零 diff，AC-14）。

---

## 验收标准回应

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 窗口内每个相位每个 agent 步都带背景 | D1（写面派生 + 响应下发）+ D2（区间判据含上界 `P = from`）+ `build_phase_response` 对 `def.executor` / `def.evaluator` 共用同一个 `resolve` 闭包（同一份 `window_section`）⇒ 两角色同等；窗口外相位 `open_backtrack_windows` 返回空集 ⇒ 零段。evaluator 的协议附录由 `evaluator_prompt` 在外层 append（`walker.rs:1542`，零改动），故窗口段仍在附录之前（D8 末段） |
| AC-2 区间判据单源 / 自环零特判 | D2（`col` = 相位表 position，三处同源）+ D6（`to_index ≤ col ≤ from_index` 在 `to == from` 时退化为单点，源码零 `if to == from`） |
| AC-3 未闭环判据不读 run 状态 | D7（推导 fn 入参只有 `entries` / `table` / `phase_id`，体内零 `SessionAnchors` / `run_id` / `anchor`）+ D2 的 `open` 定义 |
| AC-4 A2 并集与上限双分支 | D2（升序）+ D4（`saturating_sub` 等价式 + 等价链「创建序最早 ≡ take_last 先丢 ≡ `from_index` 最大」）+ D13③（零「另有 N 条」元信息） |
| AC-5 求值顺序固定 | D5（四步固定序，显式 `sort_by_key(|m| m.id)` 不依赖 `entries` 的输入顺序） |
| AC-6 单点位置与响应填充 | D1 + 「修改文件」表的 ⑥⑦⑧⑨⑩（`error_outcome` 填失败相位窗口、`build_phase_response` 填 `next_phase` 窗口、done 填空集，三者共调同一推导 fn） |
| AC-7 S2 取代原因行 | D11（删 `reason_suffix` 与 `backtrack_reason` 形参、两分支入参收敛）+ D8（窗口段在原址拼接，`feedback_suffix` 之前）+ 「公共函数 / API」表的退役行 |
| AC-8 渲染零元信息 / 原文逐字 | D8（字节契约）+ D13（只取 `reason`、标签零属性、不截断不压平不 trim）+ D12（命名不引入泳道位置的读法） |
| AC-9 文案与常量单点 | D12 + 「架构组件」表的「出口面扩项」行（`write/mod.rs` 再导出）+ 事实校准 3（`MAX_BACKTRACK_WINDOWS` 为 `pub(crate)` ⇒ orchestration 取不到，机械上无法写第二份裁剪） |
| AC-10 决策面携带窗口 | D9（`DecisionInput` 字段 + 透传形参、不传 `&PhaseNextOutcome`）+ D10（段位置 + 零副本 + 空集整段省略）+ 「修改文件」表的 `decision.rs` / `prompt.rs` / `walker.rs` 三行 |
| AC-11 零结构扩散 | 「本篇边界」段的零 diff 面 + D16 + 「数据模型 - 持久化」表（零 schema / 零迁移 / 零列）+ 事实校准 6（`TryFrom` 是拷贝解构，无需改动） |
| AC-12 交付面 | 「依赖」节（零新增依赖）+ 「修改文件」表的 `package.json` 行（`0.6.31 → 0.6.32`，`Cargo.toml` 不跟）+ 「编译闭合随动」表（9 处字面量，编译器兜住）。**口径分层**：本相位（实现）只保证**编译绿 + clippy 零新告警**；`cargo test` 全绿是**交付态判据**（AC 原文），其达成点在测试相位改写 `phase_next_test.rs` 的 4 处 S2 断言并补新用例之后（事实校准 8）——`cargo test` 的 7429 条基线零回归由 test-execution 相位判定，本相位 MUST NOT 为让红转绿而回退 S2 或改断言 |
| AC-13 体积有界 | 「数据模型 - 渲染产物形状」的体积界 + D13②（原因不截断）+ D16⑧（渲染路径零 `truncate` / `chars().take`） |
| AC-14 明确不做项零触碰 | 「本篇边界」段（`prompt.rs` 的 `spec_sync_prompt` / `merge_conflict_prompt`、`graph.ts`、`src/**`、`src/types/generated/**` 零 diff）+ D16④⑥ |
| AC-15 spec 双分支如实写清 | 两份 delta spec 已定稿（write-face「回溯重做窗口推导与 prompt 携带」与 orchestration「决策协议」各写一处双分支）；本设计 D4 提供实现侧的唯一等价链，方向固定为「裁掉 `from_index` **最大**那条 / 出线的是 `from_index` 最小的两条」 |

---

## 待决问题 (Open Questions)

1. **legacy 标记行缺 `reason` 的渲染形态（已知、生产不可达）**：`store.apply_change_backtrack` 恒写 `Some(reason)`（`store.rs:1815`）且 `store` 零改动，故生产路径不会出现「有 `backtrack_to` 无 `backtrack_reason`」的行；该形态只可能来自旧 `phase_log` 机制的兼容行。本设计按 D14 让判据不看 `reason`、渲染产出空正文块（`<backtrack reason>\n\n</backtrack>`）。若评审认为「空块污染 prompt」，退路是给判据加第四条「`reason` 非空白」并同步改 spec 三条判据为四条——**一次裁决，不在实现期暗改**。
2. **原因原文含字面 `</backtrack>` 会提前截断该条记录（已知、不设防线）**：转义等于改写原文，与 D13②「不压平」直接冲突；属决策 JSON 自由文本，概率可忽略。若日后实测出现，改法 = 转义 + 同步改「不压平」措辞（R-3）。
3. **深层嵌套不提示裁剪（已知、接受）**：命中 ≥ 上限时 agent 会形成「历史上只回溯过这两次」的认知，与库中真实记录不符；偏差限定在 ≥3 层未闭环的异常路径（正常路径 0/1/2 层零偏差）。改法已预埋：只改窗口段**标题**一处措辞（`## 回溯重做窗口（背景信息，仅显示最近 2 次）`），不动结构、判据与渲染形状（R-1）。
4. **决策面防重复回跳降级为语义级（已知、接受）**：窗口段零相位 id ⇒ 决策 agent 只能靠原因文本语义理解「这个方向试过」，MUST NOT 宣称与结构判据等价；弱化后仍显著优于现状（现状决策 prompt 零历史回溯信息），深层重复回跳由 `MAX_ROUNDS = 20` 兜底。升级路径已预埋：结构体里 `from_phase` / `to_phase` 一直在，只改渲染 fn 一处即可加回（R-8）。
5. **`MAX_BACKTRACK_WINDOWS` 的取值可调性**：本期定 2；若日后实测提示体积 / 信息量需调整，只改 `phase_table.rs` 一处常量（prompt 文案与 orchestration 侧零写死数字，事实校准 3 保证 orchestration 侧连引用都不可能）。MUST NOT 在窗口渲染路径引入「超出部分折叠」类第二形态。
6. **`open_backtrack_windows` 是否升为 `pub`（登记防误改）**：本期 `pub(crate)`（事实校准 2）。若日后图面或查询面需要直接消费窗口集合（例如在 flow 视图标记「同一轮重做」），正确做法是把该 fn 升为 `pub` 并同步 `write/mod.rs` 出口——那是一次对外 API 扩张，须单独评审，MUST NOT 在本变更或后续变更里靠「顺手 `pub`」实现。
