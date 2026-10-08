# 测试设计: desktop-archive-change

> **变更**: desktop-archive-change
> **日期**: 2026-10-08
> **依据**: proposal.md（AC-1..AC-10）+ design.md（D1–D16 决策编号）；实现任务边界见 tasks.md（测试文件清单与其「test-design / test-gen / test-execution 阶段承接」条目由本文件展开）

---

## 测试边界与框架识别

<!-- 逐文件推导自 design.md 变更清单（实现文件 + 测试文件两节）与公共 API 表；
     实现阶段零清单外测试文件。 -->

- **Rust 面**：src-tauri cargo workspace 套件，测试文件为共置 `*_test.rs` 模块（沿 orchestration / vcs / commands 先例）。新增 `crates/core/orchestration/src/archive_flow_test.rs` 与 `src/commands/archive_flow/mod_test.rs`；扩展 `crates/infra/vcs/src/git_test.rs`、`src/commands/change_flow/mod_test.rs`、`src/bindings/mod_test.rs`。orchestration dev 依赖零新增（tokio 既有）。
- **前端面**：packages/desktop vite-plus 套件，共置 `.test.ts` / `.test.tsx`（vite-plus/test describe/it/expect）。新增 `flow/archive-state.test.ts`、`hooks/use-archive-flow.test.ts`、`flow/archive-panel.test.tsx`；扩展 `change-detail-view.test.tsx`。
- **进程边界策略**：三层分工与 design 对齐——① **vcs 真件**：归档 git 子命令族以真实 git（2.54）+ tempdir 仓锚定（design D9 实验定锚的测试形态同源；`git -C <root>` 显式寻址、`TEST_PATH_LOCK` 互斥纪律沿 crate 既有锁）；② **core 假件直驱**：归档编排链经进程内脚本化假件（`ArchiveVcsPort` / `WorkerAgentPort` / `ChangeStateStore`）+ 真实 tempdir fs 驱动（D8 port 缝可测性拍板——编排断言留 core，进程执行断言落 vcs）；③ **命令层回环**：MockRuntime mock app 托管 `WorkspaceStores` / `Arc<ChangeFlowControl>` / `Arc<StopRegistry>` / `Arc<ArchiveControl>` 四态 + PATH 隔离窗口收敛 CLI 引擎（`change_flow` mod_test CliMissing 先例）。前端 `vi.mock('@tauri-apps/api/core')`（invoke / Channel 桩）。
- **迭代类型词表**：**新增**（新用例）/ **重写**（既有用例语义演进改写）/ **适配**（机械传参或 fixture 补齐，既有断言零改动）/ **迁移** / **持衡（沿用）**。本变更 run 面零触点（design 不变组件核对结论），walker / steps / snapshot / state / control / run-control-panel / change-flow-graph 等既有测试**零触点零适配**；无重写、无迁移、无废弃（retire）行。
- **golden 零触点注记**：`ChangeDetail` / `ChangeList` 投影零改动（本变更只新增 IPC 命令与 `ArchiveUpdate` 家族），corpus golden 零重写——proposal「golden（若 DTO 演进触线）」条件不成立，`DESKTOP_GOLDEN_REWRITE=1` 流程本变更不启用。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 10 条 AC。被测文件或模块为承载用例的测试文件（单值）；
     跨半边 AC 以「（同上——…半边）」行展开到各自测试文件；纯静态 / 流程性半边落
     「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 归档入口与确认面：active 呈现按钮 / 已归档与文档形态无入口 / 确认对话（完成度结论 + delta specs + worktree 告知）/ 运行中 run 显式拒绝 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（preflight 读面半边：None 三态 + run_active 透传 + merge_target） |
| AC-1 | （同上——命令面半边：preflight blank 口径、start 拒绝面含 run 在案拒绝） | packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs |
| AC-1 | （同上——确认对话呈现半边） | packages/desktop/src/views/changes/flow/archive-panel.test.tsx |
| AC-1 | （同上——按钮两态与 panel 挂载半边） | packages/desktop/src/views/changes/change-detail-view.test.tsx |
| AC-2 | 完成度警告不阻断：fail 相位 / 缺产物警告呈现且确认后照常走通；全 pass 齐备无警告轻量确认 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（核算与警告词汇半边） |
| AC-2 | （同上——对话警告行呈现与不阻断半边） | packages/desktop/src/views/changes/flow/archive-panel.test.tsx |
| AC-3 | worktree 提交与合入：dirty → 一个提交（编辑集 + 同步产物同 commit）、merge 后主仓目录在场且 HEAD 前移；干净跳过提交；已合入跳过合入 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（阶段机编排半边：调用序 / 幂等跳过） |
| AC-3 | （同上——vcs 真件半边：commit_all / merge_branch / branch_merged 的真实 git 行为） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-4 | 不吞并无关改动：主仓无关未提交 / staged 全程保持原状态（pathspec 纪律机械验证） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs（机械断言半边：status 前后对照） |
| AC-4 | （同上——编排侧 pathspec 圈定半边：dirty 圈定参 + commit_paths 调用参断言） | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs |
| AC-5 | spec 同步 agent：provenance 在案、转录可达、cwd = worktree / 主 root，主 spec 增量合并幂等；无 delta 零 agent 会话 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（会话发起 / cwd / prompt / 三态跳过半边；LLM 实际合并保真 → 不可测试项 4） |
| AC-5 | （同上——转录入口与 live 事件半边） | packages/desktop/src/views/changes/flow/archive-panel.test.tsx |
| AC-6 | agent 失败停链可重试：失败 / 停止 → 链停同步段零提交零合入零归档，重试续走成功 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（假引擎直驱半边） |
| AC-6 | （同上——命令层半边：PATH 隔离 CLI 失败收敛 + 除名） | packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs |
| AC-7 | 双写收口与摘要：写面 archive 单点收口 + 结果摘要 + 半完成重试仅补翻转 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（seal / finalize / summary / 续半边半边） |
| AC-7 | （同上——摘要呈现半边） | packages/desktop/src/views/changes/flow/archive-panel.test.tsx |
| AC-8 | merge 冲突显式失败：git 语境 + 手动引导、零破坏、可重试 | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs（真件冲突 + abort 收口半边） |
| AC-8 | （同上——编排停等与重试续走半边） | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs |
| AC-9 | legacy 与 run 面隔离：legacy 跳 git 段、裸 `archive_change` 零回归、零 RunUpdate / ConfirmWait、run 收口不自动归档、归档中 run 发起被拒 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（legacy 跳段 + 停止收敛半边） |
| AC-9 | （同上——反向互斥半边：归档进行中 run 发起被拒） | packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs |
| AC-9 | （同上——run 面隔离命令面负断言 + ArchiveSink 转译半边；裸双写零回归由既有 changes mod_test 行持衡承载） | packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs |
| AC-10 | 版本与管线：desktop 0.4.21、dev-team 2.10.44 零改动、`vp test` / `client:check` / knip / `bindings:check` 全绿 | —（见不可测试项 1 / 3；bindings 出线行为面 → bindings/mod_test.rs 节） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] / #[tokio::test]、前端 vite-plus）。
     门面 / 纯类型 / 生成物 / 装配模块（port.rs trait 声明面、lib.rs re-export、
     Cargo.toml、main.rs manage 行、bindings.ts 生成物）不建独立测试文件，统一落
     「不可测试项」声明（含行为去向）。 -->

### packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs

<!-- 新增文件节（本变更最大断言面）。挂 AC-1（preflight 读面）、AC-2（完成度核算
     与警告词汇）、AC-3（阶段机编排）、AC-4（pathspec 调用参）、AC-5（agent 段）、
     AC-6（失败停链 / 停止）、AC-7（seal / finalize / 续半边 / summary）、AC-8（编排
     停等）、AC-9（legacy 跳段）。design D2 全阶段幂等（无持久化状态机）——重试
     行以「已成功段零重复调用」为断言锚；D5 DTO 面 / D6 谓词私有镜像（经 preflight
     出线断言，零 test-only export）/ D12 词汇单点（经 Finished summary 断言）。 -->

#### 待测功能

- preflight(layout: &Layout, store: &dyn ChangeStateStore, worktree: Option<&str>, change: &str, vcs: &dyn ArchiveVcsPort, run_active: bool) -> Option<ArchivePreflight>: 读面聚合——建档在案 + status=active 门（`None` = 不可归档）、完成度核算（`phase_table` 全相位非 stale pass/skipped 私有镜像）、产物三件（proposal.md / design.md / tasks.md）fs 核对、delta specs capability 清单（定位目录 `specs/` 子树非空）、`mergeTarget`（worktree 记录在场才 `vcs.current_branch`）、`run_active` 透传
- run_archive_flow(worker, vcs, store, control, guard, request): async 阶段机（六段序 preflight → specSync → commit → merge → seal → finalize；每阶段 `Stage` running→终态先行发布、失败停该阶段 + `Finished { error }`、阶段间取消旗检查点、成功 `Finished { summary }`）；`ArchiveRequest` 携 sync_specs
- ArchiveControl: `begin`（在案 Err = 发起重入防护）/ `publish` / `set_session` / `current_session` / `is_active` / `snapshot` / `subscribe` / `request_stop`；`ArchiveGuard::finish`（终态除名）——`ChangeFlowControl` 同型镜像
- DTO 面（serde camelCase + specta `Type`）: `ArchiveStage`（线词 preflight|specSync|commit|merge|seal|finalize）/ `ArchiveStageStatus`（running|passed|skipped|failed）/ `ArchiveStageState { stage, status, detail? }` / `ArchiveSpecsStatus`（synced|skipped|none）/ `ArchiveSummary { name, archivedDir, specs, warnings }` / `ArchiveUpdate`（tag `ipc`：Stage / SessionEvent / Finished）/ `ArchiveSnapshot { stages, sessionId? }` / `ArchivePreflight`
- spec_sync_prompt(change) 单点模板（design 数据模型逐字）——经假 worker 捕获 `turn.prompt` 断言（私有面零 test-only export）
- 警告 / 摘要词汇单点（D12）——经 `Finished { summary }` 断言

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_flow_test · 全链正向（worktree + delta specs） | 正向 | 假 vcs（dirty=true / branch_merged=false / merge Ok / finalize dirty=true）+ 假 worker（Completed）+ 主仓树 tempdir：阶段序恰 Preflight→SpecSync→Commit→Merge→Seal→Finalize 全 passed；worker 恰一次——provenance `source="change"` / `source_ref="<change>/archive/spec-sync"` / permission=BypassPermissions / role=Executor / `turn.root` = worktree 绝对路径（cwd 按 D7）；vcs 调用序 `commit_all(worktree, "archive: <name>")` → `merge_branch(main_root, "change/<name>")` → `dirty(main_root, [old, new])` → `commit_paths(main_root, 两 pathspec, "archive: move <name> to archive")`；seal 后主仓 archive 树 `YYYY-MM-DD-<name>` 目录在场 + store `set_archived` 落账；`Finished { summary }` 四字段（specs=synced） | 新增 |
| archive_flow_test · 无 delta specs 零 agent 会话 | 边界 | 定位目录无 `specs/` 子树 → SpecSync skipped（detail「无 delta specs」）；worker 零调用；commit 段照常执行；summary.specs=none（AC-5 scenario 字面） | 新增 |
| archive_flow_test · sync_specs=false 用户跳过 | 边界 | delta specs 在场 + sync_specs=false → SpecSync skipped（detail「用户选择」）；worker 零调用；summary.specs=skipped | 新增 |
| archive_flow_test · 干净 worktree 跳过提交 | 边界 | 假 vcs dirty=false → Commit skipped（detail「干净」）；`commit_all` 零调用（无空提交）；merge 照常推进 | 新增 |
| archive_flow_test · 已合入双跳过直达收口 | 边界 | 假 vcs branch_merged=true → Commit / Merge 双 skipped（detail「已合入」）；`commit_all` / `merge_branch` 零调用；链径直 seal→finalize 收口（手动 merge 等价路径——AC-3 scenario） | 新增 |
| archive_flow_test · worktree 缺失未合入 Err | 异常 | 记录 worktree=Some(缺失路径) + branch_merged=false → Preflight failed（Err 引导恢复目录或手动处置）；后续五段零执行、vcs / store 写零调用 | 新增 |
| archive_flow_test · 前置重校验拒绝两态 | 异常 | 未建档 / status=archived → Preflight failed 显式 Err；零 vcs 调用、零 store 写、fs 零变化 | 新增 |
| archive_flow_test · merge 冲突停链与重试续走 | 异常 | `merge_branch` 注入 Err（git 语境串）→ Merge failed + `Finished { error }` 含该语境；seal 零执行（store 零 `set_archived`、主仓树零改名）；重试（branch_merged=true 模拟用户手动解冲突后合入）→ Commit/Merge 双跳过、seal→finalize 收口，`commit_all` 仍零调用（已成功段不重复——D2 幂等空走） | 新增 |
| archive_flow_test · agent 失败停链可重试 | 异常 | 假 worker outcome status ≠ Completed → SpecSync failed；`commit_all` / `merge_branch` 零调用 + store 零写 + fs 零改名（零提交零合入零归档变更——AC-6 字面）；重试 worker Completed → 全链收口 | 新增 |
| archive_flow_test · 停止收敛 | 异常 | 取消旗置位（agent 段后检查点）→ 链以 `Finished { error }` 收敛且 error 含「归档链已停止」（D12 词汇锚）；后续阶段零执行、vcs 零后续调用 | 新增 |
| archive_flow_test · legacy 跳 git 段 | 边界 | 记录 worktree=None → Commit / Merge 双 skipped（detail「legacy 无 worktree」）；git 段零调用（仅 finalize 的 `dirty` / `commit_paths` 半边）；delta specs 在场时假 worker 捕获 `turn.root` = 主 workspace root（D7 legacy cwd） | 新增 |
| archive_flow_test · seal 续半边仅补翻转 | 边界 | 假 store `set_archived` 首次注入 Err（改名已落盘）→ Seal failed 呈现半完成；重试：fs 已在 archive 树 + 翻转成功 → write::archive 续半边仅补翻转（不重复改名——archive 目录名不变）；Finalize dirty=false → skipped（已落盘） | 新增 |
| archive_flow_test · finalize 已落盘跳过 | 边界 | 假 vcs `dirty(main_root, [old, new])`=false → Finalize skipped（detail「已落盘」）；`commit_paths` 零调用（重复收口重试的幂等面——D11 脏探测守卫） | 新增 |
| archive_flow_test · preflight 读面聚合全字段 | 正向 | 种子：全 pass 相位 + 产物三件 + 两 capability delta specs + worktree 记录（假 vcs current_branch 注入固定名）→ `Some` 且 `completed=true` / `incompletePhases` 空 / `missingArtifacts` 空 / `deltaSpecs` 两 capability 名 / `worktree` 出线 / `mergeTarget`=注入名 / `runActive` true·false 两态透传 | 新增 |
| archive_flow_test · preflight None 三态 | 异常 | 未建档 / status=archived / 未知名 → `None`（AC-1「不可归档」读面兜底） | 新增 |
| archive_flow_test · 完成度核算与 phase_next 等价 | 边界 | 同一组 PhaseRecord 种子（全 pass / stale pass / fail 条目 / skipped 补位 / 缺相位五形态）驱动 `preflight().completed` 与 `workflow::write::phase_next(...).done` 对拍一致；`incompletePhases` = 未过相位清单（D6 谓词镜像等价性佐证——相位机零改动红线的两头锚） | 新增 |
| archive_flow_test · workflow_type 不支持 | 边界 | `phase_table` None 的类型 → preflight `completed=false`；链收口 summary.warnings 含「工作流类型 "<type>" 无相位表，完成度不可核算」（D12 词汇锚） | 新增 |
| archive_flow_test · 摘要警告词汇 | 边界 | 未完成 change 收口 → warnings 含「工作流未全部通过：<未过相位清单>」；缺产物 → 含「缺少产物文档：<三件子集>」；全 pass 且三件齐备 → warnings 空清单（D12 单点 + AC-2 无警告面） | 新增 |
| archive_flow_test · prompt 语义锚 | 正向 | 假 worker 捕获 `turn.prompt`：含 change 名插值、`## ADDED/MODIFIED/REMOVED/RENAMED Requirements` 增量语义四行、「保留 delta 未提及的主 spec 内容」与「合并幂等」指令逐字在场；含禁止段——禁 MCP 工具（change_list 等）、禁 `__TOOL_ASK_USER__`、禁读写 workflow.json、禁 git 命令（design 数据模型逐字；AC-5 prompt MUST NOT 依赖面的文本锚） | 新增 |
| archive_flow_test · ArchiveControl 登记与重入 | 正向 | `begin` 在案同键二次 `begin` → Err（含 change 名——发起幂等防护）；`ArchiveGuard::finish` 后 `is_active`=false / `snapshot`=None（终态除名）；异键互不误拒 | 新增 |
| archive_flow_test · 控制面订阅与会话槽 | 边界 | `publish` → 订阅者收 `ArchiveUpdate` 信封；`set_session` / `current_session` 往返；`request_stop` miss（无在案）幂等 false；双复合键 (root, change) 互不串台（`ChangeFlowControl` 复合键先例同型） | 新增 |
| archive_flow_test · 快照与信封形态 | 边界 | 阶段推进中每段恰 running→终态两信封（后写覆盖同段）；`ArchiveSnapshot.stages` 累积 + `sessionId` 随会话槽；`Finished` 后 snapshot None（快照只覆盖运行期——D5）；serde 线面 `ArchiveStage` 线词 / tag `ipc` 三变体出线 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ArchiveVcsPort（注入依赖入参） | 进程内脚本化假件：六方法各自可编程返回（dirty bool / branch_merged 三态 / merge Err 注入 / current_branch 固定名 / commit_paths Err 注入）+ 全方法调用与参数捕获（调用序断言锚） | 本节全部用例（阶段机编排 / 幂等跳过 / pathspec 参 / preflight 聚合） |
| WorkerAgentPort（注入依赖入参） | 预录 `WorkerTurnOutcome` 假引擎（status / final_message 可编程 + `WorkerTurnRequest` 全字段捕获——provenance / root / prompt 断言面） | specSync 段全部用例 |
| ChangeStateStore（注入依赖入参） | 既有假件装置同型（可编程记录 + 相位行种子），扩展 `set_archived` 故障注入（续半边用例）；`list_phase_records` 种子驱动谓词等价对拍 | 本节全部用例 |
| 文件系统 | 真实 tempdir 主仓树（active / archive 两树按用例布置）+ worktree 树（delta specs / 产物三件按需布置）——seal 改名与产物核对走真实 fs | 本节全部用例 |
| tokio 运行时 | `#[tokio::test]`（async 阶段机直驱；订阅经 `ArchiveControl::subscribe` 真实 broadcast） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

<!-- 既有文件扩展节。挂 AC-3（提交 / 合入 / 祖先判定真件半边）、AC-4（无关
     staged / untracked 前后保持——机械断言）、AC-8（冲突 abort 收口）。design
     D8 六方法 + D9 git 语义定锚（ff / non-ff 容忍差、pathspec `/**` 删除路径、
     冲突 abort 幂等）——真实 git 2.54 tempdir 夹具与 D9 实验同源形态；
     `TEST_PATH_LOCK` 互斥纪律沿既有。 -->

#### 待测功能

- ProcessArchiveVcs: 无状态执行器（`new()` / `Default`，`impl ArchiveVcsPort`）
- ProcessArchiveVcs::dirty(&self, root: &Path, paths: &[&str]) -> bool: `status --porcelain [-- pathspec…]` 非空即真（空 paths = 全域；gitignore 面不计）
- ProcessArchiveVcs::commit_all(&self, worktree: &Path, message: &str) -> Result<(), String>: `add -A` + `commit -m`（argv 直传零 shell）
- ProcessArchiveVcs::branch_merged(&self, main_root: &Path, branch: &str) -> Result<bool, String>: `merge-base --is-ancestor <branch> HEAD`（退出 0/1 映射 bool、>1 Err）
- ProcessArchiveVcs::merge_branch(&self, main_root: &Path, branch: &str) -> Result<(), String>: `merge --no-edit`；失败尽力 `merge --abort`（幂等）后 Err 带 git 语境 + 手动处置引导
- ProcessArchiveVcs::current_branch(&self, main_root: &Path) -> Result<String, String>: `branch --show-current`（空输出 = detached HEAD → Err 引导）
- ProcessArchiveVcs::commit_paths(&self, main_root: &Path, paths: &[&str], message: &str) -> Result<(), String>: `add -A -- <paths…>` + `commit -m <msg> -- <paths 各自 "/**" 形态>`（删除路径覆盖——D9② 定锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| git_test · ProcessArchiveVcs 构造锚 | 边界 | `new()` 与 `Default::default()` 等值（无状态可重复构造——组合根按需铸的编译锚，`ProcessWorktree` 先例同型） | 新增 |
| git_test · dirty 全域两态 | 正向 | 未跟踪 + 已修改文件在场 → true；干净仓 → false | 新增 |
| git_test · dirty gitignore 面不计 | 边界 | `.gitignore` 圈定文件（如 `node_modules/`）在场 → 全域与 pathspec 两口径均 false（D9⑤——bootstrap 产物不脏不进提交的前提锚） | 新增 |
| git_test · dirty pathspec 圈定 | 边界 | 无关脏文件 + paths 指向干净区 → false；paths 指向脏区 → true（D11 finalize 脏探测的圈定语义） | 新增 |
| git_test · commit_all 提交编辑集 | 正向 | worktree 内修改 + untracked 新文件 → 恰一个新提交（rev-list 计数 +1）且两者入树（`git ls-tree` 命中）；gitignored 目录不入提交（D9⑤） | 新增 |
| git_test · commit_all 信息 argv 直传 | 边界 | CJK / 含空格信息（`archive: <name>` 形态）→ `git log -1 --format=%s` 与入参逐字一致（零 shell 包装——R7 引号形态不适用的正面锚） | 新增 |
| git_test · branch_merged 三态 | 正向 | 分支未合入 → false；merge 后 → true；缺分支 → Err（>1 退出码面，非 false——与 miss 可辨） | 新增 |
| git_test · merge ff 成功不吞 staged | 正向 | 主仓居基线、分支领先（ff 形态）+ 预置无关 staged 条目（`A other.txt`）与无关 untracked → merge Ok 且主仓 HEAD = 分支 tip；staged 条目合入后原样 staged、untracked 原样（AC-4 / D9① 机械断言：`git status --porcelain` 前后对照） | 新增 |
| git_test · merge non-ff 拒绝 | 异常 | 主仓已前进 + 分支前进（merge commit 形态）+ 无关 staged → Err 含 git 语境（"local changes … would be overwritten" 类）；仓未落半截 merge 态；staged 原样保留（D9①——「主仓 git 状态不允许时显式 Err」实例面） | 新增 |
| git_test · merge 冲突显式失败与 abort 收口 | 异常 | 主仓与分支改同一文件 → Err 含冲突语境 + 手动处置引导文案；Err 返回后仓干净——`git status` 无 `UU` 态、无 MERGE_HEAD 在案（尽力 `merge --abort` 已执行——D9③）；主仓工作区文件内容与冲突前一致（零破坏——AC-8 字面） | 新增 |
| git_test · current_branch 两态 | 正向 | 分支居位 → 返回分支名；`git checkout --detach` → Err 含 detached HEAD 引导（D8 空输出映射） | 新增 |
| git_test · commit_paths 改名 pathspec | 正向 | 预置无关 staged 条目；目录改名（旧删新增）后 `commit_paths([old, new], msg)` → Ok 且新提交同时含旧路径删除与新路径新增（`/**` 形态覆盖已删除目录——D9②「裸目录 pathspec 报 did not match」的定锚反面）；无关 staged 前后 `git status --porcelain` 逐字一致（AC-4 机械断言）；`commit -- pathspec` 不受 index 干净度约束（既有 staged 不阻断提交） | 新增 |
| git_test · PATH 隔离 Err 面 | 异常 | PATH 隔离窗口（共享锁串行化、测毕恢复——crate 既有纪律）→ `merge_branch` / `commit_paths` / `branch_merged` Err 含「git 不可用」引导（`git()` 执行器既有面随族扩展） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 git 子进程 + tempfile 仓（产品硬依赖同口径；`git -C <root>` 显式寻址；装置沿既有 `init_repo` / `git` 断言助手扩展——worktree 侧分支铸造经 `add_worktree` 或直驱 `worktree add -b`）；PATH 隔离窗口沿 crate 自持 `TEST_PATH_LOCK` 互斥、测毕恢复 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/archive_flow/mod.rs -> packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs

<!-- 新增文件节。挂 AC-1（命令面拒绝与读面）、AC-6（命令层失败收敛）、AC-9
     （run 面隔离命令面负断言 + ArchiveSink 转译）、AC-7（回环摘要半边）。
     design D1 五命令 + `_with<R: Runtime>` 泛型测试缝（`change_flow_start_with`
     先例）；D3 正向互斥（读 ChangeFlowControl::snapshot）；D4 ArchiveSink
     （ChangeFlowSink 同构镜像）。 -->

#### 待测功能

- archive_flow_preflight(app, root, change) -> Option<ArchivePreflight>: blank root 早退 None 读语义；worktree 记录在场才 `vcs.current_branch` 出 merge_target；`ChangeFlowControl::snapshot` 出 run_active
- archive_flow_start(app, on_event: Channel<ArchiveUpdate>, root, change, sync_specs) -> Result<bool, String> + `archive_flow_start_with<R>` 泛型测试缝: blank 校验 → db 记录（未建档 / 已归档 Err）→ run 注册表在案显式拒绝 → `ArchiveControl::begin`（重入防护）→ 组合根装配（compose_turn + KernelWorkerPort + ArchiveSink + ProcessArchiveVcs）→ 订阅先行 → spawn 链 → 提前 resolve
- archive_flow_stop(app, root, change) -> Result<(), String>: 取消旗 + StopRegistry 终止当前会话（miss 幂等）
- archive_flow_state(app, root, change) -> Option<ArchiveSnapshot>: blank 早退 None
- archive_flow_watch(app, on_event, root, change) -> Result<(), String>: broadcast 补订（无在案 Ok 非错误）
- ArchiveSink: `RunEventSink` 实现——`RunUpdate::SessionEvent` 即时转译 `ArchiveUpdate::SessionEvent` 发布归档 broadcast + 同步会话槽；run 注册表与 run Channel 零写入

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · preflight 命令读面 | 正向 | MockRuntime app + 真实建档（active + 相位种子 + 产物树）→ `Some` 且字段面完整；worktree 记录 + 真实 git tempdir 主仓 → `mergeTarget` = 当前分支名（`ProcessArchiveVcs` 真件半边） | 新增 |
| mod_test · preflight blank 与 None 口径 | 边界 | blank root → None；未建档 / 已归档 → None（D6 读语义，不进库解析链路） | 新增 |
| mod_test · start 拒绝面 | 异常 | blank root / change 显式 Err；未建档 Err；已归档 Err；run 在案（`ChangeFlowControl::begin_run` 预登记）→ Err 呈现运行中原因且 `ArchiveControl::is_active` 保持 false（零登记——AC-1 正向互斥） | 新增 |
| mod_test · start 受理与事件流回环 | 正向 | legacy 建档（worktree=None）+ 主仓 active 树 tempdir + 无 delta specs + PATH 隔离 CLI 窗口 → Ok；Channel 收 Stage 序（SpecSync skipped → Commit / Merge skipped legacy → Seal passed → Finalize 状态）+ `Finished { summary }`（specs=none、archivedDir 日期前缀名）；db status=archived、目录改名落盘；终态后 `archive_flow_state` → None（除名） | 新增 |
| mod_test · agent 失败收敛与 run 面负断言 | 异常 | delta specs 在场 + PATH 隔离（CliMissing 合成收敛）→ SpecSync failed + `Finished { error }`；`ArchiveControl` 除名（state None）；全程 `ChangeFlowControl::snapshot` 恒 None、零 run 信封（run 面隔离的命令面负断言——AC-9） | 新增 |
| mod_test · 重入防护透传 | 异常 | `ArchiveControl::begin` 预登记 (root, change) 后 start → Err（链进行中重复点击——R8 防重入的命令面锚） | 新增 |
| mod_test · stop 命令 | 边界 | 预登记条目 + 已 publish 阶段后 stop → 取消旗置位（控制面观察）；无会话槽 miss 幂等 Ok；blank root / change → Ok 零副作用 | 新增 |
| mod_test · watch 补订 | 边界 | 无在案 → Ok 非错误；预登记 + publish 一信封后 watch → 该信封经 Channel 到达（broadcast 补订语义——重挂恢复面） | 新增 |
| mod_test · state 快照口径 | 边界 | 预登记 + publish 两阶段信封 → `Some`（stages 累积 / sessionId None）；除名后 → None；blank → None | 新增 |
| mod_test · ArchiveSink 转译 | 正向 | 直构 sink：`emit(RunUpdate::SessionEvent { .. })` → 归档 broadcast 收 `ArchiveUpdate::SessionEvent` + `control.current_session` 同步更新；`emit` 其他 `RunUpdate` 变体 → 归档面零信封（run 信封不外泄）且 `ChangeFlowControl::snapshot` None（run 注册表与 run Channel 零写入——D4 字面） | 新增 |
| mod_test · DTO 字段面 | 边界 | Channel 信封 JSON 出线：tag `ipc` 三变体、`ArchiveStage` 线词（preflight / specSync / commit / merge / seal / finalize）、camelCase 字段名（archivedDir / incompletePhases / runActive 等）——IPC 边界捕获（既有捕获型 Channel 装置同型） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / Tauri（进程边界） | MockRuntime mock app 四态托管：真实 WorkspaceStores（tempdir 数据根）+ `Arc<ChangeFlowControl>` + `Arc<StopRegistry>` + `Arc<ArchiveControl>`（与 main.rs 注入面同型——`change_flow` mod_test 装置扩展一态）；命令真实走 `*_with` 泛型缝；捕获型 / 丢弃型 Channel + `wait_for` 轮询既有装置沿用 | 本节全部用例 |
| CLI 引擎（进程边界） | PATH 隔离窗口（CliMissing 合成收敛——`isolate_path` 既有装置）：链真实驱动但不 spawn 真实 CLI；delta 缺席回环行零 agent 依赖（fs + vcs 半边即可收口） | 回环 / agent 失败收敛各行 |
| git 进程 | preflight merge_target 行用真实 git tempdir 仓；回环行 finalize 半边在 PATH 隔离下走 dirty=false 跳过路径（真件行归 git_test 节承载） | preflight 读面 / 回环行 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- 既有文件扩展节。挂 AC-9（反向互斥：归档进行中 run 发起被拒）。design D3：
     前置校验序列 +1 读 ArchiveControl::is_active——run 命令面唯一触点，walker /
     装配形态与其余五命令零改动。 -->

#### 待测功能

- change_flow_start_with<R: tauri::Runtime>: 前置校验序列 +1——`ArchiveControl::is_active(&root, &change)` 在案 → 显式 Err（归档进行中原因，不进入运行态零落账）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · 归档进行中 run 发起被拒 | 异常 | `ArchiveControl::begin` 预登记 (root, change) 后 `change_flow_start_with` → Err 含归档进行中原因；零 run 落账（`ChangeFlowControl::snapshot` None、库内零 StepRecord）；异 change 同 root / 异 root 同名 change 不误拒（复合键寻址——发起照常进入既有前置校验面） | 新增 |
| mod_test · 既有五命令面持衡 | 正向 | 发起 / 停止 / 应答 / 确认 / 快照 / 补订各既有行——前置序列 +1 不触既有断言面（适配注记：装置 manage 增 `Arc<ArchiveControl>` 一态后断言零改动） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / Tauri（进程边界） | 既有 MockRuntime 装置扩展 manage `Arc<ArchiveControl>` 一态；预登记直接经 `ArchiveControl::begin`（不依赖链运行时序） | 本节全部用例 |

### packages/desktop/src-tauri/src/bindings/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 既有文件扩展节。挂 AC-1（五命令出线）与 D5 DTO 家族线词；specta 生成物
     一致性守卫既有结构沿用；`bindings:check` 工具链门见不可测试项 3。 -->

#### 待测功能

- 五命令注册面: `archiveFlowPreflight` / `archiveFlowStart` / `archiveFlowStop` / `archiveFlowState` / `archiveFlowWatch`（camelCase 包装；start 携 Channel 参型）
- ArchiveUpdate 家族: tag `ipc` 三变体（stage / sessionEvent / finished）+ `ArchiveStage` / `ArchiveStageStatus` / `ArchiveStageState` / `ArchiveSpecsStatus` / `ArchiveSummary` / `ArchiveSnapshot` / `ArchivePreflight` 类型段与线词

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings_test · 五命令注册面 | 边界 | 生成 bindings 的 commands 对象含五命令（camelCase 名齐全；既有命令零删除零改名） | 新增 |
| bindings_test · ArchiveUpdate 家族线词 | 边界 | `ArchiveStage` 线词恰 preflight / specSync / commit / merge / seal / finalize 六值；`ArchiveStageStatus` 四值、`ArchiveSpecsStatus` 三值（synced / skipped / none）；`ArchivePreflight` 字段面（name / completed / incompletePhases / missingArtifacts / deltaSpecs / worktree / branch / mergeTarget / runActive）；`ArchiveSnapshot` 字段面（stages / sessionId）——D5 线格式锚 | 新增 |
| bindings_test · 既有守卫随动 | 边界 | 既有命令清单计数断言 +5 随动；重导幂等 / 篡改恢复各既有行持衡 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 export-bindings 管线产物文件读写断言（既有 file_lock 串行化装置沿用） | 本节全部用例 |

### packages/desktop/src/views/changes/flow/archive-state.ts -> packages/desktop/src/views/changes/flow/archive-state.test.ts

<!-- 新增文件节。挂 AC-7（摘要归并半边）与 D5 DTO 消费面。`run-state.ts` 同构
     纯 reducer——零 react 依赖直测。 -->

#### 待测功能

- ArchiveFlowState { stages: Record<ArchiveStage, ArchiveStageState>, liveEvents: Record<string, AgentEvent[]>, summary: ArchiveSummary | null, error: string | null }
- seedArchiveState(snapshot: ArchiveSnapshot | null): 重挂快照 → 视图模型初值（null → null 态）
- applyArchiveUpdate(state, update): Stage 后写覆盖同段 / SessionEvent 按会话缓存（seq 去重保序）/ Finished 收 summary 或 error + 终态冻结守卫

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive-state · Stage 归并 | 正向 | 同段 running 后随 passed / skipped（携 detail）/ failed → 后写覆盖（同段单槽终值）；六段互不覆盖 | 新增 |
| archive-state · SessionEvent 缓存 | 边界 | 按会话 id 分槽缓存；同 seq 事件只计一次（去重保序——live 与重放并流前提） | 新增 |
| archive-state · Finished 两态与冻结 | 边界 | `Finished { summary }` → summary 落态、error 保持 null；`Finished { error }` → error 落态；终态后迟滞信封（Stage / SessionEvent）一律忽略（不回滚运行史） | 新增 |
| archive-state · 快照种子 | 边界 | snapshot null → null；Some（stages 两段 + sessionId）→ 初值 stages 逐段一致 + sessionId 透传 + summary / error 空态 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯 reducer 内存直驱（DTO 字面量构造，零 react 零 IPC） | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-archive-flow.ts -> packages/desktop/src/views/changes/hooks/use-archive-flow.test.ts

<!-- 新增文件节。挂 AC-1（preflight 动作）、AC-6（停止）、AC-7（终态 refresh 回调）。
     `use-change-flow-run` 同构（D14）：Channel 惰性构造 + 快照恢复 + 终态回调。 -->

#### 待测功能

- useArchiveFlow({ root, change, onFinish }) -> { state, preflight, start, stop, error }: preflight 点击时取数（invoke archiveFlowPreflight）；start(syncSpecs)（Channel 构造 + invoke + 事件归并种子）；stop；快照恢复（archiveFlowState → seedArchiveState）+ 运行中补订（archiveFlowWatch）；终态一次 onFinish；invoke reject 统一 error 态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-archive-flow · preflight 动作 | 正向 | 调 preflight → invoke `archiveFlowPreflight` 以 (root, change) 透传，resolve 数据返回 | 新增 |
| use-archive-flow · start 与事件归并 | 正向 | start(true) → invoke `archiveFlowStart` 参数序（channel, root, change, true）；Channel 信封到达 → state 经 reducer 归并（stages 推进） | 新增 |
| use-archive-flow · 终态 onFinish 恰一次 | 边界 | `Finished` 信封到达 → onFinish 恰一次；此后迟滞信封不重复触发（refresh 面防抖——详情页回落已归档形态的回调锚） | 新增 |
| use-archive-flow · 快照恢复与补订 | 边界 | mount 时 state 查询 Some（非终态）→ seed + `archiveFlowWatch` invoke；None → 零 Channel 零 watch（常态路径零订阅） | 新增 |
| use-archive-flow · stop 与 error 面 | 异常 | stop → invoke `archiveFlowStop`；invoke reject → error 态（string 口径归一）；发起先行清错 | 新增 |
| use-archive-flow · 参数未就绪 no-op | 边界 | root / change null → 各动作零 invoke | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke + Channel（`use-change-flow-run.test` 既有 vi.hoisted 装置同构，resolve / reject 可切换）；reducer 真实组合不 mock | 本节全部用例 |

### packages/desktop/src/views/changes/flow/archive-panel.tsx -> packages/desktop/src/views/changes/flow/archive-panel.test.tsx

<!-- 新增文件节。挂 AC-1（确认对话）、AC-2（警告不阻断）、AC-5（转录入口）、
     AC-7（结果摘要）。design D13 对话形态 / D14 data-testid 族。 -->

#### 待测功能

- 确认对话: preflight 数据面渲染（完成度警告行 / 产物缺失警告行 / delta specs capability 清单 / worktree 合入告知 + 目标分支名 / runActive 拒绝卡替代确认）+「跳过 delta specs 同步，直接归档」checkbox（默认不勾、delta 缺席隐藏）+ 轻量确认（无警告）
- 进行面: 阶段清单（`archive-stage-<stage>` 六行）+ 停止按钮 + agent 转录入口（`useSessionTranscript` + `AgentTimeline` 直组、live 事件按会话过滤 seq 归并）
- 终态面: 结果摘要（`archive-summary` / `archive-specs-line` 三态 / 警告清单）与错误面（`archive-error`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| panel · 确认对话数据面 | 正向 | preflight fixture（未过相位清单 + 缺产物子集 + delta 两 capability + worktree + 分支名）→ `archive-confirm-dialog` 呈现；`archive-warning` 行逐条（未过相位清单行 / 产物缺失行）、delta 清单两 capability、合入告知含目标分支名（R4 防护面） | 新增 |
| panel · runActive 拒绝卡 | 异常 | fixture runActive=true → 拒绝卡呈现运行中原因；`archive-confirm-ok` 不渲染、零 start 调用（AC-1 scenario 字面） | 新增 |
| panel · 跳过同步 checkbox | 边界 | delta 缺席 → `archive-sync-skip` 隐藏；在场默认不勾 → 确认调 start(true)；勾选 → start(false)（skill "Archive without syncing" 对译——D13） | 新增 |
| panel · 轻量确认与取消 | 正向 | 全绿齐备 fixture → 无 `archive-warning` 行、确认 / 取消两按钮；取消 → 关闭且零 start（警告不阻断——AC-2 的 UI 半边：有警告时确认按钮同样可用） | 新增 |
| panel · 阶段清单推进 | 正向 | 归档进行态（state fixture：SpecSync passed / Commit running）→ `archive-stage-list` 六行在场、`archive-stage-specSync` / `archive-stage-commit` 状态与 detail 随 state 呈现 | 新增 |
| panel · 停止与转录入口 | 正向 | 进行态 → `archive-stop` 点击调 stop；sessionId 在场 → `archive-transcript` 呈现且 AgentTimeline 渲染转录事件（复用基建——零第二套时间线的渲染锚）；sessionId null → 入口不渲染 | 新增 |
| panel · 结果摘要三态 | 正向 | Finished summary fixture → `archive-summary` 呈现名 + 归档目录；`archive-specs-line` 三态文案（已同步 delta specs / 跳过 spec 同步（用户选择） / 无 delta specs——D12 对译）；warnings 清单逐条 | 新增 |
| panel · 错误面 | 异常 | Finished error fixture → `archive-error` 呈现错误串（merge 冲突 git 语境 / agent 失败语境两 fixture） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / hooks（进程边界） | `vi.mock` hooks/use-archive-flow（state / start / stop / error 经 mock 注入——面板纯呈现面）；`vi.mock` hooks/use-session-transcript（messages 注入）；AgentTimeline 真实渲染（既有装配先例） | 本节全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 既有文件扩展节。挂 AC-1（按钮两态 + panel 挂载）、AC-7（终态 refresh 回落
     已归档形态）。design D14：DetailHeader 增 `archive-trigger`；ArchivePanel 挂
     DetailHeader 与 RunControlPanel 之间；链终态一次显式 refresh。 -->

#### 待测功能

- DetailHeader 归档按钮: `detail.status === 'active'` 呈现（`archive-trigger`）；archived / 文档形态（status null）不渲染
- ArchivePanel 挂载: `selected !== null && detail.status === 'active'` 时渲染于 DetailHeader 与 RunControlPanel 之间（归档链与 run 面并置但组件隔离——DOM 序锚）
- 链终态 refresh: onFinish → 一次显式 refresh（detail 重查 → 回落已归档形态、按钮消失）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail-view · 按钮两态 | 正向 | detail fixture status='active' → `archive-trigger` 呈现；status='archived' → 不渲染；status=null（文档形态）→ 不渲染（负断言） | 新增 |
| detail-view · panel 挂载位 | 边界 | selected 在场 + status active → ArchivePanel 渲染且 DOM 序位于 detail-header 之后、run 控制面板之前；status 非 active → panel 不渲染 | 新增 |
| detail-view · 终态 refresh | 正向 | 触发 onFinish（mock hook 注入）→ detail 重查 invoke 恰一次、呈现回落已归档形态（`archive-trigger` 消失） | 新增 |
| detail-view · 既有行随动 | 正向 | 状态徽章 / worktree 信息行 / 两态分流 / 降级页各既有行——断言零改动持衡（新增挂载不触既有断言面） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | 既有 `vi.mock('@tauri-apps/api/core')` invoke + Channel 装置沿用（detail DTO fixture 注入）；归档面经 `vi.mock` hooks/use-archive-flow 注入 onFinish（refresh 触发锚） | 本节全部用例 |

---

## 不可测试项

1. **AC-10 版本交付静态半边**：`packages/desktop/package.json` 0.4.20 → 0.4.21（`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）、`plugins/dev-team` 保持 2.10.44 且 SKILL.md 原文未动 — **原因**: 静态版本声明无行为面；由 tasks 阶段五对账（`plugins/dev-team` 零改动复核）承载。
2. **crate 图与 grep 守线**：`vcs-runtime` workspace 依赖增 `orchestration`（agent-runtime 先例同构）且零 Tauri 零 tokio；`orchestration` 零依赖新增；git spawn 触点（merge / commit / pathspec 提交）仅 `crates/infra/vcs`；`ArchiveControl` 写触点仅归档链本体与 archive_flow 命令组 — **原因**: Cargo 依赖图与全源码 grep 属编译期 / 静态守线（tasks 阶段五明定），无进程内行为断言面；行为半边经 archive_flow_test 假件行（`ArchiveVcsPort` 为唯一缝——core 零 spawn 的类型面证据）与 mod_test run 面负断言行承载。
3. **AC-10 工具链门半边**：`server:check` / `client:check`（含 knip 零新增豁免）/ `bindings:check` / `vp test` 全绿 — **原因**: 工具链级套件门非单一测试文件可承载，归 test-execution 阶段承接验证（tasks 阶段五同口径）；bindings 出线行为面经 bindings/mod_test.rs 节承载。
4. **AC-5 LLM 实际增量合并的保真与幂等半边**（spec scenario「增量合并保真与幂等」的 agent 执行半边：主 spec 仅按增量语义变化、保留未提及内容、两次同步结果一致） — **原因**: spec 同步是智能段（proposal 硬点：「delta 合并需 LLM 判断，不能确定性编码」），假引擎不产生真实文件编辑、真实引擎行为非确定；测试面承载为——prompt 文本锚（增量语义四行 + 保留未提及 + 幂等指令 + 禁 MCP / `__TOOL_ASK_USER__` / workflow.json / git 逐字在场，archive_flow_test prompt 语义锚行）、会话通道锚（provenance / cwd / 转录经假 worker 捕获与 panel 转录行）、同步产物随提交合入的编排半边（全链正向行 `commit_all` 在 agent 之后）；真实合并质量属验收 / 集成面。路径沙箱囚禁为 compose / kernel 既有基建复用（零改动），归档链侧经 cwd 参数锚（假 worker `turn.root` 断言）承载。
5. **D3 TOCTOU 微秒窗口**（跨注册表 check-then-begin 的同刻双击竞态） — **原因**: 设计显式接受并留痕（R5 概率低；严封需跨注册表单锁，违 run 隔离红线），无确定性并发断言面；双向互斥的可测半边经 mod_test 两行（正向：run 在案拒归档；反向：归档在案拒 run）承载。
6. **门面 / 纯类型 / 生成物 / 装配模块不建独立测试文件**：`core/orchestration/src/port.rs` 的 `ArchiveVcsPort` trait 声明面、`orchestration/src/lib.rs` 与 `infra/vcs/src/lib.rs` re-export 与文档注释、`crates/infra/vcs/Cargo.toml` 依赖边、`src/main.rs` 的 `app.manage(Arc::new(ArchiveControl::new()))` 注入行、`src/commands/mod.rs` 的 `all_commands!` 注册行、`src/commands/changes/mod.rs` 组注释随动、`packages/desktop/src/types/generated/bindings.ts` 生成物 — **原因**: 纯 trait 声明 / re-export / 单行装配 / 生成物无自有行为；行为去向：ArchiveVcsPort 经 archive_flow_test 假件与 git_test 真件两节消费承载；manage 注入行经 commands/archive_flow mod_test（MockRuntime 四态托管同型）承载；注册行经 bindings 注册面行承载；`archive_change` 裸双写零改动经既有 changes mod_test 行持衡承载（AC-9 零回归）。
7. **golden 语料零触点注记**：`ChangeDetail` / `ChangeList` 投影零改动，corpus golden 与 fixtures 矩阵零重写零扩展 — **原因**: 本变更 DTO 演进只新增 IPC 命令与 `ArchiveUpdate` 家族（查询投影面零触点），proposal「若 DTO 演进触线」的条件不成立；`DESKTOP_GOLDEN_REWRITE=1` 流程不启用，留痕防实现期误触。
8. **run 面零改动组件的持衡声明**：walker / steps / snapshot / state / control / phase_next 及 run-control-panel / change-flow-graph 等既有测试零触点零适配 — **原因**: design 不变组件核对结论明定零 diff（run 面隔离红线）；「归档链零 RunUpdate / ConfirmWait 事件」的进程内负断言经 ArchiveSink 转译行（run 信封不外泄 + ChangeFlowControl 零写入）与 mod_test run 面负断言行承载；run 收口「Ready for archiving」文案逐字不变由既有 walker / 面板行零 diff 保证。
9. **环境依赖面**：真实 git 在测试环境的可用性（vcs 真件与命令层真件行前提）、Windows / Unix 平台分支互斥覆盖、R7 长路径与引号形态 — **原因**: 与产品硬依赖同口径的环境前提（git 缺失路径本身有 PATH 隔离 Err 行覆盖）；提交信息经 argv 直传零 shell 包装（D9⑤ 留痕——引号形态不适用本族），经 commit_all 信息保真行正面锚定；平台分支经 CI 矩阵承载（单平台单测只触达本机分支）。
