# 提案: archi-rule-agent-prompt-sync

> **变更**: archi-rule-agent-prompt-sync
> **日期**: 2026-11-19
> **状态**: 待评估（explore 四项决策全在本变更交付；本提案落定 explore §5 六项开放问题、两处对 explore 分类的偏离（R4）与一处矩阵保守裁量（R6），见决策表 D1–D20 与风险 R1–R12）
> **来源**: `explore.md`（requirement 类型；主题「架构模型规则与架构 Agent prompt 的同步面」，四项改造：知识分层 / 收口前引擎自检 / `archi_drift` 只读工具 / 按相位锁定模型写权）

---

## 问题

结构层同步（词表、目录路径、示例可解析性）已由 `archi_knowledge_test` 机械闭环；**语义层同步只覆盖一份关注点清单**，且判据读数对会话不可达。四个具体缺口：

1. **元素字段纪律不进 prompt，只进 spec / ADR**：`path` 的「叶元素必填、`./` 前缀、目录尾 `/`、容器恒空」、`entry` 的「约定默认三档」（单文件根 = 根文件自身 / 目录型根 = `index.ts(x)` / crate = `src/lib.rs`）、`kind` 四值语义（容器 `domain` / crate `module` / 组件文件 `component` / 包根 `package`）在知识骨架里**零承载**——骨架只给字段名清单，示例 TOML 的三个元素**全无 `path`**。抄示例写出的新元素随之落 `noPath` warn、不参与归属、不进检索语料。
2. **判据读数对会话不可达**：漂移判定 `drift::check` 只在两处 IPC 命令面与门禁 runner 被调用；`archi_drift` 是 IPC 命令，**不在 archi 工具面**。agent 因此只能「写完就交」，覆盖 / 归属 / 契约判据的读数要等门禁与评估相位。薄概览（`ArchiElementBrief`）不含 `path` 事实、单元素响应不含入口解析结果、也没有模型 toml 模板——这三件正是把 `path` / `entry` / 字段形态纪律从 prompt 移出的前置，缺了就只是「知识缺失」而非「知识分层」。
3. **写权无相位门**：模型文件今天对所有拿得到 `write` / `edit` 的会话可写——包括 `implement` / `code-review` / `acceptance` / `explore` / `debug`。真正的设计权只在 `proposal` / `dev-design` 的执行侧（以及架构页会话），其余相位写模型只会把「设计未定稿就落元素」的噪声带进模型。三处既有 spec 条文（`desktop-archi-agent:237`、`desktop-write-protection:302`、`desktop-phase-prompts:77`）恰好把这条路封成「不可能」：禁止内置锁面纳入架构域、禁止存在「该不该发写工具」的问题、禁止 prompt 指示写模型。**三条禁令的理由都是「锁是相位无关的绝对路径 glob」，没有考虑「按相位条件挂载」。**
4. **门禁盲区**：`archi_knowledge_test` 守的是派生词表在场 / 骨架零字面量零占位 / 示例可解析 / 关键词冻结；**没有任何测试把 spec 与 ADR 的字段纪律与骨架散文对拍**——散文语义是唯一没有第二事实源约束的面。

上下文账单是同一枚硬币的另一面：archi 会话每轮常驻 = 知识 3617 B + 仓库根 `AGENT.md` 逐字 6503 B ≈ 10 KB，其中骨架的「示例 TOML（348 B）+ 字段语法形态（≈500 B）+ `entry` 三条纪律（≈150 B）」全部是可脚本化、可由工具承载的部分。

## 提案

以「**知识按可判定性分层（散文只留不可判定项） + 判据读数进工具面 + 收口前引擎兜底自检 + 按相位锁定模型写权**」四件同批收口，并给分层立机械守门（A 类句子必须携带工具指路，且工具确有该字段）。

### 1. 知识分层（A/B/C/D）与骨架瘦身

判据 = **可判定性 × 时机**两轴乘积（不是「能不能脚本化」单轴）：

```
                        动作前就需要          动作后才需要
     可判定       B 上下文·一行指针         A 工具承载（读数）
   （机器能算）   模型目录 / 计数单源 /      结构诊断 / 漂移判据 /
                  看护配置 / 写面落点         入口解析 / 模板
     不可判定     C 上下文·散文纪律         D 模板工具
   （语义/意图）  写面禁令 / kind 语义 /     新元素 toml 骨架
                  静默陷阱 / 续接
```

三条补充规则：静默错误必须留上下文（`metadata` 子表、`[element.App.Auth]` 点分嵌套会被解析成**另一个元素**，工具在动作前拦不住）；写入定位必须留一行（新元素落点无处可查）；`kind` 语义不可脚本化（架构判断）。

骨架处置（3071 B → 目标 ≤ 2.2 KB，实测值冻结）：

| 处置 | 内容 | 落点 |
|---|---|---|
| 删 | 示例 TOML（348 B） | `archi_scaffold` 输出模板，模板本身可解析（机械锚） |
| 压成指路 | 字段语法形态（≈500 B） | `archi_scaffold`（字段序 + 内联 `metadata`）+ `archi_module` 字段清单 |
| 压成指路 | `entry` 三条纪律（≈150 B） | `archi_module` 新字段 `resolvedEntry`（解析结果 + 形态 + 不可用理由） |
| 压成指路 | `doc` 仅 `api = none` 可写、祖先先声明 | 既有判据面（`archi_drift` / `archi_module` 的 `is_error` 与 `apiUnexpected`） |
| 留（C，新增） | `kind` 四值语义 | 骨架散文（四行） |
| 留（B，新增） | 计数复述单源（模型表头注释以工具读数为准，改模型同批更新） | 骨架一行 |
| 留（B，新增） | 架构域看护配置 `config.json`（`exclude` / `ignore`）存在且非法即工具报错 | 骨架一行 |

派生段收缩：四面词表（`kind` / `gate` / `api` / 结构诊断）保留，目录方位由三处收为两处（模型目录 + 契约资产；**契约索引路径删除**——生成物，零手写面）。

**对 explore 分类的两处偏离**（知情，理由在 R4）：

- **结构诊断九词保留在知识里**：`desktop-archi-model`「消费面词汇集一致性对拍」把「架构会话知识文本」列为四面之一，删它就要改第六份 spec 并削弱一条机械闭环；保留成本 ~90 B。
- **契约资产目录保留在知识里**：explore 判它 A 类（`api.contractPath` 已给），但该结论只对**已存在**契约成立——新元素的 `api.status = none`、三路径全 `null`，删掉目录方位等于让「新建契约 md」无处落笔。按 explore 自己的补充规则 2（写入定位必须留一行）归 B 类。

### 2. 判据读数进工具面

| 前件 | 契约 |
|---|---|
| 薄概览补 `hasPath` | `ArchiElementBrief.hasPath: bool`——一次无参调用即可判「谁缺 `path`」 |
| 单元素补 `resolvedEntry` | `{ path, via: explicit / convention / null, problem }`；判据经 `api::paths::entry_state` **单点**（MUST NOT 第二份入口解析） |
| 新工具 `archi_scaffold` | 只读零写盘；`kind` 可选（值集由 `ElementKind::ALL` 派生）→ `{ draft }`：字段序 + 内联 `metadata` + `path` 必填提示 + 关系模板；草稿本身过 `TomlParser` 零诊断 |
| 新工具 `archi_drift` | 恒 `full`（无 `mode` 参数）；`scope` 三值 `summary`（缺省）/ `entries` / `files`；值集外 `Err` 点名合法值；过 `cap_result`；`CheckError::Config` 文案经 `is_error` 回灌（「架构域配置非法」这条读数由此零额外实现地补齐） |

`archi_drift` 的两个硬约束：**只能 full 且正好对**（`infra/agent` 不依赖 `vcs`，而 archi 臂 root = 主仓根、无 change 上下文 ⇒ `HEAD` 基准恒空 diff，incremental 本就无意义）；**报告不能直出**（本仓基线 302 条 `unmatchedFiles` ≈ 15 KB 由 `gate` 全 `off` 造成，是常态不是灾难——工具描述必须点名，摘要里 302 只出现在计数位）。

### 3. 收口前引擎自检

模型给出**收口终答**（`loop` 自然收敛支）时，引擎跑一次 `drift::check`，结果以 `SystemNotice{subtype:"archi_preclose"}` 落转录并写引擎日志，**不回灌模型、不催轮次、不发新请求**。挂点唯一（空 choice / 截断 / 异常终局 / `stop_requested` / `pause_turn` 到顶五个出口里只有自然收敛跑），fail-open（任何 `CheckError` ⇒ `archi_preclose_skipped` + 照常收口），notice **先于** `finish_success`（`TurnDone`）。

触发 = T1（本轮写过架构域）∪ T2（资产哈希变了）；都为假只在引擎日志留一行 skip，不出 notice（纯问答不产生转录噪声）。会话判定用 `turn.injections.preamble.is_some()`（该位唯一生产者就是 archi 臂），**不用** `turn.archi.is_some()`（change 相位也下发 archi 工具，会误命中）。

### 4. 按相位锁定模型写权

**政策**：模型可写 ⟺ 架构页会话（`source = "archi"`）∨ (`source = "change"` ∧ `phase ∈ {proposal, dev-design}` ∧ `role = executor`)。其余一切锁定，含 `dev-design/evaluator`（白名单按 `(phase, role)` 而非 phase）与 `explore` / `debug`。

不是「造一个写模型工具」（`archi` crate 全程只读盘，模型唯一落盘点是壳层单点 `write_atomic`；在 infra 给出真写盘工具就得打穿单点纪律），而是**掐掉其余相位对模型目录的射程**：载体为 `SessionInjections` 第五位 `protect: Option<Vec<SessionProtectRule>>`（serde-only、无 `specta::Type` ⇒ 不进 TS bindings），装配单点 `compose::model_write_lock(provenance)`，规则序**内置 → 会话 → 用户**（首个命中即定）。锁面只 `graph/model/**`：契约 md（`models/**`）不锁（`implement/executor` 持有 `archi_api`，改代码相位必须能同步契约，锁了契约判据会永远飘）、`INDEX.toml`（生成物，「MUST NOT 手改」已足够）、看护 `config.json`（不是模型）不锁。

三条既有禁令按「理由被改写、字面保留」处置：内置面仍恰一条（禁的是**相位无关**的锁，相位条件的锁是形态）；`desktop-archi-agent`「不存在该不该发写工具的问题」半句删除（问题存在，答案是「不发专用工具、发权限」）；`desktop-phase-prompts` 的「MUST NOT 指示模型写模型文件」按相位分流（白名单相位 SHALL 指示写面与自查，其余 SHALL 写「模型已锁」）。

### 5. 能力面连带（否则白名单里有权限但不会用）

| 相位 / 角色 | 现状 archi 面 | 变更 |
|---|---|---|
| `proposal` / executor | `search` | + `module`（要写必须先能定位：`modelFile` + `modelLine` 才能直接 `edit`）+ `scaffold` |
| `dev-design` / executor | `module` + `search` | + `drift`（写完自查）+ `scaffold` |
| `implement` / executor | `module` + `api` | + `drift`（代码变了模型没跟，正是它的读数） |
| archi 来源臂 | `module` + `search` + `api` | + `drift` + `scaffold` |
| `test-design` / `code-review` / `acceptance` / `dev-design` evaluator | 不变 | 不变 |

决策表：

| 编号 | 决策 | 值 |
|---|---|---|
| D1 | 承载面 | 四项决策同一 change；实现序 = 收口自检 → `archi_drift` → 相位锁 → 知识分层（explore §6；前三项互为前置，分层依赖工具面先就位） |
| D2 | 分层判据 | 两轴（可判定性 × 时机）四档 A/B/C/D；三条补充规则（静默错误 / 写入定位 / kind 语义）写入 spec |
| D3 | 骨架处置 | 删示例 TOML；字段形态 / `entry` 三条 / `doc` 判据压成工具指路；新增 `kind` 语义、计数单源、看护配置三块散文 |
| D4 | 骨架体量 | 骨架 ≤ 2.2 KB、全文 ≤ 2.8 KB，实测值冻结（现 3071 / 3617） |
| D5 | 派生段 | 四面词表保留；目录方位三处 → 两处（删契约索引） |
| D6 | `hasPath` | `ArchiElementBrief.hasPath: bool`（必填） |
| D7 | `resolvedEntry` | `{ path: Option<String>, via: Option<"explicit"｜"convention">, problem: Option<String> }`；`entry_state` 单点求值 |
| D8 | `archi_scaffold` | 只读零写盘；`kind` 可选（`ElementKind::ALL` 派生，值集外 `Err`）；`{ draft }`；草稿可解析 |
| D9 | `archi_drift` | 恒 `full`；`scope ∈ {summary, entries, files}`；无 `mode` / 无 `root` 参数；`cap_result` 收口 |
| D10 | 投影单点 | `drift_summary(&Report, duration_ms) -> Value` 为工具 `summary` 与收口 notice 共用；notice 追加 `changedAssets` |
| D11 | 资产哈希 | `archi::drift::assets_hash(root)`：`graph/model/*.toml` + `models/**/*.md` + `*.schema.json` + `graph/INDEX.toml`，文件名 + 字节累加；报告 `modelHash` 语义不变 |
| D12 | 自检触发 | T1（本轮 write / edit / bash 写入命中 `<archi_dir>/graph/**` 或 `models/**`，路径经 `foundation::layout` 派生）∪ T2（`assets_hash` 变化）；皆假只留日志 |
| D13 | 自检执行 | `spawn_blocking` + `CancelFlag::never()`（不为几十毫秒扩 `RunHandle` 公开面）；`tokio` 零新依赖（`rt` 已在） |
| D14 | 自检失败面 | fail-open 三面：`CheckError` → `archi_preclose_skipped`（带成因）+ 照常收口；MUST NOT 阻断 / MUST NOT 改 `TurnDone` / MUST NOT 回灌模型 |
| D15 | 写权政策 | 见上文政策句；两条推论（`dev-design/evaluator` 被锁、`explore` / `debug` 被锁）写进 spec |
| D16 | 锁的载体与规则序 | `SessionInjections.protect`（第五位，serde additive）+ `ProtectRules::with_session`（`load` 签名不动，~10 处测试调用零改动）；序 = 内置 → 会话 → 用户 |
| D17 | 锁面范围 | 只 `**/<archi_graph_model_rel_dir>/**`；`models/**` / `INDEX.toml` / 看护 `config.json` 不锁；`ReasonKind::Session` + 专用模板（携 `%t`，含「不要改用其他命令绕过」） |
| D18 | 矩阵终值 | 8 行（含兜底行）；`drift` 只给 `dev-design/executor` 与 `implement/executor`（**保守案**，见 R6）；`scaffold` 只给两处写权相位 + archi 臂 |
| D19 | prompt 分流 | `proposal-planner` / `dev-design-planner` SHALL 指示写面与自查；锁定侧其余 12 份 SHALL 各补**中文**「架构模型已锁」一句指路（该句零 ASCII `archi` 字样——未命中矩阵的档案另有整篇零 archi 工具名字样断言在案）；evaluator md 仍零 `backtrack` 字样 |
| D20 | 版本纪律 | `packages/desktop/package.json` patch +1（`0.7.2` → `0.7.3`）；`src-tauri/Cargo.toml` 不跟 |

**明确不做**（知情边界，非遗漏）：

- **`AGENT.md` 按来源裁剪**（explore 开放问题 4）：收益（archi 会话里 ≈2.4 KB 无关内容）高于骨架瘦身，但撞「`AGENT.md` 逐字注入、不包装不改写」与 `desktop-archi-api` 的「节结构冻结、条目数恒 5」两条既有承诺，属独立裁决。`AGENT.md` / `CLAUDE.md` 本变更**零改动**。
- **`AGENT.md` 与知识文本的 `doc` / `api` 双写去重**（explore §2.5）：去重必须同批改两份仓库根文档，收益小、风险是节内措辞漂移；改为在 `desktop-archi-agent` 里立「双真相源登记」纪律（R7）。
- **前端消费 `archi_preclose` 自动刷新画布**：知识里「写盘后画布不会自动更新、请点刷新」一行**保留**（不删）。画布刷新面属 `desktop-archi-canvas` / `desktop-archi-chat` 的独立议题。
- **`archi_drift` 给 `code-review` / `acceptance`**：保守案未勾（R6）。
- **`archi_drift` 增量模式 / change 相位下发**：需要把 `Arc<dyn ArchiveVcsPort>` 注入 `infra/agent`，本期不做。
- **给 `BulkGlobs`（`git stash` / `git clean` 等批量动作）加相位条件**：模型目录不在 `changes` 子树、`git checkout <ref> -- <path>` 走 write 三态已被会话规则覆盖；`git stash` 仍不按相位判别（protect 本就不是安全边界，如实登记 R8）。
- **`git commit` 绕过 / `bash` 内容级绕过**：沿既有「protect 是防误触护栏不是安全边界」口径，不新增声明。
- **`archi_scaffold` 之外的模板**（契约 md 模板）：`archi_api bootstrap` 已承载。
- **模型 / 契约资产内容改动**：本变更零架构域模型改动（元素 54 / 关系 122、计数表与 ADR §9 不动；`modelHash` 不变）。

## 能力

### 新增的能力

无。四项决策分别落在既有能力的既有语义面上：知识分层与工具面契约归 `desktop-archi-agent`，漂移调用面归 `desktop-archi-drift`，注入位与收敛事件序归 `desktop-agent-execution`，会话级锁归 `desktop-write-protection`，相位 prompt 分流归 `desktop-phase-prompts`。单拆 `desktop-archi-prompt-layering` 之类的能力会与这五份的既有分工全面重叠。

### 修改的能力

- **desktop-archi-agent**（9 条）
  - **MODIFIED**「`archi_module` 只读工具的定位契约」：薄概览补 `hasPath`（判据 = 模型 `path` 非空，投影不补默认值）；单元素补 `resolvedEntry`（`path` / `via` / `problem`，经 `api::paths::entry_state` 单点）；描述面加「缺 `path` 可一次判出」「入口解析结果随响应给出」两句，删与工具面重复的语法教学。新增三 scenario（缺 path 可辨 / 入口三态可辨 / 描述不越权）。
  - **ADDED**「`archi_scaffold` 只读工具的新元素模板契约」：`kind` 可选值集由 `ElementKind::ALL` 派生、值集外 `Err` 点名四值；`{ draft }` 为字段序 + 内联 `metadata` + `path` 必填提示 + 关系模板；**零写盘**；草稿过 `TomlParser` 零诊断（自证锚）。
  - **ADDED**「`archi_drift` 只读工具的漂移读数契约」：恒 full、`scope` 三值、值集外 `Err`、`cap_result` 收口、`CheckError` 经 `is_error` 回灌、`unmatchedFiles` 在 `gate` 全 `off` 仓库是常态的说法必须进描述、零跨调用缓存（`durationMs` 让浪费可见）、投影单点 `drift_summary`。
  - **MODIFIED**「archi 工具面注入与 (phase, role) 矩阵」：矩阵由 7 行收为 8 行（`dev-design` 执行 / 评估拆行）+ 新增 `archi_drift` / `archi_scaffold` 两列；proposal 补 `module` + `scaffold`；implement 补 `drift`；archi 臂五 archi 名；fail-closed 判据逐字保留。
  - **MODIFIED**「只读档位归属与成员校验不变」：挂载清单 13 → 15 项，五个 archi 工具全 `ToolTier::Read`；成员校验 / 沙箱 / 保护闸三条既有语义逐字保留。
  - **MODIFIED**「模型 = 源码（写纪律与不设专用写工具）」：写权收窄为「通路 A 限 `proposal` / `dev-design` 执行侧；通路 B 不限」；删「因而不存在该不该发写工具的问题」半句；补「写权由会话级锁（第三来源）承载，不是内置锁面扩充」；两条推论（`dev-design/evaluator` 与 `explore` / `debug` 被锁）与「缺口如实留痕」保留。
  - **MODIFIED**「架构会话的系统知识注入」：内容面改写为分层后的 B/C 清单（新增 `kind` 语义 / 计数单源 / 看护配置三块，删示例 TOML 与字段语法、`entry` 三条、`doc` 判据细节 → 全改指路）；派生段由三处目录收为两处；体量锚改「骨架 ≤ 2.2 KB / 全文 ≤ 2.8 KB，实测值冻结」。
  - **ADDED**「知识分层与工具面同批移动」：A 类句子 MUST 携带工具指路且工具面确有该字段（机械断言表）；B 类一行；C 类散文；D 类模板工具；移动分层的判据改写（原句删除与新句落地同批）。
  - **ADDED**「收口前引擎自检（架构判据读数）」：唯一挂点 = 自然收敛支；触发 = T1 ∪ T2（T1 的路径判据经 `foundation::layout` 派生）；`preamble.is_some()` 判会话；`assets_hash` 前后比对；`spawn_blocking` + `CancelFlag::never()`；fail-open 三面；notice **先于** `TurnDone`；整份报告不入转录。
- **desktop-archi-drift**（2 条）
  - **MODIFIED**「判定单源与消费面同源」：调用面三处 → 五处（+ 引擎工具面 + 引擎收口自检）；后两者恒 `Full` + `changed: []` + `change: None`，与命令面全量分支同源；「零第二套判定」判据逐字保留。
  - **ADDED**「资产哈希（`assets_hash`）与报告 `modelHash` 的分工」：`assets_hash` 覆盖面（model toml + 契约 md + `.schema.json` + `INDEX.toml`）、累加口径（文件名 + 字节，稳定序）、确定性；`modelHash` 仍只覆盖 model toml、不得被当作缓存失效依据。
- **desktop-agent-execution**（4 条）
  - **MODIFIED**「SDK 引擎（rig 进程内租户）」：注入位词面 scenario 由「四位」改「五位」（含 `protect`）；系统提示词 bullet 的位清单随动。
  - **MODIFIED**「会话级工具面注入与 explore 笔记工具」：`SessionInjections` 形态补第五位 `protect`（serde additive、无 `specta::Type`）；compose 三臂逐臂置位（`model_write_lock` 单点）；`tools` 名字清单纳入 `archi_drift` / `archi_scaffold`；上下文铸造判据的两名扩充；tier 表补两行（均 `Read`）。
  - **MODIFIED**「loop 守卫链写入保护闸与工作区根注入缝」：规则来源由「内置 + 用户」增为「内置（`config.json`）→ 会话（相位锁）→ 用户（自选）」——**守卫链四段顺序零改动**，会话规则随 `LoopTurn.protect` 同路下发（`with_session` 在泵起播时完成）。
  - **ADDED**「自然收敛前的架构判据自检与收敛事件序」：notice 先于 `TurnDone`；五个出口只自然收敛支执行；不回灌模型 / 不催轮次 / 不发新请求；fail-open 后照常收敛；非 archi 会话零 notice。
- **desktop-write-protection**（4 条）
  - **MODIFIED**「保护规则装配（内置锁定 + 用户规则合并）」：规则来源三处（内置 → 会话 → 用户）；`ProtectRules::with_session`（`load` 签名与「每 run 一次装配」IO 点不变）；会话规则 glob 由装配层用 `foundation::layout` 派生（protect 不解释路径语义）；`reason` 模板化（`%s` / `%t`）。
  - **MODIFIED**「拒绝指令文案」：新增第四形态「会话级相位锁串」（携路径 + 工具名 + 指路：写进 design / tasks 由 `dev-design` 落地，或改用架构页会话 + 「不要改用其他命令或工具尝试绕过」）。
  - **MODIFIED**「架构域路径不在内置锁面」：字面禁令逐字保留（内置面仍恰一条）；理由改写为「禁的是**相位无关**的锁（绝对路径 glob 在 worktree 与主仓同样命中，纳入即等于谁都改不了模型）；相位条件的**会话级**锁是形态，不受本条约束」；补一条「模型目录的相位条件锁挂在会话规则面」。
  - **ADDED**「会话级写入禁区（相位锁）」：政策句 + 锁面范围（只 `graph/model/**`）+ fail-closed 方向（`source_ref` 畸形 / 未知 phase / 非 change 来源 → 挂锁）+ 与架构页通路零冲突 + 批量动作缺口如实留痕。
- **desktop-phase-prompts**（1 条）
  - **MODIFIED**「桌面端特性适配（迁移是改写非搬运）」：archi 工具引用纪律按新矩阵改写（`proposal` → `search` + `module` + `scaffold`；`dev-design` 执行 → `module` / `search` / `drift` / `scaffold`，评估 → `module` / `search`；`implement` → `module` / `api` / `drift`；`code-review` / `acceptance` → 仅 `module`；未命中矩阵的档案仍零 archi 工具名）；新增「模型写面按相位分流」：两处写权相位 SHALL 指示写面与自查（含「只为已确定元素落笔」，偿还 explore 开放问题 2），锁定侧其余 12 份 SHALL 各写「架构模型已锁」一句且 MUST NOT 指示路由工具调用（evaluator md 仍零 `backtrack` 字样）。

其余能力零改动，理由：

- **desktop-archi-model**：字段语义（`path` / `entry` / `kind` / `doc`）、诊断九类与四面词汇集对拍全部零改动——变的只是「谁有权改模型」与「知识文本怎么讲」，不动判据；四面词表刻意保留（R4）。
- **repo-archi-model**：模型内容零改动（54 元素 / 122 关系、表头计数、ADR §9 零触及）；`entry` 三处显式基线不动。
- **desktop-archi-api / desktop-archi-api-check**：`entry_state` 与十三值判据零改动（只被 `resolvedEntry` 复用一次）；`api:check` 预期零 diff。
- **desktop-archi-chat / desktop-archi-canvas**：前端画布刷新面不做（见「明确不做」）；archi 会话写权不变。
- **desktop-archi-gate**：门禁 runner 与落盘零改动；本变更只加「引擎侧调用面」，不改门禁结论。
- **desktop-crate-layout**：零新 crate、零新跨 crate 依赖边（`infra/agent` 已依赖 `archi` / `foundation`；`core/agent` 只加 serde 结构）。
- **desktop-workspace-config**：`KNOWN_KEYS` / schema 镜像零改动（看护配置是另一文件，未变）。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | `ArchiElementBrief` 增 `has_path: bool`（线格式 `hasPath`）；新增 `ArchiResolvedEntry { path: Option<String>, via: Option<EntryVia>, problem: Option<String> }`，`ArchiModuleDetail` 增 `resolved_entry`；`module_overview` 填 `hasPath = !element.path.is_empty()`、`module_detail` 填 `resolvedEntry`（`api::paths::entry_state` 投影，`via` 由 `element.entry.is_some()` 派生；`Invalid` / `Absent` 落 `problem` 文案） |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` | 新增 `pub fn assets_hash(root: &Path) -> String`（覆盖面 = `graph/model/*.toml` + `models/**/*.md` + `*.schema.json` + `graph/INDEX.toml`，稳定序「相对路径 + 字节」累加，`sha256:` 前缀；遍历复用既有 `ignore` 依赖，零新依赖）；`model_hash` 与报告字段语义零改动（两者共享同一累加 helper） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | `ARCHI_DRIFT_TOOL` / `ARCHI_SCAFFOLD_TOOL` 常量；`archi_drift_definition()`（描述点名「`gate` 全 `off` 仓库里 `unmatchedFiles` 是常态」与三 scope）/ `archi_scaffold_definition()`（`kind` enum 由 `ElementKind::ALL` 派生）；`build_registry` 13 → 15（注释与 `ToolTier` 归组同步） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` | `execute` 加两支：`drift_payload`（scope 解析 → `Instant` 计时 → `drift::check(CheckInput{mode: Full, changed: &[], change: None, cancel: CancelFlag::never()})` → 投影 → `cap_result`）、`scaffold_payload`（`kind` 解析 → 模板文本）；`Err` 文案点名合法值 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_drift_view.rs`（新） | 投影单点：`drift_summary(&Report, duration_ms: u64) -> Value`、`drift_entries(...)`、`drift_files(...)`；工具响应与收口 notice 共用 `drift_summary`（零第二份装配） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preclose.rs`（新） | `touched(name, input, cwd) -> bool`（`write` / `edit` 取沙箱后 `path`；`bash` 经 `protect::written_paths` 的写面提取；命中前缀由 `foundation::layout` 的 `archi_graph_rel_dir` / `archi_models_rel_dir` 派生）；`run(&sender, turn, touched: bool)`（`assets_hash` 前后比对 → `spawn_blocking` 调 `drift::check` → `archi_preclose` / `archi_preclose_skipped` notice + 引擎日志） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | 自然收敛支：`preclose::run(...)` 先于 `finish_success`；循环内按已通过的写面调用维护 `archi_touched`（沙箱改写后的入参在手时判定，零额外 IO） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | `mint_archi_context` 的 any 判据加两新工具名；`ProtectRules::load(...).with_session(open.injections.protect)` |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules.rs` | `ReasonKind::Session`；`load` 保持签名（内置 → 用户），新增 `with_session_rules(rules, session)` 合并位（会话插在内置之后、用户之前） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs` | `ProtectRules::with_session(self, Option<Vec<SessionProtectRule>>) -> ProtectRules`（`load` 签名不动）；`match_path` 的规则序由装配位次表达（零第二份优先级逻辑）；另新增窄访问器 `written_paths(command, cwd) -> Vec<String>`（内部调 `shell::extract_file_ops` 的 write 态 + 既有 `resolve`，供 `preclose` 复用同一写面提取面，MUST NOT 第二份写面解析） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason.rs` | `SESSION_LOCK_TEMPLATE` 常量 + `session_lock_template()` 出口（`compose` 取它构造规则）；`deny_reason` 增 `ReasonKind::Session` 分支（`%s` / `%t` 替换） |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | archi 臂名字清单加 `archi_drift` / `archi_scaffold`；`archi_matrix` 改 8 行矩阵；新增 `model_write_lock(provenance) -> Option<Vec<agent::SessionProtectRule>>` 单点（archi 臂与两处写权相位 → `None`，其余 → `Some(一条)`，glob 经 `foundation::layout::archi_graph_model_rel_dir` 派生）；三臂装配产物带上 `protect` 位 |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `SessionInjections` 加第五位 `protect: Option<Vec<SessionProtectRule>>`（`#[serde(default)]`，无 `specta::Type`）；新增 `SessionProtectRule { glob: String, reason: String }`；doc 词面「四位」→「五位」 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | 按分层重写：删示例 TOML / 字段语法表 / `entry` 三条 / `doc` 判据细节；新增 `kind` 四值语义、计数单源、看护配置三条散文；`path` 纪律改为「叶必填 + 形态 + 容器恒空」一段 + 「模板用 `archi_scaffold`」指路；自检纪律压缩为工具指路（`archi_drift` / `archi_module` / `archi_api`） |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` | 派生段：删契约索引一处（保留模型目录 / 契约资产）；四面词表与降级语义零改动；体量锚随实测更新 |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md` | 只读工具行补 `archi_module` / `archi_scaffold`；新增写面句（可写模型目录、只为已确定元素落笔、写完自查） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md` | 补 `archi_drift` / `archi_scaffold` 与写面自检句（与原 `archi_module` / `archi_search` 用法行并列） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md` | 补 `archi_drift`（改代码后自查未声明依赖）；另补一句「架构模型已锁」（`implement` 在锁定侧） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/*.md`（锁定侧共 12 份 = 上行的 `implementation-generator` + `test-design-planner` / `test-gen-generator` / `code-analyze-planner` + 8 份 evaluator（`dev-design-evaluator` / `proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-evaluator` / `code-review-evaluator` / `acceptance-evaluator` / `code-analyze-evaluator`）） | 各补一句「架构模型已锁：MUST NOT 尝试写模型；需要改模型请写进产物交由 `dev-design` 相位落地」（该句为中文措辞且零 ASCII `archi` 字样；evaluator 文件零 `backtrack` 字样） |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `pnpm run bindings:export` 再生成（非手改）：`ArchiElementBrief` 携 `hasPath`、`ArchiModuleDetail` 携 `resolvedEntry` |
| `packages/desktop/package.json` | `version` patch +1（`0.7.2` → `0.7.3`）；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` 不跟 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| 薄概览带 `hasPath`（有 / 无 `path` 两态）、单元素 `resolvedEntry` 三态（显式 / 约定默认 / 不可用） | `crates/core/archi/src/dto_test.rs` | 新增 |
| `assets_hash` 四类资产各改一处即变；无改动两次相等；空目录稳定值；稳定序 | `crates/core/archi/src/drift/*_test.rs` | 新增 |
| 挂载清单 13 → 15、两名 `lookup` 命中、两新工具 `ToolTier::Read`；`archi_drift` 描述含 gate 全 off 常态句；`archi_scaffold` 的 `kind` enum 与 `ElementKind::ALL` 逐值相等 | `crates/infra/agent/src/sdk/mount_test.rs` | 扩展 |
| `archi_scaffold` 草稿过 `TomlParser` 零诊断、含字段序与内联 `metadata`、`kind` 值集外 `Err` 点名四值、零写盘（调用前后盘面字节相同） | `crates/infra/agent/src/sdk/archi_tool_test.rs` | 新增 |
| `archi_drift` 三 scope 形状（`summary` 无 `observations` / `findings` 明细，`entries` / `files` 逐级追加）、值集外 `Err`、`unmatched` 只在计数位、坏模型 `is_error`、`cap_result` 截断后计数仍在 | 同上 | 新增 |
| 收口自检触发矩阵（T1 命中 / T2 变化 / 皆假只留日志）、notice 先于 `TurnDone`、`CheckError` → `archi_preclose_skipped` 且照常收敛、请求数不变（零额外请求）、非 archi 会话零 notice、`preamble = None` 时漏判为 skip（fail-open） | `crates/infra/agent/src/sdk/preclose_test.rs`（新） | 新增 |
| 五个出口（自然收敛 / 空 choice / 截断 / 停止 / `pause_turn` 到顶）各跑一遍，仅自然收敛支出 notice | `crates/infra/agent/src/sdk/loop_test.rs` | 新增 |
| 矩阵逐格（8 行 × 5 工具维度）、archi 臂 12 个工具名（七内建 + 五 archi 名；挂载清单 15 项另含 `create_explore_note` 与两件只读 DB 工具，不在该臂）、`model_write_lock` 逐格（archi 臂 / proposal / dev-design 执行 → `None`；dev-design 评估 / implement / test-design / code-review / acceptance / 其余 / 畸形 `source_ref` → `Some`） | `crates/infra/agent/src/compose_test.rs` | 扩展 |
| 会话规则命中 / 未命中 / 文案指路 / 规则序（内置 → 会话 → 用户，首个命中即定）/ bash 写入面同样命中 / 只读工具零介入 / `load` 签名不变（既有 ~10 处调用零改动） | `crates/infra/agent/src/sdk/protect/*_test.rs` | 新增 |
| `SessionInjections` 键面四键 → 五键、round-trip、缺字段读 `None`（serde additive） | `crates/core/agent/src/runner_test.rs` | 改断言 |
| `with_session` 从 `open.injections.protect` 传导至 `LoopTurn.protect`；保护闸拒绝串与 notice 同串（双面） | `crates/infra/agent/src/sdk/runner_test.rs` | 新增 |
| 知识骨架：删示例围栏 / 字段语法表 / `entry` 三条、新增 `kind` 语义与两条一行指路、A 类指路表（每句含工具名且工具面确有该字段）、派生段两处目录、四面词表在场、体量锚（骨架 ≤ 2.2 KB / 全文 ≤ 2.8 KB）、降级为 `None` | `crates/infra/agent/src/archi_knowledge_test.rs` | 重写锚点 |
| 相位 prompt 矩阵（7 份命中文件 × 5 工具维度）、`proposal` / `dev-design-planner` 含写面与自查句、锁定侧其余 12 份含「架构模型已锁」一句（该句零 ASCII `archi` 字样；未命中矩阵的档案整篇零 archi 工具名）、evaluator md 零 `backtrack` | `crates/core/workflow/src/write/phase_table_test.rs` | 扩展（`ARCHI_PROMPT_MATRIX` 加两维、`命中md零路径模板` 的「写入模型」断言改为按相位期望） |
| DTO 新字段随动（`ArchiElementBrief` 夹具补 `hasPath`、`ArchiModuleDetail` 夹具补 `resolvedEntry`） | `packages/desktop/src/lib/ipc-contract.test.ts`、`packages/desktop/src/views/architecture/**/*.test.*`（按 `tsc` 报错逐处） | 随动 |

### 不要修改

- **`explore.md`**（本 change 的）：只读。
- **`.dev-team/architecture/**`**（模型 / 契约资产 / 索引 / 看护配置 / ADR）：模型内容零改动，`modelHash` 不变；`api:check` 预期零 diff。
- **`AGENT.md` / `CLAUDE.md`**：逐字注入纪律与节结构冻结，零改动（D18 / R7）。
- **`crates/core/archi/src/api/**`**：`entry_state` 与十三条判据零改动（只被 `resolvedEntry` 复用）。
- **`crates/infra/archi/src/runner.rs`（门禁 runner）/ `src-tauri/src/commands/archi/**`**：落盘与门禁命令面零改动（`archi_drift` IPC 命令逐字不变，与新增同名工具面互不影响）。
- **`crates/core/archi/src/model.rs` / `parse/**` / `scan/**`**：字段面、诊断九类、扫描与归属判据零改动。
- **`crates/core/workflow/src/write/phase_table.rs`**：14 条 `include_str!` 逐字不变（prompt 文本随动，装配面不动）。
- **前端 `src/views/architecture/**` 的运行时代码**：只补测试夹具；不做画布自动刷新、不改 `archi_drift` 调用方式。
- **`openspec/todo.md`**：待办勾选由流程面管理。
- **`packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`**：零新依赖（`tokio` 的 `rt` feature 已在，`archi` / `foundation` / `checks` 已在）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | `archi_module` 无参概览逐条含 `hasPath`（有 `path` 为真、无 `path` 为假，字段不省略）；单元素响应含 `resolvedEntry`，三态可辨（显式命中 / 约定默认命中 / `Absent`→`problem`），且判据只由 `api::paths::entry_state` 产出（源码复核无第二份入口解析） | `dto_test` 新增矩阵 + 源码复核 |
| AC-2 | `archi_scaffold` 返回草稿：过 `TomlParser` 零结构诊断、含字段序与内联 `metadata`、`path` 必填提示在场；`kind` 值集外 `Err` 并点名四值；调用前后盘面字节完全相同 | `archi_tool_test` + `mount_test`（enum 对齐） |
| AC-3 | `archi_drift` 恒全量、`scope` 三值逐级追加（`summary` ⊂ `entries` ⊂ `files`）、值集外 `Err` 点名合法值、`cap_result` 截断后计数仍在、坏模型 `is_error` 不 panic、零跨调用缓存（连调两次各自计时）；描述含「`gate` 全 `off` 时 `unmatchedFiles` 是常态」 | `archi_tool_test` + `mount_test` 描述断言 |
| AC-4 | 收口前自检只在自然收敛支执行一次：`archi_preclose` notice **先于** `TurnDone`、请求数不变量（零额外请求）、不回灌模型（历史无新增消息）；T1 / T2 皆假时零 notice 且引擎日志有 skip 行；`CheckError` ⇒ `archi_preclose_skipped`（带成因）+ `TurnDone` 照常成功 | `preclose_test` + `loop_test` |
| AC-5 | 写权矩阵逐格成立：`source = "archi"` 与会话级 `proposal` / `dev-design` 执行侧写 `<archi_dir>/graph/model/*.toml` 成功；`dev-design` 评估 / `implement` / `test-design` / `code-review` / `acceptance` / `explore` / `debug` 与畸形 `source_ref` 一律被拒（拒绝串含指路与「不要改用其他命令或工具尝试绕过」）；`models/**` / `INDEX.toml` / 看护 `config.json` 在所有相位可写 | `compose_test` + `protect/**/*_test` + `runner_test` |
| AC-6 | `SessionInjections` 恰五位（`preamble` / `tools` / `finish` / `ask` / `protect`）；旧线 JSON 缺 `protect` 读出 `None`；`ProtectRules::load` 签名与既有调用点零改动；会话规则位次在内置之后、用户之前（首个命中即定） | `runner_test` + `protect` 测试 + 源码复核 |
| AC-7 | 工具面矩阵逐格成立：proposal = `module` / `search` / `scaffold`；dev-design 执行 = `module` / `search` / `drift` / `scaffold`；dev-design 评估 = `module` / `search`；test-design = `search`；implement = `module` / `api` / `drift`；code-review / acceptance = `module`；archi 臂 = 七内建 + 五 archi 名；矩阵外零 archi 工具 | `compose_test` + `mount_test` |
| AC-8 | 知识文本按分层收口：零示例围栏、零字段语法教学、零 `entry` 三条；含 `kind` 四值语义 / 计数单源 / 看护配置三块；A 类句子逐句携带工具指路且该工具面确有对应字段（机械表）；派生段两处目录；骨架 ≤ 2.2 KB、全文 ≤ 2.8 KB（实测值冻结） | `archi_knowledge_test` 重写锚点 |
| AC-9 | 相位 prompt：两处写权相位含写面与自查（含「只为已确定元素落笔」），锁定侧其余 12 份各含「架构模型已锁」一句（该句零 ASCII `archi` 字样；未命中矩阵的档案整篇零 archi 工具名）；evaluator md 零 `backtrack` / 零落账指令；14 份零占位零 MCP；`include_str!` 装配面逐字不变 | `phase_table_test` |
| AC-10 | 绑定生成物零手改：`bindings.ts` / `bindings.zod.ts` 的 `hasPath` / `resolvedEntry` 只来自 `bindings:export`；前端夹具随动后 `tsc` 绿 | `pnpm run check` 链 + `git diff` 复核 |
| AC-11 | 全管线绿：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `run test` 全通过；`api:check` 零 diff（架构域资产零改动） | 静态检查链 + 全量测试 |
| AC-12 | 版本纪律：`package.json` = `0.7.3`，`src-tauri/tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本不随动 | `git diff` 复核 |

> **验收面口径**：验收止于**单元测试 + 静态检查链 + 一次真仓全量自检读数**（AC-3 / AC-4 的 `durationMs` 给出实测耗时，回答 explore 开放问题 5）；MUST NOT 以「模型按新 prompt 写出正确 `path`」这类模型行为质量作为验收条——那是 prompt 效果的**长期观测项**，不是可机械断言的产物（R1）。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 分层后 prompt 效果回归**：删掉示例 TOML 与字段语法后，模型可能不再写出 `metadata` 内联表 / 带引号 FQN 单键（静默错误：会被解析成另一个元素） | 新元素写坏且不报警（`deny_unknown_fields` 不触发） | 中 | 三条「静默错误必须留上下文」的 C 类纪律逐字保留在骨架；`archi_scaffold` 必出可解析模板（AC-2 机械锚）——模型有模板可抄；`archi_drift` / `archi_module` 的 `is_error` 与收口自检兜底；效果回归属长期观测（R11） |
| **R2 分层不到位白忙一场**：只删了散文但工具面没补齐（`hasPath` / `resolvedEntry` / `scaffold` 任一漏做），知识从「过期」退化为「缺失」 | agent 无法判断该不该写 `path` / `entry`，比现状更差 | 中 | 实现序固定为「工具面前置就位 → 再删散文」（D1）；AC-1 / AC-2 为分层的前置门；A 类指路表机械断言「句子里点到的工具确实有那个字段」 |
| **R3 收口自检的耗时与噪声**：全量扫描 ~358 源文件，耗时未知（explore 估 30–150 ms） | 收口延迟与转录噪声 | 中 | `durationMs` 进 notice 与 `summary`（首次运行即得实测）；纯问答（T1 / T2 皆假）零 notice 只留日志；`spawn_blocking` 不阻塞 runtime；fail-open 三面保证不阻断 |
| **R4 对 explore 分类的两处偏离**：结构诊断九词与契约资产目录未按 explore 标 A 移出 | 骨架瘦身幅度小于 explore 目标（≈0.9 KB vs ≈1.4 KB） | 高（已知） | 偏离理由写在本提案与 spec（诊断九词是 `desktop-archi-model` 四面词汇集的既有承诺；契约资产目录是新契约 md 的唯一写入定位）；体量锚按实测冻结，不假装达标 |
| **R5 模型可能一轮连调 `archi_drift`**：不缓存，浪费上下文与时间 | 单轮多份报告，token 与耗时上升 | 中 | 沿 `archi_search`「零跨调用缓存」纪律；`durationMs` 让浪费可见；描述点名「`files` 只在排查归属问题时用」；`cap_result` 兜流量 |
| **R6 矩阵保守案可能低估 evaluator 的收益**：`drift` 未给 `code-review` / `acceptance`，两相位判越界仍只能读模型面 | evaluator 仍可能多跑一轮 backtrack | 中 | 明确登记为**待裁决的保守案**：破「这两相位只读模型面」的既有条款需要独立裁决（时机差是理由，不是充分理由）；本期只给两处直接产出漂移的相位（写完 / 改完代码），收益与风险都最低 |
| **R7 `doc` / `api` 双真相源未去重**：`AGENT.md` 与知识文本各写一遍，措辞可能分叉 | 系统提示词自相矛盾，且无一致性测试 | 中 | 本变更**不**改 `AGENT.md`（D18）；在 `desktop-archi-agent` 立「双真相源登记」纪律并把它写进 R11 的长期项；本期知识文本对 `doc` / `api` 只保留**判据指路**（不复述 `AGENT.md` 已有措辞）以降低分叉面 |
| **R8 相位锁不是安全边界**：`git stash` / `git clean` 等批量动作不按相位判别；`bash` 内容级写入检测是启发式 | 非白名单相位仍可能经批量动作回退模型 | 中 | 与内置锁同一口径如实留痕（`desktop-write-protection` 的知情边界条款）；`git checkout <ref> -- <path>` / 重定向 / `tee` / node-inline 等**单路径**形态已被会话规则覆盖；模型目录不在 `changes` 子树，`BulkGlobs` 的还原拦截对它无增益——不为此扩 `BulkGlobs` 相位条件（explore 开放问题 3 的裁决：登记而非实现） |
| **R9 签名 / 结构面扩散**：`SessionInjections` 第五位、`ProtectRules::with_session`、`ArchiElementBrief` / `ArchiModuleDetail` 新字段、`archi_matrix` 8 行 | 漏改即编译错（安全），但夹具改动面大（前端 ~10 处测试 + Rust 多处断言） | 中 | 编译期与 `tsc` 强制全覆盖；`with_session` 保留 `load` 签名使既有 ~10 处调用零改动；前端夹具按 `tsc` 报错逐处补齐并登记在 AC-10 |
| **R10 相位 prompt 分流撞既有禁令**：`非命中md零archi字样` / `命中md零路径模板` / evaluator「零 backtrack」三条断言在案 | 写面指示句一旦措辞越界即测试红 | 中 | 措辞纪律写进 spec（中文「架构模型已锁」、零 ASCII `archi`、零 `backtrack`）；两处写权相位的写面句只提工具名与纪律，不提路径模板（路径知识归工具返回值）；测试断言按新矩阵与按相位期望同步改写（同一 change 内） |
| **R11 分层后的效果无法回归验证**：prompt 效果只能长期观测 | 分层可能「技术上完成、效果上无效」 | 中 | 本期只保证机械面（AC-8）；把「新元素 `noPath` 计数 / 新元素 `metadata` 子表写错的次数」列入后续观测项（由 `archi_drift` 读数与门禁报告提供数据面），MUST NOT 在本变更内伪造成验收条 |
| **R12 版本纪律漂移**：`package.json` 忘 bump 或 `src-tauri/Cargo.toml` 被顺手跟随 | 发布流水线版本错位 | 低 | AC-12 显式核对两处；`tauri.conf.json` 经 `../package.json` 自动跟随 |
