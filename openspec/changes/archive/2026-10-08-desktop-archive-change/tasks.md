# 任务: desktop-archive-change

> **变更**: desktop-archive-change
> **依据**: proposal.md + design.md（D1–D16 决策编号见 design）

任务边界：本列表只含实现任务；测试编写（archive_flow_test 假件直驱 / git_test 真实 git 夹具扩族含 AC-4 机械断言 / 命令面回环 / 前端 vitest）、bindings 复核由 test-design / test-gen / test-execution 阶段承接。任务内引用的文件均在 design.md 变更清单内；`plugins/dev-team` 与 `openspec/specs/**` 为只读红线零触点。

## 阶段一：vcs 归档子命令族与 port 缝

- [x] `crates/infra/vcs/src/git.rs`：`git()` 执行器改 `pub(crate)`；新增归档子命令族（全部 `git -C <root>` 同步 spawn、argv 直传零 shell 包装）——`dirty(root, paths)`（`status --porcelain [-- pathspec…]` 非空即真，空 paths = 全域）、`commit_all(worktree, message)`（`add -A` + `commit -m`）、`branch_merged(main_root, branch)`（`merge-base --is-ancestor <branch> HEAD`，退出 0/1 映射 bool、>1 Err）、`merge_branch(main_root, branch)`（`merge --no-edit`；非零退出先尽力 `merge --abort`（幂等，非 merge 态无害）再 `Err` 携 git stderr 语境与手动处置引导）、`current_branch(main_root)`（`branch --show-current`，空输出 = detached HEAD 显式 Err 引导）、`commit_paths(main_root, paths, message)`（`add -A -- <paths…>` + `commit -m <msg> -- <paths 各自附加 "/**" 形态>`——删除路径覆盖所需，design D9② 定锚）；`ProcessArchiveVcs` 无状态执行器 `impl ArchiveVcsPort`（组合根按需构造，`ProcessWorktree` 同型）
- [x] `crates/infra/vcs/src/lib.rs`：导出 `ProcessArchiveVcs`；crate 文档注释随动（归档提交 / 合入 / pathspec 提交入 vcs 工作面词汇）；`crates/infra/vcs/Cargo.toml` 增 `orchestration` workspace 依赖（agent-runtime → orchestration 先例同构，零 Tauri 零 tokio 不变）
- [x] `crates/core/orchestration/src/port.rs`：`pub trait ArchiveVcsPort: Send + Sync` 六方法（sync 签名零 tokio，`WorktreePort` 同纪律；`Err` 面为带引导文案的 String，调用方直接呈现），文档注释注明消费者归属（归档链）与 crate-layout delta 授权落位；`crates/core/orchestration/src/lib.rs` re-export

## 阶段二：归档编排链运行时（orchestration）

- [x] `crates/core/orchestration/src/archive_flow.rs`（新）：DTO 面（`ArchiveStage` / `ArchiveStageStatus` / `ArchiveStageState` / `ArchiveSpecsStatus` / `ArchiveSummary` / `ArchiveUpdate`（tag `ipc`）/ `ArchiveSnapshot` / `ArchivePreflight`——serde camelCase + specta `Type`，驻本模块，state.rs 零触碰，design D5）；`ArchiveControl` 注册表（复合键 (String, String)，`begin`（在案 Err = 发起重入防护）/ `publish` / `set_session` / `current_session` / `is_active` / `snapshot` / `subscribe` / `request_stop`）+ `ArchiveGuard`（取消旗 + `finish` 除名，`RunGuard` 同型；tokio sync 原语）；完成度谓词私有镜像（`phase_table` 全相位非 stale pass/skipped，注释锚 `phase_next::has_phase_passed` 同语义，design D6）；警告 / 摘要词汇单点常量（design D12）；`spec_sync_prompt(change)` 单点模板（design 数据模型逐字）；`preflight(&layout, store, worktree, change, vcs, run_active)` 读面聚合（含产物三件 fs 核对与 delta specs capability 清单）
- [x] `archive_flow.rs` 续：`run_archive_flow(worker, vcs, store, control, guard, request)` async 阶段机（design「阶段机」定形逐段实现）：preflight 重校验（建档 + status=active + run 在案拒 + worktree 缺失且未合入 Err 引导）→ specSync（delta 在场且 sync_specs 才发起 agent 会话：provenance `source="change"` / `source_ref="<change>/archive/spec-sync"` / role=Executor / cwd 按 D7（record.worktree 在场且 is_dir → worktree，否则主 root）/ permission=BypassPermissions；`outcome.status` 非 Completed → failed 停链）→ commit（worktree 记录在场才执行：branch_merged → skipped；worktree 目录缺失 → Err；dirty → commit_all；干净 → skipped）→ merge（worktree 记录在场才执行：branch_merged → skipped；否则 merge_branch）→ seal（`workflow::write::archive(&resolve(main_root), store, change)` 既有双写单点直调，毫秒级内联，D15）→ finalize（`dirty(main_root, [old, new])` 为真才 `commit_paths`，pathspec 相对 POSIX 串自 `foundation::layout::domain_dir_name()` 单点拼，D11；假 → skipped 已落盘）；每阶段 `Stage` 事件 running→终态先行发布、失败停该阶段并发 `Finished { error }`；阶段间取消旗检查点；成功收口 `Finished { summary }`（名 / 归档目录 / specs 三态 / 警告清单）；legacy（worktree=None）commit / merge 整段 skipped；`ArchiveGuard::finish` 终态除名
- [x] `crates/core/orchestration/src/lib.rs`：挂 `pub mod archive_flow` + re-export（`ArchiveControl` / `ArchiveVcsPort` 经 port.rs / DTO 面 / `run_archive_flow` / `ArchiveRequest`）；零依赖新增核对（agent / foundation / serde / specta / tokio / workflow 全既有）

## 阶段三：命令组、互斥与壳层装配

- [x] `src/commands/archive_flow/mod.rs`（新）：五命令薄包装（三件事纪律 `Result<T, String>`）——`archive_flow_preflight(root, change) -> Option<ArchivePreflight>`（blank root 早退 None 读语义；读 `ChangeFlowControl::snapshot` 出 run_active；worktree 记录在场才 `vcs.current_branch` 出 merge_target）；`archive_flow_start(root, change, sync_specs, on_event: Channel<ArchiveUpdate>) -> Result<bool, String>` async + `_with<R: Runtime>` 泛型测试缝：blank root / change 显式 Err → 读 db 记录（未建档 / 已归档 Err）→ run 注册表在案显式拒绝（运行中原因）→ `ArchiveControl::begin`（在案 Err = 重入防护）→ 组合根装配（`compose_turn`（store 半边注入 workspace root 实例，`change_flow_start` 同式）+ `KernelWorkerPort::new(composed, ArchiveSink)` + `ProcessArchiveVcs` + store port）→ 订阅先行 `spawn_channel_forward` → `tauri::async_runtime::spawn(run_archive_flow…)` → 提前 resolve；`ArchiveSink`（`ChangeFlowSink` 同构镜像：`RunEventSink` 实现把 `RunUpdate::SessionEvent` 即时转译 `ArchiveUpdate::SessionEvent` 发布归档 broadcast + `set_session` 同步会话槽——run 注册表与 run Channel 零写入，design D4）；`archive_flow_stop(root, change)`（取消旗 + StopRegistry 终止当前会话，miss 幂等）；`archive_flow_state(root, change) -> Option<ArchiveSnapshot>`（blank 早退 None）；`archive_flow_watch(root, change, on_event)`（broadcast 补订，无在案 Ok 非错误）
- [x] `src/commands/mod.rs`：`all_commands!` 注册五命令（changes / change_flow 之后归组）
- [x] `src/commands/change_flow/mod.rs`：前置校验序列 +1——`ArchiveControl::is_active(&root, &change)` 在案 → 显式 `Err`（呈现归档进行中原因，不进入运行态零落账，design D3 反向互斥；walker / 装配形态与其余五命令零改动）
- [x] `src/commands/changes/mod.rs`：组注释三分随动（读 + 记录面 / run 编排控制 / 归档编排流——至多注释面，`archive_change` 本体零改动）
- [x] `src/main.rs`：setup 内 `app.manage(Arc::new(orchestration::archive_flow::ArchiveControl::new()))`（与 StopRegistry / ChangeFlowControl 同型托管，注释随动）

## 阶段四：前端入口与归档面板

- [x] `packages/desktop/src/views/changes/flow/archive-state.ts`（新）：`ArchiveFlowState { stages: Record<ArchiveStage, ArchiveStageState>, liveEvents: Record<string, AgentEvent[]>, summary: ArchiveSummary | null, error: string | null }` + 纯 reducer `applyArchiveUpdate` / `seedArchiveState`（`run-state.ts` 同构；Stage 后写覆盖同段、SessionEvent 按会话缓存、Finished 收 summary / error）
- [x] `packages/desktop/src/views/changes/hooks/use-archive-flow.ts`（新）：`useArchiveFlow({ root, change, onFinish })`——preflight 动作（点击时取数）、start（syncSpecs 参）、stop、Channel 惰性构造 + 订阅至终态、`archive_flow_state` 快照恢复 + `archive_flow_watch` 补订、终态一次 `onFinish`（详情页 refresh 回落已归档形态）；invoke reject 面统一 error 态（`use-change-flow-run` 同构）
- [x] `packages/desktop/src/views/changes/flow/archive-panel.tsx`（新）：确认对话（preflight 数据面：完成度警告行 / 产物缺失警告行 / delta specs capability 清单 / worktree 合入告知 + 目标分支名 /「跳过 delta specs 同步，直接归档」checkbox 默认不勾且 delta 缺席隐藏 / runActive 拒绝卡替代确认；确认 → start，design D13）→ 进行面（阶段清单 `archive-stage-<stage>` 六行 + 停止按钮 + agent 转录入口 `useSessionTranscript(sessionId)` + `AgentTimeline` 直组、live 事件按会话过滤 seq 归并，design D14）→ 终态面（结果摘要：名 / 归档目录 / specs 行三态 `archive-specs-line` / 警告清单 `archive-summary`；错误面 `archive-error`）；data-testid 族按 design D14 清单
- [x] `packages/desktop/src/views/changes/change-detail-view.tsx`：`DetailHeader` 增归档按钮（`detail.status === 'active'` 呈现 `archive-trigger`；archive / 文档形态不渲染）；`ArchivePanel` 挂 DetailHeader 与 RunControlPanel 之间（`selected !== null && detail.status === 'active'`）；链终态触发一次显式 refresh
- [x] `packages/desktop/src/types/generated/bindings.ts`：经 `pnpm -C packages/desktop run bindings:export` 再生（五命令 + ArchiveUpdate 家族 DTO）
- [x] `packages/desktop/package.json` `version` 0.4.20 → 0.4.21（design D16：`tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）

## 阶段五：守线收口（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）零错误；crate 图审查：`vcs-runtime` workspace 内依赖 = `workflow` + `foundation` + `orchestration`（新增）且零 Tauri 零 tokio；`orchestration` 零依赖新增；core 各 crate 无 `store` / `vcs-runtime` 依赖
- [x] run 面隔离自查：`walker.rs` / `steps.rs` / `snapshot.rs` / `state.rs`（RunUpdate / ChangeRunSnapshot / ChangeRunStatus）与 `control.rs`（ChangeFlowControl）零 diff；`phase_next.rs` 及相位机写操作零 diff；`write/archive.rs` 与 `WorktreePort` 六方法零 diff（注释面除外）；grep 全源码——`ArchiveControl` 写触点仅 archive_flow 命令组与归档链本体，run 注册表零写入；grep git spawn 触点（merge / commit）仅 `crates/infra/vcs`
- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）零错误且 knip 零新增豁免；`pnpm -C packages/desktop run bindings:check` 绿
- [x] 变更清单对账：实现文件与 design.md 变更清单逐项对账（无清单外改动、无清单内遗漏）；`plugins/dev-team` 零改动复核（版本 2.10.44、SKILL.md 原文未动、三类交付产物未动）；AC-10 版本交付 0.4.21 已执行
