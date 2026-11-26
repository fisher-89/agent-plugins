# 提案: toml-sorting-and-layout

> **变更**: toml-sorting-and-layout（架构模型 TOML 的写入序：写路径上的规范化）
> **日期**: 2026-11-25
> **状态**: draft（形态已定，见 explore §一与 §6.1 三条裁决）
> **来源**: 本 change 目录 `explore.md` 全篇；前序主篇 `archi-agent-contract-knowledge-gap`（写工具落地，已归档 `b7af144d` / `dcf5b913`），其「不含」项原文即「元素在文件内的插入位置（D6 后置到 `toml-sorting-and-layout`）」。
> **性质**: **一次收敛在写面出口的规范化**——不做新工具、不加诊断、不改判据、不动 parser，只在四个 `apply_*` 的渲染前插一层「把文件收到三条不变量」的纯函数，并把顺带搬家的块经 `reordered` 报出。
> **不含**: 同层字典序清理（7 处现存违例）、关系组内 `to` 手选序改写（14 处）、任何 `archi_sort` 类手动入口（含只读预览）、序违例的诊断 / warning 暴露面、自由文本通道的即时校验（L3a 双写面是终态）、跨模型文件搬元素、模型目录进写保护锁面（L3b）。

---

## 问题

### 一 · 「插入即追加」让文件序随写入漂移，而漂移是对解析零影响的美观债

写工具落地后新元素/新关系一律追加到表尾（`edit/mod.rs:250` 元素表尾插入、`edit/mod.rs:352` 关系条目 `push`）。追加不会破坏「父先于子」（新建须落父元素文件、父须已声明），唯一能破坏的是**子树连续性**：新元素的父已有后代排在更靠前位置时，新块与前序后代之间被别人的块隔开。

实测（2026-11-25，六文件逐个数 FQN 前缀重算）：

| 文件 | 元素 / 关系 | 父先于子 | 子树连续 | 关系 `from` 组连续 |
|---|---|---|---|---|
| `01-app` / `02-backend-core` / `03-backend-infra` / `04-backend-shell` / `05-backend-tools` | 2/0 · 9/10 · 8/25 · 1/14 · 3/3 | ✔ | ✔ | ✔ |
| `06-frontend` | 32 / 77 | ✔ | **✘ 1 处（3 块）** | ✔ |

`06` 的唯一断裂：`Desktop.Views.Changes` 的三个子元素（`Flow` / `Renderers` / `Agent`，行 70–87）被追加到整个 `Views` 组之后，落在 `Welcome` 之后。而**关系面 6/6 全部组连续**——`05` 与 `06` 的 17 处「违例」是**组内 `to` 手选序**与**同层字典序**，不是组分裂，本变更一条都不改（见「不含」）。

**关键定性**：这三条不变量与解析无关。结构诊断走集合 / 图论判定（`graph.rs`）、出线 DTO 已按 FQN 排序（`dto.rs`）、漂移条目序经 DTO、结构 diff 用 `BTreeSet`、前端元素树自行 `localeCompare`、生成物按 FQN。规范化的唯一产物是**模型 TOML 的字节与随之而来的行号**——目的是让 `git diff` 长期保持可评审。

### 二 · 一次性整理与独立工具都堵不住两条写面

`graph/model/**` 有两条并存的写面（工具 + 自由文本 `write` / `edit`，L3a 双写面是终态）。任何「某次整理」都会被下一次自由文本写入打回，所以规范化只能焊在**每次写的出口**。由此派生的硬要求是：**规范序必须取最小移动形式**——若规范化本身要搬半个文件，它就等于每次写都把 diff 冲成噪声，评审面（唯一的目的）当场丢失。

### 三 · 仓库里并存两套序约定，任何机械序都会改写其中一套

| 文件 | 元素序 | 关系序 |
|---|---|---|
| `02-backend-core` | 分层依赖序（Foundation → Config → Workflow → Archi → …） | 纯 `(from,to)` 字典序 |
| `06-frontend` | DFS + 语义手选（`Views` 在 `Components` 前） | `from` 组 = 元素出现序 + 组内手选（`GeneratedBindings` 统一提前） |

同一仓库存两套互不兼容的约定：同层字典序有 7 处违例、`(from,to)` 字典序有 17 处，机械序必然择一并改写另一套的产物。这是「规范序只能取最小移动」的根本理由，也是本变更**不做**同层排序清理的理由。

### 四 · 「手改出口」的举例已被写面真相否证

三处 spec 现在写「工具表达不了的（**重排文件** / 批量整理）才手改」（`desktop-archi-agent` 两处、`desktop-phase-prompts` 一处）。规范化落地后子树连续与关系组连续由写面自动维持，「重排文件」不再是手改的正当理由；而两份 phase prompt md 与知识骨架早已只写「工具表达不了的才手改」（骨架内 `重排` 字样零命中）。**spec 与它引述的事实之间已不一致**——这是既有漂移，本变更同批校正。

### 五 · 幂等承诺与规范化天然冲突，必须显式限定

现有判据两处按「`changed = []` ⇒ 文本逐字节相等」陈述（`desktop-archi-model`「反馈面四组可对拍」、`archi_element` 幂等 Scenario）。首次写到一份**已含历史错位**的文件时（今天的 `06`），字段零变化的写也会搬迁 3 块 ⇒ 文本必变而 `changed` 仍空。不限定这句承诺，落地即自相矛盾；且这条歧义必须由 spec 说清，不能靠「反正没人这么断言」蒙过去。

---

## 提案

### 一 · 三条不变量与规范序

写入后目标文件恒满足（`E`=元素面，`R`=关系面）：

| 号 | 不变量 | 语义 |
|---|---|---|
| **E1** | 父先于子 | 元素的表在其祖先之后（现状天然成立，规范化使之为不变量） |
| **E2** | **子树连续** | 一个元素的全部后代占据紧邻它的连续区间；唯一实际修复对象 |
| **R1** | **同一 `from` 组连续** | 同一 `from` 的关系条目聚成一段；捕获「新边 append 到 AoT 尾造成组分裂」 |

**规范序的精确定义**（序敏感，不取字典序）：

- **元素面 = DFS + 同层保持首次出现序**：顶级元素按文件出现序；某元素的子元素 = 文件中所有 `<父FQN>.` 直接子，按首次出现序递归展开。
- **关系面 = 组连续 + 组间保持首次出现序**：同 `from` 的边聚成一段，段的先后 = 该 `from` 在文件中**第一次**出现的位置；段内 `to` 序原样保留，新边落组尾。
- **区域顺序**：元素区恒在关系区之前（现状 6/6 成立；混插形态会被理顺）。

**最小移动**：唯一被移动的是「不在其父子树连续区间内」的元素段与「组分裂」的关系段；同层相对序、组内相对序、每个块的内部字节（键序 / decor / 注释 / 空行）逐字保留。**对健康文件规范化是 no-op**——这正是它能挂在每次写出口的前提。

**块的口径**（`reordered` 的计量单位，见 §三）：元素面 = 一个被搬迁的「元素子树连续段」，`blocks` 放该段根元素 FQN；关系面 = 一个被并回前组的整段，`blocks` 放该段的 `from` FQN。按此口径，`06` = `moved: 3`（`Flow` / `Renderers` / `Agent` 各一段），**不是**「位置变化的元素数」（后者会报成 8）。

### 二 · 落点：`apply_*` 内、`finish` 之前，**不进自证链**

```
现状:  读源 → 改 AST（追加）→ 渲染 → 表头回填 → 重解析自证 → 回读 → 落盘
之后:  读源 → 改 AST（追加）→ 规范化（重编号 position / 重排 AoT）→ 渲染 → 表头回填 → 重解析自证 → 回读 → 落盘
```

规范的失败模式是「序不对但仍合法」。若在 `finish()` 的运行时自证里断言序，一个美观 bug 就会升级成**写入失败**（fail-closed 用错了地方）。故三条纪律：

1. **序由测试守，不由运行时守**——`finish` 链与 `verify_element` / `verify_inline_relations` 逐字不动，源码零序断言（机械断言在测试侧）。
2. **零新增 `EditErrorKind`**：规范化没有 `Err` 臂（解析已在前面完成），`EditErrorKind` 的 23 值面不变。
3. **零额外 IO**：`plan_*` 手里已有 `Model`（全文件元素序与 `line`）与 `source`，序计算是纯内存判断，不引入新读盘、新子进程、新依赖。

实现手段（`toml_edit` 0.25.15 实测）：渲染序 = `(path 非空, position)` + **稳定排序**（`src/encode.rs:183-196`），而解析器按表头连续分配 `position = 1..N`（`src/parser/document.rs:456-460`）⇒ **元素面 = 元素区内一次号重分配（permutation）**（目标序第 `i` 位编 `i+1`，关系表编 `k+1..N`）；**关系面必须真移动 Vec 成员**（`ArrayOfTables::insert` / `remove`）并同批重编号，否则 Vec 序与 position 序打架。`Table::sort_values_by` 不管子表（文档原文），故不可用；表间 decor 无需处理（`DEFAULT_TABLE_DECOR` 与既有形态同款）。

**射程两条**：① 单文件（元素不跨文件搬，主篇已定；`ModelWrite` / `expect` / `write_atomic` 本就是单文件形状）；② 只对表形态声明的元素（`[element."<FQN>"]`）动手——内联表形态的元素条目原样保留、不参与重排（写面不产出该形态）。

### 三 · 反馈面：`WriteOutcome.reordered`（唯一新增出线字段）

```json
"reordered": { "moved": 3, "blocks": ["Desktop.Views.Changes.Flow", "Desktop.Views.Changes.Renderers", "Desktop.Views.Changes.Agent"] }
```

| 字段 | 语义 | 上限 |
|---|---|---|
| `moved` | 本次顺带搬迁的**块数**（0 = 无搬家，健康文件的常态） | `usize` 天然有界；**永不截断** |
| `blocks` | 各搬迁块的首元素 FQN（元素面）/ `from` FQN（关系面），按文件序 | **cap = 8 项**，超出即截断（`moved` 仍真值），截断**不加**「…等 N 项」文本 |

形态沿仓库既有的「计数优先 + 清单有界」惯例（`archi_api` 的 `findings: { total, byCategory, items }`、db 工具的 `total` / `matched` / `returned`）。cap = 8 的依据：`WRITE_LIMITS.fqn = 256 B` 为极值 ⇒ 最坏 `8 × 256 ≈ 2.0 KB`（占 `MAX_RESULT_BYTES` 50 KB 的 4%），典型 FQN（~30 B）下约 240 B；取**前** 8 不取后 8（人读 diff 从上往下，前 8 与首批 hunk 对得上）。`moved = 0` 是主路径，故响应恒为 `{"moved":0,"blocks":[]}`（21 B）——**MUST NOT 因零值省略该字段**（与既有「投影不补默认值 / 显式字段不省略」同款）。

两个写工具的响应都带该字段（关系面同形：R1 的移动单位恒是整段，`blocks` 放 `from`），`archi_relation` 的薄形键集因此 8 → 9。

### 四 · 零暴露面：不出诊断、不出 warning、不设手动入口

三条裁决（explore §6.1，2026-11-25）逐条落地：

| 面 | 裁决 | 理由 |
|---|---|---|
| 诊断 / warning | **不加，连 warning 都不要** | 判据是「排序对解析无影响」。`DiagnosticKind` 尤其碰不得——`diagnostics` 非空即画布停摆，而 `06` 今天就有 1 处 E2 违例 |
| 运行时自证 | **不进** | 见 §二-1 |
| 手动入口 | **不要**（无 `archi_sort`，任何 `scope`、任何形态，含只读预览） | 规范化是写工具的默认行为，全量收敛由任意一次写顺带完成 |

自由文本通道造出的违例**不需要任何暴露手段**：它会在下一次工具写时被静默修好——这就是全部的处置。

### 五 · 知识面：骨架零改动，只校正三处 spec 的手改出口口径 + 工具描述加一条

- **知识骨架（`archi-knowledge.md`）零改动**：实测骨架 1777 B / 门限 1800 B（**净余 23 B**），且现有句子已是「工具表达不了的才手改，改完 `archi_module` 自查」——`重排` 字样零命中。规范化的边际信息不值得花掉 23 B 预算，也不该抬锚（explore §4.4 的「删字腾空间」前提已不成立，见 §八-②）。
- **三处 spec 校正**（`desktop-archi-agent` ×2、`desktop-phase-prompts` ×1）：「工具表达不了的（重排文件 / 批量整理）才手改」→「工具表达不了的（批量整理 / 同层手排序）才手改」，并补一句「子树连续与关系组连续由写面自动维持」。**手改面因此仍有两件正当用途**：同层兄弟手排序（规范化保留首次出现序，故手排能存活）与批量整理；而被手改**拆开**的子树会在下一次工具写时被合回（该注意事项进工具描述，不进骨架）。
- **两写工具描述各加一句（第三 → 四件事）**：写入会把目标文件归一到规范序（子树连续 + 关系组连续），手改拆开的子树在下次工具写时会被合回，顺带搬家的块经响应 `reordered` 报出，故工具描述项从三件扩为四件（`desktop-archi-agent`「archi 写工具面」的「工具描述面」条随之改写）。

### 六 · `06-frontend` 的一次性搬迁（同批人工修）

E2 一旦落地，首次写到 `06` 必然夹带 3 块搬迁。**建议在本变更内做一次纯搬迁**，使首个功能 diff 不含夹带：

```
现  … Changes │ Config DbInspector Explores Info Welcome │ Changes.Flow Renderers Agent
目  … Changes │ Changes.Flow Renderers Agent │ Config DbInspector Explores Info Welcome
```

只动位置：18 行（3 个元素块 × 5 行 + 3 个空行）整体前移，块内字节逐字不变、元素 / 关系集合与表头计数不变。行效应：原行 40–69 下移 18 行、原 70–87 移到 40–57、**关系区（88 行以后）行号零变**（删 18 加 18 都在其上方）——`desktop-archi-canvas` 里 `06-frontend.toml:512` 的示意仍然成立。

### 七 · 与 explore 的偏离（逐条）

| # | explore 草案 | 本提案 | 理由 |
|---|---|---|---|
| ① | §4.4 要求在知识骨架里把「（重排 / 批量整理）」收成「批量整理」 | **骨架零改动**；改的是三处 spec 的举例 | 骨架现状已无该括号（实测 1777 B，`重排` 零命中），且预算只剩 23 B；spec 与 md 的不一致才是要修的那件事 |
| ② | §6.1-b 只否掉诊断 / warning | 另**不**新增任何知识句（「写入自动归一」只进工具描述） | 同上：骨架无预算；工具描述无上限且是「工具语义」的正当落点 |
| ③ | §4.5 `reordered` 汇报 | 同（`{moved, blocks}`、cap = 8、`moved` 永不截断）；**cap 由引擎侧 `Reorder` 构造时施加** | 两写工具共用一份 cap，避免 infra 侧出现第二份截断逻辑 |
| ④ | §6.3-4「建议人工修 06」 | 采纳，并**列为变更范围的一行** | 让夹带搬迁不出现在首个功能 diff 里 |
| ⑤ | §2.1 记 `05` 有「1 跨 `from` 组」违例 | 实测 **6/6 组连续**；那 1 处是 `(from,to)` 字典序违例（不在本变更射程） | 逐条重算后以实测为准 |

---

## 能力

### 新增能力

无。写入序是**写回原语族的不变量**，归 `desktop-archi-model`（引擎侧语义）；工具面只是把引擎事实序列化，归 `desktop-archi-agent`。拆出去会把「一次写的产出形状」变成两份持有者。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-model` | **MODIFIED ×1** | 「模型写回原语族（元素 / 关系 upsert 与 delete）」整段拷贝后改，差异逐条：① 新增**写入序不变量**条（E1 / E2 / R1 + 规范序定义 + 最小移动 + 幂等的可断言形态 + 单文件 / 表形态射程 + 零暴露面 + 不进自证链 + 无手动入口）；②「八函数两面」补「`apply_*` 产出为文本 + `Reorder`」；③「产出形状」补 `reordered` 字段与其 cap / 真值 / 恒在场语义，并把「`changed` 空 = 幂等」限定为「`changed` 空 ∧ `moved = 0`」；④「渲染纪律」把「原样保留」限定为**块内字节与键序**（位置面由规范序决定）；⑤「自证对象 = 交付文本」的产出顺序插入「规范化」一环并明写「不进自证链」；⑥「表头计数回填」补「规范化复用同一函数」；⑦「乐观锁」补「规范化落在 `expect` 校验之后、`hash` 计算之前」；⑧「关系落点」补「新边落 `from` 组组尾」；⑨「错误面」补「本变更零新增变体」；⑩ 两条既有 Scenario（「改档保留注释与其余内容」的位置例外、「反馈面四组可对拍」的逐字节相等）按 AC-4 限定，「新建元素」Scenario 补 `moved` 的分情形断言（子树区间已是文件尾 ⇒ 0，否则 ≥ 1）；⑪ 新增 5 条 Scenario（三不变量 / 序敏感幂等 / 关系组连续与两处同源 / 搬迁块有界 / 序违例不产诊断）。 |
| `desktop-archi-agent` | **MODIFIED ×4** | ①「模型 = 源码（两条写入口与写纪律）」整段拷贝后改：手改出口表格行「重排文件 / 批量整理」→「批量整理 / 同层手排序」；新增两条「文件序边界」bullet（子树与关系组由写面维持、手改余量）；「两入口并存」Scenario 补 `archi_sort` 零存在；新增 Scenario「手改与写面对文件序的分工可辨」。②「架构会话的系统知识注入」整段拷贝后改：手改出口句保留骨架简写（「工具表达不了的才手改」）并补「适用边界由本能力裁定 = 批量整理 / 同层手排序」与「拆开会被合回」；工程约束首条补「文件序不进骨架」；体量锚补「本变更零字节改动（1777 / 1960）」；两条 Scenario（手改出口、体量实测）按此加限定。③「archi 写工具面」整段拷贝后改：写面语义 bullet 补「写入序不变量」与「MUST NOT 自行计算搬迁块」；工具描述面由三件事扩为四件事；两条 Scenario（元素 upsert、内联边等价）补 `reordered`；新增 Scenario「描述面四件事齐备且不教算法」。④「写工具的响应体与错误面契约」整段拷贝后改：jsonc 补 `reordered`；`warnings` / `changed` / `hash` / 体量上界四条 bullet 各补一句（其中 `warnings` 条的交叉引用去掉过时的「七条」字样——基线标题已是「五条」，属既有漂移的顺带校正）；新增 `reordered` bullet；薄形键集 8 → 9；错误面补「序违例不在任何一层」；两条 Scenario 补限定；新增 Scenario「搬迁块在两写工具同形」。 |
| `desktop-phase-prompts` | **MODIFIED ×1** | 「桌面端特性适配（迁移是改写非搬运）」整段拷贝后改，差异仅三处：写权相位「写面与自查」项的手改出口由「重排文件 / 批量整理」改为「工具表达不了的才用（**口径**：批量整理 / 同层手排序；子树与关系组由写面自动维持，**md 侧保留简写**）」；「该段 MUST NOT …」句补「MUST NOT 要求 agent 执行任何文件排序动作（写入序归工具）」；「写权相位指示写面与自查」Scenario 补「MUST NOT 出现『重排文件』一类表述」。两份 planner md **零改动**。 |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-drift` | 判据、五值 `kind`、报告字段与 `modelHash` 口径逐字不变；规范化不进 drift，也不新增判据。模型字节变化（`06` 搬迁）只让 `modelHash` 在运行期变化——这是本变更**如实记录**的事实，不是判据变化。 |
| `desktop-archi-api` / `desktop-archi-api-check` | `ArchiApiFindingKind` 值集、severity 单点、`docMissing` 不纳管、`passed` 判据逐字不变；`api:check` 应零差异（`Backend.Core.Archi` 无 `.schema.json` 在盘 ⇒ 不受 `archi` crate 新增内部 API 影响）。 |
| `repo-archi-model` | 元素 / 关系计数逐字不变（55 / 129、六段 `2/0 · 9/10 · 8/25 · 1/14 · 3/3 · 32/77`）；`06` 只换块位置，不增删元素、不改表头注释。**`modelHash` 必变**（模型字节变了），本条按既有「读数写实」纪律不许写成「逐字不变」。 |
| `desktop-archi-canvas` / `desktop-archi-gate` / `desktop-archi-contract-panel` / `desktop-archi-file-tree` | 画布改档仍走 `archi_set_gate` → 同一 `plan_element_upsert`（规范化对内嵌其中，签名与命令面逐字不变）；契约面板 / 门禁 / 文件树与本变更无交集；`06` 搬迁后关系区行号不变，canvas 的 `:512` 示意仍真。 |
| `desktop-agent-execution` | 工具清单五读 + 两写、tier 归属、archi 上下文铸造判据、矩阵逐字不变——本变更不新增工具。 |
| `desktop-ipc-type-bindings` | 零 DTO / 零命令变化：`WriteOutcome` / `Reorder` 是 Rust 侧进程内类型，不出线；`bindings:export` 应零 diff。 |
| `desktop-write-protection` | 写保护判定面（目标文件**路径**）与拒串族不变：规范化只改文件内容，不改目标路径、不改判定时机（保护闸仍在 `plan_*` 之后按解析出的目标文件判定）。 |
| `desktop-corpus-regression` / `desktop-mutation-testing` | 语料与变异门限机制不变；新代码须成对测试（见风险 R-8），但不改机制。 |
| 前端各能力（`desktop-archi-chat` 等） | 与本变更无交集；`desktop-archi-chat` 的「工具表达不了的才用」句已无 `重排文件` 字样，零 delta。 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit/layout.rs`（新） | 规范序纯函数：元素面 DFS 目标序 + `position` 重编号（元素区 `1..k`、关系区 `k+1..N`）；关系面按 R1 重排 `ArrayOfTables` 并同批重编号；`Reorder { moved, blocks }` + `REORDER_BLOCKS_CAP = 8`（cap 在构造处施加）；零 IO、零 `Err` 臂、零诊断构造 |
| `.../crates/core/archi/src/edit/mod.rs` | 四个 `apply_*` 在改完 AST 之后、`finish` 之前调用规范化，产出由 `String` 扩为「文本 + `Reorder`」；`WriteOutcome` 增 `reordered`；`plan_*` 装配该字段。`finish` / 自证 / 回读 / 表头回填 / 三态 / 落点 / 级联 / `expect` 逐字不动 |
| `.../crates/core/archi/src/lib.rs` | 出口随动：`Reorder`（既有 `apply_*` / `WriteOutcome` 出口面不变） |
| `.../crates/infra/agent/src/sdk/archi_tool.rs` | 两处响应装配加 `reordered`（元素面与薄形各一处；零计数、零额外解析） |
| `.../crates/infra/agent/src/sdk/mount.rs` | 两写工具 `description` 各加一句（写入归一到规范序 / 手改拆开的子树会被合回 / 顺带搬家经 `reordered` 报出）；`parameters` 与上限常量零改动 |
| `.../crates/core/workflow/src/prompts/{proposal-planner,dev-design-planner}.md` | **零改动**（登记以便评审核对：现有句子「工具表达不了的才手改」与规范化相容） |
| `.dev-team/architecture/graph/model/06-frontend.toml` | 3 块搬迁（18 行整体前移到 `Desktop.Views.Changes` 之后）；块内字节、元素 / 关系集合、表头计数逐字不变 |
| `packages/desktop/package.json` | `version` patch +1 |

### 测试文件

| 文件 | 改动 |
|---|---|
| `.../crates/core/archi/src/edit/layout_test.rs`（新） | 规范序三不变量：DFS 目标序（父先于子 / 子树连续 / 同层首次出现序）、正文乱序夹具的搬迁结果、**健康文件 no-op**（六类形态）、`position` 重编号（元素/关系交界：单元素文件 / 零关系文件 / 混插文件各一，断言渲染全文逐字）、关系组连续（组分裂并回 / 组间首次出现序 / 组内序不动 / 新边落组尾）、**Vec 序与文本序同源**（读出的关系序 == 文本序）、幂等（同一输入排两次同结果）、`moved` / `blocks` 口径（06 形态 ⇒ `moved = 3`、`blocks` 三值）与 cap 边界（>8 块夹具 ⇒ `moved` 真值、`blocks` 恰 8 项）、内联表形态零重排 |
| `.../crates/core/archi/src/edit/mod_test.rs` | 四个 `apply_*` 的 `reordered` 透出；**既有夹具归类**：违反 R1 / E2 的夹具（已知 `RELATIONS_SOURCE` 的 `A` 组分裂，`关系缺席追加且缺键即建数组` 的「新条目在尾」断言）改为合规夹具或改断言为「并回组尾」；`untouched == source` 类零差异断言保持在健康夹具上；既有 12 组用例逐条保留 |
| `.../crates/core/archi/src/dto_test.rs` / `graph_test.rs` / `parse/*` | **零改动**（登记以便评审核对：规范化不碰判据面与投影面，故不应出现红点） |
| `.../crates/infra/agent/src/sdk/archi_tool_test.rs` | 元素响应含 `reordered`（新建元素落在组中段 ⇒ `moved ≥ 1` 且 `blocks` 含该 FQN；同值重放 ⇒ `moved = 0`）；薄形键面断言 8 → 9（`reordered` 进场，其余键逐字不变）；cap 夹具（响应 < 3 KB） |
| `.../crates/infra/agent/src/sdk/mount_test.rs` | 描述面正向断言加第四条（写入归一到规范序 / `reordered`）；既有三条与「零形态示例」断言逐字保留 |
| `packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs` + `corpus/**` | **零改动**（登记以便评审核对：语料不经写面） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.../crates/core/archi/src/parse/**`、`model.rs`、`graph.rs`、`drift/**`、`api/**` | 判据面 / 投影面 / 哈希面零变化：规范化 MUST NOT 重做形态校验、MUST NOT 跑 `drift::check`、MUST NOT 让 `DiagnosticKind` 出现新用法 |
| `.../crates/core/archi/src/edit/messages.rs` | **零新增 `EditErrorKind`**（规范化无 `Err` 臂）：23 值面与 `ALL` / `criterion()` 逐字不变 |
| `.../crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs` | 骨架字节零改动：1777 B / 1960 B 冻结断言与 1800 / 2000 门限不动（净余 23 B，不花也不抬锚） |
| `.../crates/infra/agent/src/sdk/policy.rs`、`mount.rs` 的清单 / 档位 / 矩阵段、`compose.rs` | 工具数量（18）与档位（五读 + 两写）不变；`archi_matrix` 与写权锁对齐断言零改动 |
| `.../crates/infra/agent/src/sdk/archi_tool.rs` 的保护闸 / 落盘段 | `ProtectRules` 判定与 `write_atomic` 单点逐字不变（本变更只加一个响应字段） |
| 除 `06-frontend.toml` 外的五个模型文件、`.dev-team/architecture/graph/INDEX.toml`、`models/**`、`.i.rs` / `.i.ts` | 零字节：`01`–`05` 规范化是 no-op；`api:export` / `api:check` 应零 diff |
| `openspec/specs/**` 基线、本 change 的 `explore.md` | delta-only；手改基线即「同一事实两份持有者」；explore 只读。基线内「重排文件 / 批量整理」三处旧句由本 change 的 delta（MODIFIED）在归档合并时替换，**基线保持原样是本 change 的判据** |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 零 DTO 变化 ⇒ `bindings:export` 零 diff（**若出现 diff 即误改**） |
| 前端产品代码（`packages/desktop/src/**`） | 零 UI 变化：画布改档签名与交互逐字不变 |
| `src-tauri/Cargo.toml` / `tauri.conf.json` | crate 版本行 MUST NOT 跟随 `package.json`（AGENT.md 版本纪律） |
| `AGENT.md` / `CLAUDE.md` | 规范面，本变更不触 |

---

## 验收标准

1. **AC-1 · 三条不变量成立**：对六个模型文件各做一次「把 `gate` 改成其当前值」的写回，产出与源**逐字节相等**、`reordered.moved = 0`（`06` 已按 §六 人工搬迁后）；在**未搬迁**的 `06` 夹具上（临时目录副本）结果为 `moved = 3`、`blocks` 恰 `["Desktop.Views.Changes.Flow", "Desktop.Views.Changes.Renderers", "Desktop.Views.Changes.Agent"]`、其余五文件零 diff。
2. **AC-2 · 规范序定义可断言**：输出满足 E1（父先于子）+ E2（子树连续）+ 同层首次出现序；关系面满足 R1（组连续）+ 组间首次出现序 + 组内序不动；元素区恒在关系区之前。三类交界夹具（单元素文件 / 零关系文件 / `[element.*]` 与 `[[relation]]` 混插文件）各一条，断言渲染全文逐字。
3. **AC-3 · 最小移动与幂等**：规范化对健康文件 no-op（同一输入排两次结果相同、逐字节相等）；`moved` 只计被搬迁的块，不计「位置变化的元素数」（`06` ⇒ 3，不是 8）；重复同一写 ⇒ `changed = []` ∧ `moved = 0` ∧ 盘上字节零变化。
4. **AC-4 · 幂等承诺的可断言形态**：`changed = [] ∧ reordered.moved = 0` ⇔ `content` 与源逐字节相等；`moved > 0` ⇒ `content` 必变而 `changed` 仍为空（首次规范化）。spec 两处（「反馈面四组可对拍」与 `archi_element` 幂等 Scenario）按此限定，MUST NOT 留「`changed` 空 ⇒ 字节相等」的无条件表述。
5. **AC-5 · 关系面两处同源**：`ArrayOfTables` 的 Vec 序与 `position` 序同批修改——断言「模型读出的关系序列 == 文本中的关系序」；新建关系落 `from` 已有组的组尾（不是在 AoT 尾）；组分裂并回后组内相对序不变。
6. **AC-6 · 零暴露面**：`DiagnosticKind` / `EditErrorKind` / `ArchiDriftKind` / `ArchiApiFindingKind` 的值集逐字不变；`edit/layout.rs` 零诊断构造、零 `drift::check` 引用；`warnings` 不因规范化新增条目（`06` 形态的搬迁不产任何 warning）；`archi_drift` 报告结构与 `status` / `passed` 判据零变化。
7. **AC-7 · 不进自证链**：`finish` / `verify_element` / `verify_inline_relations` 逐字不动；源码对「序断言」零命中（机械断言只在测试侧）；一个含 E2 违例的源 + 一次合法 upsert ⇒ `Ok`（美观问题不得升级为写入失败）。
8. **AC-8 · `reordered` 形状与上限**：字段恒在场（零值也写 `{"moved":0,"blocks":[]}`）；`moved` 真值永不截断；`blocks` ≤ 8 项且按文件序；>8 块夹具下 `moved` 为真值、`blocks` 恰 8 项、成功响应 < 3 KB；两写工具响应都带该字段，`archi_relation` 薄形键集恰 9 项。
9. **AC-9 · 零解析影响与零生成物 diff**：写后 `archi_drift` 的结构诊断集合、元素 / 关系计数、`apiFindings` 构成逐项不变；`api:check` / `api:export` 零差异；`bindings:export` 零 diff；除 `06` 外五个模型文件零字节；`06` 只换块位置（块内字节、元素 / 关系集合、表头计数逐字不变，行数不变）。
10. **AC-10 · `06` 搬迁的行效应**：关系区（原 88 行之后）行号零变化，`06-frontend.toml:512` 仍是同一条关系条目；原 40–69 下移 18 行；`modelLine` 与新文本一致（随文本走是正确行为）。
11. **AC-11 · spec delta 形态与旧句零残留**：`openspec/changes/toml-sorting-and-layout/specs/` 恰**三个**能力目录（`desktop-archi-model` / `desktop-archi-agent` / `desktop-phase-prompts`）；**六个** MODIFIED 块（与「能力」表逐格对拍：`desktop-archi-model` ×1、`desktop-archi-agent` ×4、`desktop-phase-prompts` ×1）为基线整段拷贝后改（`### Requirement` 标题逐字一致、Scenario 净增不减——实测 13→18 / 8→9 · 8→8 · 5→6 · 5→6 / 7→7、未丢任何未冲突条目），且与基线的差异**逐条**落在本提案「能力」表所列条目内（多一条即需回改提案或回改 delta）；每条 `### Requirement` 至少一个 `#### Scenario:`（恰 4 个 `#`）且 WHEN/THEN 成对；各 delta 含 `## Module Contract`。`openspec/specs/**` 基线零改动。**旧句判据**：新文本 MUST NOT 把「重排文件」写成肯定命题——该串只允许出现在禁令句（「MUST NOT 再表述为…」）与现状引述里，不得出现在「手改出口」的适用边界正面表述中。
12. **AC-12 · 版本与改动集**：`package.json` `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/Cargo.toml` 逐字不变；`git status --porcelain` 的改动集 ⊆ 本提案「变更范围」所列文件（另加本 change 目录下的 `proposal.md` / `specs/**` / `reports/**`；`openspec/specs/**` 基线零改动）；基线测试面（10754 项）零回归。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **幂等语义滑移**：首次写到含历史错位的文件时 `changed = []` 但文本变化，既有断言写成无条件「字节相等」 | AC-4 给出可断言形态（`changed` 空 ∧ `moved` 0 ⇔ 字节相等），spec 两处 Scenario 同批限定；测试加「脏文件首次写」夹具钉住 |
| R-2 | **既有夹具违反 R1 / E2**：`edit/mod_test.rs::RELATIONS_SOURCE` 的 `A` 组分裂、`关系缺席追加且缺键即建数组` 的「新条目在尾」断言，规范化后必红 | 落地时按两类处置：健康语义夹具改成合规序；违规语义夹具改断言为「并回组尾 / `moved ≥ 1`」；MUST NOT 用 `#[ignore]` 或删断言换绿 |
| R-3 | **`position` 是 `toml_edit` 内部状态**：渲染序依赖「稳定排序 + 解析连续分配」两条实现事实，升级依赖可能静默改行为 | 断言口径限定在「模型读出的序 == 文本序」而不是 position 取值；`layout_test` 的混插 / 交界夹具即该行为的守卫，升级 `toml_edit` 时先红后判 |
| R-4 | **关系面双源**：只改 Vec 序或只改 position 序都会让渲染序与读回序打架（`apply_relation_upsert` 的 `(from,to)` 定位依赖 Vec） | AC-5 用「Vec 序与文本序同源」一条断言钉死；两处在同一函数内同批改，MUST NOT 拆成两步提交 |
| R-5 | **agent 看到意外 diff**：新元素被搬到父的子树中段，diff 里出现「搬家」而无字段语义 | `reordered.blocks` 显式报出（`moved` 真值不截断）+ 工具描述第四件事写明「写入会归一 + 手改拆开的子树会被合回」；观察期若仍困惑，后续变更再谈骨架（本变更不动 23 B 预算） |
| R-6 | **手改面被静默改写**：有人为阅读有意拆开子树，下次工具写会合回 | 这是不变量的本意（AC-1 的对偶面）。工具描述 + 三处 spec 句如实写「子树与关系组由写面维护」；MUST NOT 表述为「工具会保留任何手排位置」 |
| R-7 | **`06` 一次性搬迁与「零模型字节变化」惯例相冲** | 本提案明确列为变更范围一行并在 AC-9 / AC-10 写实（只换位置、计数不变、`modelHash` 必变）；若不搬迁，首个功能写会把 3 块夹带进无关 diff——两者相权取搬迁 |
| R-8 | **新代码的测试 / 变异覆盖**：`layout.rs` 是纯逻辑密集模块，容易被变异门限卡住 | 单测按「三不变量 × 三类交界 × cap 边界」成对铺（`layout_test.rs`），变异存活者优先补边界（空关系 / 单元素 / 组分裂 / >8 块 / 内联表形态） |
| R-9 | **MODIFIED 整段拷贝易漏条目**（6 个 requirement：model ×1 / agent ×4 / prompts ×1；基线单条最长 83 行，delta 最长 136 行——model delta 自 `### Requirement` 起至文件末的口径） | 落地时以基线文件直接复制后定点改（改动点已逐条列在「能力」表），MUST NOT 手抄重排；交付后逐条对拍「基线条目数 == delta 条目数」 |
| R-10 | **spec 与知识骨架再次分叉**：spec 的举例（批量整理 / 同层手排序）比骨架句更细 | spec 明写「md 侧一句为简写，括号内举例是 spec 侧对适用边界的裁定」；骨架字节零改动由 AC 守住（1777 / 1960 冻结值不变） |
| R-11 | **`archi_set_gate`（画布）也走规范化**：改档可能夹带搬家，用户会以为画布写坏了 | 与写工具同一实现、同一 `reordered` 事实；`06` 已人工搬迁将夹带面清零；画布命令面签名与失败语义逐字不变（零 delta 的 `desktop-archi-canvas` 即该承诺） |

---

## explore 未决项与「不含」的归属

| explore 条目 | 处置 |
|---|---|
| §1.4 / §6.3-1 一次字段修改可能夹带搬家 | **接受**（`reordered` 是它的可见化手段）；`06` 同批人工搬迁以清零首次夹带 |
| §2.1 同层字典序违例 7 处 | **不含**：保留手排序（规范化只保子树连续，同层首次出现序原样保留） |
| §2.1 关系 `(from,to)` 违例 17 处 | **不含**：14 处组内 `to` 手选序 + 跨组 3 处，机械序会摧毁「生成物中继边提前」这套排布 |
| §5.2 S3 独立 `archi_sort` 工具（任何 `scope`） | **不做**：规范化是写工具默认行为；`scope = all` 与单文件写面冲突 |
| §6.1-b 违例的暴露面（诊断 / warning） | **不做**：排序对解析无影响，唯一目的是 diff 整洁；自由文本造出的违例由下次工具写静默修好 |
| §6.2 元素区 / 关系区交界号分配、关系面双源、cap 边界 | **转验收**：AC-2 / AC-5 / AC-8 逐条覆盖（proposal 阶段定口径，细节归 design） |
| §七 与主篇的关系（不阻塞主篇） | **成立**：主篇已归档；本变更只在其 `apply_*` 内插一层，工具签名不变，响应只多一个字段 |
