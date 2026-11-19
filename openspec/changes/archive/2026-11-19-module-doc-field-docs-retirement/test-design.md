# 测试设计: module-doc-field-docs-retirement

> **变更**: module-doc-field-docs-retirement（E-2 · `doc` 单值替代 `docs` 三源 + 十三值判据 + 两支三段首错）
> **基于**: `proposal.md`（AC-1…AC-11、R-1…R-15、实现文件 / 测试文件两张表）、`design.md`（变更清单「新增 / 删除 / 修改文件」表、公共函数 / API 表、类型定义表、D1–D19、事实校准 1–17、OQ-1…OQ-9）
> **范围**: design「新增文件」为空表；「修改文件」中 **Rust 源码十五份**（`crates/core/archi/src/{model.rs, parse/toml.rs, api/paths.rs, api/mod.rs, api/contract.rs, api/check.rs, drift/report.rs, drift/mod.rs, dto.rs, search.rs, lib.rs}`、`crates/core/foundation/src/layout/mod.rs`、`crates/infra/archi/src/runner.rs`、`crates/infra/agent/src/archi_knowledge.rs`、`crates/infra/agent/src/sdk/mount.rs`）与**前端一份**（`packages/desktop/src/views/architecture/components/element-drawer.tsx`）。删除文件 `crates/core/archi/src/docs.rs` + `docs_test.rs` **不纳入被测范围**（语义面由 `api/paths.rs` / `api/mod.rs` / `api/check.rs` / `search.rs` / `dto.rs` 五节承接）。生成物、静态文本资产、模型数据、仓库纪律文本与版本号不进单元测试面（见 `## 不可测试项`）。
> **`doc` 三处进场的测试口径**: ① `ArchiModuleDetail.doc`（`api = none` 出模型原文、`declared` 恒 `null`）；② `ArchiSearchResult.doc: Vec<String>`（在盘载体路径串）；③ **`ArchiElement.doc` 是授权进场字段**（模型字段拷贝，与 `entry` 同型、字段序紧跟 `entry`、缺省 `null`、显式字段不省略）——它是 `paths::carrier_path` / `api::doc_facts` / `api::gating_findings` / `check` 的 ⑥ 与 ③ 的唯一 `doc` 取数路（`dto` 是唯一进 `ApiCheckInput` / `doc_facts` 的投影，事实校准 16）；`to_element` 提 `pub(crate)` 供检索面复用同一投影；`ArchiElementBrief` MUST NOT 带 `doc`。三处是 DTO 字段面口径，生成物侧只反映 IPC 面两处（事实校准 17）。
> **口径提醒**: ① 本设计只钉「测什么、落在哪个测试文件、mock 到什么程度」，不出测试命令与运行脚本；② `api/**` sidecar 文件计数（恰 13 份）是既有机械判据，新用例 MUST 落在既有 sidecar 内（`api/check_test.rs` / `api/mod_test.rs` / `api/paths_test.rs` / `api/contract_test.rs`），MUST NOT 新建 `api/**` 文件；③ 十六份 drift 金样的重录是测试相位动作，本节只钉断言对象与留痕口径。

## 测试框架与布局

- **Rust**：内建 `#[test]`（工作区以 `cargo nextest` 运行）；零外部断言库、零 mock 库、`core/archi` 无 dev-dependency（临时目录以 std 自建 `TempDir` RAII）。断言为 `assert!` / `assert_eq!` / `panic!`，消息用简体中文并携带实际值。
- **Rust 布局**：单元测试为同层兄弟 `*_test.rs`，由 `mod.rs` / `lib.rs` / `api/mod.rs` 末尾的 `#[cfg(test)] mod <name>_test;` 声明；crate 级集成测试在 `crates/core/archi/tests/**`（`golden_test.rs` / `vocabulary_test.rs`）。
- **源码自证**（本仓既有形态）：`include_str!("<file>.rs")` 断言某符号 / 字面量在产品源码内恰好 N 处。本变更的「单点复用」「零第二份」「零兼容壳」类判据全部落这一形态。
- **前端**：`vite-plus`（`vp test`）+ `@testing-library/react`；进程边界只 mock `@tauri-apps/api/core` 的 `invoke`（`vi.mock`），组件与 hooks 真实渲染。
- **组合用例挂靠**：按链路发起方 / 最上层调用方确定入口模块——`agent 工具 → archi::dto` 挂 `dto.rs` 的组合节；`drift::check → api::findings → check` 挂 `drift/mod.rs` 的金样节；`LocalArchiCheck → drift::check → report.passed()` 挂 `runner.rs` 节。不另建集成测试章节。

## 验收范围

> 路径相对项目根；单元格内 `->` 左侧为被测源文件、右侧为其测试文件。AC 编号与 `proposal.md`「验收标准」一一对应。

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 字段面换面干净：`model.rs` / `parse/toml.rs` 只有 `doc: Option<String>`；`deny_unknown_fields` 白名单含 `doc` 不含 `docs`；模型写回 `docs = [...]` 即 `unknownField` 停摆（本仓 `01-app.toml` 已同批删行） | `packages/desktop/src-tauri/crates/core/archi/src/model.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/model_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs` |
| AC-2 | `doc` 适用面结构性成立：`declared` 写 `doc` ⇒ 一条 `apiUnexpected`（`category = entry`、warn）且不参与其它判定；`none` 的 `doc` 指不到 ⇒ 一条 `docNotFound`（`category = doc`、error）；形态校验只在 `api::paths` 单点 | `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `.../src/api/paths_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` -> `.../src/api/check_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` -> `.../src/drift/report_test.rs` |
| AC-3 | 十三值与两支三段：`ALL` 长度 13、`as_str` 逐值对拍、`severity()` 三值（`Error` 恰 `docNotFound` / `apiMissing` / `apiSignatureMismatch`）；`category_of` 七值；元素锚点首错短路、pair 锚点独立且序 `nested → ref → shadowing`；反向判据：两支互斥 | `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` -> `.../src/api/check_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` -> `.../src/drift/report_test.rs` 与 `.../tests/vocabulary_test.rs` |
| AC-4 | `passed` 与 `status` 同批：`passed = errors 空 ∧ structuralErrors 空 ∧ error 档 apiFindings 空`；`derive_status` 在纳管子集上分档；纳管口径 = 元素非空 ∧（declared：载体在盘 / none：`doc` 已声明），`docMissing` 永不纳管；三条不变量 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `.../src/api/mod_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` -> `.../src/drift/report_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` -> `.../src/drift/mod_test.rs`；`packages/desktop/src-tauri/crates/infra/archi/src/runner.rs` -> `.../crates/infra/archi/src/runner_test.rs` |
| AC-5 | 第六段读数与成因：`docMissing 43`（41 改名 + 2 新入面）、`apiMissing 0`、其余十一值 0、`status = clean`、`passed = true`、分母 44；「不是新增 43 条债」的口径留在 spec / 报告 | fixture 级等价读数在 `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `.../src/api/mod_test.rs` 与 `.../src/drift/mod_test.rs`；真实仓库读数与 `reports/` 留痕见 `## 不可测试项` |
| AC-6 | 线面与检索面换面：`ArchiModuleDetail` 无 `docs` / `warnings`、有 `doc`（仅 `api = none` 有值）；`ArchiSearchResult.doc: Vec<String>`；**`ArchiElement` 只在 `entry` 之后新增 `doc: Option<String>`**（模型字段拷贝、与 `entry` 同型、`declared` 恒 `null` 仅限合法模型；`declared` 写了 `doc` 时原文保留供 `apiUnexpected` ⑥ 判定，投影不清零），`ArchiElementBrief` / `ArchiApiRef` 字段面零变化，`to_element` 提 `pub(crate)` 为唯一元素投影；`SearchScope` 词 `elements` / `doc` / `all` 且 `"docs"` 非法；五个类型 + `DocWarning` + `linked_doc_paths` 全仓零命中；`bindings:export` 幂等（IPC 面两处字段进场、清单成员零增删） | `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `.../src/dto_test.rs` 与 `.../tests/vocabulary_test.rs` 与 `.../crates/infra/agent/src/sdk/archi_tool_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/search.rs` -> `.../src/search_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口面，无专属测试文件）；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `.../sdk/mount_test.rs` 与 `.../sdk/tools_test.rs` |
| AC-7 | 启用判据收紧：`api::enabled(root, elements)` = 任一 `path` 非空元素的载体在盘且正文含 `api-op` / `api-types` 语言标；fence 常量单点复用 `contract::{FENCE_OP, FENCE_TYPES}`；`models/notes.md` 与无 fence 的 `<Domain>/notes.md` 都不构成启用证据 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `.../src/api/mod_test.rs`；`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `.../src/api/contract_test.rs` |
| AC-8 | adr 派生与知识文本清理：`archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` 全仓零命中（含 `*_test.rs`）；`archi_knowledge()` 零「决策记录目录」串、含 `doc` 纪律与两段示例、体量上限不变；`decisions/0001-model-granularity.md` 仍在盘 | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` -> `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs`；`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` -> `.../src/archi_knowledge_test.rs` |
| AC-9 | 前端单条入口文档：零 `DocsSection` / 零 `DocRef` 依赖；`api = none` 且 `doc` 非空 ⇒ 呈「入口文档」单条（可经 `FileViewer` 打开）；`declared` ⇒ 只有契约段一条，MUST NOT 两条；空态 testid / 文案随字段面更名 | `packages/desktop/src/views/architecture/components/element-drawer.tsx` -> `packages/desktop/src/views/architecture/components/element-drawer.test.tsx` |
| AC-10 | 金样与语料守护：`api/**` sidecar 仍恰 13 份（新用例落既有文件）；十六份 drift 金样 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并逐份留痕成因；「已知缺口」句就地留痕 | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs`；sidecar 计数在 `.../src/api/check_test.rs` 所属 crate 级断言内（`tests/golden_test.rs`） |
| AC-11 | 零扩散与版本纪律：改动集 ⊆ 白名单；`package.json` +1 patch；`tauri.conf.json` 仍跟随；`src-tauri/Cargo.toml` crate 版本不变；`openspec/specs/` 除 9 个 delta 外零 diff | 无被测源文件（机械核对）——见 `## 不可测试项` |

> **随动测试（无被测源码改动，不建 per-file 章节）**：① **`docs` → `doc` 夹具与清单随动**——`crates/infra/agent/src/sdk/archi_tool_test.rs` 的 ArchiContext 夹具（见 `dto.rs` 组合节）、`crates/infra/agent/src/sdk/tools_test.rs`（见 `mount.rs` 第二组合节）、`crates/core/archi/src/{tree_test,graph_test,diff_test,edit_test}.rs`、`crates/core/archi/src/api/{diff_test,render_test,schema_test}_test.rs`、`crates/tools/archi-api-gen/src/main_test.rs`、`src/commands/archi/mod_test.rs`、`src/bindings/mod_test.rs`、`packages/desktop/src/{app.test.tsx,views/architecture/hooks/use-archi-module.test.ts,views/architecture/architecture-view.test.tsx,views/architecture/components/architecture-canvas.test.tsx,lib/drift-marks.test.ts}`；② **`ArchiElement` 字面量补 `doc`**（事实校准 16 的九处，不补即编译 / `tsc` 红）——`crates/core/archi/src/api/paths_test.rs`、`crates/core/archi/src/api/extract/mod_test.rs`、`crates/core/archi/src/scan/{mod_test,owner_test}.rs`、`crates/core/archi/tests/pilot_test.rs`、`packages/desktop/src/lib/ipc-contract.test.ts`、`packages/desktop/src/views/architecture/components/element-tree.test.tsx`、`packages/desktop/src/views/architecture/hooks/use-archi-graph.test.ts`、`packages/desktop/src/views/changes/change-detail-view.test.tsx`。两批均为机械随动：零新断言或仅否定断言改写，断言口径不新增设计条目。

## 单元测试

### `packages/desktop/src-tauri/crates/core/archi/src/model.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/model_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：`Element` / `Relation` / `Model` 的方法签名零变化，变更是字段面（类型定义表 `Element.doc`：`docs: Vec<String>` → `doc: Option<String>`）。以下测试对象是字段面的机械锚，条目取自 design「修改文件」表 model.rs 行、「类型定义」表与 AC-1。 -->

#### 待测功能

- `Element.doc`（字段面，类型定义表）: `Option<String>` 单值、缺省 `None`、原文不归一（与 `path` / `entry` 同「只裁剪零归一」纪律）；`declared` 的入口文档恒为派生 `contract_path`，本字段只服务 `api = none`
- `Element.api`（开关语义，doc 补述）: 「入口文档载体的开关」——字段面本身不校验 `api` 与 `doc` 的组合（矛盾归判据面 `apiUnexpected`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Element.doc` | 正向 | 缺省构造（`doc: None`）与显式 `Some("./src/auth/README.md")` 逐字保留（前导 `./` 不被剥离）；`Element` 字段面清单恰 11 项（`fqn` / `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata` / `file` / `line`），`docs` 不在清单 | 表驱动（2 组 + 清单 1 组） |
| `Element.doc` | 异常 | `include_str!("model.rs")` 内 `docs` 标识符零命中（射程 = 本文件，design 事实校准 15）；`DocKind` / `DocSource` / `DocRef` / `WarningKind` 零命中；零「显式文档路径数组」类旧 doc 散文 | 单例 + 表驱动（4 符号） |
| `Element.doc` | 边界 | `Some("")`（空串）在模型层保留原样（形态判据不在本层）；超长值（> 1000 字符）保留；含 `\n` / emoji 的路径串保留；`None` 与 `Some("")` 是两个不同状态（`PartialEq` 可区分） | 表驱动（4 组） |
| `Element.api`（开关语义） | 正向 | `Element { api: Declared, doc: None }` 与 `Element { api: None, doc: Some(…) }` 两组合法（模型层不设交叉校验）；`Model::element(fqn)` 查找结果与 `doc` 值无关 | 表驱动（2 组） |
| `Element.api`（开关语义） | 异常 | 反向构造 `api: Declared, doc: Some(…)` 也被接受（矛盾由 `api::check` 报 `apiUnexpected`，模型层 MUST NOT 静默拒绝、MUST NOT 清零） | 单例 |
| `Element.api`（开关语义） | 边界 | `api: None` + `doc: None` 合法（none 无入口文档是合法态）；`path` 空 + `doc` 非空合法（载体派生不依赖 `path`） | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯数据结构） | 零 mock：`Element` 直接字面量构造 | 全部用例 |
| 产品源码文本 | 真实 `include_str!("model.rs")`（既有源码自证形态） | 旧符号零命中断言 |
| 文件系统 / 网络 / 子进程 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：`RawElement` 为私有 `struct`（`deny_unknown_fields` 白名单载体），公共入口 `TomlParser::parse` 签名不变。tomic 字段面换面（`docs: Vec<Spanned<String>>` → `doc: Option<String>`）落「修改文件」表与 AC-1。 -->

#### 待测功能

- `RawElement.doc`（字段面）: `#[serde(default)] Option<String>`；省略 → `None`；`doc = [..]` 由 serde `invalid type` 归 `invalidValue`（MUST NOT 静默取首条）
- `deny_unknown_fields` 白名单同步: 删 `docs`、加 `doc`；旧键 `docs` 与拼错键 → `unknownField`（模型停摆的反向判据）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RawElement.doc` | 正向 | `doc = "src/auth/README.md"` → `Some("src/auth/README.md")`；`doc = "./src/auth/README.md"` → 原文保留 `./`（归一在 `api::paths`，解析面零改写）；`api = "none"` 与 `doc` 同元素并存 | 表驱动（3 组） |
| `RawElement.doc` | 异常 | `doc = ["a.md"]`（数组形态）→ 一条 `invalidValue` 诊断且不产出元素 doc 值；`doc = { path = "a.md" }`（内联表）→ `invalidValue`；`doc = 1`（数字）→ `invalidValue` | 表驱动（3 组） |
| `RawElement.doc` | 边界 | 省略 `doc` → `None`；`doc = ""` → `Some("")`；超长单值（> 1000 字符）与含 `\n` / emoji 转义的字符串逐字保留；`path` 空 + `doc` 非空合法 | 表驱动（4 组） |
| `deny_unknown_fields` 白名单 | 正向 | `doc` 在白名单内（含 `doc` 的源零诊断）；其余九键（`kind` / `gate` / `api` / `entry` / `path` / `summary` / `metadata` 等）行为零变化 | 表驱动（2 组） |
| `deny_unknown_fields` 白名单 | 异常 | **AC-1 反向判据**：`docs = ["./README.md"]` → 一条 `unknownField` 诊断（停摆形态真实可复现，不静默忽略）；拼错键 `dc = "a.md"` / `docss = [..]` → `unknownField` | 表驱动（3 组） |
| `deny_unknown_fields` 白名单 | 边界 | 源文件自证：`include_str!("toml.rs")` 内 `docs` 零命中（白名单与结构体字段同步清除）；`doc` 出现 ≥ 2（结构体字段 + 传递点）；诊断行号指向出错行（既有 `classify` 语义不变） | 表驱动（2 计数 + 行号） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯字符串解析） | 零 mock：TOML 源以字符串直传 `TomlParser::parse` | 全部用例 |
| 产品源码文本 | 真实 `include_str!("toml.rs")` | 白名单零残留守卫 |
| 文件系统 / 网络 / 子进程 | 不适用（解析面零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs`

#### 待测功能

- doc_path(): 新增——`normalize_root`（`\`→`/`、剥前导 `./`、去尾 `/`）+ `is_repo_relative` 通过后 `root.join`；非法形态 → `None`（`join` 越根的安全边界）
- carrier_path(): 新增——**入口文档载体派生单点**：入参是元素投影 `&ArchiElement`（`doc` 为其授权进场字段，事实校准 16）；`Declared → contract_path(root, fqn)`；`None → doc.as_deref().and_then(doc_path)`；判据面 / 纳管面 / 检索面 / 投影面共用，MUST NOT 出现第二份实现
- is_repo_relative(): 修改（可见性）——私有提 `pub(crate)`，成为本 crate 内 repo 相对路径判据的唯一实现；doc 去掉 `docs::normalize_rel` 引用

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `doc_path` | 正向 | `"./src/auth/README.md"` → `root.join("src/auth/README.md")`；`"src\\auth\\README.md"` → 反斜杠归一为 `/`；`"./docs/a.md/"` → 前导 `./` 与尾 `/` 同时剥除；`"a/b/c/d/e.md"` 深层路径 | 表驱动（4 组） |
| `doc_path` | 异常 | `""` / `"/abs/a.md"` / `"~/a.md"` / `"~user/a.md"` / `"C:/a.md"` / `"c:a.md"` / `"a/../b.md"` / `"a//b.md"` / `"a/./b.md"` / `"."` / `"docs/.."` 一律 `None`（越根读的安全边界） | 表驱动（11 组） |
| `doc_path` | 边界 | 超长合法串（> 1000 字符、无越界段）→ `Some`；含中文 / emoji 段 → `Some`（判据只看形态，不清洗）；root 三形态（空 `Path::new("")` / 尾分隔符 `/ws/` / 含空格与中文 `/ws 含 空格/中文根`）结果同源；`doc = "a.md"` 单段 | 表驱动（5 组） |
| `carrier_path` | 正向 | `api = Declared` 的元素 → 与 `contract_path(root, fqn)` 逐字相等（`doc` 有值也同）；`api = None` + `doc = "./src/a/README.md"` → 与 `doc_path` 逐字相等 | 表驱动（2 组） |
| `carrier_path` | 异常 | `api = None` + `doc = None` → `None`；`api = None` + 非法 `doc`（`"/abs/a.md"` / `"../a.md"`）→ `None`；`api = Declared` + 非法 `fqn`（`"App..X"` / `"App/X"`）→ `None` | 表驱动（5 组） |
| `carrier_path` | 边界 | 开关单点：同一元素同时带 `fqn` 与 `doc` 时两 `api` 态取值必不同（declared 取派生、none 取声明）；`path` 空不阻断派生（派生只读 `api` / `doc` / `fqn`）；纯路径零 IO（调用前后 `exists() == false`） | 表驱动（3 组） |
| `is_repo_relative` | 正向 | `"a/b.md"` / `"a"` / `"src/auth/README.md"` → `true`；`pub(crate)` 可见性可达（`search.rs` / `api/mod.rs` 经本函数消费，编译期即证） | 表驱动（3 组） |
| `is_repo_relative` | 异常 | 空串 / 绝对前缀 `/` / `~` / 盘符（大小写 `C:` 与 `c:`）/ 空段 `a//b` / `.` 段 / `..` 段 → `false` | 表驱动（7 组） |
| `is_repo_relative` | 边界 | 与 `docs.rs` 已删的 `normalize_rel` 语义对拍：**对合法集的 pass 结论逐值一致**（原 `docs_test` 里该判据的正例集迁入本文件）；`"a/./b"` 与 `"a/../b"` 在 `doc_path` 先归一再判的差异被显式钉住（`normalize_root` 不消 `.` / `..` ⇒ 仍拒） | 表驱动（2 组 + 1 对照） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯路径运算） | 零 mock：`root: &Path` / `doc: &str` / `fqn` 直传 | 全部用例 |
| `foundation::layout` 派生 | 真实调用（`contract_path` 的 base 来自真实 `archi_models_dir`，替身会自造第二事实源） | `carrier_path` / `contract_path` 对拍 |
| `ArchiElement` 元素夹具 | 真实字面量构造（含 `doc` 字段——投影是唯一取数路，替身会掩盖「`doc` 未进场则载体不可达」） | `carrier_path` 全部用例 |
| 文件系统 | 不替身——本文件新函数零 IO，只用「调用前后 `exists() == false`」做纯推导反向观测 | 零 IO 边界用例 |
| 产品源码文本 | 真实 `include_str!("paths.rs")` | 单点 / 零第二份归一守卫 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`

#### 待测功能

- DocFact（新增类型）: `Absent`（`api = none` 且未声明 `doc`）/ `Missing`（载体已派生或已声明但不在盘）/ `Present { has_api_fence: bool }`（在盘 + 正文是否含 api fence）；入参 `elements` 是 `ArchiElement` 投影（`doc` 为授权进场字段，事实校准 16——`none` 支的载体只能从投影取）；读失败按 `Present { has_api_fence: false }`（探针不得打成 `Err`）
- enabled(): 新增签名 `enabled(root: &Path, elements: &[ArchiElement]) -> bool`——判据 = `doc_facts` 中任一 `Present { has_api_fence: true }`；删除 `has_markdown` / `MARKDOWN_SUFFIX`
- load_contracts(): 快照增 `doc_facts`（同批装配）；契约解析输入源不变（仍只认 `contract_path`）
- findings(): 一次 `load_contracts` → 由 `snapshot.doc_facts` 派生 `enabled` → `api::check`；零第二份装配
- gating_findings(): 口径改 `match element.api`——`Declared` 判载体 `is_file()`、`None` 判 `doc.is_some()`；签名不变
- artifact_check(): 增 `elements_of(root)?` 后调 `enabled(root, &elements)`；模型结构诊断 ⇒ `ApiError::Element`（既有错误面首次兑现）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DocFact`（三态装配） | 正向 | `api = none` 未声明 `doc` → `Absent`；载体不在盘 → `Missing`；在盘含 `api-op` fence → `Present { true }`；在盘零 fence → `Present { false }`；在盘含 `api-types` fence → `Present { true }` | 表驱动（5 组） |
| `DocFact`（三态装配） | 异常 | 载体存在但不可读（非法 UTF-8 字节 / 目录占位同名路径）→ `Present { false }` 且不 `Err`（启用判定是探针）；`declared` 且契约 md 缺席 → `Missing`（不是 `Absent`——载体已派生） | 表驱动（3 组） |
| `DocFact`（三态装配） | 边界 | `elements` 空 → 空表；`path` 空元素不装配（`path` 非空才参与）；同一元素重复出现在表内 → 单键（FQN 为键，BTreeMap 幂等）；**载体来源 = 元素投影 `ArchiElement.doc`**（`api = none` 且模型未声明 `doc` → `Absent`；`api = declared` 即便元素投影带了 `doc` 也按派生 `contract_path` 装配——`doc` 在该支不参与）；fence 判据精度：行首 ``` 的 info string 按空白 / 逗号切 token（` ```json api-op ` 与 ` ```api-op ` 同判；散文里提及 `api-op` 不算；` ```API-OP ` 大小写不算；`~~~` 不认） | 表驱动（5 组 + 5 形态） |
| `enabled` | 正向 | 单份载体在盘含 `api-op` fence → `true`；两份载体其一含 fence → `true`；fence 位于文件第 2 行 / 文末 → `true` | 表驱动（3 组） |
| `enabled` | 异常 | **AC-7 反向判据**：`models/notes.md`（非任何元素载体）→ `false`；`<Domain>/notes.md`（载体位置但无 fence）→ `false`；契约 md 在盘但只含普通代码块 → `false`；`models/` 仅 `*.toml` → `false`；`models/` 缺席 → `false`；旧判据的残留形态（`models/README.md` 任意 md）→ `false` | 表驱动（6 组） |
| `enabled` | 边界 | 冷启动零读盘：零元素 / 零载体 → `false` 且零 `read_to_string`（以不存在的 root 断言不 panic）；`api = none` 元素的 `doc` 含 fence → `true`（OQ-2 字面口径：同一条 `doc_facts` 事实、零 `api` 分支）；签名单点自证：`include_str!("mod.rs")` 内 `pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool {` 恰 1 处，`has_markdown` / `MARKDOWN_SUFFIX` 零命中，`MARKDOWN_SUFFIX` 类第二判据零残留 | 表驱动（2 组 + 源码自证） |
| `load_contracts` | 正向 | 快照 `doc_facts` 覆盖全部 `path` 非空元素；`contracts` / `schemas` / `symbols` 三表与改动前同夹具逐项相等；`api = none` 元素的 `doc` 即使在盘带 fence 也不进 `contracts` | 表驱动（3 组） |
| `load_contracts` | 异常 | 契约非法（缺 `api-op` 块的 heading / 双向 gate 不符）仍 `Err(ApiError::Contract)`——`doc_facts` 装配不吞契约错误；契约文件不可读仍 `Err(ApiError::Io)` | 表驱动（2 组） |
| `load_contracts` | 边界 | `elements` 空 → 三表空 + `doc_facts` 空；同一 root 两次调用 `doc_facts` 逐键相等（零缓存但确定性）；每份载体至多读一次（源码自证 `read_to_string` 在 `doc_facts` 内恰 1 处，`load_contracts` 解析读另有 1 处——双读是本变更显式取舍，OQ-7） | 表驱动（2 组 + 计数） |
| `findings` | 正向 | 有 fence 载体 → 与 `load_contracts` + `api::check` 单次装配的条目逐条相等（顺序也相等）；`enabled = false` → 空 `Vec` 且零契约解析（坏契约不参与） | 表驱动（2 组） |
| `findings` | 异常 | 启用前置不独立判定：源码自证 `findings` 体内零 `enabled(` 调用、`load_contracts(` 恰 1 次、`snapshot.doc_facts` 被读取；「载体存在但不可读」形态 → `enabled = false`（`Present { false }`）而不触发 `ApiError::Io` | 单例 + 1 组 |
| `findings` | 边界 | 空元素表 → 空 `Vec`；单份载体 + 空契约 → `docMissing` 一条（不是 `Err`）；两次调用逐条相等（报告面可重复） | 表驱动（3 组） |
| `gating_findings` | 正向 | `declared` + 载体在盘 → 该元素条目纳管；`none` + `doc` 已声明（即便文件指不到）→ 纳管（声明即承诺）；`none` + `doc` 指不到 ⇒ `docNotFound` 条目在纳管结果内 | 表驱动（3 组） |
| `gating_findings` | 异常 | `declared` + 载体不在盘（`docMissing`）→ 不纳管；`none` + 未声明 `doc`（`docMissing`）→ 不纳管；`element = None` 的条目 → 不纳管；元素不在 `elements` 表内 → 不纳管 | 表驱动（4 组） |
| `gating_findings` | 边界 | `docMissing` 两支都不纳管（同元素同时满足两 `api` 态假设时仍落选）；`declared` + 写了 `doc` + 载体在盘（`apiUnexpected`）→ 纳管；条目顺序与输入顺序一致（过滤不改序） | 表驱动（3 组） |
| `artifact_check` | 正向 | `enabled` 由元素表派生（同 `enabled(root, &elements)` 值）；`mismatched` / `orphaned` / `missing` 三列与 `meta` 字段与改动前同夹具逐项相等 | 表驱动（2 组） |
| `artifact_check` | 异常 | 模型结构诊断非空 → `Err(ApiError::Element)`（不产出半真结果）；无模型（`.dev-team` 缺席）→ `Ok` 且 `enabled = false`（冷启动） | 表驱动（2 组） |
| `artifact_check` | 边界 | 元素表只取一次（源码自证 `elements_of(root)` 恰 1 处）；生成物三列计算零改动（既有断言逐条保留）；`enabled` 字段仍是同一 `bool`（形状零变化） | 表驱动（2 组 + 计数） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（`models/**`、模型 toml、载体 md） | 真实 std `TempDir` RAII + 真实写盘（无替身、无内存 FS）；不可读载体以「非法 UTF-8 字节」注入（既有形态） | 全部用例 |
| `foundation::layout` 派生 | 真实调用（换根 / 派生即被测语义） | 载体路径与 `enabled` 用例 |
| `paths::carrier_path` / `doc_path` | 真实调用（单点复用正是被测语义，替身会让判据面与检索面各造一份事实） | 全部用例 |
| `contract::{FENCE_OP, FENCE_TYPES, info_tokens}` | 真实常量与切分（复用即被测语义） | fence 判据用例 |
| 环境变量 / 时钟 / 网络 / 子进程 | 不适用（本文件零外部依赖） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs`

#### 待测功能

- FENCE_OP / FENCE_TYPES / info_tokens(): 修改（可见性）——提 `pub(crate)`，供 `has_api_fence`（启用判据与 `apiDocMissing`）复用；MUST NOT 抄第二份字面量或第二份切分；解析、双向 gate、`$defs` 规则、落点闸零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `FENCE_OP` / `FENCE_TYPES` | 正向 | 常量值恰 `"api-op"` / `"api-types"`；`api/mod.rs` 经 `contract::{FENCE_OP, FENCE_TYPES}` 消费（`use` 行在案 + `has_api_fence` 体内零字面量） | 表驱动（2 符号） |
| `FENCE_OP` / `FENCE_TYPES` | 异常 | 字面量唯一性：两常量在 `contract.rs` 内各恰 1 处定义，`api/mod.rs` / `api/check.rs` 内 `"api-op"` 与 `"api-types"` 引号字面量零命中 | 表驱动（2 计数 × 2 文件） |
| `FENCE_OP` / `FENCE_TYPES` | 边界 | `pub(crate)` 可达性（跨模块引用编译期即证，用例以 `api/mod.rs` 侧消费断言承载）；零第二常量（无 `FENCE_OP_ALT` / `API_FENCE` 一类同值别名） | 单例 |
| `info_tokens` | 正向 | `"api-op"` / `"json api-op"` / `"api-types,api-op"` / `"  api-op  "` → token 集含目标值；纯空格 / 逗号分隔两种形态同判 | 表驱动（4 组） |
| `info_tokens` | 异常 | `"api-opx"` / `"notapi-op"` / `"API-OP"` → 不含（整 token 匹配，非子串） | 表驱动（3 组） |
| `info_tokens` | 边界 | 空 info `""` → 空 token 集；连续分隔符 `"api-op,,api-types"` → 忽略空段；既有 `contract::parse` 的 fence 识别断言逐条不变（可见性变更零语义漂移） | 表驱动（2 组 + 回归） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯字符串 / 常量） | 零 mock：info string 与契约源字符串直传 | 全部用例 |
| 产品源码文本 | 真实 `include_str!("contract.rs")` / `include_str!("mod.rs")` / `include_str!("check.rs")` | 字面量唯一性守卫 |
| 文件系统 / 网络 / 子进程 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs`

#### 待测功能

- check(): 修改（求值形态）——十三值、`api` 状态两支三段首错短路 + 三条 pair 锚点；`none` 支 `apiUnexpected` → `docMissing` → `docNotFound`；`declared` 支 `docMissing` → `apiDocMissing` → `apiUnexpected`（写了 `doc`）→ `apiEntryMissing` → `apiMissing` → 一致性（`signature → reexport → constructor` 首条即停）；零读内容、零写盘、零 spawn
- category_of(): 修改（七值）——`DocMissing` / `DocNotFound` / `ApiDocMissing` → `"doc"`，其余六值逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check`（两支三段） | 正向 | `none` 支：`doc` 在盘且正文无 fence（无约定属合法）→ 零 finding；`doc` 在盘且含 fence（有示例约定但 `api = none`）→ 零 finding（本支不判 fence）；`declared` 支：载体在盘含 fence + 契约 op / `$defs` 全部兑现 + 入口 `Resolved` + 签名一致 → 零 finding | 表驱动（3 组） |
| `check`（两支三段） | 异常 | 逐值正例（每值一条，序号 = design 求值序）：① `none` + 派生 `contractPath` 在盘 → `apiUnexpected`；② `none` + `doc` 未声明 → `docMissing`；③ `none` + `doc` 指不到盘 → `docNotFound`；④ `declared` + 契约 md 不在盘 → `docMissing`；⑤ `declared` + 契约在盘无 fence → `apiDocMissing`；⑥ `declared` + 写了 `doc` → `apiUnexpected`；⑦ `entry` `Absent` / `Invalid` → `apiEntryMissing`；⑧ 契约声明 op 而代码无实现 → `apiMissing`；⑨ 签名不一致 → `apiSignatureMismatch`；⑩ 透传键集不等 → `apiReexportDrift`；⑪ 契约声明构造器 → `apiConstructorDeclared`；⑫ 父 `$defs` 与子交集 → `apiShadowing`；⑬ 跨模块 `$ref` 目标缺失 → `apiRefMissing`；⑭ 父子跨 crate → `apiNestedCrossCrate`（pair 锚点三条在下面单列） | 表驱动（14 组） |
| `check`（两支三段） | 边界 | 首错短路：同元素同时 `docMissing` + `apiEntryMissing` → 只一条 `docMissing`；`apiDocMissing` + `apiMissing` + `apiSignatureMismatch` → 只一条 `apiDocMissing`；`apiMissing` + `apiReexportDrift` / `apiConstructorDeclared` → 只一条 `apiMissing`；一致性三者在场 → 只一条 `apiSignatureMismatch`；`none` 元素同时「派生契约在盘 + `doc` 未声明」→ 只一条 `apiUnexpected`（①先于②） | 表驱动（5 组） |
| `check`（两支三段） | 边界 | 两支互斥（反向判据）：同一 `elements` 表里一个 `none` 元素与一个 `declared` 元素各报本支判据（互不串档）；`none` 元素零 `apiEntryMissing`（R-4 语义变化）；`path` 空元素零 finding；`apiUnexpected` 双形态各自单向（`none` + 派生在盘 ⇒ 只 ①；`declared` + 写 `doc` ⇒ 只 ②） | 表驱动（5 组） |
| `check`（两支三段） | 边界 | **`doc` 取数路唯一**（事实校准 16）：③ 与 ⑥ 的 `doc` 事实只来自 `input.elements[*].doc`（`ArchiElement` 投影）——夹具 `doc = Some(…)` 时 ③ / ⑥ 各自可产出；同一夹具把投影 `doc` 置 `None` 则两判据均不可达（反向证明它无第三条取数路）；`input.doc_facts` 只供 ⑤ / ④ 的 fence / 缺省事实，MUST NOT 被当成 `doc` 声明的第二来源 | 表驱动（2 组 + 1 对照） |
| `check`（pair 锚点） | 正向 | 三条 pair 判据各自单例：`apiNestedCrossCrate` / `apiRefMissing` / `apiShadowing` 各一条；三条同时在场 → 三条各一条且序为 `nested → ref → shadowing`（跨锚点不比较、不互相短路） | 表驱动（4 组） |
| `check`（pair 锚点） | 异常 | pair 锚点不落 `relation`：pair 判据条目 `element` 归属形态不变、`relation` 槽位零占用（`graph.rs` 的 `DependsOnAncestor` / `unknownReference` 不被触发）；元素锚点满额（每元素一条）时 pair 仍独立产出（不吃元素名额） | 表驱动（2 组） |
| `check`（pair 锚点） | 边界 | pair 锚点零命中（结构全合法）→ 零条目；pair 判据与元素锚点同元素共存时二者互不挤占（一个元素既出 `apiMissing` 又作为 pair 起点出 `apiNestedCrossCrate`）；序稳定性：同一输入两次求值逐条相等 | 表驱动（3 组） |
| `category_of` | 正向 | 六个既有值逐字不变：`apiEntryMissing` / `apiUnexpected` → `entry`、`apiMissing` / `apiUndeclared` → `contract`、`apiSignatureMismatch` → `signature`、`apiReexportDrift` / `apiShadowing` / `apiRefMissing` → `reexport`、`apiNestedCrossCrate` → `nesting`、`apiConstructorDeclared` → `constructor` | 表驱动（逐值） |
| `category_of` | 异常 | 新增三值全部 → `"doc"`（`DocMissing` / `DocNotFound` / `ApiDocMissing`）；无其它值落 `doc` | 表驱动（3 + 反向 10） |
| `category_of` | 边界 | 值域封闭：`ArchiApiFindingKind::ALL` 逐值取 category → 集合恰 7 个（`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor` / `doc`）；返回值全小写、零空白 | 表驱动（1 次全量 + 2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ApiCheckInput` 的 `contracts` / `schemas` / `symbols` / `doc_facts` | 真实值直构（`Contract` 经 `contract::parse` 产生，`symbols` 直构 `ApiSymbol`，JSON 直构）——构造即判据输入，替身会抹掉判据 | 全部用例 |
| 文件系统 | 真实 std `TempDir` + 落空文件（只为 `contract_path(...).is_file()` 与 `carrier_path` 探针；**fence 事实来自 `doc_facts` 入参而非本文件读盘**） | 载体在盘 / 不在盘用例 |
| `api::paths` | 真实调用（派生单点即被测口径） | 载体路径用例 |
| 产品源码文本 | 真实 `include_str!("check.rs")` | 零读盘 / 零第二份判定守卫（如需要） |
| 网络 / 子进程 / 时钟 | 不适用（纯读 `input`） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs`

#### 待测功能

- severity(): 修改（三值）——`ApiUndeclared ⇒ Off`；`DocNotFound` / `ApiMissing` / `ApiSignatureMismatch ⇒ Error`；其余 `_ => Warn`（通配处保留并注释「新增变体落哪一档须显式决定」）
- as_str(): 修改（逐值补词）——`ArchiApiFindingKind` 三新词 `docMissing` / `docNotFound` / `apiDocMissing`；`ApiFindingSeverity` 三值词 `off` / `warn` / `error`
- has_error_tier(): 新增——「纳管子集含任一 `Error` 档」的唯一判据，`derive_status` 与 `passed()` 共用
- ArchiDriftReport::passed(): 新增——`errors` 空 ∧ `model.structural_errors` 空 ∧ `has_error_tier(&api_findings)` 为假；门禁布尔单点

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `severity` | 正向 | `ApiUndeclared → Off`；`DocMissing → Warn`、`ApiDocMissing → Warn`；`DocNotFound` / `ApiMissing` / `ApiSignatureMismatch → Error`（三项恰为 `Error` 全集） | 表驱动（逐值） |
| `severity` | 异常 | 反向白名单：`Error` 档集合恰 `{DocNotFound, ApiMissing, ApiSignatureMismatch}`（其余十值皆非 `Error`）；`Warn` 档集合不含 `ApiUndeclared` | 表驱动（2 组） |
| `severity` | 边界 | `ALL` 逐值恰落一档（13 值 → `Off` 1 + `Error` 3 + `Warn` 9）；`ApiFindingSeverity::ALL` 长度 3 | 表驱动（2 计数） |
| `has_error_tier` | 正向 | 含一条 `Error` 档 → `true`；仅 `Warn` → `false`；仅 `Off` → `false`；空切片 → `false` | 表驱动（4 组） |
| `has_error_tier` | 异常 | `element = None` 的 `Error` 条目也算 `true`（函数只看档位，纳管筛选归 `gating_findings`——分工单点） | 单例 |
| `has_error_tier` | 边界 | 同一切片重复条目 → 仍 `true` 且零 panic；`Warn` + `Error` 混合 → `true`；`apiFindings` 槽位为空数组 → `false` | 表驱动（3 组） |
| `ArchiDriftReport::passed` | 正向 | 零 `errors` / 零 `structuralErrors` / 零 `Error` 档 → `true`（`warnings` 非空、`apiFindings` 全 `Warn` 仍 `true`） | 表驱动（2 组） |
| `ArchiDriftReport::passed` | 异常 | `errors` 非空 → `false`；`structuralErrors` 非空 → `false`；`api_findings` 含 `Error` 档（无其它 errors）→ `false`（旧「`apiFindings` MUST NOT 影响 `passed`」语义反转） | 表驱动（3 组） |
| `ArchiDriftReport::passed` | 边界 | 43 条 `docMissing`（全 `Warn`）在场 → `true`；`api_findings` 空数组 → `true`；`status` 与 `passed` 一致性锚：构造 `status = Errors` 由 `Error` 档单独触发的报告 → `passed() == false` | 表驱动（3 组） |
| `as_str` | 正向 | 十三值词逐值对拍（`docMissing` / `docNotFound` / `apiDocMissing` 三新词在案，旧十词零变化）；`ApiFindingSeverity` 三值词 `off` / `warn` / `error` | 表驱动（13 + 3） |
| `as_str` | 异常 | 词面唯一：13 个 `as_str` 值两两不等；三档词面两两不等；零驼峰外的大小写变体 | 表驱动（2 组） |
| `as_str` | 边界 | `REPORT_VERSION` 仍为 `3`（字段面无增删）；`api_findings` 序列化键名不变（`apiFindings`）；`ArchiDriftReport` 字段清单与序零变化 | 表驱动（3 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯枚举 + 结构体） | 零 mock：`ArchiDriftReport` / `ArchiApiFinding` 字面量直构 | 全部用例 |
| 序列化 | 真实 `serde_json`（报告线形即被测面） | `REPORT_VERSION` / 键名用例 |
| 时钟 / 文件系统 / 网络 | 不适用（本文件零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` -> `packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs`

#### 待测功能

- `ArchiApiFindingKind::ALL` / `as_str()`: 十值 → 十三值；词汇三面（枚举 / `ALL` / 线格式词）逐值对拍
- `ApiFindingSeverity`: 二值 → 三值（`ALL` 长度 2 → 3），三面词对拍

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiApiFindingKind` 词汇面 | 正向 | `ALL.len() == 13`；`ALL` 逐值 `as_str` 恰 13 个互不相同的词；序列化值与 `as_str` 逐值相等（`serde_json::to_value` 与 `as_str` 对拍） | 表驱动（1 次全量） |
| `ArchiApiFindingKind` 词汇面 | 异常 | 旧断言 `ALL.len() == 10` 必须改为 13；三新词在案、旧十词零漂移；`severity()` 在词汇面逐值可求（无 panic / 无未覆盖臂） | 表驱动（2 组） |
| `ArchiApiFindingKind` 词汇面 | 边界 | 词汇组表述校准：`WarningKind` 退役后「四组词汇」→「三组」（`DocKind` / `DocSource` 两组同步删除）；`DocRef` / `DocSource` / `WarningKind` 在本文件零引用 | 单例 |
| `ApiFindingSeverity` 词汇面 | 正向 | `ALL.len() == 3` 且逐值 `as_str` = `off` / `warn` / `error`；序列化值同词 | 表驱动（1 次全量） |
| `ApiFindingSeverity` 词汇面 | 异常 | 旧断言 `ALL.len() == 2` 必须改为 3；无第四档残留（`Info` / `Fatal` 一类零命中） | 单例 |
| `ApiFindingSeverity` 词汇面 | 边界 | `severity()` 的返回值 ∈ `ALL`（13 值逐值验证）；词汇面与 `desktop-archi-api-check` 的「十判据 → 十三判据」表述不混写（本文件零「十判据各自正反例」串） | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：真实枚举 + 真实 `serde_json` | 全部用例 |
| 文件系统 / 网络 | 不适用 | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：`derive_status` 是私有函数（design「公共函数 / API」表未列，私有函数不进表）；公共入口 `drift::check(CheckInput) -> Result<Report, CheckError>` 签名与推导序不变。变更是 `derive_status` 的档位口径（`errors` 分支追加 `has_error_tier(gating)`）与模块 doc 表述，见 design「修改文件」表 drift/mod.rs 行与 D11 / D12。 -->

#### 待测功能

- `derive_status`（私有，档位口径）: `skipped → errors（structural | errors[] | 纳管子集含 Error）→ noChanges → warnings（warnings[] 非空 | 纳管子集非空）→ clean`；`warnings` 分支的「纳管子集非空」保留原样，`errors` 分支新增「error 档判据」
- `check()`（组合装配）: `gating_findings` 入参与 `api_findings` 装配逐字不变，`status` 随 error 档判据抬到 `errors`；`REPORT_VERSION` 仍 3

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `derive_status`（档位口径） | 正向 | 纳管子集含一条 `Error`（`docNotFound`）→ `Errors`；纳管子集仅 `Warn`（`apiDocMissing` / `apiEntryMissing`）→ `Warnings`；纳管子集为空且 `api_findings` 43 条 `docMissing` → `Clean` | 表驱动（3 组） |
| `derive_status`（档位口径） | 异常 | `errors[]` 非空 → `Errors`（不受 api 档位影响）；`structuralErrors` 非空 → `Errors`；`skipped`（无模型）优先于一切 → `Skipped` | 表驱动（3 组） |
| `derive_status`（档位口径） | 边界 | `noChanges`（可判文件 0）优先于 `warnings`；`warnings[]` 非空 + 纳管子集空 → `Warnings`；纳管子集空 + `warnings[]` 空 + `scannedFiles > 0` → `Clean`；不变量：`status == Errors ⟺ passed() == false`（error 档单触发 / warn-only 两态各一条） | 表驱动（4 组） |
| `check()`（组合装配） | 正向 | 端到端：契约在盘含 fence + 实现漂移 → 报告 `status = Errors`、`apiFindings` 含 `apiSignatureMismatch`、`passed() == false`；契约在盘含 fence + 全部兑现 → `status = Clean` / `passed() == true` | 表驱动（2 组） |
| `check()`（组合装配） | 异常 | 模型结构诊断非空 → `check` 的 `Err` 面零变化；契约资产非法 → `CheckError::Config`（既有映射，零第四变体）；零入口文档的存量项目（43 条 `docMissing`）→ `Clean` + `passed() == true` | 表驱动（3 组） |
| `check()`（组合装配） | 边界 | 不变量「`Error` 档判据 ⇒ 该元素必在纳管子集」：逐条 error 档 finding 断言其 `element` 在 `gating_findings` 结果内（结构性安全网）；`REPORT_VERSION == 3`；`gating` 入参仍来自 `api::gating_findings`（源码自证恰 1 处调用） | 表驱动（2 组 + 单例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 / 契约 / 源文件） | 真实 std `TempDir` + 真实写盘；三态与快照面沿用既有夹具 | 全部用例 |
| `api::findings` / `api::gating_findings` / `report::has_error_tier` | 真实调用（判定单源即被测语义，替身会让 `status` 与 `passed` 分叉） | 全部用例 |
| 时钟 / git / 子进程 | 不适用（`CheckInput` 直传模式与文件清单） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs`

> 组合用例挂靠（链路方向：`drift::check`（判定单源）→ `api::findings` → `api::check` → 报告 `apiFindings` / `status` → 金样快照）。入口模块是 `drift::check`，故金样与语料守护用例落在本节。

#### 待测功能

- 金样快照（`tests/corpus/drift/expected/*.json`，十六份）: 首错改写后逐份经 `DESKTOP_GOLDEN_REWRITE=1` 显式重录；`api-mismatch` case 的契约夹具补 fence 使 `apiDocMissing` 不遮蔽既有 `apiSignatureMismatch` 场景
- 语料守护: `api/**` sidecar 仍恰 **13** 份（新判据用例落 `check_test.rs` / `mod_test.rs`），`tests/corpus/api/` 仍零存在

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 金样快照 | 正向 | 十六份 `expected/*.json` 逐份与 `DESKTOP_GOLDEN_REWRITE=1` 重录后的实际报告逐字节相等；`api-mismatch.full.json` 重录后仍含 `apiSignatureMismatch` 且契约夹具带 fence（`apiDocMissing` 未遮蔽）；`empty-model.full.json` 的 `apiFindings` 仍空数组 | 表驱动（16 份） |
| 金样快照 | 异常 | 未置位 `DESKTOP_GOLDEN_REWRITE=1` 时只读比对，任一差异即红且文案携带「以 `DESKTOP_GOLDEN_REWRITE=1` 重写并 review diff」；金样缺 `apiFindings` 键（旧金样未重写）→ 红；首错改写致「同元素多报降为一条」的 case 逐份留痕（哪一条被短路） | 表驱动（2 组 + 留痕清单） |
| 金样快照 | 边界 | 全量 / 增量两模式的同一 case 快照仍一致（`two-modes-consistent` 两态）；报告字段面零变化（`version` / `timestamp` / 槽位序）；十六份计数不变（无新增 / 删除 case） | 表驱动（3 组） |
| 语料守护 | 正向 | `src/api/**` sidecar 恰 13 份（清单逐名对拍）；`tests/corpus/api/` 不存在 | 单例 |
| 语料守护 | 异常 | 新增第 14 份 `api/**` 文件 → 红；新建 `tests/corpus/api/` → 红 | 表驱动（2 组） |
| 语料守护 | 边界 | 十三值判据的正反例全部落在既有 sidecar（`check_test.rs` / `mod_test.rs` / `paths_test.rs` / `contract_test.rs`），零新增 `api/**` 文件；「十判据各自正反例」表述未被扩写成「十三判据各自正反例」（spec 已知缺口句就地留痕） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（case 目录 / 契约夹具 / 源文件） | 真实语料目录与真实写盘；`DESKTOP_GOLDEN_REWRITE` 是测试开关（环境变量，非 mock） | 全部用例 |
| `drift::check` / `api::findings` / `api::check` | 真实调用（全链同源即被测语义） | 全部用例 |
| 时钟 / git / 子进程 | 不适用（报告 `timestamp` 不参与逐字节比对口径，沿用既有比对器） | 全部用例 |

### `packages/desktop/src-tauri/crates/infra/archi/src/runner.rs` -> `packages/desktop/src-tauri/crates/infra/archi/src/runner_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：`runner.rs` 只改一行（`let passed = report.passed();`），`LocalArchiCheck` 的 `ArchiCheckRunner` 实现签名与执行体判定规则零变化（design 事实校准 1 / D16）。本节用例钉「门禁布尔随 error 档判据变化」这一同源语义，以及既有断言的反转。 -->

#### 待测功能

- `ArchiCheckOutcome.passed`（消费面）: 由 `report.errors.is_empty() && report.model.structural_errors.is_empty()` 改为读 `report.passed()`（`error` 档 `apiFindings` 从此可阻断归档）
- `compose_text` / 摘要 / `fault` 面: 本变更零改动（`OQ-5` 已知呈现缺口按现文案断言，不得顺手修）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiCheckOutcome.passed` | 正向 | 契约在盘含 fence 且实现漂移（`apiSignatureMismatch`，`error` 档）→ `passed == false` 且报告 `status == Errors`（原「`api_findings` MUST NOT 影响 `passed`」断言**反转**）；干净仓库 → `passed == true`（既有用例零改动） | 表驱动（2 组） |
| `ArchiCheckOutcome.passed` | 异常 | 仅 `warn` 档 apiFindings（`docMissing` / `apiEntryMissing` / `apiDocMissing`）→ `passed == true` 且 `diagnostics` 不含「N 条违例」计入契约判据（OQ-5 呈现缺口按现文案断言，MUST NOT 顺手修）；`fault = Some(Stopped)` → `passed == false`（既有分支零变化） | 表驱动（2 组） |
| `ArchiCheckOutcome.passed` | 边界 | 零入口文档的存量项目（43 条 `docMissing`）→ `passed == true`、`status == Clean`；报告落盘失败降级 `None` 不影响 `passed`；`runner.rs` 源码自证：`report.passed()` 恰 1 处、`errors.is_empty() && report.model.structural_errors.is_empty()` 零命中（零第二份判定） | 表驱动（2 组 + 源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 / 契约 / 报告目录） | 真实 `TempDir` + 真实写盘；失败面以「目录位置普通文件占位」注入（沿用既有） | 全部用例 |
| `LocalArchiCheck` / `drift::check` / `report.passed()` | 真实调用（同源即被测语义） | 全部用例 |
| 取消标志 / 异步唤醒 | 真实 `CancelFlag` 与既有手写 `Waker`（进程内真实类型，非替身） | `Stopped` 用例 |
| 子进程 / 网络 | 不适用 | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`

#### 待测功能

- SearchScope::parse(): 修改（词面）——`"elements"` / `"doc"` / `"all"`；`"docs"` 为非法值且文案列三合法值（MUST NOT 保留别名）
- module_detail(): 修改（投影）——`doc` = `api = none` 时的模型原文、`declared` 恒 `null`；`docs` / `warnings` 装配删除
- search(): 修改（结果字段）——结果携 `doc: Vec<String>`
- linked_doc_paths(): **删除**——关联集路径集随关联集消失；产品侧零消费者
- to_element(): 修改（可见性）——私有 `fn` 提 `pub(crate)`，成为模型 → 元素投影的唯一单点（检索面经同一投影取 `doc` 事实，MUST NOT 另拼第二份 `ArchiElement`）；`doc` 逐元素拷贝（模型原文、不归一、不清零）
- `ArchiElement.doc`（新增字段，类型定义表）: `Option<String>`，字段序紧跟 `entry`；模型字段拷贝——单值、缺省 `null`、**显式字段不省略**；合法模型下 `declared` 恒 `null`，写了的矛盾留原文供 `check` ⑥ 报 `apiUnexpected`；`ArchiElementBrief` MUST NOT 带 `doc`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SearchScope::parse` | 正向 | `"elements"` / `"doc"` / `"all"` → 三变体逐值 | 表驱动（3 组） |
| `SearchScope::parse` | 异常 | `"docs"` → `Err`，文案含 `elements` / `doc` / `all` 三值；`""` / `"DOC"` / `"Docs"` / `"all "` / `"element"` → `Err` | 表驱动（6 组） |
| `SearchScope::parse` | 边界 | 大小写敏感（`"DOC"` 非法）；错误文案恰列三合法值且无第四词；`"doc"` 与 `"docs"` 语义不可互换（反向判据） | 表驱动（3 组） |
| `module_detail` | 正向 | `api = none` + `doc = "./src/a/README.md"` → `doc == Some("./src/a/README.md")`（模型原文不归一）；`api = declared` → `doc == None` | 表驱动（2 组） |
| `module_detail` | 异常 | 元素不存在 → `Err`（文案含元素数提示，既有面）；模型结构诊断非空 → `Err`；序列化键集零 `docs` / 零 `warnings`（`serde_json` 键集合对拍） | 表驱动（3 组） |
| `module_detail` | 边界 | `api = none` + 未声明 `doc` → `null`（合法空态，不补默认值）；`declared` 即便模型写了 `doc` 也出 `null`（投影不复述模型矛盾）；`doc` 带 `./` 前缀原文保留；字段序 `children → doc → api`；邻域 / `children` / `api` 三投影逐项不变 | 表驱动（5 组） |
| `search`（投影） | 正向 | 结果 `doc: Vec<String>` 逐条为 repo 相对串；`scope = All` 同时出元素与入口文档；`scope = Doc` 只出文档 | 表驱动（3 组） |
| `search`（投影） | 异常 | 空白 query → `Err`；模型诊断非空 → `Err`；`scope = Elements` → `doc` 空数组 | 表驱动（3 组） |
| `search`（投影） | 边界 | 路径字典序（多条目排序）；同 stem 两条目；零命中 → 两数组空；结果序列化键名 `doc`（不是 `docs`） | 表驱动（4 组） |
| `linked_doc_paths`（删除） | 正向 | 替代面到位：`carrier_path` 承担载体派生（检索 / 投影两消费者各 1 处）；`dto.rs` 内 `pub fn linked_doc_paths` 零命中 | 单例 |
| `linked_doc_paths`（删除） | 异常 | `include_str!("dto.rs")` 内 `linked_doc_paths` 零命中；`DocRef` / `DocKind` / `DocSource` / `ArchiWarning` / `WarningKind` 五类型定义与 `ALL` / `as_str` 零残留；`src/commands/archi/mod.rs` 零调用 | 表驱动（5 符号 + 1 调用面） |
| `linked_doc_paths`（删除） | 边界 | 零兼容壳：无 `#[deprecated]` 转发、无 `linked_doc_paths_v2` 一类别名、`lib.rs` 出口零再导出 | 单例 |
| `to_element` / `ArchiElement.doc` | 正向 | `api = none` + 模型 `doc = "./src/a/README.md"` → 投影 `doc == Some("./src/a/README.md")`（原文、不归一）；`api = declared` 且模型零 `doc` 行 → 投影 `doc == None`；`entry` 有值 / 无值时 `doc` 拷贝互不影响 | 表驱动（3 组） |
| `to_element` / `ArchiElement.doc` | 异常 | **授权进场的可达性**：`api = declared` 且模型写了 `doc` → 投影仍保留原文（`doc.is_some()`）——投影不清零，否则 `check` ⑥ 的 `apiUnexpected` 在判据面上不可达（`doc` 无第三条取数路）；`ArchiElementBrief` 零 `doc` 字段（`serde_json` 键集不含 `doc`）；`to_element` 内 `api` 分支零命中（源码自证：投影零 `match element.api`） | 表驱动（2 组 + 源码自证） |
| `to_element` / `ArchiElement.doc` | 边界 | 字段面：`ArchiElement` 字段清单恰 12 项且 `doc` 紧跟 `entry`；`doc = Some("")` / 超长值（> 1000 字符）/ 含 `\n` / emoji 的串逐字保留（形态判据在 `api::paths`，投影零校验）；`doc` 序列化键恒在场（`null` 不省略）；同一模型两次投影逐字段相等 | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 / 契约 md / `doc` 载体） | 真实 `TempDir` + 真实写盘（std RAII） | 全部用例 |
| `parse` / `graph_view` / `search` / `api::paths` | 真实调用（投影零改写即被测语义） | 全部用例 |
| 序列化 | 真实 `serde_json` | 键集 / 键名用例 |
| 网络 / 子进程 / 时钟 | 不适用 | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs`

#### 待测功能

- `SearchScope` 词汇面: 三值保留、值名词 `docs` → `doc`（`Doc` 变体）；DTO 线格式词与枚举三面对拍
- 退役词汇组: `DocKind` / `DocSource` / `WarningKind` 三组词汇对拍删除，词汇组表述由四组校准为三组

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SearchScope` 词汇面 | 正向 | 三值线格式词恰 `elements` / `doc` / `all`（`serde_json` 与 `SearchScope::parse` 双向对拍）；`Doc` 变体与服务端词 `doc` 一致 | 表驱动（3 组） |
| `SearchScope` 词汇面 | 异常 | 旧词 `docs` 在 `dto.rs` 与 `bindings.ts` 词面零命中（MUST NOT 保留别名）；`elements` / `all` 两词零漂移 | 表驱动（2 组） |
| `SearchScope` 词汇面 | 边界 | `SearchScope::parse` 的三合法值集合 == 枚举三值线格式集（双向闭合）；词汇组计数校准后（`ElementKind` / `GateTier` / `SearchScope` 等）三面一致 | 表驱动（2 组） |
| 退役词汇组 | 正向 | `vocabulary_test.rs` 内 `DocKind` / `DocSource` / `WarningKind` 三标识符零引用（import 行与断言同批删） | 单例 |
| 退役词汇组 | 异常 | 产品源码内三类型零定义、零 `as_str`、零 `ALL`；`ArchiModuleDetail.docs` / `.warnings` 零字段 | 表驱动（2 组） |
| 退役词汇组 | 边界 | 「四组词汇」表述校准为「三组」；词汇面与 `lib.rs` 出口清单一致（零悬空出口） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：真实枚举 + 真实 `serde_json` | 全部用例 |
| 文件系统 / 网络 | 不适用 | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`

> 组合用例挂靠（链路方向：agent 工具 `archi_module` / `archi_search` 执行体 → `archi::dto` 投影；测试落点在消费侧 `archi_tool_test.rs`，被测语义是本文件 `module_detail` / `search` / `SearchScope` 的响应形状）。`archi_tool.rs` 执行体本变更零改动（不在 design 修改文件表内），故不单设章节。

#### 待测功能

- `archi_module` 响应: `detail["doc"]` 单值（`api = none` 有值 / `declared` 为 `null`）；`docs` / `warnings` 键退场
- `archi_search` 响应与 scope: `doc` 数组（路径串）、`scope = "doc"` 合法、`scope = "docs"` 非法

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_module` 响应 | 正向 | `api = none` + `doc = "./src/auth/README.md"` → `detail["doc"] == "./src/auth/README.md"`（模型原文、不归一）；`api = declared` → `detail["doc"] == null`；同值同源：`api = none` 时 `detail["doc"] == detail["element"]["doc"]`（同一模型 `doc` 原文，段与元素投影两处不漂移） | 表驱动（3 组） |
| `archi_module` 响应 | 异常 | 响应零 `docs` / 零 `warnings` 键（`as_object().keys()` 对拍）；模型写坏 → `Err` 结构化诊断（既有面零变化）；`api = declared` 且模型写了 `doc` → `detail["doc"] == null` 而 `detail["element"]["doc"]` 保留原文（元素投影是判据 ⑥ 的取数路，MUST NOT 清零） | 表驱动（3 组） |
| `archi_module` 响应 | 边界 | 省略 `element` 的薄概览零 `doc` 字段（`ArchiElementBrief` 五项形状不变）；`detail["element"]["doc"]` 在 `null` 时键仍显式在场（不省略）；夹具 `docs = [...]` 全部改 `doc` 后零解析诊断；`read_file` 不参与（正文用通用读） | 表驱动（4 组） |
| `archi_search` 响应与 scope | 正向 | `scope = "doc"` → `elements` 空数组、`doc` 非空；缺省 `all` → 两数组皆可非空；`doc` 条目为路径串而非四字段对象 | 表驱动（3 组） |
| `archi_search` 响应与 scope | 异常 | `scope = "docs"` → `Err` 且文案列 `elements` / `doc` / `all`；空白 `query` → `Err`（既有面） | 表驱动（2 组） |
| `archi_search` 响应与 scope | 边界 | 语料零正文（入口文档正文关键词不出条目）；`scope = "elements"` → `doc` 空数组；响应键名 `doc`（不是 `docs`） | 表驱动（3 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 / 契约 / 入口文档 / 源码夹具） | 真实 `TempDir` + 真实写盘（沿用既有 `archi_tool_test` 形态） | 全部用例 |
| `archi::dto` / `archi::api` | 真实调用（工具响应经单点投影，替身会让工具面与 IPC 面分叉） | 全部用例 |
| 会话上下文 / 子进程 / 网络 | 不适用（`ArchiContext` 只持 root） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/search.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/search_test.rs`

#### 待测功能

- search(): 修改（语料）——语料 = `path` 非空元素的在盘入口文档载体（`carrier_path(&dto::to_element(element))` + `is_file`，条目经 `api::display` 归一）；`doc` 事实只能来自 `ArchiElement` 投影（`to_element` 是 `pub(crate)` 单点，MUST NOT 另拼第二份 `ArchiElement`、MUST NOT 自建 `doc` 归一）；`SearchOutcome.doc: Vec<String>`；`matches_doc(path, tokens)` 只吃路径 + `file_stem`；`file_stem` 从 `docs.rs` 迁入；排序 = 路径字典序

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `search`（语料） | 正向 | `declared` 元素契约 md 在盘 → 命中（路径含关键词 / 文件名 stem 命中）；`api = none` 元素 `doc` 在盘 → 命中；驼峰分词（`AuthToken` → `auth` / `token`）在路径上生效 | 表驱动（3 组） |
| `search`（语料） | 异常 | 载体不在盘（契约声明了但未写）→ 不入语料（不产出条目）；`path` 空元素 → 不入语料；空白 query → `Err`；`decisions/*.md`（ADR）即使正文含关键词 → 零条目（`text` 源退役） | 表驱动（4 组） |
| `search`（语料） | 边界 | `scope = Elements` → `doc` 空；`scope = Doc` → `element_fqns` 空；`scope = All` 两数组皆可非空；排序 = 路径字典序（多条目）；同 stem 两元素各一条；`file_stem` 迁移后形态不变（无扩展名 / 多点文件名 / 空串）；投影单点自证：`search.rs` 内 `dto::to_element(` 恰 1 处、`ArchiElement {` 字面量零命中（投影不另拼，事实校准 16） | 表驱动（6 组 + 源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（契约 md / `doc` 载体 / ADR / 源文件） | 真实 `TempDir` + 真实写盘 | 全部用例 |
| `carrier_path` / `api::display` | 真实调用（语料集合与判据面必须同一集合） | 全部用例 |
| `ModelGraph` | 真实 `parse` + `ModelGraph::build`（图单点即被测语义） | 全部用例 |
| 网络 / 子进程 / 时钟 | 不适用 | 全部用例 |

### `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` -> `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs`

#### 待测功能

- archi_decisions_dir(): **删除**——唯一非测试消费者 `docs.rs` 已删；产品 `.rs` 零命中，不保留改指别名
- archi_decisions_rel_dir(): **删除**——唯一非测试消费者 `archi_knowledge.rs` 同批删该句
- `DECISIONS_DIR_NAME`（私有常量）: 删除；零悬空引用

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_decisions_dir` / `archi_decisions_rel_dir`（删除） | 正向 | 替代面到位：载体派生改由 `api::paths::carrier_path` 承担（`layout` 侧零替身函数）；`archi_dir` / `archi_models_dir` / `archi_models_rel_dir` / `archi_graph_dir` / `archi_graph_rel_dir` / `archi_config_path` / `is_archi_rel_path` 的返回值与改动前逐字相等 | 表驱动（既有断言全量） |
| `archi_decisions_dir` / `archi_decisions_rel_dir`（删除） | 异常 | `include_str!("mod.rs")` 内三符号（`DECISIONS_DIR_NAME` / `archi_decisions_dir` / `archi_decisions_rel_dir`）零命中；`mod_test.rs` 内三符号零引用（既有全部引用同批删）；全包产品 `.rs`（`*_test.rs` 除外）零命中 | 表驱动（3 符号 × 2 文件） |
| `archi_decisions_dir` / `archi_decisions_rel_dir`（删除） | 边界 | 零兼容壳：`lib.rs` / `mod.rs` 内零 `pub use … archi_decisions_*` 再导出、零 `#[deprecated]` 别名、零 `archi_adr_*` 转发函数；`decisions/0001-model-granularity.md` 文件本体仍在盘（数据面由仓库结构 / static-check 守，用例只断言符号面无访问单点） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯路径推导） | 零 mock：`Path` 字面量与真实 `TempDir` | 全部用例 |
| 产品源码文本 | 真实 `include_str!("mod.rs")` / 既有全包 `.rs` 收集器 | 符号零残留守卫 |
| 文件系统 | 不替身——只对既有 `TempDir` 做存在性反向观测（零 IO 语义） | 纯推导用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`

#### 待测功能

- build(): 修改（格式串与实参）——派生段删「决策记录目录：{}」一段（实参序少一个 `archi_decisions_rel_dir()`），保留「模型目录 / 契约资产 / 契约索引」三处方位；四面词表注入、`join` 单点、`<>` 拒绝、降级 `None` 逐字不变（骨架 md 的字段面 / 示例换面由同一 `build` 输出面断言）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `build` | 正向 | 输出含三处方位标签（「模型目录：」+ `archi_models_rel_dir()`、「契约资产：」+ `/<Domain>/<FQN>.md`、「契约索引：」+ `archi_graph_rel_dir()` + `/INDEX.toml`）；字段面八项含 `doc` 且含「只有 `api = none` 元素可写 / `declared` 的入口文档恒为派生落点」纪律；示例两段形态（`api = "none"` + `doc = …` 一段、`declared` 零 `doc` 行一段） | 表驱动（3 标签 + 2 示例段） |
| `build` | 异常 | 输出零「决策记录目录」串（既有断言删除后以零命中反向钉住）；输出零 `docs` 词；`build("   ")` → `Err("知识骨架为空")`；骨架含 `<>` → `Err` 含「占位符」（降级判据零变化） | 表驱动（4 组） |
| `build` | 边界 | 体量：`knowledge()` 输出 `len() <= 4096` 且相对改动前**下降**（删一句后实测值写入断言，MUST NOT 预填估算）；示例 TOML 经真实 `archi::parse` 零诊断（`declared` 段零 `doc` 行、`none` 段 `doc` 单值可解析）；骨架尾部空白不入文本；`build(SKELETON)` 与 `knowledge()` 产物相等 | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数 + 编译期内嵌骨架） | 零 mock：`build(skeleton: &str)` 字符串直传，`SKELETON` 走 `include_str!` | 全部用例 |
| `foundation::layout` 相对族 | 真实派生（派生串即被测值） | 三方位用例 |
| `archi::dto` 词表常量 | 真实 `*::ALL` 常量（词表只从代码派生） | 四面词表回归 |
| `archi::parse` | 真实解析（示例可解析即被测语义） | 示例 TOML 用例 |
| 文件系统 / 网络 | 不适用（纯函数、零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：`archi_module_definition` / `archi_search_definition` 为私有函数，变更只在描述串（工具名常量、参数形状、READONLY_TOOLS、注入矩阵零变化），见 design「修改文件」表 mount.rs 行与 AC-6。 -->

#### 待测功能

- `archi_module` 工具描述: 删「关联文档（四字段条目）」与「`docNotFound` 警告（只报不拦）」，改为「入口文档单值 `doc`（仅 `api = none` 元素有值；`declared` 走 `api.contractPath`）」
- `archi_search` 工具描述: 「四字段文档条目」→「入口文档路径」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_module` 描述 | 正向 | 描述含「入口文档」与「`api = none`」纪律句；含 `api.contractPath` 指路；`kind` / `gate` 词表注入仍在场且连接形态仍为 `" / "` | 表驱动（3 组） |
| `archi_module` 描述 | 异常 | 描述零「关联文档（四字段条目）」、零「`docNotFound` 警告」、零 `WarningKind` 引用（编译期 + 源码自证 `include_str!("mount.rs")` 内 `WarningKind` 零命中） | 表驱动（3 组） |
| `archi_module` 描述 | 边界 | 工具名常量 / 参数形状（`element` 可选、`required: []`）/ `READONLY_TOOLS` / 注入矩阵逐项零变化 | 表驱动（3 组） |
| `archi_search` 描述 | 正向 | 描述含「入口文档路径」；匹配对象句仍为「元素 FQN / summary / metadata 值 / 文档路径与文件名（不搜文档正文）」 | 表驱动（2 组） |
| `archi_search` 描述 | 异常 | 描述零「四字段文档条目」；零「关联文档」旧串 | 表驱动（2 组） |
| `archi_search` 描述 | 边界 | 描述体量有界（工具描述不就正文扩张）；两工具定义仍在 `mount()` 产出集合内且顺序不变 | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯定义构造） | 零 mock：`mount()` 真实产出 `ToolDefinition` 后字段断言 | 全部用例 |
| `archi::dto` 词表骨架 | 真实 `parse_dsl`（既有形态：真实解析模型字符串建元素表） | 描述词表用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`

#### 待测功能

- `archi_search` 的 `scope` enum 面: `["elements", "docs", "all"]` → `["elements", "doc", "all"]`（工具 schema 单源）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_search` scope enum | 正向 | `parameters["properties"]["scope"]["enum"] == json!(["elements", "doc", "all"])`；三值顺序与 `SearchScope` 词面一致 | 单例 |
| `archi_search` scope enum | 异常 | enum 不含 `"docs"`（长度恰 3、无别名、无第四值）；`query` 仍必填 | 表驱动（2 组） |
| `archi_search` scope enum | 边界 | schema 面其余部分零变化（`element` 可选 / `required: ["query"]`）；工具名清单与源码自证计数（既有 `fn archi_search_definition` 断言）零变化 | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：真实 `mount()` 产出的 `ToolDefinition` | 全部用例 |
| 文件系统 / 网络 | 不适用 | 全部用例 |

### `packages/desktop/src/views/architecture/components/element-drawer.tsx` -> `packages/desktop/src/views/architecture/components/element-drawer.test.tsx`

<!-- design.md 未声明该文件的公共 API 变更：`ElementDrawer` 的 props 签名零变化（`ArchiModuleDetail` 换面属 dto 侧），变更是内部组件 `DocsSection` / `DocRow` 退役 + `EntryDocSection` 新增（design 修改文件表 + D15 / AC-9）。 -->

#### 待测功能

- `EntryDocSection`（新增内部组件）: `api = none` 时恒渲染——`doc` 非空 → 一条「入口文档」行（`FileViewer` + `read_file`）；`doc = null` → 空态文案「无入口文档」+ `data-testid="archi-entry-doc-empty"`；`api = declared` 时不渲染
- 段序与「不混装」: 概览 → 契约 `ContractSection` → 入口文档 `EntryDocSection` → 豁免；`declared` 只有契约段一条 `ContractPathRow("契约")`，MUST NOT 两条入口文档

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `EntryDocSection` | 正向 | `api = none` + `doc = "src/auth/README.md"` → 段标题「入口文档」+ 一条行；点击触发 `read_file` 恰 1 次，载荷 `{ path: "/ws/src/auth/README.md", offset: null, limit: null }`（root 前缀由 `FileViewer` 拼成绝对路径，沿用既有 `read_file` 断言形态） | 表驱动（2 组） |
| `EntryDocSection` | 异常 | `read_file` 失败 → 行内错误呈现、段不消失（`FileViewer` 既有面）；`api = declared` → 零入口文档行（即便 payload 里带 `doc` 也 MUST NOT 渲染） | 表驱动（2 组） |
| `EntryDocSection` | 边界 | `api = none` + `doc = null` → 空态文案「无入口文档」+ testid `archi-entry-doc-empty`；`api = declared` 且 `doc = null` → 该段整段不渲染（空态也不渲染）；`root = null` → 段不渲染、零 `read_file` | 表驱动（3 组） |
| 段序与不混装 | 正向 | 段序：概览 h2 → 契约段 h3（`ContractPathRow("契约")`）→ 入口文档段 h3 → 豁免；`declared` 元素 `ContractPathRow("契约")` 恰 1 条 | 表驱动（2 组） |
| 段序与不混装 | 异常 | `declared` 元素零第二条入口文档（「入口文档」段标题零出现）；`docs` / `DocRef` 夹具删除后组件零类型依赖（`tsc` + 源码自证 `DocsSection` / `DocRow` 零命中） | 表驱动（2 组） |
| 段序与不混装 | 边界 | 空态 testid 更名后旧 `archi-docs-empty` 零命中；`api = none` 空态与「段不渲染」可区分（空态有 testid、declared 无节点）；`invoke` 命令面零新增（`archi_module` + 按需 `read_file`） | 表驱动（3 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | `vi.mock`（进程边界：IPC 出线）；按命令名分发 `archi_module` / `read_file` / `archi_set_gate` | 全部用例 |
| `onChanged` 回调 | `vi.fn()` 观测（重取触发即被测语义） | 改档 / 空态用例 |
| React 渲染树 / `FileViewer` | 真实渲染（不替身：段序与可点击性即被测语义） | 全部用例 |
| 文件系统 / 网络 | 不适用（前端不直接读盘） | 全部用例 |

## 不可测试项

| 对象 | 原因 | 承接方 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/docs.rs` + `docs_test.rs`（整文件删除） | 删除面无可测行为；三源关联 / `DocsOutcome` / `DocWarning` / `normalize_rel` 的能力面由 `api/paths.rs`（载体派生单点）、`api/mod.rs`（`doc_facts`）、`api/check.rs`（新判据）、`search.rs`（新语料）、`dto.rs`（投影换面）五节承接 | 上述五节的用例；文件删除由 static-check 的「零命中」核对 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口面） | 纯模块出口声明（`pub mod docs` / 四个 `pub use` 词退场），无运行时行为；`crate/archi` 无 `lib_test.rs` 惯例 | 编译期（下游引用）+ AC-6 的「全仓零命中」机械核对；行为面已由 `dto.rs` / `search.rs` 两节覆盖 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`（静态骨架资产） | `include_str!` 编译期内嵌的文本资产，无独立测试落点；内容面（字段面八项 / 两段示例 / 写纪律 / 零 `<>`）经 `archi_knowledge::build` 装配后由 `archi_knowledge_test` 断言 | `crates/infra/agent/src/archi_knowledge_test.rs`（本设计 `archi_knowledge.rs` 一节）；`<>` 占位符与体量闸由 `build` 既有判据守 |
| `.dev-team/architecture/models/01-app.toml`（删 `docs` 一行） | 模型数据面，无代码行为；删行与 parser 白名单同批是**防停摆**的机械事实 | `parse/toml_test.rs`（`docs` → `unknownField` 的反向判据）+ 桌面端 static-check（`archi_graph` 零诊断、53 / 120 不变、`modelHash` 变化留痕） |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts`（生成物） | `bindings:export` 的生成产物，MUST NOT 手改；五类型离场 / `doc` 两处进场由 `dto.rs` 的 specta 注解驱动 | `dto.rs` 一节的序列化 / 字段面断言 + 桌面端 `bindings:export` 幂等核对（`git diff` 复核）；`src/bindings/mod_test.rs` 的出线清单随动（机械） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` / `src/commands/archi/mod_test.rs` 的随动断言 | 被测源码（`src/bindings/mod.rs`、`src/commands/archi/mod.rs`）不在 design「修改文件」表内 ⇒ 无新增 / 修改的公共 API；两处只做机械随动：出线 DTO 清单删五类型、`DocRef` 四字段断言删、`linked_doc_paths` 调用删、「契约投影 MUST NOT 进 `docs` 列表」改指「契约路径不出现在 `doc` 内」 | 两测试文件自身（测试相位机械随动，零新设计条目） |
| `packages/desktop/src/{app.test.tsx, views/architecture/hooks/use-archi-module.test.ts, views/architecture/architecture-view.test.tsx, views/architecture/components/architecture-canvas.test.tsx, lib/drift-marks.test.ts}` 的 DTO 载荷随动 | `docs: []` / `warnings: []` → `doc: null`（否则 `tsc` 红），属机械面；`element-drawer.test.tsx` 的行为面已单设章节 | 各测试文件自身（机械随动）；前端零新行为断言 |
| 真实仓库读数（AC-5）：`docMissing 43` / `apiMissing 0` / 其余十一值 0 / `status = clean` / `passed = true` / 分母 44 / `api = none` 2 / `modelHash` 变化 | 需真实仓库模型 + 十六份金样 + 宿主门禁运行态；`reports/archi/` 是门禁运行时产物（不入库、不作判红面），`apiFindings: []` 或空目录 MUST NOT 充证据（R-13） | 测试相位实测 + change 报告留痕（`reports/`）；fixture 级等价读数已在 `api/mod_test` 与 `drift/mod_test` 两节；金样重录留痕在 `tests/golden_test.rs` 一节 |
| AC-8 的「`decisions/0001-model-granularity.md` 仍在盘」 | 文件存在性事实，无运行时行为可断言 | 桌面端 static-check（`test -f`）；符号面零命中已在 `layout/mod_test` 一节 |
| AC-11 的零扩散与版本纪律（`git status --porcelain` ⊆ 白名单、`package.json` +1 patch、`tauri.conf.json` 仍跟随、`src-tauri/Cargo.toml` 不变、`openspec/specs/**` 除 9 个 delta 外零 diff） | 变更集比对与版本号是机械步骤（静态脚本由桌面端执行，agent 内不重复执行） | 桌面端 static-check / release workflow（`.github/workflows/desktop-release.yml`） |
| `AGENT.md` / `CLAUDE.md` `## 模块 API 契约` 节改写（`doc` 单值纪律 + `error` 档判据进 `passed` + 十三值指路） | 仓库根纪律文本，无代码行为；判定靠字面与结构（要点数 5 → 6、两份节内正文逐字相同） | 桌面端 static-check 的 grep / 逐条对拍 |
| `crates/tools/archi-api-gen/src/main_test.rs` 与 `crates/core/archi/src/{tree_test,graph_test,diff_test,edit_test}.rs`、`api/{diff_test,render_test,schema_test}.rs` 的夹具 `docs` 行清理 | 语义零变化（`doc` 不是生成物、不是源文件、不进扫描面），仅夹具字段名随动 | 各测试文件自身（机械随动，不新增设计条目） |
| `ArchiElement` 字面量夹具补 `doc`（事实校准 16 的九处：`api/paths_test.rs`、`api/extract/mod_test.rs`、`scan/{mod_test,owner_test}.rs`、`tests/pilot_test.rs`、`src/lib/ipc-contract.test.ts`、`element-tree.test.tsx`、`use-archi-graph.test.ts`、`change-detail-view.test.tsx`） | 字段面进场的机械面（不补即编译 / `tsc` 红），语义零变化；字段面语义本身已在 `dto.rs` / `api/paths.rs` / `api/check.rs` 三节断言 | 各测试文件自身（机械随动）；`api/paths_test.rs` 的载体用例见 `api/paths.rs` 一节 |
