# 提案: module-doc-field-docs-retirement

> **变更**: module-doc-field-docs-retirement（E-2 · `doc` 单值字段替代 docs 三源 + 十三值判据 + 三段求值）
> **日期**: 2026-11-18
> **状态**: draft
> **来源**: 本 change 目录 `explore.md`（§0–§13 全篇 + 附录 A 二十条裁决沿革；§12 的 4 项均为实现期事实核对，不改口径）
> **前置已完成**: E-1 `2026-11-18-module-entry-docs-semantics` 已合入并归档——`contract_path` = `<models>/<Domain>/<FQN>.md`、索引落 `<archi_dir>/graph/INDEX.toml`、`api/` 目录退役、`archi_api_dir` 族已删。E-2 因此无前置阻塞。
> **性质**: **能力替换 + 判据面收紧**——`docs: Vec<String>` 与整个 `docs.rs` 三源关联能力退役，`doc: Option<String>` 单值取代，且**只对 `api = none` 的元素有意义**（`api = declared` 的入口文档恒为派生落点 `contract_path`，不写也不许写）；判据面由十值扩十三值、按 `api` 状态分两支各自首错短路并落到 `error` 档，`passed` 判据首度被契约判据接管。**行为变化集中在三处**：`apiFindings` 读数由 `0 / 41` 变 `0 / 43`（`docMissing` 接走 41 条改名 + 2 条新入面）、`passed` 可被 `error` 档判据打假、`archi_search` 的 `scope` 词 `docs` → `doc`。
> **不含**: 数据面回填（2b：新建两份工具 crate README + 41 份契约 md）；`$ref` ↔ `relation` 的同形对齐判据（explore 附录 A.2，属相邻议题）。

---

## 问题

### 现状链路

```text
模型元素（Element）
├── path: Vec<String>      覆盖路径
├── api: ApiStatus         declared（缺省）/ none —— 契约状态指针
├── entry: Option<String>  实现入口指针
└── docs: Vec<String>      显式文档路径数组
        │
        └── docs.rs 三源关联：explicit（docs 数组）/ adjacent（path 目录扫描）/ text（decisions/*.md 正文）
              │
              ├── ArchiModuleDetail.docs: Vec<DocRef>（恒四字段 path/title/kind/source）
              ├── ArchiModuleDetail.warnings: Vec<ArchiWarning>（WarningKind 单值 docNotFound，只报不拦）
              └── ArchiSearchResult.docs: Vec<DocRef>（scope = elements / docs / all）
```

### 病灶一 · 入口文档不可能经 docs 三源出线（位置纪律反转，E-1 已使它成为现实）

E-1 把契约资产搬进 `<archi_dir>/models/<Domain>/` 后，三条既有边界同时作废：

| 现状锚点 | 撞上了什么 |
|---|---|
| `docs.rs` 硬排除 `is_archi_rel_path`（archi 树整棵不可见） | 入口文档在 archi 树内 ⇒ **不可能**经 docs 三源出线 |
| `desktop-archi-module-docs`「模块文档 MUST NOT 存放于 `<archi_dir>/` 内」 | 位置纪律直接反转 |
| `desktop-archi-module-docs`「契约资产不经 docs 三源」（不新增第四源、不新增 `kind = api`） | 契约 md 成为入口文档后，该条要么改写成「入口文档是并列面」，要么作废 |

结论：入口文档只能是**并列面**（像 `api` 字段），不能塞回 docs 列表。本条是「为什么退役 docs 而不是扩一条来源」的全部理由。

### 病灶二 · `contract_path` 既是派生值又是契约装载源，再加一个可写的 `doc` 会静默放行

```text
E-1 落地后的事实                     doc 自由声明（explore 早期设计）的后果
contract_path = <models>/<D>/<FQN>.md   doc 可指别处
load_contracts 只读 contract_path       ┌ doc 指到别处的带 fence 文件
                                        ├ apiDocMissing 因「有 fence」不报
                                        ├ 装载器读派生路径 → 契约不在案
                                        └ apiMissing 形态①已退场 ⇒ 也不报
                                                    ↓
                                            零判据，静默通过
```

修法（explore 裁决 #20，本提案采纳）：**declared 元素的入口文档就是 `contract_path`**，`doc` 只服务 `api = none`——两个事实合并成一个，缺陷结构性消失，`contract::parse` 的输入源零改动。附带收益：「模块入口唯一」由路径派生兑现，变成结构性事实而非纪律。

### 病灶三 · 判据面把「没写」与「写错」混成同一个桶，且 `status` 纳管口径与触发条件互斥

- 今天 `apiMissing` 一个值同时承载「契约文件缺失」（迁移期主形态，41 条）与「契约声明了但代码无实现」——前者是「还没写」，后者是「承诺未兑现」，两者该有不同的门禁后果。
- `docNotFound`（旧，`WarningKind`）读 `docs` 数组且**只报不拦**；新 `docNotFound` 读 `doc` 单值且**纳管 `status`**——名字复用而非能力延续。
- `gating_findings` 现口径为「`api = declared` 且契约 md 在案」，而 `apiDocMissing` 的触发条件恰是「契约在盘但不成立（无 fence）」；两侧交集按旧口径永远为空 ⇒ 该判据进不了纳管面，`status` 上的黄灯静默丢失。
- `passed` 今天只由 `errors` + `model.structuralErrors` 决定，`apiFindings` 无论多严重都不影响门禁布尔——于是「补了文档之后的实现漂移」永远红不了。

### 现状实测（逐条在案）

| 事实 | 读数 |
|---|---|
| 元素 / 关系 | 53 / 120（`path` 非空 **44**；容器 / 包根 9 个无 `path`） |
| `api = none` | **2**（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`） |
| 契约在案 | **1**（`App.Backend.Infra.ArchiRuntime`，md 含 `api-op` + `api-types` fence） |
| `docs` 行 | **1 处 / 2 条**（`01-app.toml:7`，`App` → `./README.md` / `./AGENT.md`） |
| 两个工具 crate 的 README | **不存在**（`export-bindings/` 只有 `Cargo.toml` / `build.rs` / `src/`；`archi-api-gen/` 只有 `Cargo.toml` / `src/`）⇒ 2b 是「新写」不是「回填」 |
| `docs.rs` 消费者 | `dto.rs`(14) / `search.rs`(4) / `drift/report.rs`(1) / `infra/agent/src/sdk/mount.rs`(2) / `lib.rs`(2) |
| `archi_decisions_dir` 非测试消费者 | 唯一一处 `docs.rs`（`:155` / `:254`） |
| 判据面基线 | `apiEntryMissing 0` + `apiMissing 41` = **41**、`status = clean`、`passed = true`（第五段，已在 `desktop-archi-api-check` 在案） |
| 金样 | `corpus/drift/expected/*.json` 十六份；`api/**` sidecar 用例恰 **13** 份（`golden_test.rs:1468` 断言） |
| 前端 `docs` 消费者 | `element-drawer.tsx` 的 `DocsSection`（`api.contractPath` 已由 `ContractSection` 呈现 ⇒ 文档段对 declared 元素冗余） |

---

## 提案

### 一 · 字段面：`docs: Vec<String>` → `doc: Option<String>`，适用面由 `api` 状态决定

| 位置 | 变化 |
|---|---|
| `core/archi/src/model.rs` `Element` | `docs: Vec<String>` → `doc: Option<String>` |
| `core/archi/src/parse/toml.rs` `RawElement` | 同上；`deny_unknown_fields` 白名单同步（删 `docs`、加 `doc`） |
| 形态校验 | 复用 `api::paths` 既有 repo 相对 POSIX 判据（`normalize_root` + `is_repo_relative`；`is_repo_relative` 提 `pub(crate)`）——`./` 前缀不参与身份、无绝对前缀 / 无盘符 / 无 `..` / 无空段 |
| 与 `contract_path` 的关系 | **按 `api` 状态分工**（见下表），MUST NOT 双向可写 |

```text
api = declared（缺省）   入口文档 = 派生 <models>/<Domain>/<FQN>.md（= contract_path）
                         `doc` MUST NOT 写；写了即 apiUnexpected，且该值不参与任何判定
api = none               入口文档 = `doc` 声明（可选；不写 = 无入口文档，合法但不静默）
```

`api` 状态因此升格为**入口文档载体的开关**，而不是「契约有无」：

| `api` | 载体 | `doc` 字段 | 判定 |
|---|---|---|---|
| `declared` | 派生 `contract_path` | 不写 | 在盘 + 有 fence ⇒ 正常；不在盘 ⇒ `docMissing`；在盘无 fence ⇒ `apiDocMissing` |
| `declared` | —— | **写了** | **`apiUnexpected`**（两事实矛盾） |
| `none` | `doc` 声明 | 写了 | 在盘 ⇒ 正常；不在盘 ⇒ `docNotFound` |
| `none` | —— | 未写 | **`docMissing`**（无入口文档） |
| `none` | 派生路径不该有契约 | —— | 派生路径在盘 ⇒ **`apiUnexpected`**（既有，前置） |

### 二 · 判据面：十值 → 十三值，按 `api` 状态分两支、各自首错短路

新增三值（`ArchiApiFindingKind` + `ALL` + `as_str` + `severity()` 四处同批）：

| 新值 | 语义 | category | 档位 | 纳管 `status` |
|---|---|---|---|---|
| `docMissing` | **入口文档未就位**（declared：契约 md 未写 / none：`doc` 未声明） | `doc` | warn | **否** |
| `docNotFound` | **声明指不到**（仅声明类有此形态） | `doc` | **error** | 是 |
| `apiDocMissing` | 契约 md 在盘但**无 api fence**（「有文档无约定」中间态） | `doc` | warn | 是 |

`category` 开放值集由六值扩**七值**（+ `doc`）；`apiUndeclared` 仍 `off`、其余一致性判据仍 `warn`。

**求值顺序写死**（`check.rs` 函数体内，零配置键；`drift/config.rs` 保持两键封闭）：

```text
for element in elements where element.path 非空:
  api = none 支（3 步，与 declared 支互斥）
    ① 派生契约路径在盘 → apiUnexpected（continue）   ← 与 doc 无关，前置（否则一条缺 README 的元素会把放错位置的契约遮住）
    ② doc 未声明 → docMissing（continue）
    ③ doc 不在盘 → docNotFound
  api = declared 支（6 步）
    ④ 派生契约 md 不在盘 → docMissing（continue）    ← 原 apiMissing 形态①，改名不改事实
    ⑤ 契约在盘但无 fence → apiDocMissing（continue）
    ⑥ doc 字段被写了 → apiUnexpected（continue）
    ⑦ entry_state = Absent / Invalid → apiEntryMissing（continue）
    ⑧ 契约声明 op / $defs 而代码无实现 → apiMissing（只剩形态②）
    ⑨ 一致性判据（signatureMismatch / reexportDrift / constructorDeclared）
pair 锚点（不进元素循环，与元素锚点互不挤占）
    ⓐ apiNestedCrossCrate → ⓑ apiRefMissing → ⓒ apiShadowing
```

- **分界不是排版而是依赖**：①–⑥ 只读模型 + 盘（载体存活性、fence 有无），**零导出集依赖**；⑦ 起经 `element_symbols` 需 `entry_state = Resolved`，无入口即空符号集。
- **首错短路**：每个元素锚点命中第一条即 `break`；pair 锚点内部序为「结构前提 → 目标可达性 → 命名冲突」，跨锚点不比较。
- **pair 锚点不得落 `relation`**：父子在模型里没有边（层级由 FQN 最长前缀推导），`$ref` 目标可能不在模型内——写成 `relation` 会被 `graph.rs` 的 `DependsOnAncestor` / `unknownReference` 先判结构错。

### 三 · 档位三值与 `passed` 判据（本变更唯一跨进门禁面的地方）

`ApiFindingSeverity` 由二值（`Off` / `Warn`）扩三值（+ `Error`）。`derive_status` 在纳管子集上分档：任一 `Error` ⇒ `errors`；否则非空 ⇒ `warnings`。

```text
passed = errors 空 ∧ model.structuralErrors 空 ∧ error 档 apiFindings 空
```

于是四档对应四层承诺深度：**入口文档还没写不红（warn）/ 声明指不到红 / 有文档无 fence 黄 / 有契约无实现红**。

**结构性收益**（安全网由短路结构给出，不靠白名单）：`error` 档只可能落在「入口文档已就位」的元素上 ⇒ **存量项目没补文档就永远不会红**。

**代价照账**：`passed` 被 `error` 档接管后，契约实现漂移从此能阻断归档。这是既有 spec 自己规定的同批动作（`desktop-archi-api-check`「把某类判据改 `error` 档与改 `passed` 判据 SHALL 同批进行」），其反向纪律「`apiFindings` MUST NOT 影响 `passed`」随之作废。

### 四 · 纳管子集：由「契约在案」改为「入口文档承诺已成立」

```text
纳管(条目) ⟺ element 非空 ∧ (
      api = declared : 派生契约 md 在盘
    ∨ api = none     : `doc` 字段已声明        ← 声明本身即承诺（docNotFound 判的正是它）
)
```

- `docMissing` 两侧都不成立（它是「承诺尚未给出」）⇒ **永不纳管**，`status` 不被 43 条读数拖成常态 `warnings`。
- `docNotFound`（error）与 `apiDocMissing`（warn）都纳管——按 explore §3.5「none 取 `doc` 在盘」的字面写法，`docNotFound` 的载体必然不在盘，二者互斥；本提案取「**已声明**」，使档位表与纳管表同时成立。此为 explore 措辞的补记（R-2），不改任何裁决。
- 既有实现口径「`api = declared` **且**契约 md 在案」落到「载体按 `api` 状态取值」，`is_file` 仍是同一次调用，单点仍在 `api::gating_findings`。
- **`status == errors ⟺ passed == false` 不变量保持**：`error` 档判据（`docNotFound` / `apiMissing` / `apiSignatureMismatch`）的触发条件都蕴含纳管成立。

### 五 · 线面与检索面

| 项 | 变化 |
|---|---|
| `ArchiModuleDetail.docs: Vec<DocRef>` | **删除** |
| `ArchiModuleDetail.warnings: Vec<ArchiWarning>` | **删除**（`WarningKind` 单值枚举随 `DocNotFound` 一并退役——空枚举不留；`ArchiWarning` 因此零消费者，同批退役） |
| `ArchiModuleDetail.doc: Option<String>` | **新增**：repo 相对 POSIX 路径单值，无 `title` / `kind` / `source`。只有 `api = none` 元素会填 |
| `ArchiElement.doc: Option<String>` | **新增（授权进场字段）**：模型字段的拷贝，与 `entry` 同型（单值、缺省 `null`、显式字段不省略、字段序紧跟 `entry`）。`archi_graph` 的 `elements[]` 与 `archi_module` 的 `element` 共用该投影，故载体派生（`paths::carrier_path`）、`doc_facts` 装配、`gating_findings` 的 none 支与 `check` 的 ⑥ / ③ 都以它为入参——它不新增第三个事实源，与 `ArchiModuleDetail.doc` 同值同源。`ArchiElementBrief` MUST NOT 加字段 |
| `DocRef` / `DocKind` / `DocSource` / `DocWarning` | 退役 |
| `ArchiSearchResult.docs: Vec<DocRef>` | → `doc: Vec<String>`：语料 = `path` 非空元素的**在盘入口文档载体**（declared 取派生 `contractPath`、none 取 `doc`），条目退化为路径串 |
| `SearchScope` | 三值保留、**值名改 `doc`**：`elements` / `doc` / `all`；Rust 变体 `Docs` → `Doc`，`parse` 匹配臂与非法值文案同步 |
| `dto::linked_doc_paths` | 退役（关联集路径集的唯一含义随关联集消失；产品侧零消费者——文档正文早归通用 `commands::fs::read_file`） |

**次生后果（显式承认）**：ADR 不再经 `text` 源自动关联到元素 ⇒ `decisions/*.md` 退出 archi 自动检索语料，只能被入口文档链接、被文件树看见、被通用文件搜索命中。

### 六 · `api::enabled` 收紧（E-1 移交项，行为变化不只是措辞）

现判据 = `<archi_models_dir>` 递归 ≥1 份 `*.md`，`models/` 里放一份笔记就会误判契约面启用。改为「**存在任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标**」——即判据面 `DocFact::Present { has_api_fence: true }` 的同一事实，杂项 md 不再误启用；`enabled` 由 `enabled(root)` 改 `enabled(root, elements)`，`DocFact` 装配后单点判定。

### 七 · 两处配套清理

| 项 | 处置 |
|---|---|
| `foundation::layout` 的 `archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` | **删除**（唯一非测试消费者是 `docs.rs`）；`decisions/` 目录与 `0001-model-granularity.md` 本体不动——退役的是派生单点，不是资产 |
| `archi_knowledge.rs` 知识文本「决策记录目录：{}」一句 | 删除（ADR 定位改由入口文档链接 / 文件树承担） |

### 八 · 落地时序（对 explore §9 的一处收正）

explore §9 把「模型删 `docs` 行」划入 2b（数据面）。**这不可行**：`deny_unknown_fields` 白名单一旦删 `docs`，仍写着 `docs` 的 `01-app.toml` 立刻产 `unknownField` 并使整图停摆（画布、`archi_module`、门禁同时失效）。故：

```text
2a（本 change）：字段面 + 判据 + 线面 + docs.rs 删除 + archi_decisions_dir 删除 + 前端 + agent + spec 改写
                **+ `01-app.toml` 删那 2 行 docs**（与白名单同批，否则本仓模型当场停摆）
2b（另批，全为加法、零停摆风险）：两个工具 crate 补 `doc` 行 + 新建两份 README；41 份契约 md 渐进补齐
```

### 九 · 读数（第六段）

```text
第六段：apiEntryMissing 0 + docMissing 43 = 43
历史：28/13(41) → 15/26(41) → 12/27(39) → 12/29(41) → 0/41(41) → 0/43(43)

43 = 41（declared 且派生契约 md 不在盘 → 改名自 apiMissing 形态①）
   +  2（api = none 且未声明 doc → 首次进入判据面）
分母 44 = 53 元素 − 9 个无 path 的容器 / 包根；1 个契约在案元素零判据
```

**必须读准**：这不是「新增 43 条债」，而是「同一批 41 份契约换了个桶名 + 2 个新入面的元素」；`apiMissing` 形态①移出后只剩形态②，读数归 0。`status` 仍 `clean`、`passed` 仍 `true`（43 条全 warn 且 `docMissing` 不纳管）。

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-module-docs` | **REMOVED ×5**（整份退役） | 三源关联 + 恒四字段契约 / 合并去重与 `docNotFound` 只报不拦 / 文档位置与邻近扫描范围 / 看护与检索不读 specs / 契约资产不经 docs 三源——五条全数退役，Reason 与 Migration 逐条给；**四条迁移边界的承接方**：检索面三条（不读 `specs` / 不返回正文 / 不提供跨元素反向索引）归 `desktop-archi-agent`（ADDED），看护与门禁不读 `specs` 归 `desktop-archi-drift`（ADDED） |
| `repo-archi-model` | MODIFIED ×3 | 基线落位与模型文件划分（`docs` 纪律 → `doc` **仅 `api = none` 用**；`App` 的两份根文档回归普通仓库文件、不进模型）；图自洽与停摆禁令（`docNotFound` 零告警 → 新口径）；模型与裁定记录的可检索性（ADR `text` 源自关联退役，检索改口径） |
| `desktop-archi-model` | MODIFIED ×3 | 架构模型文件格式与字段封闭（八字段面 `docs` → `doc`，值域与形态校验、`api` 状态作为载体开关）；消费面词汇集一致性对拍（删「文档诊断 docNotFound」一行）；IPC 命令面契约（`ArchiModuleDetail` 的 `docs` + `warnings` → `doc`） |
| `desktop-archi-api-check` | MODIFIED ×5 | 词汇封闭十值 → **十三值** + `category` 七值；十条判据 → **十三值三段两支首错 + pair 锚点**；判据级档位二值 → **三值**；报告并列槽位（`passed` 判据 + 纳管子集口径 + `REPORT_VERSION` 不动仍 3）；验证语料与基线（**第六段 `0 / 43`**、金样首错重录、sidecar 仍 13 份不新增） |
| `desktop-archi-api` | MODIFIED ×1 | 契约资产形态与唯一手写真相：契约 md = declared 元素的**入口文档**（落点唯一、`doc` MUST NOT 另指）、`api` 状态 = 入口文档载体开关；原「不接入 `docs` 三源」改为「不接入文档关联面（三源已退役）」 |
| `desktop-archi-contract-panel` | MODIFIED ×3 | 检查命令契约（`enabled` 口径改「载体在盘且含 fence」+ 冷启动态新形态）；两轴并列呈现与状态矩阵（`category` 六值 → 七值含 `doc`）；抽屉契约段（「文档段」退役后的不混装措辞） |
| `desktop-archi-agent` | MODIFIED ×3 + ADDED ×1 | `archi_module`（`docs` → `doc`、`warnings` 退役）；`archi_search`（`scope` 词 `doc`、语料改入口文档、结果字段 `doc: Vec<String>`）；系统知识注入（字段面 / 示例两段 / 决策记录目录句删除）；**ADDED** 检索面纪律承接（不读 `specs` / 不返回正文 / 无跨元素反向索引） |
| `desktop-archi-canvas` | MODIFIED ×1 | 抽屉内容与本期边界：「文档」段（三源条目 + ADR 条目）→ **入口文档单条**（仅 `api = none` 且 `doc` 非空时呈），declared 走契约段、MUST NOT 两条 |
| `desktop-archi-drift` | MODIFIED ×1 + ADDED ×1 | 报告结构与 `status` 五值（纳管子集口径 / `passed` 判据变化 / `WarningKind` 退役后的词汇组表述）；**ADDED** 看护与门禁不读 `specs`（迁移边界承接） |

`desktop-archi-file-tree`：**零 delta**（全文无 `docs` 字段依赖；E-1 已把「契约资产与契约索引不进树」写成扩展名判据，`doc` 面不加新条目类型）。

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/model.rs` | `Element.docs: Vec<String>` → `doc: Option<String>` |
| `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` | `RawElement.docs` → `doc: Option<String>`；`deny_unknown_fields` 白名单同步（删 `docs` / 加 `doc`） |
| `packages/desktop/src-tauri/crates/core/archi/src/docs.rs` + `docs_test.rs` | **整文件删除**（三源关联、`DocsOutcome` / `DocWarning` / `normalize_rel` / `title_of` / ADR 读盘 / 邻近扫描）；`file_stem` 迁驻 `search.rs`（检索命中仍在用） |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 退役；`ArchiModuleDetail.docs` + `.warnings` 删、`.doc: Option<String>` 加；`ArchiSearchResult.docs` → `doc: Vec<String>`；`SearchScope::Docs` → `Doc`（`parse` 匹配臂与非法值文案）；`ArchiElement.doc: Option<String>` 加（模型字段的拷贝，与 `entry` 同型；`to_element` 逐元素拷贝）；`module_detail` / `search` 装配改写；`linked_doc_paths` 退役 |
| `packages/desktop/src-tauri/crates/core/archi/src/search.rs` | 语料改「`path` 非空元素的在盘入口文档载体」；`SearchOutcome.docs: Vec<DocRef>` → `doc: Vec<String>`；`matches_doc` 吃路径串；`file_stem` 从 `docs.rs` 迁入 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | `pub mod docs` 删；`DocKind` / `DocRef` / `DocSource` / `WarningKind` 出口删；`#[cfg(test)] mod docs_test` 删 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` | 新增**入口文档载体派生单点**（declared → `contract_path`；none → `doc_path`，后者 = `normalize_root` + `is_repo_relative` 归一后 join root）；`is_repo_relative` 提 `pub(crate)` |
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | `ApiSnapshot` / `ApiCheckInput` 增 `doc_facts: BTreeMap<String, DocFact>`（load 阶段一次性装配：`Absent` / `Missing` / `Present { has_api_fence }`）；`load_contracts` 同批产出；`enabled(root)` → `enabled(root, elements)`（判据改为「任一载体在盘且含 fence」）；`gating_findings` 口径改「载体按 `api` 状态取值」 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` | `FENCE_OP` / `FENCE_TYPES` 提 `pub(crate)`（`apiDocMissing` 与 `enabled` 复用同一常量，MUST NOT 抄第二份字面量） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` | 十三值三段两支首错求值 + `category_of` 加 `doc` + pair 锚点（nested → ref → shadowing）；`missing_contracts` 只剩形态②（形态①改名 `docMissing`）；`apiUnexpected` 增「declared 写了 doc」形态；模块头注释求值序改写 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` | `ApiFindingSeverity` 二值 → 三值（+ `Error`、`ALL` / `as_str`）；`ArchiApiFindingKind` 十值 → 十三值（`ALL` / `as_str` / `severity()`）；模块头「四组词汇」表述随 `WarningKind` 退役校准 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` | `derive_status` 在纳管子集上分档（任一 `Error` ⇒ `errors`）；`check()` 的 `passed` 判据追加「error 档判据为空」；`gating` 传参口径随 `gating_findings` 改 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/gate.rs`（若 `passed` 落在此处，按实际落点改） | 门禁布尔 `passed` 判据追加 error 档判据项；`LocalArchiCheck` 的报告结论同源 |
| `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 删 `DECISIONS_DIR_NAME` / `archi_decisions_dir` / `archi_decisions_rel_dir` |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` | 派生段删「决策记录目录：{}」一句；字段面与示例改写 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | 字段面八项 `docs` → `doc`（并写明「`doc` 只服务 `api = none`」）；示例改两段（`api = "none"` + `doc = "<某 README>"` / declared 零 `doc` 行）；写纪律措辞随「只能改模型与 `doc` 所指文档」 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | `archi_module` 描述去「关联文档（四字段条目）」+ 去 `docNotFound` 警告句、加「`doc` 仅 `api = none` 元素有值」；`archi_search` 描述与 `scope` enum → `["elements", "doc", "all"]` |
| `packages/desktop/src/views/architecture/components/element-drawer.tsx` | `DocsSection` / `DocRow` 删；`doc` 非空时（仅 `api = none`）呈「入口文档」单条（`FileViewer` + `read_file`）；declared 沿用契约段 `ContractPathRow("契约")`，MUST NOT 两条入口文档 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 重导（`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 退场，`doc` 三处进场：`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`） |
| `.dev-team/architecture/models/01-app.toml` | 删 `docs = ["./README.md", "./AGENT.md"]` 一行（**与 parser 白名单同批**，见提案八） |
| `AGENT.md` / `CLAUDE.md` `## 模块 API 契约` | 加一条要点：`doc` 单值仅 `api = none` 用、declared 入口文档 = 派生 `contract_path`；把「契约在案元素的判据进 status、不进 passed」改写为「入口文档在案的 error 档判据进 `status` **与** `passed`」；十三值 / 三段求值一句指路；两份逐字相同 |
| `packages/desktop/package.json` | `version` +1 patch（0.6.52 → 0.6.53；桌面源码改动，无例外） |

### 测试文件

| 对象 | 判断 |
|---|---|
| `crates/core/archi/src/docs_test.rs` | **删除**（与 `docs.rs` 同批） |
| `crates/core/archi/src/api/check_test.rs` | 改 + 扩：十三值正反例（`docMissing` 两支 / `docNotFound` / `apiDocMissing`）、两支互斥与首错（同元素只报一条）、pair 锚点三条、`apiUnexpected` 双形态、`api = none` 元素退出入口判据、`category` 七值打标、`severity()` 三值与 `ALL` 逐值对拍——**全部落在既有 sidecar 内，MUST NOT 新增 `api/**` 文件**（`golden_test.rs:1468` 断言 sidecar 恰 13 份） |
| `crates/core/archi/src/api/mod_test.rs` | 改：`enabled` 用例族由「递归任意 md」重写为「载体在盘且含 fence」（原 `README.md` / 大小写 / 深度 / 符号链接 / 同名目录五类形态按新判据重录）；`gating_findings` 用例加 none 支（`doc` 已声明纳管 / 未声明不纳管）；`ApiCheckInput` 字面量补 `doc_facts`（机械） |
| `crates/core/archi/src/dto_test.rs` | 改：`WarningKind` / `DocKind` / `DocSource` / `DocRef` 词汇用例删；`SearchScope::parse("doc")` 与非法值文案；`module_detail` 的 `doc` 两态（none 有值 / declared `null`）与 `docs` / `warnings` 缺席；`ArchiElement.doc` 两态（`to_element` 拷贝，none 有值 / declared `null`）与 `ArchiElementBrief` 零 `doc`；`linked_doc_paths` 用例删；`search` 语料改入口文档 |
| `crates/core/archi/src/search_test.rs` | 改：语料夹具由「`docs` 数组 + 邻近 md + ADR」改为「契约 md 在盘 / `doc` 声明在盘」；`search` 结果字段 `doc: Vec<String>`；scope 值名换词 |
| `crates/core/archi/src/model_test.rs` + `parse/toml_test.rs` | 改：`docs` 语料 → `doc` 单值三态（省略 → `None` / 单值 / 数组被拒 `invalidValue`）+ 拼错键 `unknownField`（`doc` 入白名单的机械面） |
| `crates/core/archi/src/tree_test.rs` / `graph_test.rs` / `diff_test.rs` / `edit_test.rs` | 改：夹具里的 `docs = [...]` 行按新字段面清掉（机械；判定零变化） |
| `crates/core/archi/tests/vocabulary_test.rs` | 改：删 `WarningKind` / `DocKind` / `DocSource` 三组词汇对拍；`SearchScope` 三值词面换 `doc`；`ArchiApiFindingKind::ALL` 长度 10 → 13 与 `severity()` 三值对拍 |
| `crates/core/archi/tests/golden_test.rs` + `corpus/drift/**` | 改：首错改写使「同元素多报」降为一条 ⇒ 十六份金样**需核对并重录**（`DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工确认）；`api-mismatch` case 的契约夹具补 fence 使 `apiDocMissing` 不遮蔽既有 `apiSignatureMismatch` 场景（或按新序重录该 case 的期望条目并留痕）；sidecar 计数断言（13）**不变** |
| `crates/core/foundation/src/layout/mod_test.rs` | 改：`archi_decisions_dir` / `archi_decisions_rel_dir` 用例删；`archi_models_dir` / `archi_graph_dir` 族零改动 |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 改：「决策记录目录」断言删；字段面八项断言 `docs` → `doc`；示例片段两段化；「虚构 docs 路径只报 warning」侧证删（`docs` 面已不存在） |
| `crates/infra/agent/src/sdk/archi_tool_test.rs` | 改：夹具 `docs = [...]` → `doc`；响应断言 `detail["doc"]` 单值 + 薄概览无 `doc`；`scope = "doc"` 档与 `docs` 档非法值 |
| `crates/infra/agent/src/sdk/tools_test.rs` | 改：`archi_search` 的 `scope` enum 断言 `["elements", "doc", "all"]` |
| `crates/tools/archi-api-gen/src/main_test.rs` | 改：仅夹具 `docs` 行清理（机械；生成器语义零变化） |
| `src/commands/archi/mod_test.rs` | 改：`linked_doc_paths` 用例删（函数退役）；`archi_module` 响应的 `doc` 字段断言；「关联集零契约正文」否定断言改指「契约路径不出现在 `doc` 内」 |
| `src/bindings/mod_test.rs` | 改：出线 DTO 清单删 `DocRef` / `DocKind` / `DocSource` / `WarningKind`、`ArchiWarning`；`DocRef` 四字段断言删；清单长度连带核对随动（`ArchiElement` 本就在清单内，`doc` 是字段级进场 ⇒ 清单成员不增删） |
| `packages/desktop/src/views/architecture/components/element-drawer.test.tsx` | 改：`docsFixture` 删；`docs: []` fixture → `doc: null`；`archi-docs-empty` 用例改「入口文档空态」；「契约段在文档段之前」的段序断言改为「契约段在入口文档段之前」；新增 declared 元素零入口文档行的否定断言 |
| `packages/desktop/src/views/architecture/hooks/use-archi-module.test.ts` / `architecture-view.test.tsx` / `architecture-canvas.test.tsx` / `lib/drift-marks.test.ts` | 改：DTO 载荷样例的 `docs` → `doc`（机械） |
| 验收留痕（`reports/`） | 三份实测：① 判据面读数（`docMissing 43`、其余 12 值各 0、`status = clean`、`passed = true`）+ 逐桶成因；② 模型读数（53 / 120 不变、`modelHash` **必然变化**——`01-app.toml` 删一行）；③ `api:export` 后 `INDEX.toml` / `.schema.json` / `.i.rs` 逐字节零变化（租约面零改动） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `api/contract.rs` 的解析 / 三向 gate / `$defs` 规则 | `contract::parse` 的**输入源零改动**（declared 的入口文档就是 `contract_path`），五类位置语义与四条校验逐字不变；本变更只提 `FENCE_OP` / `FENCE_TYPES` 的可见性 |
| `api/extract/**` / `api/emit/**` / `api/render.rs` / `api/schema.rs` | 取数、渲染、索引装配零改动（判据读符号集的口径不变） |
| `api/diff.rs` / `infra/archi/src/api_artifacts.rs` | 生成物三类差异与写盘单点零改动（`doc` 不是生成物） |
| `drift/config.rs` | 保持两键封闭（`ignore` / `exclude`）+ `deny_unknown_fields`——求值顺序写死，MUST NOT 引入配置键、MUST NOT 留空壳键 |
| `drift/report.rs` 的 `REPORT_VERSION` | 仍为 **3**（字段面无新增 / 删除——`apiFindings` 形状不变，只换取值与档位；递增须有形状变化作理由） |
| `crates/core/archi/src/tree.rs` / `scan/**` | 文件树枚举面与漂移扫描面零改动（`doc` 不是源文件） |
| `desktop-archi-file-tree` / `desktop-gate` 等未列能力 | 出现 spec diff 即误改 |
| `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md` 与 `.schema.json` | 试点契约内容零改动（它是第六段唯一零判据样本） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 目录与文件本体留盘；本变更不写 ADR（计数与 `docs` 无关） |
| 两个工具 crate 的 `README.md` 与 41 份契约 md | **2b 数据面，不在本 change**（explore §9 / §12.2）；本 change MUST NOT 顺手新建 |

---

## 验收标准

1. **AC-1 · 字段面换面干净**：`model.rs` / `parse/toml.rs` 只有 `doc: Option<String>`，`docs` 字样在产品源码零命中；`deny_unknown_fields` 白名单含 `doc`、不含 `docs`——反向判据：模型写 `docs = [...]` 即 `unknownField` 并使模型停摆（本仓 `01-app.toml` 的两行已同批删除，`archi_graph` 零诊断）。
2. **AC-2 · `doc` 适用面结构性成立**：`api = declared` 的元素写 `doc` ⇒ 一条 `apiUnexpected`（`category = entry`、warn）且该值不参与任何其它判定；`api = none` 的元素 `doc` 指向不存在文件 ⇒ 一条 `docNotFound`（`category = doc`、**error**）。`doc` 的形态校验经 `api::paths` 单点（`docs::normalize_rel` 已随 `docs.rs` 删除，MUST NOT 留第二份归一实现）。
3. **AC-3 · 十三值与三段两支**：`ArchiApiFindingKind::ALL` 长度 **13**、与 `as_str` 逐值对拍、`severity()` 三值（`Error` 恰为 `docNotFound` / `apiMissing` / `apiSignatureMismatch` 三项，其余一致性判据 `warn`、`apiUndeclared` `off`）；`category_of` 七值（+ `doc`）。每元素锚点**首错短路**（同元素只产一条），pair 锚点独立于元素循环且序为 `nested → ref → shadowing`。反向判据：两支互斥（一个元素不同时报两支的判据）。
4. **AC-4 · `passed` 与 `status` 同批兑现**：`passed = errors 空 ∧ model.structuralErrors 空 ∧ error 档 apiFindings 空`；`derive_status` 在纳管子集上分档（任一 `Error` ⇒ `errors`）。纳管口径 = `element` 非空 ∧（declared：契约 md 在盘 / none：`doc` 已声明），`docMissing` **永不纳管**。三条不变量：① `status == errors ⟺ passed == false`；② 存量项目（零入口文档）`status` 恒 `clean`；③ 单元用例：`docMissing` 43 条在场而 `status = clean`。
5. **AC-5 · 第六段读数与成因**：`apiFindings` = `docMissing 43`（41 条改名自 `apiMissing` 形态① + 2 条 `api = none` 未声明）、`apiMissing 0`（形态①移出后只剩形态②）、其余十一值各 0、`status = clean`、`passed = true`；分母 44 与 `api = none` 2 在案。spec 与报告 SHALL 写明「不是新增 43 条债」，`41 / 43` MUST NOT 被并排写成「债务增长」。
6. **AC-6 · 线面与检索面换面**：`ArchiModuleDetail` 不再有 `docs` / `warnings`，新增 `doc: Option<String>`（仅 `api = none` 有值）；`ArchiSearchResult.doc: Vec<String>`（语料 = `path` 非空元素的在盘入口文档载体）；`ArchiElement` 只在 `entry` 之后新增 `doc: Option<String>`（模型拷贝，与 `entry` 同型，`archi_graph` 与 `archi_module` 同值同源），`ArchiElementBrief` / `ArchiApiRef` 字段面零变化；`SearchScope` 线格式词 `elements` / `doc` / `all`，`"docs"` 为非法值并列出合法值；`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` / `DocWarning` / `linked_doc_paths` 全仓零命中；`bindings:export` 幂等。
7. **AC-7 · 启用判据收紧（行为变化）**：`api::enabled(root, elements)` 判据 = 任一 `path` 非空元素的入口文档载体在盘且正文含 `api-op` / `api-types` fence 语言标；fence 语言标常量单点复用 `contract::FENCE_OP` / `FENCE_TYPES`（MUST NOT 抄字面量）。反向判据：`models/notes.md`（非任何元素的载体）与 `<Domain>/notes.md`（无 fence）都 MUST NOT 构成启用证据；本仓读数仍 `true`。
8. **AC-8 · adr 派生与知识文本清理**：`foundation::layout` 的 `archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` 三符号全仓零命中（含 `*_test.rs`）；`archi_knowledge()` 输出零「决策记录目录」串、含 `doc` 字段纪律与两段示例、体量上限不变；`decisions/0001-model-granularity.md` 文件本体仍在盘。
9. **AC-9 · 前端单条入口文档**：`element-drawer.tsx` 零 `DocsSection` / 零 `DocRef` 依赖；`api = none` 且 `doc` 非空 ⇒ 呈「入口文档」单条（可经 `FileViewer` 打开）；`api = declared` ⇒ 只有契约段一条 `ContractPathRow("契约")`，**MUST NOT 出现两条入口文档**；空态 testid 与文案随字段面更名。
10. **AC-10 · 金样与语料守护**：`api/**` sidecar 仍恰 **13** 份（新判据用例落在 `check_test.rs`，MUST NOT 新建 sidecar 文件）；十六份 drift 金样由 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认，逐份留痕「哪一条因首错改写而变化」；`desktop-archi-api-check` 的「已知缺口」句就地留痕——「十判据各自正反例」MUST NOT 被读成「十三判据各自正反例」。
11. **AC-11 · 零扩散与版本纪律**：`git status --porcelain` 的改动集 ⊆ 本表所列文件（含 `.dev-team/architecture/models/01-app.toml` 一行删除）；`packages/desktop/package.json` `version` 相对落地时 `HEAD` 恰 +1 patch；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本逐字不变；`openspec/specs/` 下除本 delta 的 9 个能力外零 diff（`desktop-archi-file-tree` 零 diff 是判据之一）。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **`deny_unknown_fields` 白名单与模型数据不同批 ⇒ 本仓模型当场停摆**（explore §9 把删 `docs` 行划进 2b，不可行） | 提案八已收正：`01-app.toml` 的一行删除落 **2a**；2b 只剩纯加法（`doc` 行 + 两份 README + 41 份契约 md）。AC-1 的反向判据把「写 `docs` 即停摆」写成机械面 |
| R-2 | **纳管子集措辞与档位表互斥**（explore §3.5 写「none 取 `doc` 在盘」，而 `docNotFound` 的成因恰是载体不在盘 ⇒ 它永远落选，与其「纳管 = 是 / error」矛盾） | 取「`api = none` ⇒ **`doc` 已声明**」作为纳管判据，使档位表与纳管表同时成立；不改任何裁决，仅补记措辞。AC-4 用三条不变量把口径钉住（含 `status == errors ⟺ passed == false`） |
| R-3 | **`apiSignatureMismatch` 的档位沿革自相矛盾**（explore §3.4 的锚点表写 `error`，§3.5 的档位表把它归入「其余一致性判据 warn」） | 取 §3.4（`error`），理由是 §0 的目标是「补了文档之后的**实现漂移**能真正红门禁」，而签名不符正是实现漂移；§3.6 的结构性论证（`error` 档只落在「文档在案且带 fence」的元素上）对签名判据同样成立。AC-3 把 `severity()` 的三值白名单写死（`docNotFound` / `apiMissing` / `apiSignatureMismatch`），避免落地时再猜 |
| R-4 | **`api = none` 元素静默退出入口判据**（explore 的 none 支只有三步，没有 `apiEntryMissing`；这与现行 spec「`api = none` 不豁免本值」直接冲突） | 显式承认为语义变化并留痕：`entry` 的判据面只服务「契约面的代码取数」（`element_symbols` 需 `Resolved`），而 `api = none` 元素的契约面整体不参与，入口解析对它们零消费者。本仓读数 0 不变（两工具 crate 各有显式 `entry`）。`desktop-archi-api-check` 的该句与 `repo-archi-model` 的三处 `entry` 后果表述同批改写 |
| R-5 | **`passed` 被 error 档接管后能阻断归档**（首次让契约判据参与门禁布尔） | 这是既有 spec 自己规定的同批动作，照账兑现；结构性安全网是短路顺序（`error` 档只可能落在入口文档在案的元素上）⇒ 存量项目零风险。AC-4 ② 把它写成机械断言（零入口文档 ⇒ `status` 恒 `clean`） |
| R-6 | **首错改写使十六份 drift 金样读数变化**，「同元素多报」降为一条（如 `apiMissing` + `apiReexportDrift`、`apiMissing` + `apiConstructorDeclared`） | AC-10 要求显式重写 + 逐份留痕成因；代码注释里「与 ① 独立、MUST NOT 与 ② 双报」一类纪律句必须同批改写，否则下一个人会按旧口径写回归用例 |
| R-7 | **`apiDocMissing` 占首错名额 ⇒ 它是 `apiMissing` 的可达性上界**（「写了 doc 但没 fence」的元素始终到不了 `error` 面） | 显式承认为刻意缺口，补偿是它纳管 `status`（该元素至少 `warnings`，不会静默 `clean`）⇒「没写 doc」与「写了 doc 没写契约」在状态词上可区分。spec 内 MUST NOT 把它写成「不影响结论」 |
| R-8 | **`enabled` 判据收紧是行为变化不是措辞**（`enabled(root)` → `enabled(root, elements)`，`mod_test` 的五类形态用例全部要重录） | 提案六与 AC-7；`enabled` 的消费者只有四处（`findings` / `artifact_check` / 面板 DTO / 测试），签名改动面可控；默认「冷启动零读盘」（载体不在盘 ⇒ 零文件读）写进实现约束 |
| R-9 | **`doc` 的归一实现出现第二份**（`docs.rs::normalize_rel` 随文件删除，容易被逐个消费者各自重写） | 载体派生与形态校验收进 `api::paths` 单点（`doc_path` + `is_repo_relative` 提 `pub(crate)`），AC-2 把它写成判据；`search` 只吃载体路径，不自建归一 |
| R-10 | **`archi_search` 的 `scope` 词改名破坏既有调用面**（`docs` → `doc` 是非法值） | agent 工具面零前端消费者；`parse` 的非法值文案列出合法值，用例（`archi_tool_test` / `tools_test` / `dto_test`）同批换词；MUST NOT 为兼容保留 `docs` 别名（双词会让 scope 语义二义） |
| R-11 | **ADR 退出自动检索语料**是显式接受的次生后果，容易被读成回归 | `repo-archi-model` 的 ADR scenario 与 `desktop-archi-agent` 的「关键词检索同时命中元素与裁定记录」同批改写；`desktop-archi-module-docs` 的 REMOVED Migration 段写明 ADR 的三条替代抵达路径（入口文档链接 / 文件树 / 通用搜索） |
| R-12 | **`desktop-archi-module-docs` 整份删除会静默丢弃仍有效的纪律**（四条迁移边界、不返回正文、无跨元素反向索引） | 删前先指定承接方：检索面三条 → `desktop-archi-agent`（ADDED）、看护与门禁不读 `specs` → `desktop-archi-drift`（ADDED）。承接 MUST 与 REMOVED 同一 change 落地，MUST NOT「先删后补」 |
| R-13 | **`Reports/archi` 与门禁在线读数的误读**（宿主门禁二进制是变更前构建，E-1 已立口径） | 本 change 的读数以**测试相位 + change 报告留痕**为准；`reports/archi/` 仍视为门禁运行时产物（不入库、不计入交付面、不作判红面），MUST NOT 以 `apiFindings: []` 或空目录充当 AC-5 证据 |
| R-14 | **2b 未做时 `docMissing` 43 条会被读成「实现没完成」** | spec 与报告写死第六段的成因（41 改名 + 2 新入面），并写明「末行全 0 的前提是按现有实现写契约」；`docMissing` `warn` 且不纳管 ⇒ 交付面 `status` 仍 `clean`、`passed` 仍 `true` |
| R-15 | **`doc` 在 declared 元素上的「写了也不参与判定」容易被实现成「静默忽略」** | AC-2 要求「写了即 `apiUnexpected`」——它是两事实矛盾的可见面，短路顺序里排 ⑥（`docMissing` / `apiDocMissing` 之后），确保它不是「第一个报出的错」而是「已具契约者身上的矛盾」 |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §12.1 `apiUndeclared` 的语义前提（声明集依赖 fence 存在） | **采纳并落 spec**：`apiUndeclared` 档位 `off` 不变，其声明集只在「契约在盘且含 fence」（即过 ④ ⑤）时才有对象——该句写进 `desktop-archi-api-check` 的判据定义 |
| §12.2 43 份入口文档的写法（41 份契约 + 2 份 README） | **不在本 change**：2b 数据面规划，本 change MUST NOT 新建任何 README / 契约 md（见「不要修改」） |
| §12.3 既有 drift 金样复核 | **本 change 内做**：首错改写 ⇒ 重录 + 逐份成因留痕（AC-10 / R-6） |
| §12.4 第六段实测复跑 | 已结案（E-1 合入后离线可推）；本 change 的读数以测试相位实测复核（AC-5） |
| §13.4.1 `desktop-archi-module-docs` 的纪律承接方 | **已定**：检索面三条 → `desktop-archi-agent`（ADDED）；看护与门禁不读 `specs` → `desktop-archi-drift`（ADDED）（R-12） |
| §13.4.2 `App` 两份根文档的去处 | **已定**：`App` 是 `package`（无 `path`），新纪律不要求也不允许它写 `doc`；`README.md` / `AGENT.md` 回归普通仓库文件，由仓库结构本身表达、不进模型面。该句写进 `repo-archi-model`「基线落位与模型文件划分」 |
| §13.4.3 `decisions/` 在 `archi_decisions_dir` 删除后的定位 | **已定**：目录与 ADR 文件留在盘上，程序化访问面归零；将来需要程序化访问须重新引入单点（MUST NOT 手写字面量）。写进 `workspace-layout-resolution` 的 delta 范围外——本变更只删符号，不新增纪律；该口径以 `desktop-archi-module-docs` 的 REMOVED Migration 段留痕 |
