# 提案: module-entry-docs-semantics

> **变更**: module-entry-docs-semantics（E-1 · 契约资产目录迁移 `api/` → `models/` + `graph/`）
> **日期**: 2026-11-17
> **状态**: draft
> **修订**: 2026-11-18 · 回溯后的事实校准（绑定 2 行 doc 注释 / 金样 1 行 base 例外 / 两处 doc 散文串纳入实现面 / 留痕报告重生成）——四条原断言均与实测互斥，已在病灶三与 AC-1 / AC-3 / AC-4 / AC-5 / AC-8 逐条改写，风险 R-12…R-15 留痕（其中「留痕报告重生成」一条已由第二次修订重划为移交前置）
> **修订（2026-11-18 第二次 · AC-4 边界重划）**: 上一稿把「41 条读数 + `reports/archi/<timestamp>.json` 留痕」写成本 change 的交付判据，实测在结构上不可达成：宿主门禁二进制是**变更前构建**（`paths` 仍指已退役的 `api/` ⇒ `enabled = false` ⇒ 只能写 `apiFindings: []`），任何相位都无权重建 / 重启该进程。故 AC-4（含第七节 ⑤、R-15）改为**本 change 可机械判的两面**：① 源码判据——`api::paths::directory` 的实现体 == `archi_models_dir(root)`、`api::enabled` 的实现体逐字为 `has_markdown(&paths::directory(root), 0)`（判据形态零改动，换根单点传导）；② 离线可推读数——`path` 非空 44 − `api = none` 2 − 契约在案 1 = **41**（`apiEntryMissing 0` + `apiMissing 41`），并配反向判据「有模型、零 md ⇒ 未启用」。41 条留痕报告改记为**宿主重建后的移交前置**；`reports/archi/` 判为**门禁运行时产物**（不入库、不计入交付面、不作判红面），MUST NOT 以 `apiFindings: []` 或空目录充当证据，MUST NOT 为其设置隐藏 ACL / 权限护栏（不入库即可）。其余 10 条验收标准未动。
> **来源**: `openspec/changes/module-entry-docs-semantics/explore.md`（E.1–E.9 全篇；E.5.7 / E.6.6 / E.7 / E.8 的 `doc` 语义部分不在本变更，见「明确不做」）
> **性质**: **行为零变化的机械单元**——契约资产与生成物 `git mv` 换段、`foundation::layout` 派生族增删、三处路径单点换 base、`enabled` 与孤儿扫描换根、`INDEX.toml` 说明段与 `schema` 列两行随动。**判据面读数零变化**（基线 `apiFindings` `0 + 41 = 41`、`status = clean`、`passed = true`；换段后本 change 侧以**源码判据 + 离线推导 `44 − 2 − 1 = 41`** 复核，在线读数属宿主重建后的移交前置）、**模型读数零变化**（53 / 120、`modelHash` 逐字不变）、**契约生成物除 `INDEX.toml` 两行外逐字节不变**。**三处非契约产物的随动必须说准**（三条都不是「零 diff」）：`types/generated/bindings.ts` 恰 2 行 doc 注释（specta 把 `dto.rs` 的 Rust doc 注释抄进生成物 ⇒ **DTO 形状零变化不等于绑定文件零 diff**，字段 / 联合成员 / 顺序零变化）、`tests/corpus/drift/expected/api-mismatch.full.json` 恰 1 行 `occurrences[].file` 换 base（契约夹具暂存根由 `archi_models_rel_dir()` 派生，其余十五份金样逐字不变）、`reports/archi/<timestamp>.json` **不属交付面**（门禁运行时产物：宿主二进制是变更前构建 ⇒ 它只能写 `apiFindings: []`；41 条读数与干净留痕是**宿主重建后的移交前置**，见 AC-4 / R-15）。另有 **2 处产品 `.rs` 的 doc 散文串**换段（`api/extract/mod.rs:5`、`model.rs:46`）——doc-only，含在实现文件表与 AC-5 的判据面内。
> **不含**: `doc` 单值字段与 docs 三源退役、判据面四层阶梯与十三值、`archi_search` 文档语料换面、前端契约视图的文档轴（全部归后续 `doc` 语义单元 E-2）；任何模型字段面改动；任何前端产品代码改动。

---

## 问题

### 现状链路

```text
<root>/.dev-team/architecture/
├── config.json
├── decisions/*.md                    ← ADR
├── models/
│   ├── 01-app.toml … 06-frontend.toml   ← 模型（parse_dir 只读一级 *.toml，不递归）
│   └── （无子目录）
├── api/                              ← 契约资产独立平面
│   ├── INDEX.toml                       ← 生成物（全局唯一一份，落契约目录根）
│   └── Infra/
│       ├── App.Backend.Infra.ArchiRuntime.md          ← 唯一手写真相
│       └── App.Backend.Infra.ArchiRuntime.schema.json ← 生成物（与 md 同目录）
└── （graph/ 不存在）
```

模型、ADR、契约三棵树分居。契约 md 的事实身份已是「读这一份就懂这个模块」（`## 职能` + `## 不变量`），但它躺在**模型资产之外**的第三个平面里。

### 病灶一 · 入口文档的落点被目录结构挡在门外

todo 原句的第三件是「每个模块指定模块入口文档」，总体原则是「模块入口唯一」。裁决（explore §E.5 第 1 条）已定为**契约 md = 模块入口文档**——但若契约资产仍住在 `api/`，那么「模型上声明某个文档是这个模块的入口」就意味着**模型要指到一个与它平行、且被 `docs` 三源整棵封死的目录**。E-2 要落 `doc` 字段，先得让被声明的对象**住在模型资产树内的一个确定位置**；否则「模块入口唯一」只是模型里的一行字符串，落点在树外。E-1 就是这一步：把契约资产搬进 `models/<Domain>/`，使「入口文档」与「模型」同树。

### 病灶二 · 索引与模型解析抢同一个根（搬家的硬钉子）

`INDEX.toml` 不能随契约 md 一起进 `models/` 根，两处代码会立刻判红：

| 点 | 现状判据 | `INDEX.toml` 落 `models/` 根的后果 |
|---|---|---|
| `parse::parse_dir` | `models/` **一级** + `.toml` 后缀（不递归） | 索引被当模型文件解析，`[[entry]]` 在 `RawDocument.deny_unknown_fields` 下产 `unknownField` 诊断 |
| 模型 `status` | 任一结构诊断即停摆 | 整图停摆（画布、`archi_module`、门禁同时失效） |
| `drift::model_files` / `model_hash` | `models/*.toml` 一级、按扩展名，逐文件字节入哈希 | 索引进 `modelHash` ⇒ **全部漂移金样换段** |

三个备选处置（收紧模型收面为 `NN-<域>.toml` / 索引进 `models/.index/` / 索引改 `.json` 外壳）都会动一条**与目录迁移无关**的判据。E-1 取第四个：**索引换家**到 `<archi_dir>/graph/` —— 模型收面判据（一级 + `.toml`）与 `modelHash` 的取面**零改动**，冲突在目录层消失而不是在判据层被绕开。

### 病灶三 · `api` 段是三处派生的唯一持有者，且生成物真的会变

| 点 | 现状 | 迁移后 |
|---|---|---|
| `foundation::layout` | `API_DIR_NAME = "api"` / `archi_api_dir` / `archi_api_rel_dir` | 三者退役；新增 `GRAPH_DIR_NAME` / `archi_graph_dir` / `archi_graph_rel_dir`；契约目录直接取既有 `archi_models_dir` |
| `api::paths::directory` | `archi_api_dir(root)` | `archi_models_dir(root)` |
| `api::paths::index_path` | `directory(root).join("INDEX.toml")` | `archi_graph_dir(root).join("INDEX.toml")`（索引与契约目录**首次分家**） |
| `api::enabled` | `<api>` 递归含 ≥1 份 `*.md` | `<models>` 递归含 ≥1 份 `*.md`（判据形态不变，只换根） |
| `api::diff::api_dir_artifacts` | 单根递归收 `.schema.json` + `INDEX.toml` | 两处派生：`models/**` 的 `.schema.json` + `graph/INDEX.toml` |
| `api::schema::build_index` | 说明段「契约面：目录 `<archi_api_rel_dir>`」 | 说明段改取 `archi_models_rel_dir`（索引与契约分家后，写成索引自身目录即指错地方） |
| `infra/agent/archi_knowledge` | 注入「契约目录：`<archi_api_rel_dir>`」 | 注入「契约资产：`<models_rel>/<Domain>/<FQN>.md`（索引 `<graph_rel>/INDEX.toml`）」 |

**由此得到一个必须说准的事实**：explore §E.9.2 第 1 条要求「`api:export` → 零 diff（生成物字节不变，只换路径）」**不成立**。`INDEX.toml` 的说明段与已声明元素的 `schema` 列都内嵌 repo 相对路径，迁移后**恰有 2 行变化**（本仓现状：说明段目录串 + `App.Backend.Infra.ArchiRuntime` 的 `schema` 行）；`.schema.json` 的 `$ref` 是相对路径（`./<FQN>.schema.json` / `../<Domain>/…`），不嵌目录 ⇒ **零字节变化**；`.i.rs` 零变化。验收因此写「差异恰两行」，不写「零 diff」。

另有三处随动**不属生成器**（不是 `api:export` / `api:check` 的产物），但同样会出线，MUST NOT 被读成误动、也 MUST NOT 被写成「零 diff」：

| 面 | 随动 | 不变量（判红面） |
|---|---|---|
| `packages/desktop/src/types/generated/bindings.ts` | 恰 **2 行 doc 注释**（`:633` `ApiStatus` 的方位串、`:757` `ArchiApiRef.contractPath` 的方位串）——specta 把 Rust doc 注释抄进生成物，故「DTO 形状零改动」**不等于**「绑定文件零 diff」 | 类型 / 联合成员 / 字段名 / 字段序零变化；`bindings.zod.ts` 零 diff；`bindings:export` 幂等（重复导出不再产生第三行 diff） |
| `crates/core/archi/tests/corpus/drift/expected/api-mismatch.full.json` | 恰 **1 行**（`:58` 的 `occurrences[0].file`：`.dev-team/architecture/api/App/App.Mod.md` → `.dev-team/architecture/models/App/App.Mod.md`）——`occurrences[].file` 是 repo 相对路径，契约夹具的暂存根由 `archi_models_rel_dir()` 派生 | 该份的 `modelHash` / `kind` / `category` / `message` / `line` / `specifier` / `typeOnly` 逐字不变；其余十五份 `expected/*.json` 逐字节不变 |
| `<change>/reports/archi/<timestamp>.json`（门禁运行时产物，非生成器产物） | **不属本 change 交付面**：宿主门禁二进制是变更前构建（`paths` 仍指已退役的 `api/` ⇒ `enabled = false`），它每跑一次只能写出 `apiFindings: []`；任何相位都无权重建 / 重启该进程 | 41 条读数（`apiEntryMissing 0` + `apiMissing 41`、`status = clean`、`passed = true`）与干净留痕 SHALL 由**宿主以换段后源码重建后的门禁 / verify 相位**重跑生成（**移交前置**）。`apiFindings: []` 的报告、空目录、该目录的存在性 MUST NOT 充当 AC-4 证据；该目录不入库、不计入交付面、不作判红面（`git status` 出现 `??` 属预期），MUST NOT 为其设置隐藏 ACL / 权限护栏 |

**另有两处产品 `.rs` 的 doc 散文串指向旧落点，它们既不在上面的 51 行字面量扫描面内、也不在三符号的符号扫描面内**：`api/extract/mod.rs:5`（`<archi_dir>/api/<Domain>/<FQN>.md`）与 `model.rs:46`（「接口定义写在架构域 `api` 目录的契约资产里」）。若把「产品 `.rs` 无 `api` 段残留」当成只有符号与引号字面量两层，这两处就会被潜规则豁免——而「旧位置清空」的语义又要求它们不许留。故 AC-5 把判据面拆成四层（符号 / 引号字面量 / 旧方位串 / 段拼接），把这两处 doc 串显式纳入实现文件表；产品代码逻辑零改动（doc-only）。

### 病灶四 · `models/` 的双身份需要 spec 承认

`repo-archi-model` 现写「`models/` 恰六个 `NN-<域>.toml`」。迁移后同一目录会出现 `<Domain>/` 契约子树，而解析之所以仍安全，靠的是「一级 + `.toml` 后缀」这条**既有**收面判据。这条判据今天只活在代码里，spec 的目录树断言反而会成为「搬完就有 spec 说假话」的隐患——必须把双身份与收面判据写成机械事实（判据实现零改动）。

### 现状实测（逐条在案）

| 事实 | 读数 |
|---|---|
| 契约资产 | 1 份手写 md + 1 份 `.schema.json` + 1 份 `INDEX.toml`（`Infra/App.Backend.Infra.ArchiRuntime`） |
| 生产代码触点 | 7 个文件：`foundation/src/layout/mod.rs`（定义）、`api/paths.rs`（`directory`）、`api/schema.rs`（索引说明段）、`dto.rs`（契约投影面的 doc 注释串）、`infra/agent/src/archi_knowledge.rs`（知识注入）、`api/extract/mod.rs:5` 与 `model.rs:46`（doc 散文方位串，doc-only） |
| 测试侧字面量 | `.dev-team/architecture/api…` 共 **51 行 / 18 个文件**（全在 `*_test.rs` 与 `tests/*`） |
| 前端 | 产品代码零命中；3 个测试文件的 4 行 fixture 串（DTO 载荷样例，不参与派生） |
| 文档 | `AGENT.md` / `CLAUDE.md` 该节各 **3 行**需改（第 26 / 27 / 30 行 = 第 ① / ② / ⑤ 条）：`architecture/api` 字面量命中各 2 行（26 / 30），第 27 行「`INDEX.toml`（契约目录根）」无字面量但方位词同样必须换 |
| 判据面基线 | 基线 `apiEntryMissing 0` + `apiMissing 41` = 41，`status = clean`、`passed = true`、`enabled = true`（出自**变更前构建**的宿主门禁）。**换段后本 change 侧只判两面**：① 源码——`api::paths::directory` 体 == `archi_models_dir(root)`、`api::enabled` 体逐字 `has_markdown(&paths::directory(root), 0)`；② 离线可推——`path` 非空 44 − `api = none` 2 − 契约在案 1 = **41**。在线读数与 `reports/archi/` 留痕属**移交前置**（宿主重建后；AC-4 / R-15） |
| 模型读数 | 53 元素 / 120 关系，`modelHash` 为全部漂移金样的锚 |
| 生成物 | `INDEX.toml` 1 份（说明段 + 44 条 entry）、`.schema.json` 1 份、`.i.rs` 1 份（`crates/infra/archi/src/lib.i.rs`） |
| 出线面绑定 | `bindings.ts` 两行含 doc 方位串（`:633` / `:757`），`bindings.zod.ts` 零 doc 串 ⇒ 换段后恰前者 2 行随动、后者零 diff |
| 漂移金样 | `corpus/drift/expected/*.json` 十六份，其中仅 `api-mismatch.full.json` 的 `occurrences[].file` 内嵌契约路径（`:58`）⇒ 换段后恰 1 行差异 |

---

## 提案

### 一 · 资产换段（`git mv`，内容零改）

```text
architecture/
├── config.json
├── decisions/*.md
├── graph/
│   └── INDEX.toml                    ← 契约索引（生成物；由 api/INDEX.toml 搬入后重生成）
├── models/
│   ├── 01-app.toml … 06-frontend.toml    ← 模型（一级，收面判据不动）
│   └── Infra/
│       ├── App.Backend.Infra.ArchiRuntime.md
│       └── App.Backend.Infra.ArchiRuntime.schema.json
└── （api/ 退役）
```

- `<Domain>/` 子目录布局**照搬**（`domain_of` / `domain_dir` / 跨目录 `$ref` 前缀 `../<Domain>/` 零改动）——这是本单元唯一「搬迁不引发重写」的部分。
- `api/` 目录迁移后**必须不存在**（`git mv` 天然搬空，零空目录残留）。**判据面要说准**：孤儿枚举严格两处派生（`models/**` + `graph/`），`api/` 落在枚举面之外 ⇒ 残留**不会**被 `api:check` 判「应删未删」；缺席由目录树读数 + `pilot_test` 负向断言 + 同提交 `git mv` 守（AC-1 / R-14），MUST NOT 在 spec 或验收里写「`api:check` 会报出来」。
- `INDEX.toml` 换家而不是就地改判据：见病灶二。

### 二 · 派生族：增 `graph`、删 `api`、契约目录复用 `archi_models_dir`

| 变更 | 形态 |
|---|---|
| 新增 | `GRAPH_DIR_NAME = "graph"`；`archi_graph_dir(root) -> <archi_dir>/graph`；`archi_graph_rel_dir() -> <archi_rel_dir>/graph` |
| 删除 | `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`（产品 `.rs` 三符号零命中） |
| 不新增 | **契约资产目录没有新函数**，直接取既有 `archi_models_dir` —— MUST NOT 造 `archi_contract_dir` 之类别名（「一个目录两个名字」会让函数名与语义永久错位） |

### 三 · `paths` 三路径：只换 base，索引另起派生点

- `directory(root)` → `archi_models_dir(root)`；`index_path(root)` → `archi_graph_dir(root).join(INDEX_FILE_NAME)`（**MUST NOT** 再由 `directory(root).join(…)` 派生）；`contract_path` / `schema_path` 只换 base。
- `domain_of` / `domain_dir` / `ref_relative`（跨域 `$ref` 前缀）/ `entry_*` 全族 / `EntryForm` / `generated_path`：**零改动**。

### 四 · 两处判据换根（语义零改动）

| 判据 | 迁移后 | 语义守恒点 |
|---|---|---|
| `api::enabled` | `<archi_models_dir>` 递归含 ≥1 份 `*.md` | 「递归 / 深度上限 / 符号链接不跟随」逐字不变；**模型文件不是启用证据**（一级 `NN-<域>.toml` 在场而零 md ⇒ `false`），本仓读数仍 `true` |
| `api::diff` 孤儿扫描 | `models/**` 的 `.schema.json` + `graph/INDEX.toml` | 两处共用同一私有递归，差异只允许「根不同、收面后缀不同」；深度上限 / 符号链接不跟随 / `read_dir` 只此一处三语义照搬，MUST NOT 第二份手写递归（`starts_with(root)` 越根复核驻写盘单点 `infra/archi`，本变更零触及） |

**显式留白（收紧留给后续单元）**：`enabled` 仍是「递归任意 `*.md`」，因此有人在 `models/` 里放一篇笔记即会误判「契约面启用」。收紧为「`<Domain>/<FQN>.md` 形态」是**行为变化**，与判据面的十三值同批处理（E-2），本变更 MUST NOT 顺手改。

### 五 · 生成物与文档串跟随（契约生成物两行 + 绑定两行 doc + 文档三行 + 一处知识文本）

| 对象 | 迁移后 | 差异量 |
|---|---|---|
| `INDEX.toml` | 说明段「契约面：目录 `<archi_models_rel_dir>`」 | 1 行 |
| `INDEX.toml` | `ArchiRuntime` 的 `schema` 列（repo 相对路径含 `models/Infra/`） | 1 行 |
| `.schema.json` | 不变（`$ref` 相对路径与 `$id` 都不嵌目录） | 0 |
| `.i.rs` / `.i.ts` | 不变 | 0 |
| `bindings.ts` | 2 行 doc 注释（specta 抄 `dto.rs` 的 doc）——`:633` `ApiStatus`、`:757` `ArchiApiRef.contractPath` | 2 行（类型面 0） |
| `bindings.zod.ts` | 不变（无 doc 串） | 0 |
| `AGENT.md` / `CLAUDE.md` 该节第 ① / ② / ⑤ 条（现文件第 26 / 27 / 30 行） | ① 手写真相位置换 `models/<Domain>/`；② 的「`INDEX.toml`（契约目录根）」换 `graph/`；⑤ 的索引串换 `graph/INDEX.toml` | 各 **3 行**（两份共 6 行），要点数仍 5、两份副本该节正文逐字相同 |
| `archi_knowledge` 注入段 | 「契约资产：`<models_rel>/<Domain>/<FQN>.md`（索引 `<graph_rel>/INDEX.toml`）」 | 1 段（`archi_knowledge.rs` + 其测试断言） |

### 六 · 字面量跟随（测试侧为主）

`*.dev-team/architecture/api…` 共 51 行 / 18 文件，绝大多数是测试夹具的落点常量与读盘断言 → 一次机械替换为「契约 md/schema 在 `models/<Domain>/`、索引在 `graph/`」；其中 `paths_test` 的「索引层级 = 契约目录本身」与「来源文件内零 `"api"` 字面量」两条断言需按新结构重写（后者结论仍成立，只是被禁的段名不再是 `api`）。**另有两处产品 doc 散文串**（`api/extract/mod.rs:5` 的 `<archi_dir>/api/<Domain>/<FQN>.md`、`model.rs:46` 的「架构域 `api` 目录」）指向旧落点：两者都不含 `.dev-team/architecture/api` 形态、都不是引号字面量，故既不在上面的 51 行字面量扫描面内、也不在 `API_DIR_NAME` 三符号的符号扫描面内——单列在「实现文件」表（doc-only），判据面见 AC-5 的第 ③ 层；否则「产品 `.rs` 无 `api` 段残留」与「保留陈旧 doc」互斥。

### 七 · 落地顺序（中间态允许不绿，提交前必须绿）

```text
① git mv 资产（md + schema → models/Infra/；INDEX.toml → graph/）
② 改布局派生族与七处生产代码（layout / paths / schema / dto / archi_knowledge + 两处 doc-only：extract/mod.rs、model.rs）
③ 改测试侧字面量与断言语料
④ api:export（重生成 INDEX.toml 两行；.schema.json / .i.rs 应零变化）
⑤ api:check 三列空 + **判据面两面复核**（源码：`paths::directory` 体 == `archi_models_dir`、`api::enabled` 体逐字未动；离线可推：`path` 非空 44 − `api = none` 2 − 契约在案 1 = 41 ⇒ `0 / 41`、`status = clean`）+ 十六份金样中十五份逐字不变、`api-mismatch.full.json` 恰一行换 base
```

顺序敏感：paths 已指向新位置而资产未搬时，`api:export` 会在新位置**新建**文件；资产已搬而 paths 未改时，`api:check` 会把旧位置当「应生成未生成」。故①②必须同批落地，④⑤是最终验收。**留痕报告不在本序列内**：宿主门禁是变更前构建（`paths` 仍指 `api/`），它跑出的 `apiFindings: []` 是该构建的正常输出、不构成「判过了」，任何相位也无权重建 / 重启它；`reports/archi/` 判为门禁运行时产物（不入库、不计入交付面、不作判红面），41 条读数与干净留痕由宿主重建后重跑生成（**移交前置**，见 AC-4 / R-15）。

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `workspace-layout-resolution` | MODIFIED ×2 | `archi_dir` 族：删 `api` 三符号、增 `graph` 两函数与常量；rel 族同批换段。`Layout` / `resolve` / `is_archi_rel_path` 零改动 |
| `desktop-archi-api` | MODIFIED ×4 | 资产形态（落位 + `models/` 双身份 + 契约与模型同树互不干扰）、`schema.json` 与 `INDEX.toml` 纯派生（索引换家 + 说明段串 + 索引落模型根的判红面）、生成物入仓与零 diff 门（落点表、孤儿扫描两处派生、换段后字节差异恰两行、**绑定只随 doc 两行随动**的口径、**旧位置缺席不由 `api:check` 判红**的机制文案、两处 doc 散文串（`api/extract/mod.rs` / `model.rs`）随段）、仓库根文档纪律节（第 ① / ② / ⑤ 条三条要点的目录串） |
| `desktop-archi-model` | MODIFIED ×2 | 多文件聚合与声明定位（把「一级 + `.toml` 后缀」写成机械事实 + 索引不得落模型根 + 契约子树混居零干扰）；IPC 命令面契约（`api` 并列字段的定位串与 Scenario 路径） |
| `desktop-archi-api-check` | MODIFIED ×2 | 十条判据（启用条件换根 + 「模型文件不是启用证据」）；验证语料与基线（**读数逐项不变**、目录换根不算新换段、语料夹具随新落点摆放、**金样一行换 base 的显式例外**、**留痕报告改记为移交前置**：本 change 侧只判「`paths::directory` 指 `<archi_models_dir>` + `enabled` 形态零改动 + 离线可推 `44 − 2 − 1 = 41`」，`reports/archi/` 为门禁运行时产物，不以 `[]` / 空目录充证据） |
| `desktop-archi-contract-panel` | MODIFIED ×1 | 检查命令契约（`enabled` 的目录串 + 冷启动态新形态：「目录在、零 md」） |
| `desktop-archi-agent` | MODIFIED ×1 | `archi_module` 的 `api` 字段定位串 + 新增「契约路径零 `api` 段残留」Scenario |
| `desktop-crate-layout` | MODIFIED ×1 | 工具 crate 的段名字面量纪律换段（`models/<Domain>` + `graph`）+ 消费者登记 |
| `desktop-archi-module-docs` | MODIFIED ×1 | 「契约资产不经 docs 三源」的被排路径换段（**语义零改动**：仍是 archi 树整棵不可见，不加新规则） |
| `desktop-archi-file-tree` | MODIFIED ×1 | 枚举面与漂移判定面逐条一致（新增「契约资产与契约索引不进树」的显式事实与 Scenario；判据零改动，仍是扩展名收面）+ Module Contract 路径实例换段 |
| `repo-archi-model` | MODIFIED ×1 | 基线落位与模型文件划分：`models/` 双身份、`graph/INDEX.toml`、契约子树不计入计数 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md` | `git mv` 自 `api/Infra/`（内容逐字不变） |
| `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.schema.json` | `git mv` 同上（字节不变） |
| `.dev-team/architecture/graph/INDEX.toml` | `git mv` 自 `api/INDEX.toml`，随后 `api:export` 重生成（差异恰 2 行） |
| `.dev-team/architecture/api/**` | 删除（迁移后不存在） |
| `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 删 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`；增 `GRAPH_DIR_NAME` / `archi_graph_dir` / `archi_graph_rel_dir` |
| `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` | `directory` → `archi_models_dir`；`index_path` → `archi_graph_dir/INDEX.toml`；文档注释串跟随 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | `enabled` 换监测根（`paths::directory(root)` 已随之改指，确认零第二处判断） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` | `api_dir_artifacts` 两处派生（`models/**` 的 `.schema.json` + `graph/INDEX.toml`） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` | `build_index` 说明段取 `archi_models_rel_dir()` |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | doc 注释定位串换段（**字段面零改动**）：`ApiStatus`（第 83 行「架构域 `api` 目录」）与 `ArchiApiRef::contract_path`（第 361 行 `<archi_dir>/api/<Domain>/<FQN>.md`）改指 `models/<Domain>/` 方位；DTO 字段名 / 顺序 / serde / specta 派生零变化 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` | **doc-only**：模块 doc 第 5 行方位串 `<archi_dir>/api/<Domain>/<FQN>.md` → `<archi_models_dir>/<Domain>/<FQN>.md`；取数器分派、签名归一、`extract` / `extract_element` 行为与两个入口的能力面逐字不变 |
| `packages/desktop/src-tauri/crates/core/archi/src/model.rs` | **doc-only**：第 46 行 `api` 字段 doc 的「架构域 `api` 目录」→「架构域 `models/<Domain>/` 目录」；字段面（八项）、`deny_unknown_fields` 与解析实现零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` | 注入段由「契约目录」改为「契约资产 + 契约索引」两方位 |
| `packages/desktop/src/types/generated/bindings.ts` | 生成物随动（`bindings:export` 重导出）：恰 **2 行 doc 注释**（`:633` `ApiStatus`、`:757` `ArchiApiRef.contractPath`）换段——specta 抄 `dto.rs` 的 Rust doc，故「形状零改动」不等于「文件零 diff」；类型 / 联合成员 / 字段名 / 字段序零变化 |
| `AGENT.md` / `CLAUDE.md` | `## 模块 API 契约` 节**三条要点**的目录串换段：① 第 26 行手写真相 → `models/<Domain>/<FQN>.md`、② 第 27 行 `INDEX.toml` 方位「契约目录根」→ `graph/`、⑤ 第 30 行索引与计数串 → `graph/INDEX.toml`（两份逐字相同，要点数仍 5） |
| `packages/desktop/package.json` | `version` +1 patch（相对落地时 `HEAD` 现值；桌面源码改动，无例外） |

### 测试文件

| 对象 | 判断 |
|---|---|
| `crates/core/foundation/src/layout/mod_test.rs` | 改：`archi_api_dir` / `archi_api_rel_dir` 三例改名为 `graph` 族；新增「契约目录 == `archi_models_dir`（同值零新函数）」与「三符号零命中」两断言 |
| `crates/core/archi/src/api/{paths,mod,diff,schema,contract,render}_test.rs` + `api/emit/*_test.rs` + `api/extract/mod_test.rs` | 改：落点常量与 fixture 换段；`paths_test` 的「索引层级 = 契约目录本身」重写为「索引 = `graph/` 根、与契约目录分家」；「零 `api` 字面量」断言保留并扩到 `graph` / `models` 段拼接零行内命中 |
| `crates/core/archi/src/{drift/mod_test,drift/report_test,dto_test}.rs` | 改：夹具落点换段；`modelHash` 相关用例的断言值不变（收面零改动）；`dto_test` 的 doc 方位串守卫（`include_str!` + `matches` 计数）扩到 `api/extract/mod.rs` 与 `model.rs`——三份产品源文旧方位串零命中，把 AC-5 ③ 的两处 doc 散文串变成机械断言 |
| `crates/core/archi/tests/{pilot_test,golden_test}.rs` | 改：`pilot_test` 的仓库资产只读断言换新路径，并新增**一条负向断言**「`<archi_dir>/api` 目录不存在」（旧位置缺席的机械面——见 AC-1）；`golden_test` 的契约夹具暂存路径换 `models/**`——**十五份 `expected/*.json` 逐字不变，`api-mismatch.full.json` 恰 1 行 `occurrences[].file` 换 base** |
| `crates/infra/agent/src/{archi_knowledge_test,sdk/archi_tool_test}.rs` | 改：知识文本断言改为「契约资产：」+「契约索引：」两条；agent 工具夹具落点换段（四字段响应断言零变化） |
| `crates/tools/archi-api-gen/src/main_test.rs` | 改：19 处落点字面量换段；「清空契约目录 → 元素转 absent、索引目录由 layout 派生重建」用例按新结构重写 |
| `crates/infra/archi/src/{api_artifacts_test,runner_test}.rs` | 改：夹具落点换段（写盘单点实现零改动） |
| `crates/core/archi/src/tree_test.rs` | 改：新增「契约资产与模型文件都不进树」用例（`desktop-archi-file-tree` 的 delta）；既有归属 / 枚举用例零改动 |
| `packages/desktop/src-tauri/src/commands/archi/mod_test.rs` | 改：夹具落点换段；「文档关联集不含契约目录」否定断言改指 `models/<Domain>/**` |
| `packages/desktop/src/{lib/ipc.test.ts,views/architecture/components/architecture-canvas.test.tsx,views/architecture/components/element-drawer.test.tsx}` | **不改**（4 行 DTO 载荷样例，任意字符串即可；改了也不增证据） |
| 验收留痕（`reports/`） | 五份实测：① 目录树读数（`api/` 不存在、`models/` 一级恰 6 toml、`graph/` 恰 1 索引）；② `api:export` 的逐字节 diff（`INDEX.toml` 恰 2 行，`.schema.json` / `.i.rs` 各 0）；③ 判据面**离线可推**读数（`path` 非空 44 − `api = none` 2 − 契约在案 1 = **41** ⇒ `0 / 41`、`status = clean`）+ 源码两面（`paths::directory` 体 == `archi_models_dir`、`api::enabled` 体逐字未动）；④ `archi_drift` 的 `modelHash` 逐字不变 + 金样差异只允许 `api-mismatch.full.json` 的 1 行；⑤ **`reports/archi/<timestamp>.json` 属门禁运行时产物**——不入库、不计入交付面、不作判红面；41 条读数与干净留痕是**宿主重建后的移交前置**，MUST NOT 以 `apiFindings: []` 的报告、空目录或该目录的存在性充当证据 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `crates/core/archi/src/model.rs` 的字段面 / `parse/toml.rs` / `parse/mod.rs` | 字段面与收面判据零改动（本变更只承认既有判据，不改它）；`model.rs` 唯一允许的 diff 是第 46 行 `api` 字段 doc 串（见实现文件表），字段 / 解析改动即误动 |
| `crates/core/archi/src/api/check.rs` 及其语料语义 | 十条判据的词汇、档位、互斥序零改动（只有夹具落点动） |
| `crates/core/archi/src/api/{contract,emit,render}.rs` 的判据与渲染逻辑 | 契约格式、取数、渲染零改动；`api/extract/mod.rs` 只允许模块 doc 方位串（第 5 行）一行 diff，取数与签名归一零改动 |
| `crates/core/archi/src/{docs,search,drift,scan,tree}.rs` | 关联三源、检索语料、漂移判定、归属判据零改动（archi 树不可见性天然覆盖新路径） |
| `crates/infra/archi/src/api_artifacts.rs` | 写盘单点按 `GeneratedFile.path` 的父目录创建，零段名知识 |
| `packages/desktop/src/**` 产品代码 | 落点串只经 DTO 出线；`enabled` 只经 IPC 消费 |
| `packages/desktop/src/types/generated/bindings.zod.ts` 与其余生成面 | 零 doc 串 ⇒ 零 diff；出现 diff 即误动。**`bindings.ts` 不在此列**——它恰随 `dto.rs` 的 doc 换段而变 2 行（见实现文件表），但类型 / 联合成员 / 字段名 / 字段序零变化；把「DTO 形状零改动」读成「绑定文件零 diff」即误判 |
| `.dev-team/architecture/models/0*.toml` | 元素 / 关系 / 字段逐字不变（53 / 120） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 计数口径不含契约目录，零触及 |
| `openspec/specs/**` 除上表 10 个能力 | 出现 diff 即误改 |

---

## 验收标准

1. **AC-1 · 目录树换段干净**：`api/` 不存在；`models/` 一级 `find -maxdepth 1 -name '*.toml'` 读数 = **6**（无 `INDEX.toml`）；`models/Infra/` 有 1 份 md + 1 份 `.schema.json`；`graph/` 恰 1 份 `INDEX.toml`。**旧位置缺席的判据面（说准机制）**：`api:check` 的孤儿枚举严格覆盖 `models/**` 与 `graph/` **两处派生**（本变更有意如此，MUST NOT 为守旧位置而把枚举根放宽到 `<archi_dir>`），故 `api/` 残留对 `api:check` **不可见**——MUST NOT 把「`api:check` 报应删未删」写成本条的判红面（该机制在「索引与契约分家」之后不成立）。旧位置缺席由两条机械面守：① `pilot_test` 的负向断言 `<archi_dir>/api` 目录不存在；② 目录树读数（`ls .dev-team/architecture/api` 零命中）+ 迁移动作本身（`git mv` 与代码改动**同一提交**内完成）。
2. **AC-2 · 生成物差异恰两行**：迁移前后逐字节比对，`INDEX.toml` 差异 = 说明段「契约面：目录」+ 已声明元素的 `schema` 列；全部 `.schema.json` 与 `.i.rs` 差异 = **0**；随后 `api:check` 的 `mismatched` / `orphaned` / `missing` 三列皆空、退出码 0。**MUST NOT 把 AC-2 写成「零 diff」**（explore §E.9.2 第 1 条在此作废，理由见病灶三）。
3. **AC-3 · 模型读数零变化（金样显式一处例外）**：`archi_graph` 的 `diagnostics` 空、元素 **53** / 关系 **120**；`archi_drift` 的 `modelHash` **逐字不变**。十六份 `expected/*.json` 中**十五份逐字节不变**，`api-mismatch.full.json` 恰 **1 行**差异（`:58` 的 `occurrences[0].file`：`.dev-team/architecture/api/App/App.Mod.md` → `.dev-team/architecture/models/App/App.Mod.md`）——`occurrences[].file` 是 repo 相对路径、契约夹具暂存根由 `archi_models_rel_dir()` 派生，故换段必然改这一行；该份的 `modelHash` / `kind` / `category` / `message` / `line` / `specifier` / `typeOnly` 与其余字段逐字不变。**MUST NOT 把 AC-3 写成「十六份金样逐字不变」**（与 `occurrences[].file` 的路径语义互斥）。反向判据：把 `INDEX.toml` 复制到 `models/` 根 ⇒ 模型出 `unknownField` 诊断且 `modelHash` 变化（本条即「索引必须换家」的判红面）。
4. **AC-4 · 契约面换根零语义变化（本 change 判据 = 源码两面 + 离线可推；留痕报告为移交前置）**：
   - **源码面**：`api/paths.rs` 的 `directory` 实现体 == `archi_models_dir(root)`（契约目录与模型目录同址）；`index_path` 体 == `archi_graph_dir(root).join(INDEX_FILE_NAME)`（索引与契约目录分家）；`api/mod.rs` 的 `enabled` 体逐字为 `has_markdown(&paths::directory(root), 0)`——判据形态（递归含 ≥1 份 `*.md` / `MAX_API_WALK_DEPTH` / 符号链接不跟随）零改动，换根经 `directory` 单点传导，MUST NOT 出现第二处判断。
   - **离线可推读数**：`path` 非空元素 **44** − `api = "none"` **2** − 契约在案 **1** = `apiMissing` **41**；配 `apiEntryMissing` **0** ⇒ **0 / 41（总 41）**、`status = clean`，与基线表逐项一致（搬目录既不新增也不消除任何一条）。
   - **反向判据**：观察根下保留 `models/` 一级六个 `NN-<域>.toml` 而 `models/**` 递归零份 `*.md` ⇒ `enabled = false`、`apiFindings = []`（**模型文件不是启用证据**）；在 `models/<Domain>/` 放入一份契约 md ⇒ `enabled = true`、全部 `path` 非空元素重新纳入；两例共用同一判定函数，MUST NOT 在命令层另判一套。由测试相位用例守。
   - **留痕报告是移交前置，不是本 change 的交付物**：`reports/archi/<timestamp>.json` 由门禁进程写出，而宿主门禁二进制是**变更前构建**（`paths` 仍指已退役的 `api/` ⇒ `enabled = false` ⇒ 只能写 `apiFindings: []`），**任何相位都无权重建 / 重启它**。故 `apiFindings` 长度 **41**（`kind` 分桶同基线）、`status = clean` 的报告 SHALL 由宿主以**换段后源码重建后的**门禁 / verify 相位重跑留痕。本 change 侧 MUST NOT 以 `apiFindings: []` 的报告、空目录或该目录的存在性充当本条证据；`reports/archi/` SHALL 视为**门禁运行时产物**——不入库、不计入交付面、不作判红面（`git status` 出现 `??` 属预期），MUST NOT 为其设置隐藏 ACL / 权限护栏（不入库即可）。
5. **AC-5 · 段名纪律（四层判据，各层判什么说准）**：
   - **① 符号面**：产品 `.rs`（`*_test.rs` 除）内 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` 三符号零命中（已删除，不留改指别名）；
   - **② 路径段字面量面**：产品 `.rs`（`*_test.rs` 除）内 `"api"` 作为**架构域路径段**的引号字面量零命中（`grep -rn '"api"'` 零行）。**本层 MUST NOT 覆盖 Rust 模块名**：`crates/core/archi/src/api/` 目录与 `archi::api` 模块路径仍在（契约判定的归属面），被禁的只是文件系统路径段；
   - **③ 旧方位串面**：产品 `.rs` 内 `architecture/api` 与 `<archi_dir>/api/` 两串零命中，**含两处 doc 散文**（`api/extract/mod.rs:5` 换 `<archi_models_dir>/<Domain>/<FQN>.md`、`model.rs:46` 换「架构域 `models/<Domain>/` 目录」）——这两处既不是引号字面量也不含 `architecture/api` 形态，只靠 ①② 会漏，故单列；测试侧判据是 `.dev-team/architecture/api` **夹具落点串**归零（`grep -rn '.dev-team/architecture/api' --include=*.rs` 零行），而 `architecture/api` 作为**否定断言的被检串**允许留在 `contains` / 注释里（本仓换段后残留 8 行，全在断言串），MUST NOT 用「`architecture/api` 全仓零命中」当判据；
   - **④ 段拼接面**：`models` / `graph` 段作为**路径段**的行内拼接零命中（唯一触点 `foundation/src/layout/mod.rs`；测试夹具的 `format!(".dev-team/architecture/models/…")` 串不属产品派生路径，不受本层约束；`crates/infra/store/src/model.rs` 的 `.field("models", …)` 是 Debug 字段标签、非路径段，属既有命中而非本条判据面）；
   - **反向判据**：① / ② / ③ / ④ 任一层被破坏（恢复 `archi_api_dir`、写回 `"api"` 段拼接、复原 `extract/mod.rs:5` 或 `model.rs:46` 的旧串）即判红。
6. **AC-6 · 知识文本换段**：`archi_knowledge()` 输出含 `<models_rel>/<Domain>/<FQN>.md` 与 `<graph_rel>/INDEX.toml` 两个方位、零 `architecture/api` 串；四次词表注入与 4KB 体量上限不变；`archi_knowledge_test` 三处断言（引导语 / 派生串 / 体量）同批更新并全绿。
7. **AC-7 · 两处判据换根不换语义**：`api::diff` 的孤儿枚举在两处各有一例（`models/<Domain>/Ghost.schema.json` → `orphaned`；`graph/INDEX.toml` 内容不符 → `mismatched`），且深度上限 / 符号链接不跟随两语义各有一条用例、`starts_with(root)` 越根复核仍在写盘单点（`infra/archi/src/api_artifacts.rs`，零改动，夹具换段即可）；实现侧 `read_dir` 仍只出现在有界枚举里（无第二份递归）。
8. **AC-8 · 零扩散（绑定面按形状与 doc 分开判）**：`git status --porcelain` 的改动集 ⊆ {本 change 目录、三份资产迁移、`api/` 删除、`layout/mod.rs`、`api/{paths,mod,diff,schema}.rs`、`api/extract/mod.rs`（doc-only 一行）、`model.rs`（doc-only 一行）、`dto.rs`、`archi_knowledge.rs`、上列测试文件、`types/generated/bindings.ts`（**恰 2 行 doc 注释**）、`AGENT.md` / `CLAUDE.md`、`package.json`}；`packages/desktop/src/**` **产品代码**零 diff；`types/generated/bindings.zod.ts` 零 diff；`bindings.ts` 的类型 / 联合成员 / 字段名 / 字段序零变化（diff 恰 2 行、全在 doc 注释内）；`bindings:export` 幂等（重复导出不再产生第三行 diff）。**MUST NOT 把本条写成「`types/generated/**` 零 diff」**——`dto.rs` 的 doc 换段经 specta 抄进绑定，「DTO 形状零变化」与「绑定文件零 diff」不是等价命题。
9. **AC-9 · 版本纪律**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 现值恰 +1 patch（只加一次）；`tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变。
10. **AC-10 · 相关能力零漂移**：`openspec/specs/` 下除本变更 delta 的 10 个能力外零 diff；`desktop-archi-drift` / `desktop-archi-gate` / `desktop-archi-canvas` / `desktop-archi-model`（除本 delta 两条）/ `desktop-archi-api-check` 的判据语义与计数 / `desktop-ipc-type-bindings` 出现 diff 即误改。
11. **AC-11 · 文档纪律节三条要点换段且两份不分化**：迁移后 `grep -n 'architecture/api' AGENT.md CLAUDE.md` 零命中；两文件该节内 `grep '契约目录根'` 零命中；`## 模块 API 契约` 节仍恰 5 条要点，且第 ① 条含 `models/<Domain>/<FQN>.md`、第 ② 条索引方位写 `graph/`、第 ⑤ 条索引与计数串写 `graph/INDEX.toml`；两份该节正文逐字相同（节内 diff 零行、全文 diff 仍恰 2 处且全在节外）。**行数口径**：该节需改 **3 行**（现文件第 26 / 27 / 30 行 = 第 ① / ② / ⑤ 条），MUST NOT 记作 2 行（26 / 27）——第 27 行不含 `architecture/api` 字面量，仅凭字面量归零不足以证明换段完成。反向判据：只改 `AGENT.md` 不改 `CLAUDE.md` ⇒ 该节 diff 出线即判红。

> **不列为 AC 的流程项**：十条判据词汇、互斥求值序、纳管子集、`REPORT_VERSION`、`check` 链四段、`entry_candidates` 三态、`$ref` 相对路径规则——各有既有 spec 落点且本变更零改动，不另设无 spec 落点的断言。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **`models/` 双身份被后人当「可以随便递归」**：收面判据是「一级 + `.toml` 后缀」，一旦有人为图方便改成递归，`<Domain>/` 子目录里的东西立刻进模型面 | spec 把两条判据写成机械事实（`desktop-archi-model`「多文件聚合与声明定位」），并给「契约子树混居零干扰」与「只留契约子树 ⇒ 空模型零诊断」两例；`repo-archi-model` 的落点 Scenario 用 `find -maxdepth 1` 读数钉住 |
| R-2 | **索引被事后挪回 `models/` 根**（「索引跟契约住一起更自然」的直觉） | AC-3 的反向判据（复制到 `models/` 根 ⇒ `unknownField` + `modelHash` 变化）与 `desktop-archi-api`「索引落模型目录根被拒」Scenario 同批落地；`index_path` 是独立派生点（非 `directory(root).join`），结构上也不易被顺手改回 |
| R-3 | **`enabled` 的杂项 md 误启用**：`models/**` 任意 `*.md` 都算启用证据 | 登记为**已知留白**（explore §E.9.3 第 1 条）：本变更保持判据形态不动；收紧为 `<Domain>/<FQN>.md` 属行为变化，与判据面十三值同批处理（E-2）。验收只钉「模型文件不是启用证据」这一半（模型 toml 不构成启用） |
| R-4 | **孤儿扫描从单根变两棵**，容易出现第二份手写递归或漏掉 `graph/` 侧 | 明写「两处派生、三语义逐条同形、MUST NOT 第二份递归」；AC-7 给两处各一例 + 三语义各一条用例；实现侧断言 `read_dir` 仍只在有界枚举内 |
| R-5 | **「零 diff」被照抄**：explore §E.9.2 第 1 条与「生成物字节不变」的说法与实测冲突 | AC-2 显式改为「差异恰两行」并注明该条作废的理由；spec 内同段写「MUST NOT 把该差异记成零 diff」——否则落地者会以为生成器没生效而反复重跑 |
| R-6 | **落地顺序错**（先改 `paths` 后搬资产）导致 `api:export` 在新位置建文件、或在旧位置重复生成 | 提案七节固定五步顺序；`git mv` 与代码改动必须**同一提交**内完成；中间态允许不绿，提交前 AC-2/AC-4 必须绿 |
| R-7 | **知识文本改动牵连三处断言**（引导语 / 派生串 / 4KB 体量），漏改即红但易被当作「体量超限」误判 | `desktop-archi-agent` 的 Module Contract 三行同批登记；AC-6 把三处断言点名；体量读数按新段实测后写死 |
| R-8 | **51 行字面量漏改**（18 个测试文件）——判红面是明确的（落点不存在 / orphan / 断言失败），但工作量大且机械 | 按文件清单逐条推进；`grep -rn 'architecture/api' --include=*.rs` 归零即证；`.dev-team/architecture/api` 在产品代码零命中（已实测），故不涉及产品夹具。**文档面同理且更易漏**：`AGENT.md` / `CLAUDE.md` 该节需改 3 行，其中第 27 行「`INDEX.toml`（契约目录根）」不含 `architecture/api` 字面量，纯 grep 驱动的替换会漏——按「三条要点逐条对拍」而非「字面量归零」把关（AC-11） |
| R-9 | **前端 3 个测试的 4 行旧串**被当成漏改，或反过来被顺手改掉绑定面 | 明写「不改」并给理由（DTO 载荷样例、零派生）；AC-8 的改动集把 `src/**` 产品代码排除在外，`types/generated/` 只放行 `bindings.ts` 的 2 行 doc 注释（见 R-12） |
| R-10 | **`graph/` 语义未定**：本变更按「契约索引的家」命名 `archi_graph_dir`，若将来它变成「一切图派生资产的家」，索引路径需再加一层 | 函数名对两种语义都成立；spec 内明写「本变更内该目录恰住 `INDEX.toml` 一份；将来扩住属独立变更，MUST NOT 在本变更内预建」；若最终定名 `archi_index_path`，改名属独立变更（一次符号替换） |
| R-11 | **与 E-2 的接缝**：`doc` 字段、docs 三源退役、十三值判据不在本变更；本变更只搬位置 | 变更范围的「明确不做」与 explore 未决项表逐条留痕；E-2 若反向要求契约资产换位置，布局族需二次改——故 E-1 的落点选择以 explore §E.5.5 / §E.5.6 的既有裁决为准，不在本变更内重新开裁 |
| R-12 | **「DTO 形状零改动 ⇒ 绑定零 diff」是伪等价**：specta 把 `dto.rs` 的 Rust doc 注释原样抄进 `bindings.ts`，而本变更恰好要改两处 doc 方位串 ⇒ 绑定文件必然出 diff | AC-8 把绑定面拆成「形状 / doc」两半判：类型、联合成员、字段名、字段序零变化 + diff 恰 2 行全在 doc 注释；`bindings.zod.ts` 零 diff；`bindings:export` 幂等。spec 四处（`desktop-archi-api` 的类型绑定再生成、`desktop-archi-model` 与 `desktop-archi-agent` 的 dto.rs 行、`desktop-archi-contract-panel` 的出线形状与生成物行）同批改写，MUST NOT 残留「零再生成」一类措辞 |
| R-13 | **「十六份金样逐字不变」同样是伪命题**：`occurrences[].file` 存的是 repo 相对契约路径，换段必然改 `api-mismatch.full.json` 一行 | AC-3 给显式例外（十五份逐字节不变 + 该份恰 1 行、其余字段逐字不变）；`desktop-archi-api-check` 的语料守护与 Module Contract 同批写明，MUST NOT 让落地者以为「金样动了就是判据漂移」而回退夹具路径 |
| R-14 | **旧位置残留没有 `api:check` 判红面**：孤儿枚举严格两处派生（`models/**` + `graph/`），`api/` 落在枚举面之外 | spec 文案改成「机制诚实版」：清空由同提交 `git mv` + `pilot_test` 负向断言 + 目录树读数守，MUST NOT 写「`api:check` 报应删未删」；若日后确需机械守旧段缺席，应单开变更把枚举根放宽到 `<archi_dir>` 并同步改 spec。`desktop-archi-api` 的「落点迁移后旧位置必须清空」Scenario 与 Module Contract 的 `api/**` 行同步改写 |
| R-15 | **留痕报告会被读成「判过了」**：`reports/archi/<timestamp>.json` 由门禁进程写出，而宿主是变更前构建（`paths` 仍指 `api/`）⇒ 它只能写 `apiFindings: []`，形态上与「契约面未启用」同形 | 口径二分：本 change 侧只判「源码换根（`paths::directory` 指 `<archi_models_dir>`、`enabled` 体逐字未动）+ 离线可推读数（`44 − 2 − 1 = 41`）+ 反向判据（有模型、零 md ⇒ 未启用）」；41 条留痕报告归**移交前置**（宿主以换段后源码重建后由门禁 / verify 相位生成）。`reports/archi/` 判为运行时产物：不入库、不计入交付面、不作判红面（`??` 属预期）；MUST NOT 以 `[]` 或空目录充证据，MUST NOT 用隐藏 ACL / 权限护栏「造」一个干净交付面 |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §E.5.3 / §E.5.5 索引落点（`models/` 根 / `.index/` / `INDEX.json` / `graph/`） | **取 `graph/`**（第四处置）：模型收面判据与 `modelHash` 零改动；另三处置各需动一条与目录迁移无关的判据 |
| §E.5.6 md 布局（保留 `<Domain>/` 子目录） | **采纳**：`domain_of` / `domain_dir` / 跨域 `$ref` 前缀零改动 |
| §E.5.7–§E.5.10 / §E.6.6 / §E.7 / §E.8（`doc` 字段、docs 三源退役、判据十三值） | **不在本变更**：归 E-2；本变更只为它铺「契约资产住在模型树内」这块地 |
| §E.6.3 第 1 条 `graph/` 的语义 | **按「契约索引的家」落地**（`archi_graph_dir`），扩住属独立变更；见 R-10 |
| §E.6.3 第 2 条 `enabled` 是否收紧 | **不收紧**（保持「递归任意 `*.md`」）；收紧属行为变化，与十三值同批；见 R-3 |
| §E.9.2 第 1 条「`api:export` 零 diff」 | **修正为「差异恰两行」**：`INDEX.toml` 说明段与 `schema` 列内嵌 repo 相对路径，迁移必然改动；见病灶三 / R-5 |
| §E.9.2 第 3–6 条（模型零诊断 / `modelHash` 不变 / `enabled` 仍 `true` / 契约视图可读） | **采纳并落 AC-3 / AC-4**：模型两条落 AC-3；`enabled` 与契约视图的读数在换段后由 AC-4 的**源码两面 + 离线可推**承接（`enabled === true` / 三路径可读的在线面属宿主重建后的移交前置） |
| §E.9.3 第 2 条（orphan 扫描两棵的三语义照搬） | **采纳**：AC-7 + `desktop-archi-api` 生成物入仓条的「孤儿扫描覆盖两个根」 |
| §E.9.1「不碰」清单与旧位置清空的判红面 | **按「枚举面之外」落地**：孤儿枚举严格两处派生，`api/` 残留不由 `api:check` 判红；改由 `pilot_test` 负向断言 + 目录树读数 + 同提交 `git mv` 守，spec 文案同步改写（AC-1 / R-14） |
| 留痕报告 `reports/archi/*.json`（`apiFindings`） | **改记为移交前置**：报告由变更前构建的宿主门禁写出（`paths` 仍指 `api/` ⇒ `apiFindings: []`），任何相位无权重建 / 重启它；41 条读数与干净留痕由宿主重建后的门禁 / verify 相位生成。本 change 侧判据改为源码两面（`paths::directory` 指 `<archi_models_dir>` / `enabled` 形态零改动）+ 离线可推 `44 − 2 − 1 = 41` + 反向判据（有模型零 md ⇒ 未启用）；`reports/archi/` 为运行时产物（不入库、不作证据、不作判红面）（AC-4 / R-15） |
| 「DTO 形状零改动」与「十六份金样逐字不变」两条等价 | **均证伪并改写**：`dto.rs` 的 doc 换段经 specta 进 `bindings.ts`（2 行）、`occurrences[].file` 内嵌契约路径使 `api-mismatch.full.json` 差 1 行（AC-8 / AC-3 / R-12 / R-13） |
| §E.2 / §E.3 / §E.4（四种落法与迁移地基） | **已裁于 explore**（落法 c + `doc` 字段），本变更只执行其中 E-1 的目录部分 |
