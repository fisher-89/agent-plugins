# 变更 E-1 · 契约资产目录迁移（`api/` → `models/` + `graph/`）

workflow_type: requirement

> 主线：`openspec/explores/module-entry-declaration.md`（todo 原句的第三件：**模块入口文档**）。
> 这条与「模块入口」同源但可独立裁决：入口文档是**文档面**的唯一性，入口文件是**代码面**的唯一性。
> **拆分**：E-1 目录迁移（本文档）；E-2 `doc` 字段替代 docs 能力见
> [`module-doc-field-docs-retirement.md`](module-doc-field-docs-retirement.md)（判据四层阶梯 / 十三值）。
> **阅读顺序**：§E.1–E.3 是裁决前推演（存推理链），§E.4 是原始四问，§E.5 是裁决链（含接缝 A/B），
> §E.6 与 §E.9 是 E-1 的落地形态与定稿。**E-2 相关裁决不在此文档**（§E.5.7 只留指针）。

## E.1 现状：入口文档无处安放

今天的文档面是**关联**（`desktop-archi-module-docs`）而不是**声明**：

```text
element_docs(element) = { explicit(docs 字段) } ∪ { adjacent(path 目录内 *.md 递归) } ∪ { text(ADR 正文含 FQN) }
                        ↓ 去重取最强源，按 源强度 → path 字典序
                   Vec<DocRef{path,title,kind,source}>     ← 没有「哪一份是入口」
```

两条现状让「模块入口文档」这一步必须先期裁决：

- **字段被占用且被收紧**：`repo-archi-model` spec 规定「`docs` 只由包根元素 `App` 显式指向仓库面文档，其余元素留空」。也就是说，除了 `App`，任何模块**都不允许**自己声明文档——与「每个模块指定模块入口文档」直接冲突。
- **`App` 恰好有两份**：`01-app.toml` 的 `docs = ["./README.md", "./AGENT.md"]`。如果「入口文档唯一」，第一条要回答的就是「哪份是入口」（面向人 = README、面向 agent = AGENT，二者都不是显然的答案）。
- **事实上的入口文档已经存在，但被显式排除**：契约 md（`.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md`）的 `## 职能` + `## 不变量` 就是「读这一份就懂这个模块」的文本；而 `desktop-archi-module-docs` 有一条 requirement 明确封死「契约资产不经 docs 三源」（不新增第四源、不新增 `kind = api`）。若入口文档落在契约 md 上，必须**回头改那条 requirement**，不能就地绕过。

---

## E.2 「模块入口文档」四种落法

| 落法 | 形态 | 与既有裁决的冲突 |
|---|---|---|
| a 新字段（如 `doc = "./README.md"`） | 模型里显式唯一 | 与「`docs` 只给 `App`」并列而不冲突；要新增字段面 + `docNotFound` 同族告警 |
| b `docs` 首位即入口 | 零 schema 变更 | 依赖数组顺序语义（隐式契约），且要放开「只给 `App`」的纪律 |
| c 契约 md 承担 | `<FQN>.md` 的 `## 职能` / `## 不变量` 已是入口文档 | **直接撞** `desktop-archi-module-docs`「契约资产不经 docs 三源」那条 requirement，必须改那条 |
| d 邻近约定（`README.md` / `AGENT.md`） | 零字段，自动 | 「唯一」失效（`App` 两份并存即是现实），且无法覆盖「没有 README 的 15 个 crate」 |

另一条隐含动机值得记下：`desktop-archi-module-docs` 的迁移边界写着「模块文档逐步替代 `openspec/specs/`」，且该迁移需要「可断言的结构 + 归档 SpecSync 增量合并语义」；「每模块唯一入口文档」正是这场迁移缺的那块地基——所以 5.8 的选择会决定那块地基是长在契约 md 上还是另起一份模块 doc。

---

## E.3 与迁移边界的关系（决定这块地基长在哪）

`desktop-archi-module-docs` 的迁移边界写着「模块文档逐步替代 `openspec/specs/`」，且该迁移需要「可断言的结构 + 归档 `SpecSync` 增量合并语义」。因此 E 的四种落法不只是「字段放哪」：

| 落法 | 若选它，迁移地基长在 | 需要改的既有裁决 |
|---|---|---|
| a 新字段 | 模型（每模块一份 doc 路径） | 新增字段面 + `docNotFound` 同族告警；`repo-archi-model` 的「`docs` 只给 `App`」改为「`docs` 管关联、`doc` 管入口」 |
| b `docs` 首位 | 模型数组顺序 | 放开「只给 `App`」并给数组顺序以语义（隐式契约，风险高） |
| c 契约 md | 契约资产（`## 职能` / `## 不变量` 已是入口文档） | **改 `desktop-archi-module-docs` 的「契约资产不经 docs 三源」那条** |
| d 邻近约定 | 代码旁（`README.md` / `AGENT.md`） | 「唯一」失效（`App` 两份并存即是现实） |

> **落点**：裁为 **c + a 的复合**——入口文档 = 契约 md（§E.5 首答），模型侧由单值 `doc` 字段声明（E-2）。

---

## E.4 原始四问（已答）

1. 四种落法选一（或组合：入口文档 = 契约 md，其余关联仍走 docs 三源）；
2. `App` 的两份 docs（`README.md` + `AGENT.md`）是否收敛为一份入口文档（面向人 vs 面向 agent 的角色是否要分）；
3. 入口文档是否参与门禁（`docNotFound` 只报不拦 vs 入口文档缺失即欠账）；
4. 与变更 A 的耦合度：`entry` 与入口文档是否同 change。

**应答**：① c（见 §E.5）；② 不收敛（`doc` 语义见 E-2）；③ 参与（判据面见 E-2）；④ 不同 change——A 已归档完成（`2026-11-16-module-entry-field-spec` + 数据回填）。

---

## E.5 裁决链（E-1 相关部分）

| # | 裁决 | 归属 |
|---|---|---|
| 1 | **落法 c**：契约 md = 模块入口文档 | E-1（落点）+ E-2（语义） |
| 2 | 契约资产**全随行**进 `models/`（md + schema + INDEX），`api/` 退役 | **E-1** → §E.5.3 |
| 3 | `INDEX.toml` 落 **`<archi_dir>/graph/`** | **E-1** → §E.5.5 |
| 4 | md/schema 保留 `<Domain>/` 子目录（`domain_of` 与跨域 `$ref` 零改动） | **E-1** → §E.5.6 |
| 5 | App 两份 docs 不收敛 | E-2 |
| 6 | 入口文档只认契约 md；模块不再关联 docs | E-2 |
| 7 | 唯一 `doc` 替代原 docs 职能（可选、无缺省） | E-2 |
| 8 | 判据面：`docMissing` / `docNotFound` / `apiDocMissing` / `apiMissing` 四层阶梯 + 十三值 | E-2 |
| 9 | `archi_search` 文档语料 = 元素的 `doc` 声明 | E-2 |
| 10 | 与变更 A 不同 change | E-1 / E-2 |

### E.5.1 由裁决打开的三个接缝

裁决把契约资产从「独立平面」挪进「模型目录」，于是三条既有边界同时失效（**A / B 归 E-1，C 归 E-2**）：

```text
裁决后的目标形态（方向确定，细节未定）
<root>/.dev-team/architecture/
├── config.json
├── decisions/*.md                     ← ADR
├── models/
│   ├── NN-<域>.toml                   ← 模型（六个，parse 只读一级 *.toml）
│   ├── <Domain>/<FQN>.md              ← 契约 md = 模块入口文档（手写真相）
│   ├── <Domain>/<FQN>.schema.json     ← 生成物（与 md 同目录的既有假设）
│   └── INDEX.toml                     ← 生成物（索引，落点待裁 → §E.5.5）
└── api/                               ← 存废未定
```

| 接缝 | 现状锚点 | 裁决撞上了什么 | 归属 |
|---|---|---|---|
| **A 生成物跟随** | `paths::schema_path` = 与 md 同目录；`$ref` 用 `./<FQN>.schema.json`；`api::enabled` = `<archi_api_dir>` 递归 ≥1 `*.md` | md 搬 models 后，schema / INDEX 跟不跟？不跟则「同目录」假设破，`$ref` 单点函数与 enabled 判据都要重写 | E-1 |
| **B `models/` 双身份** | `repo-archi-model`：「`models/` 恰六个 `NN-<域>.toml`」；`archi_models_dir` 是模型目录派生单点 | 同一目录同时是「模型目录」与「模块文档目录」。模型解析只读一级 `*.toml`（`parse/mod.rs`），技术上 md/schema 混居不破坏解析；坏的是 **spec 的目录树断言** | E-1 |
| **C 入口文档的承载面** | `docs.rs` 硬排除 `is_archi_rel_path`（archi 树整棵不可见）；`desktop-archi-module-docs` 明写「模块文档 MUST NOT 存放于 `<archi_dir>/` 内」+「契约资产不经 docs 三源」 | 入口文档落在 archi 树内 ⇒ 它**不可能**经 docs 三源出线 ⇒ 只能是并列面 `doc` 字段 | **E-2** |

### E.5.2 待澄清（前两问属 E-1，均已裁）

1. **schema / INDEX 是否随行**（接缝 A）→ 全随行，索引移 `graph/`（§E.5.3 / §E.5.5）；
2. **md 在 `models/` 下的形态** → 保留 `<Domain>/` 子目录（§E.5.6）；
3. 「参与门禁」的层级 → **E-2**（进 `status` 纳管，不动 `passed`）；
4. 最小落法（契约正文即入口文档）vs 重落法（新字段）→ **E-2** 裁为「新字段 `doc`，可选无缺省」。

### E.5.3 接缝 A 第一裁：全随行（`api/` 退役）

裁决：`<FQN>.md`（入口文档）+ `<FQN>.schema.json` + `INDEX.toml` 一起进 `models/`，`api/` 目录退役。

由此得到的机械面（已逐一在代码里核对）：

| 点 | 现状 | 变化 |
|---|---|---|
| `foundation::layout::API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` | `api` 段名的唯一持有者 | 退役或改指；`api` 字面量零残留 |
| `api::paths::directory` / `index_path` | 契约目录 / 索引路径单点 | 指 `models/`；索引路径随后改 `graph/`（§E.5.5） |
| `api::paths::{contract_path, schema_path}` | `<api_dir>/<Domain>/<FQN>.*` | 只换 base，`domain_of` 分支不动 |
| `api::enabled` | `<api_dir>` 递归含 ≥1 `*.md` | 改为监测新根——**注意**：若允许杂项 md，判据会被误启用（§E.5.4 / §E.9.3） |
| `api::diff::api_dir_artifacts` | 递归收 `.schema.json` / `INDEX.toml` 判孤儿 | 扫新根；`NN-*.toml` 不匹配后缀，天然零误伤 |
| 模型解析 `parse::parse_dir` | 读 `models/` **一级** `*.toml`（`MODEL_FILE_SUFFIX`），不递归 | **`INDEX.toml` 落根即被当模型文件解析**——`deny_unknown_fields` 会把 `[[entry]]` 判成 unknownField，直接击穿模型 |
| `drift::model_files` / `model_hash` | `models/*.toml` 一级、按扩展名 | `INDEX.toml` 若在根会进 `modelHash` ⇒ 所有 drift 金样的 `modelHash` 换段 |
| `repo-archi-model`「`models/` 恰六个 `NN-<域>.toml`」 | spec 目录树断言 | 必须改写（同目录出现 md / schema / 索引） |
| 字面量 | `.dev-team/architecture/api/...` 共约 28 处（测试常量为主） | 一次 `git mv` + 常量跟随 |

### E.5.4 由 A 暴露的第一个硬钉子：索引与模型解析抢同一个根

`parse::parse_dir` 只读 `models/` 的**一级文件**、按 `.toml` 后缀收面——它不认识「这是模型文件」与「这是索引」的区别：

```text
models/
├── 01-app.toml             ✅ 模型（NN- 前缀只是 spec 约定，代码不查）
├── INDEX.toml              ❌ 被当模型文件解析 → RawDocument.deny_unknown_fields
│                              收 [[entry]] → unknownField 诊断 → 模型 status = errors
└── Infra/
    ├── App.Backend.Infra.ArchiRuntime.md      ✅ 不递归，安全
    └── App.Backend.Infra.ArchiRuntime.schema.json  ✅ 后缀不匹配，安全
```

三个可行处置（互斥）：

| 处置 | 做法 | 代价 |
|---|---|---|
| i 收紧模型收面 | `parse_dir` / `model_files` 的判据从「`.toml` 后缀」改成「`NN-<域>.toml` 文件名模式」 | 代码新增一条文件名判据；好处是把 spec 里「六个文件」从散文变成机械事实 |
| ii 索引进子目录 | `INDEX.toml` 落 `models/.index/INDEX.toml`（`parse_dir` 不递归，天然豁免） | 生成物落点偏离「契约目录根」；orphan 扫描要知道这个子目录 |
| iii 索引换外壳 | `INDEX.toml` → `INDEX.json`（后缀天然不收） | 生成物契约（spec / 测试 / 面板文案）全族改名 |
| **iv 换家（已裁）** | `INDEX.toml` 落 `<archi_dir>/graph/` | 见 §E.5.5 |

### E.5.5 接缝 A 第二裁：索引移出契约目录，进 `architecture/graph`

裁决：`INDEX.toml` 落 **`<archi_dir>/graph/`**（不是 `models/` 根、也不留 `api/`）。根目录冲突被绕开的方式是**换家**，而非收紧模型收面判据：

```text
architecture/
├── config.json
├── decisions/*.md
├── graph/
│   └── INDEX.toml                 ← 契约索引（生成物）
├── models/
│   ├── NN-<域>.toml               ← 模型（parse_dir 收面判据不动）
│   ├── <Domain>/<FQN>.md          ← 入口文档 = 契约正文
│   └── <Domain>/<FQN>.schema.json ← 生成物（与 md 同目录）
└── （api/ 退役）
```

由此新增/变化的机械面：

| 点 | 变化 |
|---|---|
| `foundation::layout` | 新增 `archi_graph_dir` / `archi_graph_rel_dir`（`graph` 段名单点）；`archi_api_dir` 一族退役 |
| `api::paths::index_path` | 从 `directory(root).join(...)` 改为 `archi_graph_dir(root).join(...)`——索引与契约目录**首次分家** |
| `api::diff` orphan 扫描 | 需覆盖两个根：`models/**` 下的 `.schema.json` + `graph/` 下的 `INDEX.toml`；`collect_api_dir` 的单根递归形态要改成「派生两处」 |
| `parse_dir` / `model_files` / `model_hash` | **零改动**（`NN-*.toml` 仍是一级唯一收面；`graph/` 与 `models/<Domain>/` 都不递归） |
| spec「INDEX.toml 落 `<archi_api_dir>` 根下、全局唯一一份」 | 改为 `<archi_graph_dir>`；「契约目录」术语拆成「契约文件（models）」与「索引（graph）」 |
| `repo-archi-model`「`models/` 恰六个 `NN-<域>.toml`」 | 仍需改写（models 里多了 md / schema / `<Domain>/` 子目录），但「六个 toml」的收面判据得以保住 |

**留待后续**：`graph/` 目前只有 `INDEX.toml` 一份住户。它是「契约索引的家」还是「一切图派生资产的家」（将来的模型快照 / 图导出）——前者的派生函数宜名 `archi_index_path`，后者才配叫 `archi_graph_dir`（§E.6.3 第 1 条）。

### E.5.6 接缝 A 第三裁：md 布局沿用 `<Domain>/` 子目录

裁决：`models/<Domain>/<FQN>.md` 与 `<FQN>.schema.json` 保留现状布局，只换 base。于是 **`domain_of` / `domain_dir` / 跨目录 `$ref` 前缀（`../<Domain>/`）零改动**——这是本单元里唯一「搬迁不引发重写」的部分。

累积后的目标形态：

```text
architecture/
├── config.json
├── decisions/*.md
├── graph/INDEX.toml              ← 生成物（§E.5.5）
├── models/
│   ├── 01-app.toml … 06-frontend.toml
│   └── <Domain>/                 ← Domain = 元素父 FQN 末段
│       ├── <FQN>.md              ← 入口文档 = 契约正文（手写真相）
│       └── <FQN>.schema.json     ← 生成物（与 md 同目录，$ref 相对路径不动）
└── （api/ 退役）
```

### E.5.7 接缝 C 的裁决：全部转移到 E-2

接缝 C（入口文档承载面 / docs 三源退役 / 判据面）的四轮裁决**不在本文档**，见
[`module-doc-field-docs-retirement.md`](module-doc-field-docs-retirement.md)：

| 原编号 | 内容 | E-2 落点 |
|---|---|---|
| E.5.7 | 入口文档承载面不可经 docs 三源 | §1.1 |
| E.5.8 | 入口文档单源 + 模块不再关联 docs（Skills 式链接） | §1.2 |
| E.5.9 | 唯一 `doc` 替代 docs 职能（抽屉冗余核对） | §1.2 / §1.4 |
| E.5.10 | `archi_search` 文档检索面 | §1.3 |
| E.6.6 / E.7 / E.8 | 判据面三次迭代 → 四层阶梯 + 十三值 | §2 |

---

## E.6 E-1 的落地形态与代码面

### E.6.1 目标形态

（同 §E.5.6 的目录树，不重复。）

`models/` 的**双身份**是本单元唯一需要 spec 改写来承认的事实：一级 `NN-<域>.toml` 是模型、`<Domain>/` 子树是契约文档与生成物；解析收面判据（一级 / `.toml` 后缀）零改动。

### E.6.2 代码面清单

**新增 / 改变**

| 位置 | 变化 |
|---|---|
| `foundation::layout` | 新增 `archi_graph_dir` / `archi_graph_rel_dir`；`archi_api_dir` / `archi_api_rel_dir` / `API_DIR_NAME` 退役 |
| `api::paths` | `directory()` → `archi_models_dir`；`index_path()` → `archi_graph_dir/INDEX.toml`；`contract_path` / `schema_path` 只换 base |
| `api::enabled` | 判据改为 `<models>` 递归 ≥1 份 `*.md`（杂项 md 风险见 §E.9.3） |
| `api::diff` | orphan 扫描改为两处派生：`models/**` 的 `.schema.json` + `graph/INDEX.toml` |
| 测试常量 | `.dev-team/architecture/api/...` 约 28 处（`api/*_test.rs`、`tests/pilot_test.rs`、`infra/agent/src/sdk/archi_tool_test.rs`、`infra/archi` 侧各一） |

**不动**

- `model.rs` / `parse/toml.rs`（字段面零改动——归 E-2）；
- `api::check`（十值词汇原样——判据归 E-2）；
- `domain_of` / `domain_dir` / 跨域 `$ref` 前缀；
- 生成物内容与渲染器；
- 前端（落点串只经 DTO 出线，`bindings` 无变化）。

**spec 改写面**：`desktop-archi-api`（资产形态 / 索引落点 / 统一目录 rationale）、`repo-archi-model`（目录树 + 布局 + 计数表外的结构断言）、`desktop-archi-contract-panel`（`enabled` 判据串）、`AGENT.md`（`## 模块 API 契约` 的目录串）。

### E.6.3 未定点（E-1 范围）

1. **`graph/` 的语义**：只住 `INDEX.toml`（派生函数宜名 `archi_index_path`），还是「一切图派生资产的家」（才配叫 `archi_graph_dir`）；
2. **契约面 `enabled` 判据**：`models/` 里若允许杂项 md 会误启用；是否收紧为「`<Domain>/<FQN>.md` 形态」——收紧属行为变化，宜与 E-2 判据同批处理。

---

## E.9 E-1 定稿

两篇的边界：

```text
E-1  目录迁移（本文档）             行为零变化的机械单元
     api/ → models/ + graph/；foundation 派生族；paths 三路径；orphan 双根；28 处字面量
     验收：api:export 零 diff、api:check 零差异、模型零诊断、modelHash 不变、enabled 仍 true
        │
        ▼ 硬依赖
E-2  doc 字段替代 docs（另一篇）     能力替换
     字段面 + 十三值判据 + 线面 + 前端 + agent + del desktop-archi-module-docs
     验收：12 / 44（代码落地）→ 12 / 43（试点回填）、status 仍 clean
```

### E.9.1 范围（做什么 / 不做什么）

| 做 | 不做 |
|---|---|
| `git mv .dev-team/architecture/api/INDEX.toml → ../graph/`、`api/<Domain>/*.md|*.schema.json → models/<Domain>/` | 不碰模型字段面（`docs` / `doc`） |
| `foundation::layout`：新增 `archi_graph_dir` / `archi_graph_rel_dir`；`archi_api_dir` / `archi_api_rel_dir` / `API_DIR_NAME` 退役 | 不改任何判据（十值词汇原样） |
| `api::paths`：`directory()` → `archi_models_dir`；`index_path()` → `archi_graph_dir/INDEX.toml`；`contract_path` / `schema_path` 只换 base | 不改 `domain_of` / 跨域 `$ref` 前缀（`<Domain>/` 子目录保留，零改动） |
| `api::enabled`：判据改为 `<models>` 递归 ≥1 份 `*.md` | 不改 `parse_dir` / `model_files` / `model_hash`（`graph/` 与 `models/<Domain>/` 都不在一级收面内） |
| `api::diff`：orphan 扫描改两处派生（`models/**` 的 `.schema.json` + `graph/INDEX.toml`） | 不改生成物内容与渲染器 |
| 约 28 处 `.dev-team/architecture/api/...` 字面量（测试常量为绝对多数） | 不动前端（落点串只经 DTO 出线） |
| spec 落点串：`desktop-archi-api`（资产形态 / 索引落点 / 统一目录 rationale）、`repo-archi-model`（目录树 + 布局）、`desktop-archi-contract-panel`（`enabled` 判据串）、`AGENT.md` | 不动 `desktop-archi-module-docs`（归 E-2） |

### E.9.2 验收（六条机械判据）

1. `pnpm api:export` → **零 diff**（生成物字节不变，只换路径）；
2. `pnpm api:check` → 三列差异全空（`mismatched` / `orphaned` / `missing`）；
3. 模型零诊断（`models/` 里多出 md / schema 与 `<Domain>/` 子目录不影响 `parse_dir`）；
4. drift 报告 `modelHash` **逐字不变**（`graph/` 不在 `models/*.toml` 收面内）；
5. `archi_api_check.enabled === true`（契约面仍启用）；
6. `/architecture` 契约视图与元素抽屉「契约」段三路径可读、零重算。

### E.9.3 两个风险点（实现期须显式核对）

1. **`enabled` 判据的杂项 md 风险**：`<models>` 递归 ≥1 份 `*.md` 会在「有人在 `models/` 里放了笔记」时误判为契约面启用。若 E-1 想收紧，改为「`<Domain>/<FQN>.md` 形态」——但那是**行为变化**，宜留给 E-2 与判据同批处理。
2. **orphan 扫描从单根变两棵**：`collect_api_dir` 的深度上限常量、符号链接不跟随语义、`starts_with(root)` 复核三处都要照搬到 `graph/` 侧，MUST NOT 出现第二份手写递归。
