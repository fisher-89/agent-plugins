# desktop-crate-layout Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**段名字面量纪律的换段**（E-1 目录迁移）：契约面的段名禁令由「`<archi_dir>/api`」改为「`<archi_dir>/models/<Domain>`（契约资产）与 `<archi_dir>/graph`（契约索引）」两段，且一律经 `foundation::layout::archi_models_dir` / `archi_graph_dir` 派生。落位、依赖方向、依赖清单、边界分类学与写盘外置**逐字不变**——本变更零 crate 增删、零 `Cargo.toml` 依赖变化。

## MODIFIED Requirements

### Requirement: 工具 crate `archi-api-gen` 落位与契约面依赖扩展

模块 API 契约的**CLI 驱动面** SHALL 落位独立工具 crate `crates/tools/archi-api-gen`（crate 裸名 `archi-api-gen`，workspace 内唯一）。本能力只登记其落位、依赖方向、边界归类与写盘实现的外置；契约格式、判据、生成语义与写盘单点分别归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-contract-panel`。

- **落位与职责**：承载 CLI 三子命令（`generate` / `--check` / `bootstrap`）与三子命令的**编排**（`generate` = 渲染 + 写盘单点；`--check` = 比对 + 打印；`bootstrap` = 草稿文本）。判定（契约解析 / `$defs` 规则 / 十条判据 / 渲染出字符串）MUST NOT 在本 crate 实现——全部在 `crates/core/archi/src/api/**`；**写盘实现同样 MUST NOT 在本 crate**——单点驻 `crates/infra/archi`。它存在的唯一理由是「CLI 入口 + 子命令编排」：`archi` 的零写盘纪律不动，写盘单点唯一性由 `infra/archi` 承担。
- **workspace 内依赖**：恰为 `archi`（契约解析 / 装配 / 判据 / 渲染 / 比对）+ `foundation`（契约资产与索引目录、生成物路径派生）+ `archi-runtime`（**唯一用途**：调生成物写盘单点 `write_artifacts`；该依赖 MUST 在 `Cargo.toml` 处带用途注释），依赖方向为 tools → core / infra 单向，MUST NOT 出现 core / infra → tools。
- **写盘经单点（不再是本 crate 的工具边界属性）**：本 crate MUST NOT 出现 `fs::write` / `create_dir_all` / `File::create`——写盘经 `archi-runtime` 的单点函数完成；该纪律 MUST NOT 被用来稀释 `archi` 的零写盘纪律，也 MUST NOT 让 app / core / infra 任何 crate 依赖本 crate。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `config`、MUST NOT 依赖 `workflow`、MUST NOT 依赖壳（`dev-team`）；**MUST NOT 直接依赖 `orchestration`**——`archi-runtime` 自带 `orchestration` 依赖（port 与产出类型），该传递边不可避免且在案留痕，本 crate 源码 MUST NOT `use orchestration::…`。进程面：本 crate 自身为 CLI 二进制，MUST NOT 派生任何子进程（零 `std::process::Command` / 零 shell）。
- **入边**：`archi-api-gen` MUST NOT 被 app / core / infra 依赖，仅经 `cargo run -p archi-api-gen`（`package.json` 的 `api:export` / `api:check`）驱动——与 `export-bindings`「不被 app 构建拖带」的裁定同式（避免 `tauri build` 把工具 bin 编进 release 产物）。新增入边 SHALL 恰为 `archi-api-gen → archi-runtime`（tools → infra 单向，唯一 tools → infra 边）。
- **`archi` crate 零新增外部依赖**：契约面所需的 `syn`（Rust 声明抽取）/ `oxc_ast` / `oxc_parser` / `oxc_span` / `oxc_syntax`（TS 声明抽取）/ `sha2`（生成物头摘要）/ `globset` / `serde_json` / `specta` 全部已在 `Cargo.lock` 且已是 `archi` 的直接依赖 ⇒ 本变更 MUST NOT 引入新外部包。`archi` 的零 `workflow` 依赖与 `workflow` ⇄ `archi` 互不依赖两条红线零变化。
- **边界分类学不增类**：生成物写盘归既有「文件边界」的工具写面（与 bindings 导出同族），MUST NOT 新增第八类边界；`infra/` 目录不新增成员（新 crate 落 `tools/`）。
- **段名字面量纪律换段**：生成器与契约面 MUST NOT 行内拼 `<archi_dir>/models/<Domain>` 与 `<archi_dir>/graph` 两个路径段——契约资产目录一律经 `foundation::layout::archi_models_dir`（该目录同时是模型目录，两个消费者共用同一派生函数，MUST NOT 新造 `archi_contract_dir` 一类别名）、索引一律经 `archi_graph_dir`（见 `workspace-layout-resolution`）。`api` 段及其派生族（`API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`）已随本次迁移删除，产品 `.rs`（`*_test.rs` 除）内 `"api"` 段字面量与三个符号零命中。该纪律与既有 `openspec` / `config.json` 双禁令同源，机械可验。
- **消费者登记**：`foundation::layout` 的 `archi_models_dir` / `archi_models_rel_dir` 由模型面与本 crate 双消费者共用，`archi_graph_dir` / `archi_graph_rel_dir` 以本 crate、`archi` 与 `infra/archi` 为消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」与「架构域 repo 相对路径派生（会话知识用）」。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/tools/archi-api-gen/Cargo.toml`、`crates/core/archi/Cargo.toml`、`crates/infra/archi/Cargo.toml` 与壳 / core / infra 各 `Cargo.toml`
- **THEN** `archi-api-gen` 的 workspace 内依赖恰为 `archi` + `foundation` + `archi-runtime`（后者带用途注释）；零 Tauri / 零 `store` / 零 `config` / 零 `workflow` / 零壳 / 零直接 `orchestration`；壳与 core / infra 各 crate 的依赖中均不出现 `archi-api-gen`；`workflow` 的依赖中不出现 `archi`

#### Scenario: 写盘单点驻 infra/archi

- **WHEN** 审查 `crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/src/**` 与 `crates/infra/archi/src/api_artifacts.rs` 的文件系统调用
- **THEN** `archi` 侧零写盘调用（`std::fs::write` / `create_dir*` / `File::create` 全无）；`archi-api-gen` 侧零写盘调用（三子命令只做编排）；写盘恰在 `infra/archi` 的 `write_artifacts` 一处（按 `GeneratedFile.path` 的父目录创建、含 `starts_with(root)` 越根复核），该层零段名知识、本变更零 diff；`archi` 与 `archi-api-gen` 两侧仍零 `std::process` / 零 `tokio::process`

#### Scenario: 零新外部依赖与零进程

- **WHEN** 比对变更前后的 `Cargo.lock` 与 `crates/core/archi/Cargo.toml`
- **THEN** `archi` 的依赖清单零新增条目（`syn` / `oxc_*` / `sha2` / `globset` / `serde_json` 均已在案）；`archi-api-gen` 自身零子进程派生（仅 CLI 入口读 argv 并调库）；workspace `members` 与 `[workspace.dependencies]` 零新增项（无新 crate）

#### Scenario: 生成器不被 app 构建拖带

- **WHEN** 审查 `package.json` 脚本与 `tauri.conf.json`
- **THEN** 生成器只经 `cargo run -p archi-api-gen` 驱动（`api:export` / `api:check`）；`tauri build` 的产物不含该 bin；`src-tauri/Cargo.toml` 的壳 `[dependencies]` 不含 `archi-api-gen`

#### Scenario: 段名禁令机械可验（换段后）

- **WHEN** 对 `crates/core/archi` / `crates/infra/archi` / `crates/tools/archi-api-gen` / `src-tauri/src/commands` 的产品 `.rs`（`*_test.rs` 除）做字面量与符号检索
- **THEN** `"api"` 路径段字面量、`archi_api_dir`、`archi_api_rel_dir`、`API_DIR_NAME` 四者零命中；`models` 段与 `graph` 段的拼接零行内命中（一律经 `foundation::layout` 派生；唯一触点仍是 `foundation/src/layout/mod.rs`）

## Module Contract

### CLI: 契约生成与门禁驱动面（零命令形态变化）

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `crates/tools/archi-api-gen/src/main.rs` + `src/main_test.rs` | 零实现改动、夹具换段 | 三子命令（`generate` / `--check` / `bootstrap`）编排与退出码语义逐字不变，落点只经 `api::paths` 与 `foundation::layout` 取得；测试的 19 处落点字面量换到 `models/<Domain>/` 与 `graph/` |
| `crates/core/foundation/src/layout/mod.rs` | 改（段名增删） | 删 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`；增 `GRAPH_DIR_NAME` / `archi_graph_dir` / `archi_graph_rel_dir`；本文件是两段名的唯一触点（见 `workspace-layout-resolution`） |
| `crates/infra/archi/src/api_artifacts.rs` | 零改动 | 写盘单点按 `GeneratedFile.path` 的父目录创建，零段名知识；`starts_with(root)` 越根复核不变 |
| `packages/desktop/package.json` | 零改动（脚本面） | `api:export` / `api:check` 两条命令串与 `check` 链四段逐字不变；只有 `version` 随桌面源码改动 +1 patch |
