# 提案: backtrack-window-context

> **变更**: backtrack-window-context
> **日期**: 2026-11-11
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/backtrack-window-context/explore.md`（657 行：现状链路 / 代码锚点 / 定稿方案 4.1–4.12 / 落点清单 / 18 条裁决 / 实现前必读 7.8）
> **性质**: 路由面语义补全——把「回溯背景」从「目标相位的一次性 prompt 后缀」改为「回溯窗口内每个相位、每个 agent 步随行的背景段」。零新表 / 零新列 / 零 schema 迁移 / 零前端改动 / 零 bindings 改动。
> **裁决基线**: explore §6 的 18 条已定（本提案逐条遵循，冲突处以裁决为准：含源头相位 / A2 并集 + 末位 2 层上限 / 跨 run 继承 / 决策 prompt 也带 / archive 两 agent 步不带 / S2 取代原因行 / XML 标签定界 + 只渲染原因 / 同位替换 / 不拆文件）。

---

## 问题

### 现象一：backtrack 之后，只有一处 prompt 知道「为什么被叫回来」

用户诉求原话：workflow backtrack 到某个历史相位后，只有一处 prompt 携带回溯原因，顺序执行的后续 step 不了解本轮运行的特殊性。

实况（`phase_next.rs:134-156`）：`reason` 的单点读取点在「全局最新条目的 `backtrack_to` 在场」这一条分支里，`reason_suffix`（`phase_next.rs:343-353`）由 `resolve` 闭包同时用于 executor / evaluator——**故只有回溯目标相位的那一拍带背景**。

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

### 症结一：窗口「自动结束」的机制决定了修复点不在写面之外

目标相位重跑落新条目后即成为全局最新条目（`backtrack_to = None`），回溯分支自然不再命中——**标记不是被清理，而是被更新的最新条目盖过**。因此：

- 丢失点是**相位边界**，不是相位内部：相位内部反馈修复走 `continue_session`，原因留在会话史里；
- 修复面只能是**跨相位 prompt 组装面**（写面 `phase_next` 的 `build_phase_response`），而不能靠「把 reason 留在某个运行期状态里」。

### 症结二：窗口边界若靠运行期状态推断，必然与「跨 run 继承」冲突

备选路线 B（`EdgeTrace` 增窗口字段、落账处置位、进源头相位清位）边界最准，但 prompt 组装从写面单点变两个 append 点、`EdgeTrace` 语义扩张（与既有 step-abstraction 的降状态方向相反），且**跨 run 继承要额外持久化**——与裁决 ③ 直接冲突。备选路线 C（落库显式窗口号 `backtrack_round` / `window_id`）要让 schema 迁移 + corpus/golden 随动 + 写面事务加宽。**两条都已否**（explore 4.12 留档），本提案只走「由既有条目派生」的路线。

### 症结三：不设层级上限时，体积不可预算

若「每条未闭环回溯记录」都进 prompt，20 条 × ≤1K 字符 ≈ 20K 字符的前言会稀释静态角色知识的主体比重，且体量随用户行为无界浮动。这是裁决 14（`MAX_BACKTRACK_WINDOWS = 2`）的唯一真实收益来源——**本提案不宣称它是精度收益**。

### 症结四：A2（多窗并集）与层级上限字面相反，含糊写进 spec 必留自相矛盾的口子

并集说「全都给」，上限说「只给最近两条」。共存点只有一种正确表述（explore 7.2）：

```
≤ MAX_BACKTRACK_WINDOWS（2）层：全部并集（0 / 1 / 2 层均正确）
>  MAX_BACKTRACK_WINDOWS    ：取末位 2 层，裁掉创建序最早（`from_index` 最大）的那层
```

同一份 spec 里只写「A2 并集」或只写「最多 2 层」都等于留坑，两处 spec（`desktop-workflow-write-face` / `desktop-change-orchestration`）都须写到这个双分支。裁剪方向的唯一正确读法（易写反，见 §5）：**保留创建序末位两层，裁掉创建序最早那条——即 `from_index` 最大那条**。

### 症结五：`from / to` 一旦进 prompt，就变成流程机制的泄漏

相位标识是流程机制的一部分，不是 agent 的领域知识（裁决 16）。但 `from / to` 又不是装饰——它们是**区间判据、堆叠序、裁剪位置与测试断言的输入**。因此结构面与渲染面必须分离：

```
结构面（BacktrackWindowEntry 五字段）        渲染面（prompt 文本）
  from_phase / from_index  ────┐
  to_phase   / to_index    ────┼──► 只取 reason 一个字段
  reason                   ────┘
```

唯一失效方式是在渲染 fn 里「顺手带出相位名」——文本看着无害，review 极难发现。故防御面必须是测试（断言 prompt 正文零相位 id 字面量），不是纪律。

## 提案

给「一次未闭环回溯」定义一个**窗口**，由既有 `PhaseStateRecord` 派生、零新表零新列，并让窗口内每个相位的每个 agent 步都携带它。

### 1. 一条未闭环回溯记录 = 一个窗口（判据三条）

```
W = { from: S, to: T, reason: R, order: marker.id }
    S = 标记行自身的 phase（发起相位，泳道位置靠后）—— 区间上界
    T = 标记行的 backtrack_to（回跳目标，泳道位置靠前）—— 区间下界
    R = 标记行的 backtrack_reason（原文逐字）
    order = 标记行的 PhaseStateRecord.id（落账序；store 写事务内 max+1）

判据一 · 是否拼接（区间，泳道 index）
connect(P, W) ⟺ to_index ≤ col(P) ≤ from_index

判据二 · 是否未闭环
open(W) ⟺ 发起相位不存在 id 大于 marker_id 的条目

判据三 · 堆叠与裁剪
H(P) = filter(open) → filter(connect P) → sort(创建序升序) → take_last(MAX_BACKTRACK_WINDOWS=2)
渲染集合（双分支）= if |H| ≤ MAX_BACKTRACK_WINDOWS { H } else { H[|H|-MAX..] }   ← 末位两层，裁掉创建序最早那条
```

- **泳道序单源 = 相位表 position**（`phase_table` 序 ≡ `detail.pipeline` 下标 ≡ 前端 `colIndex`，三处已核实同源，explore 4.1）；MUST NOT 另引第二份相位序。
- **判据不读 `run_id` / `SessionAnchors`** ⇒ 跨 run 继承（裁决 ③）天然成立，无需额外持久化。
- **「内层是否暂停外层」不需要另设规则**：内层回跳到更上游后当前相位落到外层 `to` 之前 ⇒ 判据一不命中，外层自然不拼；回放到区间内时外层再出现——一个区间条件同时治了「暂停」与「恢复」。
- **三处边界零特判**：含源头相位（P = S 命中上界，裁决 ①）、自环回溯（`to == from` 退化为单点，与常规回溯同一条代码）、嵌套回跳（多窗各自独立开放）。
- **创建序取 `id` 已足够**（explore 4.6）：标记不是新增行而是改写「发起相位最新条目」的 `backtrack_to` / `backtrack_reason`，而发起相位恒为「刚失败的相位」，其 fail 条目是决策前最后一次落账 ⇒ 标记行 id 每次回溯时都是全库最新，多个标记天然按决策先后递增。MUST NOT 引入决策时刻列。

### 2. 两层「本轮」口径（有意不同，非疏漏）

```
┌ 预算层（round / MAX_ROUNDS=20 / window_fails）── 按 run 窗口：entries[anchor..]
│   语义：单次运行的循环预算，重启换新预算（既有，零改动）
└ 背景层（回溯窗口）────────────────────────── 按 db 事件序：全量 entries
    语义：一次回跳的语义窗口，跨 run 直到闭环（本次新增）
```

两者 MUST NOT 混用：把窗口判据写成读 anchor 会把跨 run 的未闭环窗口误判为已闭环。

### 3. 单点位置：`PhaseNextOutcome` 新字段（正常响应 + error 响应都填）

walker 不直读 store（路由权威唯一、经 `ToolStepPort`），故背景由写面随响应下发，orchestration 不二次推导：

```rust
pub struct BacktrackWindowEntry {
    pub from_phase: String,   // 发起相位（泳道靠后）—— 区间上界；不进 prompt
    pub from_index: usize,    // 发起相位泳道 index
    pub to_phase: String,     // 目标相位（泳道靠前）—— 区间下界；不进 prompt
    pub to_index: usize,      // 目标相位泳道 index
    pub reason: String,       // backtrack_reason 原文逐字；prompt 只渲染此字段
}

PhaseNextOutcome
  + backtrack_window: Vec<BacktrackWindowEntry>
      正常响应（build_phase_response）：填「覆盖 next_phase 的开放窗口」（创建序升序）
      error 响应（error_outcome）      ：填「覆盖失败相位的开放窗口」← 决策输入取此处
```

消费方：`build_phase_response` → executor / evaluator prompt 追加窗口段（S2 取代既有原因行）；`decision_prompt` → `DecisionInput.backtrack_window` 渲染同一段（同一文案单点）；图面 / 前端 / 存档零改动。

**命名定案**：`from_phase` / `to_phase` + `from_index` / `to_index`（与渲染句「从 from 回溯至 to」同向）。MUST NOT 启用 `window_start` / `window_end`——那是泳道位置的读法，与渲染方向相反，两套方向并存即埋反向映射坑。

**`PhaseNextOutcome` 不出 bindings**（`src/types/generated/bindings.ts` 零命中）⇒ 新增字段零 TS 联动；新字段为 `Vec`（非 `Option`），空集即「无窗口」，`Default` 语义与「整段省略」同义。

### 4. 渲染：S2 单点 + XML 标签定界 + 零元信息

裁决 6（S2）**删除 `reason_suffix`**，统一由窗口段承载——目标相位那一行原因即窗口段里自己那一条，单点渲染。

```
## 回溯重做窗口（背景信息）

本 change 发生过下列相位回退，且均尚未回到闭环。这些是此前回溯留下的、需要解决的问题——原因原文按发生先后排列（最上面最早），保留原始换行与标点，未做截断或改写。它们是背景，不构成额外验收条件。

<backtrack reason>
设计返工：缺产物区组件
</backtrack>

<backtrack reason>
需求边界未收敛
</backtrack>
```

逐条定稿理由（explore 4.7）：

| 决策 | 理由 |
|---|---|
| 标签名固定 `<backtrack reason>`、**零属性** | 裁决 16：相位标识对 agent 不可见；顺序由出现先后表达（说明段已写「最上面最早」），无需 `order` 属性 |
| 保留 XML 标签定界 | 标签行可被行首精确匹配；原因原文跨多行也不失边界 |
| **不截断、不压平** | 逐字保留原因原文（含首尾换行与标点）；MUST NOT 套用 `diagnose_brief` 的 200 字截断（那是节点 detail 面口径） |
| 不用 `---` 横幅 / 不用 `<<<<<<<` | 原因原文可含 `---`（撞 Markdown 分隔线语义）；`<<<<<<<` 是本仓库冲突标记词汇 |
| 不做「单条退化一行」短式 | 单条与多条同一形状，避免两套渲染各自漂移 |
| 末句「不构成额外验收条件」 | 纯陈述，判断权留给 agent，不加准 AC |

**位置是契约**（裁决 17）：窗口段**原地替换 `reason_suffix`**——静态主体之后、`feedback_suffix` **之前**。`evaluator_prompt` 在外层再 append 协议附录，故 evaluator 最终序为 `header → 主体 → 窗口段 → 反馈段 → 协议附录`。MUST NOT 放到反馈段之后：反馈段是「本相位这次的失败清单」，窗口段是「为什么这个相位被叫回来」——后者是前者的上位语境。

**决策 prompt 内部位置**同款推理：既有段序为 `失败相位报告 → 失败清单 → 可回溯白名单 → 候选报告 → 报告文件 → 决策协议`，窗口段属背景信息且更宏观 ⇒ 插在 `## 失败相位报告` **之前**（段序最前）。

**白名单与窗口段不混淆**：唯一可能的语义重叠面是相位 id，而裁决 16 已把 id 从窗口段彻底删掉；两段形状（`- id（描述）：产物` vs 纯原因）、用途（选目标 vs 知道为什么被叫回来）、约束力（硬 vs 零）全不同。

### 5. 层级上限：`MAX_BACKTRACK_WINDOWS = 2`，取命中末位两层

```
命中集合 H(P) = filter(open) → filter(connect) → sort(创建序)
渲染集合 = if |H| ≤ 2 { H } else { H[|H|-2 ..] }
```

**结构性性质（由创建条件导出，非概率）**：同时开放的多窗口，按创建序排列其 `from_index` **严格递减**——要创建 `from=F_j` 的新窗口，流程必须先跑到 F_j，而这要求 F_j 之前所有相位重新 pass 一遍，凡 `from_phase` 更早（`from_index` 更小）的窗口都会因此闭环。⇒ 创建序最早的那条窗口 `from_index` **最大**，也是 `take_last` 首先丢弃的那条。**因此裁剪方向是确定且必然的：保留创建序末位两层，裁掉创建序最早即 `from_index` 最大那条**（explore 把它口语化为「最旧最宽」——可断言的判据只有 `from_index` 最大，宽度是它的常见后果而非定义），故「取末位两层」是可断言、可测试的规则而非启发式。

三种说法在本提案内 MUST 视为同一件事、不得混用为两种方向：`创建序最早` ≡ `take_last` 首先丢弃 ≡ `from_index` 最大。测试断言 MUST 直接钉在这条等价链上（见 AC-4 / 测试文件行）。

定稿示例（explore 4.5 场景三，三窗同命中 `proposal`）：

```
Win1  order=10  from=test-gen(5)  → from_index = 5   ← 创建序最早，from_index 最大 ⇒ 被裁
Win2  order=20  from=implement(4) → from_index = 4   ← 出线
Win3  order=30  from=test-design(3) → from_index = 3 ← 出线
渲染集合 = [Win2, Win3]（创建序升序），Win1 不出线
```

该示例同时钉死裁剪方向与渲染顺序：出线的是 `from_index` **最小**的两条（最近创建的两次回溯），被裁的是 `from_index` **最大**那条（创建序最早）。任何把这句读成「被裁的是 `from_index` 最小者」的实现都与本示例矛盾、必被三层用例判红。

**对 explore 一处措辞的订正**：explore 7.4 的示例断言句写作「`from_index` 最大的两条出线、最小的那条被裁」，与同段推导出的「同时开放窗口 `from_index` 严格递减、最外层恒被首先裁掉」直接矛盾——按该推导，创建序最早（最外层）`from_index` 最大，故被裁的必然是**最大**者。本提案以推导为准（explore 4.5 场景三的三窗数值可自校验：Win1 `from_index=5` 被裁、Win2/Win3 出线），测试断言按「创建序最早的 Win1 不出线」表述，不沿用该句的字面方向。

**与 A2 的取舍（诚实登记）**：本上限与「A2 保住最外层」的论据结构性冲突（explore 4.8）。接受理由：深层嵌套（≥3 层未闭环）属异常路径，`MAX_ROUNDS=20` 才是它的硬兜底；「最近两次回溯」才是 agent 当前直接受约束的层级；无上限的真实风险是信息过载而非信息不足。⇒ **A2 退化为「≤2 层时的并集」**，须在 spec 里如实写清（见症结四）。

**刻意不做的**：不做「保留最外层 + 末位」的混合拼接（顺序语义会分叉成两段）；不做「首尾各一」采样（截断位置不可预测）；**不提示被裁掉**（窗口段零「另有 N 条」元信息——提示语本身会诱导 agent 追问或自行推测缺失内容）。

### 6. 体积与文件规模

```
现状对照：单条 reason_suffix（仅目标相位）              ≈ ≤1K 字符
上限后：  2 条 ×（标签行 ~60 字 + 原因 ≤1000 字符）      ≈ ≤2.1K 字符
无上限时（原方案）：20 条 ×（同上）                     ≈ 20K 字符
```

⇒ 上限把最坏情况从 20K 压到 2.1K（约 1/10）；相对现状的最坏增量约 **+1.1K 字符/相位**——同量级、非数量级跃升，换来的是「窗口内每个相位的每个 agent 步都拿到背景」而非只有目标相位一行。实现时 MUST NOT 因「怕变大」再自行加截断。

**文件不拆**（裁决 18）：`phase_next.rs` 现 374 行，新增 ~95 行 → ~470 行，同 crate 属中等（`phase_table.rs` 852 / `phase_next_test.rs` 2156）。关键理由不是行数而是**数据依赖**：窗口判据需要 `entries` + `table` + `anchor` 同时在场，而它们正是 `phase_next` 的既有入参 / 局部变量；拆文件必须把三者跨模块传递，还多出 `mod` 声明 + `pub use` + 测试搬迁三笔账。**若变更阶段坚持拆分**，唯一可接受切面是「判据 + 渲染整体成 `write/backtrack_window.rs`」，MUST NOT 把判据与渲染分到两个文件。

### 7. 明确不做

- archive 的 spec-sync / resolve-conflicts（`prompt.rs:166 / 190` 的 `spec_sync_prompt` / `merge_conflict_prompt` 是另一套组装，且 archive 恒在窗口外，裁决 ⑤）；
- 图面（窗口内其余边仍 `forward`，用户看图无法辨识「同一轮重做」——留待后续变更）；
- 前端 / bindings / store schema / `BacktrackInput` / `backtrack.rs` 写面（零改动）。

## 能力

### 修改的能力

- **desktop-workflow-write-face** — **MODIFIED**「backtrack 白名单二次校验与 stale 语义」：补窗口记录形状（标记行即窗口：`from` / `to` / `reason` / `order = id`，零新表零新列零新时钟）。**MODIFIED**「PhaseAgentSpec 静态化与通配符退役」：prompt 组装面由「回溯原因行（append）」改为「回溯重做窗口段（append）」，零 `⚠️ 回溯原因` 行。**ADDED**「回溯重做窗口推导与 prompt 携带」：`BacktrackWindowEntry` 五字段 + `PhaseNextOutcome.backtrack_window` + 三条判据 + 上限双分支（≤ `MAX_BACKTRACK_WINDOWS` 全量并集 / 超限取创建序末位两层、被裁为 `from_index` 最大那条）+ 跨 run 继承 + 自环单点 + 文案单点与渲染单点 + 位置契约 + 体积界 + 只渲染 reason。**ADDED**「回溯窗口与轮次预算的口径分层」：预算层按 run（`entries[anchor..]`）/ 背景层按 db 事件序（全量），两者 MUST NOT 混用。
- **desktop-change-orchestration** — **MODIFIED**「相位 prompt 组装面静态化（静态主体 + 动态 append）」：动态 append 清单把「回溯原因行」改为「回溯重做窗口段」，并钉死段序（窗口段先于反馈段、evaluator 协议附录最后），条目规模语义写明双分支（≤ `MAX_BACKTRACK_WINDOWS` 全量并集 / 超限取创建序末位两层），orchestration 零二次裁剪。**MODIFIED**「决策协议」：`DecisionInput` 增 `backtrack_window`（透传自 error 响应，walker MUST NOT 自读 store / 自行推导），决策 prompt 增窗口段（同一文案单点、位置在 `## 失败相位报告` 之前、空则整段省略、只渲染原因），并就地写清上限双分支（≤ 上限全量并集 / 超限渲染末位两层、创建序最早那条不出线）。**MODIFIED**「失败升格落账与决策信息面」：`DecisionInput` 字段清单补 `backtrack_window`（避免该条的字段枚举被读成穷举）。

### 零改动能力与理由

- **desktop-phase-prompts**：16 份角色 md 零改动——窗口段是**运行时 append 的动态面**（与本能力「全静态零占位」的静态主体分属两层），写进 md 反而变成「角色知识里出现流程元信息」的错位，且会与 spec 双写漂移。
- **desktop-change-state-store**：零 schema / 零迁移 / 零新列——窗口完全由既有 `PhaseStateRecord`（`phase` / `backtrack_to` / `backtrack_reason` / `id`）派生；`store.rs` 的 `backtrack` 写事务逐字不变。
- **desktop-change-flow-view**：`graph.ts` 的 `columns` 仍由 `detail.pipeline` 下标给出（本变更只是**复用**这份序作为判据单源），回溯边标签仍取发起条目记因——图面零改动。
- **desktop-change-queries**：`detail.pipeline` 已是相位表序的投影，本变更消费它而不改它；`backtrack_reason` 字段照常出线。
- **desktop-ipc-type-bindings**：`PhaseNextOutcome` / `DecisionInput` 均不出 bindings（`src/types/generated/bindings.ts` 对二者零命中），`bindings:export` 输出零 diff；版本 patch +1 仍按纪律执行。
- **desktop-change-archive**：archive 两 agent 步的 prompt 组装（`spec_sync_prompt` / `merge_conflict_prompt`）MUST NOT 携带窗口段——archive 恒在窗口外，且那是另一套组装面。
- **desktop-checks-domain / desktop-test-execution / desktop-mutation-testing / desktop-change-create / desktop-workspace-config / desktop-crate-layout / desktop-write-protection / desktop-corpus-regression / desktop-release-pipeline**：检查域 runner、执行链、阈值、建档前置校验、配置面、crate 布局、锁面、语料、发版链全零触碰——本变更只动三个 `.rs` 的写面 / 决策面组装函数与一处结构体。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/workflow/src/write/phase_next.rs` | ① 新增 `pub struct BacktrackWindowEntry { from_phase, from_index, to_phase, to_index, reason }`（`Debug` / `Clone` / `PartialEq` / `Eq`，沿本文件既有类型先例；非 `Serialize`——不出 bindings）；② `PhaseNextOutcome` 增 `backtrack_window: Vec<BacktrackWindowEntry>`（生产 3 处字面量随之填值：全 pass 的 `done` 响应填 `Vec::new()`、`error_outcome` 填覆盖失败相位的窗口、`build_phase_response` 填覆盖 `next_phase` 的窗口）；③ 窗口推导 fn（区间判据 + 未闭环判据 + 创建序排序 + 上限双分支：命中数 ≤ `MAX_BACKTRACK_WINDOWS` 时全量出线、超限取创建序末位两层并裁掉 `from_index` 最大那条；单点；入参 `(&[PhaseStateRecord], &'static [PhaseDefinition], phase_id)`）；④ 窗口段渲染 fn（说明段 + 逐条 `<backtrack reason>` 标签，**只取 `reason`**），`pub` 并经 `write/mod.rs` 再导出供 orchestration 复用；⑤ **删 `reason_suffix`**（含 `build_phase_response` 的 `backtrack_reason: Option<&str>` 入参——回溯分支与正常分支的入参由此收敛）；⑥ 窗口段插入位置 = 原 `reason_suffix` 处（静态主体之后、`feedback_suffix` 之前）；⑦ 文件顶部 `ResolvedPhaseSpec` / `build_phase_response` 的 doc 注释同步（现写「回溯原因行」，改「回溯重做窗口段」）。净增约 95 行（374 → ~470），**不拆文件** |
| `crates/core/workflow/src/write/phase_table.rs` | 新增 `pub(crate) const MAX_BACKTRACK_WINDOWS: usize = 2;`（与 `MAX_REASON_CHARS` / `MAX_REPORT_CHARS` / `MAX_ROUNDS` 同在写面常量区，单点；prompt 文案与测试断言 MUST NOT 写死数字） |
| `crates/core/workflow/src/write/mod.rs` | 再导出扩两项：`PhaseNextOutcome` 行加 `BacktrackWindowEntry`，另加窗口段渲染 fn（单个 `pub use`，零新模块） |
| `crates/core/orchestration/src/decision.rs` | `DecisionInput` 增 `pub backtrack_window: Vec<workflow::write::BacktrackWindowEntry>`（字段 doc 注明「透传自 error 响应，walker 不自行推导」；不改 `DecisionAction` / `parse_decision` / `ensure_backtrack_allowed`） |
| `crates/core/orchestration/src/prompt.rs` | `decision_prompt` 增「回溯重做窗口」段：调用写面渲染单点（MUST NOT 另持第二份文案 / 第二份判据），空集整段省略，插入位置 = `## 失败相位报告` 之前（紧跟开头一句「相位…评估失败，需要你做出路由决策」之后） |
| `crates/core/orchestration/src/walker.rs` | ① `build_decision_input` 增形参（或从 `next` 取）`backtrack_window` 并原样透传 `next.backtrack_window.clone()`——**不自读 store、不自相位表推导**；② 调用点 `resolve_deadlock` 传 `&next.backtrack_window`；③ `:792` 的 `step_executor` doc 注释改写（现写「上下文头 + 失败反馈段 + 回溯原因后缀全在写面」，改「回溯重做窗口段」）。零新 port、零新 step、零 route 规则 |
| `packages/desktop/package.json` | `version` 按**当前 `HEAD` 值 +1 patch** 递增（现 `0.6.31` → `0.6.32`；相对递增，MUST NOT 写死；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 版本不随动）。`scripts` 零改动 |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 四态：目标相位（有窗口）/ 窗口内中间相位（有窗口）/ 源头相位（有窗口，裁决 ①）/ 窗口外相位（零窗口） | `crates/core/workflow/src/write/phase_next_test.rs` | 扩展 |
| 自环回溯（`to == from`）退化为单点区间：只在该相位拼、落账即关，与常规回溯同一条代码（反例断言：MUST NOT 存在 `if to == from` 特判分支） | 同上 | 新增 |
| 嵌套双窗（A2 并集）：下游相位同时命中两窗 → 按创建序 [R1, R2]；外层区间外相位只拿内层 | 同上 | 新增 |
| 命中 3 层 → 取末位两层：断言创建序最早的 Win1 **不出线**、Win2 / Win3 按创建序出线（结构性等价：Win1 的 `from_index` 在三条中**最大**；同时开放窗口按创建序 `from_index` 严格递减） | 同上 | 新增 |
| 内层回跳到更上游：外层当前相位「跑到上游即不拼」、回放进区间「再拼」（零状态机、零暂停标志） | 同上 | 新增 |
| 跨 run 继承：新 `run_id` + 新 anchor（`round` 归 1）仍命中同一未闭环窗口；反例断言判据不触碰 `SessionAnchors` | 同上 | 新增 |
| error 响应携窗口：`VerdictFail` / `MaxRetriesExceeded` 两变体的 `backtrack_window` = 覆盖失败相位的开放窗口 | 同上 | 新增 |
| 原因原文逐字保留：含换行 / 首尾空白 / 标点 / ≤1000 字符长文的 prompt 断言（不截断、不压平、不 trim） | 同上 | 新增 |
| 渲染零泄漏：窗口段用例断言 prompt 正文**不出现任何相位 id 字面量**（`from_phase` / `to_phase` 零命中） | 同上 | 新增 |
| 裁剪顺序：先 `filter(open) → filter(connect) → sort(创建序) → take_last`；反例断言「先 `take_last` 再 `sort`」得错层（构造 3 窗、输入条目序 ≠ 创建序，且创建序最早者 `from_index` 最大） | 同上 | 新增 |
| S2 断言改写：4 处含 `⚠️ 回溯原因` 的断言（现行号 718 / 1035 / 1084 / 2098）及相邻说明改写为窗口段断言（`## 回溯重做窗口` + `<backtrack reason>` + 原因文本） | 同上 | 改写 |
| `PhaseNextOutcome` 字面量补字段 **6 处**：`port_test.rs:27`、`walker_test.rs:931 / 952 / 967 / 992 / 1770` | `crates/core/orchestration/src/port_test.rs` + `walker_test.rs` | 扩展（编译器兜住：结构体字面量缺字段即编译失败） |
| `DecisionInput` 完整构造器补字段 **3 处**：`prompt_test.rs:124`（`base_input()` 权威 fixture）、`prompt_test.rs:610`、`decision_test.rs:361`（其余 `DecisionInput { … ..base_input() }` / `..base` 共 19 处自动随动） | `crates/core/orchestration/src/prompt_test.rs` + `decision_test.rs` | 扩展 |
| 决策 prompt 窗口段：① 多条窗口在场时按创建序渲染、每条以 `<backtrack reason>` 定界；② 空窗口整段省略（prompt 零 `## 回溯重做窗口` 字样）；③ **段位置断言**（窗口段索引 < `## 失败相位报告` 索引）；④ 零相位 id 泄漏（`input.backtrack_window` 的 `from_phase` / `to_phase` 字面量在 prompt 正文零命中）；⑤ **双分支覆盖**：2 条入参全量渲染、3 条入参同样逐条渲染（源码零二次裁剪分支，生产不变式 = 写面下发集合恒 ≤ 上限） | `crates/core/orchestration/src/prompt_test.rs` | 新增 |

### 不要修改

- `openspec/specs/**` 主基线（落主基线只在归档 SpecSync 步）。
- `crates/core/workflow/src/prompts/**`（16 份角色 md，零改动；见「零改动能力」）。
- `crates/infra/store/**`：零 schema / 零迁移 / 零新列 / 零写事务改动。
- `crates/core/workflow/src/write/backtrack.rs`：白名单二次校验 / 表位校验 / `reason ≤ 1000` / stale 传播全部逐字不动——窗口是**读面派生**，不改变写面裁定。
- `crates/core/orchestration/src/prompt.rs` 的 `spec_sync_prompt` / `merge_conflict_prompt`（archive 两 agent 步，裁决 ⑤ 明确排除）；`evaluator_protocol` / `verdict_fix_prompt` / `decision_fix_prompt` 文本（软拒纠正面只重申目标，不展开窗口段）。
- `packages/desktop/src/**`：图面（`views/changes/flow/graph.ts`）、hooks、组件、路由、生成类型全零改动；`src/types/generated/bindings.ts` / `bindings.zod.ts` 零 diff。
- `crates/core/orchestration/src/{port.rs,steps.rs,control.rs,state.rs}`：零新 port / 零新 step / 零新状态值 / 零新通知类型。
- `crates/core/orchestration/src/decision.rs` 除 `DecisionInput` 新字段外的一切（`DecisionAction` 三动作封闭集、`parse_decision`、`ensure_backtrack_allowed`、`MAX_REASON_CHARS = 500`）。
- `.github/workflows/**`、`src-tauri/tauri.conf.json`（版本经 `../package.json` 跟随）、`src-tauri/Cargo.toml` 版本行。
- `openspec/changes/backtrack-window-context/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | **窗口内每个相位的每个 agent 步都带背景**：一次 `backtrack(to=T, reason=R)` 后，从 T 到发起相位 S 的每个相位的 executor **与** evaluator prompt 均含 `## 回溯重做窗口` 段且含 R；窗口外相位零该段；`S` 自身命中（裁决 ①） | `phase_next_test` 四态用例（目标 / 中间 / 源头 / 窗外） |
| AC-2 | **区间判据单源**：判据仅按 `to_index ≤ col(P) ≤ from_index` 执行，`col` 取自相位表 position（与 `detail.pipeline` 下标同源）；自环（`to == from`）退化为单点、由同一条代码承载，源码零 `if to == from` 特判 | `phase_next_test` 自环 + 嵌套用例 + 源码审查 |
| AC-3 | **未闭环判据不读 run 状态**：`open(W)` 仅由「发起相位是否有 id 更大的条目」判定；跨 run（新 `run_id`、新 anchor、`round` 归 1）仍命中同一未闭环窗口；判据源码零 `SessionAnchors` / `run_id` 命中 | `phase_next_test` 跨 run 用例 + 源码审查 |
| AC-4 | **A2 并集与上限的双分支**：命中 0 / 1 / 2 层全量并集（按创建序升序，最早在前）；命中 3 层只渲染末位 2 层，且被裁恒为**创建序最早**那条（结构性等价：三条中 `from_index` **最大**）；窗口段零「另有 N 条」类元信息 | `phase_next_test` 双窗 + 三层用例（断言 `from_index` 最大的那条不在渲染集合、其余两条按创建序在）+ 渲染文本断言 |
| AC-5 | **求值顺序固定**：`filter(open) → filter(connect) → sort(创建序) → take_last(2)`；「先 `take_last` 再 `sort`」被反例用例判红（用例的输入条目序 ≠ 创建序） | `phase_next_test` 顺序用例 |
| AC-6 | **单点位置与响应填充**：`PhaseNextOutcome.backtrack_window` 在正常响应填「覆盖 `next_phase` 的开放窗口」、error 响应填「覆盖失败相位的开放窗口」；两处共用同一推导 fn（源码单点） | `phase_next_test` error 响应用例 + 源码审查 |
| AC-7 | **S2 取代原因行**：`⚠️ 回溯原因` 字样在产品 `.rs` 内零命中；`reason_suffix` 与 `build_phase_response` 的 `backtrack_reason` 入参删除，回溯分支与正常分支入参收敛；窗口段插入位置 = 静态主体之后、`FAIL` 反馈段之前（`prompt.find("## 回溯重做窗口") < prompt.find("⚠️ 上次评估未通过")` 同时在场时成立） | `phase_next_test` 改写断言 + `grep` 零命中 + 位置断言 |
| AC-8 | **渲染零元信息**：窗口段文案零属性（`<backtrack reason>` 标签行可被行首精确匹配）、原因逐字保留（含换行 / 首尾空白 / 标点，不截断不压平不 trim）；prompt 正文零相位 id 字面量（`from_phase` / `to_phase` 值零命中） | `phase_next_test` 原文保真用例 + `prompt_test` 零泄漏用例 |
| AC-9 | **文案与常量单点**：窗口段文案 + 渲染 fn 只在写面一处（经 `write/mod.rs` 再导出），orchestration 侧零第二份文案 / 零第二份判据；`MAX_BACKTRACK_WINDOWS` 只在 `phase_table.rs` 定义一处，prompt 文案与测试断言零写死 `2` 的业务语义（测试引用常量） | 源码审查 + `grep` 计数 |
| AC-10 | **决策面携带窗口**：`DecisionInput.backtrack_window` 由 walker 透传自 error 响应（`build_decision_input` 内零 store 读取、零相位表推导）；决策 prompt 含 `## 回溯重做窗口` 段、位置在 `## 失败相位报告` 之前（索引比较）、空集时整段省略、只渲染原因；条目数语义与写面双分支一致（≤ 上限全量并集 / 生产路径上超限部分已在写面裁为末位两层），orchestration 侧零二次裁剪 | `prompt_test` 五用例（多条 / 空集 / 段位置 / 零泄漏 / 2 条与 3 条入参均逐条渲染）+ `walker_test` 透传用例 |
| AC-11 | **零结构扩散**：零新表 / 零新列 / 零 schema 迁移 / 零 store 改动；`BacktrackInput` / `backtrack.rs` 写面逐字不变；`port.rs` / `steps.rs` / `control.rs` / `state.rs` 零改动；零新 port、零新 step 词、零新通知类型 | `git diff --stat` 逐文件审查 |
| AC-12 | **交付面**：`cargo clippy --all-targets -D warnings` 与 `cargo test` 全绿（含既有 7429 条基线零回归）；`PhaseNextOutcome` / `DecisionInput` 不出 bindings ⇒ `bindings:export` 零 diff；`vp check` / `knip` 通过；`packages/desktop` 的 `version` = 当前 `HEAD` 值 +1 patch（`tauri.conf.json` 仍为 `"../package.json"`、`src-tauri/Cargo.toml` 版本不随动） | 机械门面 |
| AC-13 | **体积有界**：单相位窗口段 ≤ `MAX_BACKTRACK_WINDOWS` ×（标签行 + `MAX_REASON_CHARS`）= 2 × (≈60 + 1000) ≈ ≤2.1K 字符；实现零自行截断（源码零新增 truncate / `chars().take(…)` 于窗口渲染路径） | 长文用例（1000 字符原因 × 2 窗）+ 源码审查 |
| AC-14 | **明确不做项零触碰**：archive 的 `spec_sync_prompt` / `merge_conflict_prompt` 零窗口段；图面 `graph.ts`、前端 `src/**`、`src/types/generated/**` 零 diff | `git diff` 逐文件审查 |
| AC-15 | **spec 双分支如实写清**：`desktop-workflow-write-face` 与 `desktop-change-orchestration` 的 delta 均写明「≤ `MAX_BACKTRACK_WINDOWS` 全量并集 / 超限取创建序末位两层」，且裁剪方向在两处只有一种表述（创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` **最大**；出线的是 `from_index` 最小的两条）；两处无「只写 A2」或「只写最多 2 层」的单边表述，也无「被裁为 `from_index` 最小」类反向表述 | spec 逐条比对（本提案交付态）+ `grep -n "最小" specs/*/spec.md` 逐行确认为「出线 / 最近创建」语境 |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **深层嵌套不提示裁剪**（裁决 15）：命中 ≥3 层时 agent 会形成「历史上只回溯过这两次」的认知，与库中真实记录不符 | agent 误判历史 | 中（限于异常路径） | 偏差范围限定在 ≥3 层未闭环；正常路径（0/1/2 层）零偏差。补提示会引入元信息、诱导追问，两害相权取不提示。**改法已预埋**：日后只改窗口段**标题**一处措辞（如「仅显示最近 2 次」），不动结构、不动判据与渲染形状 |
| R-2 | **A2 与上限字面相反**，后续维护者只读到一半就改判据 | 语义漂移 / 裁错层 | 中 | 两处 spec 都写双分支（AC-15）；裁剪方向由「创建序最早 ≡ `take_last` 首先丢弃 ≡ `from_index` 最大」的等价链钉成可断言规则；测试直接断言「`from_index` 最大的那条（创建序最早）被裁、末位两条出线」 |
| R-3 | **原因原文含字面 `</backtrack>`** 会提前截断该条记录 | 该条原因显示不全 | 低（决策 JSON 自由文本，概率可忽略） | 转义等于改写原文，与「不压平」裁决冲突 ⇒ **不设防线**，如实登记。若日后实测出现，改法是转义 + 同步改「不压平」措辞（一次裁决，不在实现期暗改） |
| R-4 | **裁剪顺序踩坑**（先 `take_last` 再排序）会裁错层且只在 ≥3 层时暴露 | 用户看到的原因不是最近创建的两次 | 中高（顺序易写反） | AC-5 专列顺序用例：输入条目序 ≠ 创建序的构造让写反必红 |
| R-5 | **渲染时顺手带出相位名**（裁决 16 唯一失效方式，文本看着无害、review 极难发现） | 泄漏流程机制给 agent | 中 | 结构面 / 渲染面分离纪律 + 两处测试断言 prompt 正文零相位 id 字面量（`phase_next_test` / `prompt_test` 各一处） |
| R-6 | **prompt 体积最坏增约 +1.1K 字符/相位** | 稀释静态角色知识比重 | 中（已是量级可控） | 上限把它从「不可预算（≈20K）」压到「确定有界（≈2.1K）」；实现 MUST NOT 再加截断（AC-13）；若日后实测有干扰，收紧 `MAX_BACKTRACK_WINDOWS` 是一个常量 |
| R-7 | **跨 run 继承被误实现为读 anchor**：判据触碰 `SessionAnchors` 会把跨 run 的未闭环窗口误判为已闭环 | 重启后背景丢失（正是本变更要修的现象） | 中 | 判据签名不含 `SessionAnchors` / `run_id`（AC-3 源码审查）；跨 run 用例用新 `run_id` + 新 anchor 断言仍命中 |
| R-8 | **决策面防重复回跳降级**：裁决 16 删掉 `from/to` 后，决策 agent 只能靠原因文本语义理解「这个方向试过」，MUST NOT 宣称与结构判据等价 | 重复回跳仍可能发生，靠 `MAX_ROUNDS=20` 兜底 | 中 | 如实登记降级（写入 `desktop-change-orchestration` delta）。弱化后仍显著优于现状（现状决策 prompt 零历史回溯信息）；**升级路径已预埋**：结构体里 `from/to` 一直在，只改渲染 fn 一处即可加回 |
| R-9 | **9 处结构体字面量漏改**（生产 3 + 测试 6）/ `DecisionInput` 3 处完整构造器 | 编译失败（非运行期静默） | 低 | Rust 结构体字面量必须给全字段 ⇒ 编译器兜住；`..base_input()` 的 20 处 spread 构造自动随动 |
| R-10 | **文件规模**（374 → ~470 行）引发拆分冲动，拆错位置会让判据与渲染分家 | 中间产物跨模块往返、「谁负责裁剪」失去位置 | 低 | 本提案定案**不拆**（数据依赖：`entries` + `table` + `anchor` 三者同在 `phase_next`）；若后续坚持，唯一可接受切面 = 判据 + 渲染整体成 `write/backtrack_window.rs` |
| R-11 | **裁剪方向写反**：把「创建序最早」与「`from_index` 最大」当成两个不同方向（如误记「被裁的是 `from_index` 最小者」），使用例与正文互相否定、无法给出确定 pass/fail | 用例自相矛盾、实现照错误方向断言 | 中 | 全篇只用一个表述「保留创建序末位两层、裁掉创建序最早即 `from_index` 最大那条」；正文、scenario、AC、测试行四处同向；`specs/*/spec.md` 内 `grep "最小"` 须零命中（AC-15） |
