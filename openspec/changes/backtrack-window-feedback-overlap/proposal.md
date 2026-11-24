# 提案: backtrack-window-feedback-overlap

> **变更**: backtrack-window-feedback-overlap（回溯窗口段 × 重试反馈段的同源重叠）
> **日期**: 2026-11-24
> **状态**: draft
> **类型**: requirement
> **来源**: 本 change 目录 `explore.md` 全篇（455 行：一手证据 §1 / 两段代码来源 §2 / 同源判据 §3 / 历史设计缺口 §4 / 量化 §5 / 备选切面 §6 / 「只留窗口段」的推演与证伪 §9–§10 / 裁决 §11）
> **裁决基线**: explore §11 已裁 —— **分格处置：同源格抑制反馈段，窗口外格保留**。本提案逐条遵循，不采用方案 a（上界开区间）/ c（信息分工）/ d（只改文案）/ e（不动，把重复写进 spec 当特性），也**不**放宽成「有窗口即抑制」。
> **性质**: prompt 组装面的**一处条件抑制**——同源格（当前相位 = 某开放窗口的发起相位）不渲染失败反馈段。零新字段 / 零 schema / 零 store 改动 / 零 IPC / 零 bindings / 零前端 / 零状态迁移；`PhaseNextOutcome` 字段面与路由判定逐字不变。收益 = 每条非自环回溯链的最后一拍不再把同一条 `PhaseStateRecord` 渲染两遍（≈300 字 / 次），并让失败上下文第一次有上界（窗口段 ≤ `MAX_BACKTRACK_WINDOWS × MAX_REASON_CHARS` ≈ 2.1K；反馈段的 `evidence` 无长度上限）。

---

## 问题

### 一 · 现象：同一条记录在源头相位被两个通道各读一半

一手证据（会话 `ses-42-1795484424107`，change `archi-model-path-and-silent-trap`，acceptance/evaluator/attempt 2）的 prompt 里前后两段指向同一件事（AC-6 冲突），文本不同：

| 信息 | 段 A（`## 回溯重做窗口`，`reason` 原文） | 段 B（`⚠️ 上次评估未通过`，`checklist` 失败项） |
|---|---|---|
| AC-6 未达成这一事实 | 有 | 有 |
| 病灶① · crates 测试负向 needle | 有（`:339/:352/:353`） | 有（同行号 + 命中短语） |
| 病灶②openspec 基线同行 MUST NOT | 有（泛化） | 有（`spec.md:384`） |
| 其余 12 条 AC 的实测证据 | 无 | 有 |
| 修法方向（怎么改） | **有**（「重写方向：…」） | 无 |

⇒ 重复的是「问题识别」，各自独有的是「修法方向」（决策者）与「通过证据 / 命中原文」（评估者）。

### 二 · 根因：同源格是结构必然，不是渲染 bug

两个通道的命中条件不同，但同源格由结构强制重合：

```
段 A 命中 ⟺ ∃ 开放窗口 W：W.to_index ≤ col(P) ≤ W.from_index   （含源头相位 W.from）
段 B 命中 ⟺ latest_phase_entry(P).verdict == Fail
同源重叠 ⟺ P = W.from ∧ W 的标记行就是 P 的最新条目
```

- **落账面**：`store.apply_change_backtrack`（`crates/infra/store/src/store.rs:1856`）**不是新增行**，而是把 `backtrack_to` / `backtrack_reason` 改写进「发起相位 `(timestamp, id)` 最大的条目」——标记行就是那条 fail 条目本身，`reason` 与 `checklist` / `report` 天然同体。
- **窗口推导**：`crates/core/workflow/src/write/phase_next.rs:362` `open_backtrack_windows`（区间 + 未闭环 + 末位 `MAX_BACKTRACK_WINDOWS` 裁剪）。
- **两段渲染**：`:419` `backtrack_window_section`（段 A，只取 `reason`）、`:440` `fail_feedback_suffix`（段 B，取 `checklist` / `report` + `attempt`）。
- **组装序**：`:465` `build_phase_response`，`:475-480` 依次拼 `header → 静态主体 → 窗口段 → 反馈段`；段 B 的输入是 `:251-252` 的 `retry_feedback`（`latest_phase_entry(next.id)` 过滤 `Verdict::Fail`）。

把「当前相位 P 的位置」摊开，四格中只有第一格是同源：

| P 的位置 | 段 A | 段 B | 关系 |
|---|---|---|---|
| **P = W.from 且最新条目即标记行** | 有 | 有（反馈就是标记行） | **同源（本变更收口）** |
| P = W.to（回溯后第一拍，走 `backtrack_to` 分支） | 有 | 无（`retry_feedback = None`） | 零变化 |
| P 在 (to, from) 中间且带自身已决 fail（决策未产出 backtrack 后重启） | 有（外层别的事） | 有（自己的事） | **异源并存**，必须保留 |
| 无开放窗口且最新条目 fail（决策 stop / 决策出错后重启） | 无 | 有（唯一失败上下文） | **窗口外**，必须保留 |

### 三 · 为什么当年没发现：设计只写了「并存」，没写「同源」

`openspec/changes/archive/2026-11-11-backtrack-window-context/explore.md` 4.7 原文设想的是「窗口内相位自身也可能带 fail 重试反馈」——即上表第 3 行（两次不同失败）；裁决 ① 给「含源头相位」的理由是「源头相位最需要知道上次因何而挂」，但 `backtrack_reason` 不是「因何而挂」（那是 checklist / report），而是决策者的路由理由与修法方向——「因何而挂」恰在段 B 里。测试（`phase_next_test.rs:2932`）固化的也是异源并存：窗口 reason 来自 test-gen、反馈来自 dev-design，断言双段在场且段序。**「源头相位同源并存」该形态只是判据的自然产物，从未被当成设计决策审视过**；`desktop-workflow-write-face` 位置契约里「反馈段是『本相位这次的失败清单』，窗口段是『为什么这个相位被叫回来』」两句，在源头相位描述的是同一件事。

### 四 · 为什么不能全局删反馈段：窗口外格是「决策 stop / 出错后重启」的恢复路径

1. **窗口判据不读 run 状态**：每次「发起运行」换新 `run_id` / 新 `SessionAnchors`（`round` 归 1），未闭环窗口照旧命中——重开一轮 run **不会**把相位挪出窗口。
2. **真正的窗口外戳记是「决策已挂账、却没产出 backtrack」**：`decision_session_id` 在动作裁决**之前**挂账（stop / 驳回路径同样留痕），故 `Stop` / 越权拒 / 决策缺席 / 决策会话异常 / `backtrack` 写面 Err 五种终局都留下「已决 + 无 marker」的 fail 条目。重启后 `fail_decided` 放行重跑、`open_backtrack_windows = []` ⇒ prompt 只余段 B。
3. **实证**：change `agent-finalize-tool-for-flow-steps`（`01a1c698`）acceptance#1 fail 且决策选 stop（理由原文「重启时请把 test-design（或 test-gen / implement）纳入白名单」）；重启后 acceptance#2 的 prompt（`ses-132-1794410024471`）零窗口段、段 B 是唯一上下文（上一轮卡住的 5–6 枚未杀死突变体）。删 B 会让决策 agent 自己建议的恢复路径零上下文。
4. **量化**：本机 db 已决 fail 条目 342 条——有 marker 301（同源格）/ 无 marker 41（≈12%，重启后的失败上下文）。这 41 条里多数是无窗口的窗口外格；落在更外层未闭环窗口区间内的部分则成为异源并存格（两种都必须保留反馈段）。全局删 B 的收益封顶仍是 301 条重复，代价是这 41 条恢复路径失明；分格处置零空洞。

### 五 · 量化与同源格的边界

- 该 prompt 全文 ≈6.3K 字符，重复面 ≈300 字（≈5% 全文、≈300 token 量级），每条非自环回溯链出现一次。
- 同源格的出现条件窄且确定：非自环回溯 → 整个窗口重走完 → 回到源头相位的那一拍；该拍恰是窗口闭环前最后一拍，故每条回溯链最多一次。
- 同 prompt 体系里还有更大的同源重复（决策 prompt 的 `## 失败相位报告` × `## 失败清单` 来自同一条记录；`## 候选相位最近一次评估报告` 全量内联）——**不在本变更范围**，见「explore 未决项」。

---

## 提案

### 一 · 判据：零新字段，一行条件

```
same_origin ⟺ open_windows.iter().any(|w| w.from_phase == def.id)
```

完备性由两条既有不变式导出，不需要 `marker_id`、不需要 schema 变动：

1. 标记行恒为「发起相位 S 的 `id` 最大条目」（`open_backtrack_windows` 的 open 判据）；
2. `latest_phase_entry(P)` 取 P 的 `timestamp` 最大者，而同相位内 `timestamp` 与 `id` 同序（`phase_log` 写事务内 `max+1`，`apply_change_backtrack` 也按 `(timestamp, id)` 取最大）⇒ `P == W.from` 且 W 开放 ⇒ W 的标记行就是 P 的最新条目，`retry_feedback` 就来自这条记录。

⇒ 实现成本 = 组装侧一个条件，不是结构体改造。

**判据 MUST 写成这一形**，MUST NOT 重新比较记录 `id` / `timestamp`（那是在消费侧另起第二份同源判定），MUST NOT 挪到 `phase_next` 调用侧重复推导（`open_windows` 已在 `build_phase_response` 内算出）。

### 二 · 落点：`build_phase_response` 一处（`phase_next.rs:475-477`）

```rust
let open_windows = open_backtrack_windows(entries, table, def.id);
let same_origin = open_windows.iter().any(|window| window.from_phase == def.id);
let window_section = backtrack_window_section(&open_windows);
let feedback_suffix = retry_feedback
    .filter(|_| !same_origin) // 同源：段 B 与窗口段读同一条记录，只留窗口段
    .map(fail_feedback_suffix)
    .unwrap_or_default();
```

- `retry_feedback` 形参与 `fail_feedback_suffix` 渲染函数**保留**（窗口外格仍是其消费者，不产生死代码）；`phase_next` 的两处 `latest_phase_entry` 返回值与路由判定逐字不动（抑制只发生在渲染层）。
- `open_backtrack_windows` / `backtrack_window_section` / `PhaseNextOutcome` 字段面 / `error_outcome` / `done` 响应 / 决策输入 `DecisionInput.backtrack_window` 全部零改动。
- `build_phase_response` 的函数 doc 现写「失败反馈段（fail 重试时在场）」，须同步为「同源格剔除后在场」；`fail_feedback_suffix` 的 doc 补一句「窗口外格消费者」。

### 三 · 三态定稿（与 explore §11.2 一致，并订正其漏项）

| 格 | 判据 | 段 A | 段 B | 依据 |
|---|---|---|---|---|
| 同源（源头相位重跑） | `open_windows.any(from_phase == P)` | ✅ | ❌ **抑制** | 反馈段 = 评估者自身旧输出回灌；窗口段 = 决策者的修法方向（explore §9.3 实证：acceptance#2 拿到「12 条已过」仍全量重跑，消费率近零；proposal#4 采纳了窗口段的修法方向） |
| 异源并存（窗口内中间相位自带已决 fail） | 窗口命中 ∧ `P != W.from` | ✅ | ✅ | 外层窗口讲另一次失败、反馈讲本相位这次失败；`phase_next_test.rs:2932` 已是该格的对照用例 |
| 窗口外（决策 stop / 出错后重启） | 无窗口 ∧ 最新条目 fail | ❌ | ✅ | 唯一失败上下文（§四.3 实证） |
| 目标相位 / 自环第一拍 | 走 `backtrack_to` 分支 | ✅ | ❌（本就 `retry_feedback = None`） | 零变化；自环永不进同源格的第一拍 |

> **对 explore §11.2 的订正**：该处的三态表漏了「异源并存」格，而 §11.3 又把 `phase_next_test.rs:2932` 称作异源格对照——两处自相矛盾。按代码事实，异源并存格**存在**：更外层的未闭环窗口尚未闭环，同时该相位自身有一条「已决但未产出 marker」的 fail（决策 stop / 出错后重启，且该相位恰好落在外层窗口区间内）。该格必须保留 A+B，且 `:2932` 用例就是它的回归防线；「三态穷尽」只在「两段的并存形态」这一意义上成立，不覆盖「仅窗口段且 B 本就不在场」的常态（窗口内已通过相位的重跑、回溯后第一拍）。

### 四 · 裁剪相容性：同源窗口恒存活

抑制判据读的是**裁剪后**的 `open_windows`。同源窗口恒为「覆盖该相位的开放窗口」中创建序最末者：它的 `from_index` = 当前相位下标（不大于任一覆盖者），且该相位重跑落新条目即闭环 ⇒ 不存在更晚创建的覆盖者。而 `take_last(MAX_BACKTRACK_WINDOWS)` 保留的正是创建序末位 ⇒ **同源窗口恒在渲染集合内**，抑制不会因 ≥3 层嵌套而失效，也不需要额外兜底。

### 五 · 登记的两项信息损失（可接受，须留痕）

1. **`（attempt N）` 在同源格消失**：段 B 是该 prompt 内唯一承载重试次数的位置。源头相位重跑时 agent 不再知道自己是第几次；评估者按「全量重评」语义工作，该项非必需。
2. **「上次 checklist 的逐项 pass / fail」消失**：同源格只剩决策者的散文修法方向。实证支持可接受——acceptance#2 拿到该清单后仍全量重跑全部 13 条 AC（消费率近零）。

两项 MUST 写进 spec 的抑制条款旁（不留白，避免后续被当成缺陷重开）。

### 六 · 明确不做

- 不做全局单通道（方案「只留 A」）：窗口外格 41/342 恢复路径零上下文，且 `fail_feedback_suffix` 退化为死代码。
- 不做区间上界开区间（方案 a）：自环回溯（`to == from`）会退化为空区间，`reason` 唯一通道消失，必须特判——破坏既有「零特判」不变量。
- 不做信息分工（方案 c）：`reason` 是自由文本，无法机械切成「方向」与「证据」，需 finish_step schema 变更 + 历史数据迁移。
- 不做文案澄清（方案 d）：零 token 收益，只降低重复感。
- 不把「同源必重复」写进 spec 当特性（方案 e）：已裁去重。

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-workflow-write-face` | **MODIFIED ×1** | 「回溯重做窗口推导与 prompt 携带」：① 新增**同源抑制**条款（判据形 `open_windows.iter().any(\|w\| w.from_phase == def.id)`、判据单点落在 `build_phase_response`、MUST NOT 比较 `id` / `timestamp`、MUST NOT 在 `phase_next` 侧二次推导、`retry_feedback` 形参与 `fail_feedback_suffix` 保留、响应字段面零变化）；② 位置契约改成双分支（两段并存时的序不变；同源格反馈段整段缺席，序退化）；③ 新增裁剪相容不变式（同源窗口恒为创建序最末者 ⇒ 恒存活）；④ 登记两项信息损失；⑤ 新增 5 条 Scenario（同源只留窗口段 / 抑制判据单源 / 窗口外格保留 / 同源窗口过裁剪仍存活 / 抑制只发生在渲染层）；⑥ 既有 Scenario「段位置先于反馈段」的 WHEN 收紧为「**不是**该窗口发起相位」 |
| `desktop-change-orchestration` | **MODIFIED ×1** | 「相位 prompt 组装面静态化（静态主体 + 动态 append）」：① 动态面清单写明反馈段在场性由写面决定，同源格由写面整段省略；② 段序改成「两段并存时」的前置条件，并写清 orchestration MUST NOT 依据 `backtrack_window` 自行抑制或补回反馈段（零本地判据）；③ 新增 Scenario「同源格反馈段由写面省略」与「决策 prompt 不受同源抑制」（决策输入仍携 fail checklist / `fail_report`）；④ 既有 Scenario「段序窗口段先于反馈段」的 WHEN 收紧为异源并存格 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-change-state-store` | 零 schema / 零迁移 / 零新列；`apply_change_backtrack` 与 `PhaseStateRecord` 字段面逐字不变（同源是读面派生） |
| `desktop-ipc-type-bindings` | `PhaseNextOutcome` / `BacktrackWindowEntry` 不出 bindings（`src/types/generated/bindings.ts` 对二者零命中），`bindings:export` 零 diff |
| `desktop-phase-prompts` | 16 份角色 md 零改动；抑制是运行时组装面的条件，不是静态角色知识 |
| `desktop-change-flow-view` / `desktop-change-queries` | 图面泳道序、边标签、`backtrack_reason` 出线面零改动；只消费既有字段 |
| `desktop-change-archive` | `spec_sync_prompt` / `merge_conflict_prompt` 是另一套组装，且 archive 从不产 fail 条目（九步 `on_fail` 全 `None`，失败一律 `Terminal`），永不出现在任一格 |
| `desktop-agent-execution` / `desktop-checks-domain` / `desktop-test-execution` / `desktop-mutation-testing` | fail 条目产出面（`PhaseSettle::Fail` 三处）与门禁分派零改动 |
| `desktop-workspace-store` / `desktop-write-protection` / `desktop-corpus-regression` / `desktop-build-modes` / `desktop-release-pipeline` | 落库载体、锁面、语料、构建模式、发版链零触碰 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | `build_phase_response`（`:465`）内、`let open_windows`（`:475`）之后新增 `let same_origin = open_windows.iter().any(\|window\| window.from_phase == def.id);`；`feedback_suffix`（`:477`）加 `.filter(\|_\| !same_origin)`；`build_phase_response` 与 `fail_feedback_suffix` 的 doc 注释同步一句。净增 ≤ 4 行；`phase_next`（`:214` / `:251-252`）与 `error_outcome` 零改动 |
| `packages/desktop/package.json` | `version` patch +1（落地时按 `HEAD` 的 `0.7.9` → `0.7.10`；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` 版本行 MUST NOT 跟随） |

### 测试文件

| 文件 | 用例 / 装置 | 处置 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs` | `回溯窗口四态_上界源头相位命中`（`:2406`，现状只断窗口段） | **补**：双角色 prompt 零 `⚠️ 上次评估未通过`；给标记行填可辨识 `report` / `checklist`，断言其正文零命中（证明抑制非空转，而非「本来就没有反馈」） |
| 同上 | `回溯窗口四态_区间内部相位双角色携背景`（`:2374`） | **补**：双角色 prompt **仍**含反馈段（异源并存格，防抑制放宽成「有窗口即抑制」） |
| 同上 | `窗口段位置先于失败反馈段`（`:2932`） | 逐字不动（异源并存 + 段序的对照用例，改动后应原样绿） |
| 同上 | `零窗口回归_既有种子窗口空集且prompt零段`（`:3105`，形态三） | 逐字不动（窗口外格 = 只反馈段） |
| 同上 | **新增**：三态矩阵用例（同源 / 异源并存 / 窗口外 同一 fixture 族逐格断言「A、B 各自在场或不在场」） | 新增 |
| 同上 | **新增**：同源窗口过层级裁剪仍存活（`MAX_BACKTRACK_WINDOWS + 1` 个窗口覆盖当前相位、同源窗口创建序最末 → 抑制仍生效） | 新增 |
| 同上 | 自环用例（`:2511`）、跨 run 用例（`:3369`）、`回溯与正常分支窗口段同形且零退役文案`（`:3043`）、`失败项随已决fail重跑prompt注入且首轮零注入`（`:717`） | 逐字不动（第一拍 `retry_feedback = None`；正常分支是异源并存格） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/**` | 零改动：`walker` 只消费写面 prompt，`DecisionInput` / `phase_next` 的 error 响应零变化；`walker_test.rs` 的段序用例由假写面自构 prompt，不经 `build_phase_response` | 零改动 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `crates/core/workflow/src/write/phase_table.rs` / `write/mod.rs` / `write/backtrack.rs` | 常量区、出口面、落账裁定零改动（无新常量、无新类型） |
| `crates/infra/store/**` | 零 schema / 零迁移 / 零写事务改动；`apply_change_backtrack` 与 `PhaseStateRecord` 字段面逐字不变 |
| `crates/core/orchestration/src/**`（含 `decision.rs` / `prompt.rs` / `walker.rs` / `finish.rs`） | 决策 prompt 的 `fail_report` + `checklist` + 窗口段三重面不在本变更；`DecisionInput` 零字段变动 |
| `crates/core/workflow/src/prompts/**` | 16 份角色 md 零改动 |
| `packages/desktop/src/**`、`src/types/generated/**` | 零 IPC / 零 UI / 零 bindings 变化；`bindings:export` 应零 diff |
| `openspec/specs/**`（基线）与 `openspec/changes/archive/**` | delta-only：本变更只写 2 份 delta，基线旧句由归档合并本 delta 的 MODIFIED 块收口 |
| `openspec/changes/backtrack-window-feedback-overlap/explore.md` | 只读输入 |
| `AGENT.md` / `CLAUDE.md` / `src-tauri/Cargo.toml` | 仓库根指引与 crate 版本行不动（版本纪律） |

---

## 验收标准

1. **AC-1 · 同源格抑制成立且非空转**：一次 `backtrack(from=test-gen, to=dev-design, reason=R)` 后流程重走完窗口、路由回 test-gen（其最新条目即标记行）⇒ executor 与 evaluator prompt 均含 `## 回溯重做窗口` 与 R，且均零 `⚠️ 上次评估未通过`、零标记行 `report` / `checklist` 正文；`backtrack_window` 仍为该窗口（响应字段面不因抑制收缩）。
2. **AC-2 · 判据单源、零第二份判定**：抑制判据在源码中只出现一次，形如 `open_windows.iter().any(|w| w.from_phase == def.id)`，位于 `build_phase_response` 内；`phase_next` 的两处 `latest_phase_entry` 取值与路由判定零改动；零新增 `marker_id` 类字段、零记录 `id` / `timestamp` 比较（源码审查 + `git diff` 逐行）。
3. **AC-3 · 异源并存格原样**：窗口内中间相位（非发起相位）带自身已决 fail 且决策未产出 backtrack ⇒ 双角色 prompt 同时含窗口段与反馈段，段序窗口段在前；`phase_next_test.rs:2932` 与 `:2374` 两项用例绿。
4. **AC-4 · 窗口外格原样**：无开放窗口且最新条目 fail（决策 stop / 决策缺席 / 越权拒 / `backtrack` 写面 Err 后重启）⇒ prompt 只含反馈段（含 `（attempt N）` 与失败项），零窗口段；`:3105` 形态三绿。
5. **AC-5 · 目标相位与自环零变化**：回溯后第一拍（`backtrack_to` 分支）仍 `retry_feedback = None`；自环（`to == from`）第一拍与落账后行为逐字不变，源码零 `if to == from` / `if id == next` 类特判。
6. **AC-6 · 裁剪相容**：命中 ≥3 个同时开放窗口且当前相位是其中某窗口的发起相位 ⇒ 该同源窗口必在渲染集合内（创建序最末，`take_last` 保留），抑制照常生效；新增用例直接钉住这条不变式。
7. **AC-7 · 三态矩阵有守护力**：新增三态矩阵用例在同一 fixture 族下逐格断言「A 在场 / 不在场」与「B 在场 / 不在场」；把判据放宽成「有窗口即抑制」时异源并存格判红，把判据收紧成「`from == to` 才抑制」时同源格判红。
8. **AC-8 · 指令面无回归**：决策 prompt 仍携 fail checklist / `fail_report` / `backtrack_window` 三面（同源抑制只作用于相位 prompt 渲染层）；`error_outcome` / `done` 响应逐字不变。
9. **AC-9 · 生成物零 diff**：`pnpm bindings:export` 后 `git diff` 对 `src/types/generated/**` 为空；`pnpm api:check` 零差异退出（零契约面变化）。
10. **AC-10 · 版本纪律**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch（`0.7.9` → `0.7.10`）；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本逐字不变。
11. **AC-11 · 改动集收敛**：`git status --porcelain` ⊆ 本提案「变更范围」所列文件；`openspec/specs/**`、`openspec/changes/archive/**`、`explore.md` 零改动；orchestration / store / 前端零改动。
12. **AC-12 · spec delta 形态**：恰 2 份 delta；两份 `### Requirement` 标题与基线逐字一致；MODIFIED 块为基线整段拷贝后再改，未丢任何未冲突 Scenario；每条 Requirement ≥1 个 `#### Scenario:`（恰 4 个 `#`），WHEN / THEN 成对；两份 delta 均含 `## Module Contract`。

---

## 风险

| # | 风险 | 处置 / 依据 |
|---|---|---|
| R-1 | **产品行为变更**：源头相位重跑时 agent 不再看到上一轮 `attempt` 与逐项 checklist（explore §11.5 两项信息损失） | 已裁接受：实证消费率近零（acceptance#2 拿到「12 条已过」仍全量重跑）；评估者按全量重评语义工作。**回滚点就是这一行条件**，spec 明写损失面避免后续当缺陷重开 |
| R-2 | **判据漂移**：把「有窗口」当同源（过宽 → 误伤异源并存），或比较记录 `id` / `timestamp` 自建同源判定（第二份真值），或挪到 `phase_next` 侧重复推导 | AC-2 源码审查 + AC-7 三态矩阵双向判红；spec 把判据形与落点写成 MUST |
| R-3 | **`timestamp` / `id` 同序假设**：`latest_phase_entry` 按 `timestamp` 取首（并列取先），`open` 判据与 `apply_change_backtrack` 按 `(timestamp, id)` 取最大——同相位同毫秒双条目下，被抑制的反馈可能来自一条非标记的旧 fail | 概率极低（同相位两次落账相距 1ms 内不现实），影响仅为少渲染一段冗余证据（窗口段仍在、路由不变）；MUST NOT 为它另立第二判据（那会重新制造 R-2）；如实登记，不设代码防线 |
| R-4 | **裁剪把同源窗口裁掉**（若判据改读别的集合，或裁剪方向被改） | 「同源窗口恒为创建序最末者」是结构性质（提案 §四），AC-6 用 `MAX_BACKTRACK_WINDOWS + 1` 窗口直接钉住 |
| R-5 | **测试守护力缺失**：既有 `:2406` 只断窗口段，改动后仍绿——不补断言等于白改 | AC-1 要求补否定断言 + 可辨识 `report` 正文零命中；AC-7 要求新增三态矩阵 |
| R-6 | **误扩大范围**：顺手去重决策 prompt 的 `report` × `checklist` × 候选报告三重面（体积收益更大） | 明确不做（explore §5 已登记）；AC-8 钉住决策面零变化 |
| R-7 | **误以为 archive 是窗口外样本** | explore §10.3 已证伪：archive 九步 `on_fail` 全 `None`，从不产 fail 条目；窗口外格的生产样本是「决策 stop / 出错后重启」 |
| R-8 | **顺带清理冲动**：`latest_phase_entry` 在 `phase_next` 内算两次（`:214` / `:251`）、`fail_feedback_suffix` 不过滤 `stale`、report 上限双数字（角色 md 500 / `verdict.rs` 2000）、`MAX_REASON_CHARS` 双值（写面 1000 / 决策面 500） | 均登记待议、本变更零触碰（`phase_next.rs` 只加一行条件 + 两处 doc 注释；其余文件零 diff） |

---

## explore 未决项与「不含」的归属

| explore 条目 | 处置 |
|---|---|
| §11.2 三态表 | 已订正：该表漏「异源并存」格，与 §11.3 的 `:2932` 异源对照用例矛盾；本提案按四格表述（同源 / 异源并存 / 窗口外 / 目标相位），异源并存格的回归防线 = `phase_next_test.rs:2932` 原样绿 |
| §6 方案 a（上界开区间） | 不做：自环退化为空区间，`reason` 唯一通道消失，须特判，破坏零特判不变量 |
| §6 方案 c（信息分工）/ d（只改文案）/ e（不动） | 不做：裁决取方案 b（同源去重） |
| §6 末尾「消费端无法判定同源、必须补 `marker_id`」 | 已更正（§9.1）：`from_phase` 已在结构体里且判据自足，本变更零 schema |
| §9.2 原判「窗口外样本 = archive」 | 已证伪（§10.3）：窗口外样本 = 决策 stop / 出错后的重启恢复；不影响裁决 |
| §9.6 / §11.6 三项顺带可清（`latest_phase_entry` 双算 / `stale` 过滤 / report 上限双数字） | 不做，登记待议（R-8） |
| §5 决策输入面三重重复（report × checklist × 候选报告） | 不做，独立变更（R-6） |
| §7 第 2 项 `MAX_REASON_CHARS` 双值（1000 / 500，实际卡口 500） | 不做，登记待议 |
| §7 第 3 项「全 pass 清单配 fail verdict」防御式回退分支 | 本变更不动该分支（窗口外 / 异源格的 `fail_feedback_suffix` 逐字保留）；其可达性属 verdict 推导单点，另行核查 |
