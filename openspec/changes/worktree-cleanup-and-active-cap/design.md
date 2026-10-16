# 设计: worktree-cleanup-and-active-cap

> **变更**: worktree-cleanup-and-active-cap
> **日期**: 2026-10-14
> **基于**: proposal.md、explore.md（定夺 1–5 + 二轮定夺 A/B/C）、specs/ 九个能力 delta

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 九份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」并把 proposal 遗留的七个待决问题定稿到常量面 / port 缝面 / walker 步体面 / 前端文案面。

工作树实况（本变更起点，`packages/desktop` 0.5.8 基线）：`ARCHIVE_STEPS` 现为 10 步（SpecSync → Seal 直连）；`StepNode` / `RunStepKind` / `ChangeStepKind` 现为十九值全词汇；create 前置现为七道（git 探测为第五道）；`ArchiveVcsPort` 已有归档子命令族（`commit_all` / `rebase_branch` / `ff_merge` / `commit_paths` / `current_branch` / `branch_merged` / `dirty` / `worktree_snapshot` / `rebase_continue` / `rebase_abort`），但无 worktree 移除面；`crates/infra/vcs/src/git.rs` 已有 `remove_worktree`（`git worktree remove --force <path>`，create 补偿面单点，`pub(crate)`）。本设计描述**本变更完成后的目标形态**：归档链 11 步（Cleanup 插 SpecSync 与 Seal 之间）、步词二十值、create 前置九道；零新表、零写面改动、零命令面签名变化。

---

## 关键设计决策（proposal 待决问题定稿）

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | Cleanup 的 `--force` 取舍 | **采用 `--force`**：复用既有 `git::remove_worktree`（`git worktree remove --force <path>`）单点，零新 spawn | bootstrap 的 `node_modules` / `target` 是磁盘大头，不带 `--force` 时恒为「脏 worktree」→ 移除必失败、清理语义等于不做；确认面告知在先（AC-8）+ branch 保留历史可溯（R3 对冲）+ 失败仅警告残留可见（AC-6） | 不带 `--force`（脏面报错常态踩雷）；探测后选择性 force（复杂度无收益） |
| D2 | 裸 `archive_change` 逃生口是否补 best-effort 清理 | **不补**：漏清接受 + spec 边界留痕（desktop-change-archive V1 边界 7） | 逃生口无前端入口、低频；`archive_change` 现为零 vcs 依赖的纯双写薄命令，补清理须在命令层装配 vcs 移除面（扩装配面）；残留由 create 前置⑨磁盘保底门槛兜底拦截 | 命令层薄包装 best-effort（双写成功后尽力 remove、失败仅警告——收益低、扩面） |
| D3 | Finalize 步与 Cleanup / Seal 的相对座位 | **留在 Seal 后（现状零移动）**；11 步序 = `… SpecSync → Cleanup → Seal → Finalize → PhaseLog` | Finalize 的 pathspec 由 Seal 产出的 `archived_date` 派生（`changes/archive/YYYY-MM-DD-<name>`），前移则主仓目录尚未改名、脏探测与 pathspec 提交失锚；「清理完成后修改 change 状态」由 Cleanup → Seal 相邻序已成立，Finalize 座位与该语义无关 | Finalize 前移至 Cleanup 前（破 `seal_outcome` 依赖，须改 Seal 语义——超范围） |
| D4 | `ArchiveVcsPort` 清理面形态 | **单方法 `remove_worktree`**（`fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>`）；**在场探测不进 port**——walker 步体 fs `Path::is_dir()` 判定（`step_commit` 既有先例）；infra 侧 `ProcessArchiveVcs` 与 `ProcessWorktree` 双 port 委托**同一 `git::remove_worktree` 单点**（零第二 spawn） | 探测是纯 fs 零 git，port 面只收 spawn 面（最小）；双 port 各表是 crate 依赖方向使然（core/orchestration MUST NOT 依赖 core/workflow 的 `WorktreePort`），infra 单点复用防漂移 | 探测 + 移除两方法（探测无须进 port）；单方法内含探测（跳过注记须回传 walker，跨层往返） |
| D5 | 上限常量命名与落位 | `ACTIVE_CHANGE_LIMIT: usize = 5` + `WORKTREE_DIR_SLACK: usize = 2`，门槛派生 `ACTIVE_CHANGE_LIMIT + WORKTREE_DIR_SLACK`（= 7）；均 create.rs 私有 `const`（沿 `BOOTSTRAP_LOCKFILE_COMMANDS` / `FEEDBACK_LOOP_LIMIT` 先例） | 派生形态自描述「+2 冗余防线」语义（改上限门槛自动随动）；硬编码不进 `config.json`（定夺 5） | 门槛独立常量 7（派生关系丢失、双处改漂移）；config 键（零真实诉求） |
| D6 | Cleanup detail / 前置拒绝文案 | 词汇单点：**成功** `已移除 worktree（{path}）`；**失败警告** `worktree 清理失败（尽力步，不影响归档收口）: {err}`（`err` 由 infra `Err` 面组装，附路径与 `git worktree remove --force {path}` 手动指引）；**缺席跳过** 新词 `TXT_SKIP_ABSENT = "worktree 已不在场，跳过清理"`；**legacy 跳过** 复用既有 `TXT_SKIP_LEGACY`；**前置⑧文案** `active change 数已达上限（当前 {count}，上限 {ACTIVE_CHANGE_LIMIT}）：请先归档或手动清理后再创建`；**前置⑨文案** `worktree 目录数已达保底门槛（当前 {count}，门槛 {threshold}）：请先以 git worktree list 对账并手动清理后再创建` | 与 walker 既有 `TXT_SKIP_*` 词族 / create 既有错误文案同格（含计数与引导，AC-1 / AC-2 / AC-6 断言面） | 各处内联散文案（漂移面） |
| D7 | 详情信息行「已清理」注记 | **不加**：清理后 `ChangeRecord.worktree` 字段与详情信息行照旧呈现原路径 | 二轮定夺 C：change 已 archived 无后续操作，字段为创建期执行锚（历史锚）；零 golden 重写 | 加注记（字段演进无收益 + detail 线面扩面） |
| D8 | 停等面清理告知文案（AC-8） | `ArchiveMergeNotice` 改写为「将执行提交 + 合入主仓当前分支：{mergeTarget}；收口后自动移除 worktree（分支 {branch} 保留）」（branch 自 `preflight.branch` 在案）；归档确认卡片头动作清单句补「自动移除 worktree」；worktree 记录在场才呈现（legacy 零清理告知，既有条件渲染不变） | 删除磁盘目录属用户可感知动作，确认在先；警告不阻断语义不变（卡片仍为 inform + confirm） | 独立告知行（卡片行数增、信息散） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 步词汇与分组单点 | `StepNode` / `RunStepKind` 十九值 → 二十值（+`Cleanup`，snake 线词 `cleanup`）；`StepNode::group()` 将 cleanup 派生 `StepGroup::Git`、`role()` 恒 `None`；`StepGroup::Git` doc 注释补 cleanup | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | model、serde、specta | Rust（中性状态类型） |
| 计划图单源 | `ARCHIVE_STEPS` 10 步 → 11 步（SpecSync 与 Seal 之间插 `Cleanup`；`prompt_ref=None` / `on_fail=None`——尽力步语义驻 walker 执行体）；requirement / test-only 两表共用同一 static 天然随动 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | state、agent（ModelLevel） | Rust（sync，编译期 const） |
| create 前置执法 | 前置七道 → 九道：⑧ active 建档数上限（store 读计数）与 ⑨ worktree 磁盘保底门槛（fs 扫描 `worktree_root` 直属目录数）插在前置④（db 同名查重）之后、前置⑤（`vcs.probe` git 探测）之前——拒绝面零 vcs 调用、零建档、零目录、零 worktree；上限常量驻本文件私有 | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | state（`ChangeStateStore` port）、`WorktreePort`、foundation::layout、std fs | Rust（sync 写面纪律） |
| 编排步词汇 | `ChangeStepKind` +`Cleanup`（serde camelCase 线词 `cleanup`；`as_str` = `cleanup`）；doc 注释 Git 步族补 cleanup | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | serde、specta | Rust |
| 落库词面映射 | `mapped_step` 补 `ChangeStepKind::Cleanup → RunStepKind::Cleanup`（二十值 1:1 零过滤） | `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | workflow::state | Rust |
| vcs 清理缝 | `ArchiveVcsPort` 增单方法 `remove_worktree`（`--force` 语义 / branch 零触碰 / `Err` 面带路径与手动清理指引） | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | std path | Rust（port 定义驻消费者） |
| vcs 缝实现 | `ProcessArchiveVcs::remove_worktree` 委托既有 `git::remove_worktree` 单点（与 `ProcessWorktree` 共点，零新 spawn）；`Err` 映射附路径与手动清理指引 | `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`（+ `git.rs` doc 注释） | orchestration::port、git 模块 | Rust（sync，spawn 驻此） |
| walker Cleanup 步 | `run_step` 增 `StepNode::Cleanup` 路由臂（SpecSync 与 Seal 臂之间）+ `step_cleanup` 执行体：`archive_context` 解析 worktree 字段 → legacy / 缺席幂等跳过（`passed` + detail 注记）；在场经 `ctx.vcs.remove_worktree`——`Err` 吞为 detail 警告、步恒 `passed`、链必收口（永不 `Terminal`）；缺 preflight 载荷沿既有计划漂移面 `Terminal` | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | port、state、control | Rust、tokio async |
| store 词面注释 | `RunNodeRecord.step` doc 十九值 → 二十值（表结构零变化、零迁移） | `packages/desktop/src-tauri/crates/infra/store/src/model.rs`（+ `store.rs` 读面 doc） | workflow::state | Rust |
| 详情投影注释 | 节点行线面 doc 十九值 → 二十值（纯 derive 零字段属性，词汇扩展自动跟随） | `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | workflow::state、specta | Rust |
| bindings 再生 | `RunStepKind` / `ChangeStepKind` TS 词面 + `"cleanup"`（specta 自动跟随，非手改） | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | TypeScript（生成物） |
| 前端词归一与节点 | `normalizeKind` / `stepWord` 词汇注释 19 → 20（cleanup 为 identity 词走 `default` 分支——commit / rebase / merge / finalize / preflight / seal 同先例）；`groupOf` 把 cleanup 归 `git`；`STEP_LABEL` 增 cleanup 词条 | `packages/desktop/src/views/changes/flow/run-state.ts` + `run-step-node.tsx` | dto（bindings 类型） | TypeScript |
| 停等面告知 | `ArchiveMergeNotice` 文案改写（清理告知 + 分支保留）与卡片头动作清单句补清理项；legacy 零清理告知 | `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | dto、hooks | TypeScript、React |

---

## 变更清单

### 新增文件

<!-- 不涉及：本变更零新增文件。Cleanup 步执行体落 walker.rs 既有归档步族、清理 port 面落既有 port.rs / lib.rs、上限常量驻 create.rs 私有、前端文案落既有组件。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | `StepNode` 增 `Cleanup` 变体（位序 SpecSync 与 Seal 之间，doc「归档 worktree 清理步（vcs.remove_worktree 尽力步）」）；`RunStepKind` 增 `Cleanup`（同位序同 doc 口径）；`StepNode::as_str()` 两臂补 `"cleanup"`；`StepNode::group()` Git 臂补 Cleanup（`Commit \| Rebase \| Merge \| Cleanup \| Finalize → Git`）；`StepNode::role()` None 臂补 Cleanup；`RunStepKind::as_str()` 补臂；`StepGroup::Git` doc 注释步族清单补 cleanup；两处「十九值」doc → 「二十值」 | 计划图 + 执行图词汇单点，白名单式扩展 19 → 20 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | `ARCHIVE_STEPS` 在 SpecSync 与 Seal 之间插 `StepDefinition { node: StepNode::Cleanup, prompt_ref: None, on_fail: None }`；`ARCHIVE_STEPS` doc 注释「10 步」→「11 步」并注记 Cleanup 尽力步语义（失败仅 step detail 警告、步恒 passed——尽力语义驻 walker 执行体，on_fail=None 形态与其余步一致） | 11 步序列单点；requirement / test-only 两表共用 static 随动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 新增私有常量 `ACTIVE_CHANGE_LIMIT: usize = 5`、`WORKTREE_DIR_SLACK: usize = 2`（门槛 = 两者之和，派生形态）；前置⑧：`store.list_change_records()` 过滤 `status == ChangeStatus::Active` 计数 ≥ 上限时 `Err`（文案 D6，含当前计数 / 上限 / 「归档或手动清理」引导；归档记录不计入）；前置⑨：`fs::read_dir(worktree_root)` 计直属子目录数（`file_type().is_dir()`；目录不存在（NotFound）计 0，其余读错误显式 `Err`）> 门槛时 `Err`（文案 D6，含目录数 / 门槛 / `git worktree list` 对账引导）；两道插在前置④与前置⑤（`vcs.probe`）之间；模块 doc「前置七道」→「前置九道」 | 上限两道全 IO 前置（拒绝面零 vcs 调用零建档零目录零 worktree）；`worktree_root` 入参即 `worktrees/{身份段}/` 父锚，派生单点零改动消费 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | `ChangeStepKind` 增 `Cleanup` 变体（位序 SpecSync 与 Seal 之间，doc「归档 worktree 清理步（vcs，尽力步）」）；`as_str()` 补 `"cleanup"` 臂；枚举 doc 注释 Git 步族清单补 cleanup（commit / rebase / merge / cleanup / finalize） | 编排侧词面与 `RunStepKind` 同词面 1:1 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | 新增词汇常量 `TXT_SKIP_ABSENT: &str = "worktree 已不在场，跳过清理"`；`run_step` 增 `StepNode::Cleanup` 路由臂（SpecSync 与 Seal 臂之间）；新增 `step_cleanup` 执行体（见架构组件行）；「归档相位 8 步」两处 section / 分支注释改 9 步并注记 Cleanup 尽力步例外；归档步族 doc 的步序描述随动 | Cleanup 执行主体；失败仅 detail 警告、永不 `Terminal`；零 branch 触点 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs` | `mapped_step` 补 `ChangeStepKind::Cleanup => RunStepKind::Cleanup` 臂；doc「十九值 1:1」→「二十值 1:1」 | 落库词面直映射零过滤 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `ArchiveVcsPort` 增方法 `remove_worktree`（签名见公共函数表；doc：`git worktree remove --force <path>`——带走 bootstrap 的 `node_modules` / `target`；branch MUST NOT 触碰（`worktree remove` 本就不动分支）；`Err` 面带路径与手动清理指引；在场探测驻调用方 fs 判定，port 零探测方法） | 归档链 → vcs 清理的进程内缝（spawn 驻 infra） |
| `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | `impl ArchiveVcsPort for ProcessArchiveVcs` 增 `remove_worktree`：委托既有 `git::remove_worktree`（与 `ProcessWorktree` 同一单点，零新 spawn），`Err` 经 `map_err` 附路径与手动清理指引（`worktree 移除失败（{path}）: {原始错误}；请手动执行 git worktree remove --force {path} 清理`） | git 工作面进程执行；infra 单点复用 |
| `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` | `remove_worktree` doc 注释补注：create 补偿面之外由归档相位 Cleanup 步经 `ProcessArchiveVcs` 复用（零逻辑改动） | 词汇注释随动（`--force` 单点已在案） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `RunNodeRecord.step` 字段 doc「十九值全词汇封闭集」→「二十值全词汇封闭集」 | 词面注释随动（表结构零变化、零迁移） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | run 节点读面 doc「十九值全词汇零过滤」→「二十值」 | 词面注释随动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 节点行线面 doc「十九值」→「二十值」（纯 derive 零字段属性——词汇扩展自动跟随，零代码改动） | 词面注释随动 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成：`RunStepKind` 与 `ChangeStepKind` TS 联合类型各 + `"cleanup"`（非手改，`bindings:check` 守卫拦截漂移） | 前端类型跟随 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | `normalizeKind` / `stepWord` doc 词汇注释「十九值」→「二十值」（cleanup 为 identity 词——snake / camel 同形，走 `default` 分支直返，与 commit / rebase / merge / finalize / preflight / seal 先例一致，零显式 case）；`groupOf` git 组判定补 `step === 'cleanup'` | 前端词归一与 git 分类单点 |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | `STEP_LABEL` 增 `cleanup: 'worktree 清理'`（`Record<ChangeStepKind, string>` 封闭联合强制补键） | cleanup 节点标签 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | `ArchiveMergeNotice` 文案改写（D8：提交 + 合入 + 「收口后自动移除 worktree（分支 {branch} 保留）」，branch 自 `preflight.branch`）；归档确认卡片头动作清单句补「自动移除 worktree」；呈现条件不变（`preflight.worktree !== null` 才渲染——legacy 零清理告知） | stop-before-archive 停等面清理告知（AC-8） |
| `packages/desktop/package.json` | `version` 0.5.8 → 0.5.9 | 版本交付（`src-tauri/Cargo.toml` 不随动；`plugins/dev-team` 零改动仍 2.10.44） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs` | mock单元补充 `remove_worktree` api | 保持类型一致 |

> **零改动沿用**：`crates/core/workflow/src/write/archive.rs` 双写本体（零 git 触点红线——Cleanup 步不属双写本体）；`ArchiveOutcome` / `CreateOutcome` 字段面（golden wire 冻结）；`ChangeRecord.worktree` 字段语义与 detail 出线（清理后历史锚原样保留）；`phase_next` / `phase_start` / `phase_log` / `backtrack` 相位机操作语义；branch `change/<name>` 生命周期（全程零 `git branch -d`，`delete_branch` 仍限 create 补偿面）；`config.json` 与 config schema（上限硬编码零新增键）；`FEEDBACK_LOOP_LIMIT` 等既有常量与 prompt 模板；`LocalToolSteps` / `control.rs` / 命令面（`create_change` / `archive_change` 签名不变，上限错误经既有 `Result<T, String>` 面透传）；语料种子与 golden 重写（`tests/fixtures/` + `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程）由测试相位承接。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `ArchiveVcsPort.remove_worktree` | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | 新增（trait 方法） | `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` | `git worktree remove --force <path>`（`-C main_root` 锚）；branch 零触碰；`Err` 面带路径与手动清理指引；唯一生产实现 `ProcessArchiveVcs`（infra/vcs） |
| `ProcessArchiveVcs.remove_worktree` | `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs` | 新增（impl 方法） | `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>` | 委托既有 `git::remove_worktree` 单点（与 `ProcessWorktree` 共点）；`Err` 映射附路径与 `git worktree remove --force {path}` 指引 |
| `StepNode.as_str` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` | 补 `Cleanup => "cleanup"` 臂 |
| `StepNode.group` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | `pub fn group(self) -> StepGroup` | Git 臂补 Cleanup（`cleanup` 归 Git 分组） |
| `StepNode.role` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | `pub fn role(self) -> Option<AgentRole>` | None 臂补 Cleanup（非 agent 步） |
| `RunStepKind.as_str` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` | 补 `Cleanup => "cleanup"` 臂 |
| `ChangeStepKind.as_str` | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | 修改 | `pub fn as_str(self) -> &'static str` | 补 `Cleanup => "cleanup"` 臂 |
| `create` | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 修改（签名不变） | `pub fn create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str, title: &str, workflow_type: &str) -> Result<CreateOutcome, String>` | 前置七道 → 九道（⑧ active 上限 / ⑨ worktree 磁盘门槛，均全 IO 前置）；建域四段组合零改动 |
| `phase_table` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | 修改（行为随表） | `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` | archive `PhaseDefinition` 随 `ARCHIVE_STEPS` 声明自然 11 步（`phase_next` 下发面自动跟随，零函数改动） |
| `create_change`（IPC） | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 不变 | `#[tauri::command] pub async fn create_change(app: AppHandle, root: String, name: String, goal: String, workflow_type: String) -> Result<CreateOutcome, String>` | 上限错误经既有 `Result<T, String>` 面透传（三件事纪律不变） |
| `archive_change`（IPC） | `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 不变 | `#[tauri::command] pub fn archive_change(app: AppHandle, root: String, id: String) -> Result<ArchiveOutcome, String>` | 裸双写逃生口零改动（D2：不补 best-effort，漏清留痕 + 前置⑨兜底） |

> `step_cleanup` 为 `walker.rs` 私有 async 执行体（非模块级导出），不入本表；`mapped_step`（run_history.rs）为私有映射函数，同不入表。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `StepNode` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | 十九值 → 二十值（+`Cleanup`，位序 SpecSync 与 Seal 之间；线词 `cleanup`；归 `StepGroup::Git`、`role = None`） |
| `RunStepKind` | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs` | 修改 | 十九值 → 二十值（+`Cleanup`，与 `StepNode` 同词面 1:1；`RunNodeRecord.step` 落库词汇白名单式扩展，零迁移） |
| `ChangeStepKind` | `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs` | 修改 | 十九值 → 二十值（+`Cleanup`；serde camelCase 线词 `cleanup` 与 `as_str` snake 线词 `cleanup` 同形） |

> `StepGroup` / `AgentRole` / `OnFail` / `StepDefinition` / `PhaseDefinition` / `ArchivePreflight` / `PhaseCursor` / `ArchiveOutcome` / `CreateOutcome` / `ChangeStateRecord` 零改动（Cleanup 为 git 步，无角色、无 prompt、无新跨步产物；`ChangeRecord.worktree` 字段语义保留）。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.5.9` | 0.5.8 → 0.5.9（`tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 仍 2.10.44） |

> 上限不进配置：`ACTIVE_CHANGE_LIMIT` / `WORKTREE_DIR_SLACK` 为 `create.rs` 私有源码常量（D5），`config.json` 与 config schema 零新增键。

---

## 数据模型

### 计划图（源码，编译期静态）

`ARCHIVE_STEPS` 声明 11 步（requirement / test-only 两表共用）：

```
PhaseStart        exec（复用；attempt max+1 分配）
Preflight         exec（只读校验 + vcs 探测；载荷入 PhaseCursor.preflight）
Commit            git（vcs.commit_all；干净跳过）
Rebase            git（vcs.rebase_branch → PhaseCursor.rebase_conflicts）
ResolveConflicts  agent（条件：无冲突 passed + detail；有冲突解冲突 agent）
Merge             git（vcs.ff_merge；已合入幂等跳过）
SpecSync          agent（条件：delta specs 在场且未跳过；否则 passed + detail）
Cleanup           git（vcs.remove_worktree --force；尽力步——失败仅 detail 警告、步恒 passed）
Seal              exec（workflow::write::archive 双写收口：目录改名 + db 翻转）
Finalize          git（vcs.commit_paths；archived_date 派生 pathspec；已落盘跳过）
PhaseLog          exec（空 checklist 落 verdict=pass）
```

每步 `on_fail = None`、`prompt_ref = None`。Cleanup 的尽力语义驻 walker 执行体（`step_cleanup` 对 remove 失败吞为 detail 警告后恒返回 `Ok`）——`on_fail` 声明面与其余步一致，例外在执行体不在计划图。Seal 整体后移到 Cleanup 之后（二轮定夺 B：「清理完成后修改 change 状态」由此序成立）；Finalize 留在 Seal 后（D3：pathspec 依赖 `seal_outcome.archived_date`）。

### create 前置校验序（九道，全 IO 前置）

```
① name kebab-case + 长度          纯内存
② goal 非空白 + title 回退 + workflow_type 合法值   纯内存
③ 主仓 active 目录已存在           fs
④ db 同名 active 建档              store 读
⑧ active 建档数上限（新）           store 读（status=active 计数 ≥ ACTIVE_CHANGE_LIMIT 拒）
⑨ worktree 磁盘保底门槛（新）       fs（worktree_root 直属目录数 > ACTIVE_CHANGE_LIMIT + WORKTREE_DIR_SLACK 拒）
⑤ git 探测三态                     vcs.probe（首个 git 触点）
⑥ branch change/<name> 已存在      vcs
⑦ worktree 目标目录已存在          fs
→ 执行段一～四（建档 → worktree add → 目录树 → bootstrap，零改动）
```

⑧⑨ 位于 git 探测之前：拒绝面零 vcs 调用（AC-1 / AC-3 断言面）；⑨ 扫描锚 = `worktree_root` 入参（命令层经 `worktree_dir(data_root, root, name).parent()` 派生的 `worktrees/{身份段}/` 目录），直属子目录即各 change worktree / 孤儿残留，逐目录计 `file_type().is_dir()`；目录不存在（NotFound）计 0（尚无 worktree 面），其余读错误显式 `Err`。

### 执行图与状态载体（db / 进程内）

| 模型 | 字段 / 变更 | 持久化 | 关系 / 说明 |
|------|------------|--------|-------------|
| `RunNodeRecord` | `step: RunStepKind` 词汇 + `cleanup`（白名单式） | native_db（表结构零变化、零迁移——serde 线词扩展，旧库可读） | cleanup 节点零过滤落库（AC-9），串行 forward 边入 `RunEdgeRecord`（边表零变化） |
| `ChangeRecord.worktree` | 零变化（清理后原样保留） | native_db | 历史锚（二轮定夺 C）；详情信息行照旧呈现路径（D7 不加注记） |
| `PhaseCursor` | 零新字段 | 进程内 | Cleanup 自持消费 `cursor.preflight`（`archive_context` 既有读取面），无跨步产物 |
| `RunStepStatus` | 零变化 | — | Cleanup 恒 `passed`（失败 / 跳过均经 `detail` 承载，不落 `failed`） |
| `StepRecord`（审计面） | 零变化 | native_db | 归档步为执行图节点非相位机步骤，既有纪律不变 |

### Cleanup 步状态机（尽力语义）

```
worktree = None            → passed + detail「legacy 无 worktree」（零 git worktree 触点）
worktree 目录缺席           → passed + detail「worktree 已不在场，跳过清理」（幂等：手动清 / 重试链）
worktree 在场 → remove Ok   → passed + detail「已移除 worktree（{path}）」
worktree 在场 → remove Err  → passed + detail 警告（路径 + git worktree remove --force {path} 指引）
                              链照常收口（Seal / Finalize / PhaseLog 续走；ArchiveOutcome 零变化）
缺 preflight 载荷（计划漂移）→ failed 节点 + Terminal（沿既有归档步漂移面，非清理失败面）
```

branch `change/<name>` 全程零触碰（`git worktree remove` 不动分支；`delete_branch` 仍限 create 补偿面）。

<!-- 路由 / API 设计：不涉及——本变更零新增 / 修改 HTTP 端点与 IPC 命令签名（create_change / archive_change 透传既有 Result<T, String> 面）。 -->

---

## 依赖

### 运行时依赖

零新增。既有依赖复用：`std::fs`（前置⑨目录扫描）、`workflow::state::ChangeStateStore`（前置⑧计数读取）、`orchestration::port::ArchiveVcsPort`（清理缝）、`crates/infra/vcs` git 子命令族（`remove_worktree --force` 既有单点）、specta / serde（词汇类型派生）。

### 构建 / 测试依赖

零新增。bindings 再生走既有 `pnpm -C packages/desktop run bindings:export` 管线（`bindings:check` 守卫）；golden 显式重写走既有 `DESKTOP_GOLDEN_REWRITE=1` 流程（语料样本扩展由测试相位承接）。

---

## 待决问题

无阻塞待决——proposal 七项待决已全部定稿（D1–D8）。遗留两项低风险注记（非本变更范围）：

- **裸 `archive_change` 漏清的后续偿还**（D2）：若逃生口使用频率超预期，best-effort 命令层包装另立变更评估（写面零 git 触点红线不动）。
- **弃置（abandon）面**（定夺 4 欠账）：active 占额度无退路的洞由错误文案引导「归档或手动清理」过渡，另立变更偿还。
