# 任务: worktree-cleanup-and-active-cap

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接（含 `create_test.rs` / `walker_test.rs` / `state_test.rs` / `phase_table_test.rs` / `git_test.rs` / `detail_test.rs` / 语料种子与 `DESKTOP_GOLDEN_REWRITE=1` golden 显式重写 / 前端 `run-state.test.ts` / `run-step-node.test.tsx` / `run-control-panel.test.tsx` / `mod_test.rs`），本列表只覆盖实现文件。顺序按依赖排列：core/workflow 词汇与计划图 → create 前置 → orchestration 词面映射 → vcs 清理缝 → walker Cleanup 步 → 注释随动 → bindings 再生 → 前端随动 → 版本交付。
>
> 依赖序要点：`phase_table.rs` 与 `run_step` 路由臂消费 `StepNode::Cleanup`（阶段一先行）；`step_cleanup` 调用 `ArchiveVcsPort::remove_worktree`（阶段四先于阶段五）；`mapped_step`（阶段三）消费两侧词面；bindings 再生（阶段七）依赖 Rust 词面定稿并先于前端随动（阶段八，`STEP_LABEL: Record<ChangeStepKind, string>` 与 `groupOf` 消费 bindings 新联合类型）；create 前置（阶段二）独立于词汇面，可并行。

## 阶段一：core/workflow 词汇与计划图

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/state.rs`：`StepNode` 增 `Cleanup` 变体（位序 SpecSync 与 Seal 之间，doc「归档 worktree 清理步（vcs.remove_worktree 尽力步）」）；`RunStepKind` 增 `Cleanup`（同位序）；`StepNode::as_str()` / `RunStepKind::as_str()` 各补 `Cleanup => "cleanup"` 臂；`StepNode::group()` Git 臂补 Cleanup（Commit / Rebase / Merge / Cleanup / Finalize → Git）；`StepNode::role()` None 臂补 Cleanup；`StepGroup::Git` doc 步族清单补 cleanup；两处「十九值」doc 改「二十值」
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`：`ARCHIVE_STEPS` 在 SpecSync 与 Seal 之间插 `StepDefinition { node: StepNode::Cleanup, prompt_ref: None, on_fail: None }`；`ARCHIVE_STEPS` doc「10 步」→「11 步」并注记 Cleanup 尽力步语义驻 walker 执行体；表侧注释中归档步序表述随动（requirement / test-only 两表共用 static 天然跟随，零第二处改动）

## 阶段二：create 前置第八 / 九道

- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`：新增私有常量 `ACTIVE_CHANGE_LIMIT: usize = 5` 与 `WORKTREE_DIR_SLACK: usize = 2`（门槛 = 两者之和派生，不落独立 7 字面量）
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`：前置⑧ active 建档数——`store.list_change_records()` 过滤 `status == ChangeStatus::Active` 计数 ≥ `ACTIVE_CHANGE_LIMIT` 时返回 `Err`（文案含当前计数与上限、引导「归档或手动清理」；archived 记录不计入），插在前置④（db 同名查重）之后、前置⑤（`vcs.probe`）之前
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`：前置⑨ worktree 磁盘保底——`fs::read_dir(worktree_root)` 计直属子目录数（`file_type().is_dir()`；NotFound 计 0、其余读错误显式 `Err`）> 门槛时返回 `Err`（文案含当前目录数与门槛、引导 `git worktree list` 对账与手动清理），紧随前置⑧；模块 doc「前置七道校验」改「前置九道校验」并补两道描述

## 阶段三：orchestration 词面与映射

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`：`ChangeStepKind` 增 `Cleanup` 变体（位序 SpecSync 与 Seal 之间，doc「归档 worktree 清理步（vcs，尽力步）」）；`as_str()` 补 `Cleanup => "cleanup"` 臂；枚举 doc 的 Git 步族清单补 cleanup
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs`：`mapped_step` 补 `ChangeStepKind::Cleanup => RunStepKind::Cleanup` 臂；doc「十九值 1:1 全映射」改「二十值」

## 阶段四：vcs 清理缝（port + infra 实现）

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`：`ArchiveVcsPort` 增方法 `fn remove_worktree(&self, main_root: &Path, worktree: &Path) -> Result<(), String>`（doc：`git worktree remove --force <path>` 带走 bootstrap 的 `node_modules` / `target`；branch MUST NOT 触碰；`Err` 面带路径与手动清理指引；在场探测驻调用方 fs 判定，port 零探测方法）
- [x] 修改 `packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`：`impl ArchiveVcsPort for ProcessArchiveVcs` 增 `remove_worktree` 委托既有 `git::remove_worktree`（与 `ProcessWorktree` 同一单点，零新 spawn），`Err` 经 `map_err` 附路径与 `git worktree remove --force {path}` 手动清理指引
- [x] 修改 `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs`：`remove_worktree` doc 注释补注 create 补偿面之外由归档相位 Cleanup 步经 `ProcessArchiveVcs` 复用（零逻辑改动）

## 阶段五：walker Cleanup 步

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：新增词汇常量 `TXT_SKIP_ABSENT: &str = "worktree 已不在场，跳过清理"`；`run_step` 增 `StepNode::Cleanup` 路由臂（SpecSync 与 Seal 臂之间，`step_cleanup(...).await?; Ok(StepFlow::Proceed)`）
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：新增 `step_cleanup` 执行体——`begin_archive_node` 开节点后经 `archive_context(cursor)` 取 `(name, worktree)`：缺 preflight 载荷沿既有漂移面落 failed 节点 + `Terminal`；`worktree=None` → `passed` + `TXT_SKIP_LEGACY`（零 git worktree 触点）；worktree 目录缺席（`Path::is_dir()` 假）→ `passed` + `TXT_SKIP_ABSENT`；在场 → `ctx.vcs.remove_worktree(main_root, worktree)`：`Ok(())` → `passed` + detail「已移除 worktree（{path}）」，`Err(error)` → **`passed`** + detail 警告「worktree 清理失败（尽力步，不影响归档收口）: {error}」（error 已含路径与手动指引），永不 `Terminal`、链照常收口；步体零 `git branch` 触点
- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：「归档相位 8 步执行体」section 注释与 `run_step` 归档步族分支注释改 9 步（注记 Cleanup 尽力步例外——失败仅 detail 警告恒 passed）；PhaseCursor 零新字段（cleanup 自持消费 preflight 载荷）

## 阶段六：词面注释随动（机械）

- [x] 修改 `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：`RunNodeRecord.step` 字段 doc「十九值」→「二十值」（表结构零变化零迁移）；`packages/desktop/src-tauri/crates/infra/store/src/store.rs` run 节点读面 doc 同步「二十值」
- [x] 修改 `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`：节点行线面 doc「十九值」→「二十值」（纯 derive 零字段属性，词汇扩展自动跟随）

## 阶段七：bindings 再生（前端随动的前置）

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `packages/desktop/src/types/generated/bindings.ts`（`RunStepKind` / `ChangeStepKind` TS 联合类型各 + `"cleanup"`；非手改）——先于阶段八（前端 `STEP_LABEL` 封闭 Record 与 `groupOf` 消费新联合类型）

## 阶段八：前端随动

- [x] 修改 `packages/desktop/src/views/changes/flow/run-state.ts`：`normalizeKind` / `stepWord` doc 词汇注释「十九值」→「二十值」（cleanup 为 identity 词走 `default` 分支直返——commit / rebase / merge / finalize / preflight / seal 先例，零显式 case）；`groupOf` 的 git 组判定补 `step === 'cleanup'`
- [x] 修改 `packages/desktop/src/views/changes/flow/run-step-node.tsx`：`STEP_LABEL` 增 `cleanup: 'worktree 清理'`（`Record<ChangeStepKind, string>` 封闭联合强制补键；Cleanup 警告 / 跳过注记经既有 detail 面呈现，节点渲染零改动）
- [x] 修改 `packages/desktop/src/views/changes/flow/run-control-panel.tsx`：`ArchiveMergeNotice` 文案改写为「将执行提交 + 合入主仓当前分支：{mergeTarget}；收口后自动移除 worktree（分支 {branch} 保留）」（branch 自 `preflight.branch`）；归档确认卡片头动作清单句补「自动移除 worktree」；呈现条件不变（`preflight.worktree !== null` 才渲染，legacy 零清理告知）

## 阶段九：版本交付与静态自查

- [x] 修改 `packages/desktop/package.json`：`version` 0.5.8 → 0.5.9（`src-tauri/Cargo.toml` 不随动；`plugins/dev-team` 零改动仍 2.10.44）
- [x] 静态自查：`write/archive.rs` 与 `ArchiveOutcome` / `CreateOutcome` 字段面零改动；`config.json` / config schema 零新增键；walker 与 vcs 清理面零 `git branch` 触点（`delete_branch` 仍限 create 补偿面）；实现面 grep「十九值」零残留（测试文件除外，归测试相位）；`plugins/dev-team` 全部零改动
