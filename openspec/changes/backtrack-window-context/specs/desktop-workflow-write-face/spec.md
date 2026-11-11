# desktop-workflow-write-face Specification（delta: backtrack-window-context）

## MODIFIED Requirements

### Requirement: backtrack 白名单二次校验与 stale 语义

写面 `backtrack` SHALL 对 `backtrack_to` 做白名单二次校验：`backtrack_to` 不在 `allowed_backtrack_phases` 内 SHALL 被拒绝且不落任何账（越权决议损坏不了状态，状态库零变更）；校验通过 SHALL 落回跳记录、对被跳过相位的 PhaseRecord 打 stale 标记并传播（与回跳落账同一事务语义），`reason` 超过 1000 字符 SHALL 被拒绝。`backtrack_to` 与发起相位相同（自环）SHALL 在白名单含该相位时正常受理——这是「重做本相位」的唯一载体（白名单自含规则见「相位定义有序 step 序列与 artifact 清单」）：落账语义为改写最新条目 `backtrack_to` + 目标相位下游 stale 传播，`phase_next` 随后以新 attempt 路由回该相位；白名单不含自身时（零 agent 步相位）自环 SHALL 被拒且状态库零变更。该口径为桌面写面自有裁定：MUST NOT 沿用「目标索引必须小于当前相位」类旧约束——承载该约束的对偶 spec 已随插件退役整目录删除，桌面路径只受本条白名单规则约束，回跳合法性的唯一判据是 `allowed_backtrack_phases`。

一次成功回溯 SHALL 恰好产出一个**回溯窗口记录**，由既有条目派生、零新表 / 零新列 / 零新时钟：记录形状 = { `from` = 标记行自身相位（发起相位，泳道位置靠后，区间上界）、`to` = 标记行 `backtrack_to`（目标相位，泳道位置靠前，区间下界）、`reason` = 标记行 `backtrack_reason` 原文逐字、`order` = 标记行 `PhaseStateRecord.id`（落账序）}。标记**不是新增行**，而是改写「发起相位最新条目」的 `backtrack_to` / `backtrack_reason` ⇒ 窗口不引入持久化载体，MUST NOT 为此新增列（如决策时刻列 / 窗口号列）或另建窗口表。`order` 取 `id` 的充分性由发起相位恒为「刚失败的相位」保证：其 fail 条目是决策前最后一次落账，故标记行 `id` 在每次回溯时都是全库最新，多个标记天然按决策先后递增。

#### Scenario: 白名单内回跳落账

- **WHEN** 以 `backtrack_to="test-gen"`（白名单内）、合法 reason 调用写面 backtrack
- **THEN** 回跳记录落库、目标相位可被 `phase_next` 路由至、被跳过相位的 PhaseRecord 带 stale 标记

#### Scenario: 目标为自身即重做本相位

- **WHEN** 以 `backtrack_to` 等于发起相位（如 `phase="implement"` / `to="implement"`，白名单含自身）调用写面 backtrack
- **THEN** 回跳落库成功、`phase_next` 随后路由回 implement 的新 attempt（重做本相位）；以 `phase="test-execution"` / `to="test-execution"`（白名单不含自身）调用时显式失败且状态库零变更

#### Scenario: 越权目标被拒绝

- **WHEN** 以 `backtrack_to` 不在 `allowed_backtrack_phases` 内调用写面 backtrack
- **THEN** 调用显式失败（错误抵达调用方），状态库零变更（无新行、无 stale 翻转、active_phase 不动）

#### Scenario: reason 超长被拒绝

- **WHEN** 以超过 1000 字符的 reason 调用写面 backtrack
- **THEN** 调用显式失败，状态库零变更

#### Scenario: 自环裁定单源自持

- **WHEN** 审查桌面写面 `backtrack` 的合法性判据
- **THEN** 判据只有 `allowed_backtrack_phases` 白名单（含自环三态：白名单内受理 / 不含自身拒绝 / 越权拒绝零落账）
- **AND** 无「与插件侧 spec 各管其面 / 插件零改动」类对偶条款或注释（对偶 spec 已删除）

#### Scenario: 窗口记录零持久化载体

- **WHEN** 对一次成功回溯审查落库形态与 `PhaseStateRecord` 字段面
- **THEN** 状态库新增的仅为「发起相位最新条目的 `backtrack_to` / `backtrack_reason` 改写 + stale 翻转」，无新表、无新列、无窗口号字段；窗口的四元 {`from`, `to`, `reason`, `order`} 全部可从既有条目直接读出

#### Scenario: 多个标记按决策先后递增

- **WHEN** 先后两次回溯（发起相位不同）落账后比较两条标记行的 `id`
- **THEN** 后一次回溯的标记行 `id` 更大（发起相位是各自时刻的刚失败相位，其 fail 条目为决策前最后一次落账）

### Requirement: PhaseAgentSpec 静态化与通配符退役

`PhaseAgentSpec` SHALL 由 `{ agent_type: String, prompt: String, model_level }` 收敛为 `{ prompt: &'static str, model_level }`：`agent_type` 字段随 `__CALL_AGENT:<role>__` 令牌退役删除；prompt SHALL 为编译期静态文本（`include_str!("prompts/<role>.md")` 产物，见 desktop-phase-prompts）。写面 SHALL 删除 `interpolate`（`<change>` / `<phase>` 占位符插值）；`phase_next` 的 `build_phase_response` SHALL 以静态 prompt + 上下文头 `change: <name>`（append）+ 回溯重做窗口段（append，见「回溯重做窗口推导与 prompt 携带」）组装，MUST NOT 做模板替换。回溯场景 MUST NOT 再以「单行回溯原因」形式追加（该行已由窗口段完全取代，单点渲染）。desktop core（workflow + orchestration）SHALL 零残留 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>`。

#### Scenario: 结构收敛与令牌退役

- **WHEN** 审查 `PhaseAgentSpec` 定义与 `phase_table` 表条目
- **THEN** 结构仅含 `prompt: &'static str` 与 `model_level`；无 `agent_type` 字段；无 `__CALL_AGENT` 字样
- **AND** 相位序 / model_level / 白名单 / 前置依赖语义不变

#### Scenario: 无模板替换

- **WHEN** `phase_next` 下发某相位 executor / evaluator prompt
- **THEN** prompt 主体为静态 md 文本，仅 append 上下文头 `change: <name>` 与（窗口命中时）回溯重做窗口段
- **AND** 无 `<change>` / `<phase>` 替换行为

#### Scenario: 删除面零残留

- **WHEN** 全仓扫描 desktop core（workflow / orchestration）源码
- **THEN** 无 `__CALL_AGENT` / `interpolate` / `strip_call_agent` / `role_brief` / `<change>` / `<phase>` 残留

#### Scenario: 单行回溯原因行零残留

- **WHEN** 全仓扫描 desktop core（workflow / orchestration）产品源码中的回溯原因渲染点
- **THEN** 无 `⚠️ 回溯原因` 字样、无 `reason_suffix` 函数；`build_phase_response` 亦无 `backtrack_reason` 一次性入参（窗口由响应内的窗口集合统一供给，回溯分支与正常分支入参收敛为同一形）

## ADDED Requirements

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

判据 MUST NOT 读取 `run_id` / `SessionAnchors`（跨 run 继承由此天然成立）。边界 SHALL 零特判：含源头相位（`P = from` 落在区间上界，最需要知道上次因何而挂）、自环回溯（`to == from` 退化为单点区间，与常规回溯同一条代码，MUST NOT 写 `if to == from` 特判）、嵌套回跳（多窗各自独立开放，下游相位可同时命中；内层回跳到更上游后外层当前相位落到区间外即不再拼，回放进区间即再拼——由区间判据自愈，MUST NOT 另设暂停 / 恢复状态）。

文案 SHALL 由单点常量承载（经 `write/mod.rs` 再导出供 orchestration 复用，MUST NOT 有第二份副本）：标题 `## 回溯重做窗口（背景信息）` + 一段纯陈述说明（按发生先后排列、最上面最早、保留原始换行与标点、未做截断或改写、属背景不构成额外验收条件）+ 逐条以 `<backtrack reason>` / `</backtrack>` 定界的**原因原文**。标签行 SHALL 零属性（`from` / `to` / 顺序号一律不渲染）；渲染 MUST NOT 取自 `reason` 之外的任何字段，MUST NOT 出现任何相位 id 字面量。原因 MUST NOT 被截断、压平或 trim（含首尾换行与标点逐字保留）。

位置 SHALL 为契约而非排版偏好：executor / evaluator 的窗口段 SHALL 原地取代被删除的单行回溯原因（静态主体之后、失败反馈段**之前**）；evaluator 的最终序 SHALL 为 `上下文头 → 静态主体 → 窗口段 → 失败反馈段 → 输出协议附录`。MUST NOT 置于失败反馈段之后（反馈段是「本相位这次的失败清单」，窗口段是「为什么这个相位被叫回来」，后者是前者的上位语境）。

体积 SHALL 确定有界：单相位窗口段 ≤ `MAX_BACKTRACK_WINDOWS` ×（标签与说明行 + `MAX_REASON_CHARS`）≈ 2.1K 字符。实现 MUST NOT 因体积顾虑再加截断。

#### Scenario: 窗口内每个相位每个 agent 步都携带背景

- **WHEN** 决策回溯 `to=dev-design` / `reason=R` 后流程经 dev-design → test-design → implement → test-gen 依次推进
- **THEN** 四个相位的 executor 与 evaluator prompt 均含 `## 回溯重做窗口` 段与 R（无一相位丢失），窗口外相位（如 code-review）零该段

#### Scenario: 含源头相位

- **WHEN** 流程推进到发起相位（如 test-gen）本身并请求其 prompt
- **THEN** 该相位 prompt 仍含窗口段（`to_index ≤ col(P) = from_index`）；该相位落账新条目后（`id` 大于标记行）窗口即闭环，其后续 prompt 零该段

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

- **WHEN** 一个落在窗口内的相位同时带 fail 重试反馈段
- **THEN** prompt 中窗口段索引小于失败反馈段索引；evaluator 的输出协议附录仍在最后

### Requirement: 回溯窗口与轮次预算的口径分层

写面的「本轮」口径 SHALL 显式分两层且 MUST NOT 混用：

- **预算层**：`round` / `MAX_ROUNDS` / 窗口 fail 计数按**单次 run 窗口** `entries[anchor..]` 计算，语义 = 单次运行的循环预算，重启换新预算（既有语义，逐字不变）；
- **背景层**：回溯窗口按 **db 事件序全量 entries** 计算，语义 = 一次回跳的语义窗口，跨 run 直到闭环。

两层的差异 SHALL 是有意设计而非疏漏：把背景层判据写成读 `anchor` 会把跨 run 的未闭环窗口误判为已闭环（正是本能力要修的现象）。实现 MUST NOT 以任何形式的「统一口径」把两层合并。

#### Scenario: 两层口径并存且互不侵蚀

- **WHEN** 同一 change 在新 run 中路由某相位（`entries[anchor..]` 为空 ⇒ `round = 1`）
- **THEN** 预算层按新 run 窗口计数（`round = 1`、窗口 fail 数从 0 起），背景层仍按全量条目命中未闭环窗口；两者取值不同却同时对同一响应的字段面成立

#### Scenario: 判据不读锚点

- **WHEN** 审查窗口推导函数的签名与函数体
- **THEN** 入参只有条目切片、相位表与目标相位 id；无 `SessionAnchors` / `run_id` / `anchor` 参与，零 `entries[anchor..]` 切片

## Module Contract

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/workflow/src/write/phase_next.rs` | 窗口推导与 prompt 携带单点 | `pub struct BacktrackWindowEntry { from_phase: String, from_index: usize, to_phase: String, to_index: usize, reason: String }`（`Debug` / `Clone` / `PartialEq` / `Eq`；非 `Serialize`——不出 bindings）；`PhaseNextOutcome.backtrack_window: Vec<BacktrackWindowEntry>`；窗口推导 fn（入参 `(&[PhaseStateRecord], &'static [PhaseDefinition], phase_id)`，输出按创建序升序；≤ `MAX_BACKTRACK_WINDOWS` 全量并集，超限取创建序末位两层、裁掉 `from_index` 最大那条）；窗口段渲染 fn（入参窗口切片，输出 `String`，只取 `reason`）；`build_phase_response` 删 `backtrack_reason` 入参、窗口段插在原单行原因处；`error_outcome` 填覆盖失败相位的窗口 |
| `crates/core/workflow/src/write/phase_table.rs` | 窗口层级上限常量 | `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2;`（与 `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 同区；prompt 文案与测试断言引用常量，零写死数字） |
| `crates/core/workflow/src/write/mod.rs` | 出口面 | `pub use phase_next::{…, BacktrackWindowEntry}` 与窗口段渲染 fn 的再导出（供 orchestration 复用同一文案单点，零第二份副本） |
| `crates/core/workflow/src/write/backtrack.rs`（零改动） | 落账裁定 | 白名单二次校验 / 表位校验 / `reason ≤ 1000` / stale 传播逐字不变；窗口是读面派生，不改变写面裁定 |
| `crates/core/workflow/src/write/phase_next_test.rs` | 窗口判据语料 | 四态（目标 / 中间 / 源头 / 窗外）、自环单点、嵌套双窗并集、三层末位裁剪、内层回跳、跨 run 继承、error 响应窗口、原因原文逐字保真、渲染零相位 id、求值顺序、S2 断言改写 |
| `crates/core/workflow/src/queries/detail.rs`（零改动） | 泳道序真值源 | `pipeline` 仍按 `phase_table` 序投影（`col` 的取值来源）；本能力只消费该序，不改其构造 |
| `crates/infra/store/**`（零改动） | 落库载体 | `PhaseStateRecord` 字段面不变；`list_phase_records` 的 id 升序语义即创建序来源；零 schema / 零迁移 |
