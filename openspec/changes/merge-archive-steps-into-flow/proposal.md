# 提案: merge-archive-steps-into-flow

> **变更**: merge-archive-steps-into-flow
> **日期**: 2026-10
> **状态**: draft
> **探索**: `openspec/changes/merge-archive-steps-into-flow/explore.md`（八轮探索已收敛：归档作为第 10 相位并入 phase → step 模型、复用 RunNodeRecord、空 checklist 落账、vcs 独立执行缝）

---

## 问题

`packages/desktop` 的 change 详情页目前并存两套执行模型：

1. **run 流程**已完成「phase → step」抽象——9 站流水线，每站下挂 step 序列（`RunStepKind` 十值全词汇），节点 + 边落库（`RunNodeRecord` / `RunEdgeRecord`），流程图常驻渲染。
2. **归档流程**仍是独立的六段阶段机（`ArchiveStage`：preflight / commit / merge / specSync / seal / finalize），进程内注册表、**不落库**，UI 是独立面板的六段清单（`ArchivePanel` + `StageList`），不在流程图中。

用户方向三句话：① 将归档步骤也并入 phase → step 模型；② 保持独立手动执行；③ UI 层也合入流程图中。两套模型的四类语义裂缝（explore 第一轮）是并入必须正面处理的根因：

- **状态词汇不对齐**：归档段有 `skipped`（五类跳过因），run step 是 `stopped`；归档 `skipped` 是幂等重试语义，run step 无对应词。
- **身份键不对齐**：run 节点有 `(phase, attempt, step, instanceNo)` 四段稳定键；归档链一次性、无 attempt / instanceNo。
- **持久化不对齐**：run 节点落库常驻；归档不落库，收口后 `ArchiveControl` 除名、六段进度蒸发。
- **执行器不对齐**：归档六段只有 specSync 与 merge 冲突解算跑 agent 会话，其余四段是纯 Rust 内联（git / 写面 / 落盘），类似但不同于 run 的 ToolStep。

explore 八轮已把目标模型收敛为「归档是 walker 第 10 相位的普通 step 序列」，逐轮用户定稿如下：

- **A（第二轮）**：归档进第 10 列（`PIPELINE_PHASES` 追加 `archive`，后端 `detail.rs` + 前端 `layout.ts` 双侧同步）。
- **B（第二轮定方向，第三轮修正）**：扩展 `RunStepKind`（不另立 `ArchiveStepKind`）；第三轮把归档从「第三面命令组」降为「walker 第 10 相位的 step 序列」。
- **C（第二轮）**：复用 `RunNodeRecord`（不新增表）；第三轮确认归档节点即当前 run 节点，C2/C3/C4 裂缝自然消解。
- **决策①（第三轮）**：归档视为普通 step，`auto_next_phase` 补充「按指定 phase 停止」能力。
- **决策②（第三轮）**：跳过语义不扩 `RunStepStatus`——「直接跳过 step（计划层不进序列）」不落节点；「step 内判断（执行了发现无事可做）」保留记录，`status=passed` + `detail` 携跳过因。
- **N1（第四轮）**：归档进 `REQUIREMENT_TABLE`（第 9 个路由相位），是 test-execution 之后第二个「确定性相位」。
- **A（第五轮）**：移除 drive 的 `evaluator.is_none()` 冗余提前检查（真正消费点 `step_evaluator` / `step_executor` 已有各自 None 检查兜底）。
- **B（第五轮）**：走 B2——walker 新增独立 vcs 执行缝（`Arc<dyn ArchiveVcsPort>`），rebase 与冲突解算拆两步（冲突解算按条件执行）。
- **C（第五轮）**：归档无 evaluator、无 checklist。
- **①（第六轮）**：`ff_advance` 走放法 3——拆三段 Rebase / ResolveConflicts / Merge。
- **②（第六轮）**：形态 X——归档走 PhaseStart，与 8 站同构。
- **③（第六轮，关键纠正）**：归档尽量不增加 phase 和 step 的特殊场景；「不需要 checklist」的正确落账形态是「空 checklist 落账」（`derive_verdict([])=Pass` 是写面既有合法输入），而非 `phase_next` 特判 status。
- **第七轮**：复用 `run_phase_log` + 空 checklist 载荷，不新增 `archive_phase_log_input` 落账函数。
- **第八轮**：连 summary 也不需要——归档落账 = `PhaseLogInput { phase: "archive", report: "", checklist: vec![], skipped: false, 三槽位 None }`；`archive_summary` 跨步产物字段随之删除，`PhaseCursor` 只新增 `rebase_conflicts` 一个跨步产物。

## 提案

把归档从「独立六段阶段机 + 独立面板」重构为「walker 第 10 相位（archive）的 10 步 step 序列」，落进现有 run 执行图与流程图的同一套词汇、身份键、持久化与渲染路径，并保持「手动执行」语义：

1. **归档成为第 10 相位**：`REQUIREMENT_TABLE` 追加 `archive` `PhaseDefinition`（`executor=None`、`evaluator=None`），`PIPELINE_PHASES` 追加第 10 列 `archive`（后端 `queries/detail.rs` 与前端 `flow/layout.ts` 双侧同步）。归档相位为确定性相位（第二例，先例 test-execution）：无 executor / evaluator 会话，末尾 PhaseLog 以空 checklist 落 `verdict=pass` 的 PhaseRecord。
2. **10 步 step 序列**（形态 X + 放法 3）：`PhaseStart`（复用）→ `Preflight`（新，只读校验 + vcs 探测）→ `Commit`（新，vcs.commit_all，step 内「干净」跳过）→ `Rebase`（新，vcs.rebase_branch，产出冲突清单入 `PhaseCursor.rebase_conflicts`）→ `ResolveConflicts`（新，条件执行：无冲突 passed+detail；有冲突 agent 解算）→ `Merge`（新，vcs.ff_merge 主仓快进）→ `SpecSync`（新，agent 会话）→ `Seal`（新，workflow::write::archive 双写收口）→ `Finalize`（新，vcs.commit_paths，step 内「已落盘」跳过）→ `PhaseLog`（复用，空 checklist + 空 report 落账）。
3. **词汇扩展**：`StepNode` / `RunStepKind` / `ChangeStepKind` 三个同词面枚举各加 8 词（`preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `seal` / `finalize`）；`StepGroup` 三分类扩为四分类（Exec / Agent / Gate / **Git**），git 步（commit / rebase / merge / finalize）归 Git、内联步（preflight / seal）归 Exec、agent 步（spec_sync / resolve_conflicts）归 Agent。
4. **walker 第三执行缝（vcs）**：`StepContext` 新增 `Arc<dyn ArchiveVcsPort>` 独立 vcs 缝；git 步在 `run_step` 直调 vcs（进程 spawn 仍驻 `crates/infra/vcs`，core 零 spawn 红线不变）；Preflight 的 vcs 探测（current_branch / branch_merged）同走 vcs 缝。
5. **移除冗余检查**：drive 主循环删除 `next.evaluator.is_none()` 硬检查；「steps 声明 Evaluator/VerdictGate 步就必须有 evaluator 定义」约束下沉到 `step_evaluator` / `step_executor` 既有 None 检查；test-execution 的 dummy evaluator 可顺带清理（独立取舍）。
6. **空 checklist 落账**：`step_phase_log` 在 test_outcome 分支之后、verdict 分支之前插入 archive 分支——构造空 report + 空 checklist 的 `PhaseLogInput` 直接 `run_phase_log`；零 `if phase == archive` 特判，三分流按「产物在否」天然分离。
7. **手动执行语义（stop-before-archive）**：`auto_next_phase` 泛化出「按指定 phase 停止」能力，archive 为固定停止点——run 走完 8 个路由相位后停在 archive 前（"Ready for archiving" 语义改写为「等待确认进入归档」），用户显式确认后才进入 archive 相位；归档相位失败 = 步失败即 `Terminal`（`on_fail=None`），直接 `Failed` 收口、不进决策分叉（与现状 archive_flow 停链一致）。
8. **持久化零新表**：归档节点/边走当前 run 的 `RunNodeRecord` / `RunEdgeRecord` 整包落库（`run_id` 天然在案，attempt 由 PhaseStart max+1 分配，六段串行 forward 边复用 `EdgeKind::Forward`）；归档完成后详情页 refresh 仍可见归档列全史（常驻渲染）。
9. **前端合入流程图**：`ArchivePanel` / `StageList` / `useArchiveFlow` / `archive-state.ts` 六段清单退役；归档六段（实为 10 步）作为 `archive` 列的 step 节点上图（WorkerAgent / ToolStep / Gate / Git 类型可辨）；前端 `RunStepGroup` + `GROUP_LABEL` 同步加 git 分类；确认对话由 stop-before-archive 停等面与 Preflight step detail 承载。
10. **命令面收编**：`archive_flow` 五命令（preflight / start / stop / state / watch）+ `ArchiveControl` + `ArchiveSink` 整体退役，归档的发起 / 停止 / 状态 / 通知收编进 `change_flow` 命令面与 `ChangeFlowControl`；写面 `archive` 双写本体零改动复用（Seal step 消费）。
11. **版本交付**：`packages/desktop` 0.4.36 → 0.4.37；`plugins/dev-team` 零改动。

## 关键决策

explore 八轮逐轮定稿，每项决策均列出被拒绝的备选方案与拒绝原因（决策代号见 `explore.md` 对应轮次）：

| 问题 | 决策 | 理由 | 被拒绝的备选方案 |
|------|------|------|------------------|
| 归档在 phase 模型中的位置（A，第二轮） | A1：归档进第 10 列（`PIPELINE_PHASES` 追加 `archive`） | 语义最干净——归档就是一个 phase，六段是其 step 序列 | A2 复用 code-analyze 列：code-analyze 语义是「代码分析」占位列，与归档无关，语义污染；A3 独立归档泳道（只进 `FlowGraph.columns` 不进 `PIPELINE_PHASES`）：与「并入 phase → step 模型」字面有张力，成为图上特殊列而非真 phase |
| 归档六段 → step 词汇（B，第二轮定方向、第三轮修正） | B1：扩展 `RunStepKind`（不另立 `ArchiveStepKind`） | 一张词表、一个节点类型、一条渲染路径；第三轮修正为「归档是普通 step 序列」 | B2 新增独立 `ArchiveStepKind`：FlowNode 联合类型 + 渲染 + 转换多一套分支，`ArchiveStepStatus` 四值独立；B3 映射现有三分类（preflight/commit/seal/finalize→ToolStep、specSync/merge→WorkerAgent）：丢失「这是归档的哪一段」语义，六段名不可见 |
| 归档是否落库（C，第二轮） | C2：复用 `RunNodeRecord`（不新增表） | 第三轮「归档视为普通 step」后 C2/C3/C4 裂缝自然消解——归档节点即当前 run 节点，`run_id` 天然在案 | C1 落库到新表（ArchiveNodeRecord/ArchiveEdgeRecord）：新增 model + store 操作面 + `ChangeStateStore` trait 扩展 + 格式版本 bump；C3 不落库仅运行期实时叠加：收口后节点随 `ArchiveControl` 除名消失，「UI 合入流程图」的常驻性不成立 |
| 归档是否进 walker 分派（决策①，第三轮） | 归档视为普通 step（从「第三面命令组」降为「walker 第 10 相位的 step 序列」），`auto_next_phase` 补充「按指定 phase 停止」能力 | 「普通 step」使 run_id / 身份键 / 边（C2/C3/C4）裂缝自然消解；停止能力保住「手动执行」语义 | 保持独立手动链（`archive_flow::drive` 独立、不进 walker 分派）：正是 C2/C3/C4 裂缝的根源，且无法上图常驻 |
| 跳过语义归宿（决策②，第三轮） | 不扩 `RunStepStatus`；「直接跳过 step」（计划层不进序列）不落节点，「step 内判断」保留记录 `status=passed` + `detail` 携跳过因 | `passed(detail)` 与 run step 既有语义同构，幂等重试靠 step 内判断自我短路 | a) `RunStepStatus` 加 `skipped`：run 侧从无跳过，污染 run 语义；b) 跳过段不落节点（跳过=没执行=不上图）：幂等形态不可回溯、六段进度缺段 |
| 归档与 phase_next 路由的关系（N1，第四轮） | N1a：归档进 `REQUIREMENT_TABLE`（第 9 个路由相位） | 更符合「普通 step」；test-execution 已是「确定性相位」先例，归档是第二例且更纯粹 | N1b 归档不进表、作为 walker done 后尾部序列：drive 主循环要加「done 后尾部序列」分支，归档成为「普通相位之后的特殊尾段」，与「视为普通 step」字面有张力 |
| drive 的 `evaluator.is_none()` 硬检查（A，第五轮） | A2：移除 drive 主循环的 `next.evaluator.is_none()` 硬检查 | 真正消费点 `step_evaluator` / `step_executor` 已有各自 None 检查兜底，「steps 声明 Evaluator/VerdictGate 步就必须有 evaluator 定义」约束下沉到 step 层更精确 | A1 归档也填 dummy evaluator（延续 test-execution 的 hack）：归档真的不需要评估，dummy 字段是纯噪音 |
| walker 的 git 执行缝（B，第五轮） | B2：walker 新增独立 vcs 缝（`Arc<dyn ArchiveVcsPort>`），rebase 与冲突解算拆两步 | 与「exec/git 两类」呼应一致；Preflight 的 vcs 探测天然归 vcs 缝；git 是第三类执行器，`StepGroup` 加 git 分类 | B1 扩 `ToolCommand`（git 变体进 `ToolStepPort`）：`ToolStepPort` 定位是「进程内直调写面（sync 零 IO）+ 检查 spawn」，与 git 的「进程 spawn + 有副作用」性质不同，会稀释该缝定位 |
| 归档相位是否有 evaluator/checklist（C，第五轮） | 归档无 evaluator、无 checklist（`executor=None` / `evaluator=None`） | 归档无评估会话，确实不需要 evaluator 与机械 checklist | 第四轮「末尾 PhaseLog 落机械 checklist（`archive_phase_log_input` 三条目）+ dummy evaluator」：归档真的不需要评估，dummy 是纯噪音（该假设被 C 直接推翻） |
| `ff_advance` 落在哪一步（①，第六轮） | 放法 3：拆三段 Rebase / ResolveConflicts / Merge | 命名最清晰——rebase 重放 / 解冲突 / 主仓快进三语义分立，`ff_advance` 独立成段 | 放法 1（Rebased 路径立即 ff_advance、Conflicted 留给 ResolveConflicts）：ff_advance 散在两处，与现状内嵌循环同构；放法 2（ResolveConflicts 收尾统一 ff_advance）：ff_advance 单点但 ResolveConflicts 名不副实 |
| 归档 phase 是否走 PhaseStart（②，第六轮） | 形态 X：归档走 PhaseStart，与 8 站同构 | 复用 phase_start 的 attempt max+1 分配与详情页「运行中 · archive」呈现，形状一致 | 形态 Y（归档不走 PhaseStart、从 Preflight 起步）：attempt 恒 0、active_phase 不写 archive，但详情页归档阶段无「运行中」呈现，且成为「无 PhaseStart 的特例相位」形状不一致 |
| 「无 checklist」的落账形态（③，第六轮关键纠正） | 路径 P：空 checklist 落账（`derive_verdict([])=Pass`） | 满足「最小特殊性」——phase_next 零特判、active_phase 零特判；空 checklist 是写面既有合法输入 | 路径 Q（不落 PhaseRecord → phase_next 特判读 `ChangeRecord.status==Archived`）：引入两个 phase 层特殊场景，违背「最小特殊性」约束（第五轮误读被作废） |
| 空 checklist 落账函数（第七轮） | 复用 `run_phase_log` + 空 checklist 载荷 | 零新增落账函数、零写面改动、零特判 | 独立 `archive_phase_log_input` 落账函数：多一个函数，无必要 |
| 归档 report 是否提供 summary（第八轮） | report 也传空串（连 summary 也不需要） | 写面 report 仅长度门零非空校验；前端零读 report 的路径命中 archive；净简化（连带删 `archive_summary` 字段） | 第七轮 `archive_summary` 跨步产物 + 非空 report 摘要：report 空后失去唯一消费者，删除更简 |

## 能力

### 新增能力

- 无（本变更修改九个既有能力）。

### 修改的能力

- **desktop-change-archive** — 归档从「独立六段阶段机」重构为「walker 第 10 相位 + 10 步 step 序列」：归档入口由 stop-before-archive 停等面 + Preflight step 承载；阶段幂等重述为 step 内判断；`archive_flow` / `ArchiveControl` / `ArchiveSink` 退役；spec 同步与冲突解算两 agent 步并入 walker；双写收口由 Seal step 消费；UI 六段清单退役。
- **desktop-change-orchestration** — walker 增加 archive 相位执行：`run_step` 8 个新 step 分支、`StepContext` 增 vcs 缝、移除 `evaluator.is_none()` 检查、`PhaseCursor` 增 `rebase_conflicts`、`step_phase_log` 增空 checklist 第三分支、`auto_next_phase` 泛化「按指定 phase 停止」、收口语义改写（archive 完成才 Completed）。
- **desktop-change-worktree** — 「merge-first 归档引导」对齐新模型：归档的提交 + 合入由 walker 的 Commit / Rebase / ResolveConflicts / Merge 步承担（非独立归档链）；冲突处置分层与主仓状态约束措辞随动。
- **desktop-change-state-store** — `RunStepKind` / `RunNodeRecord` 全词汇由十值扩为十八值（8 个归档步词）；「run 执行图落库」与「步骤审计落库可查」全词汇口径随动；`archive` 双写本体零改动但 IPC 薄命令面随归档收编进 run 调整。
- **desktop-change-queries** — `PIPELINE_PHASES` 9 列扩为 10 列（追加 `archive`）；detail 出线 `pipeline` 含 archive 列、`runs[].nodes` 全词汇十八值；golden 显式重写。
- **desktop-change-flow-view** — `PIPELINE_PHASES` 10 列；归档六段清单（`ArchivePanel` / `StageList`）退役，归档 step 节点上图；前端 `RunStepGroup` / `GROUP_LABEL` 加 git 分类；`useArchiveFlow` / `archive-state.ts` 退役。
- **desktop-workflow-write-face** — `StepNode` / `RunStepKind` / `StepGroup` 词汇与分组扩展；`REQUIREMENT_TABLE` 追加 archive `PhaseDefinition`（10 步序列）；`phase_next` 路由覆盖 archive；空 checklist 落账为既有写面合法输入（零写面改动）。
- **desktop-crate-layout** — vcs 边界的消费者由「归档链」扩为「walker（orchestration）第三执行缝」，port 缝归属定稿为 `core/orchestration`；实现仍驻 `infra/vcs`，七类边界分类学不变。
- **desktop-corpus-regression** — 语料与 golden 覆盖 archive 列、归档 8 个新步词节点、git 步分类、空 checklist PhaseRecord 与 stop-before-archive 收口语义。

### 引用沿用（零 delta）

- desktop-ipc-type-bindings — 命令面增删经 tauri-specta 再生与一致性守卫既有纪律承载，机制零改动。
- desktop-phase-prompts — 14 个 executor/evaluator 角色 md 全静态零占位红线不翻案；spec 同步 / 冲突解算 prompt 为归档步专属（非相位角色 prompt），不并入本能力。
- desktop-agent-execution — spec 同步 / 冲突解算两会话复用 WorkerAgentPort / StopRegistry / 密封转录基建，零改动。
- desktop-artifact-plugins — archive 相位无 phase 产物清单（不产 proposal/design/tasks 类文档），清单驱动发现零触点。
- desktop-checks-domain — 归档步不涉及 static-check / test-execution 检查域，零触点。
- desktop-workspace-store — 无新表、无 model 版本 bump 的既有机制不翻案（词汇扩展为同表枚举加值，处置由 design 定稿）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` — `StepNode` + 8 词、`RunStepKind` + 8 词、`StepGroup` + `Git`、`StepNode::group()` / `role()` 派生随动
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `REQUIREMENT_TABLE` 追加 archive `PhaseDefinition`（10 步序列 + `executor=None` / `evaluator=None`）；`ARCHIVE_STEPS` const 声明
- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` — 路由覆盖 archive（9 相位 → done）；`has_phase_passed(archive)` 读 archive 列 PhaseRecord（空 checklist 落账后识别）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `PIPELINE_PHASES` 9 → 10 列（追加 `archive`）；`ChangeDetail.pipeline` / `runs[].nodes` 全词汇十八值投影
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` — `ChangeStepKind` + 8 词（与 `RunStepKind::as_str` 同词面）
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — 删除 drive `next.evaluator.is_none()` 检查；`run_step` 8 个归档步分支；`StepContext` 增 `vcs: Arc<dyn ArchiveVcsPort>`；`PhaseCursor` 增 `rebase_conflicts`；`step_phase_log` 增空 checklist archive 分支；收口语义改写（archive 完成才 Completed）
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — `ArchiveVcsPort`（rebase_branch / ff_merge / commit_all / commit_paths / current_branch / branch_merged / 干净探测）port 定义；`RunRequest` 增「按指定 phase 停止」参数（形态 design 定稿）
- `packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs` — **退役**（spec 同步 prompt、冲突解算 prompt、Preflight/Commit/Merge/SpecSync/Seal/Finalize 执行体搬入 walker / vcs 缝；`ArchiveControl` / `ArchiveGuard` 删除）
- `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs` — spec 同步 prompt 与冲突解算 prompt 迁入 walker 步（`prompt_ref` 静态引用或模块内 const，design 定稿）
- `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` — `commit_all` / `rebase_branch`（冲突清单读取）/ `ff_merge` / `commit_paths` / `current_branch` / `branch_merged` 子命令族（既有成员扩展，实现不越 `infra/vcs`）
- `packages/desktop/src-tauri/src/commands/archive_flow/` — **退役**（preflight / start / stop / state / watch 命令删除；归档发起 / 停止收编进 `change_flow`）
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — 收编归档发起 / 停止 / 状态；「按指定 phase 停止」参数透传
- `packages/desktop/src/views/changes/flow/layout.ts` — `PIPELINE_PHASES` 9 → 10 列（追加 `archive`）
- `packages/desktop/src/views/changes/flow/run-state.ts` — 前端 step 词汇 + `RunStepGroup` + `GROUP_LABEL` 加 git 分类；归档步词落到对应分支
- `packages/desktop/src/views/changes/flow/archive-panel.tsx` + `archive-state.ts` + `hooks/use-archive-flow.ts` — **退役**（六段清单、`ARCHIVE_STAGES`、`applyArchiveUpdate`、`useArchiveFlow` 删除）
- `packages/desktop/src/views/changes/change-detail-view.tsx` — 归档确认面对话由 stop-before-archive 停等面 + Preflight step detail 承载；移除 `ArchivePanel` 装配
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO / 命令面演进再生成
- `packages/desktop/package.json` — `version` 0.4.36 → 0.4.37

### 测试文件

- `crates/core/workflow/src/write/phase_table_test.rs` — archive `PhaseDefinition` 10 步序列声明、`executor=None` / `evaluator=None`、`PIPELINE_PHASES` 10 列同步
- `crates/core/workflow/src/write/phase_next_test.rs` — archive 路由（8 相位 pass 后 next=archive、archive 空 checklist 落账后 done）
- `crates/core/workflow/src/queries/detail_test.rs` — 10 列 pipeline、archive 列 PhaseRecord 投影、runs nodes 十八值、golden 显式重写
- `crates/core/orchestration/src/walker_test.rs` — 归档相位 10 步执行序、git 步走 vcs 缝、Rebase → ResolveConflicts（条件）→ Merge 三段、空 checklist 落账、`evaluator.is_none()` 检查移除、archive 失败即 Terminal、stop-before-archive 停等
- `crates/core/orchestration/src/archive_flow_test.rs` — **退役**（执行体断言迁入 walker_test / vcs_test）
- `crates/infra/vcs/src/git_test.rs` — `commit_all` / `rebase_branch`（冲突清单）/ `ff_merge` / `commit_paths` / 干净探测真实 git tempdir 夹具
- `packages/desktop/src/views/changes/flow/layout.test.ts` — 10 列布局
- `packages/desktop/src/views/changes/flow/run-state.test.ts` — git 分类与 8 个归档步词分支
- `packages/desktop/src/views/changes/flow/archive-panel.test.tsx` + `archive-state.test.ts` + `hooks/use-archive-flow.test.ts` — **退役**
- `packages/desktop/src/views/changes/change-detail-view.test.tsx` — 归档列上图、stop-before-archive 停等面、`ArchivePanel` 移除断言
- `crates/core/workflow/tests/fixtures/` + `corpus_golden_test.rs` — archive 列、8 个归档步词节点、空 checklist PhaseRecord 样本 + golden 显式重写

### 删除文件

- `packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs`（+ `archive_flow_test.rs`）
- `packages/desktop/src-tauri/src/commands/archive_flow/`（mod.rs + mod_test.rs）
- `packages/desktop/src/views/changes/flow/archive-panel.tsx`（+ test）、`archive-state.ts`（+ test）、`hooks/use-archive-flow.ts`（+ test）

### 不要修改

- `plugins/dev-team` 全部（版本 2.10.44、三类交付产物；`openspec-archive-change/SKILL.md` 零改动）
- `crates/core/workflow/src/write/archive.rs` 双写语义（单点收口复用零改动；至多注释面随动）
- `crates/core/workflow/src/prompts/` 14 个静态角色 md（prompt 全静态零占位红线不翻案）
- `crates/infra/vcs` 之外的进程 spawn 面（core/orchestration 自身零 spawn 红线不变）
- `SessionRecord` / 会话转录 / 三槽位 / `StepRecord` 审计职责与词汇（不做合并；归档步审计沿既有 StepRecord 面）
- `RunStepStatus` 四值（不扩 `skipped`；归档跳过 = `passed` + `detail` 携因）
- `openspec/specs/**` 既有基线 spec（本变更只写 delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 第 10 相位与双侧同步 | `REQUIREMENT_TABLE` 含 archive `PhaseDefinition`（`executor=None` / `evaluator=None`）；`PIPELINE_PHASES` 后端 `detail.rs` 与前端 `layout.ts` 均为 10 列且含 `archive`；archive 列进 `pipeline` 投影 |
| AC-2 | 10 步序列与词汇 | archive 相位 steps = PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog；`StepNode` / `RunStepKind` / `ChangeStepKind` 三个枚举各含 8 个新词（preflight / commit / rebase / resolve_conflicts / merge / spec_sync / seal / finalize），`StepGroup` 含 Git |
| AC-3 | walker 直驱归档相位 | 假引擎 + 假 vcs port + 假写面驱动 archive 相位：10 步按序执行，git 步经 vcs 缝直调（core 零 spawn），Rebase 产出 `rebase_conflicts`、ResolveConflicts 无冲突时 passed+detail、有冲突时 agent 解算，Merge 独立 ff_merge，Seal 经写面 archive 双写收口 |
| AC-4 | 空 checklist 落账 | archive 相位末尾 PhaseLog 以 `report=""` + `checklist=[]` 落 `verdict=pass` 的 PhaseRecord；`derive_verdict([])=Pass` 识别；`phase_next` 随后路由 done；零 `if phase == archive` 特判 |
| AC-5 | evaluator 检查移除 | drive 主循环无 `next.evaluator.is_none()` 检查；声明 Evaluator/VerdictGate 步而 `evaluator=None` 的漂移仍被 `step_evaluator` 拦截；test-execution dummy evaluator 清理不影响其确定性承接 |
| AC-6 | 持久化零新表 | archive 相位一次 run 收口后查询：归档 8 个新步词节点 + PhaseStart / PhaseLog 全落 `RunNodeRecord`（身份键 `(phase, attempt, step, instance_no)` 稳定唯一），串行 forward 边落 `RunEdgeRecord`；refresh 后归档列全史常驻 |
| AC-7 | 手动执行语义 | run 走完 8 个路由相位后停在 archive 前（stop-before-archive），未确认不进 archive 相位；用户确认后 archive 相位执行、run 完成（archive 完成才 Completed）；auto 模式同样停在 archive 前（归档不自动推进） |
| AC-8 | 失败停链语义 | archive 任一步失败（commit / rebase / seal 半完成 / spec_sync / 冲突无法裁决）→ `Terminal` → run 直接 `Failed` 收口，不唤决策 agent、不触发 phase retry；重试靠 step 内判断短路（干净跳过 / 已合入跳过 / 已落盘跳过 / 双写续半边） |
| AC-9 | 前端合入流程图 | `ArchivePanel` / `StageList` / `useArchiveFlow` / `archive-state.ts` 删除；归档 10 步节点上图（git 分类可辨），确认面对话由停等面 + Preflight step detail 承载；`layout.ts` 10 列、`run-state.ts` 加 git 分类 |
| AC-10 | 命令面收编与版本 | `archive_flow` 五命令 + `ArchiveControl` + `ArchiveSink` 删除，归档发起 / 停止经 `change_flow` 命令面；`packages/desktop` version 0.4.37；`plugins/dev-team` 2.10.44 零改动；`vp test` / `client:check` / knip / `bindings:check` 全绿，无新增豁免 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 归档完成语义与 run Completed 耦合（archive 完成才 Completed） | 「Ready for archiving」语义整体改写，旧用户习惯与既有测试锚失效 | 高（确定发生） | AC-7/8 显式断言新收口语义；`ALL_PHASES_PASSED` 文案退役改写；golden 显式重写留痕 |
| R2 vcs 缝进 walker 的边界稀释（git 进程 spawn 与 ToolStep 进程内直调定位混同） | core 零 spawn 红线被误读、git 执行落错层 | 中 | vcs 缝独立于 ToolStepPort（`Arc<dyn ArchiveVcsPort>`），spawn 仍驻 `infra/vcs`；desktop-crate-layout 显式定稿 port 归属 `core/orchestration` |
| R3 归档步词汇扩展面广（三枚举 + 前端 RunStepGroup + GROUP_LABEL 五处同步） | 词汇漂移 / 渲染错配 | 中 | 词汇单点纪律保持（`StepNode` ↔ `RunStepKind` ↔ `ChangeStepKind` 同词面）；AC-2 机械断言；bindings / tsc 拦截 |
| R4 归档 agent 步 provenance 碰撞（SpecSync 与 ResolveConflicts 均复用 role=Executor → `<id>/archive/executor/<attempt>` 撞号） | 会话转录联动歧义 | 中 | provenance 定式（`<id>/<phase>/<role>/<attempt>` 扩展或按 step 定式）由 design 定稿并纳入 walker_test / 前端抽屉联动测试 |
| R5 幂等重试语义回归（归档半完成续半边 vs step 内判断短路） | 归档失败重试丢中间态 / 重复执行 | 中 | AC-8 锚定 step 内判断短路 + 双写续半边既有回归；git 步幂等（干净 / 已合入 / 已落盘跳过）机械可验 |
| R6 归档 phase 列位置与 code-analyze 占位列关系（路由序 acceptance → archive，但列序追加在第 10 位） | 图上 forward 边跨 code-analyze 空列、视觉不直观 | 低 | archive 列恒追加为第 10 列（explore 定稿）；空列恒定规则扩展覆盖；列位置最终以 design 布局锚为准 |
| R7 收编 archive_flow 命令面与 useArchiveFlow 双真相源退场 | 前端拼缝残留 / 通知面割裂 | 中 | AC-9/10 断言 archive_flow 命令与前端六段清单删除；归档实时流收敛进 change_flow 通知桥 |
| R8 跳过因 `detail` 携带的口径漂移（五类跳过因 vs run step detail 语义） | 归档跳过形态不可回溯 / 语义扭曲 | 低 | 决策②定稿：跳过 = `status=passed` + `detail` 携因（干净 / 已合入 / 无 delta / 用户选择 / 已落盘）；语料 golden 覆盖 |

### 待决问题（design 定稿，不阻塞本 proposal）

- `auto_next_phase`「按指定 phase 停止」的具体形态：`RunRequest.stop_before: Option<String>`（或 `Vec<String>`）vs 确认策略枚举；archive 为该能力的固定值。
- 归档前停等是 `confirm`（proceed 二选一）还是 `ask`（syncSpecs「同步 / 跳过」选择需要 options 面）——决定 Preflight 数据面与 stop-before-archive 的呈现载体。
- 归档 agent 步（SpecSync / ResolveConflicts）的 provenance `source_ref` 定式：复用 `<id>/archive/executor/<attempt>` 会撞号，需定 `<id>/<phase>/<step>/<attempt>` 或等价唯一定式。
- `ResolveConflicts` 的 `StepGroup` 归 Agent 还是 Git（单节点装 vcs + worker 两种执行器的混合步）；前端 git 分类徽章对 ResolveConflicts 的呈现。
- archive 列相对 code-analyze 占位列的最终视觉位置（追加第 10 位 vs 前移至 acceptance 之后）与 forward 边跨空列形态。
- 空 checklist / 空 report 的 `PhaseRecord` 在 eval 节点渲染与抽屉 report 空态的最终视觉（代价 1「接受 eval 节点」已定，样式从简可选）。
- 裸 `archive_change` 薄命令是否保留（低层双写直调面）vs 归档仅经 run 可达。
