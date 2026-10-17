# 任务: check-before-flow-start

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接（含 `phase_table_test.rs` 全表对照、orchestration walker 测试（initialize 五步序 / Terminal 三态分文案 / legacy 短路 / 一次性跳过）、`infra/vcs` `restore_baseline` 真件测试、`infra/checks` / `infra/exec` 槽位与回查测试、`src/commands/change_flow/mod_test.rs` 真件 spawn 基线红停 / 基线绿走、前端 `run-step-node.test.tsx` 穷举、`corpus_golden_test.rs` + `tests/golden/` 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写），本列表只覆盖实现文件。
>
> 依赖序要点：foundation 单点（阶段一）被 layout 消费者与 vcs / walker 排除面依赖，先行；core/workflow 词汇（阶段二）先于 phase_table 表项与 orchestration 词面（阶段三）；port 缝槽位字段与 runner 签名（阶段四）连坐 `steps.rs` 透传与 infra 实现（阶段五）——三者须同批落地编译才绿；vcs 缝（阶段五）先于 walker `step_clean`（阶段六）；bindings 再生（阶段八）依赖 Rust 词面定稿（阶段二 / 三）并先于前端随动（阶段九，`STEP_LABEL: Record<ChangeStepKind, string>` 与 `groupOf` 消费 bindings 新联合类型）。

## 阶段一：foundation 单点（名单 + 报告槽位）

- [x] 新增 `packages/desktop/src-tauri/crates/core/foundation/src/prune.rs`：`pub const PRUNED_DIR_NAMES: [&str; 6]`（`node_modules` / `dist` / `build` / `target` / `coverage` / `.nyc_output`）+ `pub fn is_pruned_dir_name(name: &str) -> bool`（dot 前缀或名单命中）；名单本体不含 `openspec/`（工件增补属消费点语义）
- [x] 修改 `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs`：增 `pub mod prune;` 声明，lib doc「刻意极小」注记补名单能力（第二消费者出现才下沉的正触发已成立）
- [x] 修改 `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs`：增 `BASELINE_REPORTS_DIR_NAME` 私有常量、`pub enum ReportSlot { Test, Baseline }`（derive `Debug, Clone, Copy, PartialEq, Eq`）、`report_dir_name(slot)` 私有映射与 `pub fn change_reports(root, change, slot)` / `pub fn change_reports_in(change_dir, slot)` 槽位单点；增 `change_baseline_reports` / `change_baseline_reports_in` 两命名变体（委托单点）；`change_test_reports(_in)` 改委托 `change_reports(.., Test)`（签名与返回值零改动）

## 阶段二：core/workflow 词汇与相位表

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/state.rs`：`StepNode` 增 `Clean` 变体（位序 `MutationTest` 后，doc「initialize 基线恢复步（vcs.restore_baseline）」）；`RunStepKind` 增 `Clean`（同位序）；`StepNode::as_str()` / `RunStepKind::as_str()` 各补 `Clean => "clean"` 臂；`StepNode::group()` Git 臂补 Clean（Commit / Rebase / Merge / Cleanup / Finalize / Clean → Git）；`StepNode::role()` None 臂补 Clean；`StepGroup::Git` doc 步族清单补 clean；`OnFail::Terminal` doc 补 initialize 检查步声明面；两处「二十值」doc 改「二十一值」
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`：`PhaseDefinition` 增 `pub backtrackable: bool` 字段（doc：表内自描述回溯资格，`allowed_backtrack_phases` 唯一过滤依据）；新增 `pub const INITIALIZE_PHASE_ID: &str = "initialize"` 与静态 `INITIALIZE_STEPS`（五步：PhaseStart / Clean `on_fail=None`、StaticCheck / TestExecution `on_fail=Some(OnFail::Terminal)`、PhaseLog `on_fail=None`，全 `prompt_ref=None`）；`REQUIREMENT_TABLE` / `TEST_ONLY_TABLE` 各在表头插 initialize 表项（description「基线检查」、steps=INITIALIZE_STEPS、artifact_manifest 空、prerequisites 空、executor / evaluator None、backtrackable=false）；两表既有 15 条目全部补 `backtrackable: true`；`allowed_backtrack_phases` 迭代增 `.filter(|phase| phase.backtrackable)`（idx==0 早退保留）；表 doc「9 相位 / 6 相位」改「10 相位 / 7 相位」并注记表头一次性语义
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`：`pub use phase_table::{…}` 再导出清单增 `INITIALIZE_PHASE_ID`

## 阶段三：orchestration 词面与映射

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`：`ChangeStepKind` 增 `Clean` 变体（位序 `MutationTest` 后，doc「initialize 基线恢复步（vcs）」）；`as_str()` 补 `Clean => "clean"` 臂；枚举 doc 词汇计数随动
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs`：`mapped_step` 补 `ChangeStepKind::Clean => RunStepKind::Clean` 臂；doc「二十值 1:1」改「二十一值」

## 阶段四：port 缝槽位与 vcs 缝（连坐 steps.rs 透传）

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`：`ToolCommand::StaticCheck` / `ToolCommand::TestExecution` 各增 `slot: foundation::layout::ReportSlot` 载荷字段（doc：报告槽位 test 缺省 / baseline 基线检查；`MutationTest` 不增——基线检查明确不含 mutation）；`StaticCheckOutcome` 增 `pub report_dir: String` 字段（doc：按槽位派生，pass / fail / 无配置均携——baseline Terminal 文案供给面）；`StaticCheckRunner` / `TestExecutionRunner` trait `run` 签名各增 `slot: ReportSlot` 参数（doc 槽位语义与单点派生约束）；`ArchiveVcsPort` 增 `fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String>`（doc：`reset --hard` + `clean -fd` 逐项 `-e`；返回恢复摘要；`Err` 带 git 语境；worktree 在场判定驻调用方，port 零探测方法；主仓 MUST NOT 传入）
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`：`execute` 的 StaticCheck / TestExecution 两分支解构 `slot` 并透传 `static_check.run(&step.root, &name, slot)` / `test_execution.run(&step.root, &name, slot)`（id → 记录 → name 解析与审计行零改动）

## 阶段五：infra 实现（checks / exec / vcs）

- [x] 修改 `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs`：`StaticCheckRunner for ProcessStaticCheck` 与私有 `execute` 签名增 `slot: ReportSlot`；报告目录按槽位经 layout 派生并回填 `StaticCheckOutcome.report_dir`（pass / fail / 无配置直过三分支均携）；`exec::record_exec_output` 调用增 slot 透传；执行语义（无配置直过 / 程序不可达显式 Err / 退出码映射）零分叉
- [x] 修改 `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs`：`TestExecutionRunner for ProcessTestExecution` 与私有 `execute` 签名增 `slot`；`reports_dir` 经 `foundation::layout::change_reports(root_path, change, slot)` 派生（复用门 `read_existing_summary` / `load_disk_sub_reports` / `prune_roots` 消费本槽位 dir——跨树零复用与 baseline 树内 mtime 复用天然成立，判定逻辑零改动）；`outcome` 内 `record_exec_output` 增 slot 透传；聚合 / 诊断 / 框架矩阵零改动
- [x] 修改 `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs`：私有 `PRUNED_DIR_NAMES` 常量退役；`is_pruned_path` 组件判定改引 `foundation::prune::is_pruned_dir_name`（路径组件迭代语义驻消费点；`EXCLUDED_DIR_NAMES` 为 CLI 复用门扫描独立集，保持不动）
- [x] 修改 `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs`：`record_exec_output` 签名增 `slot: ReportSlot`（第三参）；`exec_output_dir` / `exec_output_dir_in` 改槽位参数版并经 `foundation::layout::change_reports_in(change_dir, slot)` 派生；`read_exec_output_in` 改同 change 目录双锚探测（test 槽先 / baseline 槽后 `find_map`，首中即返、miss 仍 `None`）；`read_exec_output` 顶层签名零改动（委托 `read_in`，天然覆盖两树）
- [x] 修改 `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs`：新增 `pub(crate) fn restore_baseline(worktree: &Path, exclusions: &[&str]) -> Result<String, String>`——既有 `git()` 助手执行 `reset --hard` → `clean -fd` + 逐项 `-e <pattern>`（不带 `-x`，尊重 .gitignore）；成功返回恢复摘要串
- [x] 修改 `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`：`impl ArchiveVcsPort for ProcessArchiveVcs` 增 `restore_baseline` 委托 `git::restore_baseline`，`Err` 经 `map_err` 附 worktree 路径与「主仓未受影响」注记

## 阶段六：walker initialize 执行体

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：新增词汇常量——`TXT_SKIP_LEGACY_INITIALIZE = "legacy 主仓执行无 worktree 基线，跳过基线检查"` 与 baseline 三态失败文案（static 红 / test Error / test Fail，均携 baseline 报告目录占位 `{dir}`，三态不共用文案）
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：`run_step` 增 `StepNode::Clean` 路由臂（initialize 步族区段，`step_clean(ctx, trace, cursor).await?; Ok(StepFlow::Proceed)`）；新增 `step_clean` 执行体——主仓双保险（`ctx.request.root == ctx.request.main_root` → `passed` + legacy 跳过因，正常不可达）→ 排除面组装（`foundation::prune::PRUNED_DIR_NAMES` + `".*"` dot 模式 + `domain_dir_name()`）→ `ctx.vcs.restore_baseline(Path::new(&ctx.request.root), &exclusions)`：`Ok` → `passed` + detail 摘要；`Err` → failed 节点 + `Terminal::failed`（携 git 语境）
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：`step_static_check` / `step_test_execution` 顶部各增 `if step.on_fail == Some(OnFail::Terminal)` 前置分支（verdict_gate 同式先例），分派新增单发执行体 `step_static_check_terminal` / `step_test_execution_terminal`——单节点 begin + running emit → `run_tool` 携 `slot: ReportSlot::Baseline` → pass：`passed` + detail（output_ref 回填；test pass 另挂 `cursor.test_outcome` 后 Proceed）；fail：failed 节点（detail = diagnose_brief、output_ref 回填）+ `Err(Terminal::failed(…))` 三态分文案（static 红 / Error 环境 / Fail 测试红，携 baseline 报告目录——static 取 `outcome.report_dir`、test 取 `outcome.report_dir`）；`feedback_limit` 本体零改动
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：`drive()` 相位主体前增 legacy 短路分支——`phase == INITIALIZE_PHASE_ID && request.root == request.main_root` 时：`step_phase_start` 开相（emit phase_start 节点）→ PhaseLog 节点 begin + running emit → `run_phase_log` 以 `skipped=true` + 空 checklist + `TXT_SKIP_LEGACY_INITIALIZE` 落账（emit passed）；Clean / StaticCheck / TestExecution 三步零执行（零 vcs / 零检查 spawn、主仓零重置）；跳过 steps 迭代后落 `trace.failed_phase` 与停等块照常（零特判豁免）
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：注释随动——「相位内计划步执行体」section 补 initialize 五步序与 Terminal 例外注记（两检查步 `on_fail=Terminal` 无反馈边）；PhaseLog 机械 checklist 零改动（initialize steps 含 TestExecution 步 → `cursor.test_outcome` 自动挂载 → 三分支天然走机械分支，report 面 `outcome.report_dir` 自动指向 baseline 树）

## 阶段七：词面注释与计数随动（机械）

- [x] 修改 `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：`RunNodeRecord.step` 字段 doc「二十值」→「二十一值」（表结构零变化零迁移）；`packages/desktop/src-tauri/crates/infra/store/src/store.rs` run 节点读面 doc 同步「二十一值」
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：节点行线面 doc「二十值」→「二十一值」；pipeline 注释「requirement 9 列 / test-only 6 列」→「10 列 / 7 列（initialize 首列）」（纯 derive 零代码改动——出线随 phase_table 自动跟随）
- [x] 修改 `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：`STOP_BEFORE_ARCHIVE` doc「run 走完 8 个路由相位后必停等确认」→「run 走完全部前序路由相位（requirement 9 个 = initialize + 8、test-only 6 个 = initialize + 5）后必停等确认」（机制零改动——stop_before 恒 "archive"）

## 阶段八：bindings 再生（前端随动的前置）

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`（`RunStepKind` / `ChangeStepKind` TS 联合类型各 + `"clean"`；非手改）——先于阶段九（前端 `STEP_LABEL` 封闭 Record 与 `groupOf` 消费新联合类型）

## 阶段九：前端随动

- [x] 修改 `packages/desktop/src/views/changes/flow/run-state.ts`：`normalizeKind` / `stepWord` doc 词汇注释「二十值」→「二十一值」（clean 为 identity 词走 `default` 分支直返——commit / rebase / merge / finalize / preflight / seal / cleanup 先例，零显式 case）；`groupOf` git 组判定补 `step === 'clean'`
- [x] 修改 `packages/desktop/src/views/changes/flow/run-step-node.tsx`：`STEP_LABEL` 增 `clean: '基线恢复'`（`Record<ChangeStepKind, string>` 封闭联合强制补键）；基线输出回查经既有 `outputRef` stdout 面板直达（零渲染改动）
- [x] 修改 `packages/desktop/src/views/changes/flow/phase-column-node.tsx`：增私有 `PHASE_LABEL` 部分映射（单键 `initialize: '基线检查'`）与 `phaseLabel(phase: string): string` 回落函数（未命中返回 raw id）；列头文本改经 `phaseLabel(data.phase)`（其余相位 id 原词呈现先例保持，零结构改动）

## 阶段十：版本交付与静态自查

- [x] 修改 `packages/desktop/package.json`：`version` 0.5.12 → 0.5.13（`tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 不随动；`plugins/dev-team` 零改动仍 2.10.44）
- [x] 静态自查：`phase_next.rs` 路由逻辑零改动（表驱动天然含 initialize 表头）；mutation 面（`MutationRunner` / mutation 执行链）零改动（`INITIALIZE_STEPS` 无 MutationTest 步、`ToolCommand::MutationTest` 无 slot）；既有内容相位 `*_STEPS` 与 `prompts/*.md` 零改动；`ChangeRecord` / db 表结构零迁移（`RunNodeRecord.step` 仅词面注释随动）；缺省槽位路径与变更前逐字节一致（`change_test_reports` 委托不改返回值）；实现面 grep「二十值」零残留（测试文件除外，归测试相位）；`plugins/dev-team` 全部零改动
