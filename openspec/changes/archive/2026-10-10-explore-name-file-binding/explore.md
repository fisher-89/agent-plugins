# 探索名称与笔记文件绑定问题

日期:2026-10-08 · 状态:调研中

## 背景

desktop 探索页新建「新话题」时，用户输入的是 kebab-case 主题名（建档），但真正进入详情页后用户输入的是「探索问题」，agent 执行时根据问题内容自行总结 topic 并落盘 `openspec/explores/<topic>.md`。两条链路各自命名，导致 DB 记录名（`ExploreRecord.name`）与磁盘文件 stem 分叉：清单展示名 / 详情预览寻址（`read_explore(root, name)`）与 agent 实际写出的文件对不上，预览读不到内容，探索内容与清单条目无法关联。

用户提议：新建时用户直接提问（不必自己起 kebab-case 名），由 agent 总结探索名称并写入数据库，使 name 与文件 stem 天然一致。

## 关键事实链

```
新建「新话题」                   详情页 composer                    agent 落盘
用户输入 kebab-case 主题名 ──▶ create_explore_record ──▶ 用户输入「探索问题」
        │                        （只写 DB，不落盘）                │
        ▼                                                          ▼
ExploreRecord.name = "api-retry"              buildExplorePrompt(stance + 问题)
        │                                                          │
        │                        stance 未传 record.name ◀────────┘
        ▼                                                          ▼
read_explore(root, "api-retry")          agent 自拟 topic 写 api-retry-backoff.md
        │                                                          │
        └────────────── ✗ stem 不一致，读不到 ────────────────────┘
```

- `ExploreRecord`：主键 `id` 与 `name` 解耦；`name` 双职（展示名 = 磁盘寻址键）。见 `model.rs`。
- stance 模板（`lib/explore-stance.ts`）只拼「立场 + 笔记落盘约定 + 用户输入」，**未把当前记录的 name 注入**；其中「topic 与用户给定的主题一致」在用户只给问题的场景下形同虚设，agent 只能自拟。
- `rename_explore_record` 后端能力已存在（store 层 in-place 改名保主键保链、命令层、`use-explore-list` 的 `rename` action 均已就绪），**但 UI（清单条目）未暴露改名入口**。

## 用户方案的架构障碍

1. **agent 无法写 DB**：desktop 的 explore 会话跑在 core agent 内核上；SDK 引擎 MCP 恒空（`infra/agent/src/sdk/loop.rs`「MCP 恒空（本期无 MCP 面）」），CLI 引擎 MCP servers 来自外部 claude CLI 配置。desktop 没有给 explore 会话注入「写 explore record」的工具面，agent 只能写磁盘（bash/write），DB 写入只能走 Tauri command。
2. **鸡生蛋**：explore 会话归属靠 `sourceRef = record.id`（十进制串），即「先有 record 才能跑 agent」；而 record.name 恰恰是要由 agent 产出的东西——命名前必须先有可归属的 record。

## 候选方向（未定）

- **方向 1 · 最小贯通**：把 record.name 注入 stance，令 agent 落盘文件名强制等于 `record.name`（MUST NOT 自拟）。用户仍需自己起名；DB/文件天然一致。
- **方向 2 · agent 命名 + 事后 rename**：用户输入问题 → 建占位 record → 首条消息指令「总结 topic 并落盘」→ 事后把 DB name 改成 agent 落盘的 topic。触发点两个变体：
  - 2a 前端 watch 检测落盘后自动 rename（watch 信号不含文件名，可靠拿到 topic 名困难）；
  - 2b 给 agent 注入「写 explore name」MCP 工具（新增 MCP 面，成本最高，且与 orchestration「落账由桌面代写、禁调 MCP 写通道」的红线相悖）。
- **方向 3 · 前端本地 slug 化**：用户输入问题，前端确定性 slugify 生成 name 写 DB 并注入 stance。满足「只需提问 + name/文件一致」，但「总结」非 agent 所为。
- **方向 4 · 改名 UI 补丁**：暴露既有 rename 能力，详情页手动把 name 对齐到 agent 落盘文件名。手动补救，不解决自动关联。

## 待澄清

- 「探索名称」在用户心智里是「清单里一眼认出的标题」，还是必须等于「磁盘文件名」？两者能否解耦（name 只管展示，另设 file_stem 字段做寻址键）？
- 是否接受「用户仍需自己起名」的方向 1，还是坚持「用户只提问、agent 命名」的体验目标？

## 三个目的（用户补充，2026-10-08）

1. 清单里更好认出标题
2. 探索可以正确关联结果文件（也可以将结果落在其他位置）
3. 未来打通探索和变更，一键从探索启动变更

## 目的对应的现状缺口

### 目的 1 · 标题识别

- 现状：清单条目只显示 `record.name`（kebab-case stem），可读性差。
- 缺口：`ExploreRecord` 没有「人类可读标题」字段；`name` 被磁盘寻址职责绑架，放不了中文 / 空格 / 长标题。

### 目的 2 · 结果文件关联（可落其他位置）

- 现状：读面硬编码固定目录。`workflow/queries/explore.rs` 的 `read_explore` 恒拼 `explores_root/<name>.md`；`explore_doc_path` 同样；watch 也只订阅这一个文件。`Layout.explores_root` 恒为 `<domain>/explores`（扁平集合，无 active/archive 两棵树）。
- 缺口：探索结果一旦落到别处（如 `changes/<name>/explore.md`，或用户自定义位置），desktop 的 explore 预览根本读不到——没有「结果文件位置」字段，读面、watch 面、scan 面全部锚死单一目录单一文件名。

### 目的 3 · 探索 → 变更打通

- 现状（change 侧已有独立闭环）：
  - `change create` 时 goal 原文直写 `changes/<name>/explore.md`（`write/create.rs` 的 `write_fs_half`）；
  - change 详情页经 `markdown-doc` 插件把 `explore.md` 收录为「探索」产物，`ArtifactTabs` 可读（`artifacts/markdown_doc.rs` 的 `CURRENT_DOCS` 名单）；
  - proposal 相位有 explore→proposal 文件式交接：`write/phase_table.rs` 的 `proposal_explore_handoff()` 让提案 agent 读 `changes/<change>/explore.md` 作 free-form 上下文。
- 缺口：**desktop 的 explore 记录与 change 零关联**——`ExploreRecord` 没有指向 change 的字段；没有「从探索启动变更」的入口/命令；`openspec/explores/<topic>.md` 与 `changes/<name>/explore.md` 之间没有 promote/迁移路径（CLI skill 有 promote 机制，desktop 侧未落地）。

## 重新梳理：三目的共同根因

三个目的收敛到同一个结构性问题——**`ExploreRecord.name` 是「展示名 + 磁盘寻址键」双职字段，且缺「结果文件位置」「change 关联」两个维度**。这不是单纯「谁来命名」的交互问题，而是数据模型演进问题。

```
当前                           目的要求的形态
─────────────                  ─────────────────────────────
ExploreRecord {                ExploreRecord {
  id          ← 身份(会话锚)      id          ← 身份(会话锚)
  root                           root
  name        ← 双职:展示+寻址    name        ← 磁盘寻址键(stem)
  created_at                     title       ← 人类可读标题 (目的1)
  updated_at                     file_path?  ← 结果文件位置 (目的2)
}                                linked_change? ← 关联 change (目的3)
                                 ...
```

`id` 是真正的身份锚（会话 `source_ref` 指向它），`name` 本可自由改名——这为「标题/寻址键解耦 + 文件位置独立 + change 关联」留足了空间。

## 新的候选方向（供选择）

- **方向 A · 数据模型先行**：给 `ExploreRecord` 加 `title`（目的1）+ 结果文件位置（目的2）+ `linked_change`（目的3）字段，`name` 退化为纯 stem 寻址键。这是三目的的共同底座，其余交互都建立在其上；代价是 schema 演进（native_model 加版本）、命令层 / 前端多面改动。
- **方向 B · 探索结果并入 change 的 explore.md**：既然 change 侧已有 explore.md 闭环，「探索」可以视作 change 的前置草稿，落盘/关联直接复用 `changes/<name>/explore.md`，promote = 建 change 时把草稿迁入。目的 3 顺带解决，目的 2 的「落其他位置」也自然覆盖；但会模糊「无 change 探索」与「有 change 探索」的边界。
- **方向 C · 交互面补丁**（在 A 之前先做）：先暴露 rename UI + name 注入 stance，让「预览读空」这个最痛的问题止血；数据模型演进留待后续。

## 新待澄清

- 三个目的是否有优先级？「预览读空」是否是要先止血的燃眉之急，还是三目的要一起规划？
- 「结果落其他位置」具体指哪些位置？是仅「并入 change 的 explore.md」，还是允许用户任意指定路径？
- 目的 3 的「一键启动变更」，期望产物是「把 explore 笔记迁入 change explore.md」还是「新建 change 并带一个指向 explore 的引用」？两者语义不同（move vs link）。

## 用户决策（2026-10-08）

1. **一起规划**（三目的共同规划，先动数据模型底座）；
2. **文件位置由我规划**（「落其他位置」仅作为一种可能，是否改文件位置或计库由我判断）；
3. **move**（探索笔记迁入 change 的 explore.md）。

## 关键架构事实（支撑规划的硬约束）

### A. 数据模型演进机制现成（零迁移可行）

`ChangeRecord` 已示范「加字段」的正确姿势：`native_model(id = 9, version = 2, from = ChangeRecordV1)`，新字段 `worktree` / `base_commit` 用 `#[serde(default)]` 缺省读兼容，降级半边丢弃占位。`ExploreRecord` 当前 `version = 1`，加 `title` / `promoted_to` 可同模式走 `version = 2`，零迁移代码。

### B. change 落 worktree，explore 在主仓——move 是内容搬运，不是 rename

- `create_change` 的 explore.md 写在 **worktree 内**（`create.rs`：`change_dir = worktree_layout.changes_root.join(name)`），不是主仓。
- 探索笔记在**主仓** `openspec/explores/<topic>.md`（explore 会话 cwd = 主仓 root）。
- worktree 从主仓 HEAD 基线检出，主仓未提交的 explores 笔记**不在 worktree 里**。
- 因此 promote（move）= 读主仓笔记内容 → 写进 worktree 内 `changes/<name>/explore.md` → 删主仓原文件。跨 worktree 边界的内容搬运，不是 `fs::rename`。

### C. change 的 explore.md 目前 = goal 原文（既有契约）

`write/create.rs` 的 `write_fs_half` 把 goal 原文直写 explore.md；spec `desktop-change-create` 明确「goal 原文写入 explore.md」。promote 语义要处理「探索笔记全文」与「goal」的关系（谁覆盖谁 / 谁在头谁在尾）。

### D. CLI 侧 promote 语义 = move（与用户选择一致）

`openspec-explore/SKILL.md`：「phase-proposal ... promotes the matching draft into openspec/changes/<name>/explore.md (move)」。desktop 侧的 promote 应与此对齐。

### E. 删探索记录会级联删会话链（move 后记录不能简单删）

`delete_explore_record` 同事务级联删除名下会话（`source="explore"` + `source_ref=record.id`）。探索对话历史是有价值的资产，move 笔记后 MUST NOT 删记录，否则整个探索对话链消失。

## 规划蓝图（供讨论）

### 数据模型：`ExploreRecord` v2

```
ExploreRecord v2 {
  id           ← 身份锚（会话 source_ref 指向；永不变）
  root
  name         ← stem 寻址键（退化为纯内部 slug，不再承担展示）
  title        ← 人类可读标题（目的1，新增）
  promoted_to  ← Option<String>，指向 change name（目的3，新增）
  created_at
  updated_at
}
```

- **不引入自由 file_path 字段**（目的2 的判断）：位置由 `promoted_to` 状态确定性派生，只有两种合法形态——`promoted_to = None` → `explores/<name>.md`；`promoted_to = Some(change)` → `changes/<change>/explore.md`。存自由路径会迫使 read/scan/watch 三面放开、且引入「库路径与磁盘漂移」风险，违背 change 域「位置由 layout 派生」的既有纪律。
- `name` 退化为 stem：`title` 承担展示，「agent 总结名称」的诉求由 `title` 承接（agent 在笔记里产出标题，桌面侧解析，绕开「agent 无 DB 写通道」的约束）。

### 探索生命周期（move 状态机）

```
        [草稿]                         [已转变更]
  promoted_to = None               promoted_to = Some(<change>)
  笔记: explores/<topic>.md        笔记: changes/<change>/explore.md (已 move)
  ├─ 清单显示 title                 ├─ 清单显示 title + 「已转变更」徽标
  ├─ 预览读 explores/<topic>.md     ├─ 预览显示「已 promote 到 <change>」
  └─ 可一键启动变更 ──promote──▶    └─ 会话链保留（记录不删，仅标记）
```

### promote（move）落点

「从探索启动变更」= 新命令/入口，语义链：

```
读主仓 explores/<topic>.md
   │
   ▼
create_change(root, name=?, goal=?)   ← 建 worktree + change 记录
   │  write explore.md = 探索笔记内容（goal 关系待定，见决策点）
   ▼
删主仓 explores/<topic>.md            ← move 的「移走」半边
   │
   ▼
ExploreRecord.promoted_to = <change>  ← 记录标记（会话链保留）
```

## 待定决策点（收敛后的少数真问题）

1. **title 的生成权**：用户手填 / 前端从问题截断 / agent 在笔记产出后桌面侧解析。三者交互复杂度递增。是否接受「title 默认 = 建探索时的问题截断，可改」作为最小方案？
2. **promote 时 goal 的关系**：change 的 explore.md 是「探索笔记全文（goal 可留空）」还是「goal 头部 + 探索笔记全文」？CLI 语义是前者（move 覆盖），但 desktop 的 goal 是既有必填字段。
3. **promote 的 change name 来源**：change 名由用户在建变更时另填 kebab-case（沿既有 change-create 语义），还是尝试从探索 title/name 派生？后者又回到「agent/前端命名」的老问题。
4. **promote 后探索详情页形态**：笔记已移走，详情页预览显示「已转变更 X」+ 可跳转，而不是空态。

## 用户决策（2026-10-08 二轮）

1. **title 生成权 = 进阶**：agent 在笔记里写标题，桌面侧 watch 解析回填。
2. **promote 无需输入 goal，全文替代**：探索笔记全文直接成为 change 的 explore.md。
3. **change 也分离 title 与 kebab-case name**，分别从探索 title 与文件名继承。
4. **认可** promote 后探索详情页显示「已转变更 X」+ 跳转。

## 决策连锁与关键事实（二轮调研）

### 连锁 A · 决策 3 打开 change 域 title 演进（四层贯穿，最重）

change 的 `title` 不能只加在 UI，要贯穿四层（explore 的 `ExploreRecord` 直接出线到前端 DTO，无 workflow 中性层，改动面小；change 则相反）：

```
workflow/state.rs  ChangeStateRecord  ← 中性快照加 title
infra/store/model.rs  ChangeRecord    ← native_model version 2 → 3
infra/store/store.rs  change_state()  ← 映射单点加 title
workflow/queries/list.rs  ChangeSummary ← 清单 DTO 加 title
```

- `ChangeSummary` 当前只有 name / source / status / active_phase / created，前端 `change-list-view` 直接渲染 name 作标题。
- 「文档形态」条目（磁盘目录、db 无记录）没有 title，需 fallback 到 name。
- `ChangeRecord` 已示范 version 链（v1→v2），v2→v3 同模式零迁移。

### 连锁 B · explore name 校验口径要升级为 kebab-case

决策 3「change name 从 explore 文件名继承」的隐含前提：explore 的 name 必须本身就是合法 change name。

- explore 当前校验 = `is_single_component_name`（宽松：非空、非 . ..、不含 / \ :，允许大写/下划线）。
- change 当前校验 = `is_kebab_case`（严格：`^[a-z][a-z0-9]*(-[a-z0-9]+)*$`，≤128）。

两者不对齐。要么 explore 建档口径升级为 kebab-case（推荐，与 change 对齐，且 change 名还要唯一），要么 promote 时做转换（可能冲突）。这是决策 3 的隐藏前提，需明确。

### 连锁 C · title 回填链路（决策 1 进阶）

现有 explore 笔记事实约定「第一行 `# 标题`」（openspec/explores/*.md 均如此），但 stance / SKILL 的「笔记落盘」约定**未明文要求写标题行**（SKILL 说 free-form / no template）。

落地需要：
- stance 模板补一条「笔记开头写 `# <标题>`」；
- 前端 `use-explore-doc` 已有 watch 信号（500ms 防抖 → read_explore），在重读 content 后解析首行 `# ...`，与 record.title 不同则调新命令回填（读是纯读、写是显式写命令，不破坏「纯读」纪律）。

### 连锁 D · promote 是 move，但记录不删

决策 2（move 全文）+ 决策 4 认可的前提：`delete_explore_record` 会级联删会话链，所以 promote 是「移走笔记 + 记录打标 promoted_to」，不是删记录。

## 完整蓝图（四决策收敛后）

```
                          ExploreRecord v2                          ChangeRecord v3
  ┌─────────────────────────────────┐        ┌─────────────────────────────────┐
  │ id          ← 身份锚(会话source_ref)│        │ name(kebab) ← 主键，继承explore.name│
  │ root                             │        │ title        ← 继承explore.title │
  │ name(kebab) ← stem 寻址键        │  promote│ workflow_type                    │
  │ title       ← 人类标题(agent产出) │ ──────▶│ created_at / status / worktree   │
  │ promoted_to ← Option<change名>   │        │ ...                              │
  │ created_at / updated_at          │        └─────────────────────────────────┘
  └─────────────────────────────────┘
```

promote 时序（综合四决策）：

```
1. 探索页已建档，笔记落盘 explores/<name>.md（首行 # 标题）
2. watch 解析回填 ExploreRecord.title（决策1）
3. 用户点「启动变更」
4. promote 命令：
   a. create_change(root, name=explore.name, goal=笔记全文, title=explore.title)
      ├─ worktree 内写 explore.md = 笔记全文（决策2：goal 语义=全文替代）
      └─ ChangeRecord.title = explore.title（决策3）
   b. 删主仓 explores/<name>.md（move 半边）
   c. ExploreRecord.promoted_to = change.name（记录打标，会话链保留）
5. 探索详情页显示「已转变更 X」+ 跳转（决策4）
```

## 剩余待确认（已大幅收敛）

1. **explore name 校验口径**：是否接受「explore 建档即要求 kebab-case」（连锁 B），为决策 3 铺路？存量非 kebab explore 如何处理（导流改名 / 容忍不 promote）？
2. **手动新建 change 的 title**：决策 3 只覆盖了「promote 路径」的 title 继承；change 引入 title 字段后，`change-create-dialog` 的手动新建路径 title 从哪来（同步加 title 输入 / 默认 = name / 暂不填仅 promote 填）？
3. **promote 全文含不含 `# 标题` 行**：change 的 explore.md 是保留笔记首行标题（`# 标题` + 正文）还是去掉标题行只留正文（标题已进 ChangeRecord.title）？