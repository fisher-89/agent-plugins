# 变更 E-2 · `doc` 字段替代 docs 能力（十三值判据 · 三段求值 · 双锚点）

workflow_type: requirement

> **定位**：`openspec/todo.md:77` 第三件「模块入口文档」，接 5.8 待裁项。本文档只承载**能力面**。
> **前置已完成**：E-1（`2026-11-18-module-entry-docs-semantics`）**已合入并归档**——`contract_path` = `<models>/<Domain>/<FQN>.md`、索引落 `<archi_dir>/graph/INDEX.toml`、`api/` 目录已退役、`archi_api_dir` 族已删。
> **同源**：E-1 的 `explore.md`（`openspec/changes/archive/2026-11-18-module-entry-docs-semantics/`）§E.5.7 把接缝 C 的四项裁决移交本文档，对应关系见 §1.1。
> **状态**：口径已全部收紧（二十轮裁决，见附录 A）；E-1 落地后的复核见 §13。§12 只剩 3 项实现期事实核对。

---

## 0 一句话

模型元素新增**可选单值** `doc`（入口文档，只记路径），**只对 `api = none` 的元素有意义**——`api = declared` 的入口文档恒为派生落点 `<models>/<Domain>/<FQN>.md`，不必声明也不许声明。
`docs: Vec<String>` 与整个 `docs.rs` 三源关联能力退役；判据面按 `api` 状态分成互斥两支、各自首错短路，共**十三值**。
档位提到 `error`，使「补了文档之后的实现漂移」能真正红门禁。
`docMissing` 统一承担「**入口文档未就位**」（declared = 契约 md 未写 / none = doc 未声明）；`apiMissing` 收窄为「契约声明了但代码无实现」。

---

## 1 为什么退役：docs 三源不可能承载入口文档

### 1.1 接缝：位置纪律反转（E-1 已落地，接缝成为现实）

E-1 把契约资产搬进 `<archi_dir>/models/` 后，入口文档落进了架构域树内，三条既有边界同时作废：

| 现状锚点 | 撞上了什么 |
|---|---|
| `docs.rs` 硬排除 `is_archi_rel_path`（archi 树整棵不可见） | 入口文档在 archi 树内 ⇒ **不可能**经 docs 三源出线 |
| `desktop-archi-module-docs`：「模块文档 MUST NOT 存放于 `<archi_dir>/` 内」 | 位置纪律直接反转 |
| `desktop-archi-module-docs`：「契约资产不经 docs 三源」（不新增第四源、不新增 `kind = api`） | 契约 md 成为入口文档后，该条要么改写成「入口文档是并列面」，要么作废 |

结论：入口文档只能是**并列面**（像 `api` 字段），不可能塞回 docs 列表。

E-1 归档时把这四项裁决的来源编号一并移交（其 `explore.md` §E.5.7 的映射表）：

| E-1 原编号 | 内容 | 本文档落点 |
|---|---|---|
| E.5.7 | 入口文档承载面不可经 docs 三源 | §1.1 |
| E.5.8 | 入口文档单源 + 模块不再关联 docs（Skills 式链接） | §1.2 |
| E.5.9 | 唯一 `doc` 替代 docs 职能（抽屉冗余核对） | §1.2 / §1.4 |
| E.5.10 | `archi_search` 文档检索面 | §1.4 |
| E.6.6 / E.7 / E.8 | 判据面三次迭代 → 四层阶梯 + 十三值 | §3 |

**E-1 落地实况（本仓实测）**：`models/` 一级仍是六个 `NN-<域>.toml`，`models/Infra/` 下有一份契约 md + schema；`graph/INDEX.toml` 在案；`api/` 目录不存在；`foundation` 侧 `archi_api_dir` / `archi_api_rel_dir` 已删、`archi_graph_dir` / `archi_graph_rel_dir` 已增（`archi_decisions_dir` 仍在，处置见 §11）。

### 1.2 替代形态：单源入口 + 自链其他文档

裁决原话：「使用唯一 doc 替代原 docs 的职能」。这不是字段改名，是**能力替换**——多源关联列表被单份入口文档取代。

形态类比 Skills：`SKILL.md` 引 `references/`，系统只认那一个入口。

```text
入口文档（单源，系统认）              其他文档（自管，系统不建索引）
<models>/<Domain>/<FQN>.md   ──链接──▶  references/*.md
  ├── ## 职能     ← 「读了就知道模块做什么」        ADR
  ├── ## 不变量                                    设计说明
  ├── ## 操作（api-op fence）                      迁移记录
  └── ## 类型（api-types fence）
```

- **「模块入口唯一」由路径派生兑现**（FQN → 单一路径），不靠 docs 数组首位语义，也不靠「唯一性判据」——原 E.2 落法 b（docs 首位即入口）由此自然淘汰。
- **抽屉现状证实文档段冗余**：契约段 `ContractSection` 已在显示 `api.contractPath`（即 declared 元素的入口文档路径），文档段 `DocsSection` 整体冗余。
- **`doc` 字段因此只服务 `api = none` 的元素**（§1.3）：declared 的入口文档由派生落点给出，作者既不必声明、也不许另指别处——否则契约正文会与实际装载的路径分裂成两个事实。
- **`App` 的两份 docs 不收敛**：`./README.md` / `./AGENT.md` 并存不是歧义（面向人 vs 面向 agent）。docs 整体退役后两份文件回归普通仓库文件，只是不再被模型关联。

### 1.3 `doc` 的适用面由 `api` 状态决定

| `api` | 入口文档在哪 | `doc` 字段 |
|---|---|---|
| `declared`（缺省） | **派生**：`contract_path` = `<models>/<Domain>/<FQN>.md` | **不写**；写了即 `apiUnexpected`（两事实矛盾），且该值不参与任何判定 |
| `none` | **声明**：`doc` 指向普通文档（不带 fence） | 可选；不写 = 无入口文档（合法状态） |

**为什么 declared 不给声明位（E-1 落地后收紧）**：`contract_path` 既是派生值又是契约装载源（`load_contracts` 只认它），schema 与 `$ref` 相对前缀也都挂在它下面。若允许 `doc` 另指别处，就会出现「doc 里有 fence、装载器读的是另一个路径」——契约没被装载，而所有判据都以为它在案。**唯一合法值就是派生值 ⇒ 不必声明。** 这一条同时把「入口唯一」变成结构性事实，而非纪律。

**为什么 none 保留声明而不是推导**：这类元素的入口文档可以是任意普通文档（README / 设计说明），无派生可依；且「完全没有入口文档」必须是可表达的状态。

### 1.4 检索面：语料 = `doc` 声明

`archi_search` 是 agent 专用工具（前端零消费者），今天三 scope（`elements` / `docs` / `all`），`docs` 语料 = 三源全量（含 ADR）。

**裁决**：保留文档检索面，语料 = **所有元素的入口文档**（declared 取派生 `contract_path`、none 取 `doc` 声明，只收在盘者）；`scope` 三值语义不变但**值名改 `doc`**（`elements` / `doc` / `all`）；`DocRef` 整族退役，检索条目退化为**一条路径字符串**。

命中面与今天**逐字一致**：`search.rs::matches_doc` 只吃 `doc.path` 与 `file_stem(path)`（源码注释明写「标题取自正文首行，属正文射程」，`title` 不参与命中）⇒ `title` 在检索面本就零作用，退役零功能损失。

**次生后果（显式承认）**：ADR 不再经 `text` 源自动关联到元素 ⇒ `decisions/*.md` **退出 archi 自动检索语料**，只能被入口文档链接、被文件树看见、被通用文件搜索命中。`repo-archi-model` 的 scenario「`archi_module("App")` 的 docs 中出现该 ADR 条目」与 `desktop-archi-agent` 的「关键词检索同时命中元素与裁定记录」都必须改写/删除。

---

## 2 字段面

| 位置 | 变化 |
|---|---|
| `core/archi/src/model.rs` `Element` | `docs: Vec<String>` → `doc: Option<String>` |
| `core/archi/src/parse/toml.rs` `RawElement` | 同上；`deny_unknown_fields` 白名单同步（删 `docs`、加 `doc`） |
| 形态校验 | 复用 `normalize_root` / `normalize_rel` 同族（repo 相对 POSIX、`./` 前缀不参与身份、无绝对前缀 / 无 `..` / 无空段） |
| 与 `contract_path` 的关系 | **按 `api` 状态分工**（§1.3）：declared ⇒ 入口文档 = `contract_path`（派生，`doc` 不许写）；none ⇒ 入口文档 = `doc`（声明，可省） |

**字段纪律（写进 `repo-archi-model`）**：`path` 非空元素按 `api` 状态二选一——

```toml
# 形态 i：本模块无公共 API（api = none）
api = "none"
doc = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"   # 该 README 尚不存在，2b 一并新建

# 形态 ii：有公共 API（api 缺省 = declared）—— 零 doc 行
# 入口文档 = .dev-team/architecture/models/<Domain>/<FQN>.md（派生，不写在模型里）
```

`api = declared` / `none` 的角色因此是**入口文档载体的开关**，而不是"契约有无"：

| `api` | 入口文档载体 | `doc` 字段 | 判定 |
|---|---|---|---|
| `declared` | 派生 `contract_path` | 不写 | 文件在盘 + 有 fence ⇒ 正常；不在盘 ⇒ `docMissing`；在盘无 fence ⇒ `apiDocMissing` |
| `declared` | —— | **写了** | **`apiUnexpected`**（两事实矛盾） |
| `none` | `doc` 声明 | 写了 | 在盘 ⇒ 正常；不在盘 ⇒ `docNotFound` |
| `none` | —— | 未写 | **`docMissing`**（无入口文档） |
| `none` | 派生路径不该有契约 | —— | 派生路径在盘 ⇒ **`apiUnexpected`**（既有） |

> **实测提醒（E-1 落地后核对）**：形态 i 的例子路径今天**不存在**——两个工具 crate 目录下都没有 `README.md`（`export-bindings/` 只有 `Cargo.toml` / `build.rs` / `src/`，`archi-api-gen/` 只有 `Cargo.toml` / `src/`）。故 `docMissing` 的读数里含这 2 条，兑现方式不是"回填已有文件"而是"**新写两份 README**"（2b 数据面）。MUST NOT 在 spec 里把它写成"指向现有文档"。

---

## 3 判据面

### 3.1 十三值词表

既有十：`apiEntryMissing` / `apiMissing` / `apiUndeclared` / `apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiReexportDrift` / `apiRefMissing` / `apiNestedCrossCrate` / `apiConstructorDeclared`

新增三：`docMissing` / `docNotFound` / `apiDocMissing` = **13 值**（`ArchiApiFindingKind` + `ALL` + `as_str` + `severity()`）

`category` 开放值集新增 `doc`（三条新判据打标）。既有「单点打标 + 契约测试覆盖已知分类」纪律不变。

**名字复用留痕**：新 `docNotFound` 与退役的 `WarningKind::DocNotFound` 同名但载体不同——新的读 `doc` 单值，旧的读 `docs` 数组。spec 改写须说明是**名字复用而非能力延续**。

### 3.2 两条互斥的链（按 `api` 状态分支）

`doc` 的适用面由 `api` 状态决定（§1.3），所以判据面天然分成两支——**结构上互斥，不需要优先级排序**：

```text
api = none                           api = declared
─────────────────────────────        ─────────────────────────────────────
docMissing      无入口文档            docMissing       入口文档未就位
                (doc 未声明)                          (派生契约 md 不在盘)
   ↓                                   ↓
docNotFound     doc 指不到盘          apiDocMissing    契约 md 在盘但无 fence
                (声明类才有此形态)                     (「有文档无约定」中间态)
   ↓                                   ↓
apiUnexpected   派生路径冒出契约       apiEntryMissing  无可用入口（code 面）
                (留白被违反)                          ↓
                                     apiMissing       契约有 op/$defs、代码无实现
                                                      ↓
                                     apiSignatureMismatch  归一签名不一致
```

**语义对照（三值各自管什么）**：

| kind | 语义 | 覆盖 |
|---|---|---|
| `docMissing` | **入口文档未就位** | 统一两种成因：declared 的契约 md 未写 / none 的 doc 未声明 |
| `docNotFound` | **声明指不到**（声明类才有此形态） | none 的 `doc` 指向不存在的文件；declared 无对象（它的落点是派生的，缺失归 `docMissing`） |
| `apiMissing` | **有约定无兑现**（收窄） | 只剩原形态②：契约在案但 op / `$defs` 键在导出集里找不到。原形态①（契约文件缺失）已由 `docMissing` 承担 |

**这条划分的意义**：`docMissing` 与 `docNotFound` 不再互相纠缠——**派生缺失 = 没写（warn）、声明指错 = 笔误（error）**。同一个 kind 不必承载"两种缺口"，两个 kind 各自只有一种成因。

**链内是对象依赖，不是人为优先级**：

```text
docMissing 命中（declared）⇒ 没有契约 md ⇒ 没有 fence ⇒ apiDocMissing / apiMissing 天然零命中
                           （不是"跳过求值"，是"无对象可求值"）
```

这条结构解释的意义：首错短路（§3.4）在这一链上**遮蔽的不是真问题而是尚不存在的对象**，因此安全。

### 3.3 求值顺序（三段）

```text
for element in elements where element.path 非空:
  if element.api == none:                      ── none 支（不依赖入口）──
      ① 派生契约路径在盘 → apiUnexpected（continue）      ← 留白被违反，与 doc 无关，先判
      ② doc 未声明 → docMissing（continue）
      ③ doc 不在盘 → docNotFound
  else:                                        ── declared 支 ──
      ④ 派生契约 md 不在盘 → docMissing（continue）        ← 原 apiMissing 形态①
      ⑤ 契约在盘但无 fence → apiDocMissing（continue）
      ⑥ doc 字段被写了 → apiUnexpected（continue，两事实矛盾）
      ── 以下属 code 面（需 entry_state = Resolved）──
      ⑦ entry_state：Absent / Invalid → apiEntryMissing（continue）
      ⑧ 契约声明 op / $defs 而代码无实现 → apiMissing
      ⑨ 一致性判据（signature / reexport / constructor）
```

**分界不是排版，是依赖**：①–⑥ 只读模型 + 盘（`element.doc` 存活性、`contract_path` 存活性、fence 有无），**零导出集依赖**；⑦ 起的 code 面经 `element_symbols` 需 `entry_state = Resolved`，无入口即空符号集。

**为什么 `apiEntryMissing` 落在这里**：`api/mod.rs:411` 的 `element_symbols` 在入口非 `Resolved` 时返回空符号集——没入口就没导出集，`apiMissing` / `apiSignatureMismatch` / `apiUndeclared` 全都无对象可判。所以它短路 code 面不是抢名额，是 code 面无对象。

**顺序写死**在 `check.rs` 求值函数体中，不由配置驱动：`drift/config.rs` 保持两键封闭（`ignore` / `exclude`）+ `deny_unknown_fields`，与 `desktop-archi-api-check:127`「MUST NOT 引入配置键」同向；「可设置」搁置且**不预铺**（不留空壳键、不留 TODO）。

### 3.4 首错短路与两种锚点

裁决原话：「规则可设置检查顺序，仅提示第一个错误。」

**元素锚点**：每个元素命中第一条即 `break`，不再求后续判据。

```text
none 支                               declared 支
① apiUnexpected   warn               ④ docMissing          warn
② docMissing      warn               ⑤ apiDocMissing       warn
③ docNotFound     error              ⑥ apiUnexpected       warn
                                     ⑦ apiEntryMissing     warn
                                     ⑧ apiMissing          error
                                     ⑨ apiSignatureMismatch error
                                     ⑩ reexportDrift / constructorDeclared warn
                                        apiUndeclared       off（不产出）
```

两支内部的序号即求值序；跨支不比较（同一元素只走一支）。

> **none 支为何把 `apiUnexpected` 排第一**：它与 `doc` 无关（判的是"派生路径上冒出了契约"），而 `docMissing` 是**不入纳管**的 warn。若让 `docMissing` 先短路，一条 `api = none` 的元素只要没写 README，就能把它那份放错位置的契约遮住 ⇒ `status` 仍 `clean`。这与 §13.1 关掉的洞同类（非纳管判据遮蔽纳管判据），故显式前置。

**pair 锚点**：三条跨元素判据不在单元素循环内，它们产出**元素对**，与元素锚点并列、互不挤占：

```text
ⓐ apiNestedCrossCrate   warn   父子所属包根不同
ⓑ apiRefMissing         warn   引用方 → $ref 目标（三种问题）
ⓒ apiShadowing          warn   父子 $defs 类型名交集
```

pair 内顺序：`nested → ref → shadowing`。理由：nested 是**结构前提**（跨包不可表达，下游不用谈）→ ref 是**目标可达性**（够不够得着）→ shadowing 是**命名冲突**（够着了但重名）。

**能落 pair、不能落 relation**——形状能，语义不能：

| 判据 | 它的 pair | 为什么不能引用 `Model.relations` |
|---|---|---|
| `apiShadowing` | 父子 | 父子在模型里**没有边**——层级由 FQN 最长前缀推导，`[[relation]]` 只承载依赖；写成 relation 会被 `graph.rs` 的 `DependsOnAncestor` 判结构错 |
| `apiNestedCrossCrate` | 父子 | 同上；语义恰是「父子**不该**跨包」，声明成依赖边自相矛盾 |
| `apiRefMissing` | 引用方 → `$ref` 目标 | 最重的理由「模型里没有该元素」**不可能**是合法 relation：`graph.rs::unknown_reference` 会让那条边先变成结构错误 |

结论：pair 锚点是**判据当场算出的事实对**，MUST NOT 引用 `Model.relations`、不要求模型里有那条边。形状零改动——`ArchiApiFinding { element, target, … }` 本就是 `(from, to)`，稳定排序键 `(kind, element, target)` 与之逐字一致。

**附带收益**：⑧ 类的三条今天把 finding 记在**父元素**头上，与父自己的 doc/api 链抢同一名额；改 pair 锚点后父元素的名额干净了，子元素也照常独立参与自己的元素链。

### 3.5 档位与纳管

档位与纳管**正交**：纳管决定进不进 `status` 推导，档位决定 `status` 取 `errors` 还是 `warnings`。

| 判据 | 档位 | 纳管 `status` | 理由 |
|---|---|---|---|
| `docMissing` | warn | **否** | 「还没写」——纳管会让 `status` 在任何元素缺入口文档时恒 `warnings`，状态词失去区分度 |
| `docNotFound` | **error** | 是 | 声明指不到 = 纯笔误，**无中间态可辩**；放任即「把 doc 指向不存在文件」的消红后门 |
| `apiDocMissing` | warn | 是 | 「承诺未展开」是**补文档的合法中间态**；纳管只为亮黄灯，不红门 |
| `apiMissing` | **error** | 是 | 契约已展开、兑现是义务 |
| `apiUnexpected` | warn | 是 | 两事实矛盾；其中「declared 却写了 doc」已由契约装载面兜住（§1.3），不需红门 |
| `apiUndeclared` | off | —— | 唯一完备性判据，迁移期全开等于门禁永久红 |
| 其余一致性判据 | warn | 是（承诺在案者） | 沿用既有 |

**分档的动机是接入成本**：初始只补部分模块文档（doc 面 `warn` 可渐进），补了文档之后代码漂移应尽快修（api 面 `error`）。

**结构性收益**：`error` 档只可能落在「doc 在案且带 fence」的元素上 ⇒ **存量项目没补文档就永远不会红**。安全网由短路结构给出，不靠白名单、不靠文档纪律。

**唯一的刻意缺口**：`apiDocMissing` 是 warn 且占首错名额 ⇒ 它是 `apiMissing` 的**可达性上界**——「写了 doc 但没写 fence」的元素始终到不了 error 面。补偿是它纳管 `status`（该元素至少 `warnings`，不会静默 `clean`），于是「没写 doc」与「写了 doc 没写契约」在状态词上可区分。

**纳管子集定义改写**：**「契约在案元素」→「入口文档在案元素」**，即入口文档载体已就位——declared 取 `contract_path` 在盘，none 取 `doc` 在盘。

> 这是一处真 bug 修复：既有 `gating_findings` 的口径是「`api = declared` **且契约 md 在案**」，而 `apiDocMissing` 的触发条件恰恰是**契约不成立（无 fence）**——两者交集为空，按旧口径它永远进不了纳管面。代码面动作是把口径落到「入口文档载体在盘」，`is_file` 的调用仍是同一次，只是载体按 api 状态取值。

### 3.6 `passed` 判据

```text
passed = errors 空 ∧ model.structuralErrors 空 ∧ error 档 apiFindings 空
```

于是四档正好对应四层承诺深度：**入口文档还没写不红（warn）/ 声明指不到红 / 有文档无 fence 黄 / 有契约无实现红**。

**这是本变更唯一跨进「门禁面」的地方**，且是既有 spec 自己规定的：`desktop-archi-api-check:156` 写着「把某类判据改 `error` 档与改 `passed` 判据 SHALL 同批进行」，`:155` 写着「`apiFindings` MUST NOT 影响 `passed`」——引入 `error` 就得就地兑现这两句，后者随之作废。

**代价必须认账**：`passed` 被 error 档接管后，**契约实现漂移从此能阻断归档**。

**由此 `severity` 第一次真正驱动推导**（今天它只用于 `apiUndeclared` 关档）：

1. `ApiFindingSeverity` 由二值（`Off` / `Warn`）扩三值（+ `Error`）及 `as_str` / `ALL`；
2. `derive_status` 加分档——`gating` 非空不再一律 `warnings`，任一 `Error` ⇒ `errors`；
3. `passed` 判据同上。

**唯一不红的缺口**：「无入口」永远 `warn`（裁决：`apiEntryMissing` 不收紧）。它是「**判不了**」而非「**判错了**」——有对象、结论为否才该红。本仓该桶现为 0（barrel 变更已归零），接入期不构成实际风险。

### 3.7 `apiDocMissing` 的实现口径：一次廉价扫描

裁决原话：「扫描先按小成本实现。」

判据只需要**一个布尔**——doc 正文里有没有 api 契约——故不接 `contract::parse`（那会连带 `schema::build_schema` + `contract::verify` + `schema::verify_defs`，且坏契约走 `Err` 而非判据，语义错位）。落法：

```text
load 阶段（一次性，已有读盘面）                     check 阶段（保持零读盘）
for element where path 非空:
    载体 = if api == none { element.doc } else { contract_path(fqn) }
         │
    载体缺省（none 且未写 doc）──▶ DocFact::Absent
         │
    is_file? ──否──▶ DocFact::Missing
         │是
    读一次文本 → 扫行判有无 fence 语言标 ──▶ DocFact::Present { has_api_fence }
                                                    │
                                                    ▼
                                      input.doc_facts: BTreeMap<Fqn, DocFact>
```

- **fence 语言标单点复用**：`contract.rs:37` / `:40` 的 `FENCE_OP` / `FENCE_TYPES` 常量（`pub(crate)` 提升），检查侧 MUST NOT 抄第二份字面量。判据取「`FENCE_OP` ∈ tokens ∨ `FENCE_TYPES` ∈ tokens」，与解析器的 token 判定同源。
- **`check` 边界不破**：`check.rs` 文件头声明「纯读 `input`（零读盘、零写盘、零 spawn）」，故事实由 load 阶段装配进 `ApiCheckInput`，判据只读结构体。
- **顺带解决纳管载体**：`DocFact` 同时回答「入口文档载体在盘」与「有 fence」，`gating_findings` 的「入口文档在案」（§3.5）直接取 `DocFact::Present`，不再自行 `is_file`。
- **已知精度上限**（可接受）：只判「有无 fence 语言标」，不判 fence 是否合法、`### <op-id>` 是否齐、`api-types` 是否重复——那些是 `contract::parse` 的 `Err` 面，属于「契约解析失败」（既有诊断），不归本判据。**MUST NOT** 因精度不足而在检查侧补写半个 markdown 解析器。
- **零额外 IO**：doc 读一次即弃，不缓存全文（判据不需要正文）。

---

## 4 线面

| 项 | 变化 |
|---|---|
| `ArchiModuleDetail.docs: Vec<DocRef>` | **删除** |
| `ArchiModuleDetail.warnings: Vec<ArchiWarning>` | **删除**（`WarningKind` 单值枚举 `DocNotFound` 随之退役——空枚举不留） |
| `ArchiModuleDetail.doc: Option<String>` | **新增**：repo 相对 POSIX 路径单值，无 `title` / `kind` / `source`。**只有 `api = none` 元素会填**；declared 元素的入口文档走既有 `api.contractPath`（不重复出线） |
| `DocRef` / `DocKind` / `DocSource` / `DocWarning` | 退役 |
| `ArchiSearchResult.docs: Vec<DocRef>` | → `doc: Vec<String>`：语料 = 所有元素**在盘的入口文档**（declared 取派生 `contractPath`、none 取 `doc`），条目退化为路径串（字段名随 scope 值名对齐——继续叫 `docs` 会暗示旧四字段条目） |
| `SearchScope` | 三值保留、**值名改 `doc`**：`elements` / `doc` / `all`；Rust 变体 `Docs` → `Doc`（`rename_all = lowercase` 即得线格式词），`parse` 的匹配臂与非法值文案同步 |
| 投影族 `graph_view` / `module_detail` / `search` | 去掉 docs 装配，新增 doc 投影 |

**顺带收益**：`desktop-archi-module-docs` 的「不返回正文」纪律在 `doc` 面上变成**结构性成立**——字段里只有路径，没有承载标题/摘要的位置。

**前端零新增轴**：declared 元素的入口文档早已由契约段 `ContractPathRow("契约", api.contractPath)` 呈现，`doc` 只为 none 元素补一条；抽屉不会出现"两处都在说入口文档"的重复。

---

## 5 前端

| 位置 | 变化 |
|---|---|
| `views/architecture/components/element-drawer.tsx` | `DocsSection` 删除；`doc` 非空时（仅 `api = none` 元素）呈「入口文档」单条；declared 元素沿用契约段的 `ContractPathRow("契约", api.contractPath)`，**MUST NOT 出现两条入口文档** |
| 对应测试 | `archi-docs-empty` 空态、文档序断言、`docs` fixture 清理 |
| `types/generated/bindings.ts` / `.zod.ts` | `bindings:export` 重导 |

---

## 6 agent 面

| 位置 | 变化 |
|---|---|
| `infra/agent/src/sdk/archi_tool.rs` | `archi_module` 去 `docs`、加 `doc`；`archi_search` 文档语料改法 |
| `infra/agent/src/archi-knowledge.md` + `archi_knowledge.rs`（`include_str!` 装配） | 字段面清单（`docs` 删除、`doc` 仅 `api = none` 用）、示例改写为「`api = "none"` + `doc = "<某 README>"`」与「declared 元素零 doc 行」两段、写纪律措辞；`build()` 里「决策记录目录」一句删除（裁决 #19） |
| `infra/agent/src/sdk/mount.rs` | `archi_search` 的 tool schema `scope` enum（`:254`）→ `["elements", "doc", "all"]`；工具描述里「四字段文档条目」的措辞改为「文档路径」 |
| 注入矩阵 / 只读档位 | 零变化（工具名集合不变） |

---

## 7 spec 改写清单

| spec | 改写 |
|---|---|
| `desktop-archi-module-docs/` | 整份删除（AGENT.md：退役资产在同一 change 删 spec，不留空壳）——**但须先做 §11 的纪律承接** |
| `repo-archi-model` | 字段纪律（`docs` 退役 → `doc` **仅 `api = none` 使用**；declared 的入口文档 = 派生落点、MUST NOT 写 `doc`）、模型文件表、`docs` 相关 scenario（`docNotFound` 零告警 / `archi_module("App")` 的 ADR 条目）。`App` 是 `package`（无 `path`）⇒ 新纪律**不要求**它写 `doc`，两份根文档从此不被模型关联（去处在 §11 留档） |
| `desktop-archi-model` | 字段面（`docs` → `doc`） |
| `desktop-archi-api-check` | 十三值、`category` 初始集、基线表**第六段**（五段历史扩六段 / 总 43）、「两条切分口径」→ **按 api 状态的双支口径**、`apiMissing` 语义收窄（只剩形态②）、`docMissing` 统一承担「入口文档未就位」、纳管子集定义、档位三值、`passed` 判据、**元素锚点 + pair 锚点**、语料守护的「已知缺口」补句 |
| `desktop-archi-api` | **范围收窄**：E-1 已改完资产形态 / 索引落点 / 统一目录三段，E-2 只改「契约 md = declared 元素的入口文档（落点唯一、MUST NOT 另指）」与「`api = declared/none` 的角色 = 入口文档载体开关」两处；另须裁定 `api::enabled` 的判据（E-1 §E.6.3 未定点 2 明确移交 E-2，见 §11） |
| `desktop-archi-contract-panel` | `category = doc` 分组呈现 |
| `desktop-archi-agent` | `archi_module` 字段面、`archi_search` 文档语料、ADR 检索 scenario |
| `desktop-archi-canvas` | 抽屉「文档」段删除 / 入口文档单条 |
| `desktop-archi-drift` | 纳管子集定义（若该 spec 持有）、`passed` 判据 |
| `desktop-archi-file-tree` | 若引用 `docs` 则同步 |
| `AGENT.md` `## 模块 API 契约` | 加一句 `doc` 字段、十三值判据与三段求值 |

---

## 8 读数与验收

### 8.1 第六段

```text
第六段：apiEntryMissing 0 + docMissing 43 = 43
历史：28/13(41) → 15/26(41) → 12/27(39) → 12/29(41) → 0/41(41) → 0/43(43)
                                          ↑ barrel 变更（第五段，已归档）
```

| 时点 | `apiEntryMissing` | `apiMissing` | `docMissing` | 其余 | 总 |
|---|---|---|---|---|---|
| E-2 代码落地（字段在、数据未回填） | 0 | **0** | **43** | 0 | **43** |
| 2b 补两份 README（两个 `api = none` 工具 crate） | 0 | 0 | **41** | 0 | **41** |
| 2b 写齐 41 份契约 md（**按现有实现**写） | 0 | 0 | **0** | 0 | **0** |

**43 的构成（逐桶可核）**：

```text
41 = declared 且派生契约 md 不在盘        → docMissing（warn）「入口文档未就位」
 2 = api = none 且未声明 doc              → docMissing（warn）「无入口文档」
 1 = ArchiRuntime（契约在盘且有 fence）    → 零判据（试点元素，也是 2b 的唯一现成样本）
──
44 = path 非空元素总数（53 元素 − 9 个无 path 的容器 / 包根）
```

**第六段的性质必须写进 spec，否则下一个人会读错**：这不是「新增 43 条债」，而是——

1. **改名**：`apiMissing` 的 41 条原形态①（契约 md 未写）**改名**为 `docMissing`，事实同一；
2. **参与面 +2**：两个 `api = none` 元素首次进入判据面（过去契约面把它们整体排除，现在它们必须给出「本模块无 API + 入口文档在哪」）；
3. **形态①移出** `apiMissing` 后，它只剩形态②，**读数归 0**。

所以 `41 → 43` 的差额恰好是那 2 个 `api = none` 元素；分母 44 不变。

> 末行「全 0」的前提是**按现有实现写契约**：契约里只声明已有的 op / 类型，未写的归 `apiUndeclared`（档位 `off`，不产出）。若反过来"先写契约再补实现"，`apiMissing`（`error`）会立刻红——这正是 error 档的用途。

### 8.2 与 E-1 的 41 口径如何衔接（勿混用）

```text
E-1（api 面）：41 = 44 − api=none 2 − 契约在案 1     ← 只有 declared 元素参与 api 判据
E-2（doc 面）：43 = 41（declared 契约未写）+ 2（none 未声明）
```

两者不是两次债，是**同一批 41 份契约换了个桶名，加上 2 个新入面的元素**。任一处把「41」与「43」并排写成「债务增长」都读错。

**barrel 变更已把 `apiEntryMissing` 归零**（12 → 0，第五段完成），故第六段里这个桶是 0，不存在"被 doc 面遮住的 12 条"。若将来有元素缺入口，它按 §3.3 的 declared 支第 ⑦ 步报出（仍 `warn`）。

### 8.3 验收项

- `apiMissing` 41 → **0**（形态①移到 `docMissing`，**不是债务清零**）；`docMissing` 0 → **43**。
- `status` 仍 `clean`（纳管子集内判据 0 条）——43 条全在 warn 档且 `docMissing` 不入纳管。
- 既有 drift 金样（16 份）需复核：首错改写会改变「同元素多报」的场景数（§12）。
- **语料覆盖是已知缺口**：三条新判据**不新增 sidecar 正反例**——`golden_test.rs:1468` 断言「`api/**` sidecar 恰 13 份」，新增文件即撞红。代价须在 `desktop-archi-api-check:195` 就地留痕：「十判据各自正反例」不得被读成「十三判据各自正反例」。兜底只剩两层——crate 级 `corpus/drift/` 金样（不经本仓模型，对 `doc` 面零覆盖）与本仓实测留痕。

---

## 9 发布时序（R-10 同款）

`docs` 删除与 `doc` 新增都触发 `deny_unknown_fields`：旧版桌面端读到新模型即 `unknownField` → 结构错停摆。

```text
阶段 2a（代码）：字段面 + 判据 + 线面 + docs.rs 删除 + 前端 + agent + spec 改写 → 发版
阶段 2b（数据）：模型删 docs 行（今天仅 App 两行）/ 两个 api=none 元素补 doc（+ 新建两份 README）/ 其余 41 元素渐进补契约 md（零模型改动）
```

**任何新字段面都必须晚于含该字段的桌面端发布**，否则门禁先红（成因在宿主版本，不在实现自由度）。2a / 2b 可同 change 分两提交（沿 A / A-遗留 先例）。

---

## 10 依赖

- **E-1 已合入**（`2026-11-18-module-entry-docs-semantics`，已归档）：`contract_path` = `<models>/<Domain>/<FQN>.md`、索引 `<archi_dir>/graph/INDEX.toml`、`api/` 退役。
- E-2 因此不再有前置阻塞；E-1 留在 E-2 门上的两项（`api::enabled` 判据、`models/` 双身份的字段面含义）见 §11 与 §13。

---

## 11 退役影响面与配套清理（代码实测）

| 项 | 实测与处置 |
|---|---|
| `docs.rs` 消费者 | `dto.rs`(14) / `search.rs`(4) / `drift/report.rs`(1) / `infra/agent/src/sdk/mount.rs`(2) / `lib.rs`(2) |
| `foundation::layout::archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` | **去除**（裁决 #19）：`archi_decisions_dir` 的唯一非测试消费者是 `docs.rs`（`:155` / `:254`），三源退役即死代码 ⇒ 两个派生函数与常量一并删。连带两处非测试消费者须同批改：① `infra/agent/src/archi_knowledge.rs:51` 的知识文本「决策记录目录：{}」一句随之删除（ADR 定位改由入口文档链接 / 文件树承担，与裁决 #4 同向）；② `archi_knowledge_test.rs:129` 的「目录派生串在场」断言同步删。`decisions/` 目录本体与 `0001-model-granularity.md` **不动**——退役的是派生单点，不是资产 |
| `ArchiModuleDetail.warnings` | 前端**零消费者**（`element-drawer.tsx` 无 warnings 引用）⇒ 可直删，但须换掉 `repo-archi-model:232` 的「`docNotFound` 零告警」scenario |
| `App` 的两份 docs | 判据只对 `path` 非空元素求值，`App`（`package`，无 `path`）⇒ 新纪律**不要求**它写 `doc`，`README` / `AGENT.md` 失去模型关联。§1.2 只说「不收敛」，去处待裁 |
| `desktop-archi-module-docs` 整份删除 | ⚠️ 该 spec 还持有**仍有效**的纪律：四条「看护与检索不读 `openspec/specs/`」迁移边界、「不返回正文」、「不提供跨元素反向索引」。整份删 = 静默丢弃，**须先指定承接 spec**（`repo-archi-model` 或 `desktop-archi-agent`） |
| `contract::parse` 的输入源 | **零改动**（这是 B 方案的关键收益）：declared 元素的入口文档就是派生落点 `contract_path`，`doc` 不再参与契约装载，不存在「装载器读 A、声明写 B」的第二真相。要区分的两件事仍分层：无 fence ⇒ `apiDocMissing`（判据）；有 fence 但语法坏 ⇒ `Err`（既有诊断） |
| `title_of` / `file_stem` | `title_of` 唯一消费者是 `DocRef.title` ⇒ 随 `DocRef` 退役；`file_stem` 被 `search.rs:73` 用于命中，保留 |
| `api::enabled`（`api/mod.rs:325`） | 现判据 = `<models>` 递归 ≥1 份 `*.md`（E-1 换根，形态未变）。E-1 §E.6.3 未定点 2 **明确移交 E-2**：`models/` 里放一份杂项 md 就会误判契约面启用。E-2 应改为「**存在任一元素使入口文档载体在盘且含 fence**」——即直接复用 §3.7 的 `DocFact::Present { has_api_fence: true }`，判据与判据面同源，杂项 md 不再误启用。**这是 E-1 遗留的行为变化，不能只改措辞** |

---

## 12 仍开放的项

口径已全部收紧（原 7 项中 3 项本轮回执：scope 词 → `doc`、判据 → 廉价扫描、`archi_decisions_dir` → 去除）。剩余 4 项**均为实现期事实核对**，不影响 spec 起草：

1. **`apiUndeclared` 的语义前提**：档位 `off` 不变，但其声明集现在依赖 fence 存在，需在 spec 补一句。
2. **43 份入口文档的写法**：41 份契约 md 的「`## 职能` 写什么」+ 两份 README 的定位（2b 数据面规划）。逐元素判定已不成问题——载体由 `api` 状态决定，没有二选一。
3. **既有 drift 金样复核**：首错改写会把「同元素多报」降为一条（如 `apiMissing` + `apiReexportDrift`、`apiMissing` + `apiConstructorDeclared`），16 份金样的条目数 / 内容需核对重录；代码注释里「与 ① 独立、MUST NOT 与 ② 双报」的纪律随之改写。
4. ~~**第六段实测复跑**~~ → **已结案（E-1 合入后）**：E-1 的 proposal 已用离线推导复核 `44 − 2 − 1 = 41`（`apiEntryMissing 0` + `apiMissing 41`），现行 spec 的第五段在案值即 `0 + 41 = 41`；分母 44 与 `api = none` 2 已实测。第六段的 `0 / 43` 由此可推，无需再猜「12」（barrel 已归零）。

---

## 13 E-1 合入后的复核（本次）

E-1（`2026-11-18-module-entry-docs-semantics`）已合入归档，本文档据此复核，共发现 **1 处会静默放行的缺陷** 与 **6 处失效表述**：

### 13.1 一处真缺陷：`doc` 与派生落点分裂（已裁为 B，见裁决 #20）

```text
E-1 落地后的事实                     E-2 原设计（doc 自由声明）的后果
contract_path = <models>/<D>/<FQN>.md   doc 可指别处
load_contracts 只读 contract_path       ┌ doc 指到别处的带 fence 文件
                                        ├ apiDocMissing 因"有 fence"不报
                                        ├ 装载器读派生路径 → 契约不在案
                                        └ apiMissing 形态①已退场 ⇒ 也不报
                                                    ↓
                                            零判据，静默通过
```

**根因**：`contract_path` 既是派生值又是契约装载源，而 `doc` 是另一个可写事实。**修法（B）**：declared 元素的入口文档**就是** `contract_path`，`doc` 只服务 `api = none`——两个事实合并成一个，缺陷结构性消失，`contract::parse` 的输入源零改动。

### 13.2 六处失效表述（已就地修正）

| # | 原文表述 | 修正后 |
|---|---|---|
| 1 | 「主线：`openspec/explores/module-entry-docs-semantics.md`」 | 该文件从未在 `explores/` 存在；实际在 `openspec/changes/archive/2026-11-18-module-entry-docs-semantics/explore.md`，已更正指针并恢复 §E.5.7 的编号映射表 |
| 2 | 「E-1 未落地 / 硬依赖 E-1」 | 已合入：`api/` 退役、`archi_api_dir` 族删除、`graph/` 建立、`archi_decisions_dir` 仍在（§11 处置不变） |
| 3 | 「`docMissing` 44 条 = 44 份入口文档待写」 | **43 条**（41 契约未写 + 2 README 未建）；`api=none` 的 2 个元素是**首次进入判据面**，差额来源已写进 §8.1 |
| 4 | 「唯一 doc 替代 docs 职能（对全部元素）」 | `doc` 只服务 `api = none`；declared 的入口文档由派生兑现（§1.3） |
| 5 | 「`doc` 示例指向 `.../export-bindings/README.md`」 | 该文件**不存在**（两个工具 crate 均无 README）⇒ 例子保留但标注「2b 新建」，`docMissing` 里这 2 条是"新写"而非"回填" |
| 6 | 「`api::enabled` 无需动」 | E-1 §E.6.3 未定点 2 **明确移交 E-2**：现判据 = `<models>` 递归 ≥1 份 `*.md`，`models/` 里放一份笔记就误启用；E-2 应改为「存在入口文档载体在盘且含 fence」（复用 `DocFact`，§11） |

### 13.3 E-1 已改完、E-2 不必重复的

| 面 | E-1 已做 | E-2 的剩余动作 |
|---|---|---|
| `desktop-archi-api` 资产形态 / 索引落点 / 统一目录 rationale | 已改 | 只改「契约 md = declared 入口文档」「`api` 状态 = 载体开关」两处 |
| `repo-archi-model` 目录树 / `models/` 双身份 | 已改（`:26` / `:27`） | 只改字段面与 `docs` 相关 scenario |
| `foundation::layout` | `archi_api_dir` 族退役、`archi_graph_dir` 族新增 | 只删 `archi_decisions_dir` 族（裁决 #19） |
| 约 28 处 `.dev-team/architecture/api/...` 字面量 | 已随 E-1 换段 | 零动作 |

### 13.4 仍未解决（非阻塞，留给起草时定）

1. **`desktop-archi-module-docs` 整份删除的纪律承接方**：该 spec 还持有四条「看护与检索不读 `openspec/specs/`」迁移边界、「不返回正文」、「不提供跨元素反向索引」。承接方未定（`repo-archi-model` 还是 `desktop-archi-agent`），删前必须指定。
2. **`App` 两份根文档的去处**：`App` 是 `package`（无 `path`），新纪律不要求它写 `doc`，`README.md` / `AGENT.md` 从此不被模型关联。是否需要一条纪律（如「包根的两份根文档由仓库结构本身表达，不进模型」）未定。
3. **`decisions/` 目录在 `archi_decisions_dir` 删除后的定位**：目录与 ADR 文件都留在盘上（`0001-model-granularity.md`），但再无派生函数指向它——若将来需要程序化访问，须重新引入单点（MUST NOT 手写字面量）。

---

## 附录 A · 裁决沿革

| # | 裁决原话 / 议题 | 结论 | 影响处 |
|---|---|---|---|
| 1 | 「使用唯一 doc 替代原 docs 的职能」 | `docs: Vec<String>` 与 `docs.rs` 三源整体退役，`doc` 单值取代 | §1.2 |
| 2 | 入口文档能否经 docs 出线 | 不能（archi 树被硬排除）⇒ 只能是并列面 | §1.1 |
| 3 | `doc` 是否有派生缺省 | 无缺省：「无入口文档」是合法状态 | §1.3 |
| 4 | ADR 检索去向 | 彻底退出 archi 检索语料，只经入口文档链接 / 文件树 / 通用文件搜索抵达 | §1.4 |
| 5 | 判据面迭代 | 十值 → 十三值；`apiDocMissing` 一度删除后恢复；`apiMissing` 收窄为「有约定无兑现」 | §3.1 §3.2 |
| 6 | 「规则可设置检查顺序，仅提示第一个错误」 | 首错短路（每锚点一条） | §3.4 |
| 7 | 「doc 记录 path 即可」 | `doc: Option<String>`，无 `title` / `kind` / `source` | §2 §4 |
| 8 | 「doc → api → code 三者是顺序生成的关系，检查 docMissing 时 api 和 code 并未定档，因此可以短路」 | 短路合法；档位动机 = 存量接入成本 | §3.2 §3.5 |
| 9 | 「分别对应 warning 和 error」 | doc 面 warn、api 面 error | §3.5 |
| 10 | `docNotFound` vs `apiDocMissing` 分档 | `docNotFound` = error（纯笔误无中间态）、`apiDocMissing` = warn（合法中间态） | §3.5 |
| 11 | `apiEntryMissing` 是否收紧 | **不收紧**（报的是「判不了」不是「判错了」） | §3.6 |
| 12 | 出厂默认顺序 | **doc 面 → api 面 → entry → code 面** | §3.3 |
| 13 | 顺序是否可配置 | **写死**（`drift/config.rs` 零改动） | §3.3 |
| 14 | 跨元素判据能否落 relation | **落 pair 锚点**——形状能（已是 `(from,to)`），语义不能（父子无边、`$ref` 目标可能不在模型内） | §3.4 |
| 15 | 段号撞车（barrel 已占第五段） | **顺延**：E-2 = 第六段 | §8.1 |
| 16 | 三条新判据是否建 sidecar | **不建**（撞 `golden_test.rs:1468` 的「恰 13 份」） | §8.3 |
| 17 | `archi_search` 的 scope 词 | **改 `doc`**（`elements` / `doc` / `all`）；结果字段随之 `docs: Vec<DocRef>` → `doc: Vec<String>` | §1.4 §4 §6 |
| 18 | `apiDocMissing` 的精确判据 | **小成本扫描**：load 阶段读一次 doc 文本判「有无 fence 语言标」，不接 `contract::parse` | §3.7 |
| 19 | `archi_decisions_dir` 去留 | **去除**（两派生函数 + `DECISIONS_DIR_NAME` + 知识文本引用），`decisions/` 目录本体不动 | §11 |
| 20 | 「contract 应该是自动生成，固定输出到派生路径」→ doc 与派生落点不一致怎么办 | **B**：declared 元素**不写 `doc`**（入口文档即派生落点），`doc` 只服务 `api = none`；`docMissing` 统一承担「入口文档未就位」。由此 `apiMissing` 只剩形态②、`docNotFound` 只覆盖声明类，洞（doc 有 fence 而装载器读派生路径 ⇒ 零判据）自然消失 | §1.3 §2 §3.2 §3.3 §8 |

### A.1 被推翻的中间态（留痕，勿据此判断现状）

| 中间态 | 为何作废 |
|---|---|
| 总账 `12 / 44、总 56` | 把「分母 44」误当「桶容量」。每锚点只报一条 |
| 「`docMissing` 44 条 / 分母 44 全落 doc 面」 | 裁决 #20 后 declared 元素的入口文档由派生落点给出，`docMissing` 计 **43**（41 契约未写 + 2 none 未声明），且 1 个在案元素零判据 |
| 「`doc` 对全部元素可选无缺省」 | 裁决 #20 反转：declared 元素**不许写** `doc`（入口文档 = 派生落点），`doc` 只服务 `api = none` |
| 「`apiMissing` 形态①（契约文件缺失）退场」 | 形态①的事实没变，只是**改名**到 `docMissing`「入口文档未就位」——因为 declared 的入口文档就是契约 md，同一件事不该有两个名字 |
| 「用**结构性互斥**替代优先级短路」 | 裁决 #6 落定后改为**显式首错**。结构性互斥仍保留为「为什么短路在这里安全」的解释（§3.2），但不再是实现手段 |
| 第五段 `12 / 32` | 顺序改写（裁决 #12）后 `apiEntryMissing` 退到 code 面，12 条被 doc 面遮住 |
| 「检索条目 = 路径 + H1 标题」 | `matches_doc` 只吃 `path` + `file_stem`，`title` 本就零作用 |
| 「`apiFindings` MUST NOT 影响 `passed`」 | 引入 error 档后必须同批改写（§3.6） |
| 「`apiDocMissing` 判 `## 职能` 段缺失或空」 | 末次修正改为判「doc 正文有无 api fence」。职能段判据的动机（`contract::parse` 只收非空 `##` 段、缺段零报错、双向 gate 三条都不含职能段）仍然成立，但被 fence 判据覆盖——有 fence 就必然读了契约，无 fence 才需要指出来 |
| 「`doc ≡ contract_path` 时的双报冲突」 | 裁决 #20 从根上取消了这个议题：declared 的入口文档**就是** `contract_path`，不存在"既声明又派生"的两个事实 |
| 「四条新/改判据各一组正反例 sidecar」 | 裁决 #16 |
| ADR `text` 源自动关联 | 裁决 #4 |

### A.2 将来可引的相邻议题（不在本变更内）

`$ref` 事实与声明式 `relation` 边同形 ⇒「声明了依赖但 `$ref` 不存在」/「有 `$ref` 但没声明依赖」是两条天然可判的新判据。属「模型 vs 契约」的新对齐面，与本变更正交，且会新增 `ArchiApiFindingKind` 值——记此备将来引用。

---

## 附录 B · 代码锚点索引

| 锚点 | 位置 | 本变更的动作 |
|---|---|---|
| `Element.docs` | `core/archi/src/model.rs:56` | → `doc: Option<String>` |
| `RawElement.docs` | `core/archi/src/parse/toml.rs:45` | 同上 + `deny_unknown_fields` 白名单 |
| `docs.rs`（三源关联） | `core/archi/src/docs.rs`（21 处引用） | 整文件删除 |
| `check()` 求值 | `core/archi/src/api/check.rs` | 三段结构 + 首错 + pair 锚点 |
| `missing_contracts` | `core/archi/src/api/check.rs:224` | 形态①（契约文件缺失）改挂 `docMissing`，本函数只剩形态② |
| `category_of` | `core/archi/src/api/check.rs:45` | 新增 `doc` 分类 |
| `ApiFindingSeverity` | `core/archi/src/drift/report.rs:159` | 二值 → 三值（+ `Error`） |
| `ArchiApiFindingKind` | `core/archi/src/drift/report.rs:206` 起 | 十值 → 十三值（`ALL` / `as_str` / `severity`） |
| `derive_status` | `core/archi/src/drift/mod.rs:338` | 加档位分档 |
| `paths::directory` | `core/archi/src/api/paths.rs:238` | 已是 `archi_models_dir`（E-1 换根，E-2 零改动） |
| `paths::contract_path` | `core/archi/src/api/paths.rs:262` | 保留派生（E-1 后 = `<models>/<Domain>/<FQN>.md`），并升格为 declared 元素的**入口文档载体**（§1.3） |
| `api::enabled` | `core/archi/src/api/mod.rs:325` | 判据改为「有 `doc` 解析出带 fence 文件」（E-1 移交项，见 §11） |
| `gating_findings` | `core/archi/src/api/mod.rs:295` | 纳管子集口径改为「入口文档载体在盘」（declared → `contract_path`、none → `doc`） |
| `element_symbols` | `core/archi/src/api/mod.rs:411` | 不变（`apiEntryMissing` 位置的依据） |
| `search` / `matches_doc` | `core/archi/src/search.rs` | 语料改「在盘的入口文档」（按 api 状态取载体），条目退化路径串 |
| `SearchScope` | `core/archi/src/dto.rs:306` | `Docs` → `Doc`；`parse` 匹配臂 `"docs"` → `"doc"`；非法值文案同步 |
| `ArchiSearchResult` | `core/archi/src/dto.rs:592` | `docs: Vec<DocRef>` → `doc: Vec<String>` |
| `contract.rs` fence 语言标 | `core/archi/src/api/contract.rs:37` / `:40` | `FENCE_OP` / `FENCE_TYPES` 提升可见性，供 `apiDocMissing` 复用 |
| `archi_search` tool schema | `infra/agent/src/sdk/mount.rs:254` | `scope` enum → `["elements", "doc", "all"]` |
| `archi_decisions_dir` / `archi_decisions_rel_dir` | `core/foundation/src/layout/mod.rs:155` / `:205` | **删除**（含 `DECISIONS_DIR_NAME`） |
| 知识文本「决策记录目录」 | `infra/agent/src/archi_knowledge.rs:51` | 该句删除（ADR 定位改由入口文档链接承担） |
| `element-drawer.tsx` `DocsSection` | `packages/desktop/src/views/architecture/components/element-drawer.tsx:288` | 删除 |
| sidecar 计数断言 | `core/archi/tests/golden_test.rs:1468` | 不变（不新增 sidecar） |
