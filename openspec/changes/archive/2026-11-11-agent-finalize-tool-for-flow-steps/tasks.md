# 任务: agent-finalize-tool-for-flow-steps

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 26 项（含 3 新增文件 + 23 修改面）+ 机械改动 27 处 `SessionInjections` 字面构造 / 16 处 `definitions()` 调用点 / 73 处 `SessionToolset` 退场（见 design「编译闭合批」与「事实校准 4/5/6」）。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：新增 / 改造用例（proposal「测试文件」表逐行）归 test-design / test-gen / test-execution 相位，`cargo nextest run --workspace` / `pnpm -C packages/desktop test` 等回归命令同样**由桌面端 test-execution 相位承接**（约束 ⑤）。本相位对测试文件的唯一动作是**让工作区随字段面 / 签名变化编译通过**（`SessionInjections { …, finish: … }` / `WorkerTurnOutcome { …, finalized: … }` / `definitions(&inj, prompt)`），MUST NOT 新增用例、MUST NOT 改断言口径、MUST NOT 把用例绿灯写成完成条件。夹具的**语义**改造（`FakeWorker` 由「文本 JSON」改为「注 `SystemNotice{finish_step}` 转录取值」）同样是测试相位的活，本相位只补必需字段的**占位值**（`finalized: None`）。
>
> **依赖序（九阶段）**：
>
> - **阶段一**（Step 0：`mount.rs` 骨架 + checklist 解析器 + `derive_verdict` 迁位）纯新增 / 行为零变，**可独立提交**（proposal §"①内部再切"的回滚点）。它必须先于一切：`registry()` 与 `checklist_entries()` 是后续所有阶段的地基，此处出错可确定问题在「数据」而非「接线」。
> - **阶段二**（core 词汇面换血：删 `SessionToolset` / 加 `FinishSpec` / `SessionInjections` 改三位）**破坏性**，编译期全暴露；与阶段三**同一提交**（拆开即工作区不可编译）。
> - **阶段三**（infra 消费面接线：`policy` tier / `loop` 分派 / `compose` 名字清单 / `runner` 铸造判据 + 编译闭合批）依赖阶段一、二。
> - **阶段四**（② 协议实现：`finish.rs` + 两个校验器 + `prompt` 改写 + `port`/`transcript` + `worker`）依赖阶段一（`checklist_entries`）与阶段二（`FinishSpec`）。
> - **阶段五**（② loop 内收口 + compose `finish` 位 + 8 份 md 改写）依赖阶段三、四。
> - **阶段六**（② walker：verdict 门合并 / 反馈环退役 / 升级落账面删除 / `decision_session` 改写）依赖阶段四、五。
> - **阶段七**（③ 收尾：机械豁免复核 + 死面 `grep` 自查）依赖阶段六。
> - **阶段八**（前端工具卡）零 Rust 依赖，可与阶段四至七并行；但须在阶段九之前（版本行覆盖）。
> - **阶段九**（版本 + 边界自查 + 交接备忘）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **收口三纪律**（design D7）：同批假结果 `is_error: false` 且**真经 `sender` 发出**、`results` 收齐**先于** `return`、`Ok` 后零额外请求。任何一处写错即 provider 400 或转录语义污染——这三条**不得**为了「简化」而调整顺序。
> ① **`protect/**` 与 `tools::input_path` MUST 零 diff**（D9）：该两处出现 diff 即说明实现偏离（`finish_step` 走既有默认臂，不加特判）。
> ② **两处「不要修改」的边界**：`crates/core/orchestration/src/walker.rs` 的其余四处机械 checklist 代写点（`step_gate_fail_phase_log` / `test_execution_phase_log_input` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log`，合计 7 个条目文案）与 `feedback_limit` 本体逐字不动；`phase_table.rs` 零改动（`VerdictGate` 步的 `on_fail` 声明保持原位成惰性字段）。
> ③ **单点纪律**：`derive_verdict` 只有一份（`workflow::model`）；`finish_payload` 的提取实现只有一份（`orchestration::transcript`，`worker.rs` 与测试夹具都调它）；`checklist_entries` 只被 `finish::parameters` 与 `validate_checklist` 共用（MUST NOT 各写一份扫描）。
> ④ **生成物 / 版本纪律**：`src/types/generated/**` 不手改（本变更应零 diff）；`package.json` 版本按「落地时 `HEAD` +1 patch」相对递增，MUST NOT 写死字面号；`src-tauri/Cargo.toml` crate 版本行不跟。
> ⑤ **门禁分工**：静态脚本链（`pnpm run check` = `server:check` → `bindings:export` → `api:check` → `client:check`）与**测试回归**（`cargo nextest run --workspace`、`cargo test`、`pnpm -C packages/desktop test`）由桌面端 static-check / test-execution 相位执行，本相位不运行、不重复执行。据此本文件的自查项**只收源码级静态判据**——`cargo clippy`、`grep` 零命中、读码结构复核（如「下发面逐名不变」）、`git diff` 边界比对、前端 `vp check` / `knip`；凡**测试回归**与**整链门运行**一律改为**「交接 …（由桌面端 <相位> 承接）」**的任务条目，**MUST NOT** 写成任何阶段自查项里的「全绿 / 通过」判据。

## 阶段一：Step 0 —— `mount.rs` + checklist 解析器 + verdict 推导迁位（可独立提交，`registry` / 解析器的守卫面先行就位）

> **「纯新增」的口径**：阶段一**不引入任何行为变更**——阶段一结束时所有既有调用点仍走原语义（`tools::definitions` 保留为**一行委派 shim**），`mount::registry()` / `checklist_entries()` 已是完整实现但尚无生产消费者。这样 `mount_test` / `checklist_test` 的守卫面可以在测试相位**先行就位**（用例编写与回归执行均不由本相位承担，见阶段九交接与约束 ⑤），后续出错即可确定问题在「接线」而非「数据」（proposal §"①内部再切"）。
>
> **字面量全程单份**：定义函数与工具名常量在阶段一即**迁入** `mount.rs`（`tools.rs` 不留副本），`tools.rs` 以 `use` + 一行 `definitions` shim 维持编译；shim 在阶段三删除。这是本设计对 proposal「纯新增」的唯一收窄——换来「同一份定义字面量只存在一处」，代价是 `tools.rs` 在阶段一有 5 行 import 与 3 行 shim 的 diff。

- [x] 新建 `crates/infra/agent/src/sdk/mount.rs`：定义 `ToolMount { definition: rig::completion::ToolDefinition, tier: ToolTier, exec: ToolExec }`（`Debug + Clone`，**不派生 `PartialEq`**——内含 `serde_json::Value`）
- [x] 同文件定义 `ToolTier { Read, Write, Execute, Protocol }`（`Copy + Eq`；`Protocol` = 任何档位放行）与 `ToolExec { Pure, ExploreNote, Archi, FinishStep(agent::FinishSpec) }`（`Copy + Eq`。**阶段一 `FinishSpec` 尚未引入**：先以 `ToolExec { Pure, ExploreNote, Archi }` 三臂落地，阶段二给 `FinishStep(FinishSpec)` 补臂——`ToolExec` 是本 crate 私有形态，两阶段之间的窄形态不构成对外契约）
- [x] 同文件迁入工具名常量（**作者单点**，D4）：`pub(crate) const TOOL_NAMES: [&str; 7]`、`EXPLORE_NOTE_TOOL`、`ARCHI_MODULE_TOOL`、`ARCHI_SEARCH_TOOL`——自 `sdk/tools.rs:37/42/47/51` **逐字搬入并删除原文**；`tools.rs` 顶部改为 `use crate::sdk::mount::{ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL, EXPLORE_NOTE_TOOL, TOOL_NAMES};`（`input_path` / 执行体引用面随 `use` 生效，零语义变化）
- [x] 同文件迁入定义函数（**字面量零改**）：`generic_definition(name)`（七臂 match）、`vocabulary(iter)`、`archi_module_definition()`、`archi_search_definition()`、`explore_note_definition()`（承载原 `tools.rs:79-96` 的 `create_explore_note` 定义字面量）——自 `tools.rs` **整块搬出**，`tools.rs` 内对它们的引用改指 `mount`
- [x] 实现 `registry()`：`static REGISTRY: OnceLock<Vec<ToolMount>>` + `get_or_init(build_registry)`，签名为 `pub(crate) fn registry() -> &'static [ToolMount]`（D2）；`build_registry()` 的**十个字面构造项按序** = `read` / `grep` / `glob` / `ls` / `write` / `edit` / `bash` / `create_explore_note` / `archi_module` / `archi_search`——**前七项必须恰是七内建**（`tools: None` 的语义载体）；tier 归属：Read 六项（`read`/`grep`/`glob`/`ls`/`archi_module`/`archi_search`）、Write 三项（`write`/`edit`/`create_explore_note`）、Execute 一项（`bash`）；`exec` 归属：七内建 `Pure`、`create_explore_note` `ExploreNote`、两个 archi 工具 `Archi`
- [x] 实现 `lookup(name: &str) -> Option<rig::completion::ToolDefinition>`（`registry()` 按名取定义并克隆出口）——阶段一唯一的 registry 消费面，也是「`registry()` 不触发 `dead_code`」的保证；阶段三起兼作 compose 成员校验与 `definitions` 取表
- [x] `crates/infra/agent/src/sdk/tools.rs` 的兼容 shim（阶段三删除，D13）：保留**旧签名 + 旧语义 + 旧顺序**，但定义数据全部取自 `mount::lookup`（零字面量副本）：
  ```rust
  pub fn definitions(injections: &agent::SessionInjections) -> Vec<ToolDefinition> {
      // tools 收窄（旧语义：TOOL_NAMES 保序，未知名静默滤过）
      let mut picked: Vec<&str> = match &injections.tools {
          None => TOOL_NAMES.to_vec(),
          Some(list) => TOOL_NAMES
              .iter()
              .copied()
              .filter(|name| list.iter().any(|n| n == name))
              .collect(),
      };
      // toolset 追加（旧语义；枚举阶段二才退场）
      match &injections.toolset {
          Some(agent::SessionToolset::Explore) => picked.push(EXPLORE_NOTE_TOOL),
          Some(agent::SessionToolset::Archi { module, search }) => {
              if *module { picked.push(ARCHI_MODULE_TOOL); }
              if *search { picked.push(ARCHI_SEARCH_TOOL); }
          }
          None => {}
      }
      picked.iter().filter_map(|name| mount::lookup(name)).collect()
  }
  ```
  **MUST NOT** 在阶段一把未知名改成 `Err`（那会把事实校准 3 的语义切换提前，令阶段一不再是「行为零变」——既有「非法名滤除」语义须原样保留，其回归核验由测试相位承接）。`mount::definitions(inj, prompt) -> Result<…>` 的 `Result` 形态**在阶段三**才落——届时它接真调用点，不必过 `dead_code` 检查
- [x] `crates/infra/agent/src/sdk/mod.rs`：挂 `pub(crate) mod mount;` 与 `#[cfg(test)] mod mount_test;`（测试文件本体由测试相位填充——**本相位只落模块声明与空壳文件头，不写用例**）
- [x] 新建 `crates/core/workflow/src/model/checklist.rs`：`ChecklistEntry { id: String, item: String }` + `pub fn checklist_entries(prompt: &str) -> Vec<ChecklistEntry>`——三规则实现：① 只扫 `## 静态清单` 起、下一个 `## ` 止（`###` 子表在段内但行形态不匹配首列 ID，双保险）；② 行首匹配 `| <ID> |`（`[A-Z]+[0-9]+`，**两侧空格约束**，手写字符类判定、**不引 `regex`**）；③ 按出现序去重；同时取表第二列为 `item`（按 `|` 分割后 trim）
- [x] `crates/core/workflow/src/model/mod.rs` 挂 `pub mod checklist;` 并再导出 `checklist::{checklist_entries, ChecklistEntry}`
- [x] `crates/core/workflow/src/model/domain.rs` 新增 `pub fn derive_verdict(checklist: &[ChecklistItem]) -> Verdict`（自 `phase_log.rs:31` 的私有实现**逐字搬入**：全 pass / 空表 → `Pass`，否则 `Fail`）；`model/mod.rs` 再导出 `derive_verdict`
- [x] `crates/core/workflow/src/write/phase_log.rs`：删私有 `derive_verdict`，`use crate::model::derive_verdict;`，调用点改 `let verdict = derive_verdict(&input.checklist);`——`skipped` 约束、report 上限、单事务落库**逐字不动**
- [x] 自查 阶段一（只列静态判据）：`grep -rn "fn derive_verdict" crates/core/workflow/src` 恰 **1** 处（`model/domain.rs`）；`grep -rn "derive_verdict" crates/core/workflow/src/write/phase_log.rs` 只出现在 `use` 行与调用处；`grep -c "fn generic_definition\|fn archi_module_definition\|fn archi_search_definition" crates/infra/agent/src/sdk/tools.rs` 为 **0**（已全部迁出，字面量单份）；`cargo clippy --workspace --all-targets` 通过；读码复核「行为零变」的结构面（`tools::definitions` 仍是旧签名旧语义 shim、shim 内零定义字面量、`phase_log` 调用点改指 `model::derive_verdict`）
- [x] 交接 阶段一（测试回归，本相位不执行）：该阶段「行为零变化」的用例回归核验**由桌面端 test-execution 相位承接**；本相位不运行 `cargo nextest` / `cargo test`，也不以任何用例绿灯作为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，回归结果归承接相位）

## 阶段二：core 词汇面换血（破坏性，与阶段三**同一提交**）

> 阶段二单独不可编译（`toolset` 位删除后 `compose.rs` / `tools.rs` 的引用即红）——它与阶段三构成一个编译闭合批，任务按「改什么」而非「可提交单元」分节。阶段二末的中间态（shim 无 `toolset` 臂）只用于分步核对，不需要独立可提交。

- [x] `crates/core/agent/src/runner.rs`：删除 `SessionToolset` 枚举（含 doc，`:41-58` 整块）
- [x] 同文件新增 `FinishSpec`（D12）：`#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` + `#[serde(rename_all = "camelCase")]` + `pub enum FinishSpec { EvaluatorChecklist, Decision }`；doc 写明「收口档位词汇：core 只承载词汇，不认识 verdict / 动作语义；**不派生 `specta::Type`**（不出线 IPC）」；**不**加 `Default`（避免 `finish` 位被误缺省成「有收口」）
- [x] 同文件改 `SessionInjections`（`:60-75`）：删 `toolset` 位；加 `#[serde(default)] pub finish: Option<FinishSpec>`（第三位）；结构体 doc 改为「`preamble`（会话级系统前导）/ `tools`（本轮下发的工具名字清单，`None` = infra 内建七件）/ `finish`（收口档位，`None` = 本步无收口工具）三位」；`#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]` **逐字保留**（`Eq` 链完好是 AC-3 判据）
- [x] `crates/core/agent/src/lib.rs`：出口名单删 `SessionToolset`、加 `FinishSpec`（其余名逐字不动）
- [x] `crates/infra/agent/src/sdk/mount.rs`：`ToolExec` 补第四臂 `FinishStep(agent::FinishSpec)`（阶段一的三臂形态至此收敛为完整形态；`Copy + Eq` 保持）
- [x] `crates/infra/agent/src/sdk/tools.rs`：删阶段一 shim 的 `toolset` 追加臂（`match &injections.toolset { … }` 整段）——`SessionToolset` 已在本阶段删除，shim 的 `Some(list)` 分支保留即可编译；**shim 本体仍在**（阶段三整条删除）

## 阶段三：infra 消费面接线 + 编译闭合批（依赖阶段一、二）

- [x] `crates/infra/agent/src/sdk/mount.rs` 补齐 `definitions(injections: &agent::SessionInjections, _prompt: &str) -> Result<Vec<ToolMount>, String>` 的真形态（`tools: None` → registry 前七项；`Some(names)` → 按 registry 序过滤、未知名 `Err`（**语义自此由「静默滤过」切为 `Err`**，见事实校准 3））；`injections.finish` **本阶段忽略**（`_prompt` 亦未消费）——`finish` 臂的铸造依赖阶段四的 `orchestration::finish`，随阶段五接线（届时 `_prompt` 改实名）。同时删除阶段一的 `tools::definitions` 委派 shim
- [x] `crates/infra/agent/src/sdk/policy.rs`：删 `READONLY_TOOLS` / `WRITE_TOOLS` / `EXECUTE_TOOLS` / `listed`（整个模块面）；改 `pub fn allows(mode: AgentPermissionMode, tier: ToolTier) -> bool`——`ToolTier::Protocol | ToolTier::Read => true`，`ToolTier::Write | ToolTier::Execute => mode != AgentPermissionMode::Default`
- [x] `crates/infra/agent/src/sdk/tools.rs`：删已迁出的 `TOOL_NAMES` / 三专用常量 / `definitions` / `generic_definition` / `vocabulary` / `archi_module_definition` / `archi_search_definition`；保留 `execute` / `input_path` / `cap_result` / 七个执行体 / 五个上限常量（**逐字不动**，D9）；文件内对名字常量的引用改 `use crate::sdk::mount::{ARCHI_MODULE_TOOL, ARCHI_SEARCH_TOOL};`
- [x] `crates/infra/agent/src/sdk/loop.rs`：① `let definitions = mount::definitions(&turn.injections, &turn.question)?;`——`Err` 分支写引擎日志 + `finish_error(…, "tool_mount_error", error)` + `return history`（不静默降级）；② `let dispatched: Vec<String> = mounts.iter().map(|m| m.definition.name.clone()).collect();`；请求体 `tools` 改 `mounts.iter().map(|m| m.definition.clone()).collect()`；③ 成员校验改按 `mounts` 找 `mount`（`find(|m| m.definition.name == name)`，命中即用该 mount 走后续判定）；④ 档位门改 `policy::allows(turn.permission_mode, mount.tier)`；⑤ 执行分派改 `match &mount.exec` 四臂：`Pure` → `tools::execute(&turn.cwd, &name, &sandboxed)`、`ExploreNote` → `turn.explore_note`（缺席 is_error 回灌）、`Archi` → `turn.archi`（同上）、`FinishStep(_)` → 本阶段先走「未实现」的 `Err`（阶段五接真收口）；`sandbox_rewrite` / `deny` / `finish_success` / `finish_error` / `defend` / 截断与异常终局分支**逐字不动**
- [x] `crates/infra/agent/src/sdk/runner.rs`：`mint_archi_context(names: &[String], workspace_root: &Path) -> Option<ArchiContext>`（判据 `names.iter().any(|n| n == ARCHI_MODULE_TOOL || n == ARCHI_SEARCH_TOOL)`）；explore 铸造判据改 `open.injections.tools.as_deref().map(|names| names.iter().any(|n| n == EXPLORE_NOTE_TOOL)).unwrap_or(false)`；两处 doc 由「`toolset == …`」改「名字清单含 …」；`use` 面去 `SessionToolset`、改引 `mount` 常量
- [x] `crates/infra/agent/src/compose.rs`：`assemble_injections` 重构为「① 按 source 产名字集合（explore → 六名 + `create_explore_note`；archi → 七名 + 两 archi 名；change → 七名 + 矩阵勾选名；其余 → `None`）；② **独立**按 role 置 `finish` 位——**本阶段一律 `None`**（role 置位随阶段五接线，避免与协议实现纠缠）；③ 每个名字过 `mount::lookup` 校验（`None` 即非法名），非法名 `Err(format!("注入面非法工具名: {name}（挂载清单外，来源 {source}）"))`」；`archi_matrix(phase, role) -> Option<Vec<&'static str>>`（7 行值改名字常量）；三臂 doc 同步；`change_phase_role` / `compose_turn` / `resolve_agent_engine` / `map_protocol` 逐字不动
- [x] 编译闭合批 —— `SessionInjections` 字面构造 **27 处**补位（`toolset: …` → `finish: None`）：`core/agent/src/kernel_test.rs`(1) / `core/agent/src/runner_test.rs`(5) / `infra/agent/src/compose_test.rs`(1) / `infra/agent/src/sdk/loop_test.rs`(6) / `infra/agent/src/sdk/runner_test.rs`(3) / `infra/agent/src/sdk/tools_test.rs`(9)（`compose.rs` 的 3 处已随臂重写）。同时在 `mount.rs` 落 `#[cfg(test)] pub(crate) fn injected(names: &[&str]) -> agent::SessionInjections` 夹具供后续压平
- [x] 编译闭合批 —— `definitions()` **16** 个调用点补 `prompt` 实参：生产 `sdk/loop.rs:163` 传 `&turn.question`；测试 `tools_test`(13) / `loop_test`(1) / `runner_test`(1) 传 `""` 或被测 prompt 字面量，并对 `Result` 取值（`.expect("合法注入面")` 形态，MUST NOT `unwrap` 静默）
- [x] 编译闭合批 —— `SessionToolset` 全仓 **73** 处退场（12 文件）：`core/runner_test`(27) / `compose_test`(8) / `core/lib_test`(7) / `tools_test`(6) / `sdk/runner_test`(6) / `sdk/runner.rs`(4) / `loop_test`(4) / `compose`(4) / `tools`(2) / `core/runner.rs`(2) / `lib.rs`(1) 随实现面删除；`core/workflow/src/write/phase_table_test.rs`(2) 仅**注释文案**改写（`SessionToolset` → `infra 挂载清单`），**断言本体零改**（proposal 明列）
- [x] `packages/desktop/src-tauri/src/bindings/mod_test.rs:429`：反向锚定清单 `["SessionInjections", "SessionToolset", "TurnQuestion"]` → `["SessionInjections", "FinishSpec", "TurnQuestion"]`（注释段 `:412` / `:421` 的同句一并改写；**不新增断言**）
- [x] 自查 阶段三（只列静态判据）：`grep -rn "SessionToolset" --include=*.rs packages/desktop/src-tauri` **零命中**；`grep -rn "toolset" --include=*.rs packages/desktop/src-tauri/crates | grep -v _test` **零命中**；`grep -c "definitions(" crates/infra/agent/src/sdk/loop.rs` 为 1 且带两实参；`cargo clippy --workspace --all-targets` 通过；读码复核「下发面逐名不变」的结构面（`registry()` 十项的名字与 tier 声明、`definitions()` 按名查表、`compose` 三臂名字清单与改造前枚举取值逐名对应）
- [x] 交接 阶段三（测试回归，本相位不执行）：`RunStarted.tools` 的既有断言与 `tools_test` 的「非法名滤除」断言**由桌面端 test-execution 相位承接**核验。其中「非法名」语义由「静默滤过」变 `Err` 是本变更的**有意变更**，该断言的改造属测试相位（见阶段九交接），不作为本相位的完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，回归结果归承接相位）

## 阶段四：② 协议实现（`finish.rs` + 两个校验器 + prompt 改写 + 载荷出口）

- [x] 新建 `crates/core/orchestration/src/finish.rs`：`pub const TOOL_NAME: &str = "finish_step";`（D4 的名字单点）
- [x] 同文件 `pub enum FinishPayload { Evaluator(EvaluatorChecklist), Decision(DecisionAction) }`（D12：`Debug + Clone + PartialEq + Eq + Serialize + Deserialize` + `#[serde(rename_all = "camelCase")]` 外部 tag）
- [x] 同文件 `pub fn description(spec: FinishSpec) -> &'static str`：evaluator 形态 = 「本步收尾。按下列清单逐项给出结果；report ≤2000 字符。」+ 一句「ID 取自本 prompt 的静态清单，逐项给 evidence」；decision 形态 = 一段「三动作封闭集（backtrack / stop / ask）+ `to` 必须取 prompt 白名单内相位 + reason ≤500」；两形态均 <500 字
- [x] 同文件 `pub fn parameters(spec: FinishSpec, prompt: &str) -> serde_json::Value`：evaluator 臂 = `{ type: "object", properties: { report: {type:"string", description: "≤2000 字符"}, items: { type:"array", minItems: 1, items: { type:"object", properties: { id: { type:"string", enum: <checklist_entries(prompt) 的 ID 集> }, pass: {type:"boolean"}, evidence: {type:"string"} }, required: ["id","pass","evidence"] } } }, required: ["report","items"] }`——`enum` 数组在 ID 集为空时**省略该键**（D11 退化面，空数组是非法 JSON Schema 形态）；decision 臂 = 三动作 `oneOf`/`anyOf` 形态（`action` 枚举 + 各动作必填字段）
- [x] 同文件 `pub fn validate(spec: FinishSpec, args: &serde_json::Value, prompt: &str) -> Result<FinishPayload, String>`：按 spec 分派 `validate_checklist` / `validate_decision` 并包成 `FinishPayload` 对应臂
- [x] `crates/core/orchestration/src/verdict.rs`：删 `extract_json` / `fenced_block` / `brace_slice` / `parse_verdict`（整段）；`EvaluatorChecklist` 加 `Serialize`（字段与 serde 属性零变）；`MAX_REPORT_CHARS = 2000` **保留**（`infra/archi/src/runner.rs:19` 消费者在场）
- [x] 同文件新增 `pub fn validate_checklist(args: &serde_json::Value, prompt: &str) -> Result<EvaluatorChecklist, String>`：① 形状（`args` 为对象、`report` 为字符串、`items` 为非空数组；逐项 `pass` 为布尔、`evidence` 为字符串）→ ② ID 三分类（D11：**缺项** = 期望集 − 收到集；**清单外 ID** = 收到集 − 期望集；**未知 ID** = 元素形态层 id 缺失 / 非字符串 / 空串；三类文案分列且各含字段名 + 期望 ID 集 + 实际收到的 ID）→ ③ `id → item` 映射（`checklist_entries(prompt)` 的第二列正文，映射 miss 不可能出现——② 已保证集合相等）→ ④ `report` 上限（文案含上限 / 当前值 / 超出量）→ ⑤ `verdict` 由 `workflow::model::derive_verdict(&checklist)` 推导（模型传入的 `verdict` 字段**完全忽略**，不读不校验）
- [x] `crates/core/orchestration/src/decision.rs`：删 `parse_decision`；`DecisionAction` 加 `Serialize`（`tag = "action"` / camelCase / 三动作字段零变）；新增 `pub fn validate_decision(args: &serde_json::Value) -> Result<DecisionAction, String>`（`action` tag ∈ 封闭集 → 各动作必填字段（`backtrack`: `to` + `reason`；`stop`: `reason`；`ask`: `question` + 非空 `options`）→ `reason ≤ MAX_REASON_CHARS`）；`ensure_backtrack_allowed`（`:67`）与 `MAX_REASON_CHARS`（`:6`）**逐字不动**
- [x] `crates/core/orchestration/src/prompt.rs`：`evaluator_protocol`（`:46`）改写为「① 禁止调用 MCP 工具 phase_log / phase_next / phase_start / backtrack（落账由桌面编排代写）；② 评估完成后**调用 `finish_step` 工具收口**（参数形状由工具 schema 给出，无需在消息里输出任何 JSON）；③ `report` ≤ 2000 字符；④ 逐项 checklist 需 ID（取自本 prompt 的静态清单）+ pass + evidence」——删 JSON 形状块与 `verdict / phase / attempt / skipped` 措辞，**保留**禁调红线与上限
- [x] 同文件 `decision_prompt`（`:75`）的 `## 决策协议（最终消息必须输出且仅输出一个 JSON 对象，三选一）` 段改为工具协议（`## 决策协议（调用 finish_step 提交三选一动作，形状由工具 schema 给出）` + 三动作一行说明 + 「backtrack 目标须在白名单内、reason ≤500」红线），**删 JSON 代码块**；失败报告 / 失败清单 / 白名单 / 候选报告四段与 `backtrack_option_*` 三助手**逐字不动**
- [x] 同文件删 `verdict_fix_prompt`（`:64`）与 `decision_fix_prompt`（`:141`）；新增 `pub fn decision_answer_prompt(answer: &str) -> String`（用户应答原文 + 一句「请基于应答调用 `finish_step` 提交决策」；唯一再提示面）
- [x] `crates/core/orchestration/src/port.rs`：`WorkerTurnOutcome`（`:76`）增 `pub finalized: Option<crate::finish::FinishPayload>`（`Debug + Clone` 保持；`final_message` 字段**保留不动**——它仍是「末条 assistant 文本」的中性载体，本变更只是不再由 verdict / decision 路径消费）
- [x] `crates/core/orchestration/src/transcript.rs`：新增 `pub fn finish_payload(transcript: &[agent::AgentEvent]) -> Option<FinishPayload>`（倒序找末条 `SystemNotice { subtype, payload }` 且 `subtype == crate::finish::TOOL_NAME` → `serde_json::from_value(payload.clone()).ok()`；缺失 / 损坏 → `None`）；`final_assistant_text` / `failure_reason` 逐字不动
- [x] `crates/core/orchestration/src/lib.rs`：挂 `pub mod finish;`；`pub use finish::{description, parameters, validate, FinishPayload};`
- [x] `crates/infra/agent/src/worker.rs`：`WorkerTurnOutcome { session_id, status, final_message: final_assistant_text(&transcript), transcript, finalized: orchestration::transcript::finish_payload(&transcript) }`（注意 `transcript` 被两处借用——先算 `finalized` 再移 `transcript`，或 clone 一次）
- [x] 编译闭合批 —— `WorkerTurnOutcome` 字面构造 **4 处**补字段：`core/orchestration/src/walker_test.rs`(3) / `core/orchestration/src/port_test.rs`(1) 各补 `finalized: None`（生产 `worker.rs` 已在上一条落地）。**夹具语义改造归测试相位**
- [x] 自查 阶段四（只列静态判据）：`grep -rn "parse_verdict\|parse_decision\|extract_json\|fenced_block\|brace_slice\|verdict_fix_prompt\|decision_fix_prompt" --include=*.rs crates | grep -v _test` **零命中**；`grep -c "fn derive_verdict" crates/core/workflow/src` 仍为 1（校验器复用 `workflow::model::derive_verdict`，MUST NOT 复制实现）；`cargo clippy` 通过；读码复核 `finish_payload` 唯一的提取实现落在 `transcript.rs`、`worker.rs` 只调它不另写
- [x] 交接 阶段四（测试回归，本相位不执行）：`verdict_test` / `decision_test` 的文本解析用例删除与 `Value` 校验用例新增，以及 `transcript_test` / `port_test` / `worker_test` 的载荷提取用例，**由桌面端 test-design / test-gen / test-execution 相位承接**；本相位不以任何用例通过为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，用例结果归承接相位）

## 阶段五：② loop 内收口 + compose `finish` 位 + 8 份 md 改写

- [x] `crates/infra/agent/src/sdk/mount.rs` 补齐 `definitions()` 的 `finish` 臂（**依赖阶段四的 `orchestration::finish` 已落地**）：`Some(spec)` 时按 `orchestration::finish::{TOOL_NAME, description, parameters}` 铸造一项 `ToolMount { definition: ToolDefinition { name: TOOL_NAME.to_owned(), description: description(spec).to_owned(), parameters: parameters(spec, prompt) }, tier: ToolTier::Protocol, exec: ToolExec::FinishStep(spec) }` 追加到末尾（**静态 `registry()` 中不得包含 `finish_step`**）
- [x] `crates/infra/agent/src/sdk/loop.rs` 的 `FinishStep(spec)` 臂接真收口，顺序**严格**按 D7：① 循环头 `if finalized.is_some()` → 假结果文案 `"本步已 finish_step 收口，该调用未执行"`、`is_error: false`、`sender.send(Message { role: Tool, blocks: [ToolResult { id, content, is_error: false }] })`（**真发**）、`results.push(outcome_result(tool_call, content))`、`continue`；② `ToolExec::FinishStep(spec)` 臂调 `orchestration::finish::validate(*spec, &input, &turn.question)`——`Ok(payload)` → `finalized = Some(payload)` + 结果文案「已收口」+ `is_error: false`；`Err(message)` → `(message, true)`（当场回灌，同一 turn 内可重试）；③ `for` 结束后 `if !results.is_empty() { history.push(Message::tool_results(results)); }` **先于**收口；④ `if let Some(payload) = finalized { sender.send(SystemNotice { subtype: TOOL_NAME, payload: serde_json::to_value(&payload)? }); finish_success(...); return history; }`；⑤ `turn_index += 1;` 仅在后无 `finalized` 时走到
- [x] `crates/infra/agent/src/compose.rs`：把阶段三的「`finish` 位一律 `None`」改为按 role 置位（D5）——`"evaluator"` → `Some(FinishSpec::EvaluatorChecklist)`、`"decision"` → `Some(FinishSpec::Decision)`、其余 → `None`；**必须与矩阵未命中行解耦**（决策会话 `(fail_phase, "decision")` 恒不命中矩阵，`finish` 位不得随矩阵 `None` 一并丢）；explore / archi 两臂 `finish: None`
- [x] `crates/core/workflow/src/prompts/{acceptance,code-analyze,code-review,dev-design,implementation,proposal,test-design,test-gen}-evaluator.md`（8 份）：把同一句 `- 只产出评估结论：最终消息输出一个 checklist JSON（形状与落账红线由桌面端在会话 prompt 尾部追加的输出协议给出）。` 改为 `- 只产出评估结论：调用 finish_step 工具收口（形状与落账红线由工具参数与桌面端追加的输出协议给出）。`——**8 份逐字一致**，其余正文（尤其 `## 静态清单` 三列表结构）**零改动**
- [x] 自查 阶段五（静态判据）：8 份 md 的新串各 **1** 处命中（`grep -cF '调用 finish_step 工具收口' <8 份>` 恒 1）、旧串全 `prompts/*.md` **零命中**（`grep -cF '最终消息输出一个 checklist JSON' crates/core/workflow/src/prompts` 为 0）；6 份 `*-planner.md` / `*-generator.md` 的 `finish_step` **零字样**；`grep -c "static REGISTRY" crates/infra/agent/src/sdk/mount.rs` 为 1 且 `registry()` 内**无** `finish_step` 字面量
- [x] 交接 阶段五（测试回归，本相位不执行）：`mount_test`（`finish` 位置位时追加 `finish_step` 且 `enum` == prompt ID 集 / 其余位不追加）与 `phase_table_test`（8 份项数 5/8/7/10/7/11/9/8 + 段落与全文 ID 集合相等）**由桌面端 test-design / test-gen / test-execution 相位承接**；`loop_test` 的收口三纪律断言同理。本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，用例结果归承接相位）

## 阶段六：② walker —— verdict 门合并 / 反馈环退役 / 升级落账面删除 / `decision_session` 改写

- [x] `crates/core/orchestration/src/walker.rs`：`step_verdict_gate`（`:1563`）与 `step_verdict_gate_terminal`（`:1818`）**合并为单一裁决**，去掉 `step` 形参（`:554` 的调用点同步改）；体内：`eval_outcome` 缺席 → 计划图漂移 `Terminal::failed`（文案与现 `:1610` 逐字同款）；`let Some(FinishPayload::Evaluator(checklist)) = outcome.finalized.clone()` → `emit_node(passed, Some(format!("verdict={}", checklist.verdict.as_str())))` + `cursor.verdict = Some(checklist)` + `Ok(StepFlow::Proceed)`；`None`（或 `Decision` 臂——理论不可达）→ `emit_node(failed, Some(diagnose_brief(&reason)))` + `Terminal::failed(reason)`，`reason` 按成因两文案：转录含 `output_truncated` notice → 「评估输出被截断（`output_truncated`），未产生 `finish_step` 收口产物」；否则 → 「未调用 finish_step 收口（相位 evaluator 会话 Completed 但无收口载荷）」
- [x] 同文件删除 `step_verdict_gate` 的整个反馈环（`loop` / `feedback` 计数 / `run_worker(…, EdgeKind::Feedback)` 调用 / `feedback_limit(step, "verdict-gate")` 调用点 `:1574`）；`on_fail` 分派随之消失
- [x] 同文件删除 `step_verdict_gate_terminal` 整函数（已合入上条）
- [x] 同文件删除 `step_verdict_gate_upgraded_phase_log`（`:2218` 整函数，含 `item: "评估输出格式"` 条目 `:2253`）与其唯一调用点（`:1645` 超限分支）——**同一次改动内完成**，否则零调用者死代码 / 编译期 red
- [x] 同文件 `decision_session`（`:3416`）改写（D8）：去掉 `drift` 计数与 `DECISION_DRIFT_LIMIT`（`:132` 常量删除）；每轮 = `run_worker(…, WorkerRole::Decision, prompt, ModelLevel::High, continue_session, …)` → `decision-log` 幂等挂账（**置于动作裁决之前**，保持现状顺序）→ 读 `outcome.finalized`：`Some(FinishPayload::Decision(DecisionAction::Ask { question, options }))` → `guard.emit(RunUpdate::Ask { … })` → `guard.wait_answer().await`——`None` → `Err(Terminal::stopped("ask 停等被终止"))`；`Some(answer)` → `prompt = decision_answer_prompt(&answer)`、`continue_session = Some(session_id)`、`continue`（**不发 `WhitelistGate`**）；`Some(FinishPayload::Decision(action))` 非 `Ask` → 发 `WhitelistGate` 运行中 + 收口节点后 `ensure_backtrack_allowed` 判定（`Ok` → passed 节点 + `return Ok(action)`；`Err(message)` → failed 节点 + `Terminal::failed(format!("决策越权：{message}"))`——**一次即停**）；`finalized = None` → `Terminal::failed`（文案区分截断 / 未收口，同上口径）；`Some(FinishPayload::Evaluator(_))` → 理论不可达，`Terminal::failed("决策会话返回 evaluator 收口载荷（相位表角色漂移）")`
- [x] 同文件 `import` 面收敛（`:16` / `:26-27` / `:34-35` 四行）：去 `parse_decision` / `parse_verdict` / `decision_fix_prompt` / `verdict_fix_prompt` / `final_assistant_text`（三处消费点随退役消失后成为未用）；加 `crate::finish::FinishPayload`；`use crate::verdict::EvaluatorChecklist` 与 `MAX_REPORT_CHARS` 保留
- [x] 其余面**逐字不动**（显式核查清单）：`feedback_limit` 本体（`:1803`）、`step_gate_fail_phase_log`（`:1925`）/ `test_execution_phase_log_input`（`:1986`）/ `step_test_execution_upgraded_phase_log`（`:2070`）/ `step_mutation_test_upgraded_phase_log`（`:2142`）四处构造点与 7 个条目文案、`resolve_deadlock`（`:3302`）骨架与 `RetryGate` 节点、`build_decision_input`（`:3550`）、`run_worker`（`:3620`）、`truncate_report`（`:2047`）、`diagnose_brief`（`:3810`）
- [x] 自查 阶段六（静态判据）：`grep -rn "step_verdict_gate_upgraded_phase_log" --include=*.rs crates` **零命中**；`grep -rn '评估输出格式' --include=*.rs crates | grep -v _test` **零命中**（**生产面**口径——被测路径已整体删除，残留的两处全在 `walker_test.rs`：一条退役用例的注释与一条 `item` 断言，其整体删除归测试相位，见阶段九交接）；`grep -c 'feedback_limit(step, "verdict-gate")' crates/core/orchestration/src/walker.rs` 为 0，而 `feedback_limit(step, "static-check")` / `(step, "archi-check")` 各 **1**；`grep -rn "DECISION_DRIFT_LIMIT\|parse_verdict\|parse_decision" --include=*.rs crates | grep -v _test` 零命中；`grep -c "ChecklistItem {" crates/core/orchestration/src/walker.rs` 由 **7** 降为 **6**（差额即被退役的 `"评估输出格式"` 构造点）
- [x] 交接 阶段六（测试回归，本相位不执行）：`walker_test.rs` 的 `FakeWorker` 夹具语义改造（`with_evaluator_reports` 18 处 / `with_decision_reports` 35 处改注 `SystemNotice{finish_step}`）、verdict 门两文案 / 决策三形态用例、以及**两条退役用例的整体删除**，均由桌面端 **test-design / test-gen / test-execution 相位承接**（详见阶段九交接备忘）；本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，用例结果归承接相位）

## 阶段七：③ 收尾 —— 机械豁免复核与死面自查

- [x] 复核 `validate` 的唯一调用点是 `loop.rs` 的 `ToolExec::FinishStep` 臂（`grep -rn "finish::validate\|orchestration::finish::validate" --include=*.rs crates` 恰 1 处生产命中）；机械 checklist 的四处构造点**零 diff**（`git diff -- crates/core/orchestration/src/walker.rs` 逐块比对：`step_gate_fail_phase_log` / `test_execution_phase_log_input` / `step_test_execution_upgraded_phase_log` / `step_mutation_test_upgraded_phase_log` 无 hunk）
- [x] 复核「ID 集合相等」不外溢：`grep -rn "checklist_entries" --include=*.rs crates` 只出现在 `workflow::model::checklist`（定义）/ `model/mod.rs`（再导出）/ `orchestration::finish`（`parameters` + `validate_checklist`）三处，**零**出现在 `walker.rs` / `infra/**`
- [x] 复核落账面零变更：`git diff --stat -- crates/infra/store` **空**；`crates/core/workflow/src/write/phase_log.rs` 的 diff 只含 `derive_verdict` 迁位相关行（`skipped` 约束 / report 上限 / `log_phase` 调用逐字未动）
- [x] 复核 `ChecklistItem` 三字段与 `ChecklistItemRecord` 形状：`git diff -- crates/core/workflow/src/model/domain.rs` 只含新增 `derive_verdict`（`Verdict` / `ChecklistItem` 定义零 diff）

## 阶段八：前端 `finish_step` 工具卡

- [x] `packages/desktop/src/components/agent/agent-blocks.tsx` 新增 `FinishStepCard`（`memo` 包装，props 面与 `WebSearchCardBase` 同宽：`toolCallId` / `toolName` / `input` / `output`）：
  - evaluator 形态（`input.items` 为数组时）：摘要行「清单全通过 / 含未通过项」（纯展示推导，**MUST NOT** 写任何持久面）；逐项一行（`id` + pass 标记 + `evidence`，`evidence` 过长按行截断、`truncate` 类不抛错）；`report` 文本置于 `TraceCollapsibleContent` 内（可折叠）
  - decision 形态（`input.action` 为字符串时）：摘要行 —— `backtrack` → `backtrack → <input.to>：<input.reason>`；`stop` → `stop：<reason>`；`ask` → `question` + `options` 逐项；未知 action → 原样
  - 宽容回退：`input` 非对象 / 缺 `items` 与 `action` → `<pre>{JSON.stringify(input, null, 2)}</pre>`（沿 `AskUserQuestionCard` 先例，用 `lib/parse` 的 `asRecord` / `textOf`）
  - 钩子：卡根 `data-testid="block-tool-use"` + `data-tool-id={toolCallId}`（与 `ToolPairCard` 同形）；摘要区 `data-testid="block-finish-step-summary"`；结果正文 `data-testid="block-tool-result"`；`output?.isError` 时 `TraceLine status="error"`
- [x] `packages/desktop/src/components/agent/tool-cards.tsx`：`BUILD_IN_TOOL_CARDS` 增 `finish_step: FinishStepCard`（自 `./agent-blocks` 引入）；其余 6 键与「未注册 → `ToolPairCard`」分支**零改动**
- [x] 自查 阶段八（静态判据）：`git diff --stat -- packages/desktop/src` 只含 `components/agent/{agent-blocks.tsx,tool-cards.tsx}`（`agent-adapter.ts` / `agent-transport.ts` / `agent-messages.tsx` / `hooks/use-agent-session.ts` / `types/generated/**` 零 diff）；`vp check` 通过、`knip` 无新增未用导出（均为静态扫描面）
- [x] 交接 阶段八（测试回归，本相位不执行）：`tool-cards.test.tsx` / `agent-blocks.test.tsx` 的卡片渲染用例**由桌面端 test-design / test-gen / test-execution 相位承接**；本相位不以任何用例结果为完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，用例结果归承接相位）

## 阶段九：版本、边界自查与交接备忘

- [x] 修改 `packages/desktop/package.json` 的 `version`：按**落地时 `HEAD` 现值 +1 patch** 相对递增（当前盘面 `0.6.33`；判据取 `git diff HEAD -- packages/desktop/package.json` 的版本行恰 `+1`，MUST NOT 写死字面号）；`dependencies` / `devDependencies` **零变化**
- [x] 核对 `packages/desktop/src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零手改，跟随即生效）；`src-tauri/Cargo.toml` 的 crate 版本行**未被改动**
- [x] 自查 边界零 diff（逐项 `git diff --stat`）：`crates/infra/agent/src/sdk/protect/**`、`crates/infra/agent/src/sdk/{sandbox,context,compact,resume,normalize,web_search,bash,model_spec}.rs`、`crates/infra/store/**`、`crates/core/workflow/src/write/phase_table.rs`、`crates/core/workflow/src/prompts/*-{planner,generator}.md`（6 份）、`packages/desktop/src/lib/**`、`packages/desktop/src/hooks/**`、`packages/desktop/src/components/agent/{agent-messages.tsx,agent-adapter.ts(x) 相邻面}`、`packages/desktop/src/types/generated/**`、`.dev-team/architecture/models/**`、全部 `Cargo.toml` —— 以上**整文件零 diff**（断言面与代码面皆零）。**注释单向例外**：`sdk/archi_tool.rs` / `sdk/explore_note.rs` / `crates/infra/agent/src/lib.rs` 三处**仅 doc 注释**随名字清单同步改写（原文「`toolset == …` 铸造」→「名字清单含 … 铸造」）——它们与 `:95`「`toolset` grep 零命中」互相强制（注释留着即判据不成立），故取「注释随名字清单同步、**断言面 / 代码面零 diff**」口径，不列入整文件零 diff 名单
- [x] 自查 关键判据集中复核（AC 逐条，均为 `grep` / 读码 / `git diff` 面，不含任何用例运行）：AC-1 `TOOL_NAMES` 只被 `mount.rs` 与测试引用（`grep -rn "TOOL_NAMES" --include=*.rs crates/infra/agent/src | grep -v _test` 只命中 `mount.rs`）；AC-2 `policy.rs` 四行映射且零常量表；AC-3 `SessionToolset` / `toolset` 全仓零命中（`FinishSpec` / `finish` 位在场）；AC-4 `mount.rs` 的 `registry()` 十项 / `finish_step` 不入静态表；AC-5 loop 收口四步序（代码复核 + 假结果文案字面量在场）；AC-6 `finish_payload` 单点（读码：`worker.rs` 与测试夹具同调同一函数，不各写提取）；AC-7 门合并；AC-8 drift 常量与修正 prompt 零命中；AC-9 三类 ID Err 文案分列；AC-10 死面零命中 + 机械条目零 diff；AC-11 解析器三规则；AC-12 8 份 md 同句 + 6 份零字样；AC-13 注册表增键
- [x] 登记 三处计划外收敛（均为「任务未列、实现必需」的最小面，如实留痕以免下轮重演；三处均不改变 design 任何裁决的语义面）：
  - **① `PhaseCursor.evaluator_level` 字段删除**（`crates/core/orchestration/src/walker.rs`）：tasks 阶段六只列「反馈环 / `on_fail` 分派 / 升格落账面 / `decision_session` / `import` 面」五项，未点名该字段；design「变更清单」的 `walker.rs` 行末亦写「`PhaseCursor` 字段面零变」。该档位**唯一消费者**是 verdict 反馈边的 `run_worker(…, cursor.evaluator_level, …)` 续注，反馈环退役后无消费者 ⇒ `dead_code` 红（`clippy -D warnings` 面），故连同 `step_evaluator` 内 `cursor.evaluator_level = evaluator.model_level;` 赋值一并删除。design 该句的实质意图（`verdict` 槽仍存 `EvaluatorChecklist`、不因协议改形而换类型）逐字成立——零变的是**语义槽位**，不是「一个已无消费者的档位副本」。`executor_level` 保留（static-check / archi-check 反馈边仍在用）
  - **② `Err` 文案的「来源」归属**（`sdk/mount.rs::definitions` vs `compose.rs::injected`）：design D3 / D5 均要求「文案携非法名与**来源**」，但 `definitions` 的入参只有 `SessionInjections` 与 `prompt`，**拿不到 provenance**。取「来源在拿得到它的那一层补」：`compose::injected` 携 `来源 {source}`（装配面，唯一可归因处），`mount::definitions` 只携非法名（防御面，生产不可达——compose 已前置校验）。两处文案同前缀 `注入面非法工具名: {name}（挂载清单外…）` 便于统一检索
  - **③ `decision_session` 的动作臂扁平**（`crates/core/orchestration/src/walker.rs`）：tasks 阶段六描述的是「非 `Ask` → 发 `WhitelistGate` → 判定」的线性序；drift 环删除后该尾部再无 `continue` 需要，实现取 `return match ensure_backtrack_allowed(…) { Ok(..) => Ok(action), Err(..) => Err(..) }`——`let action = match outcome.finalized { … }` 的 `Ask` 分支内 `continue` 仍回到 `loop` 头（ask 应答续轮），非 `Ask` 分支落出后一次性 `return`。**门节点 emit 与文案零变**，仅省去一个 `break`/出口变量
- [x] 交接交付门（本相位不执行，只登记期望并向桌面端移交）：`bindings:export` / `api:check` / `client:check` 三段链（`pnpm -C packages/desktop run check`）与 `packages/desktop/src/types/generated/**` 零 diff 的**再生成核实**归桌面端 **static-check**（期望：`SessionInjections` / `FinishSpec` / `FinishPayload` 不出线，反向锚定已守住，故再生成应零 diff）；**测试回归**（`pnpm -C packages/desktop test`、`cargo nextest run --workspace`）归桌面端 **test-execution**。两者均不写入任何阶段自查项，也不作为本相位的完成判据（约束 ⑤）。**交付物 = 期望登记与移交**（本条在移交发出时即完成，门禁结果归承接相位）
- [x] 交接备忘（发给 test-design / test-gen / test-execution，本相位不写用例；**交付物 = 备忘登记与移交**，本条在移交发出时即完成，用例落地归承接相位）：
  - `mount_test.rs`（新）：`registry()` 十项唯一名与 tier 归属；名字查表；非法名 `Err`；`tools: None` = 内建七件；`finish` 位置位时追加 `finish_step` 且参数 `enum` == prompt ID 集；其余位不追加
  - `policy_test.rs`：4 tier × 3 档 12 格重写 + 与改造前三张常量表**逐格等价**对拍（`create_explore_note` 归 Write、两 archi 归 Read、`bash` 归 Execute）
  - `loop_test.rs`：收口三纪律逐条（假结果 `is_error: false` 且真经 `sender` 发出 / 同批结果进恰一条 user 消息 / 收口后零额外请求 + `TurnDone{is_error:false}`）+ 校验 Err 时 `is_error: true` 回灌且会话继续 + 6 处 `SessionInjections` 构造换位
  - `walker_test.rs`：**`FakeWorker` 夹具语义改造**（`with_evaluator_reports` 18 处 / `with_decision_reports` 35 处 → 改注 `SystemNotice{finish_step}`，沿既有 `with_transcripts()` 注入口；`finalized` 必须经 `orchestration::transcript::finish_payload` 取值——**MUST NOT** 另写提取）；verdict 门两文案；决策三形态（单轮收口 / ask 应答续轮 / 越权一次即停）；**整体删除两条退役用例**：`verdict解析失败反馈边修复_续注同evaluator会话重解析` 与 `真实写面组合_verdict解析失败反馈边超限_升格双槽位决策stop`（含 `item == "评估输出格式"` 断言——被测路径已消失）
  - `verdict_test.rs` / `decision_test.rs`：文本解析用例删除，`Value` 校验用例新增（三类 ID Err 分列 / `item` 映射为 md 正文 / report 超限报当前值 / 模型 `verdict` 无效 / 三动作合法与非法面）
  - `checklist_test.rs`（新）：三规则 + 三条实测反例（`###` 子表行不收 / `T8-U1` 与 `T8-G1` 零命中 / 出现序去重）+ 第二列正文取值
  - `phase_table_test.rs`：8 份项数（5/8/7/10/7/11/9/8，合计 65）+ 全文边界 ID 集合 == 段落内集合（prompt 文本自相位表取得，零文件 IO）
  - `transcript_test.rs` / `port_test.rs` / `worker_test.rs` / `compose_test.rs` / `sdk/runner_test.rs`：载荷提取四态（命中 / 缺 notice / 损坏 / 多条取末条）、`finalized` 字段在场与默认 `None`、提取贯通、三臂名字清单 + `finish` 位按 role 置位、per-turn 铸造名字包含正反例
  - `tool-cards.test.tsx` / `agent-blocks.test.tsx`：`finish_step` 两形态渲染 + JSON 回退 + 注册表键面
  - **不可写面（防误加）**：`sdk/protect/**` 与 `tools::input_path`（D9 的机械判据——出现 diff 即偏离）；`phase_table.rs`（`VerdictGate` 的 `on_fail` 成为惰性字段，清理留独立变更）；机械 checklist 的 7 个条目（MUST NOT 要求提供静态清单 ID）
  - 计数口径提醒：design 的 27 / 16 / 73 是**落地时刻**实测值；测试相位新增夹具会改变字面构造数与 `definitions()` 调用点数，故门禁自查**不得**以绝对数为断言，须以「每个构造点都带 `finish` 位」「每个调用点都带 `prompt` 实参」为判据
