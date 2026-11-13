# 设计: api-gen-check-in-desktop

> **变更**: api-gen-check-in-desktop
> **日期**: 2026-11-13
> **基于**: `proposal.md`（问题 / 提案一~八 / 六项裁决 / 变更范围 / AC-1…AC-19 / R-1…R-16）、`explore.md`（只读，482 行：两轴病灶 + P1/U1/③ 裁决 + 硬线一/二）与十份 delta spec（`desktop-archi-contract-panel` 6 ADDED；`desktop-archi-model` 1 RENAMED；`desktop-archi-api-check` / `desktop-archi-drift` / `desktop-archi-api` ×2 / `desktop-crate-layout` ×2 / `desktop-archi-canvas` ×2 / `desktop-archi-file-tree` / `desktop-ipc-type-bindings` 各 1 MODIFIED；`desktop-archi-agent` 1 ADDED + 1 RENAMED + 3 MODIFIED）
> **性质**: 把「契约生成物的检查 / 生成」两面从 CLI 单点接进桌面端——判定与比对留在 `core/archi`（纯读盘），写盘单点搬 `infra/archi`，壳层加两条 IPC，前端左栏加第三视图「契约」，`apiFindings` 由「报告里无人看」变「状态词 + 面板 + agent 自检」三面可见。C 面（独立门禁步）与生成物写保护本轮不做。

---

## 提案与规格同步状态

proposal 的范围裁决（A + B + D、P1 写盘归属、③ 契约在案即纳管、U1 生成权限、本轮不锁生成物）与十份 delta spec 均已定稿。本设计不重述论证，只做五件事：

1. 把 spec 已钉语义、未钉**代码形态**的地方收敛到可编译的具体签名与落点：「比对面」如何在**不碰 `scan/**`** 的前提下换成 `ignore` 感知遍历（D3）、写盘单点如何与既有私有模块共存（D10）、纳管子集与判据装配如何各只有一个落点（D6 / D7）、两条 IPC 与 CLI 如何共用同一实现（D1 / D2 / D11）。
2. 把 proposal 与代码实况的四处偏差摆到「事实校准」并给出裁定（`REPORT_VERSION` 实际落点、`archi` crate 头注释、`sdk/tools.rs` 的两行面、壳 `Cargo.toml` 用途注释）。
3. 逐条 AC 给落点（「验收标准回应」表），**不写测试策略**（用例编写与门禁执行由独立相位承接）。
4. 把三处必须显式留痕的语义收紧写成可实现形态：`.gitignore` 命中不再被扫（R-1）、`enabled = false` 不得渲成空面板（R-11）、写盘非原子且从不删文件（R-5 / R-6）。
5. 把**登记面归属**收敛干净：两类测试语料（16 份漂移金样 + `01-app.toml` 注释面）不属实现变更（事实校准 10），只登记为「测试相位承接面」并移交 test-execution；`src/**` 侧两份测试文件（`main_test.rs` / `element-drawer.test.tsx`）只作为**编译闭合面**登记进「修改文件」表、与 `bindings/mod_test.rs` 同族（事实校准 11），用例本体仍归测试相位；面板「检查失败」态的状态词口径按事实校准 9 订正（D18）。

### 起点实况核实（工作树实测；Rust 侧前缀 `packages/desktop/src-tauri/`，前端侧 `packages/desktop/src/`）

- **比对 / 写盘面（全在工具内）**：`crates/tools/archi-api-gen/src/main.rs`（301 行）——`run` `:48`、`repository_root` `:94`、`collect` `:109`（`model_elements` + `load_contracts` + `render_all`）、`write` `:122`（`starts_with(root)` 越根复核 + `create_dir_all` + `fs::write`）、`struct Report` `:139`、`differences` `:161`、`generated_files_on_disk` `:202`、`is_generated_module_file` `:222`、`walk_modules` `:229`、`walk` `:263`、`SKIPPED_DIRECTORIES` `:286`、`MAX_MODULE_WALK_DEPTH` `:289`、`model_elements` `:292`。
- **判定面**：`crates/core/archi/src/api/mod.rs`（887 行）——`ApiError` 三值 `:42`、`ApiMeta` `:83`、`GeneratedFile { path, content }` `:98`、`ApiCheckInput` `:121`、`meta()` `:141`、`check(ApiCheckInput)` 稳定排序 `:233`、`enabled` `:250`、`load_contracts` `:293`、`render_all` `:349`（schema → `.i.*` → `INDEX.toml` 末位）、`bootstrap` `:533`、`api_ref_of` `:685`、私有 `elements_of` `:785`（结构诊断 ⇒ `ApiError::Element`）、`display` `:847`、`MAX_API_WALK_DEPTH` 私有常量。
- **漂移判定**：`crates/core/archi/src/drift/mod.rs` ——`REPORT_VERSION: u32 = 2` **在 `mod.rs:40`**（不在 `report.rs`）、契约判据内联装配（`enabled` → `load_contracts` → `check`）、`derive_status` 四入参、`skipped` 早退臂构造 `api_findings: Vec::new()`；`drift/report.rs` 的 `api_findings` 字段 doc 明写「首期 MUST NOT 影响 `status` / `passed`」。
- **报告 / 语料**：`tests/corpus/drift/expected/*.json` 16 份（`api-mismatch.full.json` 现状 `status = "clean"` + 1 条 `apiSignatureMismatch`）；`DESKTOP_GOLDEN_REWRITE=1` 为显式重写闸。
- **infra 写盘面**：`crates/infra/archi/src/lib.rs` 26 行（`#[path] mod api_gen; pub use api_gen::*;` + **私有** `mod report; mod runner;`）；`report::write_report` 是既有唯一写盘函数；`Cargo.toml` 已有 `tempfile` dev-dependency。
- **命令面**：`src/commands/archi/mod.rs` 206 行（六命令 + 六 `*_inner`、`check_root`、`write_atomic`）；`src/commands/mod.rs` archi 段 `archi_file_tree` 之后即 `read_file`（尾条）。
- **清单守卫**：`src/bindings/mod_test.rs` ——`COMMAND_NAMES` / `COMMAND_WRAPPERS` 51 条（三处 `len()` 断言 + `:1991` / `:2469` 说明串）、`DTO_TYPES` 101 条（`:1144`）、`ARCHI_COMMANDS: [(&str, &str); 6]`（`:1688`）、`ARCHI_DTO_TYPES: [&str; 23]`（`:1698`）、尾部追加序期望 vec（`:1765`）、族末条对照臂 `清单遗漏即产物覆盖断言必红`。
- **agent 工具面**：`crates/infra/agent/src/sdk/mount.rs`（`TOOL_NAMES` 七件 + `EXPLORE_NOTE_TOOL` + `ARCHI_MODULE_TOOL` / `ARCHI_SEARCH_TOOL`，静态清单 10 项；`ToolTier::Read` 注释列两工具）、`sdk/tools.rs`（`:9` 再导出两常量、`input_path` 对两 archi 工具返回 `None`、`cap_result` 为 L1 上限单点）、`sdk/archi_tool.rs`（`ArchiContext::execute` 两臂）、`sdk/runner.rs`（`mint_archi_context` present 判据两名字）、`compose.rs`（`archi_matrix` 七行 + archi 来源臂两名字）、`archi-knowledge.md`（61 行，自检纪律两条）。
- **前端**：`views/architecture/components/architecture-canvas.tsx` 352 行（`CanvasBody` 持 `treeMode` / `useArchiFileTree` / 三态行；`ArchitectureCanvas` 已取 `useArchiDrift` 的 `report` 但 `CanvasBody` 只收 `driftError`）、`file-tree.tsx` 的 `TreeMode = 'file' | 'element'` 与 `TreeModeSwitch` 两钮选项表、`element-drawer.tsx` 287 行（概览 → 文档 → 豁免 → 改档，`module.detail.api` **从未被读**）、`hooks/use-archi-drift.ts`（`loadedKey` 失败清空范式）。
- **文档 / 版本**：`AGENT.md` / `CLAUDE.md` 的 `## 模块 API 契约` 第 ⑤ 条（两处逐字相同）；`packages/desktop/package.json` `version` = `0.6.39`；`knip.json` 已含 `src/**/*.i.ts` ignore。

### 事实校准（不改 spec 意图，只定实现形态）

1. **`REPORT_VERSION` 在 `drift/mod.rs` 而非 `drift/report.rs`**（proposal 两处行文写的落点是 `report.rs`）。裁定：常量改在 `drift/mod.rs:40`；`drift/report.rs` 仍改（`api_findings` 字段 doc 的「首期 MUST NOT 影响 `status` / `passed`」必须改述为纳管口径），两文件都在 proposal 的实现文件表内，语义零偏差。
2. **`sdk/tools.rs` 需两行随动**（proposal 未列）：`pub(crate) use crate::sdk::mount::{…}` 的名单要含 `ARCHI_API_TOOL`（`archi_tool` 经此引用名字常量），`input_path` 的「archi 工具无路径参数 ⇒ `None`」显式分支要含第三名（缺了功能上仍 `None`，但注释与分支的「三工具」表述就与 spec 不符）。裁定：两行改，零行为变化。
3. **壳 `src-tauri/Cargo.toml` 的 `archi-runtime` 用途注释需补一句**（proposal 未列，`desktop-crate-layout` 明文要求「壳层 … 两条契约面命令调写盘单点，MUST 在 `Cargo.toml` 处带用途注释」）。裁定：只改注释行，`[dependencies]` 条目与 crate 版本行零改动。
4. **`archi` / 工具 / 壳三处头注释已失真**：`crates/core/archi/src/lib.rs:15-17`（「生成物的唯一写盘面在工具 crate」）、`api/mod.rs:5-8`（同）、`drift/mod.rs:9`（「`api_findings` 不参与 `status` 推导」）、`archi-api-gen/src/main.rs` 与 `Cargo.toml` 头注释（「CLI 唯一写盘面」）。裁定：随本变更逐处订正——注释即登记面，留着就是下一轮的错误前提。
5. **`api::elements_of` 私有但可被子模块直达**：`api::render` / `api::diff` 作为 `api` 的子模块可 `super::elements_of` 调用（Rust 私有可见性向下开放）。裁定：**不提权**，`render.rs` 只调 `super::`，`elements_of` 签名与语义零改。
6. **`core/archi` 无 `tempfile` dev-dependency**（`api/mod_test.rs` 文件头明写「本 crate 无 dev-dependency、自建 `TempDir` RAII」）。裁定：`render_test.rs` / `diff_test.rs` 沿用既有 `TempDir` RAII 先例；`infra/archi` 已有 `tempfile`，`api_artifacts_test.rs` 直接用。
7. **新增 `pub fn write_artifacts` 不产生契约判据**：`App.Backend.Infra.ArchiRuntime` 契约只声明 `file_stem` 与类型；`apiUndeclared`（代码多出导出）当前 `severity() == Off` ⇒ 判据 ③ 直接跳过。故 AC-8 的「本仓 41 条不变、契约在案元素 0 条」成立，`.dev-team/architecture/api/**` 零 diff（AC-19）成立。
8. **`archi-tool` 的结构错文案单点**：`dto::graph_diagnostics_error` 现为 `pub(crate)`（`tree.rs` / `api/mod.rs` 两处消费）。agent 侧 `archi_api(check)` 也要同一句，裁定：提为 `pub` 并在 `lib.rs` 逐名导出（零第二份 `file:line` 拼装）；该函数是纯函数，出线零成本。
9. **`contract-panel.tsx` 的 `statusText` 判定序需订正**（回溯暴露）：现实现以 `loading || check === null` 触发「解析中…」，而检查失败时 `check` 恰为 `null`、`loading` 为 `false` ⇒ 失败态与 `archi-contract-error` 并行渲出「解析中…」，状态词与错误块自相矛盾（spec 五态表里 `loading` 与「检查 invoke reject」本是两态）。裁定：`statusText` 增 `error` 入参，判定序改为 `root === null` → `error !== null`（返回 `null`，成因只由错误块承载）→ loading → `enabled` → 零差异零判据 → `null`（D18）。
10. **两类测试语料不属实现变更**（回溯暴露）：`tests/corpus/drift/expected/*.json`（16 份）与 `.../api-mismatch/models/01-app.toml` 曾被登记进「修改文件」表，但实现相位在 `tests/**` 上零读零写（测试面归 test-gen / test-execution）⇒ 该登记会造出必勾而不可完成的任务。裁定：从「变更清单」移出，期望与承接方改登记于「测试相位承接面」；AC-7 的取值仍由本设计口径决定，落盘与回归由 test-execution 闭环。
11. **两份测试文件的编译闭合面属实现变更**（回溯暴露）：`crates/tools/archi-api-gen/src/main_test.rs`（11 项私有 helper 删除后 `super::collect` / `super::differences` / `super::write` / `Report::to_json` 四个调用点不再编译）与 `packages/desktop/src/views/architecture/components/element-drawer.test.tsx`（`ElementDrawerProps.findings` 为新增必填 prop，共享 `archi_module` mock 需补 `api` 投影）分别是 `cargo check --all-targets` 与 `tsc --noEmit` 的**编译闭合面**——不修则本设计为自己设的编译自查门（各阶段末尾）无法跑绿。裁定：与 `src-tauri/src/bindings/mod_test.rs`（既有「机械守卫」先例）同族，**三份一律登记进「修改文件」表并各配一条 tasks 条目**；射程硬界定为「让既有用例编译通过的最低机械改动」（改调用点 / 补必填 prop / 补 mock 字段 / 订正指向已删 helper 的 doc 表述），**零用例新增、零断言删除、零断言语义改写**。`tests/**`（含两份语料）仍零读写——两份文件都在 `src/` 侧，该硬线不动摇。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 渲染（应生成物）单点 | 新建 `api/render.rs`：`pub fn workspace_render(root: &Path) -> Result<Vec<GeneratedFile>, ApiError>` = `super::elements_of(root)?` → `super::load_contracts(root, &elements)?` → `super::render_all(root, &snapshot)` | 三者都已在 `core/archi`，上移只是把 CLI `collect` 的三行搬进 crate；`elements_of` 已把「结构诊断非空」映射为 `ApiError::Element`（fail-closed 现成），坏模型零写盘由它免费得到 |
| D2 | 比对单点 | 新建 `api/diff.rs`：`pub fn compare(root: &Path) -> Result<ArtifactDiff, ApiError>` = `super::workspace_render(root)?` ⊕ `generated_files_on_disk(root)`；三列串一律 `super::display(root, &path)` | 「check 即生成预览」要求 check 与 generate 用同一份应生成集，故 `compare` 内部就调 `workspace_render`（同一函数、零第二份 collect）；路径归一沿用 D55 的唯一单点 |
| D3 | 盘上生成物枚举（walker 更换） | `.i.rs` / `.i.ts`：经**既有** `scan::walk_source_files(root)` 取全部 `.rs`/`.ts`/`.tsx`/`.js`/`.jsx`，再按命名判据（`ends_with(".i.rs") \|\| ends_with(".i.ts")`）过滤 → `sort` + `dedup`；`.schema.json` / `INDEX.toml`：只在 `super::paths::directory(root)` 内深度受限枚举（复用 `super::MAX_API_WALK_DEPTH`），命中判据沿用 CLI 原文（`*.schema.json` 或 `== index_path`） | `scan::walk_source_files` 的 doc 明写「MUST NOT 复制第二份遍历」，且它已是 gitignore 感知 + 剪 `target`/`node_modules`/`dist`/`build`/`.git` + 跳过隐藏项 ⇒ 手写递归三件套（`walk` / `walk_modules` / `SKIPPED_DIRECTORIES` / `MAX_MODULE_WALK_DEPTH`）整体删除；契约目录枚举是**有界本地枚举**（与既有 `api::has_markdown` 同形、同深度常量），不构成第二份仓库遍历 |
| D4 | 三列次序与确定性 | `mismatched` / `missing` 按 `workspace_render` 的输出序（契约 FQN 序 → 各元素 `.i.*` → `INDEX.toml` 末位）；`orphaned` 按路径字节序；两驱动面（CLI / IPC）逐条相等 | `--check` 的 stdout 单行 JSON 要可逐字 diff；同一稳定序是「两驱动面逐字段同源」的可验形态，不另立排序键 |
| D5 | `artifact_check` 投影单点 | `pub fn artifact_check(root: &Path) -> Result<ArchiApiCheck, ApiError>` = `compare(root)?` + `enabled(root)` + `meta()` 的三 DTO 投影（命令层零投影）；`common_digest` 出**全量** `sha256:<64hex>`（截断只发生在生成物文本，见 `short_digest`），面板自行截断显示 | spec 明写命令层 MUST NOT 承载投影；`meta.common_digest` 的全量/短形分歧一旦落到两处必然漂移，故 DTO 只搬原值 |
| D6 | 纳管子集单点 | `pub fn gating_findings(root, elements: &[ArchiElement], findings: &[ArchiApiFinding]) -> Vec<ArchiApiFinding>`：先筛「`element.api == Declared` 且 `paths::contract_path(root, fqn).is_some_and(\|p\| p.is_file())`」的 FQN 集合，再按 `finding.element` 属于该集合过滤（`element = None` 落选）；`drift` 侧零 `is_file` 调用 | 「契约文件在案」判据与 `api_ref_of` 同源（同一 `contract_path` + `is_file`）；把 `is_file` 关进 core 单点是 AC-6 的机械判据（`grep` 可验 `drift` 零 `is_file`） |
| D7 | 判据装配单点（proposal 之外的微差） | `api/mod.rs` 增第 5 个出口 `pub fn findings(root: &Path, elements: &[ArchiElement]) -> Result<Vec<ArchiApiFinding>, ApiError>`（`enabled` 短路 → `load_contracts` → `check`）；`drift::check` 的内联装配段替换为它，agent 工具复用同一函数 | 工具面要的是「同一份判据事实」；不抽这一层就会有第二份「契约输入装配」（`drift` 一份、`archi_tool` 一份），判定单源的文件级判据当场破。净增 15 行、净删 15 行，行为逐字等价（错误映射仍 `CheckError::Config`） |
| D8 | 报告版本递增 | `drift/mod.rs` 的 `REPORT_VERSION` 2 → 3，注释改述三点兼容（`1` 无字段 / `2` 有字段但 `status` 不读判据 / `3` 纳管进 `status`）；`drift/report.rs` 的 `api_findings` 字段 doc 同步改述 | 字段零改但**推导输入**改了 ⇒ 消费方按 `version` 分支是既有纪律；两处 doc 是同一口径的两面，必须同批 |
| D9 | `status` 推导 | `derive_status` 增第 5 入参 `gating: &[ArchiApiFinding]`，`warnings` 分支改 `!warnings.is_empty() \|\| !gating.is_empty()`；`skipped` / `errors` / `noChanges` 三值序与 `passed` 派生面（`LocalArchiCheck`）逐字不动 | `gating` 只可能把 `clean` 抬到 `warnings`，永不出 `errors` ⇒ `status == errors ⟺ passed == false` 恒成立（AC-6 的机械前提） |
| D10 | 写盘单点 | 新建 `crates/infra/archi/src/api_artifacts.rs`：`pub fn write_artifacts(root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`——逐份 ① `starts_with(root)` 越根复核（第二层，越根即 `Err` 该份不写）② `fs::read_to_string` 比对，字节相同计入 `unchanged` 且**不重写**（mtime 不动）③ 否则 `create_dir_all(parent)` + `fs::write` 计入 `written`；**无删除分支**、单份失败即 `Err` 且**不回滚**已写部分；两列串经 `archi::display`；`lib.rs` 以 `pub mod api_artifacts;` 出线（`report` / `runner` 仍私有） | 与 `report::write_report` 并列为本 crate 第二处写盘单点，两处都是 dumb IO 零判定；先比对后写让「幂等且不动 mtime」在实现层自然成立；`pub mod` 是 crate 内既有「一个模块一个职责」的直线做法，避免为单个函数造 `pub use` 再导出层 |
| D11 | CLI 收窄 | `main.rs` 只留 argv 解析 + 退出码 + 三子命令编排：`Generate` → `api::workspace_render` + `archi_runtime::api_artifacts::write_artifacts`（stdout 报 `written` / `unchanged` 计数）；`Check` → `api::compare`（三列全空 → 0；否则单行 JSON 三键 → 1）；`Bootstrap` 逐字不变；`repository_root` 保留；`--check` 的 JSON 由 `serde_json::json!({"mismatched":…, "orphaned":…, "missing":…})` 组（键名与序同今日） | 子命令面、退出码语义（0 / 1 / 2）与「零子进程」纪律逐字不变；单行 JSON 的键名是脚本面契约，换实现不改形状 |
| D12 | 两条 IPC | `archi_api_check(root: String) -> Result<ArchiApiCheck, String>` = `check_root` → `archi::api::artifact_check` → `ApiError::message()`；`archi_api_generate(root: String) -> Result<ArchiApiWriteOutcome, String>` = `check_root` → `archi::api::workspace_render` → `archi_runtime::api_artifacts::write_artifacts`；各配一个 `*_inner(root: &Path)`；`commands/mod.rs` 的 archi 段插两条（`archi_file_tree` 之后、`read_file` 之前） | 沿既有「三件事纪律」与 `*_inner` 离 Tauri 可测形态；命令层零渲染 / 零比对 / 零路径归一 / 零写盘（写盘只在 infra） |
| D13 | `archi_api` 工具响应形状 | `check` 返回 `{"artifacts": <ArchiApiCheck>, "findings": {"total": n, "byCategory": {category: n}, "items": [<ArchiApiFinding>]}}`；`bootstrap` 返回 `api::bootstrap` 文本；两动作输出统一 `serde_json::to_string` → `tools::cap_result` | 「复用既有出线形状」= 直接嵌 `ArchiApiCheck` 与 `ArchiApiFinding` 的序列化形态，工具侧零字段投影；`byCategory` 由 `category_of` 已打好的 `category` 字段聚合（零新词表）；摘要与逐条两级让 `cap_result` 截断时优先保住计数；成本如实留痕——`check` 走三次装配（`graph_view` 取元素表 + `artifact_check` 的渲染 + `findings` 的契约装载），只在 agent 按需调用路径发生，IPC 面板路径仍是一次渲染（R-7 的预算约束面不含工具路径） |
| D14 | 工具面接入点 | `mount.rs`：`ARCHI_API_TOOL = "archi_api"`（`ToolTier::Read`、`ToolExec::Archi`，静态清单 11 项，`Read` 档注释与 `Archi` 臂名同步）；`archi_tool.rs` 加一臂；`runner.rs::mint_archi_context` present 判据加名；`compose.rs` 的 archi 来源臂 +1 名、`archi_matrix` 仅 `(implement, executor)` 追加；`sdk/tools.rs` 的再导出名单与 `input_path` 零路径分支 +1 名 | 全部是既有范式：`ToolExec` 零新变体、`policy::allows` 对 `Read` 三档恒放行（零放行名单）、`cap_result` 是 L1 上限单点；矩阵只加一行 ⇒ 其余相位零变化 |
| D15 | 前端取数挂载层 | `useArchiApiCheck(root: string \| null): ArchiApiCheckState`（`{ check, loading, error, refresh }`）由 **`ContractPanel` 自身消费**（面板挂载 = 视图挂载 = 首触发），`loadedKey` 判据 + 失败清旧（旧差异不冒充），`root === null` 零 invoke，零 watch 零定时器 | 契约视图是唯一消费面，hook 挂在组件内自然满足「切到 contract 才取数」；「切视图不重取 `archi_graph`/`archi_module`/`archi_drift`」由「面板零引用那三个 hook」结构保证；hook 被真实消费也满足 knip |
| D16 | 两轴并列与计数 | 上轴 `check`（不符 / 孤儿 / 缺失三分组 + 各自计数 + 合计仅在轴上出现）；下轴 `findings`（chip 由 `findings.map(f => f.category)` 的**数据派生**去重后排序，零前端类别常量，零计数类别不渲 chip；每个 chip 带自身计数）；两轴标题各自计数，**MUST NOT** 出现跨轴求和 | spec 明写 chip 「MUST NOT 在前端另立词表或类别常量」⇒ 派生是唯一合规形态；「41 + 3 = 44」会让用户以为都得手改（两轴可修复者不同） |
| D17 | 生成交互与模式权限（U1） | `mode === 'incremental'` 才渲 `archi-contract-generate`；点击开 `StandardDialog` 二次确认，正文含三件事（「将写入 N 份（不符 X + 缺失 Y）」「保留 M 份孤儿生成物不做处理」「写盘非原子，失败点之前已写」）；确认 → `commands.archiApiGenerate(root)` 恰一次 → `toast` 报 `written` / `unchanged` → `refresh()`（重跑 check 恰一次）；`generating` 期间生成与检查双禁用；`full` 渲一行 `archi-contract-readonly`「生成需开 change（或让左侧会话执行 `pnpm api:export`）」 | 与既有「写要开 change」纪律同构（`archi-gate-readonly` 同形态）；确认文案把「不删孤儿」「非原子」写死，是 R-5 / R-6 的唯一机械兜底；禁用态防并发写同一批文件 |
| D18 | 面板状态矩阵与 testid | 五态：`loading`（`archi-contract-status`「解析中…」；判据 = `loading === true \|\| check === null` **且** `error === null`）/ `enabled = false`（同 testid：「契约面未启用（应生成集此时只含 `INDEX.toml`）」且**照常列 `missing`**）/ 零差异零判据（同 testid：「生成物最新，判据 0 条」）/ 检查失败（`statusText` 返回 `null`、**不渲 `archi-contract-status`**——成因只由 `archi-contract-error` + `data-source="check"` 承载，行内 + 可重试，画布不受影响）/ 生成失败（同 testid + `data-source="generate"`，面板顶部，文案不称全或无）；testid 全集 `archi-contract-panel` / `-axis-artifacts` / `-axis-findings` / `-chip` / `-item` / `-check` / `-generate` / `-status` / `-error` / `-readonly` | `-readonly` 是 spec 清单之外唯一新增 testid（`full` 提示需要一个稳定挂钩，形态沿 `archi-gate-readonly`）；`data-source` 让「检查失败」与「生成失败」在同一挂钩下可辨（两种成因不可混说，R-5 / R-11）；**状态词判定序 `root === null` → `error !== null` → loading → `enabled` → 零差异零判据**——`error !== null` 必须短路在 loading 之前：失败时 `check` 恰为 `null` 且 `loading` 为 `false`，若不短路就会在失败态谎称「解析中…」（错误块与状态词自相矛盾） |
| D19 | 抽屉契约段 | 插在 `Overview` 与 `DocsSection` 之间：`api.status` 一行 → `declared` 时渲契约 / schema / 生成物三行（各带 `FileViewer` 触发、「查看」按钮，testid `archi-contract-path`）→ 本元素判据（`findings.filter(f => f.element === element.fqn)`，逐条 `kind` + `message`）→ 零命中渲「本元素无漂移」（`archi-contract-drawer-empty`）；`status === 'none'` 只渲一行「无契约（`api = none` 或契约文件不在案）」（同 testid）且三路径行不渲 | 数据全部来自既有 `useArchiModule` 的 `detail.api` 与父级同一份 `apiFindings` ⇒ 零新 invoke；契约正文不进本段（正文仍归 `docs` + `read_file`）；两个 `archi-contract-*` testid 是抽屉侧新增挂钩（面板那组属 `archi-contract-panel` 域，spec 未列抽屉名，故不与之复用） |
| D20 | 左栏三视图接线 | `TreeMode = 'file' \| 'element' \| 'contract'`；`TreeModeSwitch` 选项表加第三钮（`data-mode="contract"`，文案「契约」）；`TreeColumn` 加 `contract` 臂渲 `ContractPanel`（栏宽：`element` → `w-72`，`file` / `contract` → `w-80`）；`CanvasBody` 从 `ArchitectureCanvas` 收 `apiFindings` 并传给面板与抽屉 | 第三值是既有「一个栏位三种视角」范式的延伸；`focus` 共用、切换只换左栏；渲染面（`toNodes` / `toEdges` / 标记）零触碰 |
| D21 | 文档 / 注释口径订正 | `AGENT.md` + `CLAUDE.md` 第 ⑤ 条 → 「契约在案元素的判据进 `status`（`warnings` 档）、不进 `passed`，两轴经架构页「契约」视图一眼看全」（两处逐字相同）；`archi-knowledge.md` 自检纪律补 `archi_api` 两轴 + 「生成物过期用既有命令重生成，MUST NOT 手改」；`implementation-generator.md` 只读工具行补 `archi_api`；四处代码头注释按事实校准 4 订正 | 旧口径（「不进 `status`」「CLI 唯一写盘面」）在本变更落地后即失真；文档与 spec 双写必然漂移，故只写指针与最简规则 |
| D22 | 明确不做 | ① C 面（独立 `StepNode` / 门禁步）；② 生成物进写保护面（加规则或内置化）；③ `apiFindings` 进 `passed`；④ 画布节点 / 边承载契约标记；⑤ `full` 模式任何生成入口；⑥ 抽屉新增 invoke；⑦ `scan/**` / `api/check.rs` / `contract.rs` / `schema.rs` / `paths.rs` / `emit/**` / `extract/**` 改动；⑧ 面板的孤儿删除入口；⑨ 实现相位对 `tests/**` 的任何读写（含金样重写与语料注释改写——归测试相位，见「测试相位承接面」）；⑩ `src/**` 侧测试文件的**用例本体**改动（新增用例、断言语义改写、纪律断言翻转、计数断言 10 → 11、`DRIFT_CASES` 登记面、契约段用例本体——归测试相位；实现相位只承担三份机械闭合面的编译改动，事实校准 11） | 逐条对应 proposal「不要修改」与 AC-19；越界项一律拒；⑨ 是回溯暴露的职责边界（事实校准 10）；⑩ 与 `tests/**` 硬线同族——测试文件上的「能编译」属实现面，「断言说什么」属测试面 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 渲染单点（**新**） | `workspace_render(root)`：元素表 → 装载契约 → 装配四类生成物（纯读盘、零写盘、零 spawn） | `crates/core/archi/src/api/render.rs` | `super::{elements_of, load_contracts, render_all}` | 既有纯函数组合 |
| 比对单点（**新**） | `compare(root) -> ArtifactDiff`；盘上生成物枚举（`scan::walk_source_files` + 契约目录有界枚举） | `crates/core/archi/src/api/diff.rs` | `super::{workspace_render, display, paths, MAX_API_WALK_DEPTH}`、`crate::scan::walk_source_files` | `ignore`（经既有遍历）、`std::fs` 只读 |
| 判定装配出口（**改**） | 新增 `artifact_check` / `gating_findings` / `findings` 三出口 + 两子模块声明；头注释订正 | `crates/core/archi/src/api/mod.rs` | 同上 | 既有 |
| 出线 DTO（**改**） | `ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`（`specta::Type` + `camelCase`） | `crates/core/archi/src/dto.rs` | `serde` / `specta` | 既有线面 DTO 形态 |
| 出口面（**改**） | `pub use` 追加五函数 + `ArtifactDiff` + 三 DTO + `graph_diagnostics_error`；crate 头注释订正 | `crates/core/archi/src/lib.rs` | — | 既有逐名 `pub use` |
| `status` 纳管（**改**） | `REPORT_VERSION` 3、`api_findings` 装配改走 `api::findings`、纳管子集入参、`warnings` 分支扩或条件 | `crates/core/archi/src/drift/mod.rs` + `drift/report.rs` | `crate::api` | 既有 |
| 写盘单点（**新**） | `write_artifacts(root, files)`：越根复核 + 读盘比对 + 建目录 + 写；不删不回滚 | `crates/infra/archi/src/api_artifacts.rs` | `archi::{GeneratedFile, dto::ArchiApiWriteOutcome, display}` | `std::fs` 写面 |
| 执行面登记（**改**） | `pub mod api_artifacts;` + crate 头注释（两处写盘单点） | `crates/infra/archi/src/lib.rs` | — | 既有 |
| CLI 编排（**改**） | 三子命令编排 + 退出码；零本地 fs 写调用、零本地 walker | `crates/tools/archi-api-gen/src/main.rs` + `Cargo.toml` | `archi` / `foundation` / `archi-runtime` | 既有 CLI |
| 命令面（**改**） | `archi_api_check` / `archi_api_generate` 薄包装 + 两 `*_inner`；文件头口径订正 | `src-tauri/src/commands/archi/mod.rs` | `archi::api` / `archi_runtime::api_artifacts` | `#[tauri::command] #[specta::specta]` |
| 登记面（**改**） | `all_commands!` archi 段 6 → 8 条 | `src-tauri/src/commands/mod.rs` | — | 既有宏 |
| 工具面（**改**） | `archi_api` 定义 + 执行体 + present 判据 + 矩阵/来源臂 + 名字再导出 | `crates/infra/agent/src/sdk/{mount,archi_tool,tools,runner}.rs`、`compose.rs` | `archi::api` / `archi::dto` | 既有 `ArchiContext` 路由 |
| 知识 / prompt（**改**） | 自检纪律一条 + implement 相位只读工具行 | `crates/infra/agent/src/archi-knowledge.md`、`crates/core/workflow/src/prompts/implementation-generator.md` | — | 静态 md |
| 契约取数 hook（**新**） | `useArchiApiCheck(root)`：三触发、失败清旧、零轮询 | `src/views/architecture/hooks/use-archi-api-check.ts` | 生成绑定 `commands.archiApiCheck` | React 19 hooks |
| 契约面板（**新**） | 两轴分区 + chip 过滤 + 条目聚焦 + 路径行 `FileViewer` + 检查 / 按模式生成 + 五态 | `src/views/architecture/components/contract-panel.tsx` | 上述 hook、`ui/button`、`components/standard/dailog`、`components/file/file-viewer`、`sonner` | React |
| 画布接线（**改**） | 三视图分支 + `apiFindings` 下传 + 面板臂 | `src/views/architecture/components/architecture-canvas.tsx`、`file-tree.tsx` | `ContractPanel` | React |
| 抽屉契约段（**改**） | 概览与文档之间插契约段（三路径行 + 本元素判据 + 空态） | `src/views/architecture/components/element-drawer.tsx` | 既有 `useArchiModule` | React |

依赖方向零新增边（除 spec 明令的一条 tools → infra 边）：`core/archi` 仍零写盘、零 infra 依赖；`archi-runtime` 依赖清单零新增（只用既有 `archi` / `foundation` / `serde_json`）；`archi-api-gen` 新增 `archi-runtime = { workspace = true }`（唯一用途 = 写盘单点）；`agent-runtime` **不**依赖 `archi-runtime`（故 `archi_api` 只能只读）；前端零新增 npm 依赖。

### `api/diff.rs` 的内部形态（私有，不入公共 API 表）

```
pub fn compare(root: &Path) -> Result<ArtifactDiff, ApiError>
  ├ fn generated_files_on_disk(root: &Path) -> Vec<PathBuf>      // 仓库级遍历经 scan::walk_source_files
  │    └ fn api_dir_artifacts(root: &Path) -> Vec<PathBuf>       // 契约目录有界枚举（深度 ≤ super::MAX_API_WALK_DEPTH）
  └ fn is_generated_module_file(path: &Path) -> bool             // 命名即身份（*i.rs / *i.ts）
```

`compare` 主流程：`expected = workspace_render(root)?` → 逐份 `fs::read_to_string` 分流（同 ⇒ 无项 / 异 ⇒ `mismatched` / 读失败 ⇒ `missing`）→ `expected_paths: BTreeSet<PathBuf>` → 盘上集合差集（`!expected_paths.contains`）⇒ `orphaned`；三列最后统一 `display(root, …)`。

---

## 变更清单 (Change Inventory)

### 新增文件

| 文件 | 说明 | 关联子表条目 |
|---|---|---|
| `crates/core/archi/src/api/render.rs` | `workspace_render`（D1）；文件尾 `#[cfg(test)] mod render_test;` | 公共函数 `workspace_render` |
| `crates/core/archi/src/api/diff.rs` | `compare` + `ArtifactDiff` + 私有盘上枚举（D2 / D3 / D4） | 公共函数 `compare`；类型 `ArtifactDiff` |
| `crates/infra/archi/src/api_artifacts.rs` | `write_artifacts`（D10）；文件尾 `#[cfg(test)] mod api_artifacts_test;` | 公共函数 `write_artifacts` |
| `src/views/architecture/hooks/use-archi-api-check.ts` | 契约检查取数 hook（D15） | 公共函数 `useArchiApiCheck`；类型 `ArchiApiCheckState` |
| `src/views/architecture/components/contract-panel.tsx` | 左栏契约视图（D16 / D17 / D18；私有 `statusText` 承载五态判定序） | 公共函数 `ContractPanel`；类型 `ContractPanelProps` |
| `crates/core/archi/src/api/render_test.rs` | **空壳**（两行头注释、零用例零断言）：`#[cfg(test)] mod render_test;` 的解析前提；用例本体归测试相位 | —（零公共 API；`api = none` 式留白） |
| `crates/core/archi/src/api/diff_test.rs` | **空壳**（同上）：`mod diff_test;` 的解析前提 | —（零公共 API） |
| `crates/infra/archi/src/api_artifacts_test.rs` | **空壳**（同上）：`mod api_artifacts_test;` 的解析前提 | —（零公共 API） |
| `src/views/architecture/hooks/use-archi-api-check.test.ts` | **空壳**（两行注释、零用例）：knip / vitest 的文件位 | —（零公共 API） |
| `src/views/architecture/components/contract-panel.test.tsx` | **空壳**（两行注释、零用例）：面板用例本体归测试相位 | —（零公共 API） |

> **测试模块文件只落空壳**：上表末五行（两份 `*_test.rs` 与 `*_test.ts` / `*_test.tsx`）在本相位只到「仅头注释两行、零用例零断言、可编译」的形态——`#[cfg(test)] mod …;` 的解析前提与 `knip` / vitest 的文件位；它们是实现相位实际落盘的新建文件，故**登记在「新增文件」表**（而非匿于说明），但零公共 API、不入公共函数 / 类型子表，用例本体归测试相位。
>
> **测试语料不入本清单**：`crates/core/archi/tests/corpus/drift/expected/*.json`（16 份金样）与 `.../drift/api-mismatch/models/01-app.toml`（语料自述注释）位于 `tests/` 语料面，与用例一样归测试相位（test-design / test-gen / test-execution）读写，实现相位对 `tests/**` **零读写**——故二者**不是实现变更**，不进「修改文件」表；本变更对它们造成的取值变化只作期望登记，见下文「测试相位承接面」。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/dto.rs` | 线面 DTO 区新增三类型（字段序 = 声明序）；既有九组封闭词汇与既有 DTO 零改动 | D5 / AC-3 |
| `crates/core/archi/src/lib.rs` | `pub use api::{…}` 追加 `workspace_render` / `compare` / `artifact_check` / `gating_findings` / `findings` / `ArtifactDiff`；`pub use dto::{…}` 追加三 DTO 与 `graph_diagnostics_error`；crate 头注释第 5 条改写「唯一写盘面 → 判定域零写盘、写盘单点在 `infra/archi`」 | 事实校准 4 / 8 |
| `crates/core/archi/src/api/mod.rs` | `pub mod diff;` `pub mod render;` 两行；新增 `artifact_check` / `gating_findings` / `findings`；文件尾 `#[cfg(test)] mod diff_test;` `mod render_test;`；头注释「唯一写盘面是 tools」段订正 | D5 / D6 / D7 |
| `crates/core/archi/src/drift/mod.rs` | `REPORT_VERSION` 2 → 3 + 三点兼容说明；契约判据装配改调 `api::findings`；新增纳管子集调用；`derive_status` 第 5 入参与 `warnings` 分支扩或；模块 doc 第 9 行口径订正 | D7 / D8 / D9 |
| `crates/core/archi/src/drift/report.rs` | `api_findings` 字段 doc 改述为「经纳管子集影响 `status`、不进 `passed`」；字段 / 类型 / 序号零改动 | D8 |
| `crates/infra/archi/src/lib.rs` | `pub mod api_artifacts;`；crate 头注释补「本 crate 承载两处写盘单点：报告落盘与生成物写盘」 | D10 |
| `crates/tools/archi-api-gen/src/main.rs` | 删 `collect` / `write` / `struct Report` / `differences` / `generated_files_on_disk` / `is_generated_module_file` / `walk_modules` / `walk` / `SKIPPED_DIRECTORIES` / `MAX_MODULE_WALK_DEPTH` / `model_elements`；三子命令改调 `api::workspace_render` / `archi_runtime::api_artifacts::write_artifacts` / `api::compare` / `api::bootstrap`；头注释订正 | D11 / AC-4 |
| `crates/tools/archi-api-gen/Cargo.toml` | `[dependencies]` 加 `archi-runtime = { workspace = true }` 带用途注释；头注释「零依赖面 / CLI 唯一写盘面」段改写为「写盘实现已上移，本 crate 只做编排」 | D11 / AC-5 |
| `src-tauri/Cargo.toml` | `archi-runtime` 的用途注释补「+ 两条契约面命令调生成物写盘单点」；依赖条 `archi` / `archi-runtime` 与 crate 版本行零改动 | 事实校准 3 |
| `src-tauri/src/commands/archi/mod.rs` | 两条命令 + 两 `*_inner`；文件头「六命令」→「八命令」、「唯一写命令是 `archi_set_gate`」→「写命令两条（`archi_set_gate` / `archi_api_generate`）」、「其余五条 MUST NOT 写盘」→「其余六条」；`use` 区补三 DTO | D12 / AC-1 |
| `src-tauri/src/commands/mod.rs` | archi 段 `archi_file_tree` 之后、`read_file` 之前插两条 | D12 / AC-1 / AC-17 |
| `src-tauri/src/bindings/mod_test.rs` | 清单随动：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 51 → 53、`DTO_TYPES` 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26；尾部追加序期望 vec 增两条（插在 `archi_file_tree` 之后、`read_file` 之前）、族末条对照臂由 `archi_file_tree` 变 `archi_api_generate` | AC-17（机械守卫，非用例编写） |
| `crates/tools/archi-api-gen/src/main_test.rs` | 编译闭合：四处调用点改调——`super::collect` → `archi::api::workspace_render`、`super::differences` → `archi::api::compare`、`super::write` → `archi_runtime::api_artifacts::write_artifacts`（单份改 `std::slice::from_ref`）、`Report::to_json()` → `super::diff_json(&report)`；指向已删 helper 的 doc 表述（用例 doc 的「`super::differences` 直调」句）随实订正 | AC-4 的 `--all-targets` 编译闭合面（事实校准 11；机械守卫，非用例编写） |
| `packages/desktop/src/views/architecture/components/element-drawer.test.tsx` | 编译闭合：每个 `<ElementDrawer>` 调用补新必填 prop `findings={[]}`；共享 `archi_module` mock 补 `api` 投影（`{ status: 'none', contractPath: null, schemaPath: null, generatedPath: null }`）；既有断言零删除 | AC-13 的 `tsc --noEmit` 编译闭合面（事实校准 11；机械守卫，非用例编写） |
| `crates/infra/agent/src/sdk/mount.rs` | `ARCHI_API_TOOL` 常量；静态清单 10 → 11 项；`ToolTier::Read` 注释与 `ToolExec::Archi` 臂名同步；`archi_api_definition()` | D14 / AC-15 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `execute` 加 `archi_api` 一臂（`check` / `bootstrap`），响应形状见 D13，输出过 `cap_result` | D13 |
| `crates/infra/agent/src/sdk/tools.rs` | 名字再导出名单 + `input_path` 零路径分支各加 `ARCHI_API_TOOL`（两行） | 事实校准 2 |
| `crates/infra/agent/src/sdk/runner.rs` | `mint_archi_context` present 判据加名 | D14 |
| `crates/infra/agent/src/compose.rs` | archi 来源臂名单 +1；`archi_matrix` 的 `(implement, executor)` 臂追加 `ARCHI_API_TOOL` | D14 |
| `crates/infra/agent/src/archi-knowledge.md` | 自检纪律补一句（`archi_api` 两轴 + 生成物用既有命令重生成、MUST NOT 手改）；零 `<>` 占位符、体量在既有上限内 | D21 / AC-16 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 「额外可用只读工具」行补 `archi_api`；其余相位 prompt 零 diff | D21 / AC-16 |
| `src/views/architecture/components/file-tree.tsx` | `TreeMode` 扩 `'contract'`；`TreeModeSwitch` 选项表加第三钮；模块 doc 两处「两值 / 两视图」订正 | D20 / AC-14 |
| `src/views/architecture/components/architecture-canvas.tsx` | `CanvasBody` 收 `apiFindings`；`TreeColumn` 加 `contract` 臂与宽度分流；`ArchitectureCanvas` 下传 `report?.apiFindings ?? []` | D20 / AC-12 |
| `src/views/architecture/components/element-drawer.tsx` | Props 加 `findings: ArchiApiFinding[]`；新增 `ContractSection` 插在 `Overview` 与 `DocsSection` 之间 | D19 / AC-13 |
| `package.json` | `"version": "0.6.39"` → `"0.6.40"`（单行） | AC-18 |
| `AGENT.md` + `CLAUDE.md` | `## 模块 API 契约` 第 ⑤ 条改纳管口径（两文件该节逐字相同） | D21 / AC-19 |
| `src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 再生成（两包装 + 三 `export type` + zod 表项）；**禁手改** | AC-17 |

> **实现相位可触的三份机械闭合面**（都在 `src/` 侧，不是 `tests/**` 语料）：`src-tauri/src/bindings/mod_test.rs`（清单与计数）、`crates/tools/archi-api-gen/src/main_test.rs`（调用点改调 + doc 表述）、`src/views/architecture/components/element-drawer.test.tsx`（必填 prop / mock 字段）。三者只承担「让既有用例编译通过」的最低机械改动——**零用例新增、零断言删除、零断言语义改写**。其余测试面一律归 test-design / test-gen / test-execution：新测试模块（`api/{render,diff}_test.rs` / `api_artifacts_test.rs` / `use-archi-api-check.test.ts` / `contract-panel.test.tsx`）的用例本体、既有 `api/mod_test.rs` / `drift/{mod,report}_test.rs` / `dto_test.rs` / `commands/archi/mod_test.rs` / `mount_test.rs`（十项 → 十一项）/ `archi_tool_test.rs` / `runner_test.rs` / compose 用例 / `golden_test.rs` 与 `tests/corpus/**` / 前端既有四份 `*.test.tsx` 的断言改写 / `ipc-contract.test.ts`。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `workspace_render` | `crates/core/archi/src/api/render.rs` | 新增 | `pub fn workspace_render(root: &Path) -> Result<Vec<GeneratedFile>, ApiError>` | `elements_of` → `load_contracts` → `render_all`；纯读盘、零写盘、零 spawn |
| `compare` | `crates/core/archi/src/api/diff.rs` | 新增 | `pub fn compare(root: &Path) -> Result<ArtifactDiff, ApiError>` | 三类差异；三列 repo 相对 POSIX 串（经 `display`）；零写盘 |
| `artifact_check` | `crates/core/archi/src/api/mod.rs` | 新增 | `pub fn artifact_check(root: &Path) -> Result<ArchiApiCheck, ApiError>` | `compare` + `enabled` + `meta()` 的 DTO 投影单点 |
| `gating_findings` | `crates/core/archi/src/api/mod.rs` | 新增 | `pub fn gating_findings(root: &Path, elements: &[ArchiElement], findings: &[ArchiApiFinding]) -> Vec<ArchiApiFinding>` | 纳管子集筛选单点（`declared` + 契约文件在案；`element = None` 落选） |
| `findings` | `crates/core/archi/src/api/mod.rs` | 新增 | `pub fn findings(root: &Path, elements: &[ArchiElement]) -> Result<Vec<ArchiApiFinding>, ApiError>` | 契约判据装配单点（`enabled` 短路 → `load_contracts` → `check`）；`drift` 与 `archi_api` 共用 |
| `graph_diagnostics_error` | `crates/core/archi/src/dto.rs` | 修改（`pub(crate)` → `pub`） | `pub fn graph_diagnostics_error(diagnostics: &[ArchiDiagnostic]) -> String` | 结构诊断文案单点提权（agent 工具复用，零第二份拼装） |
| `write_artifacts` | `crates/infra/archi/src/api_artifacts.rs` | 新增 | `pub fn write_artifacts(root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>` | 越根复核 → 读盘比对 → 建目录 + 写；不删、不回滚；两列 repo 相对串 |
| `archi_api_check` | `src-tauri/src/commands/archi/mod.rs` | 新增（IPC 命令） | `pub fn archi_api_check(root: String) -> Result<ArchiApiCheck, String>` | 零写盘、零 spawn；`check_root` 前置 |
| `archi_api_check_inner` | 同上 | 新增 | `pub fn archi_api_check_inner(root: &Path) -> Result<ArchiApiCheck, String>` | `check_root` → `archi::api::artifact_check` → `ApiError::message()` |
| `archi_api_generate` | 同上 | 新增（IPC 命令） | `pub fn archi_api_generate(root: String) -> Result<ArchiApiWriteOutcome, String>` | 第二条写命令；只增只改不删 |
| `archi_api_generate_inner` | 同上 | 新增 | `pub fn archi_api_generate_inner(root: &Path) -> Result<ArchiApiWriteOutcome, String>` | `check_root` → `api::workspace_render` → `write_artifacts` |
| `useArchiApiCheck` | `src/views/architecture/hooks/use-archi-api-check.ts` | 新增 | `export function useArchiApiCheck(root: string \| null): ArchiApiCheckState` | 三触发（挂载 / `root` 变化 / `refresh()`）、失败清旧、零轮询 |
| `ContractPanel` | `src/views/architecture/components/contract-panel.tsx` | 新增 | `export function ContractPanel(props: ContractPanelProps): React.JSX.Element` | 两轴分区 + chip + 生成交互 + 五态 |

> `archi_tool.rs` 的执行体是 `ArchiContext` 的私有臂（`pub(crate) struct` 的方法），按本表口径不单列；其行为契约由 `desktop-archi-agent` ADDED requirement 与 D13 承载。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `ArchiApiCheck` | `crates/core/archi/src/dto.rs` | 新增 | 出线 DTO：`enabled: bool` / `meta: ArchiApiMeta` / `mismatched: Vec<String>` / `orphaned: Vec<String>` / `missing: Vec<String>`；`specta::Type` + `rename_all = "camelCase"`，字段序即声明序 |
| `ArchiApiMeta` | 同上 | 新增 | 出线 DTO：`tool_version: String` / `common_digest: String`；只两项（`schemaUri` / `generatedNotice` / `common` 不出线） |
| `ArchiApiWriteOutcome` | 同上 | 新增 | 出线 DTO：`written: Vec<String>` / `unchanged: Vec<String>` |
| `ArtifactDiff` | `crates/core/archi/src/api/diff.rs` | 新增 | 进程内类型（`Debug, Clone, PartialEq, Eq`，无 `specta`）：`mismatched` / `orphaned` / `missing` 三列 `Vec<String>`；出线由 `artifact_check` 投影 |
| `ArchiApiCheckState` | `src/views/architecture/hooks/use-archi-api-check.ts` | 新增 | `interface`：`check: ArchiApiCheck \| null` / `loading: boolean` / `error: string \| null` / `refresh: () => void` |
| `ContractPanelProps` | `src/views/architecture/components/contract-panel.tsx` | 新增 | `interface`：`root: string \| null` / `mode: CanvasMode` / `findings: ArchiApiFinding[]` / `onFocus: (fqn: string) => void` |
| `TreeMode` | `src/views/architecture/components/file-tree.tsx` | 修改 | `'file' \| 'element'` → `'file' \| 'element' \| 'contract'` |
| `ElementDrawerProps` | `src/views/architecture/components/element-drawer.tsx` | 修改 | 追加 `findings: ArchiApiFinding[]`（父级同一份报告，零新 invoke） |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.40`（现值 `0.6.39`，+1 patch） | `tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不动 |
| `dependencies.archi-runtime` | `crates/tools/archi-api-gen/Cargo.toml` | 新增 | workspace 依赖项 | `{ workspace = true }` | 唯一用途 = 调 `write_artifacts`；带用途注释 |

> 配置项零新增：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`tauri.conf.json`、`capabilities/default.json` 全部零 diff；`package.json` 的 `api:export` / `api:check` / `check` 三段命令串逐字不变。

---

## 测试相位承接面（不入实现变更清单）

本变更的判定口径（`REPORT_VERSION` 3 + 契约在案纳管）改变了两类**测试语料**的取值；同一口径还使一批**既有用例的断言失效**（十项 → 十一项、`REPORT_VERSION` 断言、纪律断言翻转、`DRIFT_CASES` 登记面等）。语料与用例本体同属测试面：实现相位对 `tests/**` 零读零写、对 `src/**` 侧测试文件只做编译闭合（事实校准 11），故**不登记为实现变更、不产生实现任务**，只在此登记期望与承接方。实现的完成判据止于 D8 / D9 落地（版本常量与 `status` 推导口径）与三份机械闭合面可编译，语料落盘与断言回归由承接方闭环。

| 文件 | 期望（由本设计判定口径决定） | 承接方 |
|---|---|---|
| `crates/core/archi/tests/corpus/drift/expected/*.json`（16 份） | 全部 `"version"` 2 → 3；唯一例外 `expected/api-mismatch.full.json` 的 `status` 由 `clean` 变 `warnings`（该元素契约在案且 1 条 `apiSignatureMismatch` 纳管）；其余 15 份除 `version` 外逐字不变 | 机械重写（`DESKTOP_GOLDEN_REWRITE=1`）+ `git diff` 逐份人工确认 → **test-execution**；断言侧改写（`tests/golden_test.rs` 的 `DRIFT_CASES` 登记面与三处硬编码版本 / `status` 断言）→ **test-design / test-gen** |
| `crates/core/archi/tests/corpus/drift/api-mismatch/models/01-app.toml` | 文件头注释由「契约面 MUST NOT 进 `status` 推导 …… `status = clean`」改为纳管口径（「该元素契约在案 ⇒ 判据纳管 ⇒ `status = warnings`；`passed` 不受影响」）；模型元素 / 条目 / 关系 / 元数据**逐字不变**（`git diff` 只含注释行） | 与金样重写同批的纯注释改写 → **test-execution**（保证语料自述与金样 `status` 不自相矛盾，R-2） |
| 其余测试面（`src/**` 侧用例本体与前端测试文件） | 断言随实现口径改写：`api/{render,diff}_test.rs` / `api_artifacts_test.rs` / `use-archi-api-check.test.ts` / `contract-panel.test.tsx` 的用例本体新写；`api/mod_test.rs`（`gating_findings`）/ `drift/{mod,report}_test.rs` / `dto_test.rs` / `commands/archi/mod_test.rs`（扩八命令）/ `mount_test.rs`（十项 → 十一项）/ `archi_tool_test.rs` / `runner_test.rs` / compose 用例 / 前端既有四份 `*.test.tsx` 的既有断言改写；`main_test.rs` 的纪律断言翻转（`fs::write` 计数 1 → 0、本地 walker 判据改为零本地副本）；`ipc-contract.test.ts` 随绑定全量核对 | **test-design / test-gen**（断言口径）→ **test-execution**（执行与回归）。实现相位在三份机械闭合面（`bindings/mod_test.rs` / `main_test.rs` / `element-drawer.test.tsx`）上只做编译改动，本行的断言改写不属其交付 |

边界口径：AC-7 的「除 `version` 外逐字不变 + 唯一例外」与 R-2 的「语料自述不得与金样矛盾」以上表前两行的实际落盘结果为准，**不由实现相位勾选**；实现相位的自查只覆盖源码面（判定面 / 写盘单点 / 清单守卫 / 前端渲染）与三份机械闭合面的可编译性（`cargo check --all-targets` / `tsc --noEmit` 跑绿），不覆盖任何断言的真假。

---

## 数据模型 (Data Model)

### 出线 DTO（三新类型，字段序即声明序，`camelCase`）

| 类型 | 字段（序） | 类型 | 语义 / 约束 |
|---|---|---|---|
| `ArchiApiCheck` | `enabled` | `bool` | 契约面是否启用（`api::enabled` 单点；`false` 时 `missing` 通常只有 `INDEX.toml` 一条） |
| | `meta` | `ArchiApiMeta` | 生成载元（`api::meta()` 投影） |
| | `mismatched` | `string[]` | 内容不符（文件在盘、字节不同） |
| | `orphaned` | `string[]` | 应删未删（命名符合生成物判据、不在应生成集） |
| | `missing` | `string[]` | 应生成未生成 |
| `ArchiApiMeta` | `toolVersion` | `string` | `CARGO_PKG_VERSION`（工具版本） |
| | `commonDigest` | `string` | 映射规则资产摘要 `sha256:<64hex>`（渲染侧只截前 12 位，DTO 不截） |
| `ArchiApiWriteOutcome` | `written` | `string[]` | 本次实际写入（新建或内容变化） |
| | `unchanged` | `string[]` | 应生成且盘上已一致（本次零写入、mtime 不动） |

三列 / 两列路径串统一口径：repo 相对 POSIX（剥 `root` 前缀、`\` → `/`、无前导 `./`、无绝对路径），经 `archi::display` 单点产出；前端组装 `root + '/' + rel` 交 `read_file`。

### 报告（`ArchiDriftReport`）

- **字段零改动**（14 字段 + `api_findings` 并列槽位形状不变，字段序不变）。
- **`version` 2 → 3**：`1` 无 `api_findings`；`2` 有该字段但 `status` 推导不读契约判据；`3` 纳管进 `status`（消费方按 `version` 分支，MUST NOT 补默认值）。
- **`status` 推导**：`skipped` → `errors`（结构错或 `errors[]` 非空）→ `noChanges`（可判文件 0）→ `warnings`（`warnings[]` 非空 **或** 纳管子集非空）→ `clean`；`passed` 派生面零改动（`errors.is_empty() && structural_errors.is_empty()`）。
- **纳管子集定义**：`apiFindings` 中 `element` 属于「`api = declared` 且契约 md 在案」元素集合的条目；`element = None` 落选。无白名单、无开关、无配置键。

### 持久化（生成物）

| 生成物 | 落点 | 本轮读写面 |
|---|---|---|
| `<FQN>.schema.json` | `<archi_api_dir>/<Domain>/` | 检查读、生成写（只增改） |
| `INDEX.toml` | `<archi_api_dir>/` 根 | 同上 |
| `<root>.i.rs` / `<root>.i.ts` | 元素根同级 | 同上 |
| 孤儿生成物 | 任意（命名即身份） | **只报告**：检查列 `orphaned`，生成不删、不移动、不改写 |

写盘语义：逐份「先比对后写」⇒ 幂等且不动 mtime；非原子（单份失败即 `Err`，已写部分保留）。

---

## 路由 / API 设计 (Route / API Design)

IPC 命令（Tauri `invoke`，命令名即 path；本地进程内调用，无鉴权面 —— `root` 为显式观察根，命令只在 `check_root` 通过后作业）：

| Method | Path（命令名） | 说明 | 输入 | 输出 | auth |
|---|---|---|---|---|---|
| invoke | `archi_api_check` | 零写盘检查（三类差异 + `enabled` + `meta`）；`enabled = false` 仍 `Ok` | `{ root: string }` | `ArchiApiCheck` | 无（本地 IPC） |
| invoke | `archi_api_generate` | 写盘（只增只改、从不删）；结构错 / 越根 / IO 失败 → `string` 错误 | `{ root: string }` | `ArchiApiWriteOutcome` | 无（本地 IPC） |

错误面统一 `Result<T, String>`（`ApiError::message()` / infra `String` 透传原文，零包装、零补默认值）。

不涉及 HTTP 端点、不新增 CLI 子命令（`archi-api-gen` 的子命令面逐字不变）。

---

## 验收标准回应

| AC | 落点 | 归属阶段（tasks） |
|---|---|---|
| AC-1 八命令 / 写命令两条 | D12 + `commands/mod.rs` 登记 | 六 |
| AC-2 check 零写盘 / generate 只增改 | D5 / D10 / D11 | 二 / 四 / 五 |
| AC-3 DTO 形状与路径串同源 | DTO 表 + D2（`display` 单点） | 一 / 二 |
| AC-4 比对面上移且判定单源 | D1 / D2 / D11；另含 `--all-targets` 的编译闭合面 `main_test.rs` 调用点改调（事实校准 11） | 二 / 五 |
| AC-5 写盘单点驻 `infra/archi` | D10 + `archi-api-gen/Cargo.toml` | 四 / 五 |
| AC-6 `status` 纳管口径 | D6 / D7 / D9 | 二 / 三 |
| AC-7 `REPORT_VERSION = 3` 与 16 份金样 | D8（版本常量与 `status` 推导口径）+ 两处语料的期望登记于「测试相位承接面」（重写与回归移交 test-execution，实现相位零读写 `tests/**`） | 三（实现）；测试相位（语料落盘与回归） |
| AC-8 本仓实测零变化 | D6（纳管集合仅 `App.Backend.Infra.ArchiRuntime`、判据 0）+ 事实校准 7 | 十三（手工登记） |
| AC-9 面板两轴并列 | D16 / D18 | 十 |
| AC-10 状态矩阵五态 | D18（含 `statusText` 判定序订正 = 事实校准 9） | 十 / 十一 |
| AC-11 生成交互与模式权限 | D17（含 `full` 只读提示）+ D20 | 十 / 十一 |
| AC-12 取数与成本纪律 | D15 + D20（面板零引用三既有 hook） | 十 / 十一 |
| AC-13 抽屉契约段 | D19；另含 `tsc --noEmit` 的编译闭合面 `element-drawer.test.tsx` 的必填 prop / mock 字段（事实校准 11） | 十一 |
| AC-14 分段控件三视图 | D20 | 十一 |
| AC-15 `archi_api` 只读工具 | D13 / D14 + 事实校准 2 / 8 | 八 |
| AC-16 知识 / prompt 随动 | D21 | 九 |
| AC-17 清单守卫随动 | 阶段七（51→53 / 101→104 / 6→8 / 23→26 + 尾序 + 对照臂） | 七 |
| AC-18 门禁 | 阶段十二版本 +1 patch；期望登记（`pnpm check` 四段移交桌面端 static-check、`cargo nextest` 与语料重写移交 test-execution，见「测试相位承接面」） | 十二（实现）；static-check / test-execution（门禁执行） |
| AC-19 边界零 diff | D22 + 阶段十三自查清单 | 十三 |

---

## 依赖 (Dependencies)

运行时依赖：

| 依赖 | 变化 | 用途 |
|---|---|---|
| `archi-runtime`（workspace） | `archi-api-gen` **新增** | 调生成物写盘单点 `write_artifacts`（唯一用途） |
| `ignore` | 零新增（`archi` 已在案） | 仓库级生成物遍历**经 `scan::walk_source_files`** 间接使用（不新增直接调用点） |
| `serde_json` | 零新增 | CLI `--check` 单行 JSON、agent 工具响应 |
| `tempfile` | 零新增（`infra/archi` 既有 dev-dependency） | `api_artifacts_test.rs` 的 tempdir |

构建 / 测试依赖：无新增。`Cargo.lock` 零净增包（无新外部 crate）；`[workspace.dependencies]` / `members` 零改动；前端零新 npm 依赖（面板只用既有 `ui/button`、`standard/dailog`、`file/file-viewer`、`sonner`、`lucide-react`）。

---

## 待决问题 (Open Questions)

1. **`archi_api` 工具 `check` 的规模策略**：本轮按 spec 只用 `tools::cap_result`（50KB L1）截断，判据多时 `items` 尾部会被切断（摘要与 `total` 保留）。若实测挤压上下文，可改为「`items` 只出前 N 条 + `total` / `byCategory` 全量」——届时只改 `archi_tool` 一处投影，出线形状不变。
2. **面板 `meta.commonDigest` 的展示长度**：本轮 DTO 出全量、面板显示前 12 位 hex（与生成物头注释同形）。若评审要求 DTO 直接出短形，只改 `artifact_check` 的投影一行（不影响其它消费面）。
3. **孤儿生成物的处置入口**：本轮只「打开定位」（`FileViewer`），无删除入口（生成从不删、面板不删）。是否加「复制路径」一类便捷动作待观察真实使用；任何删除能力都需独立裁决（写面扩宽，非本轮范围）。
4. **`full` 模式面板提示的措辞定稿**：「生成需开 change（或让左侧会话执行 `pnpm api:export`）」为 spec 给定句；若评估要求点名 `cargo run -p archi-api-gen -- generate` 的等价形态，属文案级调整（零结构影响）。
