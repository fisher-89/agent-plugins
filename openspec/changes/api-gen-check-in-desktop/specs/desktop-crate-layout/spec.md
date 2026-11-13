# desktop-crate-layout Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改两条 requirement，均为「写盘面搬家」的登记面随动：① `archi-api-gen` 不再承载写盘实现、新增 `archi-runtime` 入边；② `infra/archi` 追加「生成物写盘单点」职责与 `archi-api-gen → archi-runtime` 入边。其余四条 requirement（三层分组与依赖规则、`export-bindings` 落位、`archi` crate 落位、命名隔离）逐字不变。

## MODIFIED Requirements

### Requirement: 工具 crate `archi-api-gen` 落位与契约面依赖扩展

模块 API 契约的**CLI 驱动面** SHALL 落位独立工具 crate `crates/tools/archi-api-gen`（crate 裸名 `archi-api-gen`，workspace 内唯一）。本能力只登记其落位、依赖方向、边界归类与写盘实现的外置；契约格式、判据、生成语义与写盘单点分别归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-contract-panel`。

- **落位与职责**：承载 CLI 三子命令（`generate` / `--check` / `bootstrap`）与三子命令的**编排**（`generate` = 渲染 + 写盘单点；`--check` = 比对 + 打印；`bootstrap` = 草稿文本）。判定（契约解析 / `$defs` 规则 / 十条判据 / 渲染出字符串）MUST NOT 在本 crate 实现——全部在 `crates/core/archi/src/api/**`；**写盘实现同样 MUST NOT 在本 crate**——单点驻 `crates/infra/archi`。它存在的唯一理由是「CLI 入口 + 子命令编排」：`archi` 的零写盘纪律不动，写盘单点唯一性由 `infra/archi` 承担。
- **workspace 内依赖**：恰为 `archi`（契约解析 / 装配 / 判据 / 渲染 / 比对）+ `foundation`（契约目录与生成物路径派生）+ `archi-runtime`（**唯一用途**：调生成物写盘单点 `write_artifacts`；该依赖 MUST 在 `Cargo.toml` 处带用途注释），依赖方向为 tools → core / infra 单向，MUST NOT 出现 core / infra → tools。
- **写盘经单点（不再是本 crate 的工具边界属性）**：本 crate MUST NOT 出现 `fs::write` / `create_dir_all` / `File::create`——写盘经 `archi-runtime` 的单点函数完成；该纪律 MUST NOT 被用来稀释 `archi` 的零写盘纪律，也 MUST NOT 让 app / core / infra 任何 crate 依赖本 crate。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `config`、MUST NOT 依赖 `workflow`、MUST NOT 依赖壳（`dev-team`）；**MUST NOT 直接依赖 `orchestration`**——`archi-runtime` 自带 `orchestration` 依赖（port 与产出类型），该传递边不可避免且在案留痕，本 crate 源码 MUST NOT `use orchestration::…`。进程面：本 crate 自身为 CLI 二进制，MUST NOT 派生任何子进程（零 `std::process::Command` / 零 shell）。
- **入边**：`archi-api-gen` MUST NOT 被 app / core / infra 依赖，仅经 `cargo run -p archi-api-gen`（`package.json` 的 `api:export` / `api:check`）驱动——与 `export-bindings`「不被 app 构建拖带」的裁定同式（避免 `tauri build` 把工具 bin 编进 release 产物）。新增入边 SHALL 恰为 `archi-api-gen → archi-runtime`（tools → infra 单向，唯一 tools → infra 边）。
- **`archi` crate 零新增外部依赖**：契约面所需的 `syn`（Rust 声明抽取）/ `oxc_ast` / `oxc_parser` / `oxc_span` / `oxc_syntax`（TS 声明抽取）/ `sha2`（生成物头摘要）/ `globset` / `serde_json` / `specta` 全部已在 `Cargo.lock` 且已是 `archi` 的直接依赖 ⇒ 本变更 MUST NOT 引入新外部包。`archi` 的零 `workflow` 依赖与 `workflow` ⇄ `archi` 互不依赖两条红线零变化。
- **边界分类学不增类**：生成物写盘归既有「文件边界」的工具写面（与 bindings 导出同族），MUST NOT 新增第八类边界；`infra/` 目录不新增成员（新 crate 落 `tools/`）。
- **段名字面量纪律扩展**：生成器与契约面 MUST NOT 行内拼 `<archi_dir>/api` 段——一律经 `foundation::layout::archi_api_dir`（见 `workspace-layout-resolution`）。该纪律与既有 `openspec` / `config.json` 双禁令同源，机械可验。
- **消费者登记**：`foundation::layout` 的 `archi_api_dir` / `archi_api_rel_dir` 以本 crate 与 `archi` 为消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」与「架构域 repo 相对路径派生（会话知识用）」。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/tools/archi-api-gen/Cargo.toml`、`crates/core/archi/Cargo.toml`、`crates/infra/archi/Cargo.toml` 与壳 / core / infra 各 `Cargo.toml`
- **THEN** `archi-api-gen` 的 workspace 内依赖恰为 `archi` + `foundation` + `archi-runtime`（后者带用途注释）；零 Tauri / 零 `store` / 零 `config` / 零 `workflow` / 零壳 / 零直接 `orchestration`；壳与 core / infra 各 crate 的依赖中均不出现 `archi-api-gen`；`workflow` 的依赖中不出现 `archi`

#### Scenario: 写盘单点驻 infra/archi

- **WHEN** 审查 `crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/src/**` 与 `crates/infra/archi/src/api_artifacts.rs` 的文件系统调用
- **THEN** `archi` 侧零写盘调用（`std::fs::write` / `create_dir*` / `File::create` 全无）；`archi-api-gen` 侧零写盘调用（三子命令只做编排）；写盘恰在 `infra/archi` 的 `write_artifacts` 一处（含 `starts_with(root)` 越根复核）；`archi` 与 `archi-api-gen` 两侧仍零 `std::process` / 零 `tokio::process`

#### Scenario: 零新外部依赖与零进程

- **WHEN** 比对变更前后的 `Cargo.lock` 与 `crates/core/archi/Cargo.toml`
- **THEN** `archi` 的依赖清单零新增条目（`syn` / `oxc_*` / `sha2` / `globset` / `serde_json` 均已在案）；`archi-api-gen` 自身零子进程派生（仅 CLI 入口读 argv 并调库）；workspace `members` 与 `[workspace.dependencies]` 零新增项（无新 crate）

#### Scenario: 生成器不被 app 构建拖带

- **WHEN** 审查 `package.json` 脚本与 `tauri.conf.json`
- **THEN** 生成器只经 `cargo run -p archi-api-gen` 驱动（`api:export` / `api:check`）；`tauri build` 的产物不含该 bin；`src-tauri/Cargo.toml` 的壳 `[dependencies]` 不含 `archi-api-gen`

### Requirement: infra/archi crate 落地登记与 archi 依赖扩展

架构漂移判定域的执行面 SHALL 落位 `crates/infra/archi`（crate 裸名 `archi-runtime`），并在本能力登记其落位、依赖与边界归类：

- **落位与职责**：承载 `ArchiCheckRunner` 实现（取输入 → 调 `archi` 判定纯函数 → 落 `reports/archi` 报告 → 派生 outcome）与报告落盘单点；并承载**生成物写盘单点**（`write_artifacts`：越根复核 + 建目录 + 写文件，与报告落盘并列为本 crate 两处写盘单点，两处都只是 dumb IO，零判定）。领域判定（扫描 / 归属 / 定级 / 报告结构 / 配置读取 / 契约判据 / 生成物渲染与比对）MUST NOT 在此重复——全部在 `crates/core/archi`（见 `desktop-archi-drift` / `desktop-archi-contract-panel`）。
- **workspace 内依赖**：`archi`（判定 / 报告结构 / 生成物渲染与比对）+ `orchestration`（port 与产出类型，与 `checks-runtime` 依赖 `orchestration` 同构）+ `foundation`（报告目录派生）。依赖方向 SHALL 为 infra → core 单向，MUST NOT 出现 core → infra。依赖清单零新增（写盘单点不引入新依赖）。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `vcs-runtime`（变更文件清单经 port 注入，见 `desktop-archi-gate`「门禁输入口径与观察根」）、MUST NOT 依赖 `config`、MUST NOT 依赖 `workflow`、MUST NOT 创建任何进程（零 `std::process` / 零 `tokio::process` / 零 shell）。
- **入边**：新增入边 SHALL 为两条——`desktop-app → archi-runtime`（壳层装配注入 runner + 两条契约面命令调写盘单点，MUST 在 `Cargo.toml` 处带用途注释）与 `archi-api-gen → archi-runtime`（tools 驱动面调写盘单点，同样带用途注释）；`archi-runtime` MUST NOT 被 `workflow` / `archi` / `agent-runtime` / `checks-runtime` 依赖（agent 侧只经 `core/archi` 的只读面消费契约事实，见 `desktop-archi-agent`）。
- **`archi` crate 依赖扩展**：`crates/core/archi` SHALL 新增外部依赖 `oxc_parser` / `oxc_allocator` / `oxc_resolver`（TS / JS 词法与模块解析）与 `syn`（Rust 语法与 `use` 树），并把 `globset` 由传递依赖提为直接依赖（排除匹配）；这些依赖 MUST NOT 引入进程、Tauri、`config` 或 `workflow` 触点，`archi` 仍保持零 `workflow` 依赖（`workflow` ⇄ `archi` 互不依赖为硬红线，双向机械可验）。
- **边界分类学不增类**：本执行面零新外部依赖（`serde_json` 为锁内既有），归既有「文件边界」的读盘面 + 检查结论落盘（报告树）+ 生成物写盘（工具同族写面），MUST NOT 新增第八类边界；`infra/` 目录不新增除 `archi` 外的成员。
- **依赖成本护栏**：`Cargo.lock` 净新增包数 SHALL ≤ 40（软线 25）；超出即视为特性开关引入面失控，先收紧 `oxc` 特性再复测（成本实测与回退序见 `desktop-archi-drift`「运行期预算与语料」）。

#### Scenario: 依赖面与方向审查

- **WHEN** 审查 `crates/infra/archi/Cargo.toml`、`crates/core/archi/Cargo.toml` 与壳 `Cargo.toml`
- **THEN** `archi-runtime` 的 workspace 内依赖恰为 `archi` / `orchestration` / `foundation`（本变更零新增）；`archi` 的 workspace 内依赖仍仅 `foundation`；壳与 `archi-api-gen` 各含 `archi-runtime = { workspace = true }` 且带用途注释；`workflow` 依赖中不出现 `archi`，`archi` 依赖中不出现 `workflow`

#### Scenario: 零进程零 Tauri 审查

- **WHEN** 审查 `crates/infra/archi` 与 `crates/core/archi` 的进程触点与框架依赖
- **THEN** 两者均无进程创建、无 shell 调用、无 Tauri 依赖；`archi-runtime` 零 `store` / 零 `config` / 零 `vcs-runtime` 依赖；`agent-runtime` 的依赖中不出现 `archi-runtime`（写盘单点对 agent 面不可达）

#### Scenario: 依赖成本不越硬线

- **WHEN** 完成 `oxc_*` / `syn` 引入并执行成本实测（锁包数 / 安装包 / 主二进制 / 编译用时）
- **THEN** `Cargo.lock` 净增 ≤ 40 包；任一指标越硬线时按回退序（减特性 → 自算路径 → 正则 import）处置并留痕，MUST NOT 直接放宽阈值

## Module Contract

### CLI: `crates/tools/archi-api-gen`

| 项 | 变化 | 描述 |
|---|---|---|
| `Cargo.toml` | 改 | `[dependencies]` 加 `archi-runtime = { workspace = true }`（用途注释：调生成物写盘单点）；零新增外部包；零子进程 |
| `src/main.rs` | 收窄 | 只剩 argv 解析 + 退出码 + 三子命令编排；零 `fs::write` / 零本地 walker |
| 不被依赖 | 零改动 | 壳 / core / infra 各 `Cargo.toml` 均不出现 `archi-api-gen` |

### API: `crates/infra/archi`

| 项 | 变化 | 描述 |
|---|---|---|
| `src/api_artifacts.rs` | 新增 | `write_artifacts(root, &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`：越根复核 + 建目录 + 读盘比对后写；不删文件、不回滚 |
| `src/report.rs` | 零改动 | 报告落盘单点逐字不变（两处写盘单点并列） |
| 依赖面 | 零改动 | `archi` / `orchestration` / `foundation` + `serde_json`；零 Tauri / 零 spawn / 零 `store` / 零 `config` / 零 `workflow` |
| 入边 | 追加一条 | `archi-api-gen → archi-runtime`（tools 驱动面）；既有 `desktop-app → archi-runtime` 保留 |
