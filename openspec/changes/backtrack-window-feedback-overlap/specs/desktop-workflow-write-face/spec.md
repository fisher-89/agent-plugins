# desktop-workflow-write-face Specification（delta: backtrack-window-feedback-overlap）

## Purpose

本 delta 只改**失败反馈段与回溯窗口段在同一拍上的并存规则**，窗口推导、裁剪、渲染与响应字段面逐字不动：

- 新增「**同源抑制**」：当前相位恰是某开放窗口的发起相位时，失败反馈段整段省略，只留窗口段（两段读的是同一条 `PhaseStateRecord`，`reason` 是决策者的修法方向、`checklist` / `report` 是评估者的旧输出回灌）。
- 异源并存格（窗口内、非发起相位、自身带已决 fail）与窗口外格（无窗口、最新条目 fail——决策 stop / 出错后的重启恢复路径）**原样保留**两段 / 单段行为。
- 判据零新字段、零 schema：`open_windows.iter().any(|w| w.from_phase == def.id)`，落在 `build_phase_response` 内，MUST NOT 另起第二份同源判定。
- `BacktrackWindowEntry` 五字段、`PhaseNextOutcome.backtrack_window`、`open_backtrack_windows` / `backtrack_window_section` / `fail_feedback_suffix`、error / done 响应与决策输入零改动。

## MODIFIED Requirements

### Requirement: 回溯重做窗口推导与 prompt 携带

`phase_next` SHALL 从既有评估条目派生「回溯窗口」，并随 `PhaseNextOutcome` 下发，使回溯窗口内**每个相位的每个 agent 步**（executor 与 evaluator 同等）的 prompt 均携带该背景，直至窗口闭环。窗口闭环判据与边界 MUST NOT 依赖运行期状态或 run 锚点。

窗口记录的字段面 SHALL 为 `BacktrackWindowEntry`：

- `from_phase: String`（发起相位，区间上界）与 `from_index: usize`（其相位表序下标）；
- `to_phase: String`（目标相位，区间下界）与 `to_index: usize`（其相位表序下标）；
- `reason: String`（`backtrack_reason` 原文逐字）。

`PhaseNextOutcome` SHALL 增 `backtrack_window: Vec<BacktrackWindowEntry>`：正常响应填「覆盖 `next_phase` 的开放窗口」，decision 分叉的 error 响应填「覆盖失败相位的开放窗口」；两处 SHALL 共用同一推导函数，MUST NOT 各持一份判据。空集合即「无窗口」（MUST NOT 用 `Option` 表达同一语义）。

三条判据 SHALL 为：

1. **是否拼接** `connect(P, W) ⟺ to_index ≤ col(P) ≤ from_index`，其中 `col` = 该相位在相位表中的下标（与 `detail.pipeline` 下标、前端 `colIndex` 同源，MUST NOT 另引第二份相位序）；
2. **是否未闭环** `open(W) ⟺ 发起相位不存在 id 大于标记行 id 的条目`；
3. **堆叠与裁剪** `filter(open) → filter(connect) → 按 order 升序 → take_last(MAX_BACKTRACK_WINDOWS)`，`MAX_BACKTRACK_WINDOWS = 2` 与 `MAX_REASON_CHARS` 同驻写面常量区，prompt 文案与测试断言 MUST NOT 写死该数字。

上限 SHALL 写成双分支，MUST NOT 只写其中一支：命中窗口数 **≤ `MAX_BACKTRACK_WINDOWS`（0 / 1 / 2 层）时全量并集**（按创建序升序、最早在前，多窗互不暂停互不覆盖）；**超出上限时取创建序末位两层**，被裁的恒为创建序最早那条——它同时是该组中 `from_index` 最大者（同时开放窗口按创建序 `from_index` 严格递减，故「创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` 最大」；explore 口语化的「最旧最宽」只是该判据的常见后果，MUST NOT 反过来以「区间最宽」定义裁剪位置）。并集语义（A2）在上限内完整保留，超限则降级为末位两层：这是 A2 与上限的唯一共存点，两处 delta（本能力与 desktop-change-orchestration）都须如实写到该双分支。

判据 MUST NOT 读取 `run_id` / `SessionAnchors`（跨 run 继承由此天然成立）。边界 SHALL 零特判：含源头相位（`P = from` 落在区间上界；该拍的失败事实仍由同一条记录承载，但只经决策者的修法方向（窗口段）呈现——同一记录的逐项清单在同源格被抑制，见下「同源抑制」）、自环回溯（`to == from` 退化为单点区间，与常规回溯同一条代码，MUST NOT 写 `if to == from` 特判）、嵌套回跳（多窗各自独立开放，下游相位可同时命中；内层回跳到更上游后外层当前相位落到区间外即不再拼，回放进区间即再拼——由区间判据自愈，MUST NOT 另设暂停 / 恢复状态）。

文案 SHALL 由单点常量承载（经 `write/mod.rs` 再导出供 orchestration 复用，MUST NOT 有第二份副本）：标题 `## 回溯重做窗口（背景信息）` + 一段纯陈述说明（按发生先后排列、最上面最早、保留原始换行与标点、未做截断或改写、属背景不构成额外验收条件）+ 逐条以 `<backtrack reason>` / `</backtrack>` 定界的**原因原文**。标签行 SHALL 零属性（`from` / `to` / 顺序号一律不渲染）；渲染 MUST NOT 取自 `reason` 之外的任何字段，MUST NOT 出现任何相位 id 字面量。原因 MUST NOT 被截断、压平或 trim（含首尾换行与标点逐字保留）。

**同源抑制** SHALL 为相位 prompt 组装面的第三态：当覆盖当前相位的开放窗口集合中存在 `from_phase == 当前相位`（下称「同源格」）时，失败反馈段 SHALL 整段省略，只留窗口段。判据 SHALL 只取 `open_windows.iter().any(|w| w.from_phase == def.id)` 这一形——MUST NOT 重新比较记录 `id` / `timestamp` 自建同源判定（那是在消费侧另起第二份判据），MUST NOT 挪到 `phase_next` 调用侧重复推导（`open_windows` 已在 `build_phase_response` 内算出）。抑制 SHALL 只发生在渲染层：`retry_feedback` 形参与 `fail_feedback_suffix` 渲染函数 SHALL 保留（窗口外格仍是其消费者，不得退化为死代码），`PhaseNextOutcome` 字段面、窗口推导、路由判定与 error / done 响应逐字不变。抑制的完备性 SHALL 依赖两条既有不变式而不引入新字段：① 标记行恒为发起相位 `id` 最大的条目（本能力判据 2）；② 同源格下该标记行就是发起相位的最新条目（`apply_change_backtrack` 按 `(timestamp, id)` 取最大改写信面，`latest_phase_entry` 按 `timestamp` 取首，同相位内两者同序）——MUST NOT 以「再比一次记录身份」加固它。就「窗口段与失败反馈段的并存形态」而言三态 SHALL 穷尽且互斥：同源格只窗口段、异源并存格（窗口内但不是发起相位、自身带 fail 重试）两段并存且序不变、窗口外格（无开放窗口且最新条目 fail）只反馈段（决策 stop / 决策缺席 / 越权拒 / `backtrack` 写面 Err 后重启的恢复路径，是该拍唯一失败上下文）。「仅窗口段且失败反馈段本就不在场」的常态（窗口内已通过相位的重跑、回溯后第一拍）SHALL NOT 被读作第四种并存形态（`retry_feedback = None` 与同源抑制是两条不同的成因，二者 MUST NOT 合并实现）。

同源抑制 SHALL 与层级裁剪相容：同源窗口恒为「覆盖该相位的开放窗口」中创建序最末者（其 `from_index` = 当前相位下标，不大于任一覆盖者；且该相位重跑落新条目即闭环 ⇒ 不存在更晚创建的覆盖者），故 `take_last(MAX_BACKTRACK_WINDOWS)` 恒保留它，抑制 MUST NOT 依赖裁剪集合之外的额外兜底。

同源格的两项信息损失 SHALL 随本条款登记（可接受，MUST NOT 当作缺陷重开）：① `（attempt N）` 消失——失败反馈段是该 prompt 内唯一承载重试次数的位置；② 「上次 checklist 的逐项 pass / fail」消失——同源格只剩决策者的散文修法方向。可接受依据：评估者按「全量重评」语义工作，实证中该清单被消费的比例近零（拿到「12 条已过」仍全量重跑）。

位置 SHALL 为契约而非排版偏好：**两段并存时**，executor / evaluator 的窗口段 SHALL 位于静态主体之后、失败反馈段**之前**，evaluator 的最终序 SHALL 为 `上下文头 → 静态主体 → 窗口段 → 失败反馈段 → 输出协议附录`；**同源格下**失败反馈段整段缺席，序退化为 `上下文头 → 静态主体 → 窗口段 →（evaluator）输出协议附录`。MUST NOT 置于失败反馈段之后（反馈段是「本相位这次的失败清单」，窗口段是「为什么这个相位被叫回来」，后者是前者的上位语境）。

体积 SHALL 确定有界：单相位窗口段 ≤ `MAX_BACKTRACK_WINDOWS` ×（标签与说明行 + `MAX_REASON_CHARS`）≈ 2.1K 字符。实现 MUST NOT 因体积顾虑再加截断。

#### Scenario: 窗口内每个相位每个 agent 步都携带背景

- **WHEN** 决策回溯 `to=dev-design` / `reason=R` 后流程经 dev-design → test-design → implement → test-gen 依次推进
- **THEN** 四个相位的 executor 与 evaluator prompt 均含 `## 回溯重做窗口` 段与 R（无一相位丢失），窗口外相位（如 code-review）零该段

#### Scenario: 含源头相位

- **WHEN** 流程推进到发起相位（如 test-gen）本身并请求其 prompt
- **THEN** 该相位 prompt 仍含窗口段（`to_index ≤ col(P) = from_index`），但失败反馈段整段缺席（同源格）；该相位落账新条目后（`id` 大于标记行）窗口即闭环，其后续 prompt 零该段——此时若新条目已决但未产出新 marker，喂回的是反馈段（窗口外格）

#### Scenario: 自环回溯退化为单点且零特判

- **WHEN** 以 `backtrack_to` 等于发起相位（如 `to = from = implement`）回溯
- **THEN** 窗口区间退化为单点，只有 implement 自身 prompt 携带窗口段，其落账即闭环；源码审查确认判据零 `if to == from` 特判分支

#### Scenario: 嵌套双窗按创建序并集

- **WHEN** 两个窗口同时开放（`from=test-gen → to=dev-design` 与 `from=test-design → to=proposal`）且下游相位同时落在两区间内
- **THEN** 该相位窗口段按创建序渲染两条原因（早者在前）；只落在较窄区间的相位只渲染对应那一条

#### Scenario: 内层回跳不改外层判据

- **WHEN** 外层窗口 `[dev-design..test-gen]` 开放期间发生内层回溯 `from=implement → to=proposal`，流程被打回 proposal 后逐相位重推
- **THEN** proposal 相位只渲染内层原因（外层 `to_index` 之上，区间不命中），dev-design 相位两条同时命中（外层仍在区间内、内层区间下界命中），test-gen 相位只渲染外层原因——全程零暂停 / 恢复状态，判据由区间自愈

#### Scenario: 层级上限取末位两层

- **WHEN** 同一相位命中三个同时开放的窗口（创建序 Win1 / Win2 / Win3，`from_index` 依次为 5 / 4 / 3）
- **THEN** 渲染集合为末位两层（Win2 与 Win3，按创建序升序），创建序最早的 Win1 被裁
- **AND** 被裁的 Win1 恒为该组中 `from_index` **最大**者（同时开放窗口按创建序 `from_index` 严格递减 ⇒ 创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` 最大；「最旧最宽」仅为其常见口语后果，不作判据）；出线的是 `from_index` 最小的两条
- **AND** 窗口段零「另有 N 条未展开」类元信息

#### Scenario: 求值顺序固定

- **WHEN** 构造三窗用例：输入条目序与创建序不同（条目不按 `id` 升序排列），且创建序最早的窗口 `from_index` 最大、最后创建者最小（符合结构性性质）
- **THEN** 结果与 `filter(open) → filter(connect) → 按 order 升序 → take_last(MAX_BACKTRACK_WINDOWS)` 逐条相等（即渲染 `from_index` 最小的两条，恰为最近创建的两次回溯）
- **AND** 把顺序写成「先 `take_last` 再 `sort`」的实现取到的是输入末位两条而非创建序末位两条，在该用例下判红

#### Scenario: 跨 run 继承

- **WHEN** 回溯窗口尚未闭环时停止 run 并以新 `run_id` 重新发起（新 `SessionAnchors` 实例、`round` 归 1）
- **THEN** 目标相位的 prompt 仍含窗口段（判据不读 run 锚点）；源码审查确认窗口判据的入参不含 `run_id` / `SessionAnchors`

#### Scenario: error 响应同样携窗口

- **WHEN** 失败相位落在某开放窗口内并触发决策分叉（`VerdictFail` 或 `MaxRetriesExceeded`）
- **THEN** 该 error 响应的 `backtrack_window` 为覆盖失败相位的开放窗口集合（与正常响应同一推导函数），供决策输入直接消费

#### Scenario: 渲染零元信息零相位标识

- **WHEN** 审查窗口段渲染函数的取值与产物
- **THEN** 每条只取 `reason` 一个字段，标签行零属性；断言 prompt 正文不出现 `from_phase` / `to_phase` 的任何字面量；原因原文（含换行 / 首尾空白 / 标点 / 1000 字符长文）逐字保留

#### Scenario: 段位置先于反馈段

- **WHEN** 一个落在窗口内、但**不是**该窗口发起相位的相位同时带 fail 重试反馈段（异源并存格）
- **THEN** prompt 中窗口段索引小于失败反馈段索引；evaluator 的输出协议附录仍在最后

#### Scenario: 同源格只留窗口段

- **WHEN** 回溯 `from=test-gen` / `to=dev-design` 后流程重走完窗口、路由回发起相位 test-gen（其最新条目即标记行，标记行携 `reason` 与非空 `report` / `checklist`）
- **THEN** executor 与 evaluator prompt 均含 `## 回溯重做窗口` 与原因原文，且均零 `⚠️ 上次评估未通过`、零标记行 `report` / `checklist` 正文
- **AND** `PhaseNextOutcome.backtrack_window` 仍为该窗口（抑制只作用于 prompt 渲染层，不收缩响应字段面）

#### Scenario: 抑制判据单源

- **WHEN** 审查同源抑制的实现位置与判据形态
- **THEN** 判据在源码中只出现一次，形如 `open_windows.iter().any(|w| w.from_phase == def.id)`，且位于 `build_phase_response` 内（`open_windows` 算出之后、`feedback_suffix` 装配处）
- **AND** 零新增字段（无 `marker_id` 类成员）、零记录 `id` / `timestamp` 比较、`phase_next` 的两处 `latest_phase_entry` 取值与 `error_outcome` 零改动

#### Scenario: 窗口外格保留反馈段

- **WHEN** 决策选 stop / 决策缺席 / 决策越权被拒 / `backtrack` 写面 Err 后用户重新发起运行，失败相位「已决但无 marker」且无任何开放窗口
- **THEN** 该相位重跑 prompt 只含失败反馈段（含 `（attempt N）` 与失败项 / `report` 正文），零 `## 回溯重做窗口`；`fail_feedback_suffix` 因此保留为活代码

#### Scenario: 同源窗口在层级裁剪后仍存活

- **WHEN** 当前相位命中 `MAX_BACKTRACK_WINDOWS + 1` 个同时开放窗口，且其中之一的 `from_phase` 就是当前相位（同源窗口创建序最末、`from_index` 最小）
- **THEN** 该同源窗口恒在渲染集合内（`take_last` 保留末位创建者），失败反馈段照常被抑制——抑制不因裁剪而失效

#### Scenario: 抑制只发生在渲染层

- **WHEN** 审查同源格的响应字段面与决策输入
- **THEN** `backtrack_window` / `last_result` / `allowed_backtrack_phases` / error 两变体 / done 响应逐字不变；决策 prompt 仍携 fail checklist、`fail_report` 与窗口段（决策面不消费本抑制），抑制的唯一可观测面是相位 prompt 少一段

## Module Contract

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|----------|
| `crates/core/workflow/src/write/phase_next.rs` | 同源抑制单点 | `build_phase_response` 内加 `let same_origin = open_windows.iter().any(\|window\| window.from_phase == def.id);` 与 `feedback_suffix` 的 `.filter(\|_\| !same_origin)`；`open_backtrack_windows` / `backtrack_window_section` / `fail_feedback_suffix` 逐字不变；`PhaseNextOutcome` 字段面、路由判定、`error_outcome`、`done` 响应零改动；`build_phase_response` / `fail_feedback_suffix` 的 doc 注释各同步一句 |
| `crates/core/workflow/src/write/phase_next_test.rs` | 三态语料 | 补 `回溯窗口四态_上界源头相位命中`（零反馈段 + 标记行正文零命中）与 `回溯窗口四态_区间内部相位双角色携背景`（反馈段仍在）；新增三态矩阵用例与「同源窗口过裁剪仍存活」用例；`窗口段位置先于失败反馈段` / `零窗口回归` / 自环 / 跨 run / 两分支同形用例逐字不动 |
| `crates/core/workflow/src/write/phase_table.rs`（零改动） | 常量区 | `MAX_BACKTRACK_WINDOWS` / `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 逐字不变（本变更零新常量） |
| `crates/core/workflow/src/write/mod.rs`（零改动） | 出口面 | `BacktrackWindowEntry` / `backtrack_window_section` 等再导出不变；无新类型、无新函数 |
| `crates/core/workflow/src/write/backtrack.rs`、`crates/infra/store/**`（零改动） | 落账裁定与载体 | 白名单二次校验 / 表位校验 / `reason ≤ 1000` / stale 传播 / `apply_change_backtrack` 的「改写信面最新条目」逐字不变；同源是读面派生，不改变写面裁定 |

### Function: `crates/core/workflow/src/write/phase_next.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `build_phase_response(table, def, round, name, entries, last_result, retry_feedback)` | 改（新增一处条件） | 签名不变；`open_windows` 之后求 `same_origin`，`retry_feedback` 经 `.filter(\|_\| !same_origin)` 后才进 `fail_feedback_suffix`；`window_section` 与 `backtrack_window` 取值不变 |
| `open_backtrack_windows(entries, table, phase_id)` | 零改动 | 区间 + 未闭环 + 末位裁剪三判据逐字不动（同源判据读其输出，不读其内部） |
| `backtrack_window_section(windows)` | 零改动 | 文案、标签、只取 `reason` 的渲染纪律不变 |
| `fail_feedback_suffix(record)` | 零改动 | 失败项优先 / 全 pass 清单配 fail verdict 全量随行 / 清单空退回 `report` / `（attempt N）` 头全部保留；消费者从「所有 fail 重试」收窄为「非同源格的 fail 重试」 |
| `phase_next(...)` | 零改动 | 两处 `latest_phase_entry`（`latest_fail` 与 `retry_feedback`）、`fail_decided` 幂等闸、两道分叉判断、`backtrack_to` 分支全部逐字不变 |

### API: `PhaseNextOutcome` / `BacktrackWindowEntry`

| 项 | 变化 | 描述 |
|---|---|---|
| `PhaseNextOutcome.backtrack_window` | 零改动 | 五字段、创建序升序、空集语义、正常 / error 两处填充均不变 |
| `PhaseNextOutcome` 其余字段 | 零改动 | `executor` / `evaluator` 的 `prompt` 文本是唯一可观测变化（同源格少一段），`steps` / `artifact_manifest` / `allowed_backtrack_phases` / `last_result` / `error` 不变 |
| bindings | 零变化 | `PhaseNextOutcome` 不出 bindings，`bindings:export` 零 diff |

### CLI / Component: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| CLI / IPC 命令面 | 零改动 | 无新命令、无签名变化；`phase_next` 不在 CLI 出线 |
| 前端 `packages/desktop/src/**` | 零改动 | 无新类型、无新 UI；prompt 文本变化不投影到任何视图 |
