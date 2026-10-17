# 设计: check-before-flow-start

> **变更**: check-before-flow-start
> **日期**: 2026-10-16
> **基于**: proposal.md、explore.md（五轮勘测，决策台账 A–J 终版全部落定）、specs/ 十个能力 delta

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 十份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」并把 proposal / spec 遗留给 design 定稿的四个形态面收敛到常量面 / port 缝面 / walker 步体面 / 前端文案面：报告槽位透传形态（desktop-checks-domain）、`read_exec_output` 回查策略（desktop-test-execution）、`StaticCheckOutcome` 报告目录回填（desktop-change-orchestration 文案携目录）、initialize 人读呈现（desktop-change-flow-view）。

工作树实况（本变更起点，`packages/desktop` 0.5.12 基线）：`StepNode` / `RunStepKind` / `ChangeStepKind` 现为二十值全词汇；`REQUIREMENT_TABLE` 9 相位 / `TEST_ONLY_TABLE` 6 相位，两表均以 `proposal` 开头；`PhaseDefinition` 无回溯资格属性（`allowed_backtrack_phases` 为纯位置推导 `table[..=idx]`）；`feedback_limit`（walker.rs）把工具步 `OnFail::Terminal` 声明视为计划图漂移拒绝（verdict-gate 已有 Terminal 分支先例）；`ArchiveVcsPort` 无基线恢复面；依赖与工具产物树名单 `PRUNED_DIR_NAMES` 私有驻 `infra/checks/src/testexec/detect.rs`；报告树仅 `reports/test` 单变体（layout 单点派生，exec-outputs 同锚）；前端列头直渲染相位 id 原词（无 PHASE_LABEL 映射先例）。本设计描述**本变更完成后的目标形态**：两表 10 / 7 相位（表头一次性 `initialize`）、步词二十一值（+`clean`）、`reports/baseline` 第二报告树（槽位参数化）、`ArchiveVcsPort::restore_baseline` vcs 缝。零新表、零 db 迁移、零 IPC 命令签名变化、路由（`phase_next.rs`）零改动。

---

## 关键设计决策（proposal / spec 遗留形态面定稿）

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | 报告槽位透传形态 | **`foundation::layout::ReportSlot` 二值枚举**（`Test` 缺省 / `Baseline`）单点定义，`ToolCommand::StaticCheck` / `ToolCommand::TestExecution` 各增 `slot` 字段，`StaticCheckRunner` / `TestExecutionRunner` trait 签名各增 `slot` 参数，`steps.rs` 消费点透传 | exec / checks / orchestration 三层均已依赖 foundation——枚举驻 layout（与两目录变体同居）是唯一零新增 crate 依赖的落位；命令→runner 一条链同枚举，无第二份定义、无字符串槽位 | orchestration::port 自定义枚举（exec crate 不依赖 orchestration，需第二定义）；字符串槽位（封闭集失守） |
| D2 | baseline 目录派生 | layout 增槽位单点 `change_reports(root, change, slot)` / `change_reports_in(change_dir, slot)`；`change_test_reports(_in)` 既有签名零改动（改委托单点）；新增 `change_baseline_reports(_in)` 两命名变体（同委托） | 既有调用点（testexec / exec / 命令测试）零改动；`reports/test` 与 `reports/baseline` 后缀同源派生，runner 内零路径字面量 | 每 runner 私有后缀拼接（spec 明令禁止） |
| D3 | exec-outputs 回查策略 | **跨树查找**：`read_exec_output_in(change_dir, exec_id)` 在同一 change 目录内双锚探测（`reports/test/exec-outputs/` 先、`reports/baseline/exec-outputs/` 后），首中即返 | `exec_id` 进程内铸造全局唯一（`exec-<millis>-<seq>`），跨树无歧义；IPC `read_exec_output` 命令与前端 stdout 面板零改动（spec：回查 MUST 可达，策略 design 定稿） | 槽位参数穿透 IPC（节点 `output_ref` 仅 exec_id，前端无从携槽位——须改线面，违背零结构改动） |
| D4 | initialize 检查步 Terminal 分支形态 | `step_static_check` / `step_test_execution` 顶部增 `if step.on_fail == Some(OnFail::Terminal)` 前置分支，分派 `step_static_check_terminal` / `step_test_execution_terminal` 单发执行体（单次 spawn、无反馈边、失败即 `Terminal::failed`）；`feedback_limit` 本体零改动（缺省相位照旧拒绝无 FeedbackLoop 声明的工具步） | `step_verdict_gate` 的 Terminal 前置分支同式先例；重试基线无意义且 initialize 无 agent 会话可注入 | 改造 `feedback_limit` 认领 Terminal（语义混流——缺省相位的漂移拒绝面会被稀释） |
| D5 | legacy 判定与短路形态 | 判定 = `request.root == request.main_root`（D9 双 root 拆分推论：发起前置校验 3 保证 `record.worktree` 在场时 exec root ≠ 主 root，故相等 ⟺ `worktree=None`）；短路驻 `drive()` 相位主体前置分支——`PhaseStart`（开相 attempt，写面进程内直调）→ `PhaseLog` 以 `skipped=true` + 空 checklist + 跳过因落账，`Clean` / `StaticCheck` / `TestExecution` 三步零执行（零 vcs / 零检查 spawn、主仓零重置零 clean）；图上 `phase_start` / `phase_log` 两枚节点留痕；收口后停等照常（零特判豁免） | `phase_log` 写面前置要求 `active_phase` 匹配开相——skipped 落账必先 `phase_start`（两枚写面步非 spawn，与 spec「步序列零执行（零 vcs / 检查 spawn）」的限定语一致）；跳过因经 PhaseRecord.report + phase_log 节点 detail 双面可见 | run_step 各步内分散短路（PhaseLog 步写 `skipped=false` 空 checklist——违背「以 skipped 落账」语义）；RunRequest 增 worktree 字段（与 root 冗余，扩契约面无收益） |
| D6 | clean 排除面组装与 git 实现 | 排除面由 `step_clean` 调用方组装：`foundation::prune::PRUNED_DIR_NAMES` + `".*"`（dot 目录剪枝，gitignore 式模式）+ `foundation::layout::domain_dir_name()`（`openspec/` 工件树增补——驻调用方，不入名单本体）；port `restore_baseline(worktree, exclusions: &[&str])` 透传，infra 实现为 `git -C <worktree> reset --hard` → `git -C <worktree> clean -fd` + 逐项 `-e <pattern>`（**不带 `-x`**：尊重 .gitignore——依赖树常态已免疫，`-e` 双保险覆盖无 ignore 规则仓；`openspec/` 未跟踪面显式保护） | 名单单源两消费点（detect 剪枝 / clean 排除）共享；先 reset 后 clean = tracked 恢复 + untracked 清除的规范序；排除决策驻基线语义 owner（walker），infra 无状态 | infra 侧内置组装（openspec 增补属消费点语义但调用方失排除决策权）；带 `-x` 的 clean + 排除面（连 .gitignore 免疫层一起掀，风险面无谓扩大） |
| D7 | 失败文案与报告目录回填 | walker 词汇常量：**static 红** `基线静态检查未通过，请在基线上修复后重新发起（基线的债不属于本次变更）；报告目录 {dir}`；**test Error** `基线测试执行出错（环境 / 配置问题），请检查配置与环境后重新发起；报告目录 {dir}`；**test Fail** `基线测试未通过，请在基线上修复测试后重新发起；报告目录 {dir}`（三态不共用文案，均 `Terminal::failed`）；**legacy 跳过** `legacy 主仓执行无 worktree 基线，跳过基线检查`；`StaticCheckOutcome` 增 `report_dir: String` 字段（infra 按槽位派生回填，pass / fail 两分支均携——与 `TestExecutionOutcome.report_dir` 对称，static 文案的 `{dir}` 由此供给） | spec 要求三态文案可辨且携 baseline 报告目录；test 侧 `report_dir` 已在载荷，static 侧补齐即零推导 | walker 侧按 layout 自行派生目录（walker 无 name 解析面，须再穿 store——绕远） |
| D8 | initialize PhaseLog 形态 | **复用现三条目机械 checklist，零新代码**：initialize steps 含 TestExecution 步 → pass 后 `cursor.test_outcome` 自动挂载 → `step_phase_log` 三分支（test_outcome 在前）天然走机械 checklist 分支；mutation 恒 `None` → 第 3 条 evidence「null（未跑）」如实留痕；report 面 `outcome.report_dir` 自动指向 baseline 树（结论 + 计数 + 目录齐备）；三会话槽位恒 `None` | explore 决议 K：`test_execution_phase_log_input` 先例纯桌面代写，「或复用现三条目」为 spec 认可选项；零分支新增 = 零漂移面 | 新增 initialize 专用 checklist 文案（第四份落账形态，无增量信息） |
| D9 | initialize 人读呈现 | `STEP_LABEL` 补 `clean: '基线恢复'`（穷尽 Record 强制补键）；列头增 `PHASE_LABEL` **部分映射单键** `initialize: '基线检查'` + `phaseLabel()` 回落 raw id（其余相位沿用 id 原词呈现先例，不自造全相位标签新约定）；`groupOf` git 组补 `clean` | 前端列头现状即 raw id（proposal / archive 同式），仅 initialize 需人读标签（spec ADDED 文案要求）；最小白名单式扩展 | 全相位 PHASE_LABEL Record（二十余键新约定，超文案补充范围） |
| D10 | 回溯过滤实现 | `allowed_backtrack_phases` 增 `.filter(|phase| phase.backtrackable)`；`idx == 0` 早退保留（语义 = 表头自身无前序，非 initialize 排除机制——排除仅由表内显式属性承载）；两表既有 15 个相位条目全部补 `backtrackable: true`（机械随动） | spec 明令禁止「跳过 idx==0」位置推导作为排除机制；属性过滤沿 `prerequisites` 表内自描述先例 | 推导规则跳过表头（表结构变化即漂移——spec 点名否决） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 依赖与工具产物树名单单点（新） | 名单封闭集（`node_modules` / `dist` / `build` / `target` / `coverage` / `.nyc_output`）+ dot 剪枝判定纯函数，两消费点（testexec detect 剪枝 / vcs restore_baseline 排除面）同源；名单本体不含 `openspec/`（工件增补属消费点语义） | `packages/desktop/src-tauri/crates/core/foundation/src/prune.rs`（新；`lib.rs` 声明 `pub mod prune`） | 无（纯常量 + 纯函数） | Rust |
| 报告槽位与 baseline 目录变体 | `ReportSlot` 二值枚举 + `change_reports(_in)` 槽位单点 + `change_baseline_reports(_in)` 变体；既有 `change_test_reports(_in)` 改委托（签名零改动） | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 无 | Rust |
| 步词汇与分组单点 | `StepNode` / `RunStepKind` 二十值 → 二十一值（+`Clean`，snake 线词 `clean`，位序 MutationTest 后）；`StepNode::group()` 将 clean 派生 `StepGroup::Git`、`role()` 恒 `None`；`OnFail::Terminal` doc 补 initialize 检查步声明面 | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | model、serde、specta | Rust（中性状态类型） |
| 计划图单源 | `PhaseDefinition` 增 `backtrackable: bool`（表内自描述，沿 `prerequisites` 先例）；`INITIALIZE_STEPS` 静态五步 + `INITIALIZE_PHASE_ID` 常量；两表表头各插 `initialize` 表项（`executor=None` / `evaluator=None` / `prerequisites=[]` / `backtrackable=false` / 零 artifact_manifest）；`allowed_backtrack_phases` 按属性过滤；既有条目补 `backtrackable: true` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（+ `write/mod.rs` 再导出常量） | state（StepDefinition / StepNode / OnFail）、agent（ModelLevel） | Rust（sync，编译期 const） |
| 编排步词汇与映射 | `ChangeStepKind` +`Clean`（线词 `clean`，位序 MutationTest 后）；`mapped_step` 补 `ChangeStepKind::Clean → RunStepKind::Clean`（二十一值 1:1 零过滤） | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` + `run_history.rs` | serde、specta、workflow::state | Rust |
| 检查域 port 缝与槽位透传 | `ToolCommand::StaticCheck` / `TestExecution` 增 `slot: ReportSlot` 字段；`StaticCheckRunner` / `TestExecutionRunner` 签名增 slot；`StaticCheckOutcome` 增 `report_dir`；`ArchiveVcsPort` 增 `restore_baseline(worktree, exclusions)` sync 缝；`LocalToolSteps::execute` 两分支透传 slot（id → name 解析面不变） | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` + `steps.rs` | foundation::layout（ReportSlot）、std path | Rust（port 定义驻消费者） |
| static-check 槽位化 | `execute(root, name, slot)`：报告锚点 / 输出记录按槽位派生，`report_dir` 回填产出；执行语义（无配置直过 / 程序不可达显式 Err / 退出码映射）零分叉 | `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` | orchestration::port、exec、foundation、config | Rust、tokio async |
| test-execution 槽位化与剪枝单源 | `execute(root, change, slot)`：`reports_dir` 按槽位派生（复用门读自本槽位树 summary——跨树零复用天然成立；mtime 判定零改动）；`detect.rs` 剪枝改引 foundation 名单（私有 `PRUNED_DIR_NAMES` 退役；复用门扫描的 `EXCLUDED_DIR_NAMES` 为 CLI `REUSE_SCAN_EXCLUDED_DIRS` 同源独立集，不入本次上移面） | `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` + `detect.rs` | orchestration::port、checks、exec、foundation、config | Rust、tokio async |
| exec 输出记录槽位与回查 | `record_exec_output(root, change, slot, output)` 槽位锚定；`read_exec_output_in` 同 change 目录双锚探测（test 先 / baseline 后，exec_id 跨树唯一首中即返）——IPC 与前端零改动 | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` | foundation::layout | Rust |
| vcs 基线恢复缝 | `git::restore_baseline(worktree, exclusions)`：`reset --hard` → `clean -fd` + 逐项 `-e`（不带 `-x`）；`ProcessArchiveVcs` 委托实现（`Err` 带 git 语境与 worktree 路径） | `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` + `lib.rs` | exec、foundation、orchestration::port | Rust（sync，spawn 驻此） |
| walker initialize 执行体 | `run_step` 增 `Clean` 路由臂 + `step_clean`（排除面组装 + vcs 直调 + 主仓双保险）；`step_static_check` / `step_test_execution` Terminal 前置分支 + 两单发执行体（Error / Fail / static 红三态分文案，携 baseline 报告目录）；`drive()` legacy 短路分支（PhaseStart + skipped PhaseLog）；PhaseLog 机械 checklist 零改动复用（D8） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | port、state、foundation（prune / layout）、control | Rust、tokio async |
| 词面注释随动 | `RunNodeRecord.step` doc 二十一值（表结构零变化零迁移）；detail 节点投影 doc 二十一值 + pipeline「10 列 / 7 列」；归档停止点 doc 计数随动 | `crates/infra/store/src/model.rs` + `store.rs`、`crates/core/workflow/src/queries/detail.rs`、`src/commands/change_flow/mod.rs` | workflow::state、specta | Rust |
| bindings 再生 | `RunStepKind` / `ChangeStepKind` TS 词面 + `"clean"`（specta 自动跟随，非手改） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | TypeScript（生成物） |
| 前端词归一与人读标签 | `normalizeKind` / `stepWord` 词汇注释 20 → 21（clean 为 identity 词走 `default` 分支，零显式 case）；`groupOf` git 组补 `clean`；`STEP_LABEL` 增 clean 词条；列头 `PHASE_LABEL` 单键 + `phaseLabel()` 回落 | `packages/desktop/src/views/changes/flow/run-state.ts` + `run-step-node.tsx` + `phase-column-node.tsx` | dto（bindings 类型） | TypeScript、React |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/foundation/src/prune.rs` | 依赖与工具产物树名单共享单点：`PRUNED_DIR_NAMES` 封闭集 + `is_pruned_dir_name` dot 剪枝判定纯函数（公共项见「公共函数 / API」「类型定义」表；配套 `prune_test.rs` 归测试相位） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/foundation/src/lib.rs` | 增 `pub mod prune;` 模块声明（lib doc「刻意极小」注记补第三能力——第二消费者出现才下沉的正触发已成立） | 名单模块出口 |
| `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 增 `BASELINE_REPORTS_DIR_NAME: &str = "baseline"` 私有常量与 `pub enum ReportSlot { Test, Baseline }`；增 `change_reports(root, change, slot)` / `change_reports_in(change_dir, slot)` 槽位单点（后缀名经 `report_dir_name(slot)` 单点映射）；增 `change_baseline_reports` / `change_baseline_reports_in` 两命名变体；`change_test_reports(_in)` 改为委托槽位单点（签名与返回值零改动） | 两报告树同源派生单点；既有调用面零感知 |
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `StepNode` 增 `Clean` 变体（位序 `MutationTest` 后，doc「initialize 基线恢复步（vcs.restore_baseline）」）；`RunStepKind` 增 `Clean`（同位序同 doc 口径）；`StepNode::as_str()` / `RunStepKind::as_str()` 各补 `Clean => "clean"` 臂；`StepNode::group()` Git 臂补 Clean（`Commit \| Rebase \| Merge \| Cleanup \| Finalize \| Clean → Git`）；`StepNode::role()` None 臂补 Clean；`StepGroup::Git` doc 步族清单补 clean；`OnFail::Terminal` doc 补 initialize 检查步声明面（code-analyze VerdictGate 之外的第二消费点）；两处「二十值」doc → 「二十一值」 | 计划图 + 执行图词汇单点，白名单式扩展 20 → 21 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | `PhaseDefinition` 增 `pub backtrackable: bool` 字段（doc：表内自描述回溯资格，`allowed_backtrack_phases` 唯一过滤依据——`backtrackable=false` 者不得出现在任何相位的回溯白名单）；新增 `pub const INITIALIZE_PHASE_ID: &str = "initialize"` 与静态 `INITIALIZE_STEPS`（五步：`PhaseStart` / `Clean` 均 `on_fail=None`、`StaticCheck` / `TestExecution` 均 `on_fail=Some(OnFail::Terminal)`、`PhaseLog` `on_fail=None`，全 `prompt_ref=None`）；`REQUIREMENT_TABLE` / `TEST_ONLY_TABLE` 各在表头插 `initialize` 表项（`description: "基线检查"`、`steps: INITIALIZE_STEPS`、`artifact_manifest: &[]`、`prerequisites: &[]`、`executor: None`、`evaluator: None`、`backtrackable: false`）；两表既有 15 个条目全部补 `backtrackable: true`；`allowed_backtrack_phases` 迭代增 `.filter(\|phase\| phase.backtrackable)`（`idx == 0` 早退保留——自身无前序语义，非排除机制）；表侧 doc「9 相位 / 6 相位」→「10 相位 / 7 相位」并注记 initialize 表头一次性语义 | 表头一次性相位单源；两表共用同一 static；路由（phase_next）零改动天然含表头 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | `pub use phase_table::{…}` 再导出清单增 `INITIALIZE_PHASE_ID` | walker 短路判定消费出口 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ChangeStepKind` 增 `Clean` 变体（位序 `MutationTest` 后，doc「initialize 基线恢复步（vcs）」）；`as_str()` 补 `Clean => "clean"` 臂；枚举 doc 词汇计数随动 | 编排侧词面与 `RunStepKind` 同词面 1:1 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | `mapped_step` 补 `ChangeStepKind::Clean => RunStepKind::Clean` 臂（穷尽 match 编译期强制）；doc「二十值 1:1」→「二十一值」 | 落库词面直映射零过滤 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ToolCommand::StaticCheck` / `ToolCommand::TestExecution` 各增 `slot: foundation::layout::ReportSlot` 载荷字段（doc：报告槽位——test 缺省 / baseline 基线检查；`MutationTest` 不增——基线检查明确不含 mutation）；`StaticCheckOutcome` 增 `pub report_dir: String` 字段（doc：报告目录按槽位派生，pass / fail 均携——baseline Terminal 文案供给面）；`StaticCheckRunner::run` / `TestExecutionRunner::run` 签名各增 `slot: ReportSlot` 第三参；`ArchiveVcsPort` 增 `restore_baseline` 方法（签名见公共函数表；doc：`reset --hard` + `clean -fd` 排除面；worktree 在场判定由调用方保证，port 零探测方法；主仓 MUST NOT 传入） | 槽位链路与 vcs 缝扩展（spawn 驻 infra） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `execute` 的 `StaticCheck` / `TestExecution` 两分支：命令解构增 `slot` 并透传 `static_check.run(&step.root, &name, slot)` / `test_execution.run(&step.root, &name, slot)`（id → 记录 → name 解析面与审计行零改动） | 槽位消费点透传（LocalToolSteps 组合不变） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | ① 词汇常量：`TXT_SKIP_LEGACY_INITIALIZE`（legacy 跳过因）与 baseline 三态失败文案常量（D7，含 `{dir}` 报告目录占位）；② `run_step` 增 `StepNode::Clean` 路由臂（initialize 步族区段，`step_clean(...).await?; Ok(StepFlow::Proceed)`）；③ 新增 `step_clean` 执行体：主仓双保险（`ctx.request.root == ctx.request.main_root` → `passed` + legacy 跳过因，正常不可达——相位级短路先行）→ 排除面组装（D6：名单 + `".*"` + `domain_dir_name()`）→ `ctx.vcs.restore_baseline(Path::new(&ctx.request.root), &exclusions)`：`Ok(summary)` → `passed` + detail 摘要；`Err` → `failed` 节点 + `Terminal::failed`（携 git 语境）；④ `step_static_check` / `step_test_execution` 顶部各增 `OnFail::Terminal` 前置分支，分派 `step_static_check_terminal` / `step_test_execution_terminal` 单发执行体（单节点 running → spawn（`ToolCommand` 携 `slot: ReportSlot::Baseline`）→ pass / fail 收口；static fail 与 test Error / Fail 三态分文案 `Terminal::failed`，节点 failed 携 `output_ref`；test pass 挂 `cursor.test_outcome` 后 Proceed）；⑤ `drive()` 相位主体前置 legacy 短路分支（`phase == INITIALIZE_PHASE_ID && request.root == request.main_root` → `step_phase_start` 开相 + PhaseLog 节点 emit + `run_phase_log` 以 `skipped=true` + 空 checklist + 跳过因落账，三检查步零执行；停等块照常覆盖）；⑥ `INITIALIZE_PHASE_ID` import 与「相位内计划步执行体」section 注释随动（initialize 五步序 + Terminal 例外注记） | initialize 执行主体；一次性语义由既有落账机制天然成立（Terminal 失败零 PhaseRecord 条目 → 再发起照常路由 initialize；pass 持久跳过）；机械 checklist 零改动复用（D8） |
| `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` | `StaticCheckRunner for ProcessStaticCheck` 与私有 `execute` 签名增 `slot: ReportSlot`；报告目录按槽位经 layout 派生并回填 `StaticCheckOutcome.report_dir`（pass / fail / 无配置直过三分支均携——无配置分支 report_dir 亦回填，语义为「本槽位报告树锚点」）；`exec::record_exec_output` 调用增 slot 透传；执行语义零分叉 | static-check 槽位化；baseline 输出记录同锚 baseline 树 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | `TestExecutionRunner for ProcessTestExecution` 与私有 `execute` 签名增 `slot`；`reports_dir` 经 `foundation::layout::change_reports(root_path, change, slot)` 派生（复用门 `read_existing_summary` / `prune_roots` 消费本槽位 dir——跨树零复用与 baseline 树内 mtime 复用天然成立，判定逻辑零改动）；`outcome` 内 `record_exec_output` 增 slot 透传；聚合 / 诊断 / 框架矩阵零改动 | test-execution 槽位化；缺省槽位路径与本变更前逐字节一致 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs` | 私有 `PRUNED_DIR_NAMES` 常量退役；`is_pruned_path` 改引 `foundation::prune::is_pruned_dir_name`（组件名判定委托单点；`is_pruned_path` 自身保留——路径组件迭代语义驻消费点） | 两消费点同源单名单 |
| `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` | `record_exec_output` 签名增 `slot: ReportSlot`（第三参）；`exec_output_dir(root, change)` / `exec_output_dir_in(change_dir)` 改为槽位参数版并经 `foundation::layout::change_reports_in(change_dir, slot)` 派生；`read_exec_output_in` 改双锚探测（test 槽先 / baseline 槽后 `find_map`，exec_id 跨树唯一首中即返，miss 仍 `None`）；`read_exec_output` 顶层函数签名零改动（委托 `read_in`，天然覆盖两树） | 输出记录槽位锚定 + 回查跨树可达（D3）；生产调用点（static_check / testexec）随签名随动 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` | 新增 `pub(crate) fn restore_baseline(worktree: &Path, exclusions: &[&str]) -> Result<String, String>`：`git(worktree, ["reset", "--hard"])` → `git(worktree, ["clean", "-fd", "-e", p1, …, "-e", pn])`（逐项排除模式；不带 `-x`——尊重 .gitignore）；成功返回恢复摘要串，失败 `Err` 沿既有 `git()` 语境（命令串 + 退出码 + stderr） | 基线恢复 spawn 单点（`GIT_EDITOR` / stdio 惯例复用既有 `git()` 助手） |
| `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | `impl ArchiveVcsPort for ProcessArchiveVcs` 增 `restore_baseline`：委托 `git::restore_baseline`，`Err` 经 `map_err` 附 worktree 路径与「主仓未受影响」注记 | vcs 缝实现；spawn 驻 infra（core 零 spawn 红线） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `RunNodeRecord.step` 字段 doc「二十值」→「二十一值」（表结构零变化零迁移——core 域枚举直用，serde 线词扩展） | 词面注释随动 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | run 节点读面 doc「二十值全词汇零过滤」→「二十一值」 | 词面注释随动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 节点行线面 doc「二十值」→「二十一值」（纯 derive 零字段属性——词汇扩展自动跟随）；pipeline 注释「requirement 9 列 / test-only 6 列」→「10 列 / 7 列（initialize 首列）」（出线随 `phase_table` 自动跟随，零代码改动） | 词面与列数注释随动 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `STOP_BEFORE_ARCHIVE` doc「run 走完 8 个路由相位后必停等确认」→「run 走完全部前序路由相位（requirement 9 个 = initialize + 8、test-only 6 个 = initialize + 5）后必停等确认」 | 停等计数注释随动（机制零改动——stop_before 恒 "archive"） |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成：`RunStepKind` 与 `ChangeStepKind` TS 联合类型各 + `"clean"`（非手改，`bindings:check` 守卫拦截漂移） | 前端类型跟随 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | `normalizeKind` / `stepWord` doc 词汇注释「二十值」→「二十一值」（clean 为 identity 词——snake / camel 同形，走 `default` 分支直返，与 commit / rebase / merge / finalize / preflight / seal / cleanup 先例一致，零显式 case）；`groupOf` git 组判定补 `step === 'clean'` | 前端词归一与 git 分类单点 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `STEP_LABEL` 增 `clean: '基线恢复'`（`Record<ChangeStepKind, string>` 封闭联合强制补键）；基线输出回查经既有 `outputRef` stdout 面板直达（零渲染改动） | clean 节点人读标签 |
| `packages/desktop/src/views/changes/flow/phase-column-node.tsx` | 增私有 `PHASE_LABEL` 部分映射（单键 `initialize: '基线检查'`）与 `phaseLabel(phase: string): string` 回落函数（未命中返回 raw id——既有相位 id 原词呈现先例保持）；列头文本改经 `phaseLabel(data.phase)` | initialize 相位列头人读标签（D9，零结构改动） |
| `packages/desktop/package.json` | `version` 0.5.12 → 0.5.13 | 版本交付（`tauri.conf.json` 经 `../package.json` 引用自动跟随） |

> **零改动沿用**：`phase_next.rs` 路由（表驱动「首个未 pass 相位」天然含表头）、`phase_start.rs` / `phase_log.rs` / `backtrack.rs` 写面前置与落账语义、`MutationRunner` / mutation 执行链（基线检查明确不含 mutation——`INITIALIZE_STEPS` 无 MutationTest 步）、既有内容相位全部 `*_STEPS` 与 `prompts/*.md`、`ChangeRecord` / db 表结构（词汇扩展零迁移）、`StepRecord` 审计面（static_check / test_execution 分支既有，baseline 槽位步照常落审计行）、`LocalToolSteps` 组合面与命令层装配（`change_flow_start_with` 签名与四道前置校验零改动——exec root 解析既有语义即 initialize 执行锚）、前端转换层 / 抽屉 / stdout 面板 / 终态 reason 呈现、`openspec/explores/` 与 CLI 工作流、`plugins/dev-team`（保持 2.10.44）。测试文件（`phase_table_test.rs` / walker 测试 / vcs git 测试 / checks / exec / `mod_test.rs` / `run-step-node.test.tsx` / corpus golden 与 `tests/golden/`——`DESKTOP_GOLDEN_REWRITE=1` 显式重写）由测试相位承接。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `PRUNED_DIR_NAMES` | `packages/desktop/src-tauri/crates/core/foundation/src/prune.rs` | 新增（pub const） | `pub const PRUNED_DIR_NAMES: [&str; 6]` | 依赖与工具产物树目录名封闭集（`node_modules` / `dist` / `build` / `target` / `coverage` / `.nyc_output`）；detect 剪枝与 restore_baseline 排除面两消费点同源；名单扩展一处修改双点生效 |
| `is_pruned_dir_name` | `packages/desktop/src-tauri/crates/core/foundation/src/prune.rs` | 新增 | `pub fn is_pruned_dir_name(name: &str) -> bool` | dot 前缀或名单命中（剪枝判定纯函数；路径组件迭代语义驻各消费点） |
| `ReportSlot`（枚举值 `Test` / `Baseline`） | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | `pub enum ReportSlot { Test, Baseline }`（derive `Debug, Clone, Copy, PartialEq, Eq`；零 serde / specta——纯进程内枚举，不进 IPC） | 报告槽位封闭二值；`Test` 为缺省（既有 `reports/test` 语义零改动），`Baseline` 为 initialize 基线检查（`reports/baseline`） |
| `change_reports` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn change_reports(root: &Path, change: &str, slot: ReportSlot) -> PathBuf` | 槽位驱动报告目录单点（`reports/<test\|baseline>` 后缀经 `report_dir_name(slot)` 单点映射）；两命名变体委托本函数 |
| `change_reports_in` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn change_reports_in(change_dir: &Path, slot: ReportSlot) -> PathBuf` | change 目录变体（exec-outputs 锚定消费） |
| `change_baseline_reports` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn change_baseline_reports(root: &Path, change: &str) -> PathBuf` | baseline 变体命名出口（= `change_reports(.., Baseline)`） |
| `change_baseline_reports_in` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn change_baseline_reports_in(change_dir: &Path) -> PathBuf` | baseline 变体命名出口（change 目录形态） |
| `change_test_reports` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 修改（签名不变） | `pub fn change_test_reports(root: &Path, change: &str) -> PathBuf` | 改委托 `change_reports(.., Test)`（既有调用点零改动、路径逐字节一致） |
| `change_test_reports_in` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 修改（签名不变） | `pub fn change_test_reports_in(change_dir: &Path) -> PathBuf` | 同上（change 目录形态） |
| `INITIALIZE_PHASE_ID` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（经 `write/mod.rs` 再导出） | 新增（pub const） | `pub const INITIALIZE_PHASE_ID: &str = "initialize"` | initialize 相位名单点（表单源同驻）；walker legacy 短路判定消费 |
| `allowed_backtrack_phases` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | 修改（签名不变，行为随属性） | `pub fn allowed_backtrack_phases(table: &[PhaseDefinition], current_phase: &str) -> Vec<String>` | 迭代增 `.filter(\|phase\| phase.backtrackable)`——initialize 恒不入任何相位的回溯白名单；无位置推导排除 |
| `phase_table` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | 修改（签名不变，行为随表） | `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` | requirement → 10 相位（initialize 表头）、test-only → 7 相位（initialize 表头），其余 `None` 不退化 |
| `ToolCommand::StaticCheck` / `ToolCommand::TestExecution`（变体形态） | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 修改 | `StaticCheck { change_id: String, slot: ReportSlot }` / `TestExecution { change_id: String, slot: ReportSlot }` | 槽位随命令载荷流入（walker 构造点：缺省相位传 `Test`、initialize 两 terminal 执行体传 `Baseline`） |
| `StaticCheckRunner.run` | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 修改（trait 方法签名） | `fn run(&self, root: &str, name: &str, slot: ReportSlot) -> BoxToolFuture` | 槽位透传；唯一生产实现 `ProcessStaticCheck` |
| `TestExecutionRunner.run` | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 修改（trait 方法签名） | `fn run(&self, root: &str, name: &str, slot: ReportSlot) -> BoxToolFuture` | 槽位透传；唯一生产实现 `ProcessTestExecution` |
| `ProcessStaticCheck.run` / `ProcessTestExecution.run` | `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` / `testexec/mod.rs` | 修改（impl 方法签名） | 同 trait 签名 | 报告锚点 / 输出记录按槽位派生；执行语义零分叉 |
| `ArchiveVcsPort.restore_baseline` | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 新增（trait 方法） | `fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String>` | `reset --hard` + `clean -fd` 逐项 `-e` 排除；返回恢复摘要（步 detail 消费）；`Err` 带 git 语境；worktree 在场判定驻调用方（walker），port 零探测方法；唯一生产实现 `ProcessArchiveVcs` |
| `ProcessArchiveVcs.restore_baseline` | `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | 新增（impl 方法） | `fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String>` | 委托 `git::restore_baseline` 单点；`Err` 经 `map_err` 附 worktree 路径与主仓未受影响注记 |
| `record_exec_output` | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` | 修改（签名增 slot） | `pub fn record_exec_output(root: &Path, change: &str, slot: ReportSlot, output: &[u8]) -> Result<String, String>` | 输出记录按槽位锚定（`<report_dir>/exec-outputs/<exec_id>.log`）；exec_id 铸造与路径校验零改动 |
| `read_exec_output_in` | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` | 修改（签名不变，行为双锚） | `pub fn read_exec_output_in(change_dir: &Path, exec_id: &str) -> Option<Vec<u8>>` | 同 change 目录双锚探测（test 先 / baseline 后）；exec_id 跨树唯一，首中即返、miss 仍 `None` |
| `StepNode.as_str` / `StepNode.group` / `StepNode.role` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` / `pub fn group(self) -> StepGroup` / `pub fn role(self) -> Option<AgentRole>` | 各补 Clean 臂（`clean` / Git / None） |
| `RunStepKind.as_str` / `ChangeStepKind.as_str` | `…/core/workflow/src/state.rs` / `…/core/orchestration/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` | 各补 `Clean => "clean"` 臂 |
| `change_flow_start`（IPC） | `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 不变 | `#[tauri::command] pub async fn change_flow_start(app: AppHandle, on_event: Channel<RunNotice>, root: String, id: String, auto_next_phase: bool) -> Result<ChangeRunSummary, String>` | 前置校验与装配零改动（initialize 挂相位表头，发起面零感知） |
| `read_exec_output`（IPC） | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 不变 | `#[tauri::command] pub fn read_exec_output(stores: State<'_, WorkspaceStores>, root: String, change_name: String, exec_id: String) -> Option<String>` | 回查经 `read_exec_output_in` 双锚探测自动覆盖 baseline 树（D3，零线面改动） |

> `INITIALIZE_STEPS` 为 `phase_table.rs` 私有 static（两表共用，同 `PROPOSAL_STEPS` 先例，非模块级导出）；`step_clean` / `step_static_check_terminal` / `step_test_execution_terminal` / legacy 短路落账助手为 `walker.rs` 私有 async 执行体；`git::restore_baseline` 为 `pub(crate)`；`mapped_step`（run_history.rs）为私有映射函数——均不入本表。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `ReportSlot` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` | 新增 | 报告槽位封闭二值枚举（`Test` / `Baseline`；纯进程内，零 serde / specta） |
| `PhaseDefinition` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | 修改 | 增 `backtrackable: bool` 字段（表内自描述回溯资格；initialize 恒 false，其余 15 条目 true）；其余字段零改动 |
| `StepNode` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | 二十值 → 二十一值（+`Clean`，位序 MutationTest 后；线词 `clean`；归 `StepGroup::Git`、`role = None`） |
| `RunStepKind` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | 二十值 → 二十一值（+`Clean`，与 `StepNode` 同词面 1:1；`RunNodeRecord.step` 落库词汇白名单式扩展，零迁移） |
| `ChangeStepKind` | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | 修改 | 二十值 → 二十一值（+`Clean`；serde camelCase 线词 `clean` 与 `as_str` snake 线词 `clean` 同形） |
| `ToolCommand` | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 修改 | `StaticCheck` / `TestExecution` 变体各增 `slot: ReportSlot` 载荷字段（封闭集本体不变——零新变体） |
| `StaticCheckOutcome` | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 修改 | 增 `report_dir: String` 字段（infra 按槽位派生回填，pass / fail / 无配置三分支均携；baseline Terminal 文案供给面，与 `TestExecutionOutcome.report_dir` 对称） |

> `StepGroup` / `AgentRole` / `OnFail` / `StepDefinition` / `ArtifactSpec` / `RunUpdate` / `RunNodeRecord` / `PhaseLogInput` 零改动（clean 为 git 步无角色无 prompt；on_fail 复用既有 `OnFail::Terminal` 变体；落库载体词面扩展自动跟随）。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.5.13` | 0.5.12 → 0.5.13（`tauri.conf.json` 经 `../package.json` 引用自动跟随；`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 零改动仍 2.10.44） |

> 排除面不进配置：`PRUNED_DIR_NAMES` 驻 foundation 源码常量（单点扩展经独立小变更通道），`".*"` / `openspec/` 增补驻 `step_clean` 源码；`config.json` 与 config schema 零新增键。

---

## 数据模型

### 计划图（源码，编译期静态）

`INITIALIZE_STEPS` 声明五步（requirement / test-only 两表表头共用同一 static）：

```
PhaseStart      exec  on_fail=None   （attempt max+1 分配，复用）
Clean           git   on_fail=None   （vcs.restore_baseline；失败即 Terminal）
StaticCheck     exec  on_fail=Terminal（单发，失败即停——无反馈边、零 agent 注入）
TestExecution   exec  on_fail=Terminal（单发；Error / Fail 分文案）
PhaseLog        exec  on_fail=None   （机械 checklist 复用现三条目，D8）
```

两表相位序（10 / 7 路由相位）：

```
requirement: initialize → proposal → dev-design → test-design → implement
             → test-gen → test-execution → code-review → acceptance → archive
test-only:   initialize → proposal → code-analyze → test-design → test-gen
             → test-execution → archive
```

`prerequisites`：两表 `initialize=[]`、`proposal=[]`（其余条目零改动）——initialize 无依赖入边 ⇒ stale 传播（`dependents` BFS）零波及；无人依赖 initialize ⇒ 无下游。`backtrackable`：`initialize=false`、其余全部 `true`——`allowed_backtrack_phases` 按属性过滤，initialize 恒不入回溯白名单（基线检查不可回溯重做）。`executor` / `evaluator` / `artifact_manifest`：initialize 三者均空（确定性相位；基线报告树非文档素材，回查经节点 `output_ref`）。

### 报告树（change 目录内，两槽位分树隔离）

```
openspec/changes/<name>/
  reports/test/       ← 缺省槽位：implement / test-gen 门禁 + test-execution 相位
      exec-outputs/<exec_id>.log
      summary.json + 逐 suite 子报告
  reports/baseline/   ← baseline 槽位：initialize 相位两检查步
      exec-outputs/<exec_id>.log
      summary.json + 逐 suite 子报告
```

两树零混居（不以 exec_id / 时间戳同目录区分）；目录派生经 foundation layout 槽位单点；复用门按树各自判定（baseline 树 summary 可在输入未变时复用；clean 触碰输入 mtime 推进自动失效；run 内相位步不得复用 baseline 树 summary——reports_dir 槽位隔离天然成立，判定逻辑零改动）。exec_id 进程内铸造跨树唯一，`read_exec_output_in` 双锚探测回查可达。

### 执行图与状态载体（db / 进程内）

| 模型 | 字段 / 变更 | 持久化 | 关系 / 说明 |
|------|------------|--------|-------------|
| `RunNodeRecord` | `step: RunStepKind` 词汇 + `clean`（白名单式 20→21） | native_db（表结构零变化、零迁移——serde 线词扩展，旧库可读） | initialize 五步节点（phase_start / clean / static_check / test_execution / phase_log）零过滤落库，串行 forward 边入 `RunEdgeRecord`（边表零变化） |
| `PhaseRecord` | 零结构变化 | native_db | initialize pass 条目 = 机械 checklist（`skipped=false`、verdict=pass、三会话槽位 None）；legacy 条目 = `skipped=true` + 跳过因 report（一次性持久同 pass 语义——`has_phase_passed` 谓词两者皆认）；Terminal 失败零条目（图节点照常留痕可回查） |
| `StepRecord`（审计面） | 零变化 | native_db | baseline 槽位 static_check / test_execution 步照常落审计行（reference 携 baseline report_dir） |
| `ChangeRecord` / `RunRecord` / `RunEdgeRecord` / `PhaseCursor` | 零变化 | native_db / 进程内 | initialize 复用既有落账与跨步产物通道（`cursor.test_outcome` 挂载 → PhaseLog 机械分支）；无新跨步产物字段 |

### initialize 相位状态机

```
legacy（request.root == request.main_root）
  → PhaseStart（开相）+ PhaseLog（skipped=true，跳过因「legacy 主仓执行无 worktree 基线」）
    Clean / StaticCheck / TestExecution 零执行（零 vcs / 检查 spawn、主仓零重置）
    收口后停等照常；后续相位照常路由（一次性持久）
非 legacy：
  Clean             restore Ok  → passed（detail 恢复摘要）
                    restore Err → failed 节点 + Terminal（run failed；PhaseRecord 零条目）
  StaticCheck       passed      → Proceed（output_ref 回填）
                    failed      → failed 节点 + Terminal（static 红文案 + baseline 报告目录）
  TestExecution     pass        → cursor.test_outcome 挂载 → Proceed
                    Error       → failed 节点 + Terminal（环境文案）
                    Fail        → failed 节点 + Terminal（测试红文案）
  PhaseLog          机械 checklist 三条目（mutation 恒 None →「null（未跑）」）；
                    report 携 conclusion + 计数 + baseline 报告目录；三会话槽位 None
  收口              手动档照常 ConfirmWait（零豁免）；auto 照常推进；
                    pass 落 PhaseRecord → 后续 run 持久跳过（再发起零重跑）
失败重试            Terminal 失败 → run failed + PhaseRecord 零 initialize 条目
                    → 再发起 phase_next 仍路由 initialize → Clean 清残留 → 干净重试
```

Clean 排除面：`PRUNED_DIR_NAMES`（node_modules / dist / build / target / coverage / .nyc_output）+ dot 目录（`.*` 模式）+ `openspec/` 工件树（`domain_dir_name()`）——tracked 修改被 reset、其余 untracked 清除、排除面原样保留（bootstrap 依赖与过程文档不丢）。

<!-- 路由 / API 设计：不涉及——本变更零新增 / 修改 HTTP 端点与 IPC 命令签名（`change_flow_start` / `read_exec_output` 等透传既有 `Result<T, String>` / `Option<String>` 面；`ToolCommand` 为进程内枚举非线面）。 -->

---

## 依赖

### 运行时依赖

零新增。既有依赖复用：`foundation::layout`（槽位与目录变体、`domain_dir_name`）、`foundation::prune`（名单单点，新模块零外部依赖）、`orchestration::port::ArchiveVcsPort`（restore_baseline 缝）、`crates/infra/vcs` git 子命令族（既有 `git()` spawn 助手）、`crates/infra/exec`（输出记录单点）、specta / serde（词汇类型派生）。

### 构建 / 测试依赖

零新增。bindings 再生走既有 `pnpm -C packages/desktop run bindings:export` 管线（`bindings:check` 守卫拦截漂移）；corpus golden 显式重写走既有 `DESKTOP_GOLDEN_REWRITE=1` 流程（测试相位承接）。

---

## 待决问题

无——explore 决策台账 A–J 已全部落定，本设计把四个遗留形态面（槽位透传 / 回查策略 / static 报告目录回填 / initialize 人读呈现）定稿如上（D1–D3 / D7 / D9）。`git clean -e ".*"` 的 dot 剪枝模式覆盖面（Windows 路径语义 / 嵌套 dot 目录）由 `restore_baseline` 真件测试护栏验证（测试相位），实现面无未决项。
