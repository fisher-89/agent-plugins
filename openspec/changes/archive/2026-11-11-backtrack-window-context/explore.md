# 回溯背景的跨相位携带（backtrack window context）

workflow_type: requirement

> 用户诉求：workflow backtrack 到某个历史 phase 后，只有一处 prompt 携带 backtrack 原因；
> 顺序执行的后续 step 不了解本轮运行的特殊性。在 backtrack 闭环前（回到 backtrack 源头
> phase），中间 agent step 的 prompt 应携带该背景。

---

## 0. 结论先行

机制判断正确，范围可再收紧一层：

- **携带范围 = 回溯窗口内每个相位的全部 agent 步**（executor + evaluator 都带），起于目标相位、
  **含源头相位**（闭环最后一格），止于源头相位落账；
- **丢失点是相位边界**：目标相位之后、回到源头相位之前的每一个相位，prompt 零回溯背景；
- 相位**内部**不丢——反馈修复走 `continue_session`，原因留在会话史里。

已裁决（2026-11 用户定，18 条全见 §6）：

- ① 含源头相位；② A2 多窗并集 + **上限 2 层（超限取末位两层）**；③ **跨 run 继承**；
- ④ 决策 prompt 也带（**降级说明见 4.11**）；⑤ archive 的 spec-sync / resolve-conflicts 不带；
- ⑥ **S2：窗口段完全取代既有原因行**；⑦ 窗口表达 = 每条未闭环回溯记录起止相位，
  拼接按区间、堆叠按创建序；
- ⑧ 文案 XML 标签定界 + 原因**不截断不压平**；⑯ **from / to 对 agent 不可见，只渲染原因**；
- ⑰ 文案位置 = **与既有回溯原因信息同位**；⑱ 文件大小**建议不拆**（由变更自决）。

裁决 ③ 使「沿用 anchor 窗口」论据失效；⑦ 使「内层是否暂停外层」不需另设规则（区间判据自愈）；
② 的 2 层上限使体积从不可预算变为确定有界（≤≈2.1K 字符），代价是深层嵌套时恒裁最旧最宽层；
⑯ 使渲染退化为纯原因清单（from/to 仍服务判据与测试，只是不给 agent 看）。由此形成两层
「本轮」口径（预算层按 run、背景层跨 run）。

---

## 1. 现状链路（db 单源，只在一次路由分支被读取一次）

```
决策会话（orchestration/walker.rs:3328）
  DecisionAction::Backtrack { to, reason }
      │ ToolCommand::Backtrack（白名单随行）
      ▼
写面 backtrack（workflow/write/backtrack.rs:93）
  校验（白名单 / 表位 / reason ≤1000）→ BacktrackCommand { phase:发起, to, reason, stale_dependents }
      ▼
store 单事务（infra/store/src/store.rs:1704-1706 / 1708-1722）
  发起相位最新条目 .backtrack_to = to / .backtrack_reason = reason
  目标相位最新 pass 置 stale；stale_dependents 闭包全条目置 stale
      ▼
下一轮 phase_next（workflow/write/phase_next.rs:134-156）
  全局最新条目 backtrack_to 在场 → build_phase_response(backtrack_reason = entry.backtrack_reason)
      ▼
prompt = context_header("change: <name>") + 静态角色体 + "⚠️ 回溯原因: R"
```

`reason_suffix` 由 `resolve` 闭包同时用于 `executor` / `evaluator`（phase_next.rs:343-353），
故目标相位两个角色都在场；正常路由分支（phase_next.rs:218-228）把 `backtrack_reason`
直传 `None`，零携带。

窗口示意（requirement 表举例）：

```
test-gen#1 FAIL ──决策──▶ backtrack(to=dev-design, reason=R)
                              ┌──────────────── 重做链（回溯窗口）
                              ▼
dev-design#2   executor ✅R  evaluator ✅R      ← 目标相位：有背景
test-design#2  executor ❌   evaluator ❌      ← 丢失点 1
implement#2    executor ❌   evaluator ❌      ← 丢失点 2
test-gen#2     executor ❌   evaluator ❌      ← 源头相位重跑，无背景（丢失点 3）
code-review    …… 窗口已过
```

窗口为何“自动结束”：目标相位重跑落新条目后成为全局最新条目（backtrack_to=None），
回溯分支自然不再命中——**标记不是被清理，而是被更新的最新条目盖过**。

---

## 2. 代码锚点

| 面 | 位置 |
|---|---|
| 回溯分支读取 reason | `core/workflow/src/write/phase_next.rs:134-156` |
| reason 并入 prompt（单点） | `core/workflow/src/write/phase_next.rs:343-353` |
| 正常路由零 reason | `core/workflow/src/write/phase_next.rs:218-228` |
| 「本轮窗口」既有口径（anchor + `entries[anchor..]`） | `phase_next.rs:119-123`（round）、`181-192`（window_fails） |
| 决策产出 backtrack 并落账 | `core/orchestration/src/walker.rs:3328-3385` |
| 只给目标相位入口边上 backtrack kind | `walker.rs:306-314`（entry_kind）、`3383`（trace.incoming） |
| 决策有界输入（无窗口背景） | `walker.rs:3543` `build_decision_input` |
| 标记落库与 stale 传播 | `infra/store/src/store.rs:1704-1706`、`1708-1722` |
| 同相位反馈续注（原因不丢的原因） | `walker.rs:1497`（archi）、`1664`（verdict）、`938`（static-check） |
| 归档 agent 步另有 prompt 组装（spec-sync / resolve-conflicts） | `core/orchestration/src/prompt.rs:138` / `162` |
| 图面回溯边标签（前端取发起条目记因） | `src/views/changes/flow/graph.ts:15-25, 51` |

`PhaseNextOutcome` 不出 bindings（`src/types/generated/` 无该类型）——写面改造无 TS 联动。

---

## 3. 同源缺口（顺带量到）

1. **决策面**：窗口内二次失败时，`build_decision_input` 不含“已在某窗口内”的事实，
   可能重复朝同一目标回跳，靠 `MAX_ROUNDS=20` 兜底。（裁决 ④：本次一并补，见 4.3）
2. **图面**：只有目标相位入口那一条边是 `backtrack`，窗口内其余全 `forward`，
   用户看图无法辨识“同一轮重做”。（本次不动，留待后续）
3. **归档 agent 步**：spec-sync / resolve-conflicts prompt 与 phase prompt 是两套组装，
   窗口背景与二者都无关（且 archive 恒在窗口外）——明确排除（裁决 ⑤）。

---

## 4. 定稿方案：单条未闭环回溯记录 = 一个窗口（A2 / 跨 run 继承 / S2）

### 4.1 一条回溯记录 = 一个窗口（起止相位 + 创建序）

```
记录形状（自既有条目派生，零新表 / 零新列）

  W = { from: S, to: T, reason: R, order: <创建序> }
      S = 标记行自身的 phase（发起相位 = 回溯动作发出者，泳道位置靠后）
      T = 标记行的 backtrack_to（回跳目标 = 被打回的那一格，泳道位置靠前）
      R = 标记行的 backtrack_reason（原文逐字）
      既有过滤沿用：backtrack_to 非空且 trim 非空

判据一 · 是否拼接（起止相位区间，**泳道 index**）
  connect(P, W) ⟺ col(W.to) ≤ col(P) ≤ col(W.from)      ← 与「从 from 回溯至 to」同向

  序 = 渲染泳道 index，三处严格同一（已核实）：
    detail.rs:287-293  pipeline = phase_table.iter().map(...)   ← 后端由表生成
    graph.ts:71-75     columns = detail.pipeline.map((e, i) => colIndex: i)
    phase_next.rs      表序 = phase_table position
    ⇒ colIndex ≡ pipeline 下标 ≡ phase_table position（同 change 内自洽，
      跨 change 比较无意义——窗口判据只在同 change 内使用）

判据二 · 是否未闭环
  open(W) ⟺ ∄ S(发起相位) 的条目 e 满足 e.id > marker_id
              （发起相位还没重跑过 ⇒ 窗口没走完）

判据三 · 堆叠顺序与层级上限
  H(P) = 连接 ∧ 开放的窗口，按创建序升序（最早在前，A2 并集；不互相暂停、不互相覆盖）
  渲染集合 = if |H| ≤ MAX_BACKTRACK_WINDOWS { H } else { H[|H|-MAX_BACKTRACK_WINDOWS..] }
            MAX_BACKTRACK_WINDOWS = 2（末位两层，见 4.8）

背景(P) = take_last(MAX_BACKTRACK_WINDOWS) ∘ sort(创建序) ∘ filter(open) ∘ filter(connect P)
```

- 相位序**单源 = 渲染泳道 index**（`detail.pipeline` 下标 ≡ `phase_table` position，
  见判据一注释），MUST NOT 另引 `prerequisites` 之外的序；标记序用条目
  `PhaseStateRecord.id`（落账序；单调性论证见 4.6）。
- **判据不读 run_id / anchor** ⇒ 跨 run 继承（裁决 ③）天然成立。
- **「内层是否暂停外层」不需另设规则**：内层回跳到更上游后，当前相位落到外层
  `to` 之前 ⇒ 判据一不命中，外层自然不拼；回放到区间内时外层再出现——区间判据
  一个条件治了「暂停」与「恢复」两件事。
- 三处边界零特判：
  - **含源头相位**（裁决 ①）：P = 发起相位 S 命中区间上界 `from`；S 落账新条目后判据二关窗。
  - **自环回溯**（T == S，重做本相位）：区间退化为单点（`to_index == from_index`），
    只在 S 自身拼；落账即关。与常规回溯同一条代码。
  - **嵌套回跳**：多窗口各自独立开放，下游相位可同时命中（A2 的实质价值）；
    命中超过 `MAX_BACKTRACK_WINDOWS=2` 时取末位两层（见 4.8）。

### 4.2 两层「本轮」口径（有意不同，非疏漏）

```
┌ 预算层（round / MAX_ROUNDS=20 / window_fails）── 按 run 窗口：entries[anchor..]
│   语义：单次运行的循环预算，重启换新预算（phase_next.rs:119-123, 181-192）
└ 背景层（回溯窗口）────────────────────────── 按 db 事件序：全量 entries
    语义：一次回跳的语义窗口，跨 run 直到闭环（本次新增）
```

### 4.3 单点位置：`PhaseNextOutcome` 新字段（正常响应 + error 响应都填）

walker 不直读 store（路由权威唯一、经 `ToolStepPort`），故背景由写面随响应下发，
orchestration 不二次推导：

```rust
pub struct BacktrackWindowEntry {
    /// 发起相位（回溯动作发出者，泳道位置靠后）—— 窗口区间上界；**不进 prompt**
    pub from_phase: String,
    /// 发起相位泳道 index
    pub from_index: usize,
    /// 目标相位（被打回的那一格，泳道位置靠前）—— 窗口区间下界；**不进 prompt**
    pub to_phase: String,
    /// 目标相位泳道 index
    pub to_index: usize,
    /// 标记行 backtrack_reason 原文（逐字保留：不截断、不压平、不 trim，见 4.9）
    /// **prompt 只渲染这一个字段**
    pub reason: String,
}
// 拼接判据：to_index ≤ col(P) ≤ from_index   ← 与渲染句「从 from 回溯至 to」同向
// 堆叠序 = Vec 顺序（创建序升序，最早在前）

PhaseNextOutcome
  + backtrack_window: Vec<BacktrackWindowEntry>
     正常响应（build_phase_response）：填「覆盖 next_phase 的开放窗口」（创建序升序）
     error 响应（error_outcome）    ：填「覆盖失败相位的开放窗口」← 决策输入取此处

消费方
  build_phase_response → executor / evaluator prompt 追加「回溯重做窗口」段（S2：取代既有原因行）
  decision_prompt      → DecisionInput.backtrack_window 渲染段（同一文案单点）
  图面 / 前端 / 存档    → 零改动（index 随行备而不用，为将来图面标窗口留口）
```

**裁决 16（from / to 对 agent 不可见）**：相位标识属于流程机制、不是 agent 的领域知识；
prompt 只渲染 `reason`（去重后的原因清单）。`from_phase` / `to_phase` / 两个 index 仍留在
结构体里——它们是**判据与测试的输入**（拼接区间、堆叠序、裁剪位置断言），只是不进渲染。

**命名定案**：`from_phase` / `to_phase`（与渲染句「从 from 回溯至 to」同向）+
`from_index` / `to_index`（泳道序，`to_index ≤ col ≤ from_index`）。**弃用上一版的
`window_start` / `window_end`**——那是泳道位置的读法，与渲染方向相反，两套方向并存即
埋反向映射坑。

**agent 侧零解析风险**：字段不进 prompt（prompt 是文本拼接，非结构体序列化），
`PhaseNextOutcome` 不出 bindings，命名纯属 Rust 侧可读性。

`PhaseNextOutcome` 不出 bindings（`src/types/generated/` 无该类型）⇒ 新增字段零 TS 联动；
测试 fixture 需补 **6 处**字面量（生产 3 处 + 测试 6 处，共 9 处构造点）：
`phase_next.rs:164 / 244 / 360`（生产）、`port_test.rs:27`、`walker_test.rs:931 / 952 /
967 / 992 / 1770`（测试）。

### 4.3b 探究结论：白名单与窗口段不会混淆（原「待定项 ①」已消解）

原疑问：决策 prompt 里 `## 可回溯白名单`（可选项）与 `## 回溯重做窗口`（历史背景）
同时出现，agent 会不会混淆两段语义。

| 面 | 白名单现状渲染 | 窗口段（裁决 16 后） |
|---|---|---|
| 标题 | `## 可回溯白名单（仅可从中选择 backtrack 目标…）` | `## 回溯重做窗口（背景信息）` |
| 内容 | `- <相位 id>（<描述>）：<可操作产物>` | 纯原因文本，**零相位 id** |
| 用途 | 供 agent「选目标」 | 供 agent「知道为什么被叫回来」 |
| 约束力 | 硬（越权被拒） | 零（纯背景） |

⇒ **不混淆**：唯一可能的语义重叠面是相位 id，而裁决 16 已把 id 从窗口段彻底删掉。
两段形状、用途、约束力三者全不同。

**但裁决 16 带出一个新的、更实质的问题**：多窗口覆盖同一相位时，渲染为多条**无主原因**
并列（例：dev-design 同时被两条回溯覆盖 → 两行原因，无归属）。判断：**可接受**——

- 用户裁决原话是「增加回溯原因（待解决的问题）作为补充 prompt」；并列原因即一份
  「待解决问题清单」，agent 无需知道每条由谁提出；
- 归属属于流程机制，不是 agent 该关心的领域知识（同裁决 16 的论据）；
- 若加「以下问题可能来自不同上游相位」来消歧，就又变回元信息（与裁决 15 冲突）。

⇒ 渲染形态见 4.7；判据 / 结构体仍保留 from / to（4.3）。

### 4.4 原因行的处置（裁决：S2 —— 窗口段完全取代）

目标相位现已带单行 `⚠️ 回溯原因: R`（phase_next.rs:343-345）；S2 定案：**删 `reason_suffix`**，
统一由窗口段承载（单点渲染，目标相位那一行即窗口段里自己那一条）。代价清单：

- `phase_next_test.rs`：含 `⚠️ 回溯原因` 的断言 **4 处**（现行号 718 / 1035 / 1084 / 2098，另 705 / 720 / 1036 / 1043 / 1048 / 1085 / 1092 / 2099 为相邻注释与断言说明，随同改写）；
- spec 措辞 2 处（write-face:144/155、orchestration:438/450）；
- `walker.rs:792` 注释（「回溯原因后缀全在写面」字样）。
- 附赠收益：回溯分支与正常分支的 `build_phase_response` 入参收敛（不再有 `backtrack_reason`
  这个一次性入参，改由窗口推导统一供给）。

### 4.5 嵌套场景验算（A2 + 起止区间）

requirement 表泳道 index：`initialize=0 proposal=1 dev-design=2 test-design=3 implement=4
test-gen=5 test-execution=6 code-review=7 acceptance=8 archive=9`

```
场景一 · 双窗并集（两个窗口都开放，下游相位同时命中）

  W1(id=10) from=test-gen(5)     to=dev-design(2)     R1   区间 [2..5]
  W2(id=12) from=test-design(3)  to=proposal(1)       R2   区间 [1..3]

  dev-design(2)：W1 连接 ∧ 开（test-gen 无新条目）；W2 连接 ∧ 开
                 → 渲染 [R1(order=1), R2(order=2)]
  implement(4) ：W1 连接 ∧ 开；W2 不连接（4 > from=3）→ 只 R1
  test-gen(5)  ：W1 连接（落在 from 位）∧ 开；W2 已闭（test-design 已重跑）
                 → 只 R1（含源头相位，裁决 ①）
```

```
场景二 · 内层再回跳 → 外层「跑到上游时不拼、回放进区间后再拼」（零状态机）

  W1(id=10) from=test-gen(5)  to=dev-design(2)  R1   区间 [2..5]
  （dev-design 重跑后，链条推进到 implement 又失败）
  W2(id=20) from=implement(4) to=proposal(1)    R2   区间 [1..4]

  流程被打回 proposal，随后逐相位重推：
  proposal(1)   ：W1 不连接（1 < to=2）→ 外层自动让位，只 W2
                  ← 「跑到上游即不拼」，无暂停标志
  dev-design(2) ：W1 连接(2≤2≤5) ∧ 开（test-gen 无新条目）
                  W2 连接(1≤2≤4) ∧ 开（implement 无新条目）
                  → 两条同时命中，按创建序 [R1(order=1), R2(order=2)]
                  ← 「回放进区间即再拼」，无恢复标志
  test-gen(5)   ：W1 连接（P 落在 from 位）∧ 开；W2 不连接（5 > 4）→ 只 R1
                  含源头相位（裁决 ①）
```

注意 dev-design 同时拿到 R1 与 R2：它既被 test-gen 打回过（R1）、又随 W2 的整体
上游回滚被作废（R2）——这正是 A2 要保住的背景，两者都还没兑现。

判定一律按 `to_index ≤ col(P) ≤ from_index` 机械执行，不靠直觉。
A1 会让 dev-design 只看到 R2，而 R1 讲的恰是它要改的东西——A2 的实质理由。

```
场景三 · 层级上限生效（命中 3 层 → 只渲染末位 2 层）

  W1(id=10) from=test-gen(5)     to=proposal(1)  R1  区间 [1..5]
  W2(id=20) from=implement(4)    to=proposal(1)  R2  区间 [1..4]
  W3(id=30) from=test-design(3)  to=proposal(1)  R3  区间 [1..3]

  proposal(1)：三层全部连接（to=1 ≤ 1）∧ 全开（各发起相位均无新条目）
               |H| = 3 > MAX_BACKTRACK_WINDOWS(2) → 取末位两层
               → 渲染 [R2(order=2), R3(order=3)]，R1 被裁
```

**结构性性质（由创建条件导出，非概率）**：同时开放的多窗口，其 `from_phase` 泳道 index
**严格递减**。理由：要创建 `from=F_j` 的新窗口，流程必须先跑到 F_j，而这要求 F_j 之前
所有相位重新 pass 一遍——凡是 `from_phase` 更早的窗口都会因此关闭。反之若 `F_j > F_i`
则 `F_i` 必然已重跑、窗口 W_i 已关。⇒ 同时开放的窗口按创建序排列时 `from` 单调递减，
最外层（最早、区间最宽）恒在末位截断中被首先裁掉。

**因此**：层级上限的裁剪位置是**确定且必然**的（永远丢掉最旧最宽的那层），不依赖数据
分布；这让「取末位两层」成为一条可断言、可测试的规则，而非启发式。

### 4.6 创建序的取值：为何 `id` 已足够

- 取 `PhaseStateRecord.id`（写事务内 max+1 自增 = 落账序；`store.rs:1634`；
  并列时 `(timestamp, id)` 与 store 侧 `max_by_key` 同口径）。
- **不引入决策时刻列**：标记**不是新增行**，而是改写「发起相位最新条目」的
  `backtrack_to` / `backtrack_reason`（store.rs:1704-1706）。故标记的 `id` 语义
  = 「发起相位末次落账序」，而非「决策时刻」。
- **为什么仍单调**：每次回溯的发起相位恒为「刚失败的相位」（`resolve_deadlock` 的
  `fail_phase` 来自 `PhaseNextError`，walker.rs:3306-3309），其 fail 条目是决策前
  最后一次落账；决策在任一其它相位运行之前发生。⇒ 标记行的 id 在每次回溯时都是
  全库最新，多个标记天然按决策先后递增。自环回溯（重做本相位）同式。
- 结论：**无需新表 / 新列 / 新时钟**，窗口完全由既有条目派生（选 A 而非 C 的核心
  理由）。若将来要求「按绝对时钟排序」，那才需要落库显式记录（路线 C）。

### 4.7 窗口段文案定稿（单点常量，append 在静态主体之后、反馈段之前）

两个裁决叠加后的形态：**裁决 16**（from / to 对 agent 不可见，只渲染原因）+
**裁决「不截断不压平，用文本结构画边界」**⇒ **纯原因清单 + 边界标签，零元信息属性**：

```
## 回溯重做窗口（背景信息）

本 change 发生过下列相位回退，且均尚未回到闭环。这些是此前回溯留下的、需要解决的
问题——原因原文按发生先后排列（最上面最早），保留原始换行与标点，未做截断或改写。
它们是背景，不构成额外验收条件。

<backtrack reason>
设计返工：缺产物区组件
</backtrack>

<backtrack reason>
需求边界未收敛
</backtrack>
```

逐条设计理由：

| 决策 | 理由 |
|---|---|
| 标签名固定 `<backtrack reason>` | 零属性；`reason` 作为标签的第二 token 只用于说明「标签内是原因原文」，不承载可变数据 |
| **属性全部删除**（`from` / `to` / `order`） | 裁决 16：相位标识对 agent 不可见。**顺序由出现先后表达**（说明段已写「最上面最早」），无需 `order` 属性 |
| 保留 XML 标签定界 | Claude 系模型对标签边界识别最稳；标签行可被行首精确匹配；原因原文跨多行也不失边界 |
| **不截断、不压平** | 按裁决保留原因原文；零内容改写 |
| 不用 `---` 横幅 | 原因原文与正文都可含 `---`，撞 Markdown 分隔线语义 |
| 不用 `<<<<<<<` | 本仓库冲突标记词汇，撞语义 |
| 不做「单条退化一行」短式 | 单条与多条同一形状，避免两套渲染各自漂移 |
| 末句「不构成额外验收条件」 | 纯陈述——判断权留给 agent，不加准 AC |

**渲染取值**：每条窗口只取 `BacktrackWindowEntry.reason`；`from_phase` / `to_phase` /
两个 `index` 一律不渲染（它们只服务判据、堆叠序与测试断言）。

**多条原因并列无归属（4.3b 已论证）**：同相位被多窗口覆盖时渲染多条无主原因——
按裁决「增加回溯原因作为补充 prompt」，这正是一份「待解决问题清单」，无需归属。

**残余风险（诚实登记）**：若决策 reason 原文自身含字面 `</backtrack>`，会提前截断该条
记录。属决策 JSON 字符串自由文本，概率可忽略；放转义就等于改写原文，与「不压平」冲突，
故不设防线。

**分段边界**：反馈段（`fail_feedback_suffix`）仍在窗口段之后 append——两者可并存
（窗口内相位自身也可能带 fail 重试反馈），窗口段自带标题与闭合标签，不会被反馈段污染。

### 4.7b 位置复刻：与既有回溯原因信息同位（裁决 17）

已核实既有拼接序（`phase_next.rs:339-357`）：

```
prompt = context_header                  "change: <name>\n\n"
       + 静态角色主体                    spec.prompt
       + reason_suffix                   "\n\n⚠️ 回溯原因: <R>"      ← S2 删除此行
       + feedback_suffix                 fail 重试反馈段
```

⇒ 窗口段**原地替换 `reason_suffix` 的位置**，插在静态主体之后、`feedback_suffix` 之前。
`evaluator_prompt` 在外层再 append 协议附录（`walker.rs:1542`），故 evaluator 的最终序为
`header → 主体 → 窗口段 → 反馈段 → 协议附录`。

**决策 prompt 内部位置（与裁决 17 同款推理）**：既有决策 prompt 段序（`prompt.rs:127-140`）
为 `失败相位报告 → 失败清单 → 可回溯白名单 → 候选报告 → 报告文件 → 决策协议`。
窗口段属「背景信息」，与「失败相位报告」同性质且更宏观 ⇒ 插在 **`## 失败相位报告` 之前**
（段序最前，紧跟开头一句「相位…评估失败，需要你做出路由决策」）。

### 4.8 嵌套最大层级：取命中的末位 2 层

**裁决**：命中窗口数 ≤ 2 时全量渲染；> 2 时只取**命中的末位 2 层**（按创建序最后两条 =
最近的两次回溯），其余丢弃。

```
命中集合 H(P) = 连接 ∧ 开放的窗口，按创建序升序
渲染集合 = if |H| ≤ 2 { H } else { H[|H|-2 ..] }          ← 末位两层
```

**与 A2 的取舍（诚实登记）**：A2 选并集的核心论据是「保住最外层原因」；层级上限与该论据
**直接冲突**——且不是偶发冲突，是结构性冲突（见 4.5 场景三的结构性性质：同时开放的窗口
`from` 严格递减，被裁的永远是最旧最宽的那层）。接受该取舍的理由：

- 深层嵌套（≥3 层回溯未闭环）属异常路径，`MAX_ROUNDS=20` 才是它的硬兜底；
- 「最近的两次回溯」是 agent 当前直接受约束的层级；
- 无上限的真实风险不是信息不足而是**信息过载**：20 条 × ≤1K 字符 ≈ 20K 字符前言会稀释
  静态角色知识的主体比重。
- ⇒ **A2 退化为「≤2 层时的并集」**：多窗并集语义仍在（0/1/2 层全部正确并集），
  只是三层以上降级为末位两层。这是 A2 与上限的共存点，须在 spec 里如实写清。

**刻意不做的**：不做「保留最外层 + 末位」的拼接（顺序语义会分叉成两段）；不做
「首尾各一」的采样（截断位置不可预测，agent 无从理解为什么缺第三条）。

**是否提示被裁掉（裁决：不提示）**：窗口段 MUST NOT 出现任何「另有 N 条未展开」「仅显示最近
两次」类元信息。理由：提示语本身会诱导 agent 追问或自行推测缺失内容；按裁决「取末位两层
足以应对大多数场景」，缺失部分本就不进入判断。

**残余风险（已知并接受）**：不提示会让 agent 形成「历史上只回溯过这两次」的认知，与库中
真实记录不符。该偏差限定在**深层嵌套异常路径**（≥3 层未闭环），正常路径（0/1/2 层）零偏差；
而补一句提示又会引入元信息与追问风险，两害相权取不提示。若日后实测出现 agent 误判历史，
再考虑改为把裁剪落到窗口段标题里（一处措辞，单点可改），不动结构。

**常数单点**：`MAX_BACKTRACK_WINDOWS = 2`，与 `MAX_REASON_CHARS`（1000）同在写面常量区
（`phase_table.rs`）；窗口段渲染与测试断言同源引用，MUST NOT 在 prompt 文案里写死数字。

### 4.9 原因长度：不截断

- **原因不截断**：`backtrack_reason` 长度上限 `MAX_REASON_CHARS = 1000`（写面裁定，
  `phase_table.rs:46`；决策 prompt 引导文案口径 500，`decision.rs:6`）。窗口段逐字渲染，
  MUST NOT 套用 `diagnose_brief` 的 200 字截断（那是节点 detail 面的口径，不是背景面）。
- **体积边界（上限已可算）**：≤2 条 ×（标签行 ~60 字 + 原因 ≤1000 字符）≈ **≤2.1K 字符**。
  层级上限把体积从「不可预算」变成「确定有界」，这是设置上限的主要收益。
- **结构性处理边界**（唯一允许的加工）：整段拼接时保证标签行独立成行，不吞并相邻段；
  原因正文逐字保留（含首尾换行——标签行本身已提供视觉边界，无需 trim）。

### 4.10 文件大小建议（裁决 18：由变更自行决定是否顺便拆分）

先纠正我上一轮的口误：`phase_next.rs` 实际 **374 行**，不是「1900+ 行」。同目录最大文件是
`phase_next_test.rs`（2156 行）、`phase_table.rs`（852 行）。

估算新增量：

```
结构体 BacktrackWindowEntry          ~20 行（含注释）
MAX_BACKTRACK_WINDOWS 常量            ~3 行
窗口推导 fn（区间 + 未闭环 + 裁剪）    ~45 行
窗口段渲染 fn（标签 + 说明段）         ~30 行
────────────────────────────────────────────
合计                                 ~95 行 → 374 + 95 ≈ 470 行
```

**建议：不拆。** 依据——

1. 470 行在同 crate 内属中等（`phase_table.rs` 852 / `create.rs` 459 / 本文件测试 2156）；
2. 窗口推导与 `phase_next` **共享同一份 `entries` 与同一份表序**，拆出去必须把
   `entries` + `table` + `anchor` 当参数跨文件传，耦合不减反增；
3. 拆分会新增 `mod` 声明 + `pub use` 出口 + 测试文件搬迁，收益（文件更短）低于成本；
4. 若真要继续拆分，更值得拆的其实是 `phase_next_test.rs`（2156 行），与本改造无关。

**若变更阶段判断必须拆**：唯一有意义的切面是「窗口判据与渲染整体成模块」
（`write/backtrack_window.rs`），边界干净——输入 `(&[PhaseStateRecord], &[PhaseDefinition],
usize)`、输出 `Vec<BacktrackWindowEntry>` + `String`；MUST NOT 把判据 / 渲染分到两个文件
（会迫使中间产物跨模块往返）。

### 4.11 裁决 ④ 在裁决 16 之后是否还成立（自我质疑）

裁决 ④ 的原始理由（笔记 §3）：决策 agent 不知道「已在某窗口内」，可能**重复朝同一目标回跳**，
靠 `MAX_ROUNDS=20` 兜底。裁决 16 删掉 from/to 后，决策 prompt 里只剩原因文本——
**它还能防止重复回跳吗？**

```
保留 from/to：决策 agent 看到「test-gen→dev-design 已经回跳过」，改选别的目标  ✅
只剩原因：    看到「设计返工：缺产物区组件」，知道这个方向已经试过、失败过  ⚠️
```

结论：**仍然成立，但防重复的机制从「结构判据」退化为「语义理解」**。评估——

- 弱化后的效果仍显著优于现状（现状决策 prompt 完全没有历史回溯信息）；
- 「同一原因再次回跳同一目标」这件事，agent 从原因文本本身就能读出来（原因通常直接点名
  缺失项），退化程度可接受；
- 若日后发现重复回跳仍频发，升级路径已预埋：把 `from_phase` / `to_phase` 加回渲染即可
  （结构体里字段一直都在，只改渲染 fn 一处）。

⇒ 裁决 ④ 保留，但**如实降级说明**，不宣称它与裁决 16 前等价。

### 4.12 备选路线（已否，留档）

- **B. walker 运行期状态**：`EdgeTrace` 增窗口字段，落账处置位、进源头相位清位。优点窗口边界最准；
  代价 prompt 组装从写面单点变两个 append 点，`EdgeTrace` 语义扩张（与 `step-abstraction-model`
  降状态方向相反），且跨 run 继承要额外持久化（与裁决 ③ 直接冲突）。
- **C. 落库显式窗口号**：`PhaseRecord` / `RunRecord` 增 `backtrack_round` / `window_id`。优点图面 /
  决策面 / 复盘面直查、跨 run 可设计；代价 schema 迁移 + corpus/golden 随动 + 写面事务加宽。

---

## 5. 落点清单（未实现）

| 位置 | 动作 |
|---|---|
| `core/workflow/src/write/phase_next.rs` | `BacktrackWindowEntry`（`from_phase` / `from_index` / `to_phase` / `to_index` / `reason`）+ `PhaseNextOutcome.backtrack_window`；`MAX_BACKTRACK_WINDOWS = 2` 常量；窗口推导 fn（`to_index ≤ col ≤ from_index` + 未闭环 + 创建序 + 末位两层截断，见 4.1）；`build_phase_response` / `error_outcome` 填值；窗口段文案单点常量（4.7）；**删 `reason_suffix`（S2）**；位置 = 原地替换（4.7b）。新增约 95 行 → 文件 374 → ~470 行（**结论：不拆**，见 4.10 / 7.7） |
| `core/orchestration/src/prompt.rs` | `decision_prompt` 增「回溯重做窗口」段（`DecisionInput.backtrack_window` 只渲染 `reason`；空则整段省略；复用同一文案单点）；插入位置 = `## 失败相位报告` 之前（4.7b） |
| `core/orchestration/src/walker.rs` | `build_decision_input` 透传 `next.backtrack_window`；`:792` 注释改写（S2） |
| `core/workflow/src/write/phase_next_test.rs` | 目标 / 下游 / 源头 / 越界四态；自环（区间退化单点 `to_index == from_index`）；嵌套双窗（A2 并集 + 创建序）；命中 3 层 → 末位两层（裁最外层，断言 `from` 递减的结构性性质）；内层回跳到更上游后外层不拼（区间判据自愈）；跨 run（新 anchor 仍命中）；error 响应窗口；原因原文逐字保留（含换行 = 不压平）断言；**渲染零 from/to 泄漏断言**；S2 断言改写 |
| `core/orchestration/src/prompt_test.rs` | 决策 prompt 窗口段在场 / 空省略 / 多窗堆叠序 / 标签定界在场 / **段位置断言**（在失败相位报告之前） |
| `port_test.rs` + `walker_test.rs` | **6 处** `PhaseNextOutcome` 字面量补字段（`port_test.rs:27`、`walker_test.rs:931/952/967/992/1770`） |
| `decision_test.rs` + `prompt_test.rs` | `DecisionInput` 新增字段 → 补 2 处完整构造器（`:361` / `:610`），其余 ~19 处用 `..base_input()` / `..base` 自动随动 |
| `desktop-workflow-write-face` spec | 「backtrack 白名单二次校验与 stale 语义」补窗口判据（`to_index..=from_index` / 未闭环 / 创建序 / 跨 run）；「prompt 组装」段改写（删原因行、补窗口段标签结构） |
| `desktop-change-orchestration` spec | 「相位 prompt 组装面静态化」动态 append 清单补一项并改原因行措辞；「决策输入」段补 `backtrack_window`；补两层窗口口径（预算层按 run / 背景层跨 run）自述 |
| 明确排除 | archive 的 spec-sync / resolve-conflicts（`prompt.rs:138/162` 另一套组装）；图面 / 前端 / bindings 零改动 |

---

## 6. 裁决记录

| # | 议题 | 定论 |
|---|---|---|
| 1 | 窗口边界含不含源头相位 | **含**（源头相位最需要知道上次因何而挂；起止区间判据天然给出） |
| 2 | 嵌套回跳 A1 / A2 | **A2 多窗并集**，上限 2 层内为并集、超限降级末位两层（见 #14） |
| 3 | 跨 run 是否继承 | **继承** ⇒ 窗口判据必须改为 db 事件序（anchor 会重置） |
| 4 | 决策 prompt 是否带窗口背景 | **要**（同一份推导；**但裁决 16 后防重复回跳从「结构判据」降级为「语义理解」，如实登记见 4.11**） |
| 5 | archive 两 agent 步 | **不带**（独立 prompt 组装 + 恒在窗口外） |
| 6 | 原因行与窗口段冲突 | **S2：窗口段完全取代原因行**（单点渲染，删 `reason_suffix`） |
| 7 | 窗口表达形式 | **每条未闭环回溯记录起止相位**；拼接按区间、堆叠按创建序 |
| 8 | 内层是否暂停外层 | 无需规则——起止区间判据自动给出「跑到上游即不拼、回放即再拼」 |
| 9 | 命名 | **`from_phase` / `to_phase` + `from_index` / `to_index`**（与渲染句「从 from 回溯至 to」同向；弃 `source`/`target` 与 `window_start`/`window_end` 两版反向命名）；agent 侧零解析风险 |
| 10 | 相位序判据 | **复用渲染泳道 index**（`detail.pipeline` 下标 ≡ `phase_table` position，已核三处同源） |
| 11 | 文案语气 | **仅提供信息供 Agent 判断**（纯陈述，无准 AC 祈使句） |
| 12 | 文案结构 | **XML 标签定界**（`<backtrack reason>` … `</backtrack>`），**零属性**、正文纯原因（属性版随裁决 16 废弃） |
| 13 | 原因含换行 / 超长 | **不截断、不压平**，逐字保留（含首尾换行）；原因上限 1000 字符（写面既有裁定） |
| 14 | 嵌套最大层级 | **`MAX_BACKTRACK_WINDOWS = 2`，取命中的末位 2 层**；体积由此确定有界（≤≈2.1K 字符）。与 A2「保最外层」直接冲突、结构上恒裁最旧最宽层（4.5 场景三）——接受，深层嵌套属异常路径 |
| 15 | 裁剪是否提示 agent | **不提示**（窗口段零「另有 N 条」元信息）；不做「最外层 + 末位」或「首尾各一」的混合采样。代价：≥3 层时 agent 可能误认历史只有这两次——限定在异常路径，接受 |
| 16 | from / to 是否对 agent 可见 | **不可见**。相位标识是流程机制、非 agent 领域知识；prompt 只渲染 `reason`，字段仍留结构体供判据 / 测试（4.3 / 4.3b） |
| 17 | 文案插入位置 | **与既有回溯原因信息同位**（原地替换 `reason_suffix`，静态主体之后、反馈段之前）；决策 prompt 插在 `## 失败相位报告` 之前（4.7b） |
| 18 | 文件大小是否顺便拆分 | **建议不拆**（374 → ~470 行，同 crate 属中等；拆分需跨文件传 entries/table/anchor，耦合不减）。若必须拆，整体成 `write/backtrack_window.rs` 一个模块，MUST NOT 判据/渲染分家（4.10） |

---

## 7. 实现前必读（关键结论与陷阱）

### 7.1 体积从「不可预算」变成「确定有界」

```
无上限（原方案）：20 条 × (标签行 ~60 字 + 原因 ≤1000 字符) ≈ 20K 字符
有上限（裁决 14）： 2 条 × (同上)                          ≈ ≤2.1K 字符
现状对照：单条 reason_suffix（仅目标相位）                    ≈ ≤1K 字符
```

**结论（精确表述，勿夸大）**：上限把最坏情况从 **20K 压到 2.1K（约 1/10）**；相对现状
（≤1K）本改造的最坏增量约 **+1.1K 字符/相位**——同量级、非数量级跃升，且这笔增量换来的
是「窗口内每个相位的每个 agent 步都拿到背景」而非只有目标相位一行。裁决 14 的真实收益
是**把不可预算变成可预算**，实现时 MUST NOT 因「怕变大」再自行加截断（4.9 已定原因不截断）。

### 7.2 A2 的语义被收窄了，spec 必须如实写

**A2（多窗并集）与裁决 14（末位 2 层）在字面上是相反的两句话**：并集说「全都给」，
上限说「只给最近两条」。共存点只有一种正确表述：

```
≤ MAX_BACKTRACK_WINDOWS（2）层：全部并集（0 / 1 / 2 层均正确）
>  MAX_BACKTRACK_WINDOWS    ：取末位 2 层，裁掉最旧最宽的那层
```

**spec 里含糊带过（例如只写「A2 并集」或只写「最多 2 层」）等于留一个自相矛盾的口子**，
后续维护者必然踩。两处 spec（`desktop-workflow-write-face` / `desktop-change-orchestration`）
都须写到这个双分支。

### 7.3 唯一已知且已接受的不精确

不提示裁剪（裁决 15）⇒ 命中 ≥3 层时，agent 会形成「历史上只回溯过这两次」的认知，
与库中真实记录不符。

- **偏差范围**：仅深层嵌套异常路径（≥3 层未闭环）；正常路径（0/1/2 层）零偏差。
- **为何接受**：补提示会引入元信息，诱导 agent 追问或自行推测缺失内容（4.8）。
- **改法已预埋**：若日后实测出现 agent 误判历史，把裁剪说明落到窗口段**标题**即可
  （`## 回溯重做窗口（背景信息，仅显示最近 2 次）`），一处措辞、单点可改，**不动结构**、
  不动判据、不动渲染形状。

### 7.4 结构性性质：被裁的永远是最旧最宽的那层

同时开放的多个窗口，其 `from_phase` 泳道 index **严格递减**（4.5 场景三已推导）：

```
要创建 from=F_j 的新窗口 → 流程必须先跑到 F_j
                        → F_j 之前所有相位都已重新 pass
                        → 凡 from_phase 更早的窗口都已闭环关闭
```

**推论**：层级上限的裁剪位置是**确定且必然**的（永远丢掉最旧、区间最宽的那层），不依赖
数据分布。⇒ 「取末位 2 层」是可断言、可测试的规则而非启发式；测试可直接断言「命中 3 层时
`from_index` 最大的两条出线、最小的那条被裁」。

### 7.5 渲染面与结构面必须分离（裁决 16 的落地纪律）

```
结构面（BacktrackWindowEntry 五字段）        渲染面（prompt 文本）
  from_phase / from_index  ────┐
  to_phase   / to_index    ────┼──► 只取 reason 一个字段
  reason                   ────┘

用途分裂：
  判据 / 堆叠 / 裁剪 / 测试断言  ← 用 from/to/index（全在结构体里，一直可用）
  agent 看到的文本              ← 只有 reason（零相位标识）
```

**MUST NOT 因为「字段都在手上」就在渲染 fn 里顺手带出相位名**——这是裁决 16 唯一的失效
方式，且极难在 review 中发现（文本看着无害）。防御面是测试：窗口段渲染用例必须断言
**prompt 正文里不出现任何相位 id 字面量**（`phase_next_test.rs` / `prompt_test.rs` 各一处）。

**升级路径（已预埋）**：若日后需让决策 agent 看到回溯方向（4.11 的降级说明），只改渲染 fn
一处即可加回 `from` / `to`，结构体与判据零改动。

### 7.6 位置是契约，不是排版偏好（裁决 17 的落地纪律）

窗口段位置不是「放哪好看」，而是**与既有语义同位**：

| prompt | 位置 | 依据 |
|---|---|---|
| executor / evaluator | 原地替换 `reason_suffix`（静态主体之后、`feedback_suffix` **之前**） | 既有原因行就在此处（`phase_next.rs:339-357`）；S2 是替换而非新增 |
| evaluator 最终序 | `header → 主体 → 窗口段 → 反馈段 → 协议附录` | 协议附录由 `evaluator_prompt` 在外层 append（`walker.rs:1542`） |
| decision | `## 失败相位报告` **之前** | 背景比失败明细更宏观；段序最前 |

**MUST NOT 放在 `feedback_suffix` 之后**：反馈段是「本相位这次的失败清单」，窗口段是
「为什么这个相位被叫回来」——后者是前者的上位语境，倒序会让 agent 先读细节后读语境。

### 7.7 文件规模：不拆（裁决 18 的落地结论）

```
现状 374 行（同目录：phase_table.rs 852 / create.rs 459 / phase_next_test.rs 2156）
新增 ~95 行 → ~470 行
```

**结论：不拆。** 关键理由不是行数而是**数据依赖**：窗口判据需要 `entries` + `table` +
`anchor` 三者同时在场，而它们正是 `phase_next` 的既有入参/局部变量。拆文件必须把三者跨模块
传递，耦合不减反增，还多出 `mod` 声明 + `pub use` + 测试文件搬迁三笔账。

**若变更阶段坚持拆分**，唯一可接受的切面是「判据 + 渲染整体成 `write/backtrack_window.rs`」
（输入 `(&[PhaseStateRecord], &[PhaseDefinition], usize)`，输出 `Vec<BacktrackWindowEntry>` +
`String`）；**MUST NOT 把判据与渲染分到两个文件**——那会迫使中间产物跨模块往返，且让
「谁负责裁剪」这个单点语义失去位置。

### 7.8 实现时最易踩的六个陷阱

| 陷阱 | 正确做法 |
|---|---|
| 先截断再排序 | 必须 **filter(open) → filter(connect) → sort(创建序) → take_last(2)**；顺序颠倒会裁错层 |
| 跨 run 继承写成读 anchor | 判据 MUST NOT 触碰 `SessionAnchors`（4.1）；`round` / `window_fails` 是另一层口径（4.2），两者混用会把跨 run 窗口误判为已闭环 |
| 自环回溯当特例分支 | 自环（`to == from`）退化为单点区间，**与常规回溯同一条代码**，MUST NOT 写 `if to == from` 特判（4.1） |
| 渲染时顺手带出相位名 | 渲染 fn 只取 `reason`（7.5）；测试断言 prompt 正文零相位 id 字面量 |
| 窗口段放到反馈段之后 | 窗口段是反馈段的上位语境，必须先于它（7.6） |
| 拆分时判据 / 渲染分家 | 若拆只能整体成模块；判据与渲染同文件（7.7） |
