# 设计: desktop-archive-change

> **变更**: desktop-archive-change
> **日期**: 2026-10-08

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（desktop-change-archive 新增 + worktree / state-store / crate-layout 三修订 delta）已由提案阶段定稿；本设计不重复其内容，只在其八项「待决问题」之上定稿（见「关键设计决策」），并对归档链涉及的 git 语义以**真实 git（2.54）tempdir 实验定锚**（D9）——三处行为（ff / non-ff 对无关 staged 的容忍差、pathspec 提交对删除路径的 `/**` 形态要求、冲突后 `--abort` 收口）是方案可行性的前提，实验结论直接入设计。

版本基准注记：spec「版本交付」行 `0.4.20 → 0.4.21` 与主仓现状一致，直接执行；`plugins/dev-team` 2.10.44 零改动（SKILL.md 为只读参考源）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| ArchiveVcsPort 缝（新） | 归档链 → vcs 执行的进程内缝（消费者 = 归档链，crate-layout delta 明示「允许落 core/orchestration」） | `crates/core/orchestration/src/port.rs`（扩展） | crate 内 std | sync trait 六方法（D8）；port 属消费者、spawn 不进 core |
| git 归档子命令族（扩展） | 干净探测 / 全域提交 / 祖先判定 / 合入 / 当前分支 / pathspec 提交的进程执行 + `ProcessArchiveVcs` 装配 | `crates/infra/vcs/src/git.rs`（扩展）、`lib.rs`（导出 + 文档随动）、`Cargo.toml`（增 `orchestration`——agent-runtime 依赖 orchestration 先例同构） | orchestration（port 类型） | `git -C` 同步 spawn（D9 语义定锚）；零 tokio 零 Tauri |
| 归档编排链运行时（新） | 阶段机（校验 → spec 同步 → worktree 提交 → 主仓合入 → 双写收口 → 归档落盘）；每阶段幂等跳过、失败停该阶段、stop 检查点 | `crates/core/orchestration/src/archive_flow.rs`（新）、`lib.rs`（挂模块 + re-export） | workflow（write::archive / queries::locate_change / phase_table / state）、foundation、agent、serde/specta、tokio sync | async fn（std future）+ sync port 内联调用（D15）；零依赖新增 |
| 归档 agent prompt 单点 | skill 步骤 2 语义桌面化模板 + change 名插值（零 MCP / `__TOOL_ASK_USER__` / workflow.json 依赖） | `archive_flow.rs` 内 `spec_sync_prompt`（D4） | — | const 模板 + format 插值 |
| ArchiveControl 注册表 | 归档链进行态登记（复合键 (workspace root, change)）：broadcast、会话槽（停止寻址）、取消旗、快照 | `archive_flow.rs` 内（control.rs 零改动） | tokio sync | 镜像 `ChangeFlowControl` 形态的独立注册表（D3）；终态除名 |
| ArchiveUpdate / DTO 面 | 阶段状态 / 会话事件 / 终态信封 + 快照 / preflight / 结果摘要（serde camelCase + specta `Type`，IPC 直用） | `archive_flow.rs` 内（state.rs 零改动——run 事件面隔离） | specta | D5 |
| WorkerAgent 通道（复用零改动） | 归档 agent 会话执行：`compose_turn` 新会话 + StopRegistry 终止 + 密封转录 | `crates/infra/agent/src/worker.rs`（`KernelWorkerPort` 原样复用） | — | provenance 定式 D4；命令层以 `ArchiveSink` 转译事件（D2） |
| 写面 archive（复用零改动） | 双写单点收口：改名 + 翻转 + 续半边 | `crates/core/workflow/src/write/archive.rs` | — | 归档链末段唯一收口触点（至多注释面随动） |
| 归档链 IPC 命令组（新） | 五命令薄包装：`archive_flow_preflight` / `_start`（提前 resolve + Channel）/ `_stop` / `_state` / `_watch`；三件事纪律 `Result<T, String>` | `src/commands/archive_flow/mod.rs`（新） | orchestration、agent-runtime、vcs-runtime、store | D1；组合根装配（compose_turn + KernelWorkerPort + ArchiveSink + LocalToolSteps 式 store 注入） |
| ArchiveSink（命令层桥） | `RunEventSink` 实现：worker.rs 复用通道的 `RunUpdate::SessionEvent` 即时转译为 `ArchiveUpdate::SessionEvent` 发布归档 broadcast + 同步会话槽（run 注册表与 run Channel 零写入） | `src/commands/archive_flow/mod.rs` 内 | — | `ChangeFlowSink` 同构镜像（D4） |
| run 发起互斥前置 | `change_flow_start` 前置校验序列 +1：归档链进行中（ArchiveControl 在案）显式拒绝 | `src/commands/change_flow/mod.rs`（仅此一处） | — | run 状态机 / walker / 其余控制命令零改动（D3） |
| 壳层注入 | `app.manage(Arc::new(ArchiveControl::new()))` | `src/main.rs` | — | Tauri State（与 StopRegistry / ChangeFlowControl 同型托管） |
| 前端归档状态归并 | `ArchiveUpdate` 纯 reducer（阶段覆盖 / 实时事件缓存 / 摘要 / 错误） | `packages/desktop/src/views/changes/flow/archive-state.ts`（新） | bindings | `run-state.ts` 同构 |
| 前端归档 hook | invoke 五命令 + Channel 订阅生命周期 + 快照恢复 + 终态 refresh 回调 | `packages/desktop/src/views/changes/hooks/use-archive-flow.ts`（新） | bindings | `use-change-flow-run` 同构（D14） |
| ArchivePanel 组件 | 确认对话（警告清单 + delta specs 清单 + worktree 合入告知 + 目标分支名 + 跳过同步 checkbox）→ 阶段清单 → agent 转录入口（复用 `useSessionTranscript` + `AgentTimeline`）→ 停止 / 错误 / 结果摘要 | `packages/desktop/src/views/changes/flow/archive-panel.tsx`（新） | bindings | D13 / D14；data-testid 挂钩 |
| 详情页入口 | `DetailHeader` 增归档按钮（`detail.status === 'active'` 呈现；archive / 文档形态不渲染）+ panel 挂载 + 链终态一次显式 refresh | `packages/desktop/src/views/changes/change-detail-view.tsx` | — | D14 |
| 类型跟随 | 新命令与 DTO 再生 | `packages/desktop/src/types/generated/bindings.ts` | export-bindings 管线 | `bindings:check` 守卫 |
| 版本交付 | `packages/desktop` 0.4.20 → 0.4.21 | `packages/desktop/package.json` | — | D16 |

**不变组件（零触点核对结论）**：`orchestration/src/walker.rs` / `steps.rs` / `snapshot.rs` / `state.rs`（`RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` 逐字不动）；`orchestration/src/control.rs`（`ChangeFlowControl` 零方法新增——归档侧只读消费既有 `snapshot`）；`workflow/src/write/phase_next.rs` 及全部相位机写操作（完成度谓词在归档链内私有镜像，D6）；`workflow/src/write/worktree.rs`（`WorktreePort` 六方法零改动）；`workflow/src/write/archive.rs` 双写语义；`SessionRecord` / 会话转录 / resume 链；`crates/infra/agent/src/worker.rs`；既有五条 `changes` 组命令与六条 `change_flow` 组命令签名（`change_flow_start` 仅前置序列 +1）；`plugins/dev-team` 全部；`openspec/specs/**` 基线。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | IPC 命令面形态 | **新归档链命令组** `src/commands/archive_flow/`（五命令：`archive_flow_preflight` / `archive_flow_start` / `archive_flow_stop` / `archive_flow_state` / `archive_flow_watch`）；既有裸双写 `archive_change` **保留零改动**（含 merge-first 引导与续半边语义，AC-9 零回归；互斥不圈裸命令——V1 边界，用户自担）。命令组三分注释随动：changes（读 + 记录面）/ change_flow（run 编排控制）/ archive_flow（归档编排流，带 agent 会话的第三面） | 升级 `archive_change` 为链入口会替换裸双写语义（db-inspector 级用户失去纯双写面，违 AC-9「零回归」）；归档链与 run 同级是编排流而非记录面，changes 组装不下（compose_turn + Channel + spawn） |
| D2 | 归档链状态呈现载体 | **Channel 同构流**：`archive_flow_start(root, change, sync_specs, on_event: Channel<ArchiveUpdate>)` 提前 resolve（接受幂等重入防护）+ 阶段 / 会话事件 / 终态经 Channel 流出；`archive_flow_state` 快照（重挂恢复）+ `archive_flow_watch` 补订——`change_flow_start` / `agent_start` 双先例的合体形态。「已成功阶段不重复执行」不靠持久化状态机，靠**全阶段幂等**（干净跳过提交、祖先已合跳过合入、agent 同步幂等、双写续半边、落盘提交脏探测跳过）——重试快速空走到未完成阶段，语义等价且零新增持久面 | agent 同步分钟级（R8）：一次性 await 无进度面必致重复点击；轮询是第二套拉取面。幂等路线使归档链零磁盘自有状态（一切事实在 git / db / 磁盘三单点内），可重入性可机械验证 |
| D3 | 互斥登记载体与双向校验 | **ArchiveControl 自持注册表**（`archive_flow.rs` 内，复合键 (workspace root, change)，`begin` 在案即拒绝重入 = 发起幂等防护）；正向：`archive_flow_start` 读 `ChangeFlowControl::snapshot` 在案 → 显式拒绝「运行中」；反向：`change_flow_start` 前置校验序列 +1 读 `ArchiveControl::is_active` → 显式拒绝「归档进行中」（spec scenario 实现位，run 命令面唯一触点）。遗留注记：双向检查为 check-then-begin，跨注册表 TOCTOU 微秒窗口理论上存在（用户同刻双击两侧），V1 接受并留痕（R5 概率低；严封需跨注册表单锁或互嵌临界段，属 run 注册表语义改动，违隔离红线） | 复用 run 注册表会把归档态混入 run 状态面（隔离红线）；run 注册表 / walker 零改动的代价是窗口接受——与 spec「互斥登记载体由 design 定稿」的授权一致 |
| D4 | worker.rs 复用形态、provenance 定式与 prompt 归属 | 归档 agent 经 `KernelWorkerPort` 原样执行（命令层 `compose_turn` 新会话 + `ArchiveSink`）；**provenance：`source="change"`、`source_ref = "<change>/archive/spec-sync"`**（无 attempt 段——链不持久化 attempt 计数；重试各会话共享同 ref，provenance 反查按 ref 聚合同步全史，符合既有 `agent_sessions` 按 ref 过滤面）；`WorkerTurnRequest.role` 复用 `WorkerRole::Executor`（该字段对 port 不透明——`KernelWorkerPort` 不读 `turn.role`，role→步映射是 walker 内部消费，归档链不进 run 步词汇，零枚举扩展）。`RunUpdate::SessionEvent` 仅作 worker.rs 既有签名的**内部载体**：`ArchiveSink` 即时转译为 `ArchiveUpdate::SessionEvent` 发布归档 broadcast——run 注册表与 run Channel 零写入（「零 RunUpdate 事件」红线按其 scenario 语义执行：run 状态通道零新事件）。prompt 单点驻 `archive_flow.rs`（`spec_sync_prompt`）——prompt.rs 是相位 run 三角色家族，归档 prompt 非相位面，独立模块自持 | Module Contract 指定 worker.rs 复用（转录密封 + final 提取不复制）；旁路自驱 `ComposedTurn` 会复制事件收集逻辑。source_ref 带 attempt 段无消费者且需持久计数，弃 |
| D5 | DTO 面（ArchiveUpdate 家族） | 见「数据模型」：`ArchiveStage`（preflight / specSync / commit / merge / seal / finalize 六段）、`ArchiveStageStatus`（running / passed / skipped / failed）、`ArchiveStageState { stage, status, detail? }`（skipped 的 detail 携带跳过因：干净 / 已合入 / 无 delta specs / 用户跳过 / 已落盘）、`ArchiveSpecsStatus`（synced / skipped / none）、`ArchiveSummary { name, archivedDir, specs, warnings }`、`ArchiveUpdate`（tag `ipc`：`Stage` / `SessionEvent` / `Finished { summary?, error? }`）、`ArchiveSnapshot { stages, sessionId }`（终态除名，与 run 快照同期语义）、`ArchivePreflight`（见 D6/D13）。serde camelCase + specta `Type`，驻 `archive_flow.rs`（state.rs 零触碰） | 与 `RunUpdate` 家族同构但独立类型——归档链阶段词汇与 run 步词汇不共枚举（隔离红线在类型面落地）；快照只覆盖运行期，终态由 db / 磁盘事实承载（详情页 refresh 回归已归档形态） |
| D6 | 完成度核对载体与 preflight 读面 | `archive_flow_preflight(root, change) -> Option<ArchivePreflight>`（`None` = 不可归档：未建档 / 已归档 / 未知名——前端据此不呈现入口路径的兜底）。核算：`phase_table(record.workflow_type)` 全相位以「非 stale 的 pass / skipped 条目在位」判定（`has_phase_passed` 同语义谓词在 `archive_flow.rs` **私有镜像**——相位机零改动红线；等价性测试经 `phase_next` 公开产出 `done` 旗在同一组 PhaseRecord 夹具上佐证）；workflow_type 不受支持（table None）→ `completed=false` + 专项警告。`ArchivePreflight { name, completed, incompletePhases, missingArtifacts, deltaSpecs, worktree, branch, mergeTarget, runActive }`——产物核对 = `locate_change` 定位目录内 `proposal.md` / `design.md` / `tasks.md` 三件在场（requirement 工作流定式，skill 例子同面；markdown_doc 注册表现役名单同源）；`deltaSpecs` = 定位目录 `specs/` 子树非空的 capability 清单；`mergeTarget` = `vcs.current_branch`（worktree 记录在场才探测）；`runActive` = run 注册表在案（对话呈现拒绝因）。MUST NOT 读取 workflow.json / 调 MCP（双向墙） | 谓词镜像 vs 导出：导出触碰相位机文件（红线），镜像 5 行 + 等价测试两头兼顾；preflight 把「确认对话数据面」与「链执行」解耦——确认面零 Channel 依赖、按钮点击到对话呈现是纯读路径 |
| D7 | delta specs 探测与同步 cwd | 探测（preflight 与链内同源）：`locate_change(layout, record.worktree, change)`（主仓优先、worktree 回退，与产物读取同位）的 `specs/` 子树非空。同步会话 **cwd = `record.worktree` 在场且 `is_dir` → worktree 绝对路径；否则主 workspace root**（spec 定式）。角落留痕：已合入且 worktree 已被手动清理后的补同步 → cwd 落主 root，同步产物落主仓未提交区（用户手动提交——与 legacy 提交面同语义，V1 边界） | 探测与产物读取同位避免第二套定位链；cwd 按**记录**而非定位结果（已合入后定位命中主仓、worktree 仍活时，同步须仍在 worktree 内做——产物随本 change 分支再合入，主仓 aftermath 最小） |
| D8 | vcs port 缝落位与方法集 | **`ArchiveVcsPort` 驻 `core/orchestration/src/port.rs`**（消费者 = 归档链；crate-layout delta 明示允许；`WorktreePort` 六方法零改动）；实现 `ProcessArchiveVcs` 驻 `infra/vcs/src/git.rs`，复用 crate 内 `git()` 执行器（`pub(crate)` 化即可）。六方法（sync 签名零 tokio）：`dirty(root, paths: &[&str]) -> bool`（`status --porcelain [-- pathspec…]` 非空；空 paths = 全域；gitignore 面不计——bootstrap 的 node_modules 天然不脏不进提交）、`commit_all(worktree, message)`（`add -A` + `commit -m`）、`branch_merged(main_root, branch) -> bool`（`merge-base --is-ancestor <branch> HEAD`，退出 0/1 映射，>1 Err）、`merge_branch(main_root, branch)`（`merge --no-edit`；失败尽力 `merge --abort` 后 Err 带 git 语境 + 手动引导）、`current_branch(main_root) -> String`（`branch --show-current`，空输出 = detached HEAD → Err 引导）、`commit_paths(main_root, paths, message)`（`add -A -- <paths…>` + `commit -m <msg> -- <paths 各自 `/**` 形态>`；内部合成 glob——裸目录 pathspec 对已删除目录报 "did not match"，D9 定锚） | 消费者归属纪律（checks 先例：`StaticCheckRunner` 驻 orchestration、spawn 落 infra/checks）；vcs 增 `orchestration` 依赖有 agent-runtime → orchestration 先例，crate 图机械规则（infra 无 Tauri）不破 |
| D9 | git 语义定锚（真实 git 2.54 tempdir 实验验证，测试形态同源） | ① **无关 staged 条目**：ff 合入容忍且原样保留 staged（`A other.txt` 存活）；**non-ff（merge commit）拒绝**（"Your local changes to the following files would be overwritten by merge"，退出 2）——这是 state-store delta「主仓 git 状态不允许时显式 Err」的实例面：链停合入段 + 引导（stash / commit 后重试），**不吞并不破坏恒成立**；AC-4 成功路径取 ff 形态（主仓未前进）构造，无关未提交文件（untracked / unstaged）两种合入形态下均原样保留。② **pathspec 提交**：`git add -A -- <old> <new>` + `git commit -m <msg> -- "<old>/**" <new>/**` 提交改名且无关 staged 原样保留；裸目录 pathspec 对已删除目录报错（`/**` 形态覆盖删除路径——实验反复定锚）；`commit -- pathspec` 不受 index 干净度约束。③ **冲突**：退出 1 + `UU` 态，`git merge --abort` 恢复干净（幂等，非 merge 态调用无害）。④ **祖先判定**：`merge-base --is-ancestor` 退出 0 = 已合入。⑤ **worktree 全域提交**：`add -A` 尊重 .gitignore（bootstrap 产物不入库）；untracked 新文件（spec 同步产物）入 `??` 面、进提交。提交信息经 argv 直传（无 shell 包装——0.4.18 raw_arg 引号教训不适用本族，R7 留痕） | 三处行为是方案前提：若 non-ff 也吞 staged 则 AC-4 需预检拦截设计；若 pathspec 裸目录可用则 `commit_paths` 形态更简。实验定锚后按最小面定形，测试夹具（真实 git tempdir）逐条复刻 |
| D10 | 提交信息派生与合入形态 | 提交信息 = **固定前缀 + change 名，不取 goal**（goal 多行长文本不适合单行信息）：worktree 提交 `archive: <name>`；归档落盘提交 `archive: move <name> to archive`。合入 = `git merge --no-edit change/<name>`（**默认形态**：可 ff 则 ff、主仓前进则 merge commit，信息用 git 默认 `Merge branch 'change/<name>'`） | `--no-ff` 弃（无谓 merge commit）；`--ff-only` 弃（主仓前进即虚假失败，与幂等续走相性差）；合入信息自铸无增益 |
| D11 | 归档收口后主仓改名落盘 | **自动 pathspec 提交**（proposal 倾向，AC-4 已约束）：finalize 段在双写收口后执行——脏探测 `dirty(main_root, [old, new])` 为真 → `commit_paths`（pathspec 自 `foundation::layout::domain_dir_name()` 单点拼相对 POSIX 串：`openspec/changes/<name>` 与 `openspec/changes/archive/<date>-<name>`）；为假 → 跳过（detail「已落盘」——用户手动提交过或重复收口的重试幂等面）。失败（git 语境）显式停等，重试时 seal 走续半边、finalize 幂等收口 | 自动落盘使归档后主仓 git 面即收即净（「完成即归档」语义完整性）；pathspec 纪律机械保证不吞并（D9 ②）；脏探测守卫消解「重试时改名已提交 → pathspec 空匹配报错」的幂等破口 |
| D12 | 警告与摘要词汇单点 | 词汇铸造收 `archive_flow.rs` 单点（测试断言锚）：完成度 `工作流未全部通过：<未过相位清单>`；workflow_type `工作流类型 "<type>" 无相位表，完成度不可核算`；产物 `缺少产物文档：<proposal.md/design.md/tasks.md 子集>`；合并告知 `将提交 worktree 并合入主仓当前分支：<branch>`；specs 行三态 `已同步 delta specs` / `跳过 spec 同步（用户选择）` / `无 delta specs`；停止收敛 `归档链已停止`。**无 delta specs 时摘要 specs 行照常呈现**（"No delta specs" 对译——三态枚举天然承载，不设缺席分支） | 文案单点使断言与 UI 不漂移（`CreateOutcome.warnings` 先例）；skill Output On Success 面对译齐全（名 / 位置 / specs 行 / 警告） |
| D13 | 确认面对话形态 | 点击归档 → `preflight` → 对话呈现：完成度结论（未过相位清单警告行）、产物缺失警告行、delta specs capability 清单（在场时）、worktree 记录在场时合并告知 + 目标分支名（R4）、`runActive` 时以拒绝卡替代确认按钮（呈现运行中原因，AC-1）。可选项：**「跳过 delta specs 同步，直接归档」checkbox（默认不勾）**——skill "Archive without syncing" 对译，勾选即 `sync_specs=false`（delta 在场才有意义，缺席时隐藏）；无警告时确认为轻量确认（「确认归档」/「取消」）。警告一律不阻断（inform + confirm） | skill 护栏逐条对译；分支名呈现消解 R4（合入目标漂移的最小成本防护） |
| D14 | 前端落位 | **按钮驻 `DetailHeader`**（`detail.status === 'active'` 呈现，`data-testid="archive-trigger"`；已归档 / 文档形态不渲染——status null）；**`ArchivePanel` 挂 DetailHeader 与 RunControlPanel 之间**（归档链与 run 面并置但组件隔离）；hook `use-archive-flow` 镜像 `use-change-flow-run`（Channel 惰性构造 + 快照恢复 + 终态回调 refresh——详情页回落已归档形态、按钮消失）；转录入口 = `useSessionTranscript(sessionId)` + `AgentTimeline` 直组（`SessionTranscriptPanel` 带 role 分页语义不硬复用）；live 事件 = 归档 channel `SessionEvent` 按会话 id 过滤，与库内重放按 seq 归并（既有装配函数同源语义）。data-testid 族：`archive-confirm-dialog` / `archive-warning` / `archive-sync-skip` / `archive-confirm-ok` / `archive-confirm-cancel` / `archive-stage-list` / `archive-stage-<stage>` / `archive-summary` / `archive-specs-line` / `archive-error` / `archive-stop` / `archive-transcript` | 详情页头部是提案候选位之一且与 run 控制面板（run 面域）分离——归档链刻意不进 run 面组件树；转录复用既有 hook + 渲染器，零第二套时间线 |
| D15 | async / spawn 面 | 链本体 `async fn`（std future；`tauri::async_runtime::spawn` 启动）；git 同步子命令与写面 `archive`（fs 改名 + db 事务，毫秒级）**内联调用**（`LocalToolSteps` 同式先例）；分钟级面只有 agent turn（本就是 future）；bootstrap 式 `spawn_blocking` 不引入（无分钟级同步 spawn 段）。ArchiveControl 用 tokio sync 原语（control.rs 同库同型） | 阻塞时长定级决定包装形态：亚秒 git / 毫秒写面内联即可，过度包装反而复制 create 的分钟级特例 |
| D16 | 版本交付与守线 | `packages/desktop` 0.4.20 → **0.4.21**（`tauri.conf.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 2.10.44 零改动（SKILL.md 原文只读引用，prompt 语义桌面化重写而非引用文件路径——agent 无插件面） | spec 数值与现状一致直接执行；prompt 若引用插件路径会在用户仓（无该插件）断裂——桌面化是唯一可行形态 |

### 阶段机（run_archive_flow 定形）

```
preflight（重校验）：get_change 在案 + status=active；run 注册表在案 → 拒；
  worktree 记录在场且目录缺失且分支未合入 → Err 引导（恢复目录或手动处置）
specSync：delta specs 在场且 sync_specs → agent 会话（cwd / provenance / prompt 见 D4/D7；
  outcome.status=Completed → passed，否则 failed 停链）；delta 缺席 → skipped（无 delta specs）；
  sync_specs=false → skipped（用户选择）
commit（worktree 记录在场才执行）：branch_merged → skipped（已合入）；
  worktree 目录缺失（未合入形态）→ Err 引导；dirty → commit_all；干净 → skipped（干净）
merge（worktree 记录在场才执行）：branch_merged → skipped（已合入）；否则 merge_branch
  （冲突 / 状态不允许 → Err 停等，git 语境 + 手动引导；解冲突后重试续走）
seal：write::archive(&resolve(main_root), store, change)（既有双写单点；半完成重试走续半边）
finalize：dirty(main_root, [old, new]) → commit_paths；假 → skipped（已落盘）
Finished{summary}（名 / 归档目录 / specs 三态 / 警告清单）
```

legacy（`worktree=None`）：commit / merge 两段整段 skipped（detail「legacy 无 worktree」），链 = 校验 →（同步）→ seal → finalize。停止：`archive_flow_stop` → 取消旗（阶段间检查点）+ StopRegistry 终止当前 agent 会话（会话槽经 `ArchiveSink` 首事件先行同步，寻址不依赖 turn 收口）；收敛 `Finished{error: 已停止}`。

### 验收标准对齐

| AC | 落点 |
|----|------|
| AC-1 | D6 preflight（`None` 面 + runActive）+ D13 对话 + D14 按钮两态 |
| AC-2 | D6 完成度 / 产物核对 + D12 警告词汇 + D13 不阻断 |
| AC-3 | 阶段机 commit / merge 段 + D9 ④ 幂等跳过 |
| AC-4 | D9 ①② + D11 pathspec 纪律（机械化验证：`git status` 前后对照） |
| AC-5 | D4/D7 agent 段 + prompt（数据模型）幂等语义 + D12 specs 三态 |
| AC-6 | 阶段机 specSync 失败停链 + stop 检查点 + 幂等续走 |
| AC-7 | seal 段复用写面 + Finished summary + D11 finalize |
| AC-8 | D9 ③ merge 冲突面（abort 尽力 + Err 引导） |
| AC-9 | legacy 段跳过 + `archive_change` 零改动（D1）+ D2 独立 Channel + D3 双向互斥 |
| AC-10 | D16 版本 + 守线任务（tasks 阶段五） |

---

## 数据模型

### DTO（`archive_flow.rs`，serde camelCase + specta `Type`）

```rust
pub enum ArchiveStage { Preflight, SpecSync, Commit, Merge, Seal, Finalize }   // 线：preflight|specSync|commit|merge|seal|finalize
pub enum ArchiveStageStatus { Running, Passed, Skipped, Failed }
pub struct ArchiveStageState { pub stage: ArchiveStage, pub status: ArchiveStageStatus, pub detail: Option<String> }
pub enum ArchiveSpecsStatus { Synced, Skipped, None }                          // 线：synced|skipped|none
pub struct ArchiveSummary { pub name: String, pub archived_dir: String,
                            pub specs: ArchiveSpecsStatus, pub warnings: Vec<String> }
#[serde(tag = "ipc")] pub enum ArchiveUpdate {
    Stage { stage: ArchiveStageState },
    SessionEvent { session_id: String, event: agent::AgentEvent },
    Finished { summary: Option<ArchiveSummary>, error: Option<String> },
}
pub struct ArchiveSnapshot { pub stages: Vec<ArchiveStageState>, pub session_id: Option<String> }
pub struct ArchivePreflight { pub name: String, pub completed: bool,
    pub incomplete_phases: Vec<String>, pub missing_artifacts: Vec<String>,
    pub delta_specs: Vec<String>, pub worktree: Option<String>, pub branch: Option<String>,
    pub merge_target: Option<String>, pub run_active: bool }
```

### provenance 与提交信息

| 面 | 定式 |
|---|---|
| 归档 agent provenance | `source="change"`、`source_ref="<change>/archive/spec-sync"`、permission=BypassPermissions、`SessionRef::New`、agent=None |
| worktree 提交 | `archive: <name>` |
| 主仓合入 | `git merge --no-edit change/<name>`（信息 git 默认） |
| 归档落盘提交 | `archive: move <name> to archive`，pathspec = `openspec/changes/<name>` + `openspec/changes/archive/<YYYY-MM-DD>-<name>`（相对 POSIX 串自 `foundation::layout::domain_dir_name()` 单点拼；commit 形态各附 `/**`） |

### spec 同步 prompt（`spec_sync_prompt(change)` 单点，语义源 = SKILL.md 步骤 2）

```
你执行 change「{change}」归档链的 delta specs 同步段（openspec-archive-change skill
步骤 2 的桌面化执行；完成度核对与归档确认已由桌面完成，无需重复）。

对每个 {change} 目录 `openspec/changes/{change}/specs/` 下的 `<capability>/spec.md`：
1. 读 delta 与主基线 `openspec/specs/<capability>/spec.md`（主基线可能不存在）。
2. 按增量语义合并——delta 表达意图而非整体替换，保留 delta 未提及的主 spec 内容：
   - `## ADDED Requirements`：requirement 缺席则追加；已存在则更新为与 delta 一致。
   - `## MODIFIED Requirements`：只应用增量——新增 scenario、修改列出的 scenario、
     修订描述；不复制既有 scenario。
   - `## REMOVED Requirements`：整块移除该 requirement。
   - `## RENAMED Requirements`：把 FROM: requirement 改名为 TO:。
3. capability 主 spec 缺席时创建：简短 `## Purpose`（TBD 可）+ ADDED requirements。
4. 合并幂等：对已同步的主基线重跑本段应零变化。

约束（必须遵守）：
- 只编辑 `openspec/specs/**`；不修改 `openspec/changes/{change}/`（delta 原件由归档
  收口整体迁移）。
- 禁止调用 MCP 工具（change_list 等）、禁止 __TOOL_ASK_USER__、不读写 workflow.json。
- 不执行任何 git 命令——提交与合入由桌面编排代执行。
- 完成后最终消息简述各 capability 的合并动作（added / modified / removed / renamed）。
```

---

## 变更清单

### 实现文件

- `crates/core/orchestration/src/port.rs`（扩展）— `ArchiveVcsPort` trait（六方法，D8）
- `crates/core/orchestration/src/archive_flow.rs`（新）— 阶段机 / ArchiveControl / ArchiveGuard / DTO 面 / 完成度谓词镜像 / 警告词汇单点 / `spec_sync_prompt`
- `crates/core/orchestration/src/lib.rs` — 挂 `archive_flow` 模块 + re-export（零依赖新增）
- `crates/infra/vcs/src/git.rs`（扩展）— 归档子命令族 + `ProcessArchiveVcs`（`git()` 执行器 `pub(crate)` 化）
- `crates/infra/vcs/src/lib.rs` — 导出 + crate 文档注释随动；`Cargo.toml` 增 `orchestration`
- `src/commands/archive_flow/mod.rs`（新）— 五命令 + `_with` 泛型测试缝 + `ArchiveSink`
- `src/commands/mod.rs` — `all_commands!` 注册五命令
- `src/commands/change_flow/mod.rs` — 前置校验序列 +1（归档进行中拒绝；组注释随动）
- `src/commands/changes/mod.rs` — 组注释三分随动（至多注释面）
- `src/main.rs` — `app.manage(Arc::new(ArchiveControl::new()))`
- `packages/desktop/src/views/changes/flow/archive-state.ts`（新）— 纯 reducer
- `packages/desktop/src/views/changes/hooks/use-archive-flow.ts`（新）— invoke + Channel + 恢复
- `packages/desktop/src/views/changes/flow/archive-panel.tsx`（新）— 确认对话 / 阶段 / 转录 / 摘要
- `packages/desktop/src/views/changes/change-detail-view.tsx` — DetailHeader 按钮 + panel 挂载 + 终态 refresh
- `packages/desktop/src/types/generated/bindings.ts` — 再生
- `packages/desktop/package.json` — 0.4.20 → 0.4.21

### 测试文件（test-design / test-gen 阶段承接）

- `crates/core/orchestration/src/archive_flow_test.rs`（新）— 假 vcs + 假引擎直驱：阶段序、幂等跳过四态、失败停等续走、agent 失败 / 停止停链、无 delta 零会话、legacy 跳 git 段、互斥拒绝、完成度谓词与 `phase_next.done` 等价佐证
- `crates/infra/vcs/src/git_test.rs`（扩展）— 真实 git tempdir（`TEST_PATH_LOCK` 互斥纪律）：提交 / 合入（ff / non-ff）、冲突 abort 收口、**无关 staged / untracked 前后保持**（AC-4 机械断言）、pathspec `/**` 删除路径、干净探测 gitignore 面、detached HEAD / 缺分支 Err 面
- `src/commands/archive_flow/mod_test.rs`（新）— 薄包装回环、DTO 字段面、blank root 口径
- `packages/desktop/src/views/changes/flow/archive-panel.test.tsx`、`archive-state.test.ts`、`hooks/use-archive-flow.test.ts`、`change-detail-view.test.tsx`（扩展）— 按钮两态、对话警告呈现、阶段 / 摘要断言（data-testid 挂钩）

### 不要修改（红线对齐 proposal）

walker / 相位机（`phase_next.rs` 等写操作）/ `RunUpdate` / `ChangeRunSnapshot` 事件面；`write/archive.rs` 双写语义（至多注释面）；`WorktreePort` 六方法；`ChangeFlowControl`（只读消费）；`SessionRecord` / 转录 / 轮统计面；`plugins/dev-team` 全部；`openspec/specs/**` 基线；workflow.json 双向墙。
