# 测试设计: module-entry-docs-semantics

> **变更**: module-entry-docs-semantics（E-1 · 契约资产目录迁移 `api/` → `models/` + `graph/`）
> **基于**: `proposal.md`（AC-1…AC-11、R-1…R-15、实现文件 / 测试文件两张表）、`design.md`（第六次修订 · 变更清单 · 新增 / 修改文件表、公共函数 / API 表、D1–D18、事实校准 1–12、OQ-1…OQ-6）
> **范围**: design「新增文件」为空表；「修改文件」中 **Rust 源码九份**（`foundation/src/layout/mod.rs`、`archi/src/api/{paths,mod,diff,schema}.rs`、`archi/src/dto.rs`、`archi/src/api/extract/mod.rs`、`archi/src/model.rs`、`infra/agent/src/archi_knowledge.rs`），其中 `api/extract/mod.rs` 与 `model.rs` 是 **doc-only 一行**（design 事实校准 7 / D16）。另有 `archi/src/tree.rs` 按 `desktop-archi-file-tree` delta 补用例但**源文件零 diff**（附加节）。其余为资产迁移（`.dev-team/architecture/**`，含 `api/**` 的删除）、生成物 `bindings.ts`、两份仓库根 md（`AGENT.md` / `CLAUDE.md`）与 `packages/desktop/package.json` 一行版本 —— 后三类不进单元测试面（见 `## 不可测试项`）。被删除的**源码符号** `archi_api_dir` / `archi_api_rel_dir` 按 design 公共 API 表纳入被测面（断言形态是「零命中」）。
> **回溯校准（本轮）**: ① 绑定面按「形状 / doc」两半判——`bindings.ts` 恰 2 行 doc 注释随动，MUST NOT 写成「`types/generated/**` 零 diff」（R-12 / D15 ①）；② 金样给**显式一处例外**——十五份逐字节不变 + `api-mismatch.full.json:58` 恰 1 行换 base（R-13 / 事实校准 9）；③ 两处 doc 散文串（`api/extract/mod.rs:5`、`model.rs:46`）纳入实现面，判据归 AC-5 第 ③ 层（D16），机械守卫落 `dto_test`；④ **AC-4 的 41 条读数与留痕报告是移交前置、不是本 change 的测试面**——宿主门禁二进制是变更前构建（`paths` 仍指 `api/` ⇒ `enabled = false` ⇒ `apiFindings: []`），任何相位无权重建 / 重启它，故本设计只在 `api/mod.rs` 一节钉「换根实现 + 离线可推读数（`44 − 2 − 1 = 41`）」，运行态读数与 `reports/archi/<timestamp>.json` 归门禁 / verify 相位在宿主重建后留痕；`reports/archi/` 是**门禁运行时产物**（不入库、不计入交付面、不作判红面）；AC-4 在本 change 侧按**四面**机械判：① `paths::directory` 体 == `archi_models_dir(root)`、② `api::enabled` 体逐字 `has_markdown(&paths::directory(root), 0)`、③ 离线可推 `44 − 2 − 1 = 41`、④ 反向判据「六个 toml 在场而递归零 `*.md`」；`apiFindings: []` 的报告、空目录、目录存在性三者 MUST NOT 充证据，**MUST NOT 施加隐藏 ACL / 权限护栏**（交付面纪律只有「不入库」一条，见 design 事实校准 11 / 12、D17 / D18）。
⑤ **本轮补正（回溯 T6 / T8-U3）**：`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs` 纳入「测试侧纯机械夹具换段」清单（`:437-457` 的契约 md 落点 `.dev-team/architecture/models/Api/App.Api.Subject.md`；`:573` / `:603` 的「`missing` 恰 `INDEX.toml`」断言须以「父目录末段 = `graph`」收口（`ends_with("graph/INDEX.toml")`，Windows 按分隔符归一后同判）），并在 AC-1 行与 `## 不可测试项` 各设承接位；`api/extract/mod.rs` 与 `model.rs` 两节的 `#### 待测功能` 改写为 doc-only 入口形的无序列表项（`extract_element()` / `extract()` / `Element.api` / `Model::element()`），不再是仅 HTML 注释的空段。

**测试框架与布局**（按既有测试文件实测）：

- 框架：Rust 内建 `#[test]`（工作区以 `cargo nextest` 运行，见 `reports/baseline/packages_desktop_src-tauri_rust/nextest.toml`）；无外部断言库、无 dev-dependency、无 mock 库。
- 断言风格：`assert!` / `assert_eq!` / `panic!`，消息为简体中文并携带上下文（`{root:?}` / `{path:?}` / 实得字符串）。
- 布局：单元测试为同层兄弟 `*_test.rs`（`crates/core/archi/src/api/paths_test.rs` 等），由 `mod.rs` 末尾 `#[cfg(test)] mod <name>_test;` 声明；`dto_test` / `model_test` / `tree_test` 由 `crates/core/archi/src/lib.rs` 的 `#[cfg(test)] mod` 块声明。测试项名用中文标识符（`fn enabled递归扫描到md()`）——本设计沿用该命名风格，不引入英文新命名。
- 进程边界：文件系统用 std 自建 `TempDir` RAII（`std::env::temp_dir()` + `Drop` 清理），不是替身类型；`crates/core/archi/tests/*.rs` 为集成测试面（不在本节设章节）。
- **源码自证**（既有惯用形态，`include_str!("<file>.rs")`）：断言某符号 / 字面量在产品源码内恰好 N 处。本变更的段名纪律（AC-5 四层）与「零第二份递归」（AC-7）完全落在这一形态上，`diff_test` 的四条计数断言（`read_dir` 恰 1、`depth > super::MAX_API_WALK_DEPTH` 恰 1、`walk_source_files(root)` 恰 1、`pub fn ` 恰 1）逐字不变。
- 前端：`vite-plus`（`vp test`），本变更**零前端产品代码 diff**、三个前端测试文件的 4 行旧串按 design 明写「不改」⇒ 不设章节。

## 验收范围

> 路径相对项目根；单元格内 `->` 左侧为被测源文件、右侧为其测试文件。AC 编号与 `proposal.md`「验收标准」一一对应。

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 目录树换段干净：`api/` 不存在；`models/` 一级恰 6 个 `*.toml`（无 `INDEX.toml`）；`models/Infra/` 恰 1 md + 1 `.schema.json`；`graph/` 恰 1 份 `INDEX.toml`。**旧位置缺席的判据面**：孤儿枚举严格两处派生 ⇒ `api/` 残留对 `api:check` 不可见，缺席由 `pilot_test` 负向断言 + 目录树读数 + 同提交 `git mv` 守（MUST NOT 写成「`api:check` 报应删未删」） | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` -> `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs`（`archi_graph_dir` / `archi_graph_rel_dir` 派生）；`packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs`（契约目录 == `archi_models_dir`、索引 == `archi_graph_dir`）；真仓库读数由 `crates/core/archi/tests/pilot_test.rs`（新增「`<archi_dir>/api` 目录不存在」负向断言）与桌面端 static-check 承接；`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（机械换段：契约 md 夹具落点 `.dev-team/architecture/models/Api/…`、`missing` 断言收口到「父目录末段 = `graph`」（`ends_with("graph/INDEX.toml")`，Windows 按分隔符归一后同判）——索引落点的行为面见证，见 `## 不可测试项`） |
| AC-2 | 生成物差异恰两行（`INDEX.toml` 说明段目录串 + 已声明元素 `schema` 列），`.schema.json` / `.i.rs` / `.i.ts` 零字节；`api:check` 三列空 | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs`（`build_index` 说明段取 `archi_models_rel_dir()`）；`packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs`（base 换段 ⇒ `schema` 列路径随动）；逐字节 diff 由 static-check 承接 |
| AC-3 | 模型读数零变化：`diagnostics` 空、53 元素 / 120 关系、`modelHash` 逐字不变；**十五份 `expected/*.json` 逐字节不变 + `api-mismatch.full.json:58` 恰 1 行 `occurrences[0].file` 换 base**（该份其余字段逐字不变）；反向判据（`INDEX.toml` 落 `models/` 根 ⇒ `unknownField` + 哈希变化） | `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs`（索引收面只在 `graph/` 根，`models/` 根不被枚举）；模型解析面零改动，读数由 `crates/core/archi/tests/golden_test.rs`（暂存根 `models/**`）与 `archi_drift` 读数承接——见 `## 不可测试项` |
| AC-4 | 契约面仍启用且读数不变：**本 change 侧判据（四面）** = ① `paths::directory` 体 == `archi_models_dir(root)`；② `enabled` 体逐字 `has_markdown(&paths::directory(root), 0)`（判据形态零改动、「模型文件不是启用证据」）；③ 离线可推读数 `44 − 2 − 1 = 41`（`apiEntryMissing 0` + `apiMissing 41`、`status = clean`）；④ 反向判据（清空 `models/**` 的 md、保留六个 toml ⇒ `enabled = false`）。**移交前置**：`enabled === true`、`apiFindings` 长度 41、`passed = true` 与 `reports/archi/<timestamp>.json` 的干净留痕须在宿主以换段后源码重建 / 重启后由门禁 / verify 相位产出（`reports/archi/` 是运行时产物，不入库、不计入交付面、不作判红面）；`apiFindings: []` 的报告、空目录、目录存在性三者 MUST NOT 充证据，MUST NOT 施加隐藏 ACL / 权限护栏 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`（`enabled` 换根、②④ 的用例 `enabled模型文件不是启用证据`、`apiFindings` 计数不因换根而变的 fixture 级等价断言）；① 的源码面在 `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `.../src/api/paths_test.rs`（`directory == archi_models_dir`）；运行态读数与留痕报告见 `## 不可测试项`（移交前置） |
| AC-5 | 段名纪律**四层**：① 符号面（`API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` 零命中）；② 路径段引号字面量面（`"api"` 零命中，**不覆盖 Rust 模块名**）；③ 旧方位串面（`architecture/api` / `<archi_dir>/api/` 零命中，**含 `api/extract/mod.rs:5` 与 `model.rs:46` 两处 doc 散文**；测试侧判据是 `.dev-team/architecture/api` 夹具落点串归零，`architecture/api` 作为否定断言的被检串允许留存）；④ 段拼接面（`models` / `graph` 行内拼接零命中，唯一触点 `layout/mod.rs`） | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` -> `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs`（①②④）；`packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs`（②）；`packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs`（②）；`packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`（③：三份产品源文 `include_str!` 守卫扩面）；`packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs`、`packages/desktop/src-tauri/crates/core/archi/src/model.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/model_test.rs`（③ 的落笔对象，机械守卫在 `dto_test`） |
| AC-6 | 知识文本换段：输出含 `<models_rel>/<Domain>/<FQN>.md` 与 `<graph_rel>/INDEX.toml` 两方位、零 `architecture/api`；四面词表与 4096 上限不变（实测 3646 字节）；三处断言（引导语 / 派生串 / 体量）同批更新 | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` -> `.../src/archi_knowledge_test.rs` |
| AC-7 | 两处判据换根不换语义：`models/<Domain>/Ghost.schema.json` → `orphaned`；`graph/INDEX.toml` 内容不符 → `mismatched`；深度上限 / 符号链接不跟随各一条用例；`read_dir` 仍只在有界枚举内（零第二份递归）；`starts_with(root)` 越根复核仍在写盘单点（零改动文件，夹具换段即可） | `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs`；`starts_with(root)` 一条驻 `packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs`（见 `## 不可测试项`） |
| AC-8 | 零扩散：改动集 ⊆ 白名单；`packages/desktop/src/**` **产品代码**零 diff；`bindings.zod.ts` 零 diff；`bindings.ts` 的 diff **恰 2 行且全在 doc 注释内**（类型 / 联合成员 / 字段名 / 字段序零变化）；`bindings:export` 幂等 | `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`（doc-only 的源码自证 + DTO 线形零变化）；`bindings.ts` 的两行 doc 与幂等性由桌面端 static-check 承接——见 `## 不可测试项` |
| AC-9 | 版本纪律：`package.json` `version` +1 patch；`tauri.conf.json` 保持 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变 | 无被测源文件（纯配置行）——见 `## 不可测试项` |
| AC-10 | 相关能力零漂移：`openspec/specs/**` 除 10 个 delta 外零 diff；`api/check.rs`、`api/{contract,emit,render}.rs`、`tools/archi-api-gen/src/main.rs`、`infra/archi/src/api_artifacts.rs` 零 diff | 无被测源文件（变更集外的文件出现 diff 即误改）——见 `## 不可测试项` |
| AC-11 | 文档纪律节三条要点换段且两份不分化：`architecture/api` 与「契约目录根」零命中；要点数仍 5；第 ① / ② / ⑤ 条方位串各自到位；`AGENT.md` 与 `CLAUDE.md` 该节正文逐字相同 | 无被测源文件（两份 md 纪律文本）——见 `## 不可测试项` |
| 附加 · `desktop-archi-file-tree` delta | 契约资产（`models/<Domain>/<FQN>.md` / `.schema.json`）与契约索引（`graph/INDEX.toml`）不进文件树；判据仍是扩展名收面（源文件零改动，delta 要求新增用例） | `packages/desktop/src-tauri/crates/core/archi/src/tree.rs` -> `.../src/tree_test.rs`（附加行：源文件零 diff，design「修改文件」表未列该文件） |

> **测试侧纯机械夹具换段的文件**（不新增用例，故不建 per-file 章节）：`api/{contract,render,check}_test.rs`、`api/emit/{mod,rust,ts}_test.rs`、`api/extract/mod_test.rs`（夹具落点；doc 断言归 `dto_test`）、`drift/{mod_test,report_test}.rs`、`tests/{pilot_test,golden_test}.rs`（`pilot_test` 另加负向断言、`golden_test` 另含金样一行例外）、`tools/archi-api-gen/src/main_test.rs`、`infra/archi/src/{api_artifacts_test,runner_test}.rs`、`infra/agent/src/sdk/archi_tool_test.rs`（agent 工具夹具：`:437-457` 的契约 md 落点 `.dev-team/architecture/models/Api/App.Api.Subject.md`、`:573` / `:603` 的「`missing` 恰 `INDEX.toml`」断言由 `ends_with("INDEX.toml")` 收口到「父目录末段 = `graph`」（`ends_with("graph/INDEX.toml")`，Windows 按分隔符归一后同判）（钉住新落点）、四字段响应断言零变化）、`src-tauri/src/commands/archi/mod_test.rs` —— 把 `.dev-team/architecture/api/**` 落点串替换为「契约 md/schema 在 `models/<Domain>/`、索引在 `graph/`」，断言值一律不变（唯一例外：`tools/archi-api-gen/src/main_test.rs` 的「清空契约目录」用例按新结构重写，判据口径见 `api/paths` 与 `api/diff` 两节）。

## 单元测试

### `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` -> `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs`

#### 待测功能

- archi_graph_dir(): 新增——`<archi_dir>/graph` 派生，契约索引 `INDEX.toml` 的唯一合法定位函数；全从 `archi_dir` 派生、纯推导零 IO、不存在的 root 亦正常返回
- archi_graph_rel_dir(): 新增——`<archi_rel_dir>/graph` 的 repo 相对串；`/` 分隔、零前导 `./`；消费者限于架构会话知识文本与段名纪律登记
- archi_api_dir(): 删除——`api` 段退役，产品 `.rs` 零命中，不保留改指别名（`archi_contract_dir` 一类 MUST NOT 出现）
- archi_api_rel_dir(): 删除——同上；契约目录**不新增函数**，直接复用 `archi_models_dir` / `archi_models_rel_dir`（D2）
- 常量与签名纪律（纪律锚，非新增 API）: `GRAPH_DIR_NAME` 私有常量与两个 `pub fn` 签名行——段名唯一触点、零行内拼段、函数体纯推导（无分支 / 无 `exists()`）
- 既有族零回归（回归锚）: `archi_dir` / `archi_models_*` / `archi_decisions_*` / `archi_config_path` / `archi_rel_dir` / `is_archi_rel_path` / `Layout` / `resolve` 逐字不变（D3 只允许 `api` 三符号换成 `graph` 三符号）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_graph_dir` | 正向 | `archi_graph_dir(root) == archi_dir(root).join("graph")` 逐字相等、尾段恒 `graph`、必比 `archi_dir(root)` 多一段；三种 root 形态（绝对 `/tmp/x` / 含空格与非 ASCII `/ws 含 空格/中文根` / 空 root） | 表驱动（3 组） |
| `archi_graph_dir` | 异常 | 非遮蔽：与 `archi_models_dir` / `archi_decisions_dir` / `archi_config_path` 两两不等且互不为前缀（索引目录不在任何既有子目录之内） | 单例 |
| `archi_graph_dir` | 边界 | 纯推导零 IO：`/definitely/does/not/exist` / 空 Path / 相对路径 `relative/a/b` / 含 `..` 段 `a/../b` / 尾分隔符 `/ws/` 五形态——返回可判路径、不 panic、不创建目录（调用前后 `path.exists() == false`）；超长 root（1200 字符）不 panic | 表驱动（5 组 + 1 超长） |
| `archi_graph_rel_dir` | 正向 | `== format!("{}/graph", archi_rel_dir())`；以 `archi_rel_dir()` 为前缀；尾段恰 `graph`（三者逐字对拍） | 单例 |
| `archi_graph_rel_dir` | 异常 | 形态禁令：零 `\`、零前导 `./`、零前导 `/`、零尾 `/`、零连续 `//`、零空段 | 单例 |
| `archi_graph_rel_dir` | 边界 | 相对族三值（`models` / `decisions` / `graph`）共享 `archi_rel_dir()` 前缀、两两不等、尾段各为单段；`root.join(archi_graph_rel_dir()) == archi_graph_dir(&root)`（绝对 tempdir / 尾分隔符 / 空 root 三形态） | 表驱动（3 组） |
| `archi_api_dir` / `archi_api_rel_dir`（删除） | 正向 | 替代面到位：`graph` 族一进一出对拍——`GRAPH_DIR_NAME` 常量定义在案且两个 `pub fn` 签名行恰 2 处；`api` 族符号零定义（`matches("pub fn archi_api")` == 0） | 单例 |
| `archi_api_dir` / `archi_api_rel_dir`（删除） | 异常 | `layout/mod.rs` 内 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir` 零命中；全包产品 `.rs`（`*_test.rs` 除）内三符号零命中（复用既有全包扫描的收集器）；`layout/mod.rs` 内 `"api"` 引号字面量零命中（段名换 `"graph"`） | 单例 + 表驱动（3 符号） |
| `archi_api_dir` / `archi_api_rel_dir`（删除） | 边界 | 零兼容壳：`lib.rs` 与 `mod.rs` 内零 `pub use … archi_api_dir` 再导出、零 `#[deprecated]` 改指别名、零 `archi_contract_dir` 一类「一个目录两个名字」的转发函数（`matches("archi_contract")` 全包零命中） | 单例 |
| 契约目录复用（`archi_models_dir` / `archi_models_rel_dir`，零改动回归锚） | 正向 | `Path::new(root).join(archi_models_rel_dir()) == archi_models_dir(root)`（两族一致，root 三形态）；契约目录无新函数：源码自证 `pub fn archi_` 集合中无 `contract` 项 | 表驱动（3 组） |
| 契约目录复用（零改动回归锚） | 异常 | 「一个目录两个名字」禁令的反向判据：`archi_models_dir` 与 `archi_models_rel_dir` 的函数体各只 `join(MODELS_DIR_NAME)` / `format!("{}/{}", …)` 一次，零改指分支（体逐字未变） | 单例 |
| 契约目录复用（零改动回归锚） | 边界 | `archi_models_dir` / `archi_models_rel_dir` 各恰 1 处定义；两者与 `graph` 族互不遮蔽（`archi_graph_dir(root) != archi_models_dir(root)` 且互不为前缀） | 单例 |
| 常量与签名纪律 | 正向 | `GRAPH_DIR_NAME` 常量定义在案（`const GRAPH_DIR_NAME: &str = "graph";`），且在 `mod.rs` 内出现 ≥ 3（定义 1 + `archi_graph_dir` 体 1 + `archi_graph_rel_dir` 体 1，doc 引用另计）；两个 `pub fn` 签名行恰 2 处 | 表驱动（2 计数） |
| 常量与签名纪律 | 异常 | 常量零外溢：除 `layout/mod.rs` 外产品 `.rs` 零引用 `GRAPH_DIR_NAME`（段名唯一触点）；`mod.rs` 内 `"graph"` 引号字面量恰 1 处（常量定义，函数体内零行内拼段） | 表驱动（2 计数） |
| 常量与签名纪律 | 边界 | `graph` 族函数体形态：`archi_graph_dir` 体恰 1 次 `join(GRAPH_DIR_NAME)`、`archi_graph_rel_dir` 体恰 1 次 `format!(…)`，零额外分支 / 零 `exists()` 调用（纯推导的结构证明） | 单例 |
| 既有族零回归 | 正向 | `archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `archi_config_path` / `archi_rel_dir` / `archi_models_rel_dir` / `archi_decisions_rel_dir` / `is_archi_rel_path` 对同一 root 的返回值与改动前逐字相等（既有断言零改动） | 表驱动 |
| 既有族零回归 | 异常 | `is_archi_rel_path` 的负样本集零变化（`openspec/architecture/…` / `.dev-team/archived/…` / 大小写变体仍 `false`） | 表驱动 |
| 既有族零回归 | 边界 | `Layout` 三字段与 `resolve` 语义零变化（`changes_root` / `archive_root` / `explores_root` 逐字），报告槽位族零触碰 | 表驱动 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：全部函数纯路径推导，无入参注入点 | 全部用例 |
| 文件系统 | 不替身——只做「零 IO」的反向观测（`path.exists()` 断言），沿用既有 `TempDir` RAII 建临时根 | 纯推导零 IO / 互不遮蔽用例 |
| 产品源码文本 | 真实 `include_str!("mod.rs")` + 既有全包 `.rs` 收集器（不 mock、不改收集口径） | 段名纪律 / 常量与签名计数用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs`

#### 待测功能

- directory(): 转发 `foundation::layout::archi_models_dir(root)`——契约目录本身 == 模型目录，本文件是它的唯一派生点；doc 方位串换段（D2 / D4）
- index_path(): `foundation::layout::archi_graph_dir(root).join(INDEX_FILE_NAME)`；MUST NOT 由 `directory(root).join(…)` 派生（索引与契约目录首次分家）
- contract_path(): 只换 base——结果变 `<archi_models_dir>/<Domain>/<FQN>.md`；FQN 形态判据与 `None` 语义零变化
- schema_path(): 只换 base——结果变 `<archi_models_dir>/<Domain>/<FQN>.schema.json`
- INDEX_FILE_NAME: 文件私有 → `pub(crate) const`；索引文件名唯一持有者（写盘目标与孤儿扫描收面共用，`diff.rs` MUST NOT 再持一份 `"INDEX.toml"`，事实校准 2）
- 派生族零回归（回归锚）: `domain_of` / `ref_relative` / `entry_candidates` / `entry_state` / `entry_*` / `generated_path` / `EntryForm` / `module_name` / `normalize_root` / `checked_root` 的形态与判据逐字不变（只换 base 与索引派生点）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `directory` | 正向 | `directory(root) == foundation::layout::archi_models_dir(root)`（同一值，不是相等前缀），末段恒 `models`；root 三形态（含空格 / 非 ASCII / 尾分隔符） | 表驱动（3 组） |
| `directory` | 异常 | 源码自证：`paths.rs` 内 `"api"` / `"models"` / `"graph"` 三个段名引号字面量零命中（段名只经派生函数，零行内拼接）；且 `archi_api_dir` 零命中、`archi_models_dir` / `archi_graph_dir` 均在 import 行与函数体内出现 | 单例 |
| `directory` | 边界 | 空 root → 纯相对形式逐字相等；盘符 root `C:\demo\alpha` 与尾分隔符 root 逐字拼接；超长 root（1200 字符）不 panic | 表驱动（4 组） |
| `index_path` | 正向 | `index_path(root) == foundation::layout::archi_graph_dir(root).join(INDEX_FILE_NAME)`；末段恰 `INDEX.toml`；父目录恰 `graph/` 根 | 单例 |
| `index_path` | 异常 | **重写既有「索引层级 = 契约目录本身」**：`index_path(root).parent() != Some(directory(root))`、`!index_path(root).starts_with(directory(root))`——索引与契约目录分家（旧断言与新结构直接矛盾，必须改为分家判据） | 单例 |
| `index_path` | 边界 | 全局唯一：同一 root 连续 5 次调用同一值（与元素数无关）；不存在 root 纯推导返回且零 IO（不创建 `graph/`）；派生结果无 `.` / `..` 段 | 表驱动（5 次 + 1 缺 root） |
| `contract_path` | 正向 | `<archi_models_dir>/<Domain>/<FQN>.md` 逐字；父目录 == `directory(root).join("<Domain>")`；`App.Backend.Infra.ArchiRuntime` → `Infra/` 段 | 单例 |
| `contract_path` | 异常 | 形态判据零变化：`""` / `App/X` / `App\X`（路径分隔符）/ `C:App`（盘符）/ `App..X` / `App.` / `App.X.` 一律 `None`（FQN 兼作文件名，放行即越界写） | 表驱动（7 组） |
| `contract_path` | 边界 | 单段 FQN `App` → `Root/` 目录（`ROOT_DOMAIN` 缺省零改动）；超长合法 FQN（> 1000 字符、零分隔符）→ `Some` 且不 panic；特殊字符 FQN（emoji / 换行）形态合法即放行（判据只看形态，不清洗）；root 三形态（不存在 / 空 / 尾分隔符）父链同源 | 表驱动（4 组） |
| `schema_path` | 正向 | 与 `contract_path` 同目录、末段 `<FQN>.schema.json`；两函数父目录逐字相等 | 单例 |
| `schema_path` | 异常 | 与 `contract_path` 共用同一形态判据（`None` 集逐值对拍 7 组，零第二套判据） | 表驱动（7 组） |
| `schema_path` | 边界 | 单段 FQN → `Root/`；root 三形态父链同源；suffix 恒 `.schema.json`（零 `.json` 形态） | 表驱动（3 组） |
| `INDEX_FILE_NAME` | 正向 | 常量值恰 `"INDEX.toml"`；`api/diff.rs` 经 `super::paths::INDEX_FILE_NAME` 消费（收面后缀唯一持有者在 `paths`） | 单例 |
| `INDEX_FILE_NAME` | 异常 | 源码自证：`paths.rs` 内 `"INDEX.toml"` 字面量恰 1 处（常量定义），`diff.rs` 内零命中；无 `INDEX_SUFFIX` 一类同值第二常量 | 单例 |
| `INDEX_FILE_NAME` | 边界 | `pub(crate)` 可见性可达性——`crate::api::diff` 的 `api_dir_artifacts` 两处调用之一以本常量为后缀实参（跨模块引用在编译期即证明，用例侧以 diff 侧断言承载） | 单例 |
| 派生族零回归（回归锚） | 正向 | `domain_of` / `ref_relative` / `entry_candidates` / `entry_state` / `entry_*` 全族 / `generated_path` / `EntryForm` / `module_name` 的既有断言逐条不变（本变更不动这些形态与判据） | 表驱动 |
| 派生族零回归（回归锚） | 异常 | 既有形态判据的负样本零回归：非法 `entry` 四理由（`Malformed` / `OutsidePath` / `NotOnDisk` / `Duplicated`）与非法 `root`（绝对 / 盘符 / `..`）仍各报原结论 | 表驱动 |
| 派生族零回归（回归锚） | 边界 | 生成物落点仍与入口同级（`root` 之外零影响）、`ts_import_specifier` / `language_of` 的 `Unknown` 归 Rust 语义零变化 | 表驱动 |

> 组合用例挂靠：`contract_path` / `schema_path` 的落点与 `directory` 的 base 换段是同一条派生链（`domain_dir` → `contract_path` / `schema_path`），链路发起方是本文件的派生单点，故组合断言写在本节（不断言 `contract_path` 与 `schema_path` 之外的模块）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯路径运算被测面） | 零 mock：`root: &Path` 与 `fqn: &str` 直传，`Path::new("/ws")` 一类字面根 | 全部用例 |
| `foundation::layout` 派生函数 | 真实调用（不 stub：`directory` / `index_path` 的「同源」正是被测语义，替身会自造第二事实源） | `directory` / `index_path` / 落点断言 |
| 文件系统 | 不替身——本文件除 `entry_state` 外零 IO；落点用例只用纯推导断言（既有 `TempDir` 只用于 `entry_state` 的既有用例） | 全部用例 |
| 产品源码文本 | 真实 `include_str!("paths.rs")` / `include_str!("mod.rs")`（既有源码自证形态） | 段名纪律 / 常量持有者用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`

#### 待测功能

- enabled(): 判据 = `<archi_models_dir>` 递归含 ≥1 份 `*.md`（根随 `paths::directory` 换段自动跟随，**代码零改动**，事实校准 1）；doc 明写「一级 `NN-<域>.toml` 不是启用证据」；MUST NOT 收紧为 `<Domain>/<FQN>.md` 形态（D7）；MUST NOT 在命令层另判一套
- 契约面读数守恒（跨模块组合：`enabled` → `load_contracts` → `api_findings`）: 启用条件换根后，元素数 / `path` 非空元素数不变 ⇒ `apiEntryMissing 0` + `apiMissing 41` 的 fixture 级等价读数不变；「有模型、零契约 md」仍是未启用

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `enabled` | 正向 | `models/Infra/App.X.md` → `true`；两层深 `models/Infra/Sub/x.md` → `true`；`models/README.md` → `true`（判据只看 `*.md` 在场，命名合法性归 `load_contracts`）；`models/01-app.toml` 与 `models/Infra/App.X.md` 并存 → `true` | 表驱动（4 组） |
| `enabled` | 异常 | **新增反向判据（AC-4 ④，design 点名的用例 `enabled模型文件不是启用证据`）**：`models/` 一级恰 6 个 `NN-<域>.toml` 而零 `*.md` → `false`（模型文件不是启用证据——本变更要钉的那一半）；`models/` 只含 `*.schema.json` 与 `graph/INDEX.toml` → `false`；`x.MD` → `false`（大小写敏感）；`x.md.txt` → `false`；同名目录 `models/x.md/` → `false`（目录不是文件）；符号链接目录指向含 md 的树 → `false`（不跟随，unix 断言、windows 建不成时只断言不 panic） | 表驱动（6 组） |
| `enabled` | 边界 | `models/` 缺席（无 `.dev-team`）→ `false` 零 panic；`models/` 存在但空目录 → `false`；深度上限两侧（`MAX_API_WALK_DEPTH == 4`：`<sub>^4/x.md` 命中、`<sub>^5/x.md` 不命中且返回 `false` 不栈溢出）；`models/` 内同时有契约子树与模型 toml → `true`（双身份混居零干扰） | 表驱动（4 组） |
| `enabled` | 边界 | 单点判定自证（AC-4 ②）：`include_str!("mod.rs")` 内 `has_markdown(&paths::directory(root), 0)` 恰 1 处且逐字未变（命令层无第二处判断）；`enabled` 体内零 `archi_models_dir` / 零段名字面量（根只经 `paths::directory` 单点） | 单例 |
| 契约面读数守恒（跨模块：`enabled` → `load_contracts` → `api_findings`） | 正向 | 契约 md 与模型 toml 同住 `models/`（md 在 `<Domain>/` 子目录）时启用且 `load_contracts` 正常出三表；模型 toml 混居不产生结构诊断 | 单例 |
| 契约面读数守恒（跨模块：同上） | 异常 | 「有模型、零契约 md」形态 → `enabled = false` 且 `api_findings` 为空（既有 `artifact_check未启用契约面仍ok` / `findings未启用返回空` 两用例的夹具换段后语义不变） | 单例 |
| 契约面读数守恒（跨模块：同上） | 边界 | 冷启动态新形态的边界：`models/` 目录在、`graph/` 目录在、契约子树为空（零 `<Domain>/` 子目录）→ `enabled = false`；元素数与 `path` 非空元素数不变时 `apiFindings` 计数不因段名换根而变（`apiEntryMissing 0` / `apiMissing` 41 的 fixture 级等价断言） | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（`models/**` / `graph/**` / 模型 toml） | 真实 `TempDir` RAII + 真实写盘（std，无替身、无内存 FS）；`fs::create_dir_all` 造空目录与同名目录 | 全部 `enabled` 用例 |
| `paths::directory` | 真实调用（换根本身即被测语义，替身即绕过） | 全部用例 |
| 符号链接 | 真实 `std::os::unix::fs::symlink` / `std::os::windows::fs::symlink_dir`，建不成时降级为「不 panic、结果可判」 | 符号链接不跟随即用例 |
| 时钟 / 网络 / 子进程 | 不适用（全链零外部依赖） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：`api_dir_artifacts` 与 `collect_artifacts` 均为私有函数（design 公共 API 表明确「私有函数不进本表」），换根只落在这两处；公共入口 `compare` 的签名、三列语义与求值序零变更（design「修改文件」表 diff.rs 行原话「`compare` 与三列语义零改动」）。以下条目是**既有公共入口的回归锚**（不是新增 API），AC-7 的观测面必须落在它上面。 -->

#### 待测功能

- compare()（两处派生，回归锚）: 孤儿扫描由单根变两处——`<archi_models_dir>/**` 收 `.schema.json`、`<archi_graph_dir>/**` 收 `INDEX.toml`；两处共用一个 `collect_artifacts(directory, suffix, depth, found)`，唯一差异 = 根实参 + 后缀实参（D5）；`mismatched` / `orphaned` / `missing` 三列语义与 `Err` 口径零改动
- compare()（零第二份递归，回归锚）: `read_dir` 仍恰 1 处、深度上限仍复用 `MAX_API_WALK_DEPTH`、符号链接不跟随、`walk_source_files(root)` 仍只此一处；四条既有源码自证断言逐字不变
- compare()（段名纪律，回归锚）: 两处根各经派生函数（`super::paths::directory` / `foundation::layout::archi_graph_dir`），收面后缀各经唯一持有者（`super::SCHEMA_FILE_SUFFIX` / `super::paths::INDEX_FILE_NAME`）；文件内 `"INDEX.toml"` / `"models"` / `"graph"` 引号字面量零命中（AC-5 ②）
- compare()（既有语义守恒，回归锚）: 三列语义、序（`workspace_render` 输出序 / 路径字节序）、`Err` 口径与零写盘纪律与改动前逐字一致

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `compare`（孤儿扫描两处派生） | 正向 | `models/<Domain>/Ghost.schema.json`（不在应生成集）→ `orphaned` 恰含该串；`graph/INDEX.toml` 内容不符 → `mismatched` 恰含该串；两列互不交叠、`missing` 为空 | 单例 |
| `compare`（孤儿扫描两处派生） | 异常 | 契约目录外同名文件不入列：`src/App/Ghost.schema.json` → 不入 `orphaned`；`models/INDEX.toml`（索引错位到模型根）→ 不入 `orphaned`（索引收面只在 `graph/` 根）；`models/App/Ghost.schema.json.bak` / `graph/INDEX.toml.txt` / `graph/INDEX.toml.bak` → 后缀不匹配不入列；`models/App/Ghost.schema.json/`（同名目录）→ 不入列 | 表驱动（6 组） |
| `compare`（孤儿扫描两处派生） | 边界 | 深度上限两侧、两处各一条：`models/a/b/c/d/Deep.schema.json`（depth 4，`== MAX_API_WALK_DEPTH`）入列、`models/a/b/c/d/e/f/Ghost.schema.json`（depth 6）不入列；`graph/` 深层嵌套的同名后缀文件同判（收面后缀仍只收 `INDEX.toml`）；符号链接不跟随：`models/` 与 `graph/` 各一条（链接指向含孤儿文件的树 → 不入列，unix 断言 / windows 降级为不 panic） | 表驱动（4 组 + 2 链接） |
| `compare`（零第二份递归，源码自证） | 正向 | 一份递归两处调用：`include_str!("diff.rs")` 内 `collect_artifacts(` 恰 4 处（定义 1 + 递归自调 1 + 两处顶层派生 2），两次顶层调用的实参对分别为 `(super::paths::directory(root), super::SCHEMA_FILE_SUFFIX)` 与 `(foundation::layout::archi_graph_dir(root), super::paths::INDEX_FILE_NAME)`（根与后缀两实参不同，四个实参各恰 1 处） | 单例 |
| `compare`（零第二份递归，源码自证） | 异常 | `include_str!("diff.rs")`：`read_dir` 恰 1 处、`depth > super::MAX_API_WALK_DEPTH` 恰 1 处、`walk_source_files(root)` 恰 1 处、`pub fn ` 恰 1 处（四条既有断言逐字不变）；`collect_api_dir` / `path == index` 零命中（旧收面判据已退役） | 表驱动（4 计数 + 2 零命中） |
| `compare`（零第二份递归，源码自证） | 边界 | 深度上限复用既有常量（零新常量）：`SKIPPED_DIRECTORY` / `MAX_MODULE_WALK_DEPTH` 零命中、`WalkBuilder` 零命中；`symlink_metadata` + `is_symlink` 跳过与 `!is_file` 跳过两段仍在（三语义照搬的源码面） | 表驱动（4 组） |
| `compare`（段名纪律） | 正向 | 两处根各经派生函数：`include_str!("diff.rs")` 内 `super::paths::directory` 与 `foundation::layout::archi_graph_dir` 各恰 1 处；收面后缀经 `super::SCHEMA_FILE_SUFFIX` / `super::paths::INDEX_FILE_NAME` 各恰 1 处 | 表驱动（4 计数） |
| `compare`（段名纪律） | 异常 | `include_str!("diff.rs")` 内 `"INDEX.toml"` / `"models"` / `"graph"` 引号字面量零命中（行内拼段零残留，AC-5 ②） | 单例 |
| `compare`（段名纪律） | 边界 | 索引文件名无第二持有者：`INDEX_FILE_NAME` 的定义只出现在 `paths.rs`，`diff.rs` 仅以 `super::paths::INDEX_FILE_NAME` 引用 | 单例 |
| `compare`（既有语义守恒） | 正向 | 零差异基线：按应生成集逐份落盘且内容逐字相同 → 三列全空、不 `Err`（`materialize` 后的既有用例，夹具落点换段）；三类差异同场互不混淆（两份不符 + 两份缺 + 两份孤儿，序 = `workspace_render` 输出序 / 路径字节序） | 单例 |
| `compare`（既有语义守恒） | 异常 | 结构错模型 → `Err`（与 `workspace_render` 同口径，命令面不返回部分差异），`Err` 前后零写盘；盘上无关同名文件（非 `.i.rs` / `.i.ts`）不误报 | 表驱动（2 组） |
| `compare`（既有语义守恒） | 边界 | 零写盘：调用前后 `root` 递归快照（字节 + mtime）相等、盘上文件数不变；`diff.rs` 内零 `fs::write` / `create_dir` / `File::create`；`root` 为空 `Path` / 不存在 / 指向文件 → 落 `Result` 内不 panic；盘上 1000 份孤儿 → `orphaned.len() == 1000` 且按字节序 | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（源码树 / `models/**` / `graph/**`） | 真实 `TempDir` RAII + 真实写盘（`temp.write` / `write_absolute`），无内存 FS | 全部用例 |
| `scan::walk_source_files` | 真实调用（gitignore 感知与剪枝即被测语义；替身会让「零第二份遍历」断言失效） | 零差异 / 构建产物树 / gitignore 用例 |
| `api::render::workspace_render` | 真实调用（应生成集是 `compare` 的事实源，替身会自造第二事实源） | 全部用例 |
| `foundation::layout` / `paths` | 真实派生（换段后的根即被测形态） | 两处派生用例 |
| 符号链接 | 真实 `std::os::*::symlink`（建不成时降级为不 panic） | 两处「不跟随」用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs`

#### 待测功能

- build_index(): 说明段第三行的「契约面：目录 {}」取 `foundation::layout::archi_models_rel_dir()`（MUST NOT 取索引自身目录——索引与契约已分家，写成索引目录即指错地方，D8）；条目装配、七字段、字典序、`$defs` 规则、债表扫描逐字不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `build_index` | 正向 | 说明段第三行含 `foundation::layout::archi_models_rel_dir()` 的逐字结果，且以「契约面：目录」起句；同段仍含生成器版本（`archi-api {CARGO_PKG_VERSION}`）、`映射规则` 与 `common.json` 摘要前 12 位；说明段行数仍恰 3 | 单例 |
| `build_index` | 异常 | 说明段零 `foundation::layout::archi_graph_rel_dir()`（索引目录绝不出现在说明段——分家后的方向判据）；说明段零 `architecture/api` 串（旧方位零残留）；语言词禁令保持（全文本零 `crate` / `Cargo.toml` / `Rust` / `TypeScript`） | 单例 |
| `build_index` | 边界 | `entries = []` → 说明段仍 3 行、零 `[[entry]]`、`toml` 反解析为空表；条目序（FQN 字典序）与七字段键集逐字不变；同一 `entries` + `meta` 两次装配逐字相等（确定性 / 零状态依赖）；单条目 + 多条目（> 40 条）两规模说明段一致 | 表驱动（2 规模） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：`entries: &[IndexEntry]` + `meta: &ApiMeta` 直传；`meta()` 取编译期常量与真实摘要 | 全部用例 |
| `toml_edit` | 真实反解析（`DocumentMut` / `to_toml` 往返），不 mock | 说明段行数 / `[[entry]]` 用例 |
| `foundation::layout` | 真实派生（`archi_models_rel_dir()` 的字符串即被测值，替身即使断言失效） | 说明段方位用例 |
| 文件系统 / 时钟 / 子进程 | 不适用（本函数零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：本变更为 **doc-only**（`ApiStatus` 与 `ArchiApiRef::contract_path` 的 doc 方位串换段，字段名 / 顺序 / serde / specta 派生逐字不变，D15 ① / 事实校准 6）。以下条目是「doc 事实」与既有投影入口的回归锚；`dto.rs` 的公共函数签名一个字节都不动。 -->

#### 待测功能

- doc 方位串换段（doc-only，AC-5 ③ / AC-8）: `dto.rs` 内两处 `models/<Domain>/` 方位串在场、零 `architecture/api` / `架构域 api 目录` / `<archi_dir>/api/`
- 三份产品源文守卫（doc-only，跨文件，AC-5 ③ / D16）: 机械守卫扩面到 `dto.rs` + `api/extract/mod.rs` + `model.rs`——三份源文旧方位串 `matches` 计数归零，把「旧位置清空」从 review 记忆变成机械断言
- module_detail()（既有投影入口，回归锚）: `api` 字段经 `crate::api::api_ref_of` 单点取；DTO 线形（字段名 / 顺序 / `null` 语义 / `as_str` 分支序）零变化；`contractPath` / `schemaPath` / `generatedPath` 随 `paths` 派生自然换段
- graph_view() / 既有出线面（回归锚）: 类型注册表、字段声明序、枚举词表零变化（形状面零改动的行为证据）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| doc 方位串换段（`dto.rs`） | 正向 | `include_str!("dto.rs")` 内 `models/<Domain>/` 恰 2 处（`ApiStatus` 与 `ArchiApiRef::contract_path` 各一）、含 `<archi_dir>/models/<Domain>/<FQN>.md` | 表驱动（2 计数 + 1 串） |
| doc 方位串换段（`dto.rs`） | 异常 | `include_str!("dto.rs")` 内 `architecture/api`、`架构域 api 目录`、`<archi_dir>/api/` 三串 `matches` 计数各为 0（旧方位零残留） | 表驱动（3 串） |
| doc 方位串换段（`dto.rs`） | 边界 | 段落归属：换段只在 doc 注释内——`pub struct ArchiApiRef` 与 `pub enum ApiStatus` 的字段 / 变体行零改动（与改动前逐字对拍）、`dto.rs` 内 `foundation::layout::archi_api_dir` / `archi_api_rel_dir` 零命中 | 单例 |
| 三份产品源文守卫（`dto.rs` + `api/extract/mod.rs` + `model.rs`） | 正向 | 三份源文各 `include_str!` 一份，`models/<Domain>/` 在场（`dto.rs` 2 处；`api/extract/mod.rs` 1 处 `<archi_models_dir>/<Domain>/<FQN>.md`；`model.rs` 1 处「架构域 `models/<Domain>/` 目录」） | 表驱动（3 文件） |
| 三份产品源文守卫 | 异常 | 三份源文内 `architecture/api`、`架构域 api 目录`、`<archi_dir>/api/` 三串 `matches` 计数全 0——`api/extract/mod.rs:5` 与 `model.rs:46` 不回退即成立（反向判据：复原任一旧串 ⇒ 本用例判红） | 表驱动（3 文件 × 3 串） |
| 三份产品源文守卫 | 边界 | 守卫面不越界：`api/extract/mod.rs` 只允许这一行 doc diff——取数器分派 / 签名归一 / `extract` / `extract_element` 的能力面零改动（既有 `extract/mod_test.rs` 用例全绿即证）；`model.rs` 字段面（八项 + 两个指针）与 `deny_unknown_fields` 零改动（既有 `element字段数恰十二个…` 用例逐字不变） | 单例 × 2 文件 |
| `module_detail` / `api_ref_of` 投影（既有两路） | 正向 | 契约 md 落在 `models/<Domain>/` 且 schema 生成物同目录时，两路投影逐字段相等；`status` / `contractPath` / `schemaPath` / `generatedPath` 与 `paths::contract_path` / `schema_path` / `generated_path` 逐字一致（断言零变化，仅夹具落点换段） | 单例 |
| `module_detail` / `api_ref_of` 投影（既有两路） | 异常 | 无 md → 两路都是 `none` 且三路径全 `None`；模型字段 `api` 零回写（`Declared` 保持）；入口缺席（无 `path` / 入口不落盘）→ `generatedPath` 为 `None` 而 `status` 判据不变 | 表驱动（3 组） |
| `module_detail` / `api_ref_of` 投影（既有两路） | 边界 | root 形态（空 Path / 不存在 root）与 `<Domain>` 缺省（单段 FQN → `Root/`）下三路径逐字随 `paths` 派生、不 panic | 表驱动（3 组） |
| DTO 线形（零形状改动） | 正向 | `ApiStatus` 二值逐值对拍（`Declared` / `None`，`ALL` 序与 `as_str` 分支序一致）、`ArchiApiRef` 字段名与声明序逐字不变；`ArchiApiRef` / `ArchiApiCheck` / `ApiMeta` 的出线名与字段面零变化 | 表驱动（2 值 + 3 类型） |
| DTO 线形（零形状改动） | 异常 | 非法枚举值反序列化被 serde 拒收（零静默降级）；缺字段反序列化的结果与改动前逐字相同（既有出线口径零变化） | 表驱动（2 组） |
| DTO 线形（零形状改动） | 边界 | `bindings:export` 幂等与 `bindings.zod.ts` 零 diff 作为形状零变化的机械证据（由 static-check 承接，见 `## 不可测试项`）；三路径字段的 `null` 语义逐字不变 | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 toml / 契约 md / 入口文件） | 真实 `TempDir` RAII（既有 `healthy_dir` 等辅助，落点换段） | 投影两路用例 |
| `paths` / `drift` 取数面 | 真实调用（投影单点语义即被测对象） | 投影两路用例 |
| 产品源码文本 | 真实 `include_str!("dto.rs")` / `include_str!("api/extract/mod.rs")` / `include_str!("model.rs")`（既有源码自证形态，不加新的收集器） | doc 方位串 / 三源守卫用例 |
| 序列化面 | 真实 `serde_json::to_value` 往返（不 mock binder） | 线形用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：本文件只允许模块 doc 第 5 行的方位串一行 diff（`<archi_dir>/api/<Domain>/<FQN>.md` → `<archi_models_dir>/<Domain>/<FQN>.md`，doc-only，design 事实校准 7 / D16）；`extract` / `extract_element` / `normalize_signature` / 取数器分派的签名与行为逐字不变。故本节按「doc-only 入口形」只列**既有入口的零变化声明**：该 doc 事实的**机械守卫**落 `dto_test` 的「三份产品源文」用例（见 `dto.rs` 一节），既有行为回归由本文件 `mod_test.rs` 的既有用例（夹具落点换段）承担。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（doc-only 一行；取数器分派、签名归一、`extract` / `extract_element` 两个入口的能力面逐字不变）。AC-5 ③ 的判据不在此建条目，归 `dto.rs` 一节的「三份产品源文守卫」。 -->

- extract_element(): 公共入口签名与行为零变化（本变更只改模块 doc 第 5 行的方位串，doc-only 一行 diff）——整元素导出集的取数面、`rust::extract_crate` 的闭包遍历与读盘边界逐字不变
- extract(): 同上——单文件入口面的能力面（`rust` / `ts` 分派、`normalize_signature` 归一、零读盘零写盘）逐字不变；本文件内旧方位串（`architecture/api`、`<archi_dir>/api/`）零命中（AC-5 ③ 的落笔对象，机械断言在 `dto_test`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| doc 方位串（跨文件守卫，挂靠 `dto_test`） | 正向 | 见 `dto.rs` 一节「三份产品源文守卫」正向行（`include_str!("api/extract/mod.rs")` 内 `<archi_models_dir>/<Domain>/<FQN>.md` 在场） | 表驱动（与 `dto.rs` 同一用例） |
| doc 方位串（跨文件守卫，挂靠 `dto_test`） | 异常 | 见 `dto.rs` 一节「三份产品源文守卫」异常行（旧串 `architecture/api` / `<archi_dir>/api/` 计数归零） | 表驱动（与 `dto.rs` 同一用例） |
| `extract_element` / `extract`（行为面零变化） | 边界 | 既有 `mod_test.rs` 用例（夹具落点换段）逐条全绿 ⇒ 整元素导出集与单文件入口面的签名 / 分派 / 产出零改动；本文件 diff 恰 1 行（`//!` doc 行）、零新增用例（doc 事实的边界口径见 `dto.rs` 一节「三份产品源文守卫」边界行） | 表驱动（既有用例集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 产品源码文本 | 真实 `include_str!("api/extract/mod.rs")`（守卫写在 `dto_test`，零新增收集器） | doc 方位串用例 |
| 文件系统 / 取数器 | 不新增 mock——行为面零改动，沿用 `mod_test.rs` 既有 `TempDir` 与真实源文 | 既有回归 |

### `packages/desktop/src-tauri/crates/core/archi/src/model.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/model_test.rs`

<!-- design.md 未声明该文件的公共 API 变更：本文件只允许第 46 行 `api` 字段 doc 的方位词一行 diff（「架构域 `api` 目录」→「架构域 `models/<Domain>/` 目录」，doc-only，D16）；`Element` 八字段 + 两个指针、`deny_unknown_fields`、解析实现逐字不变。故本节按「doc-only 入口形」只列**既有字段面的零变化声明**：机械守卫同样落 `dto_test` 的「三份产品源文」用例；字段面回归由 `model_test.rs` 既有用例承担。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（doc-only 一行；字段面与解析实现零改动）。AC-5 ③ 的判据归 `dto.rs` 一节的「三份产品源文守卫」。 -->

- Element.api（`api` 字段 doc，doc-only，无签名 / 字段面变化）: 第 46 行方位词由「架构域 api 目录」换为「架构域 `models/<Domain>/` 目录」；八字段 + 两个指针、`deny_unknown_fields`、解析实现逐字不变
- Model::element(): 既有 FQN 查找入口行为零变化（字段面零改动）——本文件内旧方位串（`架构域 api 目录`、`architecture/api`）零命中（AC-5 ③ 的落笔对象，机械断言在 `dto_test`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| doc 方位串（跨文件守卫，挂靠 `dto_test`） | 正向 | 见 `dto.rs` 一节「三份产品源文守卫」正向行（`include_str!("model.rs")` 内「架构域 `models/<Domain>/` 目录」在场） | 表驱动（与 `dto.rs` 同一用例） |
| doc 方位串（跨文件守卫，挂靠 `dto_test`） | 异常 | 见 `dto.rs` 一节「三份产品源文守卫」异常行（`架构域 api 目录` 计数归零） | 表驱动（与 `dto.rs` 同一用例） |
| `Element` 字段面 / `Model::element()`（行为面零变化） | 边界 | 既有 `element字段数恰十二个且两个指针各一处` 与 `parse_with_models` 系列用例逐条全绿 ⇒ 八字段 + 两个指针 / `deny_unknown_fields` / FQN 查找逐字不变；本文件 diff 恰 1 行（`///` doc 行）、零新增用例（doc 事实的边界口径见 `dto.rs` 一节「三份产品源文守卫」边界行） | 表驱动（既有用例集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 产品源码文本 | 真实 `include_str!("model.rs")`（守卫写在 `dto_test`） | doc 方位串用例 |
| 文件系统 | 不新增 mock——解析面零改动，沿用 `model_test.rs` 既有纯字符串 `parse_with_models` 辅助 | 既有回归 |

### `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`

#### 待测功能

- build(): 派生段末句由「…；契约目录：`<archi_api_rel_dir>`」改为「…；契约资产：`<archi_models_rel_dir>/<Domain>/<FQN>.md`（手写真相）；契约索引：`<archi_graph_rel_dir>/INDEX.toml`」，实参序改四元（模型目录 / 决策记录目录 / 模型目录 / 索引目录）；骨架校验、四面词表注入、`join` 单点、`Err` 文案逐字不变（D11）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `build` | 正向 | 输出含「契约资产：」标签 + `foundation::layout::archi_models_rel_dir()` + `/<Domain>/<FQN>.md` + 「（手写真相）」；含「契约索引：」标签 + `foundation::layout::archi_graph_rel_dir()` + `/INDEX.toml`（两标签各恰 1 处）；四面词表（`ElementKind` / `GateTier` / `ApiStatus` / `DiagnosticKind` 全值）仍在场且连接形态仍为 `" / "`；三次调用逐字相等 | 表驱动（2 标签 + 4 词面） |
| `build` | 异常 | 输出零 `architecture/api` 串；输出零「契约目录：」旧标签（两种写法并存必然漂移，旧标签零命中）；注入串零 `<>` 相邻两字符（`include_str!("archi_knowledge.rs")` 内 `<>` 仍恰 4 处：两处 doc 引用 + 判据 1 + 文案 1，注入串 MUST NOT 产生第五处）；`build("   ")` → `Err("知识骨架为空")`、骨架含 `<>` → `Err` 含「占位符」（既有降级判据零变化） | 表驱动（4 组） |
| `build` | 边界 | 体量：`knowledge()` 输出 `len() == 3646` 且 `<= 4096`（实现期实测后写死，MUST NOT 预填估算值，事实校准 8）；骨架 `trim_end()` 尾界形态（骨架尾部空白不入文本）；两个方位串共享 `archi_rel_dir()` 前缀（同源派生）；真实骨架 `build(SKELETON)` 与 `knowledge()` 产物相等 | 表驱动（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯函数被测面） | 零 mock：`build(skeleton: &str)` 骨架以字符串直传，`SKELETON` 走 `include_str!` 编译期内嵌 | 全部用例 |
| `foundation::layout` 相对族 | 真实派生（派生串即被测值） | 两方位用例 |
| `archi::dto` 词表常量 | 真实 `*::ALL` 常量（词表只从代码派生，替身会自造第二份词表） | 四面词表用例 |
| 文件系统 / 网络 / 子进程 | 不适用（纯函数、零 IO） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/tree.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/tree_test.rs`

> 附加行：本变更对该源文件**零 diff**（design「修改文件」表未列它；`desktop-archi-file-tree` delta 只要求把既有判据写成机械事实并补 Scenario，proposal「测试文件」表点名 `tree_test`），故 `待测功能` 只列既有公共入口（design 未声明公共 API 变更），用例补齐 delta 要求的枚举面断言。

#### 待测功能

公共 API 零变更：`file_tree(root) -> Result<ArchiFileTree, String>` 签名与枚举判据零改动（仍是 `scan::is_source_file` 的五扩展名收面 + `ExcludeSet`）；本变更只把「契约资产与契约索引不进树」由既有判据推得的机械事实补成用例。

- file_tree(): 枚举面收面不变——`models/<Domain>/<FQN>.md` / `.schema.json` 与 `graph/INDEX.toml` 因扩展名不在收面内而不入 `entries`；改名成源文件扩展名（`models/<Domain>/x.rs`）后照常入列，证明「不进树」由扩展名收面达成而非路径黑名单；与漂移判定面对同一根给出同一集合

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `file_tree`（枚举面） | 正向 | `models/<Domain>/<FQN>.md` 与同目录 `<FQN>.schema.json` 落盘后不进 `entries`；`graph/INDEX.toml` 不进 `entries`；同时存在的源文件（`src/x.rs`）照常入列 | 单例 |
| `file_tree`（枚举面） | 异常 | 反向：把契约 md 改名为源文件扩展名（`models/<Domain>/x.rs`）→ **入列**（判据是扩展名、不是路径段——证明「契约资产不进树」由扩展名收面达成，而非路径黑名单） | 单例 |
| `file_tree`（枚举面） | 边界 | 树条目扩展名集合 ⊆ 漂移判定面同一收面（两面对同一根给出的文件集合一致）；`models/` 一级 `NN-*.toml` 模型文件不入树（`.toml` 不在收面内） | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（模型 / 契约资产 / 索引 / 源文件） | 真实 `TempDir` RAII + 真实写盘 | 全部用例 |
| `scan` / `OwnerIndex` / `ExcludeSet` | 真实调用（枚举单源即被测语义） | 全部用例 |
| `dto::graph_view` | 真实调用（模型只经图单点取，替身会让树与漂移面分叉） | 全部用例 |

## 不可测试项

| 对象 | 原因 | 承接方 |
|---|---|---|
| `.dev-team/architecture/{models/Infra,graph}/**` 三份资产迁移与 `api/**` 删除（AC-1） | 数据资产迁移（`git mv`，内容逐字 / 字节不变），无运行时行为可断言；`graph/` 目录由搬入决定存在，无代码派生。**旧位置缺席的判据面须说准**：孤儿枚举严格两处派生（`models/**` + `graph/`），`api/` 残留对 `api:check` **不可见**，不得写成「`api:check` 报应删未删」 | `crates/core/archi/tests/pilot_test.rs` 的仓库资产只读断言换新路径 + **新增负向断言「`<archi_dir>/api` 目录不存在」**（测试相位产物）；目录树读数由桌面端 static-check（`ls` / `find -maxdepth 1 -name '*.toml'`）承接；行为层等价断言已在 `api/paths` / `api/diff` 两节 |
| `INDEX.toml` 的逐字节差异恰两行（AC-2）与 `.schema.json` / `.i.rs` / `.i.ts` 零字节变化 | 生成物比对是 `api:export` + `api:check` 的读盘机械步骤，静态脚本由桌面端执行，agent 内不重复 | 桌面端 static-check（`api:export` 后 `git diff` 恰 2 行；`api:check` 三列空）；行为层等价断言 = `schema_test`（说明段取 `archi_models_rel_dir()`）+ `paths_test`（`schema` 列 base 换段） |
| AC-3 的真实读数（53 / 120、`diagnostics` 空、`modelHash` 逐字不变）与**金样一行例外** | 需真实仓库模型与金样文件树，属集成 / 门禁面；本变更对解析面零改动，`models/` 双身份的安全由「一级 + `.toml` 后缀」既有判据保证。口径：**十五份 `expected/*.json` 逐字节不变 + `api-mismatch.full.json:58` 恰 1 行 `occurrences[0].file` 换 base**（`occurrences[].file` 是 repo 相对路径、暂存根由 `archi_models_rel_dir()` 派生），MUST NOT 写成「十六份逐字不变」 | `crates/core/archi/tests/golden_test.rs`（暂存根改 `models/**`；该份其余字段逐字对拍）与桌面端 `archi_drift` 读数；反向判据（`INDEX.toml` 复制到 `models/` 根 ⇒ `unknownField` + `modelHash` 变化）由 `archi_graph` 诊断面 + 同一金样面承接 |
| AC-4 的运行态读数（`enabled === true`、`apiFindings` 分桶 `0 + 41 = 41`、`status = clean`、`passed = true`）与 `reports/archi/<timestamp>.json` 留痕 | **移交前置，不是本 change 的交付物 / 测试面**：宿主 `dev-team` 门禁二进制是变更前构建（`paths` 仍指 `api/` ⇒ `enabled = false` ⇒ 判据零条目），本 change 任何相位无权重建 / 重启该进程；`reports/archi/` 是**门禁运行时产物**——不入库、不计入交付面、不作判红面（`git status` 的 `?? …/reports/archi/` 属预期，`apiFindings: []` 是该构建的正常输出，既不判红也不作证据）。本 change 侧只承接结构面与离线可推读数（`44 − 2 − 1 = 41`）；`apiFindings: []` 的报告、空目录、目录存在性三者 MUST NOT 充证据，交付面纪律只有「不入库」一条、MUST NOT 施加隐藏 ACL / 权限护栏（design 事实校准 12 / D18） | 宿主以换段后源码重建 / 重启后由门禁 / verify 相位留痕（design 事实校准 11 / D17）；本 change 内的结构面判据 = `enabled` 单点换根已在 `api/mod.rs` 一节，`api:export` / `api:check` 面已在 `## 不可测试项` 的生成物行 |
| AC-7 三语义中的 `starts_with(root)` 越根复核 | 驻写盘单点 `crates/infra/archi/src/api_artifacts.rs` 的 `write_artifacts`，**源文件零改动**、判据不变，本变更只换其夹具落点（事实校准 4） | 既有 `api_artifacts_test.rs` 用例（夹具换段，零新断言）；另两语义（深度上限 / 符号链接不跟随）已在 `api/diff.rs` 一节 |
| `packages/desktop/package.json` 的 `version` +1 patch（AC-9） | 纯配置行，无运行时行为；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 `[package] version` MUST NOT 随动 | 版本纪律与 release workflow（`.github/workflows/desktop-release.yml`）机械承接 |
| AC-8 的零扩散核对（改动集 ⊆ 白名单、`src/**` **产品代码**零 diff、`bindings.zod.ts` 零 diff、`bindings.ts` diff **恰 2 行且全在 doc 注释内**、`bindings:export` 幂等）与 AC-10（`openspec/specs/**` 除 10 个 delta 外零 diff、`api/check.rs` / `api/{contract,emit,render}.rs` / `tools/archi-api-gen/src/main.rs` / `infra/archi/src/api_artifacts.rs` 零 diff） | 白名单比对与绑定幂等是 `git status --porcelain` / `bindings:export` 的机械步骤。**MUST NOT 把本条写成「`types/generated/**` 零 diff」**：specta 把 `dto.rs` 的 doc 抄进 `bindings.ts`，换段必然产生恰 2 行（`:633` `ApiStatus`、`:757` `ArchiApiRef.contractPath`）——形状零变化与文件零 diff 不是等价命题（R-12）。**`reports/archi/` 不参与本条核对**：门禁运行时产物，`??` 出现属预期、MUST NOT `git add`、既不加分也不判红（D14 / D17） | 桌面端 static-check；`dto.rs` 的「零形状改动」行为层证据在 `dto_test` 的线形与源码自证用例 |
| `AGENT.md` / `CLAUDE.md` `## 模块 API 契约` 节三条要点换段（AC-11） | 仓库根纪律文本，无代码行为；判定靠字面与结构（`architecture/api` 零命中、「契约目录根」零命中、要点数 = 5、两份节内正文 diff 零行）。**行数口径**：该节需改 **3 行**（第 26 / 27 / 30 行 = 第 ① / ② / ⑤ 条），第 27 行不含字面量，故须按「三条要点逐条对拍」而非仅字面量归零 | 桌面端 static-check 的 grep / 逐条对拍判定 |
| `crates/tools/archi-api-gen/src/main_test.rs` 的「清空契约目录 → 元素转 absent、索引目录由 layout 派生重建」用例 | CLI 集成测试（`crates/tools/**`，不经单元测试模块声明）；本变更新结构下该用例需**重写**而非新增单元断言 | 该集成测试自身（判据口径见 `api/paths`「索引在 `graph/` 根、与契约目录分家」与 `api/diff`「索引收面只在 `graph/` 根」两节）；门禁段由 static-check 承接 |
| `crates/core/archi/tests/{pilot_test,golden_test}.rs`、`crates/infra/archi/src/{api_artifacts_test,runner_test}.rs`、`crates/infra/agent/src/sdk/archi_tool_test.rs`、`src-tauri/src/commands/archi/mod_test.rs` 的落点字面量换段 | 集成 / 落盘单点面：只做夹具落点换段与读盘断言换路径，零新行为断言（`pilot_test` 另加旧位置负向断言；`commands/archi/mod_test` 的「文档关联集不含契约目录」否定断言改指 `models/<Domain>/**`）。`archi_tool_test.rs` 的两处机械面：`:437-457` 的 `api_root` 把契约 md 写在 `.dev-team/architecture/models/Api/App.Api.Subject.md`（旧 `api/Api/…` 落点零命中）；`:573` / `:603` 的「`missing` 恰 `INDEX.toml`」断言须以「父目录末段 = `graph`」收口（`ends_with("graph/INDEX.toml")`，Windows 按分隔符归一后同判）（仅 `ends_with("INDEX.toml")` 钉不住新索引根）——四字段响应断言（`findings` / `artifacts` / `byCategory` / `items`）零变化 | 各测试文件自身（机械换段）；真读数由桌面端门禁承接 |
| `packages/desktop/src/{lib/ipc.test.ts,views/architecture/components/architecture-canvas.test.tsx,views/architecture/components/element-drawer.test.tsx}` 的 4 行旧串 | design 明写「不改」：DTO 载荷样例，任意字符串即可，改了也不增证据（改反而触碰前端产品测试面） | 无（不动）；`src/**` 产品代码零 diff 由 static-check 守 |
| `enabled` 对 `models/**` 任意 `*.md` 的误启用收紧（OQ-4 / R-3） | 判据形态按 D7 保持不动；收紧为 `<Domain>/<FQN>.md` 属**行为变化**，与判据面十三值同批（E-2） | 本变更只钉「模型文件不是启用证据」这一半（`api/mod.rs` 一节）；收紧留待 E-2 |
| 旧位置残留（`api/**`）对 `api:check` 的机械判红（D6 / R-14） | 严格两处派生下 `api/` 残留不在枚举面内 ⇒ 对 `api:check` 不可见，AC-1 已把机制写成「机制诚实版」；若日后确需机械守旧段，应单开变更把枚举根放宽到 `<archi_dir>` 并同步改 spec | 桌面端 static-check 的目录树读数（`api/` 零命中）+ `pilot_test` 负向断言 + 同提交 `git mv` |
