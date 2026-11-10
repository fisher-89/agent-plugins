# desktop-crate-layout Specification（delta: module-api-contract-format）

## ADDED Requirements

### Requirement: 工具 crate `archi-api-gen` 落位与契约面依赖扩展

模块 API 契约的**唯一写盘面** SHALL 落位独立工具 crate `crates/tools/archi-api-gen`（crate 裸名 `archi-api-gen`，workspace 内唯一）。本能力只登记其落位、依赖方向、允许写盘的理由与边界归类；契约格式、判据与生成语义归 `desktop-archi-api` / `desktop-archi-api-check`。

- **落位与职责**：承载 CLI 三子命令（`generate` / `--check` / `bootstrap`）与生成物写盘。判定（契约解析 / `$defs` 规则 / 十条判据 / 渲染出字符串）MUST NOT 在本 crate 实现——全部在 `crates/core/archi/src/api/**`。它存在的唯一理由是「生成物要写盘」与「工具需要 CLI 入口」：`archi` 的零写盘纪律不动。
- **workspace 内依赖**：恰为 `archi`（契约解析 / 装配 / 判据 / 渲染）+ `foundation`（契约目录与生成物路径派生），依赖方向为 tools → core 单向，MUST NOT 出现 core → tools。
- **允许写盘（工具边界，正当属性）**：本 crate SHALL 允许写文件系统（生成物 `*.schema.json` / `INDEX.toml` / `*.i.rs` / `*.i.ts` 与目录创建），与既有 `export-bindings`（写 `src/types/generated/**`）同族。该许可 MUST NOT 被用来稀释 `archi` 的零写盘纪律，也 MUST NOT 让 app / core / infra 任何 crate 依赖本 crate。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `config`、MUST NOT 依赖 `orchestration`、MUST NOT 依赖 `workflow`、MUST NOT 依赖任何 infra crate、MUST NOT 依赖壳（`dev-team`）。进程面：本 crate 自身为 CLI 二进制，MUST NOT 派生任何子进程（零 `std::process::Command` / 零 shell）。
- **新增入边**：无。`archi-api-gen` MUST NOT 被 app / core / infra 依赖，仅经 `cargo run -p archi-api-gen`（`package.json` 的 `api:export` / `api:check`）驱动——与 `export-bindings`「不被 app 构建拖带」的裁定同式（避免 `tauri build` 把工具 bin 编进 release 产物）。
- **`archi` crate 零新增外部依赖**：契约面所需的 `syn`（Rust 声明抽取）/ `oxc_ast` / `oxc_parser` / `oxc_span` / `oxc_syntax`（TS 声明抽取）/ `sha2`（生成物头摘要）/ `globset` / `serde_json` / `specta` 全部已在 `Cargo.lock` 且已是 `archi` 的直接依赖 ⇒ 本变更 MUST NOT 引入新外部包。`archi` 的零 `workflow` 依赖与 `workflow` ⇄ `archi` 互不依赖两条红线零变化。
- **边界分类学不增类**：生成物写盘归既有「文件边界」的工具写面（与 bindings 导出同族），MUST NOT 新增第八类边界；`infra/` 目录不新增成员（新 crate 落 `tools/`）。
- **段名字面量纪律扩展**：生成器与契约面 MUST NOT 行内拼 `<archi_dir>/api` 段——一律经 `foundation::layout::archi_api_dir`（见 `workspace-layout-resolution`）。该纪律与既有 `openspec` / `config.json` 双禁令同源，机械可验。
- **消费者登记**：`foundation::layout` 的 `archi_api_dir` / `archi_api_rel_dir` 以本 crate 与 `archi` 为消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」与「架构域 repo 相对路径派生（会话知识用）」。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/tools/archi-api-gen/Cargo.toml`、`crates/core/archi/Cargo.toml` 与壳 / core / infra 各 `Cargo.toml`
- **THEN** `archi-api-gen` 的 workspace 内依赖恰为 `archi` + `foundation`；零 Tauri / 零 `store` / 零 `config` / 零 `orchestration` / 零 `workflow` / 零 infra crate / 零壳；壳与 core / infra 各 crate 的依赖中均不出现 `archi-api-gen`；`workflow` 的依赖中不出现 `archi`

#### Scenario: 写盘仅限工具边界

- **WHEN** 审查 `crates/core/archi/src/api/**` 与 `crates/tools/archi-api-gen/src/**` 的文件系统调用
- **THEN** `archi` 侧零写盘调用（`std::fs::write` / `create_dir*` / `File::create` 全无），写盘只出现在工具 crate；`archi` 侧仍零 `std::process` / 零 `tokio::process`

#### Scenario: 零新外部依赖与零进程

- **WHEN** 比对变更前后的 `Cargo.lock` 与 `crates/core/archi/Cargo.toml`
- **THEN** `archi` 的依赖清单零新增条目（`syn` / `oxc_*` / `sha2` / `globset` / `serde_json` 均已在案）；`archi-api-gen` 自身零子进程派生（仅 CLI 入口读 argv 并调库）

#### Scenario: 生成器不被 app 构建拖带

- **WHEN** 审查 `package.json` 脚本与 `tauri.conf.json`
- **THEN** 生成器只经 `cargo run -p archi-api-gen` 驱动（`api:export` / `api:check`）；`tauri build` 的产物不含该 bin；`src-tauri/Cargo.toml` 的壳 `[dependencies]` 不含 `archi-api-gen`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/tools/archi-api-gen/Cargo.toml`（新） | 工具 crate 落位与依赖 | 裸名 `archi-api-gen`；workspace 内依赖恰为 `archi` + `foundation`；外部 `serde_json`；零 Tauri / 零 store / 零 config / 零 orchestration / 零 workflow / 零壳 |
| `crates/tools/archi-api-gen/src/main.rs`（新） | CLI 唯一入口 | 三子命令 `generate` / `--check` / `bootstrap`；`--check` 零写入、有差异即非零退出并列文件；目录创建经 `foundation::layout` 派生；零判定逻辑 |
| `src-tauri/Cargo.toml` | workspace 成员登记 | `members` 增 `crates/tools/archi-api-gen`；`[workspace.dependencies]` 增 `archi-api-gen = { path = "crates/tools/archi-api-gen" }`（入口注释说明「CLI 工具，不被 app 依赖」）；壳 `[dependencies]` MUST NOT 增该 crate |
| `packages/desktop/package.json`（脚本面） | 驱动入口 | `api:export` = `cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`（写盘）；`api:check` = 同一调用加 `--check`（零写入、工具内比对，覆盖四类生成物：`<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）；`check` 链在 `bindings:export` 之后插 `api:check` |
| `crates/core/archi/Cargo.toml` | 依赖面零新增 | 契约面无需新外部依赖；零 Tauri / 零 store / 零 spawn / 零 `workflow` 四条结论零变化 |
