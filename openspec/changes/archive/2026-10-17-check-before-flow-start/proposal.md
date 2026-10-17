# 提案: check-before-flow-start

> **变更**: check-before-flow-start
> **日期**: 2026-10-16
> **状态**: draft

---

## 问题

桌面端变更流程（change flow run）启动前缺少基线绿门：worktree 建域自主仓 HEAD（基线恒 HEAD），若主仓基线本身是红的（lint 挂 / 测试挂），首个 run 的 implement 相位 static-check 步会把基线的债甩给 executor 会话修——烧光 FeedbackLoop 预算（5 次）还可能修出与本次变更无关的改动。现状 `change_flow_start_with` 的 4 道前置校验全部是毫秒级元数据校验（已建档 / workflow_type 命中 / exec root 存在 / 无并行 run），没有任何对基线质量的检查。

用户需求：变更流程启动前，先在基线上执行 static-check 与 test-execution（明确**不含 mutation**——mutation 用 `--baseline skip` 且贵，基线检查排除它合理），任一不通过则停止流程，并提示用户在基线上修复后重新发起——把「基线的债」与「本次变更的债」在发起前分家。

## 提案

explore 五轮勘测后设计完整收敛（决策台账 A–J 全部落定，见 `explore.md` 第四 / 五轮）。核心形态：**两表表头一次性 `initialize` 相位**，复用既有相位机全链机制，零新路由规则：

1. **表头一次性相位 `initialize`**：`REQUIREMENT_TABLE` / `TEST_ONLY_TABLE` 各在表头插入 `INITIALIZE` 相位（确定性相位：`executor=None` / `evaluator=None`），步序 `PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog`。`phase_next` 表驱动天然路由（首个未 pass 相位）——路由零改动；pass 落 PhaseRecord 持久 → 后续 run 永久跳过（一次性门）；任一步失败以 `Terminal` 收敛 run failed 且不落 eval 条目 → 再发起照常路由 initialize、clean 清残留后干净重试。
2. **clean 步恢复基线**：两检查前先恢复 worktree 至基线态（`git reset --hard` + `git clean -fd`，排除面 = 依赖与工具产物树名单 + dot 目录 + `openspec/` 工件树——裸 clean 会删掉 bootstrap 装好的 node_modules / target，基线测试直接 Error）。新增 `ArchiveVcsPort::restore_baseline(worktree, 排除面)` 缝（walker 直调 vcs 先例同 `step_commit`；实现驻 `crates/infra/vcs`，core 零 spawn 红线不变）。**legacy 红线**：`worktree=None` 的存量记录整相位短路（步序列零执行、零 spawn、主仓零重置，skipped 落账 + 跳过因——restore 在主仓执行会 reset 用户未提交改动，灾难）。
3. **两门禁步失败即 Terminal**：initialize 相位的 StaticCheck / TestExecution 步声明 `on_fail=Terminal`（重试基线无意义且无 agent 会话可注入——不走 FeedbackLoop）；walker 工具步执行体需加 `OnFail::Terminal` 分支（现状 `feedback_limit` 把工具步 Terminal 声明视为计划图漂移拒绝；verdict-gate 已有 Terminal 分支先例）。失败文案区分：test-execution `conclusion=Error`（环境 / 配置问题）引导查配置环境、`conclusion=Fail`（测试红）引导修基线测试；static-check 失败引导修基线。检查在 worktree（exec_root）内执行——主仓未提交改动不进 worktree，基线语义纯净。
4. **报告分离**：基线检查报告落 `reports/baseline` 子树（与 run 内 `reports/test` 零混居；不做复用红利——run 内 test-execution 相位步不复用 baseline summary）。infra/checks 两 runner 报告锚点参数化（test 缺省 / baseline 变体），foundation layout 单点派生两变体；exec-outputs 与 `read_exec_output` 回查面同锚覆盖。
5. **回溯排除 + 停等一致 + 机械 checklist**：`PhaseDefinition` 增 `backtrackable: bool` 表内自描述属性（沿 `prerequisites` 先例；initialize=false），`allowed_backtrack_phases` 按其过滤——后续相位决策 backtrack 候选不含 initialize（基线检查不可回溯重做）；stale 传播走 `prerequisites`，initialize 无依赖入边零波及。initialize 收口后停等与全相位一致（手动档照常 ConfirmWait，零特判）。PhaseLog 走机械 checklist 分支（零 agent——steps 含 TestExecution 步则 `test_outcome` 自动挂载，`step_phase_log` 三分支天然分派，沿 `test_execution_phase_log_input` 纯桌面代写先例），报告目录指向 baseline 树。
6. **词汇与呈现连坐**：`StepNode::Clean` 入步词汇封闭集（二十值 → 二十一值白名单式扩展；`group()` 归 Git、`role()` 为 None，穷尽 match 编译期强制补），`RunStepKind` / `ChangeStepKind` 同词面 1:1（`clean` 线词），bindings 再生成，前端 `STEP_LABEL`（穷尽 Record）+ `normalizeKind` / `stepWord` 映射 + git 分类徽章补词。前端**零结构改动**：图节点 / outputRef stdout 面板 / 终态 reason 全现成，仅文案补充（clean 步标签 + initialize 相位名列头）。
7. **名单共享单源**：`PRUNED_DIR_NAMES` + dot 剪枝语义自 `infra/checks/src/testexec/detect.rs`（私有）上移 foundation 公开共享——detect 剪枝与 vcs restore_baseline 排除面两消费点同源，无第二份名单。

## 能力

全部为既有能力修改（无新增能力；新关注点用 ADDED、基线冲突面用 MODIFIED）：

| 能力 | delta | 内容 |
|------|-------|------|
| desktop-workflow-write-face | 3 MODIFIED | 表头 initialize 相位（requirement 9→10 / test-only 6→7 路由相位）；`PhaseDefinition.backtrackable` + `INITIALIZE_STEPS` + `StepNode::Clean`（二十→二十一值）；test-only 前置依赖表加 initialize 行 |
| desktop-change-orchestration | 1 ADDED + 5 MODIFIED | ADDED：initialize 相位执行体（clean / Terminal / Error-Fail 分文案 / 报告分离 / 一次性 / 停等 / legacy 短路）；MODIFIED：archive 相位序号（第 10→11 路由相位）、停等计数（8 个路由相位→前序路由相位含 initialize）、Git 节点与步词汇封闭集、vcs spawn 家族（+restore_baseline）、static-check 门禁相位限定（+initialize 例外） |
| desktop-test-execution | 1 ADDED | baseline 报告槽位与树隔离（报告分离、复用门按树隔离、零复用红利、回查覆盖） |
| desktop-checks-domain | 2 ADDED | 两 runner 报告锚点参数化（baseline 变体经 layout 单点）；依赖与工具产物树名单上移 foundation 共享单源 |
| desktop-change-flow-view | 1 ADDED + 4 MODIFIED | ADDED：initialize 相位列与 clean 步词呈现（零结构改动 + 文案）；MODIFIED：列数（9/6→10/7 列）×2、词汇归一（二十→二十一值）、auto 停等计数 |
| desktop-change-queries | 1 MODIFIED | detail pipeline 出线列数（10/7）与 stepKind 词汇（二十一值含 clean） |
| desktop-change-state-store | 1 MODIFIED | `RunNodeRecord.step_kind` 封闭集二十一值（clean 入列） |
| desktop-change-archive | 3 MODIFIED | archive 相位序号（第 11 路由相位）与停等计数（前序路由相位含 initialize）×3 处 |
| desktop-change-worktree | 1 MODIFIED | stop-before-archive scenario 计数随动 |
| desktop-corpus-regression | 1 MODIFIED | 语料词汇（二十一类）+ initialize 相位样本 |

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `PhaseDefinition` 增 `backtrackable: bool`；`INITIALIZE_STEPS` 静态声明 + 两表表头 `INITIALIZE` 表项；`allowed_backtrack_phases` 按 `backtrackable` 过滤
- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — `StepNode::Clean` / `RunStepKind::Clean`（`group()` 归 Git、`role()` None；穷尽 match 编译期强制补）
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — `ArchiveVcsPort::restore_baseline(worktree, 排除面)` 新缝方法
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — initialize 相位执行体：`step_clean`（vcs 直调 + legacy 短路先例）、工具步（static-check / test-execution）`OnFail::Terminal` 专用分支（现 `feedback_limit` 拒绝工具步 Terminal）、PhaseLog 机械 checklist 沿 `test_execution_phase_log_input`
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — `ChangeStepKind::Clean`
- `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` — ToolStep 面随动（报告槽位透传，如需）
- `packages/desktop/src-tauri/crates/core/foundation/src/` — 依赖与工具产物树名单单点（新，公开共享）+ `layout` baseline 报告目录变体（`change_baseline_reports`）
- `packages/desktop/src-tauri/crates/infra/vcs/` — `restore_baseline` 实现（`reset --hard` + `clean -fd` 排除面；worktree 在场判定由 walker 保证）
- `packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs` — 剪枝引 foundation 名单（私有名单退役）
- `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs`、`static_check.rs` — 报告锚点参数化（baseline 变体）
- `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` — exec-outputs 锚点随槽位派生；`read_exec_output` 回查覆盖 baseline 树
- `packages/desktop/src/types/generated/bindings.ts` — export-bindings 再生成（ChangeStepKind / RunStepKind 新变体）
- `packages/desktop/src/views/changes/flow/run-step-node.tsx` — `STEP_LABEL` 补 `clean`（穷尽 Record）+ git 分类徽章；`run-state.ts` — `normalizeKind` / `stepWord` 补 `clean` 线词
- `packages/desktop/package.json` — version 0.5.12 → 0.5.13（tauri.conf.json 经引用自动跟随）

### 测试文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs` — 全表对照（10 / 7 相位、INITIALIZE_STEPS、backtrackable 过滤、prerequisites）
- `packages/desktop/src-tauri/crates/core/orchestration` walker 测试 — initialize 五步序、Terminal 收敛（Error / Fail 分文案）、机械 checklist 落账、legacy 短路、一次性 pass 跳过（walker_test Terminal / emit 断言先例）
- `packages/desktop/src-tauri/crates/infra/vcs` — `restore_baseline` 测试（排除面保留 / tracked 重置 / untracked 清除）
- `packages/desktop/src-tauri/crates/infra/checks`、`infra/exec` — 槽位参数化与回查测试
- `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` — 真件 spawn 先例（`write_static_shim` + `openspec/config.json` 登记 + `prefix_path` PATH 注入）直用：基线红停 / 基线绿走
- `packages/desktop/src/views/changes/flow/run-step-node.test.tsx` — `STEP_LABEL` 穷举（二十一词）
- `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/golden/` — initialize 相位样本入语料，diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写

### 不要修改

- `plugins/dev-team`（版本保持 2.10.44；插件仓已弃用，零改动）
- 突变面（`MutationRunner` / Stryker / cargo-mutants 执行链——基线检查明确不含 mutation）
- 既有内容相位的步序列与 prompt（proposal→…→acceptance 各 `*_STEPS` 与 `prompts/*.md` 零改动）
- `openspec/explores/`、CLI 工作流与 workflow.json（双向墙）
- `phase_next.rs` 路由逻辑（表驱动天然含表头，零改动）、`ChangeRecord` / db 表结构（词汇扩展零迁移）

## 验收标准

1. 新建 change 发起 run：initialize 相位先于 proposal 执行（图上 initialize 列 Clean / StaticCheck / TestExecution / PhaseLog 节点齐备），检查在 worktree（exec_root）内执行，报告与 exec-outputs 落 `openspec/changes/<name>/reports/baseline/`。
2. 基线 static-check 失败（真件 shim 配置为红）→ run 以 Terminal 收敛 failed，文案引导在基线修复后重新发起；PhaseRecord 零 initialize 条目；再发起 → clean 恢复基线后干净重试。
3. 基线 test-execution `conclusion=Fail` → 停止 + 测试红文案（引导修基线测试）；`conclusion=Error` → 停止 + 环境文案（引导查配置 / 环境）——两态文案可辨。
4. initialize 全绿 → PhaseLog 机械 checklist 落账 pass（零 agent、三会话槽位恒 None）；手动档停等确认照常；再发起（或 auto 推进）不再重跑 initialize（PhaseRecord 持久跳过）。
5. clean 幂等安全：依赖与工具产物树（node_modules / dist / build / target / coverage / .nyc_output）与 dot 目录、`openspec/` 工件树保留；tracked 修改被 reset、其余 untracked 清除。
6. legacy（`worktree=None`）change：initialize 整相位短路——零 spawn、主仓零重置，跳过因可见，后续相位照常。
7. `allowed_backtrack_phases` 不含 initialize（任一后续相位的回溯候选白名单排除表头）；stale 传播零波及。
8. 前端零结构改动：initialize 相位列（requirement 10 列 / test-only 7 列）与 clean 步词标签（git 分类）呈现，基线报告经节点 outputRef stdout 面板回查；全管线绿（static-check + 前后端测试 + bindings 一致性守卫）。

## 风险

1. **clean 误删面（最高）**：排除面漏项（如新依赖管理器的产物树名不在名单）→ 依赖被删 → 基线测试 Error 且 bootstrap 需重跑。缓解：foundation 单源名单 + restore_baseline 测试覆盖既有管理器；名单扩展为独立小变更通道。
2. **reset 丢弃未提交编辑**：initialize 失败后的残留、以及存量 change 再发起时已 pass 相位的未提交代码会被重置（PhaseRecord 显示 pass 而代码丢失的语义断裂）。决议 I：当前单用户、无存量包袱，接受零兼容特判——多用户 / 多人协作场景需另立变更（该取舍已在 spec 留痕）。
3. **legacy 主仓红线**：restore_baseline 若误在主仓执行 = reset 用户未提交改动（灾难）。缓解：clean 步仅 worktree 在场时执行 + legacy 整相位短路 + 双测试护栏。
4. **全量基线测试时长**：分钟级测试进入 run 序曲（invoke 已提前 resolve 不悬挂，节点可观测），auto 模式下用户感知「发起后长时间停在 initialize」；属预期成本，节点 running 态可视。
5. **报告树分裂的回查面**：`read_exec_output` 需覆盖两树（槽位参数 vs 跨树查找、exec_id 跨树唯一性）——形态由 design 定稿，spec 仅约束回查可达。
6. **词汇连坐面广**：StepNode / RunStepKind / ChangeStepKind / bindings / 前端穷尽 Record / corpus golden 全链随动（20→21）——编译期穷尽 match 与穷举测试保障，golden 重写面大需人工确认。
