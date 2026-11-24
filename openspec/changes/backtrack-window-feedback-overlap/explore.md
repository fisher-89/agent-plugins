# 回溯窗口段与重试反馈段的同源重叠

workflow_type: requirement

> **状态：已裁决** —— 分格处置（同源格抑制反馈段 / 窗口外格保留），见 §11。
> 观察起点：会话 `ses-42-1795484424107`（change `archi-model-path-and-silent-trap`，acceptance/evaluator/attempt 2）
> 的 prompt 里前后两段 backtrack 信息，内容指向同一件事（AC-6 冲突），文本不同。
> 结论：**不是两个通道各说一次同一批事实，而是同一条 `PhaseStateRecord` 被两个语义通道各读了一半。**

---

## 1. 一手证据：那一拍的 prompt

`session_event`（sessionId=`ses-42-1795484424107`, seq=1）的 prompt 结构：

```
change: archi-model-path-and-silent-trap
## 角色自述 / 阶段要求 / 静态清单        ← 静态角色 md（acceptance-evaluator.md）
## 回溯重做窗口（背景信息）              ← 段 A：window_section
   <backtrack reason>
   AC-6 与计划冲突且部分不可满足，需改写：(1) crates/** 零命中把测试负向 needle 计入——
   archi_knowledge_test.rs:339 与 :352/:353 的 assert!(!contains) 列表逐字持有禁令短语…
   (2) 基线 openspec/specs 要求命中同行含 MUST NOT，但 delta-only 下不可改基线…
   重写方向：正向表述面零命中，命中仅允许于负向断言与 MUST NOT 禁令句…
   </backtrack>
⚠️ 上次评估未通过（attempt 1），请针对以下失败项修复：   ← 段 B：fail_feedback_suffix
   - proposal 的每个验收标准（目标）均已达成：13 条 AC 中 12 条实测通过：…（同两个病灶、
     同两个文件行号，重复一遍）… AC-6 未达成：archi_knowledge_test.rs:339、:352、:353 …；
     且 AC-6 的 openspec/specs/** 禁令句条款在 delta-only 改动集下无法满足（spec.md:384）…
```

两段的「信息面分解」：

| 信息 | 段 A（窗口 reason） | 段 B（反馈 evidence） |
|---|---|---|
| AC-6 未达成这一事实 | 有（「与计划冲突且部分不可满足」） | 有（「AC-6 未达成」） |
| 病灶①：crates 测试负向 needle | 有 + 行号 `:339/:352/:353` | 有 + 同行号 + 具体命中短语 |
| 病灶②：openspec 基线同行 MUST NOT | 有（泛化表述） | 有 + `spec.md:384` |
| 其余 12 条 AC 的实测证据 | 无 | 有 |
| 修法方向（怎么改） | **有**（「重写方向：…」） | 无 |

⇒ 重复的是「问题识别」，各自独有的是「修法方向」（决策者）与「通过证据 / 命中原文」（评估者）。

---

## 2. 两段各自的代码来源

```
                     ┌─────────────────────────────────────────────────────────┐
PhaseStateRecord     │ id=1379  acceptance  attempt=1  verdict=fail  stale=true │
（同一条记录）        │ report     = 「验收结论：AC-1~AC-5… AC-6 未达成 → fail」  │
                     │ checklist  = [A1 fail, evidence=「13 条 AC 中 12 条…」]   │
                     │ backtrack_to     = "proposal"                           │
                     │ backtrack_reason = "AC-6 与计划冲突…重写方向：…"          │
                     │ decision_session_id = ses-27…                            │
                     └───────────┬───────────────────────────┬─────────────────┘
                                 │                           │
        open_backtrack_windows（判据：区间 + 未闭环）      latest_phase_entry(acceptance)
        from=acceptance → to=proposal                     .filter(verdict == Fail)
                                 │                           │
        backtrack_window_section（只渲染 reason）          fail_feedback_suffix（checklist 失败项）
                                 │                           │
                     段 A：## 回溯重做窗口                 段 B：⚠️ 上次评估未通过
```

- 落账面：`store.apply_change_backtrack`（`infra/store/src/store.rs:1856`）**不是新增行**，而是
  「发起相位最新条目」上加写 `backtrack_to` / `backtrack_reason`（`:1883-1885`）。
  ⇒ 标记行就是那条 fail 条目本身，reason 与 checklist/report 天然同体。
- 窗口推导：`workflow/src/write/phase_next.rs:356-419`（三条判据 + 末位 2 层裁剪）。
- 渲染：`phase_next.rs:419-441`（段 A）、`phase_next.rs:447-470`（段 B）；
  组装顺序 `phase_next.rs:480-500`：`header → 静态主体 → 窗口段 → 反馈段`。

---

## 3. 同源重叠的判据：当前相位恰是**某开放窗口的发起相位**

两个通道的命中条件：

```
段 A 命中  ⟺ ∃ 开放窗口 W：W.to_index ≤ col(P) ≤ W.from_index      （含源头相位 W.from）
段 B 命中  ⟺ latest_phase_entry(P).verdict == Fail
同源重叠  ⟺ P = W.from（源头相位）∧ W 的标记行就是 P 的最新条目
```

把「位置」和「是否同源」摊开：

| 当前相位 P 的位置 | 段 A（窗口） | 段 B（反馈） | 是否同源 |
|---|---|---|---|
| P = to（目标相位，回溯后第一拍） | 有 | 无（走 backtrack 检测分支，`retry_feedback=None`） | — |
| P 在 (to, from) 中间，历史无 fail | 有 | 无 | 否 |
| P 在 (to, from) 中间，历史有 fail 条目 | 有 | 有 | **否**（两条不同记录、两次不同失败） |
| **P = from（源头相位，闭环最后一格）** | 有 | 有（最新条目 = 标记行，fail） | **是（结构必然）** |
| P = from 重跑落新条目之后 | 无（窗口闭环） | 视新条目 verdict | 否 |
| 自环回溯 P = from = to（第一拍） | 有 | 无（backtrack 分支） | — |

⇒ 同源重叠是一个**很窄但完全确定**的窗口：非自环回溯 → 整个窗口重走完 → 回到源头相位的那一拍。
这一拍恰好也是「窗口即将闭环」的最后一拍，所以每条回溯链最多出现一次。

真实轨迹（该 change，phase 表 id 序）：

```
id=1379 acceptance#1 FAIL ──决策 ses-27──▶ backtrack(to=proposal, reason=R1379)
                                          W = [proposal(1) .. acceptance(8)]，marker=1379
id=1380 proposal#4      pass   （窗口内，反馈段：无/异源）
id=1381 dev-design#3    pass
id=1382 test-design#2   pass
id=1383 implement#3     fail   （另一次自环回溯 implement→implement，marker=1383）
id=1384 implement#4     pass   （自环窗口此时闭环）
id=1385 test-gen#3      pass
id=1386 test-execution#3 pass
id=1387 code-review#3   pass
──────▶ ses-42: acceptance 的 prompt = 段 A(1379.reason) + 段 B(1379.checklist)   ★ 同源重叠
id=1388 acceptance#2    pass   （id>1379 ⇒ W 闭环）
id=1389 archive#1       pass   ⇒ done
```

顺带确认：决策会话 `ses-27-1795462539701`（decision prompt）的窗口段为空——决策发生时
本次 backtrack 还没落账，所以决策 prompt 里不会有本次 reason，不构成同源。

---

## 4. 当年设计里有没有意识到？——意识到「并存」，没意识到「同源」

`openspec/changes/archive/2026-11-11-backtrack-window-context/explore.md`

- 4.7「分段边界」原文：
  > 反馈段（`fail_feedback_suffix`）仍在窗口段之后 append——两者可并存
  > （**窗口内相位自身也可能带 fail 重试反馈**），窗口段自带标题与闭合标签，不会被反馈段污染。
  ⇒ 设想的是「目标/中间相位」那类并存（两次不同的失败），即上表第 2/3 行。
- 裁决 ①「含源头相位」的理由原文：
  > 源头相位**最需要知道上次因何而挂**
  ⇒ 但 `backtrack_reason` 不是「因何而挂」（那是 checklist/report），而是**决策者的路由理由与修法方向**；
     「因何而挂」恰在段 B 里。这句表述把两段在源头相位必重复的根因藏了起来。
- 测试固化的是**异源并存**：
  `phase_next_test.rs:2940-2985` 构造「目标相位 dev-design 重跑又 fail」→ 断言
  `窗口段在场 ∧ 反馈段在场 ∧ window_at < feedback_at`，窗口 reason 来自 test-gen、反馈来自
  dev-design —— 两次不同失败。**没有「源头相位同源并存」的用例**（该形态只是判据的自然产物，
  从未被当成设计决策审视）。
- spec：`desktop-workflow-write-face/spec.md:312` 的位置契约把两段语义写成
  「反馈段是『本相位这次的失败清单』，窗口段是『为什么这个相位被叫回来』」——
  在源头相位这两句描述的是**同一件事**。

---

## 5. 量化

- 该 prompt 全文约 6.3K 字符；段 A 约 380 字，段 B 约 600 字；其中重复的「问题识别」约 300 字
  （≈5% 全文、≈300 token 量级），每条非自环回溯链出现一次。
- 同 prompt 体系里还有更大的重复面（同一模式、规模更大）：
  1. **决策 prompt**：`## 失败相位报告`（record.report）与 `## 失败清单`（record.checklist 的
     fail 项 evidence）来自**同一条记录**，事实面几乎逐段复述。
     实例：`ses-27-1795462539701`（acceptance#1 的决策 prompt）——report 里已完整写了 AC-6 两病灶，
     checklist 的 A1 evidence 又写了一遍同样两病灶。
  2. **`## 候选相位最近一次评估报告`**：把 8 个相位的最新 report 全量内联；
     该决策 prompt 里这一段长度是段 A/段 B 的数倍。
- ⇒ 用户注意到的 300 字同源重叠，是「同一条 record 多路渲染」这一模式里最小的一处；
  真正的体积大户在决策输入面（report + checklist + 候选报告三重）。

---

## 6. 若要去重，可能的切面（仅列形状与代价，未实现）

| # | 方案 | 做法 | 代价 / 风险 |
|---|---|---|---|
| a | 区间上界改开区间 | 判据改 `to_index ≤ col < from_index` | 源头相位彻底不带窗口段；但**自环回溯**（to==from）退化为空区间 ⇒ reason 唯一通道消失，必须为自环特判，破坏既有「零特判」不变量，且违背裁决 ① |
| b | 同源去重 | 组装侧判 `open_windows.iter().any(|w| w.from_phase == def.id)` → 抑制 `retry_feedback`（**零新字段**，见 §9 证明） | 需重新定义「省谁」（省 reason 丢修法方向，省 feedback 丢证据与失败清单结构）；需补「源头相位同源并存」用例（当前没有）；跨 run 语义不变 |
| c | 信息分工 | 窗口段只留「修法方向」、反馈段只留「证据」 | reason 是自由文本，无法机械切分；要让决策者另产「方向」字段 ⇒ finish_step schema 变更、算子约束、历史数据迁移 |
| d | 只改文案 | 源头相位时窗口段说明句加注「其中描述你自身的条目即本次回溯来源」 | 零结构改动，零 token 节省；只是降低 agent 的重复感 |
| e | 不动 | 认为「决策者视角 + 评估者视角」双读是有价值的冗余 | 需要把「同源必重复」写进 spec，避免后人当成 bug 反复重开 |

关键约束（~~已修正~~）：上一版笔记断言「消费端无法判定同源、必须补 `marker_id`」——**这是错的**，
`from_phase` 已在结构体里且够用，见 §9。去重不需要任何 schema 变动。

---

## 7. 顺带发现（同一片代码里的其它「多处真相」）

1. **report 上限两个数字**：8 份 evaluator 角色 md 写「report 不超过 500 字符」
   （`prompts/*-evaluator.md`），同一条 prompt 的输出协议附录写 `MAX_REPORT_CHARS = 2000`
   （`orchestration/src/verdict.rs:7`）。agent 在同一篇 prompt 里读到两个上限。
2. 写面 `MAX_REASON_CHARS = 1000`（`workflow/write/phase_table.rs:46`）与决策面
   `MAX_REASON_CHARS = 500`（`orchestration/src/decision.rs:7`）同名不同值：实际卡口是 500。
3. `fail_feedback_suffix` 在「checklist 全 pass 但 verdict fail」的漂移形态下会把全量
   checklist 随行（不虚减），此时段 B 体积不受任何常量约束（evidence 无长度上限）；
   而段 A 有明确上界（2 × 1000）。被「收紧」的一段反而有界。

---

## 8. 待用户裁决的下一步方向

- 只把「同源必重复」登记进 spec / 注释，不动代码（把现状说清楚，避免后续误判为 bug）。
- 做同源去重（方案 b），需要先决定省哪一段、并给结构体补 marker 标识。
- 把镜头拉远，先处理决策输入面的 report / checklist / 候选报告三重重复（体积收益更大）。
- 或者只做文案层澄清（方案 d），成本最低。

---

## 9. 后续讨论：「只保留窗口段」是否可行

### 9.1 先纠正一条成本估计：同源判据不需要新字段

上一版 §6 说「消费端无法判定同源、必须给 `BacktrackWindowEntry` 补 `marker_id`」——错。
`from_phase` 已经在结构体里，且判据自足：

```
① 标记行 entry 恒为「发起相位 S 的 id 最大条目」
     —— open_backtrack_windows 的 open 判据：∄ 同 S 且 id > marker.id 的条目
② latest_phase_entry(P) 按 timestamp 取最大，同相位内 timestamp 与 id 单调同序（写事务 max+1）
③ 若 P == W.from_phase，则 W 在 open 集合内 ⇒ W 的标记行就是 P 的最新条目
⇒ 同源 ⟺ open_windows.iter().any(|w| w.from_phase == P)      （无需 marker id、无需 schema 变动）
```

⇒ 「同源去重」实现成本 = **组装侧一个条件**（抑制 `retry_feedback`），不是结构体改造。

### 9.2 但「只保留窗口段」比「同源去重」覆盖面大得多

段 B（`fail_feedback_suffix`）的触发面与段 A 不重合：

```
段 A 命中 ⟺ P 落在某开放窗口区间内        （窗口存在才命中）
段 B 命中 ⟺ P 最新条目 verdict == Fail    （与窗口无关）
```

三态摊开后，删 B 各格剩什么（**后续已被证伪为两态，见 §10**，此处保留推理过程）：

| 格 | 现状 | 只留 A 后 | 评价 |
|---|---|---|---|
| 同源（P = W.from，来源链尾一拍） | A + B（同一记录两半分） | 只剩 A | ✅ 去重目标，成本 0 |
| ~~异源（P 在 (to,from) 中间且自身有 fail）~~ | ~~A + B（两次不同失败）~~ | — | ❌ **此格不存在**（§10.2） |
| 窗口外（无回溯的 fail 重跑） | 只有 B | **零上下文** | ❌ 空洞，且样本不是我原先猜的 archive（§10） |

`requirement` 表 9 相位中「无 evaluator spec」的只有 `test-execution`（有 `OnFail::Escalate`，
首败即分叉）与 `archive`（无评估步，fail 直接重跑）。⇒ 窗口外格的生产样本 = archive。

### 9.3 支持「同源格删 B」的三条依据（其中一条是实证）

1. **语义**：源头相位 = 该评估者自身。段 B 是它自己上一轮的输出（checklist/report）回灌，
   段 A 是**别人（决策者）**给的新信息。在「重新全量评估」语义下，自己旧输出的边际价值最低。
2. **实证（本 change 链）**：acceptance#2 在拿到段 B 明示「12 条 AC 已实测通过」的情况下，
   仍然全量重跑全部 13 条（只读探针编译 + `nextest -p archi` 1428 + agent-runtime 69 + `api:check`）。
   ⇒ 段 B 的「已过清单」没有被当作跳过依据消费；而段 A 的「重写方向」被 proposal#4 采纳
   （AC-6 改判为三面分判）。**两段的实际消费率不对称。**
3. **体积**：段 A 有确定上界（`MAX_BACKTRACK_WINDOWS × MAX_REASON_CHARS` ≈ 2.1K）；
   段 B **无上界**（`evidence` 无长度校验，「全 pass 清单配 fail verdict」漂移形态还全量随行）。
   只留 A 是这套 prompt 里第一次让失败上下文真正可预算。

### 9.4 保留意见（两格不该跟着删）

- **窗口外**：删 B 后该相位重跑时 prompt = 角色静态 md + header，零失败上下文（§10 给了实证样本与触发族）。
- ~~**异源（中间相位自身 fail）**~~：该格不存在（§10.2）——任何 fail 条目被决策后，
  要么成为某窗口的 `from`（同源格），要么不产生窗口（窗口外格），无第三种。

### 9.5 推荐形状：按信息归属选通道（三态），判据仍只有一行

```
let same_origin = open_windows.iter().any(|w| w.from_phase == def.id);
let retry_feedback = if same_origin { None } else { <现状取值> };
```

- 同源 → 只 A（去重目标达成）
- 异源 / 窗口内 → A + B（保留事实面）
- 窗口外 → 只 B（保留唯一上下文）

成本与「全局删 B」几乎相同（都是 `build_phase_response` 内一处），但零空洞。
`fail_feedback_suffix` / `retry_feedback` 参数都保留，`latest_phase_entry` 的第二处调用点保留。

### 9.6 若坚持全局单通道，需登记的三项代价

1. archive 相位重试零上下文（`fail_feedback_suffix` 函数退化为死代码并删除）。
2. executor 相位重试失去 `- <检查项>：<evidence(file:line)>` 的结构化事实面，只剩决策者散文摘要；
   评估者相位尚可靠全量重评兜底，执行者相位没有等价兜底。
3. `attempt` 计数消失（段 B 是 prompt 内唯一承载 `（attempt N）` 的地方）。

顺手清掉的两处遗留（删 B 的附带收益）：

- `fail_feedback_suffix` 的取值不过滤 `stale`（`latest_phase_entry` 只按 timestamp 取最大），
  理论上可能把已作废的 fail 条目当反馈渲染。
- 「全 pass 清单配 fail verdict」的防御式回退分支（`failing.is_empty()` → 全量随行）一并消失；
  该形态是否可达取决于 verdict 推导单点，需在 spec 侧确认后登记。

---

## 10. 修正（用户追问「窗口外重跑是不是系统异常后重新点运行」）

### 10.1 结论先行

用户的直觉指向了真实现象，但归因需两处修正：

1. **「重开一轮 run」本身不会把相位挪出窗口**。窗口判据不读 `run_id` / `SessionAnchors`
   （跨 run 继承，backtrack-window-context 裁决 ③），每次「发起运行」都是新 `run_id` + 新
   `SessionAnchors`（`change_flow/mod.rs:207`），`round` 归 1，但窗口照旧命中。
2. **真正的窗口外戳记是「决策已挂账、却没产出 backtrack」**：`decision_session_id` 由
   `decision-log` 在**动作裁决之前**挂账（`walker.rs:3194-3210`，注释原文「置于动作裁决之前——
   stop / 驳回路径同样留痕」）。于是下列终局都留下「已决 + 无 marker」的 fail 条目：

   | 终局 | 结果 |
   |---|---|
   | 决策选 `Stop` | `Terminal{Stopped}` |
   | 决策 backtrack 越权（白名单二次校验拒） | `Terminal::failed` |
   | 决策缺席（未调 `finish_step` / 输出截断） | `Terminal::failed` |
   | 决策会话自身异常（Failed / Stopped / Running 漂移） | `Terminal` |
   | backtrack 工具写面 Err（表位 / reason 超长） | `Terminal` |

   这五种之后重新「发起运行」：`fail_decided = true` 放行重跑，`open_backtrack_windows = []`
   ⇒ prompt = header + 静态主体 + **段 B**，零窗口段。

3. **相位阶段的系统异常反而不落这一格**：evaluator / executor 会话死在 `SettleMode::Deferred`
   路径上直接 `Terminal`，不写 fail 条目 → 重启后仍按既有条目路由，窗口不受影响。

### 10.2 「异源格」不存在（判据完备性）

任何 `verdict = fail` 条目都只能由 evaluator 步 / test-execution 门禁 / archi-check 门禁产出
（`PhaseSettle::Fail` 三处调用：`walker.rs:1166` / `:1484` / `:1901`），而这三个相位都满足
`evaluator.is_some() || escalates`，故其 fail 必然触发决策分叉（除非 `fail_decided`）。
决策分叉必写 `decision_session_id`。于是：

```
fail 条目被决策过
  ├─ 决策产出 backtrack ⇒ marker 写在同一 entry ⇒ P 成为某窗口的 from ⇒ 重跑时同源（A+B）
  └─ 决策未产出 backtrack ⇒ 无 marker、无窗口          ⇒ 重跑时只 B（窗口外）
```

⇒ 两态穷尽，没有「A 讲别处、B 讲自己」的纯异源格。

### 10.3 archive 不是样本（推翻 §9.2 原判）

archive 九步 `on_fail` 全为 `None`，失败一律 `Terminal`（`walker.rs:599-634` 段注释：
「其余步失败一律 Terminal：相位表 `on_fail = None`」），且 archive 只在收口时
`PhaseSettle::Pass`（空 checklist → `derive_verdict([]) = Pass`）。**archive 从不产生 fail 条目**，
故永不出现在窗口外格。

### 10.4 实测量化（本机 db）

```
已决 fail 条目（verdict=fail ∧ decision_session_id 非空）  = 342
  ├─ 有 marker（backtrack_to 非空）= 301  ⇒ 同源格
  └─ 无 marker（backtrack_to 为空） = 41  ⇒ 窗口外格（≈12%）
```

### 10.5 生产实证：`agent-finalize-tool-for-flow-steps`（change `01a1c698`）

```
id=875 code-review#1  pass
id=876 acceptance#1   FAIL  decision=ses-124  backtrack_to=null   ← 已决但无 marker
        └─ 决策 ses-124 选 stop，理由原文：「本次可回溯白名单为空，任何 backtrack 目标都会被
           写面白名单二次校验拒绝，无法在本轮内修复，故终止。**重启时请把 test-design（或
           test-gen / implement）纳入白名单**。」                       ← 明示「重启」为恢复路径
    run-1794369399807  stopped「会话被终止」
    run-1794378012387  stopped「决策 stop: …白名单为空…」
    run-1794387603428  stopped「会话被终止」（acceptance 重启会话 ses-128，被再次终止）
    run-1794410024444  completed
id=878 acceptance#2   PASS   ← 重启后重跑，prompt 零窗口段
id=879 archive#1      pass
```

实证会话 `ses-132-1794410024471`（run-1794410024444 的 acceptance/evaluator/2）的实际 prompt 结构：

```
change: agent-finalize-tool-for-flow-steps
## 角色自述 / 阶段要求 / 静态清单 A1–A5     ← 静态 md
（零「## 回溯重做窗口」段）                  ← 窗口外
⚠️ 上次评估未通过（attempt 1），请针对以下失败项修复：
  - A1 proposal 每条 AC 有实现证据：… AC-13 的「未杀死项逐条归因、无第三类」不成立：
    mutation-seg-0.json 中 agent-blocks.tsx:31/:109/:110/:320/:108 与 tool-cards.tsx:186
    共 5–6 枚未杀死项未登记进 test-design.md「不可测试项」…            ← 段 B：唯一上下文
## 输出协议
```

⇒ **窗口外格 = 「决策 stop / 出错 → 用户重启」的恢复路径**，而这条路径恰是决策 agent
自己建议的（stop reason 明写「重启时请…」）。删 B 会让这条恢复路径零上下文：
重启的评估者不知道自己上一轮判了什么、卡在哪几枚突变体上。

### 10.6 对取舍的最终影响

- 只删同源格（`open_windows.iter().any(|w| w.from_phase == P)` 抑制 `retry_feedback`）：
  去掉 301 条已决 fail 重跑里的重复，41 条窗口外重跑保住唯一上下文。
- 全局删 B：41/342（≈12%）的已决重跑变零上下文，且集中在「决策弃权 / 出错」这一最需要
  上一轮失败清单的时刻。

建议仍是分格处置，但理由从「archive 会空洞」更正为「决策 stop 后的重启恢复路径会空洞」。

---

## 11. 裁决（2026-11，用户采纳）

**分格处置：同源格抑制反馈段，窗口外格保留。** 不采用全局单通道。

### 11.1 实现形状（`phase_next.rs` 单点，零 schema 变动）

```rust
// build_phase_response 内，紧接 open_windows 之后
let same_origin = open_windows.iter().any(|w| w.from_phase == def.id);
let feedback_suffix = retry_feedback
    .filter(|_| !same_origin)      // 同源：段 B 与窗口段读同一条记录，只留窗口段
    .map(fail_feedback_suffix)
    .unwrap_or_default();
```

- `retry_feedback` 形参保留、`fail_feedback_suffix` 保留（窗口外格与异源格仍是唯一消费者，
  不产生死代码）。
- 判据落在 `build_phase_response` 内而非 `phase_next` 调用侧：`open_windows` 已在此处算出，
  零重复推导。
- `phase_next` 的两处 `latest_phase_entry` 返回值不动（抑制只发生在渲染层，不改路由判定）。
- 零字段新增、零 spec 结构变动、零 bindings 影响（`PhaseNextOutcome` 不出 bindings）。

### 11.2 三态定稿（与 §10.2 的判据完备性一致）

| 格 | 判据 | 段 A | 段 B | 依据 |
|---|---|---|---|---|
| 同源 | `open_windows.any(from_phase == P)` | ✅ | ❌ **抑制** | 段 B 是评估者自身旧输出回灌，段 A 是决策者的修法方向（§9.3） |
| 窗口外 | `open_windows` 空 ∧ 最新条目 fail | ❌ | ✅ | 决策 stop / 出错后的重启恢复路径，唯一上下文（§10.5） |
| 目标相位 | 走 `backtrack_to` 分支 | ✅ | ❌（原本就 `retry_feedback = None`） | 零变化 |

### 11.3 测试面（含一处「已存在但未断言」的落点）

`回溯窗口四态_上界源头相位命中`（`phase_next_test.rs:2424`）**就是同源格**：
`backtrack_marker(20, "test-gen", …, "dev-design", reason)` 设了 `decision_session_id`
（`:130`）⇒ `fail_decided = true` 放行重跑，`retry_feedback = Some(marker)`，
而 `same_origin = true`。当前该 prompt **确实带反馈段**（`entry` 的 `report = "test-gen 报告"`，
`checklist` 空 ⇒ 走 `body = record.report` 分支）。但测试只断言窗口段（`assert_both_roles_carry`
不查反馈段）⇒ 改动后仍绿，**必须补断言才有守护力**。

| 用例 | 现状 | 处置 |
|---|---|---|
| `回溯窗口四态_上界源头相位命中`（:2424） | 断言窗口段在场 | **补**：`!prompt.contains("上次评估未通过")`（同源抑制） |
| 窗口段与反馈段并存（:2910-2985） | from=test-gen、P=dev-design（异源）→ 断言双段 + 段序 | 不变（异源格不受影响，正是对照） |
| fail 重试仅反馈段零窗口段（:3150） | 窗口外格 | 不变 |
| 自环（:2513-2590） | 第一拍走 backtrack 分支，`retry_feedback = None` | 不变；自环永不进同源格（落账即闭环 ⇒ 无 `from == P` 的开放窗口 ∧ 反馈段同现） |
| **新增**：三态矩阵用例组 | — | 同源 / 异源 / 窗口外 三格在同一 fixture 族下逐格断言「A、B 各自在场或不在场」 |

### 11.4 spec 同步面（两处，同一条双分支写法）

- `desktop-workflow-write-face/spec.md:312`（位置契约）：现文只写「窗口段先于反馈段」，
  须补「**同源抑制分支**」——本相位为某开放窗口发起相位时，反馈段整段省略，只留窗口段；
  其余情形两段并存且序不变。
- `desktop-change-orchestration/spec.md:487`（段序）：动态面清单须登记同一抑制规则，
  并写清判据来源 = 写面下发的 `backtrack_window`（orchestration 仍零二次推导）。

MUST 写明「同源」判据是 `open_windows` 中存在 `from_phase == 当前相位`，**不是**重新比较
记录 id / timestamp——否则等于在编排侧另起一份同源判定（第二份判据）。

### 11.5 登记的两项信息损失（可接受，需留痕）

1. **`（attempt N）` 在同源格消失**：段 B 是该 prompt 内唯一承载重试次数的位置（§9.6 第 3 项）。
   源头相位重跑时 agent 不再知道自己是第几次。评估者按「全量重评」语义工作，该项非必需。
2. **「上次 checklist 的逐项 pass/fail」消失**：同源格只剩决策者的散文修法方向。
   实证支持可接受——acceptance#2 拿到该清单后仍全量重跑（§9.3 第 2 项），消费率近零。

### 11.6 顺带可清（不在本次范围，登记待议）

- `phase_next` 内 `latest_phase_entry(&entries, next.id)` 算了两次（`:214` 与 `:251`），
  可合并为一次绑定后复用。
- `fail_feedback_suffix` 取值不过滤 `stale`（§9.6）。
- report 上限双数字（角色 md 写 500 / `verdict.rs` 写 2000，§7 第 1 项）。

