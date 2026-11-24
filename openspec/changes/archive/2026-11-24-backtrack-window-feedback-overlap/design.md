# 设计: backtrack-window-feedback-overlap

> **变更**: backtrack-window-feedback-overlap（回溯窗口段 × 重试反馈段的同源重叠）
> **日期**: 2026-11-24
> **基于**: `proposal.md`（问题五节 + 提案六节 + 能力十二份 + AC-1…AC-12 + R-1…R-8）、`explore.md`（只读输入，§11 裁决基线）、`specs/` 下 2 份 delta（`desktop-workflow-write-face` / `desktop-change-orchestration`）
> **性质**: 相位 prompt 组装面的**一处条件抑制**——当前相位是某开放窗口的发起相位（同源格）时，失败反馈段整段不渲染。零新字段 / 零 schema / 零 store 改动 / 零 IPC / 零 bindings / 零前端 / 零状态迁移；`PhaseNextOutcome` 字段面与路由判定逐字不变。
> **不含**: 全局单通道（「只留窗口段」）、区间上界开区间、`reason` 信息分工、文案澄清；决策 prompt 的 `report` × `checklist` × 候选报告三重面去重；`latest_phase_entry` 双算 / `stale` 过滤 / report 上限双数字 / `MAX_REASON_CHARS` 双值四项顺带清理（proposal R-8，登记待议）。

---

## 起点实况（`HEAD` 基线实测）

| 面 | 实测（文件:行） |
|---|---|
| 判据输入 | `phase_next.rs:362` `open_backtrack_windows`：区间 `to_index ≤ col(P) ≤ from_index`（含上界 `P = from`）/ 未闭环（发起相位无 `id` 更大的条目）/ 按标记行 `id` 升序 `take_last(MAX_BACKTRACK_WINDOWS)`；入参不含 `run_id` / `SessionAnchors` |
| 组装序 | `:465` `build_phase_response`：`:475` `open_windows` → `:476` `window_section` → `:477` `feedback_suffix` → `:479-482` 单点插值 `format!("{context_header}{}{window_section}{feedback_suffix}", spec.prompt)`（双角色共用同一闭包） |
| 反馈段消费者 | `:251-260` `retry_feedback = latest_phase_entry(&entries, next.id).filter(verdict == Fail)`（`Option<&PhaseStateRecord>`）作为第七个实参传入 `:472`；`:440` `fail_feedback_suffix` 是唯一渲染点（`⚠️ 上次评估未通过（attempt N）` 在生产源码中只在 `:457` 出现，其余命中为测试断言与假写面自构 prompt） |
| 回溯分支 | `:168` 回溯分支调用 `build_phase_response(…, None)`——目标相位第一拍 `retry_feedback` 恒 `None`，与同源抑制无交集 |
| 落账改写信面 | `store.rs:1856` `apply_change_backtrack`：`:1876` `max_by_key(\|record\| (record.timestamp, record.id))` 取发起相位最新条目改写入 `backtrack_to` / `backtrack_reason`（**标记行 = 该 fail 条目本身**，`reason` 与 `checklist` / `report` 同体） |
| 最新条目口径 | `latest_phase_entry`（`:321`）按 `timestamp` 严格大于替换（并列保留先见者）⇒ 同相位内 `timestamp` 与 `id` 同序时，最新条目就是标记行 |
| 常量 | `phase_table.rs:52` `MAX_BACKTRACK_WINDOWS = 2`、`:46` `MAX_REASON_CHARS = 1000`（本变更零新常量） |
| 出口面 | `write/mod.rs:21-23` 再导出 `backtrack_window_section` / `phase_next` / `BacktrackWindowEntry` / `LastResult` / `PhaseNextError` 等；`build_phase_response` 与 `fail_feedback_suffix` 均**私有** ⇒ 公共面零变化 |
| 生成物 | `packages/desktop/src/types/generated/bindings.ts` 对 `PhaseNextOutcome` / `BacktrackWindowEntry` 零命中；`.dev-team/architecture/models/**` 无 workflow 元素 ⇒ `bindings:export` / `api:export` / `api:check` 预期零 diff |
| 既有断言面 | `phase_next_test.rs:2406`（上界源头相位，**只断窗口段在场**）/ `:2374`（区间内部相位 + 已决 fail）/ `:2932`（段序，异源格）/ `:3043`（回溯与正常分支同形）/ `:3105`（形态三：无窗口 fail 重试）/ `:2347`（下界，回溯分支）/ `:2511`（自环直调）/ `:3628`（窗口段体积界） |
| 版本 | `packages/desktop/package.json:3` = `0.7.9`；`src-tauri/tauri.conf.json:4` = `"../package.json"`；`src-tauri/Cargo.toml:6` = `0.1.0`（自立版本行） |
| 工作树 | `git status --porcelain` 只有本 change 目录未跟踪（实现面零预置改动） |

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 判据形态与落点 | 在 `build_phase_response` 内、`open_windows` 算出之后新增 `let same_origin = open_windows.iter().any(\|window\| window.from_phase == def.id);`，即 `feedback_suffix` 的装配处把 `retry_feedback` 经 `.filter(\|_\| !same_origin)` 后才进 `fail_feedback_suffix`。**判据只此一处**：MUST NOT 比较记录 `id` / `timestamp` 自建同源判定，MUST NOT 在 `phase_next` 调用侧二次推导 | `open_windows` 已在该函数内算出（`phase_next` 与 error 分支共用同一推导函数）；`from_phase == def.id` 直接读既有字段，零新字段、零结构体改造。消费侧再比记录身份＝第二份真值（AC-2 / R-2） |
| D2 | 抑制只发生在渲染层 | `retry_feedback` 形参与 `fail_feedback_suffix` 渲染函数**保留**；`PhaseNextOutcome` 字段面、`backtrack_window` 取值、`error_outcome` / `done` 响应、`phase_next` 的两处 `latest_phase_entry` 与两道分叉判断、白名单推导全部逐字不变 | 窗口外格（决策 stop / 决策缺席 / 越权拒 / `backtrack` 写面 Err 后重启）仍是 `fail_feedback_suffix` 的消费者，删它即死代码 + 恢复路径失明（AC-4 / AC-8 / R-1） |
| D3 | 完备性不新增 marker 字段 | 依赖两条既有不变式：① 标记行恒为发起相位 `id` 最大的条目（`open_backtrack_windows` 的 open 判据）；② 同源格下该标记行即发起相位的最新条目（`apply_change_backtrack` 按 `(timestamp, id)` 取最大改写信面、`latest_phase_entry` 按 `timestamp` 取首）。残差如实登记：同相位同毫秒双条目下被抑制的反馈可能来自非标记的旧 fail——影响仅少渲染一段冗余证据（窗口段仍在、路由不变），MUST NOT 为它另立第二判据 | 零 schema / 零迁移是本变更的可行性前提；为极小概率另立判据会重新制造 R-2 |
| D4 | 与层级裁剪相容 | 抑制判据读**裁剪后**的 `open_windows`，不做任何额外兜底：同源窗口恒为「覆盖该相位的开放窗口」中创建序最末者（其 `from_index` = 当前相位下标 ≤ 任一覆盖者；该相位重跑落新条目即闭环 ⇒ 无更晚创建者），`take_last` 保留的正是创建序末位 | 由 AC-6 用例（`MAX_BACKTRACK_WINDOWS + 1` 个窗口、同源窗口创建序最末）直接钉住；不在代码里加判据（R-4） |
| D5 | 文档面 | 只改两处：`build_phase_response` 函数 doc（失败反馈段 → 「fail 重试且非同源格时在场」）与 `fail_feedback_suffix` doc（补一句「窗口外格消费者；同源格由 `build_phase_response` 整段省略，本函数零判据」）；另在条件处留一行代码注释说明「两段读同一条记录，只留窗口段」。`ResolvedPhaseSpec` / `PhaseNextOutcome` / `BacktrackWindowEntry` 的 doc 一律不动 | proposal 授权面恰为两处 doc；`ResolvedPhaseSpec` 的「失败反馈段 / 回溯重做窗口段（append）」陈述仍成立（仍是 append，只是条件在场），改了会让 diff 超出预期面（AC-11） |
| D6 | 三态口径不合并成因 | 「同源抑制」与「`retry_feedback = None`」是两条不同成因，MUST NOT 合并实现（不加 `if same_origin \|\| retry_feedback.is_none()` 之类的合并分支，不把回溯分支改成传某些哨兵值）；「仅窗口段且反馈段本不在场」的常态（窗口内已通过相位的重跑、回溯后第一拍）不计入并存形态 | 「两段的并存形态」才是三态穷尽的范围；合并会把「本来就没有反馈」与「有但抑制」压在一条判据上，日后无法分辨（spec「抑制只发生在渲染层」条款） |
| D7 | 版本 | `packages/desktop/package.json` `0.7.9` → `0.7.10`；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随（文件零 diff）；`src-tauri/Cargo.toml` 的 `version = "0.1.0"` 逐字不变 | AGENT.md 版本纪律（AC-10） |
| D8 | 改动集白名单 | 允许出 diff：本 change 目录文稿 / `specs/**`、`crates/core/workflow/src/write/phase_next.rs`、`packages/desktop/package.json`。测试面（`phase_next_test.rs`）由**测试相位**承接，本相位不改、不跑；`crates/core/orchestration/**`、`crates/infra/store/**`、`crates/core/workflow/src/prompts/**`、`packages/desktop/src/**`、`src/types/generated/**`、`openspec/specs/**`、`openspec/changes/archive/**`、`AGENT.md` / `CLAUDE.md`、`src-tauri/Cargo.toml` 零 diff | 出现白名单外 diff 即回退（AC-11） |
| D9 | 明确不做 | 全局删反馈段 / 区间上界开区间 / `reason` 信息分工 / 文案澄清 / 决策面 `report` × `checklist` × 候选报告去重 / `latest_phase_entry` 双算与 `stale` 过滤等四项顺带清理，全部零触碰 | proposal「明确不做」与 explore §6 / §11；后两类收益更大但独立成变更（R-6 / R-8） |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| prompt 组装面（**改**） | 同源抑制单点：`open_windows` 之后求 `same_origin`，`retry_feedback` 经 `Option::filter` 后才进反馈段渲染 | `crates/core/workflow/src/write/phase_next.rs`（`build_phase_response`） | 本 crate 内既有 `open_backtrack_windows` / `fail_feedback_suffix` | `Option::filter` + 既有 `format!` 单点插值 |
| 窗口推导（零改） | 区间 / 未闭环 / 按创建序 `take_last` 三判据，输出即 `backtrack_window` 与窗口段输入 | 同上（`open_backtrack_windows`） | `PhaseStateRecord` 全量条目 + 静态相位表 | 纯函数，零 IO、零 run 状态 |
| 渲染单点（零改） | 窗口段（只取 `reason`，标签行零属性）/ 失败反馈段（失败项优先、清单空退回 `report`、`（attempt N）` 头） | 同上（`backtrack_window_section` / `fail_feedback_suffix`） | — | 纯字符串拼装 |
| 路由判定（零改） | `backtrack_to` 分支 / 全 pass `done` / 重试上限 / `VerdictFail` 分叉 / `fail_decided` 幂等闸 | 同上（`phase_next` / `error_outcome`） | `ChangeStateStore` / `SessionAnchors` | 只读状态机（不改库） |
| 编排消费面（零改） | 原样下发写面 prompt；零依据 `backtrack_window` 的抑制 / 补回分支 | `crates/core/orchestration/src/{walker.rs,prompt.rs}` | `PhaseNextOutcome` | 零分支数据面消费 |
| 决策面（零改） | 决策 prompt 仍携 fail checklist / `## 失败相位报告` / 窗口段三面 | `crates/core/orchestration/src/decision.rs` / `prompt.rs` | `DecisionInput` | 另一渲染面，不触达本抑制 |
| 落账载体（零改） | 标记改写信面（`(timestamp, id)` 最大条目）——同源是**读面派生**，写面裁定不变 | `crates/infra/store/src/store.rs`（`apply_change_backtrack`） | 既有 `PhaseRecord` 表 | 单事务读改写（既有） |

依赖方向零新增边：改动全在 `core/workflow` 内，`infra → core` 不变，零新 crate 依赖。

### 数据流（改后）

```text
PhaseStateRecord 全量条目 + 静态相位表
        │
        ├─ open_backtrack_windows(entries, table, def.id)      ← 零改动（区间 / 未闭环 / take_last）
        │        ⇒ open_windows: Vec<BacktrackWindowEntry>
        │             ├─ any(from_phase == def.id) ⇒ same_origin          ← 新增（判据单点）
        │             ├─ backtrack_window_section(&open_windows)          ← 零改动
        │             └─ PhaseNextOutcome.backtrack_window = open_windows  ← 零改动（不因抑制收缩）
        │
        └─ retry_feedback = latest_phase_entry(entries, next.id) filter(Fail)   ← 零改动
                 └─ .filter(|_| !same_origin)  ── 同源格从此处截断
                        └─ fail_feedback_suffix(record)                    ← 零改动（窗口外格仍消费）

prompt = change: <name> + 静态角色主体 + window_section + feedback_suffix
         同源格：feedback_suffix = ""（窗口段仍在）
         异源并存格 / 窗口外格：两段 / 仅反馈段，与改动前逐字节相同
```

---

## 变更清单 (Change Inventory)

> **覆盖口径**：proposal「变更范围 - 实现文件」**2 行逐行覆盖**（`phase_next.rs` / `package.json`）。proposal「测试文件」表的用例面由测试相位承接——不进本清单（进清单即要求 `tasks.md` 有对应任务，与「本相位不写测试任务」互斥），本设计只在「验收标准回应」里点名其守的 AC 与需补的断言方向（`phase_next_test.rs:2406` 现只断窗口段在场，改动后仍绿，属 R-5 的守护力缺口）。
> **随动面（测试相位执行，本相位不改）**：`phase_next_test.rs` 的 `回溯窗口四态_上界源头相位命中` 需补「零 `⚠️ 上次评估未通过` + 标记行 `report` / `checklist` 正文零命中」；`回溯窗口四态_区间内部相位双角色携背景` 需补「反馈段仍在」；`窗口段位置先于失败反馈段` / `零窗口回归_既有种子窗口空集且prompt零段` / 自环 / 跨 run / 两分支同形 / 体积界用例逐字不动。

<!-- 新增文件（New Files）：本变更零新增文件——抑制落在既有单点 `build_phase_response` 内，零新模块、零新类型、零新常量；测试面无新建文件（用例落在既有 `phase_next_test.rs`）。 -->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/workflow/src/write/phase_next.rs` | ① `build_phase_response`（`:465`）内，`let open_windows = …`（`:475`）之后插入 `let same_origin = open_windows.iter().any(\|window\| window.from_phase == def.id);`（含一行注释：窗口段与反馈段读同一条标记行，只留窗口段）；② `:477` `let feedback_suffix = retry_feedback.filter(\|_\| !same_origin).map(fail_feedback_suffix).unwrap_or_default();`；③ `build_phase_response` 的 doc（`:462-464`）把「失败反馈段（fail 重试时在场）」改为「（fail 重试且非同源格时在场——同源格整段省略）」；④ `fail_feedback_suffix` 的 doc（`:437-439`）补一句「消费者为窗口内非同源格的 fail 重试与窗口外格的 fail 重启；同源格由 `build_phase_response` 整段省略，本函数零判据」 | 签名逐字不变：`fn build_phase_response(table: &'static [PhaseDefinition], def: &PhaseDefinition, round: u32, name: &str, entries: &[PhaseStateRecord], last_result: Option<LastResult>, retry_feedback: Option<&PhaseStateRecord>) -> PhaseNextOutcome`；`open_backtrack_windows`（`:362`）/ `backtrack_window_section`（`:419`）/ `fail_feedback_suffix` 函数体 / `phase_next`（`:119`）/ `error_outcome`（`:269`）/ `latest_phase_entry`（`:321`）零 diff；净增代码 ≤ 4 行（不含 doc 与注释） |
| `packages/desktop/package.json` | `"version"`: `0.7.9` → `0.7.10` | D7 / AC-10；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`（零 diff），`src-tauri/Cargo.toml` 的 `version = "0.1.0"` 逐字不变 |

<!-- 配置（Config）：本变更零配置键改动——不新增 `KNOWN_KEYS` 条目、不动 `crates/core/config/config.schema.json` 镜像、不新增 `.dev-team/config.json` 的 `exclude` / 锁面规则；抑制判据是写面读面派生，不是配置面。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `phase_next` | `crates/core/workflow/src/write/phase_next.rs` | 零改动 | `pub fn phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | 两处 `latest_phase_entry`（`:214` `latest_fail` / `:251` `retry_feedback`）、两道分叉判断、`backtrack_to` 分支与 `done` / `Err` 出口逐字不变（AC-2 / AC-8） |
| `open_backtrack_windows` | 同上 | 零改动 | `pub(crate) fn open_backtrack_windows(entries: &[PhaseStateRecord], table: &[PhaseDefinition], phase_id: &str) -> Vec<BacktrackWindowEntry>` | 同源判据读其输出，不读其内部；三判据与裁剪逐字不动（AC-6 的裁剪相容性由此函数语义保证） |
| `backtrack_window_section` | 同上 | 零改动 | `pub fn backtrack_window_section(windows: &[BacktrackWindowEntry]) -> String` | 文案与「只取 `reason`、标签行零属性」的渲染纪律不变；同源格下窗口段照常在场 |
| `build_phase_response`（私有） | 同上 | 改（内部新增一处条件，签名不变） | `fn build_phase_response(table: &'static [PhaseDefinition], def: &PhaseDefinition, round: u32, name: &str, entries: &[PhaseStateRecord], last_result: Option<LastResult>, retry_feedback: Option<&PhaseStateRecord>) -> PhaseNextOutcome` | 私有函数，列此仅为钉住签名与调用面不变；唯一行为差异在 `executor` / `evaluator` 的 `prompt` 文本（同源格少一段） |
| `fail_feedback_suffix`（私有） | 同上 | 零改动（仅 doc） | `fn fail_feedback_suffix(record: &PhaseStateRecord) -> String` | 失败项优先 / 全 pass 清单配 fail verdict 全量随行 / 清单空退回 `report` / `（attempt N）` 头全部保留；消费者收窄为「非同源格的 fail 重试」 |

> 无 CLI 子命令、无 HTTP 端点、无 IPC 命令面变化（`phase_next` 不出 CLI / 不出 bindings）。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `PhaseNextOutcome` | `crates/core/workflow/src/write/phase_next.rs` | 零改动 | 字段集与填充不变：`backtrack_window` 仍为裁剪后的开放窗口集合（**不因抑制收缩**），`steps` / `artifact_manifest` / `allowed_backtrack_phases` / `last_result` / `error` / `round` / `done` 逐字不变 |
| `BacktrackWindowEntry` | 同上 | 零改动 | 五字段（`from_phase` / `from_index` / `to_phase` / `to_index` / `reason`）与「prompt 只渲染 `reason`」的纪律不变；**不新增 `marker_id` 类成员** |
| `ResolvedPhaseSpec` | 同上 | 零改动 | `prompt` / `model_level` 不变；doc「失败反馈段 / 回溯重做窗口段（append）」陈述仍成立（条件在场也是 append） |
| `PhaseNextError` | 同上 | 零改动 | 两变体（`VerdictFail` / `MaxRetriesExceeded`）与载荷不变 |
| `DecisionInput` | `crates/core/orchestration/src/decision.rs` | 零改动 | `fail_report` / `allowed` / `candidates` / `backtrack_window` 字段面与有界纪律不变（决策面不消费本抑制） |

---

## 数据模型 (Data Model)

**零 schema / 零迁移 / 零新列**：本变更不写库、不改写面裁定，`PhaseStateRecord` 与 `BacktrackWindowEntry` 的字段面逐字不变。同源是**读面派生**——由既有记录字段的排列直接判出，不落新状态。

**参与判据的既有字段（只读）**：

| 字段 | 用途 |
|---|---|
| `phase` / `backtrack_to` / `backtrack_reason` | 窗口候选的发起 / 目标 / 原因原文（`open_backtrack_windows` 的输入） |
| `id` | 创建序（落账序）与未闭环判据（发起相位无 `id` 更大条目即开放）；窗口裁剪按 `id` 升序 |
| `timestamp` | `latest_phase_entry` 的取首键（最新条目） |
| `verdict` | 反馈段的准入（`Verdict::Fail`） |
| `decision_session_id` | `fail_decided` 幂等闸（已决条目放行一次重跑） |
| `attempt` / `report` / `checklist` | 反馈段正文（同源格下整段不再消费） |

**关系与不变量（抑制完备性的依据，均既有）**：

1. **标记行恒为发起相位 `(timestamp, id)` 最大条目**（`apply_change_backtrack` 改写信面）；
2. **`latest_phase_entry` 取 `timestamp` 最大者**；同相位内 `timestamp` 与 `id` 同序（`phase_log` 写事务内 `max+1`）⇒ 同源格下最新条目就是标记行，被抑制的反馈正来自该记录；
3. **同源窗口恒为覆盖该相位者中创建序最末**（其 `from_index` = 当前相位下标）⇒ `take_last(MAX_BACKTRACK_WINDOWS)` 恒保留它。

**四格判据矩阵（实现与断言的共同真相）**：

| 格 | 判据（代码事实） | 段 A（窗口段） | 段 B（失败反馈段） | 归属 |
|---|---|---|---|---|
| **同源** | `open_windows.any(from_phase == P)` ∧ `P` 最新条目为标记行（Fail + 已决） | 在场 | **抑制** | 本变更收口 |
| **异源并存** | 窗口命中 ∧ `P != W.from` ∧ `P` 自身带已决 fail | 在场 | 在场（窗口段在前） | 零变化（`phase_next_test.rs:2374` / `:2932`） |
| **窗口外** | 无开放窗口 ∧ 最新条目 fail（决策 stop / 缺席 / 越权拒 / `backtrack` 写面 Err 后重启） | 缺席 | 在场（含 `（attempt N）` 与失败项） | 零变化（`:3105` 形态三） |
| **目标相位 / 自环第一拍** | 走 `backtrack_to` 分支（`retry_feedback = None`） | 在场 | 缺席（本就为 `None`，与同源抑制无关） | 零变化（`:2347` / `:2511`） |

**持久化**：无数据库写入、无迁移脚本、无 JSON 载体字段变化；prompt 是运行时拼装产物，不落库。同源格的唯一可观测差异是相位 prompt 少一段文本（约 −300 字 / 次）。

---

## 路由 / API 设计 (Route / API Design)

<!-- 不涉及：零新增 / 删除 IPC 命令，零 HTTP 端点，零 DTO 字段变化。`phase_next` / `error_outcome` 的响应字段面逐字不变，`PhaseNextOutcome` / `BacktrackWindowEntry` 均不出 bindings（`src/types/generated/bindings.ts` 零命中），auth 概念（无）不变；前端零改动（prompt 文本不投影到任何视图）。 -->

---

## 依赖 (Dependencies)

### 运行时依赖

零新增。改动只用标准库 `Option::filter` 与既有 `format!`；`crates/core/workflow` 的现有依赖（`agent` 的 `ModelLevel`、`crate::model` / `crate::state` / `crate::write::phase_table`）全部够用。`crates/core/orchestration` / `crates/infra/**` / 前端零新增。

### 构建 / 测试依赖

零新增。复用既有脚本与门禁：`pnpm -C packages/desktop run bindings:export`（生成步骤，本变更预期零 diff）、`api:export` / `api:check`（本变更零契约面变化）、`cargo test -p workflow`（测试相位执行）。零新 crate、零新语料、零新夹具。

---

## 验收标准回应

| AC | 设计落点 |
|---|---|
| AC-1 | D1 / D2 + 「数据流（改后）」：同源格 `feedback_suffix` 截断为空串，`window_section` 与 `backtrack_window` 照常 ⇒ 双角色 prompt 含 `## 回溯重做窗口` 与 `reason` 原文、零 `⚠️ 上次评估未通过`、零标记行 `report` / `checklist` 正文；响应字段面不收缩。断言面由测试相位承接（proposal「测试文件」表第 1 行：补否定断言 + 给标记行填可辨识 `report` / `checklist`，以证明抑制非空转） |
| AC-2 | D1：判据在源码中只出现一次（`open_backtrack_windows` 之后、`feedback_suffix` 装配处），形态 `open_windows.iter().any(\|window\| window.from_phase == def.id)`；`phase_next` 两处 `latest_phase_entry` 与路由判定零 diff；零新增字段、零 `id` / `timestamp` 比较（`git diff` 可逐行核） |
| AC-3 | 四格矩阵「异源并存」行 + D2：`P != W.from` ⇒ `same_origin = false` ⇒ 反馈段照常渲染，段序（窗口段在前）由既有插值序保证；`phase_next_test.rs:2932` / `:2374` 的判据不被触碰 |
| AC-4 | 四格矩阵「窗口外」行 + D2：无开放窗口 ⇒ `same_origin = false` ⇒ 反馈段（含 `（attempt N）`）照常；`:3105` 形态三的 fixture 无 backtrack 条目，零影响 |
| AC-5 | D2 / D6 + 起点实况「回溯分支」行：`backtrack_to` 分支（`:168`）恒传 `None`，抑制只作用于非 `None` 的 `retry_feedback`；自环（`to == from`）首拍同样走回溯分支，落账新条目后窗口由未闭环判据关闭 ⇒ 窗口外格。源码零 `if to == from` / `if id == next` 类特判 |
| AC-6 | D4：同源窗口恒为覆盖该相位者中创建序最末（`from_index` = 当前相位下标）⇒ `take_last` 恒保留，抑制不因裁剪失效；不在代码里加兜底。`MAX_BACKTRACK_WINDOWS + 1` 窗口的断言由测试相位承接（proposal「测试文件」表第 6 行） |
| AC-7 | D1 / D6：判据形态本身即双向判据的支点——放宽成「有窗口即抑制」会让异源并存格的反馈段消失，收紧成「`from == to` 才抑制」会让同源格的反馈段留下；三态矩阵用例由测试相位承接（proposal「测试文件」表第 5 行，同一 fixture 族逐格断言 A / B 在场性） |
| AC-8 | D2 + 类型定义表：`DecisionInput` 零字段变动、`decision_prompt` 零改动（orchestration delta「决策 prompt 不受同源抑制」）；`error_outcome`（`:269`）与 `done` 响应零 diff |
| AC-9 | 起点实况「生成物」行：`PhaseNextOutcome` / `BacktrackWindowEntry` 不在 bindings（实测零命中）、`.dev-team/architecture/models/**` 无 workflow 元素 ⇒ `bindings:export` / `api:check` 零 diff（D8 白名单核对） |
| AC-10 | D7：`0.7.9` → `0.7.10`；`tauri.conf.json` 仍 `"../package.json"`；`src-tauri/Cargo.toml` 版本行逐字不变 |
| AC-11 | D8 白名单：`git status --porcelain` ⊆ 本 change 目录 + `phase_next.rs` + `package.json`；orchestration / store / prompts / 前端 / 生成物 / 基线 spec / archive / `explore.md` / `AGENT.md` / `src-tauri/Cargo.toml` 零 diff |
| AC-12 | delta 形态已由 spec 相位落定（本设计不改 spec）：2 份 delta；`### Requirement` 标题与基线逐字一致（`desktop-workflow-write-face:288` `回溯重做窗口推导与 prompt 携带`、`desktop-change-orchestration:485` `相位 prompt 组装面静态化（静态主体 + 动态 append）`）；write-face delta 16 个 `#### Scenario:`（基线 11 + 新增 5，未丢任何未冲突 Scenario）、orchestration delta 6 个（基线 4 + 新增 2）；两份均含 `## Module Contract` |

**信息损失登记（R-1，随本设计留痕，MUST NOT 后续当缺陷重开）**：

1. 同源格 `（attempt N）` 消失——失败反馈段是该 prompt 内唯一承载重试次数的位置；
2. 同源格「上次 checklist 的逐项 pass / fail」消失——只剩决策者的散文修法方向（实证：拿到「12 条已过」仍全量重跑全部 13 条 AC，消费率近零）。

---

## 待决问题 (Open Questions)

1. **同源格是否需要一个不含 checklist 的最小「第 N 次重试」提示**：本设计取 proposal §五.1 的登记（不做）——`attempt` 与清单同段，只保留其一需新造第二处文案点，与「零新常量 / 零新渲染函数」冲突。若后续要求补，须作为独立变更（涉及窗口段文案面与 `ResolvedPhaseSpec` 语义），MUST NOT 在本变更内顺手加。
2. **R-3 的边界（同相位同毫秒双条目）**：`latest_phase_entry` 并列保留先见者、`apply_change_backtrack` 并列取 `id` 更大者，理论上存在「被抑制的反馈来自非标记的旧 fail」的极小概率窗口。本设计按 proposal 如实登记、**不设代码防线**（另立判据即重新制造 R-2）。若将来 `phase_log` 写入语义改变（不再 `max+1` 递增或允许同毫秒批量落账），该假设须重新评估——评估入口是本设计的「不变量 2」。
