# 设计: archi-check-and-gate

> **变更**: archi-check-and-gate
> **日期**: 2026-10-28
> **基于**: `proposal.md` 与 `specs/{desktop-archi-drift,desktop-archi-gate,desktop-archi-canvas,desktop-change-orchestration,desktop-workflow-write-face,desktop-checks-domain,desktop-change-state-store,desktop-change-queries,desktop-change-flow-view,desktop-corpus-regression,desktop-crate-layout,repo-archi-model,workspace-layout-resolution}/spec.md`

---

## 提案与规格同步状态

proposal 的范围裁决（含五条 explore 落地口径）、两份 ADDED 能力 spec 与十一份 MODIFIED/ADDED 能力 spec 均已定稿。本设计不重复其论证，只做四件事：把「模型 × 源码 → 一份报告 → 两个消费面」落成可实现的结构（crate 内模块边界、两套解析器与一套归属的接缝、报告字段与 `status` 推导的落点）、把 spec 未给的成形缺口以决策留痕收口（模型读取路径、豁免优先级、时间戳与文件名、审计摘要行与 `status` 的冲突、计数不变量、checks 槽位守卫、本仓模型追加口径）、给出逐函数签名与前端切分，并按 AC-1…AC-21 逐条对齐（§7）。

**交付面二分**：本变更有两块互不重叠的落地面——① **判定域 + 门禁执行体 + 消费面**（`crates/core/archi` 的 `scan/` `drift/`、`crates/infra/archi`、implement 相位步、命令面与画布标记）；② **本仓架构模型数据的追加**（`.dev-team/architecture/{models/03-backend-infra.toml, models/04-backend-shell.toml, decisions/0001-model-granularity.md}`，按 `repo-archi-model` 的漂移纪律「新增 workspace crate 在同一 change 内补齐元素 / 边 / 计数」）。②不引入任何判定语义，只让 ① 新增的 crate 在模型里成立；两者的变更清单与任务分列，MUST NOT 用其一的口径豁免另一处（proposal §11 / R-13：原「本仓模型零改动」仅覆盖 `gate` 档位，不覆盖元素 / 关系 / 计数）。

**起点核实**（工作树实况；Rust 侧路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）：

- **`archi` 现状**：`crates/core/archi/src/lib.rs` 八模块（`model` / `parse` / `graph` / `docs` / `search` / `diff` / `edit` / `dto`）+ 关键类型 `pub use`；`dto::graph_view`（`dto.rs:561`）已是「解析 → 建图 → 诊断合并」的唯一公开投影入口，返回 `ArchiGraph { elements: Vec<ArchiElement>, relations: Vec<ArchiRelation>, diagnostics: Vec<ArchiDiagnostic> }`，**结构错时返回停摆态**（`elements` / `relations` 空 + 诊断全量，`dto.rs:563-569`）；`dto::load` 与 `to_element` / `to_relation` / `to_diagnostic` **全部私有**（`dto.rs:516-556`）。`ArchiElement` 携 `fqn` / `gate` / `path: Vec<String>`（原文不归一），`ArchiRelation` 携 `from` / `to`；`model::Diagnostic` **无 serde / specta 派生**（`model.rs:95-107`），故线面结构错只能是 `dto::ArchiDiagnostic`。
- **`archi` 依赖面**：`Cargo.toml` = `foundation` + `ignore` / `petgraph` / `serde` / `serde_json` / `specta` / `toml` / `toml_edit`（全 `workspace = true`，零新增包）。`globset` / `syn 2.0.119` 在 `Cargo.lock` 内（传递依赖），`oxc_parser` / `oxc_allocator` / `oxc_resolver` **不在锁内**（proposal 症结三的「真新增」判断经核实成立）。
- **`foundation::layout` 现状**：`ReportSlot`（`mod.rs:44-50`）为**封闭二值** `Test` / `Baseline`，`report_dir_name`（`:84-89`）二分臂，`change_reports(root, change, slot)`（`:93`）与 `change_reports_in`（`:98`）为槽位单点；`archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`（`:130-157`）在位，`CONFIG_FILE_NAME`（`:16`）为私有常量、`config_path`（`:79`）为工作区配置唯一出口——`archi_config_path` 尚不存在。
- **步词现状**：`StepNode` / `RunStepKind` / 审计 `StepKind` 三枚举驻 **`crates/core/workflow/src/state.rs`**（`StepNode:256-299`、`RunStepKind:487-560`、`StepKind:31-56`，各 21 / 21 / 8 臂）；**`ChangeStepKind` 不在 workflow**，而在 `crates/core/orchestration/src/state.rs:48-91`（21 臂，`as_str:96-120`）；`run_history::mapped_step`（`run_history.rs:17-41`）为 21 臂穷尽映射；`control::is_session_step`（`control.rs:459-468`）为**正向枚举 `matches!`**（五类 WorkerAgent 步）。
- **相位表现状**：`IMPLEMENT_STEPS`（`write/phase_table.rs:254-289`）为 6 步，`FEEDBACK_LOOP_LIMIT = 5`（`:63`，`pub(crate)`）。
- **编排现状**：`LocalToolSteps`（`steps.rs:22-56`）持三 runner + vcs + store 五字段，`execute`（`:86`）为三分支检查门禁（`StaticCheck` / `TestExecution` / `MutationTest`）——mutation 分支已是「id → 记录 → name 解析 + vcs 取数注入 + runner + 审计行」的范式（`steps.rs:260-297`）；`run_step` 分派（`walker.rs:640-712`）为 `StepNode` 穷尽匹配；`step_static_check`（`:943-1065`）是反馈边执行体的模板（fault 先行 → passed → failed → 超限升格 → `continue_session` 续注）；`step_fail_phase_log`（`:1859`）硬编码「静态检查」文案与 `item`，唯一调用点 `:1024`；`settle_fault`（`:3729`）的 `template: Option<&str>` 为环境文案注入面；`diagnose_brief`（`:3707`）按 200 字符截断节点 detail；`MAX_REPORT_CHARS = 2000` 在 `verdict.rs:6`（`pub`）。
- **checks 现状**：`ProcessStaticCheck::run(root, name, slot, cancel)`（`infra/checks/src/static_check.rs:41`）与 `ProcessTestExecution::run(root, change, slot, cancel)`（`testexec/mod.rs:77`）均把 `slot` 直传 `foundation::layout::change_reports`——`ReportSlot` 长出第三值后两者会**静默接受** `archi` 槽位（spec 明令必须显式 `Err`）。
- **命令面现状**：`commands/archi/mod.rs` 五命令 + 五 `*_inner`（`check_root` 空白 / 非目录统一 `Err`）；`all_commands!`（`commands/mod.rs:27-80`）为唯一登记面；装配点 `commands/change_flow/mod.rs:171-179` 的 `LocalToolSteps::new` 六实参；壳 `Cargo.toml` 已依赖 `archi` 与 `vcs-runtime`。
- **前端现状**：`views/architecture/components/architecture-canvas.tsx` 已被**三处**挂载（`architecture-view.tsx:91` 传 `mode="full"`、`change-detail-view.tsx:265` 与 `run-control-panel.tsx:324` 传 `mode="incremental"`），`CanvasMode = 'full' | 'incremental'`（`element-drawer.tsx:35`）与 `ArchiDriftMode` 词面同形；`marks.ts` 四值边标记 / 两值节点标记已是「未知值退化中性」的纯函数；`use-archi-graph.ts` 为三触发取数范式；`STEP_LABEL`（`run-step-node.tsx:28-50`）为 `Record<ChangeStepKind, string>` **穷尽表**，`GROUP_LABEL`（`:20-25`）四类不变，`run-state.ts` 的 `normalizeKind` / `stepWord` 为 `default` 兜底 switch、`groupOf`（`:161-181`）`default` 即 `toolStep`。
- **本仓模型现状**（`.dev-team/architecture/`，R-13 口径更正的取证面）：`models/` 六文件表头为 `01` 3 元素 / 0 关系、`02` 8 / 9、`03` **7 / 21**、`04` 1 / **12**、`05` 2 / 1、`06` 28 / 64（合计 **49 元素 / 107 关系**，后端 43 = 9 + 21 + 12 + 1）；`grep "ArchiRuntime" .dev-team` 零命中，`03` 段仍只有 7 个 infra crate 元素、`04` 段 12 条壳出边；裁定记录 `decisions/0001-model-granularity.md` 的计数复述分布在 `§1`（`:9`）、`§4`（`:36`）、`§5`（`:40`）、`§6`（`:44`）、`§9`（`:65-67`）。`crates/infra/archi/Cargo.toml` 的 `[dependencies]` 已是 `archi` / `foundation` / `orchestration` 三条，壳 `Cargo.toml:26` 已有 `archi-runtime = { workspace = true }`——即新增元素的四条边在代码侧已可逐条取证，缺的只是模型侧登记。
- **版本面**：`packages/desktop/package.json:3` = `0.6.4`（→ `0.6.5`）；`src-tauri/tauri.conf.json:4` 为 `"../package.json"` 跟随；`src-tauri/Cargo.toml:6` 独立 `0.1.0` 不随动。

**五处事实校准（不改 spec 结论，只定实现形态）**：

1. **`ChangeStepKind` 的落点与 proposal 不符**：proposal 把三枚举都记在 `crates/core/workflow/src/state.rs`，实际 `ChangeStepKind` 驻 `crates/core/orchestration/src/state.rs`。本设计把该文件补进变更清单（新增改动），并在 `run_history::mapped_step` 补臂。
2. **`control.rs` 零代码改动**：`is_session_step` 是**正向枚举** `matches!`（只有五类会话步为真），`ArchiCheck` 天然判假——不需要、也不应该改成否定列表。proposal 该行列出的三文件中，`control.rs` 落地为「复核 + 既有测试断言」，`run_history.rs`（映射臂）与 `lib.rs`（re-export）才有代码改动。
3. **`crates/infra/checks` 需两处槽位守卫**：proposal「变更范围」未列、且「不要修改」称 checks 零改动，但 `desktop-checks-domain` MODIFIED 明令「两 runner MUST NOT 接受 `archi`（传入即 `Err`）」。spec 优先：两 runner 各加一处前置守卫（各 3 行），执行语义零分叉——这不是「改检查逻辑」，而是让槽位封闭集的第三值不被静默吞掉。
4. **三处画布挂载 vs spec 的「两挂载点」**：`change-detail-view.tsx` 与 `run-control-panel.tsx` 是**同一个 change 详情根**下的两个条件挂载（页签面板），观察根同为 `detail.worktree`（归档回落主仓根）。因标记接线落在 `architecture-canvas.tsx` 内部（三处共用同一组件），故「两挂载点同源取数」由单点接线天然成立，不需要第三份接线，也不需要新增 `root` 透传。
5. **「本仓模型零改动」口径作废（R-13）**：本变更新建 workspace crate，`repo-archi-model` 的漂移纪律（`specs/repo-archi-model/spec.md:86`）与双向覆盖判据（`:84`）同时生效，模型三件套（`models/03-backend-infra.toml` / `models/04-backend-shell.toml` / `decisions/0001-model-granularity.md`）因此是**实现文件**而非零改动面（proposal §11 与「变更范围 - 实现文件」表 `:264-266`）。原表述只剩一处有效语义：`gate` 档位全 `off` 不动（收紧按模块独立变更）。本设计据此把三件套并入变更清单（§变更清单）、数据模型（§持久化三）与任务（tasks.md 阶段十四）。 另：`test-design.md` 的零改动面表（`:1176`）仍按旧口径把 `.dev-team/architecture/models/*.toml` 与 `decisions/*` 列为「不要修改」，属同一处陈旧口径，归测试设计相位同步删除——本设计与 tasks 不再复述该口径。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 报告结构的落点与线面命名 | 全部报告类型驻 `crates/core/archi/src/drift/report.rs`，**线面名一律 `Archi*` 前缀**（`ArchiDriftReport` / `ArchiDriftEntry` / `ArchiDriftOccurrence` / `ArchiDriftSuppressedEntry` / `ArchiDriftObservation` / `ArchiDriftModel` / `ArchiGateCount` / `ArchiScanReport` / `ArchiExcludedFile` / `ArchiMatchedElement` / `ArchiUnresolvedFile` / `ArchiDriftMode` / `ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiSuppressVia`），并 `pub use ArchiDriftReport as Report`（spec Module Contract 的短名） | specta 的类型注册表按**裸类型名**唯一，`dto.rs:80-85` 已为该约束立先例（`ArchiDiagnosticKind` + `as` 别名）；报告是三组封闭词汇（`status` 五值 / 漂移 `kind` 四值 / 观察项 `kind` 两值）的唯一持有者，各带 `ALL` 常量供语料对拍 |
| D2 | 模型读取路径 | `drift::check` 经 **`crate::dto::graph_view(root)`** 取模型（元素 / 关系 / 结构诊断），`drift` 内**不新开 load 管线**、不二次 `ModelGraph::build` | `dto::load` 与 `to_diagnostic` 私有且 `dto.rs` 在本变更零改动面；`graph_view` 已给出判定所需的全部事实（`gate` / `path` / 关系对 / 结构诊断的线面形状），复用它使「图单点构建」纪律零缺口，也不产生第二份 `Diagnostic → ArchiDiagnostic` 映射 |
| D3 | 停摆态与 `skipped` 的可辨性 | `graph_view` 结构错时返空图 + 诊断 ⇒ `check` 在调用它**之前**判「模型是否存在」：`archi_models_dir(root)` 缺席或内含零个 `*.toml` → `status = skipped`（不调 `graph_view`）；模型在但结构错 → 照常走判定，`status = errors` | 空模型（冷启动合法）与无模型（没检查）在 `graph_view` 的输出上不可辨，而二者在门禁语义上完全相反（前者 `clean`、后者 `skipped`）。判据放在读模型之前是唯一不需要新增 DTO 字段的做法 |
| D4 | 扫描 / 判定的职责分界 | `scan/` 只出**事实**（文件归属、按目标文件聚合的跨元素依赖、排除 / 未解析留痕、内联豁免标记）；`drift/` 只出**结论**（对账、定级、豁免裁决、`status` 推导、报告装配） | 「判定单源」（AC-10）要求归属 / 定级 / status 三件事只有一处；把「读盘提取」与「判违规」分开后，命令面与门禁面都只剩「取输入 → 调 `check` → 投影 / 落盘」 |
| D5 | 内联豁免的判据位置 | 内联标记（`// archi-ignore-next-line: <reason>`）由 **`scan` 侧**判定（原文在手），产出附在依赖事实上的 `inline_ignore: Option<String>`；配置豁免与最终定级在 `drift` | spec 要求「用 import 声明的 span 行号回原文取该行或其上一行匹配」——只有抽取现场持有源文本；若挪到 `drift`，跨元素依赖事实就要连带携带整份源文件内容（1000 文件量级的无谓驻留） |
| D6 | 归属三值不做「off 特判」 | `owner_of` 恒返 `Owner::{Element{fqn, gate}, Ambiguous(Vec<String>), None}`（**含 off 元素**）；off 的语义由调用侧决定：**源文件**归 `off` 元素 → 计 `unmatchedFiles`、零违例，但**依赖照常抽取**（抽取是事实面、定级才是结论面）；**目标文件**归 `off` 元素 → 仍可作 `to`（`off` 只表示「本模块不参与定级」，不表示「元素不存在」） | 若把 off 折进 `None`，则「`strict` 元素 import `off` 元素」的边会因目标不可归属而丢失——那正是 ② 该报的违例（定级只看引入方）。同理 `Ambiguous` 不能任选，故单列第三值。**「off → 零依赖抽取」不是本决策**：`off` 只抑制定级，不做「看不见」处理——否则 gate 全 `off` 的工程（本仓即此形态）会把全部声明边误判成「声明未兑现」（与 `desktop-archi-drift` ① 的定义相反），且 ② 的事实面整体消失 |
| D7 | 目标不可归属的处置 | 目标文件归 `Ambiguous` / `None` → **丢弃该边**，文件已计入 `unmatchedFiles`（`Ambiguous` 另记 `ambiguousOwner` warning） | spec 症结三：「解析不到 / 归属不确定，一律不许冒充违例，只能留痕」——看不见的依赖不该变成红点 |
| D8 | 排除规则的形态 | `DEFAULT_EXCLUDE_RULES: &[&str]` 存**具体 glob**（花括号简写展开为 12 条），doc 注释逐条标注对应的七条约定；`excluded[].rule` 记**命中的具体 glob** | globset 无花括号交替方言（`{a,b}` 不支持，与 `checks::globmatch` 的花括号展开是两套引擎，`archi` 也不允许依赖 `checks`）；spec 场景逐字要求 `rule: "**/*.test.ts"`，记具体形态才逐字成立 |
| D9 | 时间戳、文件名与 Windows 合法性 | `timestamp` = RFC3339 UTC（`2026-10-20T08:31:12Z`，`time::OffsetDateTime::now_utc()`）；报告文件名 = `<YYYYMMDDTHHMMSSZ>.json`（从 `timestamp` 过滤出数字 + `T` + `Z`，**无冒号**） | 冒号在 Windows 文件名上非法，而报告要落 worktree 的 `reports/archi/`（跨平台）；时间取 `time`（锁内既有版本），不用 `SystemTime` 手算日历（易错） |
| D10 | `status` 推导序与 `passed` 的关系 | 推导序固定为：`skipped`（无模型）→ `errors`（`model.structuralErrors` 非空）→ `errors`（`errors[]` 非空）→ `noChanges`（可判文件为 0）→ `warnings`（`warnings[]` 非空）→ `clean`。门禁侧 `passed = errors.is_empty() && structuralErrors.is_empty()`。**取值口径**：`status` 线格式统一走 `#[serde(rename_all = "camelCase")]`——五值落线为 `clean` / `errors` / `warnings` / `noChanges` / `skipped`（单值词 camelCase 与小写同形，`noChanges` 必须为驼峰；`lowercase` 会产出 `nochanges`，与 spec 词汇不自洽）。封闭词汇的**规范面**是 `as_str()`（逐值与 `ALL` 对拍），serde 只做线名映射 | 结构错必须在 `noChanges` **之前**：坏模型 + 零可判文件若落 `noChanges`，会得到「步 failed 且 detail 写未检查」的自相矛盾。按本序，`status == errors ⟺ passed == false` 恒成立，与 `desktop-archi-gate` 的三态分派逐条对齐。取值口径写死在决策里，避免「决策写 `lowercase`、实现走 camelCase」的二义 |
| D11 | 结构错不进 `errors[]` | 结构错只在 `model.structuralErrors[]`（元素/关系层面的诊断不进条目的 `occurrences`）；`errors[]` 只装漂移条目 | 同一事实不在两处各存一份（否则反馈文本会重复、`occurrences` 语义被污染）；`status = errors` 覆盖两者，消费方零歧义 |
| D12 | `model.warnings[]` 的形状与覆盖 | 用漂移条目形状 `Vec<ArchiDriftEntry>`（`kind ∈ {pathNotFound, noPath}`）；**不产出 `docNotFound`** | spec 字段面注释列出 `docNotFound`，但文档关联是 `desktop-archi-model` 的文档面（`docs::all_docs` 要全量 walk + 全量读 ADR），把它们塞进门禁步会为一项不影响 `passed` 的提示付整份 md 读盘；本报告只承载判定域自己的模型警告。若评审要求覆盖，属纯增量（见待决问题 2） |
| D13 | ① 声明未兑现的产出规则 | 两模式**都**产出，且**永远只进 `observations[]`**；`message` 按模式显式声明扫描面——全量模式写「已覆盖观察根下的全部源文件，仍未见 `A → B` 的 import」、增量模式写「只覆盖变更集内的源文件，本次扫描面内未见 …」，使增量模式的陈述不被读成全仓结论 | spec「增量产出 ②④；① 无基准 → 只作观察项」+ R-6 明写接受「增量报未兑现而不进 `status`」；把口径写进 `message` 是让噪声可解释的最小代价。全量模式的文案 MUST NOT 用「只覆盖…」的退让句式——全量确实覆盖全部源文件，这才是 ① 的成立条件 |
| D14 | 门禁结论与文本装配的归属 | `passed` / 摘要 / 违例明细 / 固定处置提示**全部由 `infra/archi` runner 装配**，`walker` 只做 `diagnose_brief(&outcome.diagnostics)` 与注入；文本上限复用 `orchestration::verdict::MAX_REPORT_CHARS`（2000），**零新常量** | `desktop-archi-gate` 明令「反馈诊断文本由本 runner 侧装配且有界」；walker 侧不引入任何 `archi` 词汇（`workflow` ⇄ `archi` 零依赖的同一纪律延伸到编排层词汇：walker 只认 `passed` / `diagnostics` / `fault`） |
| D15 | 报告路径与节点 `detail` / `output_ref` | 有报告的文件路径时（`status ∈ {clean, warnings, errors}`）detail = 「摘要；报告 <ref>」；`skipped` / `noChanges` 的 detail **逐字**为「未检查（无模型）」/「未检查（无可判文件）」（**不附路径**）；`report_ref` **MUST NOT** 写入节点 `output_ref` | AC-13 要求逐字（附路径就破）；「报告路径随步 detail 呈现」对报告在位的三态成立；`output_ref` 是 exec-outputs 回查通道（`read_exec_output` 只认 `reports/test\|baseline` 的 exec_id），架构报告是 JSON 文件、走报告路径 |
| D16 | 审计摘要行不含 `status=` | `steps.rs` 的审计摘要为 `archi_check passed=<bool>`，`reference = report_dir`（`report_ref` 不进审计行） | spec 的摘要样例写了 `status=<status>`，但 `ArchiCheckOutcome` 的字段面由同一 spec 钉死为五字段（无 `status`）。二者冲突时字段面为准（加字段会同时破坏两份 spec 的 outcome 契约）；状态词在节点 detail 与报告文件里可查，`status` 不因此丢失 |
| D17 | 升格落账单点复用 | 把 `step_fail_phase_log` 改名为 `step_gate_fail_phase_log`，新增 `GateFailLog { item, reason, diagnostics }` 参数；static-check 与 archi 共用一份 | 两者行为逐字相同（代写 fail checklist + `skipped=false` + 仅 executor 槽位），差别只有 checklist 条目标题与 report 首句。复制第二份（约 40 行）是本仓明令禁止的形态 |
| D18 | `settle_fault` 的 archi 调用形态 | `template = None`（消息即 `fault` 原文），`report_dir = outcome.report_dir`，`output_ref = None` | spec 明令「settle_fault 分支仅作为同形保留（不产生任何新增环境文案常量）」；零进程 ⇒ 生产路径 `fault` 恒 `None`，`Environment` 臂只防御性存在 |
| D19 | checks 两 runner 的 `archi` 槽位守卫 | `static_check::execute` 与 `testexec::execute` 各加一处前置守卫：`slot == ReportSlot::Archi` → `Err("archi 槽位归架构门禁 runner…")`；其余零改动 | spec「archi 槽位归架构门禁」场景要求显式 `Err`；不加守卫则第三值被静默接受（报告目录会算对，但报告体是漂移 JSON 而非 checks schema——正是 spec 要防的混居） |
| D20 | 前端标记映射的优先级 | `driftMarks` 先按 `suppressed[]` 打 `exempt`（带 `reason`），再按 `observations[]`（`unfulfilledDeclaration`）打 `unfulfilled`（边 + `from` 节点）——同一对两段都命中时取 `unfulfilled` | 两段在语义上互斥（`suppressed` = 代码有该依赖但被豁免；`unfulfilledDeclaration` = 声明存在而代码没有），冲突只可能来自拼接了不同时点的报告；取「未兑现」是更值得提示的偏差，且不静默丢信息 |
| D21 | 漂移取数的三触发与降级 | `useArchiDrift(root, mode)` 抄 `use-archi-graph` 的三触发（挂载 / `root` 变化 / 手动 `refresh`），零 watch 零轮询；取数失败 → `report = null`，画布**照常渲染零标记图** + 行内提示（`data-testid="archi-drift-error"`） | `desktop-file-watch` 的显式取数纪律零变化；漂移面是叠加信息，取数失败 MUST NOT 影响图本身（`architecture-canvas` 既有失败面已由 `archi_graph` 的错误接管） |
| D22 | 扫描计数的不变量 | `scannedFiles = 可判源文件数 − excluded.len()`；增量模式 `changedFiles` = 变更集内**源文件**数（扩展名命中四种 + `.rs`），全量模式恒 `0`（无变更集概念） | spec 的示例数字（「12 个源文件，其中 5 个被排除 → `scannedFiles = 5`」）与减法不自洽；以不变量为准（`unresolved` / `excluded` / `scanned` 三数可机械核对），语料断言按不变量写 |
| D23 | Rust 侧长尾一律 `unresolved` | 建树期与解析期共五类不可判定全部落 `unresolved[]`（宏生成 `mod` / `include!` / 孤儿 `.rs` / `mod` 声明定位不到文件 / `use` 路径无法落到文件）；外部 crate（`std` / 第三方）**静默丢弃**（不记 `unresolved`，不算违例）。**crate 表按源码标识符形态登记**（清单名 `archi-runtime` → 键 `archi_runtime`）——`use` 路径里写的是标识符，用连字符名查表必然全落空；`#[path]` 取值先归一（`\` → `/`、折叠 `.` / `..`）并限根（绝对路径 / 上溯出观察根不采信）；member 清单读 / 解析失败、缺 `[package] name`、无入口一律 `Err`（member 目录无 `Cargo.toml` 属 glob 过近似，跳过） | R-2：漏判的代价是「少报」（假绿）而非「误报」，而猜的代价是错误归因；外部 crate 无元素覆盖，记进去只是噪声（spec 场景明令「不进 `errors` / `warnings` / `unresolved`」） |
| D24 | `archi` 的外部直接依赖清单 | 新增 oxc 家族 **6 件**（`oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` 同版 `=0.146.0` + `oxc_resolver` `=11.24.3`，TS/JS 词法、AST 与模块解析）与 `proc-macro2`（`features = ["span-locations"]`，`syn` 的 span 行号）、`syn 2`（`features = ["full"]`，Rust 项解析）、`globset 0.4`（排除匹配，提为直接）、`sha2 0.10`（`modelHash`）、`time 0.3`（`timestamp`）；`foundation` 仍是唯一 workspace 内依赖 | proposal 只列了前四件 + `globset`；实测落地为 oxc **6 件**——`oxc_ast` / `oxc_span` / `oxc_syntax` 是 `oxc_parser` 的同版伴随件（`ParserReturn.program` / `SourceType` 需直接引用其类型，声明为直接依赖零新增锁包），`oxc_resolver` 是独立 11.x 版本线；`proc-macro2` 同理是 `syn` 行号的伴随件（缺 `span-locations` 则 `use` 项行号恒 0）。`sha2` / `time` 是报告两个字段的必然依赖（`foundation` 的 sha2 未导出通用摘要函数，`time` 是 `checks-runtime` 的既有时钟来源），三者均锁内既有、零新增包、零进程 / 零 Tauri 触点 |
| D25 | 本仓架构模型数据的追加口径 | **只追加、不收紧**：`03-backend-infra.toml` 新增元素 `App.Backend.Infra.ArchiRuntime`（`kind = "module"` / `gate = "off"` / `path = ["./packages/desktop/src-tauri/crates/infra/archi/"]` / 一句中文 `summary`，按字母序插在 `AgentRuntime` 之后）与其 3 条出边（→ `Core.Archi` / `Core.Foundation` / `Core.Orchestration`）；`04-backend-shell.toml` 新增 1 条壳出边（→ `Infra.ArchiRuntime`）；计数三处同值（模型表头 / `repo-archi-model` 计数表 / ADR `§1`/`§4`/`§5`/`§6`/`§9`）= 50 元素 / 111 关系（后端 47、`03` 段 8 / 24、`04` 段 13）；`crates/tools/specta-zod-patched` 单列为**既有已知缺口**、MUST NOT 顺手补齐 | 四条边的代码侧证据已存在（`archi-runtime` 的 `[dependencies]` 三条 + 壳 `Cargo.toml` 的 `archi-runtime`），模型侧缺口不是取舍而是漏登记；`gate` 全 `off` 不变意味着本变更不给自家仓库设任何门禁（R-4），故追加只让图重新自洽，不改变「恒绿」事实。计数单点复算（5 组数字同源）是 R-14 的唯一缓解手段；已知缺口单列是 R-15 的要求 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 扫描编排 | 源文件枚举（gitignore 感知）→ 排除 → 归属 → 逐文件依赖抽取（两套解析器）→ 跨元素边聚合 → `ArchiScanReport` 成型；取消标志在被枚举与解析循环内轮询 | `crates/core/archi/src/scan/mod.rs`（新） | `ignore::WalkBuilder`、`crate::scan::{exclude, owner, ts, rust}`、`crate::dto::ArchiElement`、`crate::drift::report`、`foundation::cancel` | `ignore 0.4` |
| 排除矩阵 | 默认七条约定的具体 glob 常量 + 架构域配置 `exclude` 追加（globset 编译一次复用）；`excluded[{ file, rule }]` 逐条留痕；与工作区 `tests[].excludes` 零耦合 | `crates/core/archi/src/scan/exclude.rs`（新） | `globset` | `globset 0.4`（`GlobSetBuilder` + 下标→规则表，最小下标优先） |
| 归属单点 | 元素 `path` 条目 → 归一（`\` → `/`、剥 `./`、去尾分隔符）最长前缀匹配；唯一最长 → `Element`，同长多命中 → `Ambiguous`（不任选），无命中 → `None` | `crates/core/archi/src/scan/owner.rs`（新） | `crate::dto::{ArchiElement, GateTier}` | 纯函数（元素数百级，逐文件线性扫描） |
| TS / JS 提取 | 静态 `import` / `import type` / `export … from` 的 specifier + 行号 + `typeOnly`；动态 `import()` / `require()` / 语法错 → `unresolved`；specifier → 磁盘路径（自被解析文件向上取最近 tsconfig / jsconfig 的 `baseUrl` + `paths`，再 node 规则） | `crates/core/archi/src/scan/ts.rs`（新） | `oxc_parser` / `oxc_allocator`（词法 + AST）、`oxc_resolver`（模块解析 + tsconfig） | oxc（版本见 §6 与待决问题 1） |
| Rust 提取 | workspace 清单 + member glob → crate 清单（零子进程）→ 逐 crate 模块树（`mod x;` 外部 / `mod x { … }` 内联 / `#[path]` 优先 / `cfg` 两臂同路径）→ `use` 树（`crate::` / `self::` / `super::` / workspace crate 名，crate 表按标识符形态 `-` → `_` 登记）模块路径最长前缀匹配落到文件 | `crates/core/archi/src/scan/rust.rs`（新） | `syn 2`（`full`）、`toml`（既有）、`globset`（member glob）、`std::fs` | 零子进程、零宏展开 |
| 判定单点 | `check(CheckInput) -> Result<Report, CheckError>`：模型读取（`dto::graph_view`）→ 看护配置 → 扫描 → ① / ② / ④ 对账 → 定级（只看引入方）→ 豁免裁决（声明 > 配置 > 内联）→ `occurrences` 聚合 → `status` 推导 → 报告装配 | `crates/core/archi/src/drift/mod.rs`（新） | `crate::dto::{graph_view, ArchiElement, ArchiDiagnostic, GateTier}`、`crate::scan`、`crate::drift::{config, report}`、`foundation::{cancel, layout}`、`sha2`、`time` | 纯读盘、零写盘、零 spawn |
| 报告结构 | 三组封闭词汇（`ArchiCheckStatus` 五值 / `ArchiDriftKind` 四值 / `ArchiObservationKind` 两值，各带 `ALL`）+ 全部 `Archi*` wire 类型（serde camelCase + `specta::Type`）+ `pub use ArchiDriftReport as Report` | `crates/core/archi/src/drift/report.rs`（新） | `serde` / `specta`、`crate::dto::{ArchiDiagnostic, GateTier}` | 纯数据（零 IO、零判定） |
| 看护配置 | `<archi_dir>/config.json` 读取（经 `foundation::layout::archi_config_path`）：缺失即空配置、`deny_unknown_fields`、`ignore[].reason` 必填非空白、`from` / `to` 必须是模型内 FQN、`exclude[]` 必须是含通配符的文件级 glob | `crates/core/archi/src/drift/config.rs`（新） | `foundation::layout::archi_config_path`、`serde_json`、`globset`（glob 合法性） | 单文件 JSON（零第三个配置文件） |
| 路径派生 | `ReportSlot` 扩为封闭三值（`Test` / `Baseline` / `Archi`）+ `report_dir_name` 三分臂 + `ARCHI_REPORTS_DIR_NAME` + `archi_config_path(root)` | `crates/core/foundation/src/layout/mod.rs`（修改） | `std::path` | 既有布局模块（纯推导零 IO） |
| 词面单点 | `StepNode` / `RunStepKind` / 审计 `StepKind` 尾部追加 `ArchiCheck` + 三个 `as_str` / `group` / `role` 臂；`ChangeStepKind` 尾部追加（orchestration 侧） | `crates/core/workflow/src/state.rs`、`crates/core/orchestration/src/state.rs`（修改） | 无 | 既有枚举（bincode 下标纪律：只尾部追加） |
| 相位表 | `IMPLEMENT_STEPS` 7 步（`ArchiCheck{3}` 插在 StaticCheck 与 Evaluator 之间）+ `ARCHI_FEEDBACK_LIMIT = 3` | `crates/core/workflow/src/write/phase_table.rs`（修改） | 无 | 静态 const 表 |
| port 与产出 | `ArchiCheckOutcome` 五字段、`ToolCommand::ArchiCheck { change_id }`、`ToolStepOutput::ArchiCheck`、`ArchiCheckRunner` 四参 port | `crates/core/orchestration/src/port.rs`（修改） | `foundation::cancel` | port 落消费者（与三 runner 同型） |
| 分派与审计 | `LocalToolSteps` 第四 runner（字段 + 构造入参 + `execute` 形参）+ `ArchiCheck` 分支（name 解析 → `change_edit_set` → runner → 审计行） | `crates/core/orchestration/src/steps.rs`（修改） | `ArchiveVcsPort`、`ArchiCheckRunner`、`workflow::state::StepKind` | 与 mutation 分支同式 |
| 反馈边执行体 | `step_archi_check`：fault 先行 / `FeedbackLoop{3}` 计数 / 超限代写 fail 落账 + `StepFlow::Upgraded` / 未超限 `continue_session` 注入同一 executor 会话；`step_gate_fail_phase_log` 参数化复用 | `crates/core/orchestration/src/walker.rs`（修改） | port 产出、`run_tool` / `settle_fault` / `diagnose_brief` | 与 `step_static_check` 同型 |
| 门禁执行体 | `LocalArchiCheck`：调 `archi::drift::check`（增量、`change = Some(name)`）→ 落盘 `reports/archi/<timestamp>.json` → 派生 `passed` / 有界 `diagnostics` / `report_dir` / `report_ref`；`fault` 除取消外恒 `None`；写盘失败降级 `None` | `crates/infra/archi/src/runner.rs`（新） | `archi`、`orchestration::port`、`foundation::{cancel, layout}` | 零 spawn / 零 vcs / 零 store / 零 config |
| 报告落盘单点 | 目录派生（`foundation::layout::change_reports(root, name, ReportSlot::Archi)`）+ 目录创建 + 文件名（`<YYYYMMDDTHHMMSSZ>.json`）+ 序列化写盘 | `crates/infra/archi/src/report.rs`（新） | `foundation::layout`、`serde_json` | 单点（runner 内零路径字面量） |
| 命令面 | `archi_drift(root, mode)` + `archi_drift_inner(root, mode, vcs)`：root 检查 → 增量经 vcs 缝取变更集 → 调 `check` → 错误串映射；命令层零判定 | `src-tauri/src/commands/archi/mod.rs`（修改） | `archi::drift`、`vcs-runtime::ProcessArchiveVcs`、`tauri` / `specta` | 沿既有五命令的三件事纪律 |
| 装配 | `LocalToolSteps::new` 增第四 runner `Arc::new(LocalArchiCheck::new())`；`all_commands!` 增一行 | `src-tauri/src/commands/change_flow/mod.rs`、`commands/mod.rs`（修改） | `archi-runtime` | 组合根（零判定） |
| checks 槽位守卫 | 两 runner 前置 `archi` 槽位显式 `Err` | `crates/infra/checks/src/static_check.rs`、`crates/infra/checks/src/testexec/mod.rs`（修改） | `foundation::layout::ReportSlot` | 三行守卫（执行语义零分叉） |
| 标记映射 | `driftMarks(report, graph)`：只读 `observations` / `suppressed` 两段，按 `(from, to)` 命中边、按 `from` 命中节点；零判定零 IO | `packages/desktop/src/views/architecture/lib/drift-marks.ts`（新） | 生成绑定类型 | 纯函数 |
| 漂移取数 | `useArchiDrift(root, mode)`：三触发、`mode` 透传 `archi_drift`、失败降级 `error` | `packages/desktop/src/views/architecture/hooks/use-archi-drift.ts`（新） | `commands.archiDrift`（生成绑定） | React 19 hook（抄 `use-archi-graph`） |
| 画布接线 | `graph = driftMarks(report, graph)` 后交渲染；漂移取数失败降级零标记 + 行内提示；三处挂载共用 | `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`（修改） | `useArchiDrift`、`driftMarks`、`marks.ts`（零改动） | 既有画布 |
| 步词呈现 | `normalizeKind` / `stepWord` 补 `archi_check` ↔ `archiCheck`；`STEP_LABEL` 补 `archiCheck: '架构检查'` | `packages/desktop/src/views/changes/flow/run-state.ts`、`run-step-node.tsx`（修改） | 生成绑定 `ChangeStepKind` | 既有穷尽表 |
| 本仓模型数据（数据资产，非运行组件） | 登记新增 crate 的元素与四条边、同步三处计数复述（模型表头 / spec 计数表 / ADR 复算表）；零运行期角色、零新字段、零 `gate` 收紧 | `.dev-team/architecture/models/03-backend-infra.toml`、`.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`（修改） | 无（纯数据 / 文档；消费方是 `crates/core/archi` 的既有读面） | TOML 模型文件 + Markdown 裁定记录 |

依赖方向：新增**四条**边且与模型追加逐条对应——`App.Backend.Infra.ArchiRuntime → {App.Backend.Core.Archi, App.Backend.Core.Foundation, App.Backend.Core.Orchestration}`（`archi-runtime` 的 `[dependencies]` 声明，登记在 `03` 段）与 `App.Backend.Shell → App.Backend.Infra.ArchiRuntime`（壳的 `[dependencies]`，装配注入 `LocalArchiCheck`，登记在 `04` 段）；`archi` 的 workspace 内依赖仍仅 `foundation`；`workflow` ⇄ `archi` 双向零依赖（`workflow` 侧只经 `StepNode` 声明 `ArchiCheck` 词，不知道 `archi` 存在）。四条边全为 infra → core / 同层，不触 `dependsOnAncestor`；`archi-runtime` 的 `tempfile` 在 `[dev-dependencies]`，按段落整体排除、不入模。

---

## 变更清单 (Change Inventory)

> 路径口径：**新增文件**用仓库根相对全路径；**修改文件** Rust 侧省略 `packages/desktop/src-tauri/` 前缀（与 proposal 同口径），前端侧写全 `packages/desktop/src/…`。
> 测试文件（`*_test.rs` / `tests/**` / `*.test.ts(x)`）不在本清单射程——测试由独立相位承接；唯一例外是 AC-18 要求的成本实测表文档 `crates/core/archi/tests/README.md`（测量记录，非测试代码）。
> **模型三件套在射程内**：`.dev-team/architecture/models/03-backend-infra.toml`、`.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md` 是本变更的**实现文件**（proposal §11 与「变更范围 - 实现文件」表末三行，R-13），不属零改动面；零改动面只剩 `models/{01-app,02-backend-core,05-backend-tools,06-frontend}.toml`、`models/**` 的 `gate` 档位（全 `off`）、`.dev-team/architecture/config.json`（不创建）。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs` | 扫描编排：枚举 → 排除 → 归属 → 两套解析器抽取 → 跨元素边聚合 → `ArchiScanReport`；`ScanInput` / `ScanOutcome` / `ImportFact` / `scan()` / `inline_ignore()` | 公共函数表 `scan` 行；类型表 `ScanInput` / `ScanOutcome` / `ImportFact`；D4 / D5 / D22 |
| `…/crates/core/archi/src/scan/exclude.rs` | `DEFAULT_EXCLUDE_RULES` 常量 + `ExcludeSet`（默认 + 配置追加，globset 单编译）；`excluded[{file, rule}]` | 公共函数表 `ExcludeSet::build` / `ExcludeSet::matched`；D8 |
| `…/crates/core/archi/src/scan/owner.rs` | `Owner` 三值 + `OwnerIndex`（元素 `path` 归一最长前缀匹配） | 公共函数表 `OwnerIndex::build` / `owner_of`；D6 / D7 |
| `…/crates/core/archi/src/scan/ts.rs` | `TsResolver`：oxc 静态 import / `export … from` 抽取 + 就近 tsconfig / jsconfig 解析 + specifier 落盘 | 公共函数表 `TsResolver::new` / `extract` / `resolve`；D23（动态导入） |
| `…/crates/core/archi/src/scan/rust.rs` | `RustResolver`：workspace 清单 + member glob → crate 清单 → 模块树（四种形态）→ `use` 树最长前缀匹配 | 公共函数表 `RustResolver::new` / `unresolved` / `extract` / `resolve`；D23 |
| `…/crates/core/archi/src/drift/mod.rs` | 判定单点 `check(CheckInput)`：模型读取 → 配置 → 扫描 → 对账 / 定级 / 豁免 / 聚合 → `status` → 报告 | 公共函数表 `check` / `CheckError::message`；D2 / D3 / D10 / D11 / D13 / D22 |
| `…/crates/core/archi/src/drift/report.rs` | 三组封闭词汇（各带 `ALL`）+ 全部 `Archi*` wire 类型 + `pub use ArchiDriftReport as Report` | 类型表全线；D1 |
| `…/crates/core/archi/src/drift/config.rs` | `<archi_dir>/config.json` 读取与校验（缺失即空、`deny_unknown_fields`、`reason` 必填、FQN 存在、glob 合法性） | 公共函数表 `load_config`；配置表两键 |
| `packages/desktop/src-tauri/crates/infra/archi/Cargo.toml` | crate 声明 `archi-runtime`：`archi` + `orchestration` + `foundation` + `serde_json`；`[dev-dependencies] tempfile` | 依赖表；`desktop-crate-layout`「infra/archi crate 落地登记」 |
| `…/crates/infra/archi/src/lib.rs` | 出口：`pub use runner::LocalArchiCheck;` + `mod report;` | 公共函数表 `LocalArchiCheck::new` |
| `…/crates/infra/archi/src/runner.rs` | `impl ArchiCheckRunner for LocalArchiCheck`：调判定 → 落盘 → 派生 outcome；`compose_text`（有界诊断 / 摘要 / 固定处置提示） | 公共函数表 `LocalArchiCheck::new` / `ArchiCheckRunner::run`；D14 / D15 / D18 |
| `…/crates/infra/archi/src/report.rs` | 报告目录派生 + 目录创建 + 文件名 + 写盘单点（`pub(crate) fn write_report`） | 公共函数表 `write_report` 行（本 crate 唯一写盘面）；D9 |
| `packages/desktop/src/views/architecture/lib/drift-marks.ts`（新） | 报告 → 标记纯函数（只读 `observations` / `suppressed`） | 公共函数表 `driftMarks`；D20 |
| `packages/desktop/src/views/architecture/hooks/use-archi-drift.ts`（新） | 漂移取数 hook（三触发、`mode` 透传、失败降级） | 公共函数表 `useArchiDrift`；D21 |
| `packages/desktop/src-tauri/crates/core/archi/tests/README.md`（新） | 成本实测表（五项依赖成本：NSIS / 主二进制 / 锁包数 / CI 冷构建 / 本地增量 + 一项运行期预算 p95）+ 版本钉定表 + 回退序留痕 + 未测移交标注 | §6；AC-18 |
| `openspec/changes/archi-check-and-gate/reports/archi/model-sync-check.md`（新，change 目录内工件） | 架构模型同步的核对留痕（双向覆盖差集 / 四段计数 / `archi_graph` 正常态取证）；非漂移报告、无消费面（见 §持久化三末段） | D25；AC-21 |

### 修改文件

| 文件路径 | 具体修改内容 | 说明 |
|----------|--------------|------|
| `crates/core/archi/Cargo.toml` | `[dependencies]` 增 `globset` / `oxc_allocator` / `oxc_ast` / `oxc_parser` / `oxc_resolver` / `oxc_span` / `oxc_syntax` / `proc-macro2` / `sha2` / `syn`（`features = ["full"]`）/ `time`；注释补「零 Tauri / 零 store / 零 spawn / 零 workflow / 零 config」与「oxc 家族 6 件 + `proc-macro2` 为本变更唯一真新增直接依赖面（`ast` / `span` / `syntax` / `proc-macro2` 为伴随件，零新增锁包）」 | D24；`foundation` 仍是唯一 workspace 内依赖 |
| `crates/core/archi/src/lib.rs` | `pub mod drift; pub mod scan;`（字母序插入）+ `pub use drift::report::{ArchiCheckStatus, ArchiDriftKind, ArchiDriftMode, ArchiDriftReport, ArchiObservationKind};` + `pub use drift::CheckError;` + `pub use drift::{check, CheckInput};` + `pub use drift::config::{CheckConfig, IgnoreRule};` + `pub use scan::{scan, ScanInput, ScanOutcome};` | 装配与出口；模块 doc 补第四条纪律（判定单源）。出口**分三批**落地（报告结构 + `CheckError` → 扫描 → 判定与配置），每批只引用当批已定义的符号——`CheckError` 是 `scan` 返回类型的必需件，故随报告结构同批出线 |
| `crates/core/foundation/src/layout/mod.rs` | ① 常量组增 `ARCHI_REPORTS_DIR_NAME = "archi"`；② `ReportSlot` 增 `Archi` 变体（doc 补槽位语义）；③ `report_dir_name` 三分臂；④ 新增 `pub fn archi_config_path(root: &Path) -> PathBuf`（`archi_dir(root).join(CONFIG_FILE_NAME)`） | `desktop-checks-domain` / `workspace-layout-resolution`；`CONFIG_FILE_NAME` 复用，零第二个文件名字面量 |
| `crates/core/workflow/src/state.rs` | `StepNode` / `RunStepKind` / `StepKind` 三枚举**尾部追加** `ArchiCheck` + 各自 `as_str` 臂；`StepNode::group()` 归 `Exec`、`role()` 归 `None` | AC-16；bincode 下标纪律（只尾部追加） |
| `crates/core/workflow/src/write/phase_table.rs` | `ARCHI_FEEDBACK_LIMIT: u32 = 3`（`pub(crate)`）；`IMPLEMENT_STEPS` 插 `ArchiCheck`（`on_fail = FeedbackLoop { limit: ARCHI_FEEDBACK_LIMIT }`）于 StaticCheck 与 Evaluator 之间（6 → 7 步） | AC-11；`TEST_ONLY_TABLE` 零 `ArchiCheck`（表测试守卫） |
| `crates/core/orchestration/src/state.rs` | `ChangeStepKind` **尾部追加** `ArchiCheck` + `as_str` 臂 | 事实校准 1（proposal 记在 workflow，实际在此） |
| `crates/core/orchestration/src/port.rs` | `ArchiCheckOutcome`（五字段）+ `ArchiCheckRunner` port（`run(root, name, changed, cancel)`）+ `ToolCommand::ArchiCheck { change_id }`（尾部）+ `ToolStepOutput::ArchiCheck(ArchiCheckOutcome)`（尾部） | `desktop-change-orchestration` Module Contract |
| `crates/core/orchestration/src/steps.rs` | `LocalToolSteps` 增 `archi: Arc<dyn ArchiCheckRunner>` 字段与 `new` 入参；`execute` 增形参；`ToolCommand::ArchiCheck` 分支（name 解析 → `change_edit_set(root, "HEAD")` → runner → `StepKind::ArchiCheck` 审计行） | AC-15 / D16；取数失败 `?` 直达 `Terminal` |
| `crates/core/orchestration/src/walker.rs` | ① `run_step` 增 `StepNode::ArchiCheck` 臂；② 新增 `step_archi_check`（fault 先行 / 计数 / 超限升格 / 续注）；③ `step_fail_phase_log` → `step_gate_fail_phase_log`（`GateFailLog { item, reason, diagnostics }` 参数化，static-check 调用点同步）；④ `impl TryFrom<ToolStepOutput> for ArchiCheckOutcome` | AC-12 / AC-13 / D17 / D18 |
| `crates/core/orchestration/src/run_history.rs` | `mapped_step` 增 `ChangeStepKind::ArchiCheck => RunStepKind::ArchiCheck` 臂 | 21 臂 → 22 臂 |
| `crates/core/orchestration/src/lib.rs` | `pub use port::{… ArchiCheckOutcome, ArchiCheckRunner …}`（字母序插入） | 出口 re-export |
| `crates/core/orchestration/src/control.rs` | **零代码改动**（复核项）：`is_session_step` 为正向 `matches!`，`ArchiCheck` 天然判假 | 事实校准 2 |
| `crates/infra/checks/src/static_check.rs` | `execute` 首部增 `ReportSlot::Archi` 守卫（显式 `Err`） | D19 / 事实校准 3 |
| `crates/infra/checks/src/testexec/mod.rs` | `execute` 首部同上守卫 | D19 |
| `src-tauri/Cargo.toml` | `members` 增 `"crates/infra/archi"`（字母序）；`[workspace.dependencies]` 增 `archi-runtime = { path = "crates/infra/archi" }` 与收敛项 `globset` / oxc 家族 6 件（`oxc_allocator` / `oxc_ast` / `oxc_parser` / `oxc_resolver` / `oxc_span` / `oxc_syntax`）/ `proc-macro2`（`span-locations`）/ `syn`（`sha2` / `time` 已在既有收敛项内，直接复用）；壳 `[dependencies]` 增 `archi-runtime = { workspace = true }`（带用途注释） | `desktop-crate-layout`；收敛项与 `crates/core/archi/Cargo.toml` 的 `{ workspace = true }` 声明**同批**落地，版本在声明同批钉定（oxc 五件同版 + `resolver` 独立版本线、只开 parser 所需特性，见待决问题 1） |
| `src-tauri/src/commands/archi/mod.rs` | 增 `archi_drift`（`#[tauri::command] #[specta::specta]`）+ `archi_drift_inner(root, mode, vcs)`；模块 doc 补第六命令 | 公共函数表两行；命令层零判定（AC-10） |
| `src-tauri/src/commands/change_flow/mod.rs` | `LocalToolSteps::new(...)` 增第四 runner `Arc::new(LocalArchiCheck::new())`（形参插在 `mutation` 之后、`vcs` 之前）+ 一句注释 | 装配（零判定） |
| `src-tauri/src/commands/mod.rs` | `all_commands!` 增 `$crate::commands::archi::archi_drift,` | 唯一登记面 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | `useArchiDrift(root, mode)` 接线；`CanvasBody` 收 `driftError` 渲染行内提示；`const marked = driftMarks(report, graph)` 后交既有 `toNodes` / `toEdges` | AC-19 / D20 / D21 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | `normalizeKind` 增 `case 'archi_check': return 'archiCheck';`；`stepWord` 增 `case 'archiCheck': return 'archi_check';` | AC-16（`groupOf` 走既有 `default` → `toolStep`，零改动） |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `STEP_LABEL` 增 `archiCheck: '架构检查'`（`Record<ChangeStepKind, string>` 穷尽表） | AC-16；`GROUP_LABEL` 四类不变 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 再生成：`archiDrift` 命令、16 类报告 DTO、`archiCheck` / `archi_check` 步词、`ArchiDriftMode` | 生成物（`bindings:check` 零 diff 为门） |
| `packages/desktop/package.json` | `version` `0.6.4` → `0.6.5` | 版本纪律（`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 不随动） |
| `.dev-team/architecture/models/03-backend-infra.toml` | 表头注释 `# 03 · infra 适配层（7 元素 / 21 关系）` → `（8 元素 / 24 关系）`（第二行注释逐字不变）；在 `[element."App.Backend.Infra.AgentRuntime"]` 之后插入 `[element."App.Backend.Infra.ArchiRuntime"]`（`kind = "module"`、`gate = "off"`、`path = ["./packages/desktop/src-tauri/crates/infra/archi/"]`、一句中文 `summary`，不复述 FQN、不写证据清单）；在 `AgentRuntime` 的 8 条边之后插入 3 条 `[[relation]]`（`from = "App.Backend.Infra.ArchiRuntime"` → `App.Backend.Core.Archi` / `App.Backend.Core.Foundation` / `App.Backend.Core.Orchestration`，各带一句中文 `desc`） | D25；`repo-archi-model`「后端下钻到 crate 层的元素覆盖」漂移纪律 + 「后端关系只取 Cargo 依赖声明」（`archi-runtime` 的 `[dependencies]` 三条，`tempfile` 属 dev 段不入模）；其余 7 元素 / 21 条边逐字不变 |
| `.dev-team/architecture/models/04-backend-shell.toml` | 表头 `# 04 · Tauri 壳（1 元素 / 12 关系）` → `（1 元素 / 13 关系）`；在 `App.Backend.Shell → App.Backend.Infra.AgentRuntime` 之后插入 1 条 `[[relation]]`（`to = "App.Backend.Infra.ArchiRuntime"`，`desc` 写「架构门禁 runner 的构造与注入」） | D25；壳 `Cargo.toml` 的 `archi-runtime = { workspace = true }`（AC-17 的要求）；其余 12 条逐字不变 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 计数复述五处同步：`§1`（`:9`）`49 元素 / 107 关系` → `50 元素 / 111 关系` 且后端 `43 条（9 + 21 + 12 + 1…）` → `47 条（9 + 24 + 13 + 1…）`；`§4`（`:36`）「后端仍 43 条」→ 47；`§5`（`:40`）「`App.Backend.Shell` 出边 12 条」→ 13；`§6`（`:44`）「基线 49 个元素」→ 50；`§9`（`:65-67`）复算表 `03(7)`→`03(8)`、`03(21)`→`03(24)`、`04(12)`→`04(13)`、元素 `49（容器 10 + 叶 39）`→`50（容器 10 + 叶 40）`、关系 `107`→`111`、后端 `43`→`47` | D25；`repo-archi-model`「计数复述单源」；六条判据（三档粒度 / 容器 `path` / dev 排除 / ui 收敛 / `gate` 全 `off` / 封闭排除面）与 `§9` 的前端拆分式 `64 = 58 + 6` **零改动** |

### 公共函数 / API

> 口径：Rust 侧列 `pub` / `pub(crate)` 模块级函数与 port 方法；前端列跨文件导出函数。私有函数（`enumerate` / `inline_ignore` 之外的扫描内部工序、`report_dir_name`、`file_stem`、`compose_text`、`step_archi_check`、`step_gate_fail_phase_log`、`settle_fault`、`diagnose_brief` 等）不入表。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `layout::archi_config_path` | `crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn archi_config_path(root: &Path) -> PathBuf` | `<archi_dir>/config.json`（从 `archi_dir` 派生、复用 `CONFIG_FILE_NAME`）；纯推导零 IO；架构域看护配置唯一合法定位函数 |
| `layout::report_dir_name` | 同上 | 修改 | `fn report_dir_name(slot: ReportSlot) -> &'static str` | 二分臂 → 三分臂（`test` / `baseline` / `archi`）；私有，消费面经 `change_reports` |
| `StepNode::as_str` / `group` / `role` | `crates/core/workflow/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` / `pub fn group(self) -> StepGroup` / `pub fn role(self) -> Option<AgentRole>` | 各补 `ArchiCheck` 臂（`"archi_check"` / `StepGroup::Exec` / `None`） |
| `RunStepKind::as_str` | 同上 | 修改 | `pub fn as_str(self) -> &'static str` | 补 `ArchiCheck => "archi_check"` |
| `StepKind::as_str` | 同上 | 修改 | `pub fn as_str(self) -> &'static str` | 审计包络八臂 → 九臂 |
| `ChangeStepKind::as_str` | `crates/core/orchestration/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` | 补 `ArchiCheck => "archi_check"`（节点身份串口径） |
| `ArchiCheckRunner::run` | `crates/core/orchestration/src/port.rs` | 新增 | `fn run(&self, root: &str, name: &str, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture` | 门禁 runner port；`changed` 由 `steps.rs` 经 `ArchiveVcsPort::change_edit_set(root, "HEAD")` 注入；槽位恒 `ReportSlot::Archi`（不设形参）；取消以 `fault = Some(RunFault::Stopped)` 正常返回 |
| `LocalToolSteps::new` | `crates/core/orchestration/src/steps.rs` | 修改 | `pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>, test_execution: Arc<dyn TestExecutionRunner>, mutation: Arc<dyn MutationRunner>, archi: Arc<dyn ArchiCheckRunner>, vcs: Arc<dyn ArchiveVcsPort>, store: Arc<dyn ChangeStateStore>, run_id: String) -> Self` | 第四 runner 插在 mutation 之后（组合根形参序即声明序） |
| `drift::check` | `crates/core/archi/src/drift/mod.rs` | 新增 | `pub fn check(input: CheckInput<'_>) -> Result<Report, CheckError>` | 判定唯一入口（命令面与门禁面同源）：模型 → 配置 → 扫描 → 对账定级 → 豁免 → `status` → 报告。spec Module Contract 的 `check(CheckInput) -> Report` 落地为 `Result`：看护配置非法（AC-6「检查不执行且不产出报告」）与取消（门禁要落 `fault = Stopped`）两个形态必须可辨，MUST NOT 折叠成报告内的一个状态值 |
| `CheckError::message` | 同上 | 新增 | `pub fn message(&self) -> String` | 错误面人读文案（`Stopped` → 「检查已取消」；其余透传内层） |
| `drift::config::load_config` | `crates/core/archi/src/drift/config.rs` | 新增 | `pub fn load_config(root: &Path, known: &BTreeSet<String>) -> Result<CheckConfig, String>` | 经 `archi_config_path` 定位；文件缺失 → 空配置；未知键 / `reason` 缺失或空白 / FQN 不存在 / 非法 `exclude` → `Err`（文案指明条目） |
| `scan::scan` | `crates/core/archi/src/scan/mod.rs` | 新增 | `pub fn scan(input: ScanInput<'_>) -> Result<ScanOutcome, CheckError>` | 枚举 → 排除 → 归属 → 抽取 → 聚合；取消置位 → `Err(CheckError::Stopped)`；零写盘、零 spawn |
| `scan::inline_ignore` | 同上 | 新增 | `pub(crate) fn inline_ignore(source: &str, line: u32) -> Option<String>` | 取该行与其上一行匹配 `// archi-ignore-next-line:`，冒号后非空文本即 reason；空 reason 视为未标记 |
| `ExcludeSet::build` | `crates/core/archi/src/scan/exclude.rs` | 新增 | `pub fn build(extra: &[String]) -> Result<ExcludeSet, String>` | 默认七条 + 配置追加（globset 编译一次）；非法配置 glob / 无通配符条目 → `Err` |
| `ExcludeSet::matched` | 同上 | 新增 | `pub fn matched(&self, rel_path: &str) -> Option<String>` | 命中返回命中的规则原文（最小下标优先，确定性） |
| `OwnerIndex::build` | `crates/core/archi/src/scan/owner.rs` | 新增 | `pub fn build(elements: &[ArchiElement]) -> OwnerIndex` | 只收 `path` 非空元素；条目归一（`\` → `/`、剥 `./`、去尾分隔符） |
| `OwnerIndex::owner_of` | 同上 | 新增 | `pub fn owner_of(&self, rel_path: &str) -> Owner` | 最长前缀：唯一 → `Element`；同长多命中 → `Ambiguous`（字典序）；无命中 → `None` |
| `TsResolver::new` | `crates/core/archi/src/scan/ts.rs` | 新增 | `pub fn new(root: &Path) -> TsResolver` | 观察根锚（specifier 落盘与 tsconfig 向上查找） |
| `TsResolver::extract` | 同上 | 新增 | `pub fn extract(&self, file_rel: &str, source: &str) -> (Vec<ImportFact>, Vec<ArchiUnresolvedFile>)` | 静态 `import` / `import type` / `export … from`；动态 `import()` / `require()` / 语法错 → `unresolved` |
| `TsResolver::resolve` | 同上 | 新增 | `pub fn resolve(&mut self, file_rel: &str, specifier: &str) -> Option<String>` | 就近 tsconfig / jsconfig 的 `baseUrl` + `paths`（解析器按配置树缓存）→ node 规则；失败 `None` |
| `RustResolver::new` | `crates/core/archi/src/scan/rust.rs` | 新增 | `pub fn new(root: &Path) -> Result<RustResolver, String>` | 解析 workspace 清单 + member glob → crate 清单 → 逐 crate 模块树；清单不可解析 → `Err` |
| `RustResolver::unresolved` | 同上 | 新增 | `pub fn unresolved(&self) -> &[ArchiUnresolvedFile]` | 建树期不可判定项（宏 `mod` / `include!` / 孤儿 `.rs` / `mod` 定位失败） |
| `RustResolver::extract` | 同上 | 新增 | `pub fn extract(&self, file_rel: &str, source: &str) -> (Vec<ImportFact>, Vec<ArchiUnresolvedFile>)` | 单文件 `use` 树抽取（`crate::` / `self::` / `super::` / workspace crate 名）；`syn` 解析失败 → `unresolved` |
| `RustResolver::resolve` | 同上 | 新增 | `pub fn resolve(&self, file_rel: &str, specifier: &str) -> Option<String>` | 模块路径最长前缀匹配落到文件；外部 crate → `None`（静默丢弃） |
| `LocalArchiCheck::new` | `crates/infra/archi/src/runner.rs` | 新增 | `pub fn new() -> Self` | 零参构造（组合根装配；另附 `impl Default`） |
| `commands::archi::archi_drift` | `src-tauri/src/commands/archi/mod.rs` | 新增 | `pub fn archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String>` | `#[tauri::command] #[specta::specta]`；构造 `ProcessArchiveVcs` 后转 `*_inner` |
| `commands::archi::archi_drift_inner` | 同上 | 新增 | `pub fn archi_drift_inner(root: &Path, mode: ArchiDriftMode, vcs: &dyn ArchiveVcsPort) -> Result<ArchiDriftReport, String>` | `check_root` → 增量经 `vcs.change_edit_set(root, "HEAD")?` 取变更集 / 全量空集 → `drift::check`（`change: None`、`cancel: CancelFlag::never()`）→ `CheckError::message` 映射；命令层零判定 |
| `LocalArchiCheck::run`（`impl ArchiCheckRunner`） | `crates/infra/archi/src/runner.rs` | 新增 | `fn run(&self, root: &str, name: &str, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture` | `check`（增量、`change = Some(name)`）→ `write_report` → `{ passed, diagnostics, fault, report_dir, report_ref }`；`Stopped` → `fault = Some(RunFault::Stopped)`；写盘失败 → `report_ref = None` 且结论保留 |
| `write_report` | `crates/infra/archi/src/report.rs` | 新增 | `pub(crate) fn write_report(root: &Path, name: &str, report: &ArchiDriftReport) -> Result<String, String>` | 目录经 `change_reports(root, name, ReportSlot::Archi)` 派生 + `create_dir_all` + 文件名（`<YYYYMMDDTHHMMSSZ>.json`）+ `serde_json::to_vec_pretty`；返回文件路径 |
| `driftMarks` | `packages/desktop/src/views/architecture/lib/drift-marks.ts` | 新增 | `driftMarks(report: ArchiDriftReport \| null, graph: ArchiGraph): ArchiGraph` | 只读 `observations` / `suppressed`：`unfulfilledDeclaration` → 边 `unfulfilled` + `from` 节点 `unfulfilled`；`suppressed` → 边 `exempt` + `reason`；其余保持 `none`；`report === null` 原样返回 |
| `useArchiDrift` | `packages/desktop/src/views/architecture/hooks/use-archi-drift.ts` | 新增 | `useArchiDrift(root: string \| null, mode: ArchiDriftMode): { report: ArchiDriftReport \| null; loading: boolean; error: string \| null; refresh: () => void }` | 三触发（挂载 / `root` 变化 / 手动刷新），零 watch；`root === null` 不 invoke；失败落 `error` 且 `report = null` |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `ArchiCheckStatus` | `crates/core/archi/src/drift/report.rs` | 新增 | 封闭五值 `clean` / `errors` / `warnings` / `noChanges` / `skipped`（serde `camelCase`——`noChanges` 为驼峰，见 D10 取值口径）+ `as_str` + `ALL: [Self; 5]` |
| `ArchiDriftMode` | 同上 | 新增 | 封闭二值 `full` / `incremental`（serde `lowercase`，specta 出线为 TS 联合）+ `as_str` + `ALL` |
| `ArchiDriftKind` | 同上 | 新增 | 漂移条目词汇四值 `undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`（serde `camelCase`）+ `as_str` + `ALL` |
| `ArchiObservationKind` | 同上 | 新增 | 观察项词汇两值 `unfulfilledDeclaration` / `unusedDeclaration`（serde `camelCase`）+ `as_str` + `ALL`（`unusedDeclaration` 本变更不产出，词汇位由 `desktop-archi-drift` 明列） |
| `ArchiSuppressVia` | 同上 | 新增 | 豁免来源两值 `config` / `inline`（serde `lowercase`） |
| `ArchiDriftReport`（别名 `Report`） | 同上 | 新增 | `{ version: u32, timestamp: String, mode: ArchiDriftMode, root: String, change: Option<String>, base: Option<String>, model_hash: String, model: ArchiDriftModel, scan: ArchiScanReport, errors: Vec<ArchiDriftEntry>, warnings: Vec<ArchiDriftEntry>, suppressed: Vec<ArchiDriftSuppressedEntry>, observations: Vec<ArchiDriftObservation>, status: ArchiCheckStatus }` |
| `ArchiDriftModel` | 同上 | 新增 | `{ elements: u32, relations: u32, by_gate: ArchiGateCount, structural_errors: Vec<ArchiDiagnostic>, warnings: Vec<ArchiDriftEntry> }`（结构诊断复用 `dto::ArchiDiagnostic`——同一语义的同一类型；`warnings` 用漂移条目形状，见 D12） |
| `ArchiGateCount` | 同上 | 新增 | `{ strict: u32, warn: u32, off: u32 }` |
| `ArchiScanReport` | 同上 | 新增 | `{ mode: ArchiDriftMode, changed_files: u32, scanned_files: u32, excluded: Vec<ArchiExcludedFile>, unmatched_files: Vec<String>, matched: Vec<ArchiMatchedElement>, unresolved: Vec<ArchiUnresolvedFile> }` |
| `ArchiExcludedFile` | 同上 | 新增 | `{ file: String, rule: String }`（`rule` = 命中的具体 glob，D8） |
| `ArchiMatchedElement` | 同上 | 新增 | `{ element: String, files: Vec<String> }`（元素 FQN 字典序、`files` 路径字典序） |
| `ArchiUnresolvedFile` | 同上 | 新增 | `{ file: String, specifier: Option<String>, message: String }` |
| `ArchiDriftEntry` | 同上 | 新增 | `{ kind: ArchiDriftKind, from: Option<String>, to: Option<String>, gate: Option<GateTier>, message: String, occurrences: Vec<ArchiDriftOccurrence> }`（`from` 为定级依据；path 类条目 `occurrences` 恒一条、`line = 0`） |
| `ArchiDriftOccurrence` | 同上 | 新增 | `{ file: String, line: u32, specifier: Option<String>, type_only: bool }`（`line == 0` = 位置未知，沿 `ArchiDiagnostic` 同口径） |
| `ArchiDriftSuppressedEntry` | 同上 | 新增 | `{ from: String, to: String, via: ArchiSuppressVia, reason: String, file: Option<String>, line: Option<u32> }`（`via = config` 时 file / line 为 `None`） |
| `ArchiDriftObservation` | 同上 | 新增 | `{ kind: ArchiObservationKind, from: String, to: String, message: String }` |
| `ArchiCheckOutcome` | `crates/core/orchestration/src/port.rs` | 新增 | `{ passed: bool, diagnostics: String, fault: Option<RunFault>, report_dir: String, report_ref: Option<String> }`（`derive Debug, Clone, PartialEq, Eq`；零 `Serialize`——不进 IPC / `RunUpdate`，与既有三 outcome 同型） |
| `CheckConfig` | `crates/core/archi/src/drift/config.rs` | 新增 | `{ ignore: Vec<IgnoreRule>, exclude: Vec<String> }`（`Default` = 空两项） |
| `IgnoreRule` | 同上 | 新增 | `{ from: String, to: String, reason: String }`（两者均为模型内元素 FQN） |
| `CheckInput<'a>` | `crates/core/archi/src/drift/mod.rs` | 新增 | `{ root: &'a Path, mode: ArchiDriftMode, changed: &'a [String], change: Option<&'a str>, cancel: CancelFlag }` |
| `CheckError` | 同上 | 新增 | 封闭三值 `Stopped` / `Config(String)` / `Scan(String)`（`derive Debug, Clone, PartialEq, Eq`；取消与失败面必须可辨——runner 靠它映射 `fault = Stopped`）；与报告结构**同批落地**（`scan` 的返回类型依赖它，后置定义会让扫描面不可编译） |
| `ScanInput<'a>` | `crates/core/archi/src/scan/mod.rs` | 新增 | `{ root: &'a Path, mode: ArchiDriftMode, changed: &'a [String], exclude: &'a [String], cancel: CancelFlag }` |
| `ScanOutcome` | 同上 | 新增 | `{ report: ArchiScanReport, edges: Vec<DependencyEdge> }` |
| `DependencyEdge` | 同上 | 新增 | `{ from: String, to: String, file: String, line: u32, specifier: String, type_only: bool, inline_ignore: Option<String> }`（已剔除同元素内边；`inline_ignore` 见 D5） |
| `ImportFact` | 同上 | 新增 | `{ specifier: String, line: u32, type_only: bool }`（两套解析器的共同产出形状） |
| `ExcludeSet` | `crates/core/archi/src/scan/exclude.rs` | 新增 | 持 `globset::GlobSet` + 规则原文表（下标对齐；非 `Clone`） |
| `Owner` | `crates/core/archi/src/scan/owner.rs` | 新增 | 封闭三值 `Element { fqn: String, gate: GateTier }` / `Ambiguous(Vec<String>)` / `None` |
| `OwnerIndex` | 同上 | 新增 | 持归一后的 `(元素 FQN, 前缀, gate)` 条目表（构建一次复用） |
| `TsResolver` | `crates/core/archi/src/scan/ts.rs` | 新增 | 持观察根 + tsconfig / jsconfig 缓存 + `oxc_resolver::Resolver` 缓存（按配置树） |
| `RustResolver` | `crates/core/archi/src/scan/rust.rs` | 新增 | 持 crate 清单 + 模块路径 → 文件表 + 建树期 `unresolved` |
| `ReportSlot` | `crates/core/foundation/src/layout/mod.rs` | 修改 | 封闭二值 → **封闭三值** `Test` / `Baseline` / `Archi`（纯进程内枚举，零 serde / specta） |
| `GateFailLog<'a>` | `crates/core/orchestration/src/walker.rs` | 新增 | `{ item: &'a str, reason: &'a str, diagnostics: &'a str }`（`step_gate_fail_phase_log` 的载荷；私有） |
| `StepNode` / `RunStepKind` / `StepKind` | `crates/core/workflow/src/state.rs` | 修改 | 三枚举尾部追加 `ArchiCheck`（21 → 22 / 21 → 22 / 八 → 九臂），词面 `archi_check` |
| `ChangeStepKind` | `crates/core/orchestration/src/state.rs` | 修改 | 尾部追加 `ArchiCheck`（21 → 22，camelCase 线词 `archiCheck`） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `ignore` | `<archi_dir>/config.json`（经 `foundation::layout::archi_config_path`） | 新增 | `[{ from: string, to: string, reason: string }]` | `[]`（文件缺失 / 键缺失） | 第 2 层豁免的元素对；`reason` 必填非空白、`from` / `to` 必须是模型内 FQN、**禁 glob**；被抑制条目进 `suppressed[]` |
| `exclude` | 同上 | 新增 | `[string]` | `[]` | 文件级 glob，**追加**于七条默认排除约定之后（不替换）；无通配符条目显式 `Err` |

> 工作区配置（`openspec/config.json`）的 `KNOWN_KEYS` 七项、`config.schema.json` 镜像与前端 config 页**零变化**——架构域配置不是工作区配置，其读取面落 `crates/core/archi`（不经 `config` crate）。本变更零新增环境变量、零运行时开关。

---

## 数据模型 (Data Model)

### 持久化一：报告文件 `<change>/reports/archi/<YYYYMMDDTHHMMSSZ>.json`

- **写入面唯一**：`infra/archi/src/report.rs::write_report`（目录经 `foundation::layout::change_reports(root, name, ReportSlot::Archi)` 派生；`reports/test` / `reports/baseline` 字节零变化、summary 索引不含漂移条目）。
- **写时序**：先 `create_dir_all` 再整份 `serde_json::to_vec_pretty` 一次 `fs::write`（无半写拼接）；文件名取自报告 `timestamp` 的紧凑形态（无冒号，跨平台合法）。同一次门禁一份；重跑覆盖不同时间戳、不删旧报告（与 `reports/test` 同口径的堆积，R-10 已接受）。
- **消费面**：门禁审计（`report_ref` / 审计行 `reference` = 报告目录）、`archi_drift` 命令面（不落盘、直接返回同一结构）、前端画布标记（命令面取数）、未来 ratchet 决策。全量模式**不落盘**（可重算观测）。

字段面与形态见 D1 与类型表；三条不变量：

| 不变量 | 内容 |
|--------|------|
| 词汇封闭 | `status` 五值 / `errors[]`+`warnings[]` 的 `kind` 四值 / `observations[].kind` 两值，各在 `report.rs` 单点定义并带 `ALL`；新增变体不同步即语料红 |
| 结论单源 | `passed` 不由报告携带——门禁侧派生 `errors.is_empty() && model.structuralErrors.is_empty()`；报告只给 `status` 与条目 |
| 落痕不删 | `suppressed[]` / `scan.excluded[]` / `scan.unresolved[]` 一律留痕（豁免与「没扫到」都可审计） |

### 持久化二：架构域看护配置 `<archi_dir>/config.json`

- 单文件两键（`ignore[]` / `exclude[]`），`deny_unknown_fields`；缺失即空配置（冷启动合法起点，零报错）。
- 读取面**唯一**：`crates/core/archi/src/drift/config.rs::load_config`（只经 `foundation::layout::archi_config_path` 定位；产品 `.rs` 无 `config.json` 字面量）。
- 数据约束：`ignore[].reason` 非空白；`ignore[].from` / `to` 必须是模型内 FQN（指向已删除元素即配置漂移 → 显式 `Err`）；`exclude[]` 每条须含通配符（目录写 `src/legacy/**`）。

### 持久化三（数据资产）：本仓架构模型 `<archi_dir>/models/` 的追加

本变更不改模型格式（`desktop-archi-model` 零改动），只在既有文件里**追加数据**并同步计数复述。追加面固定为四条边 / 一个元素：

| 追加项 | 落点 | 内容 |
|--------|------|------|
| 元素 `App.Backend.Infra.ArchiRuntime` | `models/03-backend-infra.toml`（`AgentRuntime` 之后，字母序） | `kind = "module"` / `gate = "off"` / `path = ["./packages/desktop/src-tauri/crates/infra/archi/"]` / 一句中文 `summary`；容器归属由 FQN 前缀 `App.Backend.Infra` 表达（容器已在 `03` 声明，MUST NOT 重复声明） |
| 边 `ArchiRuntime → Core.Archi` | 同上（3 条边一组） | `desc`：判定与报告结构取自 core 判定域 |
| 边 `ArchiRuntime → Core.Foundation` | 同上 | `desc`：报告槽位 / 取消信号 / 架构域路径派生的基础件 |
| 边 `ArchiRuntime → Core.Orchestration` | 同上 | `desc`：门禁 runner 实现编排的步骤端口 |
| 边 `Shell → Infra.ArchiRuntime` | `models/04-backend-shell.toml`（`AgentRuntime` 之后） | `desc`：架构门禁 runner 的构造与注入 |

四条边与 `Cargo.toml` 的 `[dependencies]` 逐条相等（`archi-runtime` 三条 + 壳一条；`serde_json` 与 `oxc_*` / `syn` / `sha2` / `time` 属外部依赖，不连边；`tempfile` 在 dev 段，整体排除）。

**计数不变量（三处同值，任一陈旧即漂移）**

| 口径 | 变更前 | 变更后 | 复述面 |
|------|--------|--------|--------|
| 元素总数 | 49 | **50**（容器 10 + 叶 40） | `models/03` 表头与 `04` 表头的同批改动、`repo-archi-model` 计数表、ADR `§6` / `§9` |
| 关系总数 | 107（后端 43 + 前端 64） | **111**（后端 47 + 前端 64） | 同上 + ADR `§1` / `§4` |
| `03` 段 | 7 元素 / 21 关系 | **8 元素 / 24 关系** | `03` 表头 / spec 计数表 / ADR `§9` |
| `04` 段 | 1 元素 / 12 关系 | 1 元素 / **13 关系** | `04` 表头 / spec 计数表 / ADR `§5` / `§9` |
| 逐文件拆分 | `0 + 9 + 21 + 12 + 1 + 64` | `0 + 9 + 24 + 13 + 1 + 64` | ADR `§9` |

**已知缺口单列**：`crates/tools/specta-zod-patched`（vendored 上游 exporter）先于本变更存在且无元素，本变更 MUST NOT 顺手补齐（补缺或豁免判据归独立变更）——双向覆盖核对时它是**唯一**允许的非空差集，其余零差集（15 个自研 crate ↔ 15 个 crate 元素）。

**核对留痕（change 交付面）**：`openspec/changes/archi-check-and-gate/reports/archi/model-sync-check.md` 记录双向覆盖差集、四段计数与 `archi_graph` 正常态取证。它是**核对证据**而非漂移报告——不写入 `<timestamp>.json` 报告序列、不被任何消费面（门禁 / 命令面 / 画布）读取，故不构成本变更「报告写入面唯一」纪律的例外。

### 进程内结构（不落库）

```
drift::check(CheckInput)
  ├── foundation::layout::archi_models_dir(root) 存在性 → skipped 早退
  ├── dto::graph_view(root)            → ArchiGraph（elements / relations / diagnostics）
  ├── drift::config::load_config       → CheckConfig { ignore, exclude }
  ├── scan::scan(ScanInput)            → ScanOutcome { report, edges }
  │     ├── enumerate（ignore::WalkBuilder，gitignore 感知）
  │     ├── exclude::ExcludeSet         → excluded[]
  │     ├── owner::OwnerIndex           → matched[] / unmatched[] / ambiguousOwner（`off` 覆盖文件计 unmatched，但依赖照抽——D6）
  │     ├── ts::TsResolver | rust::RustResolver → edges[] / unresolved[]
  │     └── inline_ignore(source, line) → edges[].inline_ignore
  └── 对账：declared(BTreeSet<(from,to)>) vs edges（按 (from,to) 聚合）
        ├── ② 未声明 + 配置豁免 → suppressed{via=config}
        ├── ② 未声明 + 内联豁免 → suppressed{via=inline, file, line}
        ├── ② 未声明 + 无豁免 → 按 from.gate 定级（strict→errors / warn→warnings / off 不出现）
        ├── ④ path 失效 / noPath / ambiguousOwner → 按元素档 + model.warnings
        └── ① 声明无对应边 → observations（永不进 status）
        → status 推导（D10）→ Report
```

**零库改动**：本变更不新增 / 不迁移任何 db 表；`RunNodeRecord.step` 的 bincode 下标编码不变（`ArchiCheck` 尾部追加，存量行解码语义逐字不变）。

---

## 路由 / API 设计 (Route / API Design)

### IPC 命令（1 条新增，显式 `root`，错误面 `Result<T, String>`）

| 方法 | 路径 | 说明 | 输入 | 输出 | auth |
|------|------|------|------|------|------|
| invoke | `archi_drift` | 架构漂移检查（两模式；命令层零判定） | `root: string`（观察根，workspace 级 = 主仓根 / change 级 = `detail.worktree`）、`mode: ArchiDriftMode`（`"full"` \| `"incremental"`） | `ArchiDriftReport`（全量不落盘、增量经 vcs 缝取变更集后同源判定） | 无（桌面进程内命令，与其余 `archi_*` 同轨） |

行为约定：

- **模式分流**：`full` → `changed = &[]`、`base = null`、不落盘；`incremental` → `vcs.change_edit_set(root, "HEAD")?`（git 失败显式 `Err`，MUST NOT 降级空清单）、`base = "HEAD"`。
- **无 change 上下文**：命令面恒 `change = None`（报告 `change` 字段为 null），与门禁面（`change = Some(name)`）的区别仅此与 `timestamp`（AC-10）。
- **错误面**：`check_root` 失败（空白 / 非目录）、模型结构错**不** `Err`（结构错进报告 `model.structuralErrors` 并令 `status = errors`）、配置非法 / 扫描硬失败 → `Err`（文案指明条目）；取消不可达（命令面 `CancelFlag::never()`）。
- 命令经 `all_commands!` 单点登记，`commands.archiDrift` 由 `bindings:export` 生成。

### 前端取数（零新路由）

| 挂载点 | 组件 | `mode` | 观察根 |
|--------|------|--------|--------|
| `/architecture` | `architecture-view.tsx` → `ArchitectureCanvas` | `full` | 当前 workspace 根 |
| change 详情（架构页签 / 流程页画布） | `change-detail-view.tsx` / `run-control-panel.tsx` → `ArchitectureCanvas` | `incremental` | `detail.worktree`（归档 change 与 legacy 回落主仓根） |

- 取数三触发（挂载 / 根切换 / 手动刷新），零 watch 零轮询；聚焦切换与节点点击**零 IPC**。
- 失败降级：`error` 行内持久呈现 + 图照常渲染（零标记）；`report === null` 时 `driftMarks` 恒等返回。
- 标记映射为取数侧纯函数（`driftMarks`），`marks.ts` 四值边标记 / 两值节点标记的样式契约**零改动**。

---

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 落位 | 用途 | 版本 / 来源 |
|------|------|------|-------------|
| oxc 家族 **6 件**：`oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` / `oxc_resolver` | `archi` | TS / JS 词法与 AST（静态 import + `typeOnly` + 行号）、模块路径解析（`baseUrl` + `paths` + node 规则） | **真新增**（`oxc_parser` / `oxc_allocator` / `oxc_resolver` 三件及其传递树不在锁内；`ast` / `span` / `syntax` 为 `oxc_parser` 同版伴随件，声明为直接依赖零新增锁包）；已钉定家族五件 `=0.146.0` + `oxc_resolver` `=11.24.3`，特性只开 parser 所需（不引 semantic / codegen）——见 §6「版本钉定」与待决问题 1 的关闭记录 |
| `syn` | `archi` | Rust 项解析（`mod` / `use` / `#[path]` / `cfg` 属性），`features = ["full"]` | 锁内 `2.0.119`（零新增包） |
| `globset` | `archi` | 排障匹配（默认七条 + 配置追加）与 member glob | 锁内 `0.4.20`（由传递依赖提为直接） |
| `sha2` | `archi` | `modelHash`（`models/*.toml` 聚合 sha256，`sha256:` 前缀） | 锁内（`foundation` 已用；本 crate 需直接引用） |
| `time` | `archi` | `timestamp`（RFC3339 UTC） | 锁内（`checks-runtime` 已用） |
| `ignore` | `archi`（既有） | gitignore 感知遍历（`WalkBuilder`） | 锁内 |
| `serde_json` | `infra/archi` | 报告序列化落盘 | 锁内 |
| `foundation` / `orchestration` / `archi` | `infra/archi` | 路径派生 / port 与产出类型 / 判定与报告结构 | workspace |
| `vcs-runtime`（`ProcessArchiveVcs`） | `src-tauri`（既有） | `archi_drift` 增量模式取变更集（`change_edit_set`） | workspace |

**依赖方向断言**（机械可验）：`archi` 的 workspace 内依赖恰为 `foundation`；`archi-runtime` 恰为 `archi` + `orchestration` + `foundation`；`workflow` 依赖中不出现 `archi`，`archi` 依赖中不出现 `workflow`；`infra/` 只新增 `archi` 一个成员；`archi` 与 `archi-runtime` 零 `std::process` / `tokio::process` / shell。同一组声明必须在模型侧逐条有对应 `[[relation]]`（`archi-runtime` 三条 + 壳一条，D25）——`Cargo.toml` 与 `.dev-team/architecture/models/{03,04}` 互为证据，单向缺一即「关系证据」核对失败。

### 构建 / 测试依赖

| 依赖 | 落位 | 用途 |
|------|------|------|
| `tempfile` | `infra/archi` `[dev-dependencies]` | 报告落盘 / 写盘失败降级用例（与 `infra/checks` 同式） |
| `specta` / `specta-typescript` / `specta-zod` | 壳（既有） | 报告 DTO 与 `archi_drift` 命令出线（`bindings.ts` + `bindings.zod.ts`） |

### 成本预算与实测表（AC-18）

阈值（软线 / 硬线）与回退序先写死；实测四段式（基线 → 变体 → 差分 → 判读，全程同一台机器），**不常态化跑 CI**（共享 runner 抖动会淹没差值）。

**分工按「谁有能力测」二分**——不把不可测项压给没有环境的相位：

| 口径 | 指标 | 执行相位 | 本文件状态 |
|------|------|----------|------------|
| 本地可自证（零构建、零测试命令，只数锁文件） | `Cargo.lock` 净新增包数 | 实现相位 | **已实测回填**（998 → 1038，+40） |
| 须测试装置 | 单次检查 p95（300 元素 / 1000 源文件，release） | **test-execution 相位**（执行 `tests/perf.rs` 后回填） | **已实测回填**（617.4 ms，release，n = 10） |
| 须 release 打包 / 基线对照构建 | NSIS 安装包体积 / 主二进制体积 / CI 冷构建（单腿）/ 本地增量（改 `archi` 一文件） | **发布实测**（具 release 打包环境的相位或人工实测） | 未测（移交发布实测） |

实现相位无 release 打包环境（CI 冷构建单腿更无法本地产生）、且 MUST NOT 执行任何测试命令，故**依赖成本四项**只做未测登记与移交标注，MUST NOT 以本地临时跑值充数；**p95 于 acceptance 前按 release 口径补跑回填**（test-execution 门禁实跑为 debug 口径，其连通性样本不作判据）。下表未测文本与 `crates/core/archi/tests/README.md` 的同名表**逐字一致（含未测文本一致）**；改任一处必须同批改另一处。

**阈值（与 `tests/README.md`「一、阈值」逐字一致）**

| 指标 | 软线 | 硬线 | 备注 |
|------|------|------|------|
| NSIS 安装包 | +0.8 MB | +1.5 MB（≤ 8.2 MB） | 越硬线触发回退序 |
| 主二进制 | +3 MB | +5 MB（≤ 31.6 MB） | 同上 |
| `Cargo.lock` 净新增包数 | ≤ 25 | ≤ 40 | oxc 家族是唯一真新增面 |
| CI 冷构建（单腿） | +45 s | +90 s | 共享 runner 抖动会淹没差值——不常态化跑 |
| 本地增量（改 `archi` 一文件） | +8 s | +15 s | 同上 |
| 单次检查（300 元素 / 1000 源文件） | 1.5 s | 3 s（p95，release） | `std::time::Instant` 计时，零统计框架 |

**回退序（代价递增，越硬线即按序执行并留痕）**：① 收 `oxc_parser` 特性（只 parse，不引 semantic / codegen）→ ② 自算路径替代 `oxc_resolver`（约 150 行；`exports` / `imports` 退化）→ ③ 正则 import 替代 `oxc_parser`（`typeOnly` 与行号退化，且实质推翻「type-only 算架构依赖」的裁决——**必须回到设计讨论**，MUST NOT 实现层自定）。每次回退后重测，并把两轮数据一并留在表内。

**版本钉定（本相位可确证的部分；与 `tests/README.md`「三、实测表 · 版本钉定」逐字一致）**

| 依赖 | 钉定值 | 判据 |
|------|--------|------|
| `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` | `=0.146.0` | 家族同版发布面中**本工具链可用的最新同版**（`rustc 1.95.0`；0.147.0 起 `rust-version = 1.96.0`、0.153.0 起 1.97.0——超出本仓工具链）。`oxc_ast` / `oxc_span` / `oxc_syntax` 是 `oxc_parser` 的同版伴随件（`ParserReturn.program` / `SourceType` 需直接引用其类型），声明为直接依赖**零新增锁包** |
| `oxc_parser` 特性面 | `default-features = false` | 只开 parser 所需（不引 `regular_expression` / semantic / codegen）——回退序 ① 已在钉版时执行 |
| `oxc_resolver` | `=11.24.3` | 独立版本线（11.x，与 oxc 家族不同版），取本工具链（`rust-version = 1.95.0`）可用的最新值 |
| `syn` | `2`（`features = ["full"]`） | 锁内既有 2.0.119，零新增包 |
| `proc-macro2` | `1`（`features = ["span-locations"]`） | 锁内既有；`syn` 的 `use` 项行号依赖该特性（fallback span 行列记录） |
| `globset` | `0.4` | 锁内既有 0.4.20（由传递依赖提为直接） |
| `sha2` / `time` | 既有收敛项 | `modelHash` / `timestamp`；锁内已有消费者 |

> **件数（实测 `crates/core/archi/Cargo.toml` 与 `[workspace.dependencies]` 逐项对齐）**：oxc 家族直接依赖共 **6 件**（上表第一行为 `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` 五件同版 + `oxc_resolver` 一件独立版本线），另加 **`proc-macro2`**（`syn` 的 span 行号伴随件，锁内既有、随 `span-locations` 特性提为直接）——「三件」只是「真新增锁包的三件」（parser / allocator / resolver），直接依赖面 MUST NOT 按三件计。

**依赖成本实测表（四段式；与 `tests/README.md`「三、实测表 · 依赖成本」逐字一致）**

| 指标 | 基线（实测） | 变体（实测） | 差分 | 判读 |
|------|--------------|--------------|------|------|
| NSIS 安装包 | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 本仓无 release 打包环境；MUST NOT 以临时跑值充数 |
| 主二进制 | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 同上 |
| `Cargo.lock` 净新增包数 | 998 包（HEAD） | 1038 包（含本变更） | **+40**（34 个新包名 + 版本分裂条目；其中 `archi-runtime` 为本仓新 crate ⇒ 外部包 39） | 越软线（≤ 25）、**正好落在硬线（≤ 40）内**；回退序 ① 已在钉版时执行（`oxc_parser` `default-features = false`），再往下即回退序 ② |
| CI 冷构建（单腿） | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 同上 |
| 本地增量（改 `archi` 一文件） | 未测（移交发布实测） | 未测（移交发布实测） | 未测 | 同上 |

> 上表四组「未测（移交发布实测）」是本仓环境的**机械事实**（无 release 打包环境、CI 单腿无法本地产生），不是待办遗忘；执行相位回填后必须同批替换本行与 `tests/README.md` 同格。

> `oxc_*` / `syn` 的引入是本变更唯一真新增依赖面；若锁包数越硬线（≤ 40），按回退序 ① 先收紧特性再复测。

**运行期预算（与 `tests/README.md`「三、实测表 · 运行期预算」逐字一致）**

| 指标 | 软线 | 硬线 | 实测 p95 | 数据来源 |
|------|------|------|----------|----------|
| 单次检查（300 元素 / 1000 源文件，release） | 1.5 s | 3 s | **0.62 s**（617.4 ms，release，n = 10；p50 596.3 ms） | `tests/perf.rs`（10 次取 p95） |

> 判读：`cargo test -p archi --release --test perf -- --nocapture`（cargo 1.95.0，本机）实测
> 617.4 ms，**落在软线（1.5 s）内**，距硬线（3 s）余量约 4.9 倍；不触发回退序任一步。样本
> （ms）：`601.5 / 605.0 / 592.8 / 588.4 / 583.5 / 617.4 / 593.7 / 613.5 / 596.3 / 612.5`。
> 回填时点：acceptance 前（test-execution 门禁只跑到 debug 口径，其连通性样本不作判据）。

---

## 验收标准对齐

> 测试用例与语料装置归 test-design / test-gen / test-execution 相位（proposal「测试文件」表）；本表只标设计落点。

| AC | 设计落点 |
|----|----------|
| AC-1 扫描面与测试 scope 正交 | `scan/exclude.rs` 默认七条常量（零读取工作区配置，D8）；`excluded[]` 逐条留痕；`ExcludeSet::build` 追加语义 |
| AC-2 归属单点四形态 | `scan/owner.rs` 三值 `Owner`（D6）；`unmatchedFiles` / `ambiguousOwner` / `noPath` 由 `scan` 与 `drift` 分别落痕 |
| AC-3 两套解析器一套归属 | `scan/ts.rs`（tsconfig `paths`）、`scan/rust.rs`（四形态模块树 + 外部 crate 丢弃）；长尾一律 `unresolved`（D23） |
| AC-4 定级只看引入方 | `drift/mod.rs` 定级分支（`from.gate` 三档）；`typeOnly` 仅标注；目标未建模 → warning |
| AC-5 `occurrences` 聚合 | `drift/mod.rs` 按 `(from, to)` 分桶后单条成型（不按行铺开） |
| AC-6 豁免两层留痕 | 声明 > 配置 > 内联（D5）；`suppressed[]` 两来源（`config` / `inline`）携带 `via` / `reason` / `file` / `line`；配置非法三形态显式 `Err`（`load_config`） |
| AC-7 两模式一致与落盘面分离 | `check` 的 `mode` / `base` / `changed` 分流；`report.rs` 仅在 runner 内被调用（全量不落盘）；`noChanges` 判据（D10 / D22） |
| AC-8 `status` 五值可辨 | D10 推导序 + `ArchiCheckStatus::ALL`；`observations` 不进 `status`；`modelHash`（D24，`sha2`） |
| AC-9 看护配置面写死 | `drift/config.rs` + `foundation::layout::archi_config_path`（唯一字面量持有者）；工作区配置零改动 |
| AC-10 判定单源 | `drift::check` 唯一入口（D4）；命令面 `archi_drift_inner` 与 `LocalArchiCheck::run` 只做「取输入 → 调判定 → 投影 / 落盘」 |
| AC-11 只挂 implement | `IMPLEMENT_STEPS` 7 步 + `ARCHI_FEEDBACK_LIMIT`；`TEST_ONLY_TABLE` 零 `ArchiCheck`；零运行时开关 |
| AC-12 反馈边语义 | `step_archi_check`（fault 先行 / 计数 ≤3 / 超限升格 / `continue_session`）；固定处置提示与 `MAX_REPORT_CHARS`（D14）；16 份 prompt md 零改动 |
| AC-13 `skipped` / `noChanges` 不冒充通过 | runner `compose_text` 逐字文案 + walker `diagnose_brief` 直通（D15） |
| AC-14 runner 落盘与降级 | `infra/archi/src/report.rs::write_report` + runner 写盘失败降级 `report_ref = None`；报告槽位三分臂隔离 |
| AC-15 输入口径 | `steps.rs` 的 `change_edit_set` 注入失败即 `Err`；`check` 的模型与源码同根（D2，`root` 单参贯穿）；零工作区配置读取 |
| AC-16 步词汇 22 值全链路 | 四枚举尾部追加（§变更清单）+ `mapped_step` 臂 + 前端 `normalizeKind` / `stepWord` / `STEP_LABEL` 穷尽表 + golden 随动（测试相位） |
| AC-17 依赖与分层 | D24 + §6 依赖方向断言；`archi-runtime` 壳依赖带用途注释 |
| AC-18 成本与预算 | §6 四表（阈值 / 版本钉定 / 依赖成本四段式 / 运行期预算）+ 回退序；`Cargo.lock` 行由实现相位本地自证回填（998 → 1038，+40，落硬线内）；p95 行于 acceptance 前按 release 口径补跑回填（617.4 ms，软线内，`cargo test -p archi --release --test perf`）；其余四项按「未测（移交发布实测）」登记（NSIS / 主二进制 / CI 冷构建 / 本地增量）；两份表逐字一致（含未测文本一致） |
| AC-19 标记生产与呈现 | `drift-marks.ts` 纯函数（D20）+ `architecture-canvas.tsx` 单点接线（D21）；`marks.ts` 零改动 |
| AC-20 交付面 | `bindings:export` 再生成（机械）；`package.json` 0.6.5；`vp check` / `knip` / `cargo clippy -D warnings` |
| AC-21 模型与新增 crate 同 change 同步 | D25 + §持久化三：`03` 段追加元素与 3 条边（8 元素 / 24 关系）、`04` 段追加 1 条壳出边（13 关系）、全基线 50 / 111（后端 47 + 前端 64）；计数三处同值（模型表头 / spec 计数表 / ADR `§1`/`§4`/`§5`/`§6`/`§9`）；`specta-zod-patched` 单列为既有已知缺口、其余 15 个自研 crate 零差集；`gate` 全 `off` 不动；取证走 `archi_graph(root)`（零诊断零 `warnings`）+ `archi_module("App.Backend.Infra.ArchiRuntime")`（三出边 + Shell 入边）+ `grep` 计数 + 核对留痕 `reports/archi/model-sync-check.md` |

---

## 待决问题 (Open Questions)

1. **`oxc_*` 家族 6 件（+ `proc-macro2`）的版本与特性面**（**已关闭**）：落地值 `oxc_parser` / `oxc_allocator` / `oxc_ast` / `oxc_span` / `oxc_syntax` `=0.146.0`（家族同版；本仓工具链 `rustc 1.95.0` 可用的最新同版）、`oxc_resolver` `=11.24.3`（独立 11.x 版本线），另加 `proc-macro2`（`features = ["span-locations"]`，`syn` 的行号伴随件、锁内既有）；`[workspace.dependencies]` 单一收敛项、零分散版本号；`oxc_parser` `default-features = false` 即回退序 ① 已在钉版时执行。锁包数增量判读见 §6（998 → 1038，+40，落硬线内），无需触发回退序 ②。
2. **`model.warnings[]` 是否覆盖 `docNotFound`**：本期只产 `pathNotFound` / `noPath`（D12）；若评审要求与 spec 字段面注释逐字对齐，属纯增量（复用 `archi::docs::all_docs`，代价是门禁步多一次 md 读盘）。
3. **增量模式 `observations[]` 的噪声量级**：按 spec 与 R-6 的既定裁决照产（D13）。若首个真实工程反馈「未兑现」条目淹没报告，加「按元素对折叠 / 仅列前 N 条」属纯增量，不影响 `status` 与门禁语义。
4. **`archi_drift` 是否给 agent 一份只读工具**：本期明确不给（`desktop-archi-agent` 零改动）；若后续需要，注入矩阵与工具面另立变更。
