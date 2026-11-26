# 任务: archi-generator-access-gap

> 顺序即依赖：前一组解锁后一组。每项以文件为入口，落地口径见 `design.md`（决策 `D-1`…`D-16`、附录 A–E）与 `specs/` 九份 delta。
> 纪律：接口面与实现同批（导出面一改，接口定义文件同批改）；生成物（`INDEX.toml` / `.i.*`）与 bindings **MUST NOT 手改**；`pnpm api:check` / `cargo check` / `pnpm` 门禁等机械步骤由桌面端执行，agent 内不重复执行。

## 阶段 1 · 端口与唯一实现（`core/archi` + 壳层）

- [x] `crates/core/archi/src/api/mod.rs`：紧跟 `GeneratedFile` 定义新增 `pub trait ArtifactWriter: Send + Sync`，唯一方法 `fn write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`；`use crate::dto::ArchiApiWriteOutcome;` 随动；doc 写明「纯类型：本 crate 仍零写盘、零 spawn、零 IO，实现由壳层注入」。
- [x] `crates/core/archi/src/lib.rs`：既有两个 `pub use api::{…}` 名单并入 `ArtifactWriter`（保持既有排序）；crate 根 doc 第 5 条纪律句（契约面纯读）补一句「写盘端口是类型，不破本 crate 只读盘」。
- [x] `src-tauri/src/commands/archi/mod.rs`：新增 `pub struct ArchiArtifactWriter;` 与 `impl archi::ArtifactWriter for ArchiArtifactWriter`——函数体只 `archi_runtime::api_artifacts::write_artifacts(root, files)` 一次（零自有写盘逻辑、零 `fs::write`、零 `create_dir_all`）；模块头 doc 补一句「agent 工具 `archi_generate` 经本端口委托同一写盘单点（第二驱动面，新增调用方而非新实现）」。

## 阶段 2 · 注入链五跳（端口从组合根流到 per-turn 上下文）

- [x] `crates/infra/agent/src/sdk/runner.rs`：`SdkRunner` 增字段 `artifact_writer: Option<Arc<dyn archi::ArtifactWriter>>`（`new` 置 `None`）+ builder `pub fn with_artifact_writer(self, writer: Option<Arc<dyn archi::ArtifactWriter>>) -> Self`（与 `with_ask_port` 同型、doc 同措辞）。
- [x] `crates/infra/agent/src/sdk/runner.rs`：`PumpPayload` 增同名字段，并在 `open_session` 的载荷字面量里填 `artifact_writer: self.artifact_writer.clone()`。
- [x] `crates/infra/agent/src/lib.rs`：`EngineFacade` 增字段 `artifact_writer`；`new` / `with_resume_transcript` 两处穷尽字面量置 `None`，其余 8 处穷尽字面量（`with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store` / `with_ask_registry` / `with_ask_port` / `with_workspace_root` / `with_db_stores`）传 `self.artifact_writer`；新增 `pub fn with_artifact_writer(self, writer: Option<Arc<dyn archi::ArtifactWriter>>) -> Self`；`runner_for` 的 sdk 臂 builder 链上 `.with_artifact_writer(self.artifact_writer.clone())`。
- [x] `crates/infra/agent/src/compose.rs`：`compose_turn` 末位增参 `artifact_writer: Option<Arc<dyn archi::ArtifactWriter>>` 并链到 `EngineFacade::with_artifact_writer`；模块头 doc 补一句（生成物写盘端口由组合根注入、经门面 → runner → per-turn 上下文透传，`agent-runtime` 仍不依赖 `archi-runtime`）。
- [x] `src-tauri/src/commands/exec/mod.rs`：`compose_turn(...)` 末位传 `Some(Arc::new(ArchiArtifactWriter) as Arc<dyn archi::ArtifactWriter>)`，import 补 `use crate::commands::archi::ArchiArtifactWriter;`（注释指明「写盘单点的唯一实现，本处是通用会话（含 `/architecture` 页内会话）的注入点」）。
- [x] `src-tauri/src/commands/change_flow/mod.rs`：同上（同型一处，change 流程相位会话的注入点）。
- [x] 编译传导（机械、零断言变化）：`crates/infra/agent/src/compose_test.rs` 与 `crates/infra/agent/src/worker_test.rs` 的每个 `compose_turn(...)` 调用点末位补 `None`——MUST NOT 改动任何既有断言、用例或测试名。

## 阶段 3 · 工具面（挂载 / 沙箱臂 / 执行体）

- [x] `crates/infra/agent/src/sdk/mount.rs`：新增常量 `pub(crate) const ARCHI_GENERATE_TOOL: &str = "archi_generate";`（doc 写明「专用**写**工具名：只写三类生成物，不写模型、不写契约；执行体在 `sdk/archi_tool`」）。
- [x] `crates/infra/agent/src/sdk/mount.rs`：新增 `archi_generate_definition()`——`parameters: serde_json::json!({ "type": "object", "properties": {} })`（无属性、无 `required`），描述按 `design.md` 附录 B 五句逐句在场（用法与三字段 / 零产出正常 / 孤儿常态 / 覆盖风险与 `git status` / 冲突处置），零本仓命令名、零「`.schema.json` 是生成物」表述。
- [x] `crates/infra/agent/src/sdk/mount.rs`：`build_registry` 在 `archi_relation` 项之后、`db_tables` 之前插入 `archi_generate` 项（`ToolTier::Write` / `ToolExec::Archi`）；`Vec::with_capacity(18)` → `19`；`registry()` 与 `build_registry()` 两段 doc 的「十八项 / 十八个字面构造项」及其构造序描述改「十九」并把新项写进序；`ToolTier::Read` / `ToolTier::Write` 的 doc 名单、`ToolExec::Archi` 的「七个 archi 工具（五只读 + 两写）」→「八个（五只读 + 三写：两模型写 + 一生成物写）」同批随动。
- [x] `crates/infra/agent/src/sdk/mount.rs`：`archi_api` 描述第 ③ 句「过期用既有生成命令重生成」改「过期调 `archi_generate` 重生成」（四句结构不变）；`ARCHI_API_TOOL` 常量 doc 的「重生成仍由会话经既有 `bash` 命令完成」整句删除，改指 `archi_generate`。
- [x] `crates/infra/agent/src/sdk/tools.rs`（`D-10`）：`pub(crate) use crate::sdk::mount::{…}` 名单补 `ARCHI_GENERATE_TOOL`（供 `archi_tool` 以 `tools::ARCHI_GENERATE_TOOL` 引用，与既有七个常量同形）；`input_path` 的显式 `None` 臂补该名；模块 doc 的「七个架构工具（五只读 + 两写）」改「八个 archi 工具（五只读 + 三写：两模型写 + 一生成物写）」。若 D-10 被拒：`archi_tool` 直引 `mount::ARCHI_GENERATE_TOOL`，接受该 doc 计数失真（`design.md` Q-3）。
- [x] `crates/infra/agent/src/sdk/archi_tool.rs`：`ArchiContext` 增两字段 `writer: Option<Arc<dyn archi::ArtifactWriter>>` 与 `generate: bool`（doc：`remedy` 第二 / 第三态判据位，mint 时按名字清单置位）；构造子 `with_writer(self, writer: Option<Arc<dyn archi::ArtifactWriter>>) -> Self`（与 `with_protect` 同型）与 `with_generate(self, available: bool) -> Self`；`new` 置 `None` / `false`。
- [x] `crates/infra/agent/src/sdk/archi_tool.rs`：新增 `generate_payload(&self) -> Result<Value, String>`（形态见 `design.md` 附录 E）——端口缺席即 `Err`（点名装配缺失、生产不可达）→ `archi::workspace_render(&self.root)`（`ApiError::message()` 透传，结构错带 `file:line`）→ `protect` 缺席同款 `Err` → 逐份 `ProtectRules::evaluate_target(ARCHI_GENERATE_TOOL, &file.path)`，`conflicts` 非空即 `{ written: [], unchanged: [], conflicts: [...] }` 零写盘 → `writer.write(&self.root, &files)` → `{ written, unchanged, conflicts: [] }`；`path` 串一律经 `archi::display`。
- [x] `crates/infra/agent/src/sdk/archi_tool.rs`：`execute` 增第八臂 `tools::ARCHI_GENERATE_TOOL => self.generate_payload()?`；模块头「五只读 + 两写」→「五只读 + 三写（两模型写 + 一生成物写）」、`execute` doc 的臂清单与「七支共用同一 per-turn 上下文」→「八支」。
- [x] `crates/infra/agent/src/sdk/archi_tool.rs`：`api_payload` 的 `check` 分支加顶层 `remedy`（`design.md` 附录 C 三态）——判据为 `artifacts` 三列是否全空 + `self.generate`；`artifacts` 与 `findings` 两轴内容与结构零改动；键恒在场（`null` 也出键）；`api_payload` 函数 doc 的响应描述随动（`{ artifacts, findings, remedy }`，并补一句「`remedy` 只在工具响应层，不进 DTO / bindings」）。

## 阶段 4 · 矩阵与相位锁

- [x] `crates/infra/agent/src/sdk/runner.rs`：`use crate::sdk::mount::{…}` 名单补 `ARCHI_GENERATE_TOOL`；`mint_archi_context` 判据七名 → 八名（+ 该名），并在铸造时 `.with_generate(names.iter().any(|name| name == ARCHI_GENERATE_TOOL))`；函数 doc 的判据句随动。
- [x] `crates/infra/agent/src/sdk/runner.rs`：`session_pump` 的 archi 上下文接线改为 `with_protect(protect.clone()).with_writer(payload.artifact_writer.clone())`（写作位置紧邻 `ProtectRules` 装配之后的既有注释处）。
- [x] `crates/infra/agent/src/compose.rs`：`archi_matrix` 的 `("proposal","executor")` / `("dev-design","executor")` 两格追加 `mount::ARCHI_GENERATE_TOOL`；`("test-design","executor")` 改 `vec![mount::ARCHI_SEARCH_TOOL, mount::ARCHI_MODULE_TOOL]`；矩阵 doc 补 `generate`（两起草相位 + archi 臂是唯一落点，implement 不给的理由）与 `test-design` 加 `archi_module` 的理由。
- [x] `crates/infra/agent/src/compose.rs`：archi 来源臂名字清单七名 → 八名（`names.push(mount::ARCHI_GENERATE_TOOL);` 接在 `ARCHI_RELATION_TOOL` 之后）；模块头 doc 的「`archi` 来源 = 七工具名 + 七 archi 名（读五 + 写两）」→「+ 八 archi 名（读五 + 三写）」。
- [x] `crates/infra/agent/src/sdk/protect/reason.rs`：新增 `CONTRACT_LOCK_TEMPLATE`（`design.md` 附录 A 逐字）与 `pub(crate) fn contract_lock_template() -> &'static str`（doc：契约资产目录拒串的唯一来源，MUST NOT 复用 `SESSION_LOCK_TEMPLATE`、MUST NOT 在装配层再抄一份）；模块头 doc 补一句「契约串是会话类模板的第二个实例，五类文案形态计数不变」；既有四串逐字不动。
- [x] `crates/infra/agent/src/compose.rs`：`model_write_lock` 的 `Some` 分支由「恰一条」改「恰两条」——第二条 `glob = format!("**/{}/**", foundation::layout::archi_models_rel_dir())`、`reason = crate::sdk::protect::contract_lock_template().to_owned()`；`Some(vec![…])` 内的顺序为模型目录在前、契约目录在后（规则序 = 优先级）。
- [x] `crates/infra/agent/src/compose.rs`：`model_write_lock` 的 doc 与成因注释整段改写——删「契约资产 md 不锁（`implement/executor` 持有 `archi_api`，改代码相位必须能同步契约）」，如实写「契约资产入锁的理由（`apiMissing` / `apiSignatureMismatch` 是唯二 `error` 档）与代价（判据在 `implement` 内不再可自愈，实现相位发现契约错的唯一出路是 backtrack）」，并写明锁面只到 `models/**`（`INDEX.toml` / `.i.*` / `config.json` 不锁）；模块头 doc 的 `protect` 位描述「`Some(一条)`」→「`Some(两条：模型 + 契约)`」。

## 阶段 5 · 知识 / prompt / 提示面

- [x] `crates/infra/agent/src/archi-knowledge.md`：单句换指（`design.md` 附录 D）——「生成物过期用既有命令重生成、MUST NOT 手改」→「生成物过期调 `archi_generate` 重生成、MUST NOT 手改生成物」；零其它字节改动（「读五件 / 写两件」行不动）。
- [x] `crates/infra/agent/src/archi_knowledge.rs`：模块头 doc 的「两写工具」→「三写工具（两模型写 + 一生成物写）」；若有门限表述随实测值更新（本文件零数字门限，数字锚在测试侧）；拼装 `build` / 降级 `knowledge` 语义逐字不动。
- [x] `crates/core/workflow/src/prompts/dev-design-planner.md`：工具行补 `archi_generate`（含「入口不在盘 / 接口面留白的新模块零产出是正常的」与「MUST NOT 手改生成物」）；接口面纪律句删括注「（实现相位同一条纪律）」。
- [x] `crates/core/workflow/src/prompts/proposal-planner.md`：工具行补 `archi_generate`（同上零产出句）；补「本 change 涉及元素的 `<叶段>.md` 由本相位写，路径经 `archi_module` 的 `contractPath` 取（勿手拼目录），且文档 MUST NOT 列接口（导出名归 `.schema.json`）」。
- [x] `crates/core/workflow/src/prompts/test-design-planner.md`：`archi_search` 行后补 `archi_module` 用法句（取 `assets.interface` 的 `ops` / `types` 键名核对 `design.md` 的「公共函数 / API」）+ 边界句「接口定义是手写真相，MUST NOT 用 grep 源码导出的方式替代它」；过程 4b 的既有禁令（「不得 grep 源文件导出声明或虚构条目」）逐字保留。
- [x] `crates/core/workflow/src/prompts/implementation-generator.md`：接口纪律整段反转——① 不改契约（`.schema.json` 与 `.md` 本相位受相位锁保护，改了会被拒）；② 生成物新鲜度自查（`archi_api` 的 `check`，据 `remedy` 转告用户）；③ 两条出路（契约对就改代码追上它 / 契约不可实现或写错就停下述冲突由用户决定 backtrack，或改用架构页会话）；删「改了导出面就同批同步接口定义」与「生成物过期用既有命令重生成」两类正句。
- [x] `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx`：`mode === 'full'` 分支文案改「生成需开 change，或让左侧会话生成」；`data-testid` / 类名 / 位次与 `incremental` 分支（含 `archiApiGenerate` 调用面）零改动。

## 阶段 6 · 版本与只读自查

- [x] `packages/desktop/package.json`：`version` 相对落地时 `HEAD` 现值 +1 patch；`src-tauri/tauri.conf.json`（经 `"version": "../package.json"` 跟随）与 `src-tauri/Cargo.toml`（crate 版本行不跟随）MUST NOT 手改。
- [x] 只读自查（模型面）：`archi_drift` 零 error（覆盖 / 归属 / 接口面判据；`apiFindings` 的构成不因本变更改变）；`archi_module Backend.Core.Archi` 的 `assets` 三份仍在盘、`fileHash` 与写前一致（本轮零模型写入）。
- [x] 只读自查（措辞与计数）：`grep` 三类死指代在产品源码与四份 prompt 内零命中——「本仓的生成命令」「既有命令重生成」「既有生成命令」；逐处核对计数措辞（`mount.rs` 十九项 / `compose.rs` 八名 / `archi_tool.rs` 三写 / `tools.rs` 八个 archi 工具），并确认 `archi_api` 描述内零 `api:export` 一类本仓命令名。
- [x] 只读自查（界外确认）：`packages/desktop/src/types/generated/**` 与 `src/bindings/**` 零 diff（`remedy` 与生成响应都在工具响应层）；`.dev-team/architecture/**` 零字节变化；`src-tauri/Cargo.toml` 依赖清单零新增。机械门禁（`api:check` / `cargo check` / `pnpm` 链）由桌面端执行，agent 内不重复执行。
