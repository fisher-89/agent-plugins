# 设计: archive-merge-first

> **变更**: archive-merge-first
> **日期**: 2026-10-08

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（desktop-change-archive 六 requirement 修订 + RENAMED、desktop-change-worktree 两 requirement 修订）已由提案阶段定稿；本设计不重复其内容，只在其八项「待决问题」之上定稿（见「关键设计决策」D2–D7、D11 各就位）。归档链现行实现（desktop-archive-change 落地面）是本变更的改动基线：`archive_flow.rs` 六段阶段机、`ArchiveVcsPort` 六方法、`ProcessArchiveVcs`、五命令组、前端面板——全部在场可查，本设计在其之上做**链形重整 + 地基修复**，不新建架构面。

版本基准注记：spec「版本交付」行 `0.4.25 → 0.4.26` 与主仓现状一致（0.4.25 为 desktop-archive-change 自身的未提交 bump），直接执行；`plugins/dev-team` 2.10.44 零改动（零 skill 溯源后归档链语义完全自持，SKILL.md 不再被任何面引用）。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `ArchiveVcsPort`（签名演进） | 归档链 → vcs 缝随冲突语义扩展：`merge_branch` 返回 `MergeOutcome`（冲突态保留 + 清单）；新增 `worktree_snapshot`（后验 A/B 快照基面）、`commit_merge`（冲突解后链代收口）、`abort_merge`（lean 收口） | `crates/core/orchestration/src/port.rs`（扩展） | crate 内 std | sync trait 零 tokio（既有纪律）；类型 `MergeOutcome` / `WorktreeSnapshot` / `StatusEntry` / `IndexEntry` 驻本文件（port 载荷归消费者 crate，非 IPC 面） |
| git 归档子命令族（扩展） | `merge --no-edit` 冲突不-abort 语义（`diff --name-only --diff-filter=U -z` 归一清单）；快照四连（`rev-parse HEAD` / `rev-parse -q --verify MERGE_HEAD` / `status --porcelain -z` / `ls-files -s -z` 解析）；`commit --no-edit` / `merge --abort` | `crates/infra/vcs/src/git.rs`（扩展）、`lib.rs`（`ProcessArchiveVcs` impl 随动） | orchestration（port 类型，既有） | argv 直传零 shell；`-z` 全线消除引号 / 非 ASCII 形态（R6——0.4.18 raw_arg 教训的读取面对偶） |
| 归档编排链运行时（重整） | 阶段序重整「校验 → worktree 提交 → 主仓合入（含冲突 agent 分支 + 后验 + lean 收口）→ spec 同步 → 双写收口 → 落盘提交（扩围）」；Commit 段 merged 短路砍除；SpecSync cwd 恒主 root | `crates/core/orchestration/src/archive_flow.rs` | 既有全部（零依赖新增） | 阶段机形态不变（running→终态信封 / check_stop / Terminal）；冲突分支为 Merge 段内嵌子序列（零新阶段枚举值） |
| 后验纯函数 | `verify_resolution(baseline, after, conflicts)`（A/B 快照对比规则）+ `residual_markers(root, conflicts)`（fs 扫描冲突标记——locale 免疫） | `archive_flow.rs` 内 `pub(crate)` | std fs | 纯函数单测全覆盖分支 + 真实 git tempdir 锚定解析形态（git_test 承接） |
| 归档 agent prompt 单点（一族两模板） | `spec_sync_prompt(change)`（溯源摘除重写——语义单源 = spec requirement 自述）+ `merge_conflict_prompt(change, conflicts)`（新：冲突清单 + 只编辑清单内文件 + 逐文件 add + 禁收口 / 改史命令） | `archive_flow.rs` 内（prompt.rs 是相位 run 三角色家族，归档 prompt 非相位面——D4 先例维持） | — | const 模板 + format 插值；零 MCP / `__TOOL_ASK_USER__` / workflow.json / skill 溯源 |
| WorkerAgent 通道（复用零改动） | 解冲突 agent 是归档链第二个会话面：`compose_turn` 新会话 + StopRegistry 终止 + 密封转录 | `crates/infra/agent/src/worker.rs`（原样） | — | `KernelWorkerPort` + 命令层 `ArchiveSink`（既有转译面——会话事件透明与停止寻址零改动） |
| ArchiveControl（复用零改动） | 会话槽单槽覆写 = 当前活跃会话（两会话严格串行不并存——先冲突解算后 spec 同步） | `archive_flow.rs` 内（既有） | — | 停止寻址语义天然成立；快照 `session_id` = 当前会话（D5） |
| 写面 archive / Finalize 扩围 | 双写单点收口零改动；落盘段 pathspec = 归档改名两路径 + delta capability 主 specs 子树 | `archive_flow.rs`（Finalize 段）+ `workflow/src/write/archive.rs`（零触点） | foundation（`domain_dir_name` 既有） | `commit_paths` 既有 glob 纪律直接复用（`{domain}/specs/<cap>` 各附 `/**`） |
| 命令组（零签名改动） | 五命令签名 / 装配 / 互斥面全部不动；仅模块注释的六段序描述随动 | `src/commands/archive_flow/mod.rs` | — | 冲突摘要经 `Stage.detail` / `Finished.error` 既有信封（D7——零新 IPC 字段） |
| 前端归档面（呈现序随动） | `ARCHIVE_STAGES` 呈现序对齐执行序；Merge 段 running detail 提示（后端透传）；lean 咨询面 = 既有失败面 | `packages/desktop/src/views/changes/flow/archive-state.ts`、`archive-panel.tsx`（注释面） | bindings | data-testid 零新增（`archive-stage-merge` / `archive-error` 既有挂钩承载 AC 断言） |
| 版本交付 | `packages/desktop` 0.4.25 → 0.4.26 | `packages/desktop/package.json` | — | D12 |

**不变组件（零触点核对结论）**：`orchestration/src/walker.rs` / `steps.rs` / `snapshot.rs` / `state.rs`（`RunUpdate` / `ChangeRunSnapshot` 事件面）；`control.rs`（`ChangeFlowControl`）；`workflow/src/write/archive.rs` 双写语义与 `WorktreePort` 六方法；`crates/infra/agent/src/worker.rs`；`SessionRecord` / 会话转录面；五命令签名与 `ArchiveSink` 转译逻辑；`plugins/dev-team` 全部；`openspec/specs/**` 基线；DTO 面**零新增**（`ArchiveStage` 值域不变仅声明序对齐、`ArchiveUpdate` / `ArchiveSummary` / `ArchivePreflight` 逐字不动——冲突语义全部经既有 detail / error 串承载）。

---

## 关键设计决策

| # | 问题（proposal 待决 / 设计面） | 定稿 | 理由 |
|---|------|------|------|
| D1 | 阶段序重整与枚举面 | `ArchiveStage` 六值**零增删零改名**（wire 值 `preflight|commit|merge|specSync|seal|finalize` 不变——serde 按名序列化），仅 Rust 声明序与前端 `ARCHIVE_STAGES` 呈现序对齐新执行序（commit、merge 前置于 specSync）。阶段机骨架（running→终态成对信封、同段后写覆盖、check_stop 检查点、`Terminal` 两态）逐字维持；`drive()` 内段序重排 + 各段语义随动 | 枚举值域不变使快照 / 重挂 / bindings 全部零迁移；DTO 零增量是本变更呈面最小的路径（冲突子阶段不进枚举——见 D7） |
| D2 | Commit 段跳过依据（含待决 8：目录缺失已合入形态的跳过因） | **干净探测唯一跳过依据**：`branch_merged` 查询从 Commit 段整体移除（死法 A 根源——零提交分支 tip=基线，主仓前进即误判）。段内分支归置：worktree=None → skipped「legacy 无 worktree」；worktree 目录缺失 → skipped「已合入」（**前置已拦未合入形态**——Preflight 的 dir-missing+未合入 Err 引导仍在，Commit 段不设第二次 merged 查询；Preflight 与 Commit 间微秒级 TOCTOU（用户恰在此窗口删目录）退化为 dirty 探测失败→干净跳过，非破坏性，留痕）；目录在场 → dirty 即 `commit_all`、干净即 skipped「干净」——**可达性不再构成任何跳过分支**。Merge 段保留 merged 幂等跳过（此时判定安全：提交段已保证分支携带内容） | 砍除 vs 特判（proposal 已否决特判面宽）；目录缺失形态复用既有词汇「已合入」不加词汇员——它就是跳过的真实依据（内容已在主仓，提交无从发生也无需发生） |
| D3 | `ArchiveVcsPort` 签名形态（待决 1） | **枚举返回 + 三新方法，不拆独立 conflict_files 探针**：`merge_branch -> Result<MergeOutcome, String>`（`Merged` \| `Conflicted(Vec<String>)`——清单即 `diff --name-only --diff-filter=U -z` 归一结果，冲突态保留不 abort；非冲突失败——非零退出且清单空——维持既有「尽力 abort + Err 带 git 语境与手动引导」面，AC-8 语义不变）；`worktree_snapshot -> Result<WorktreeSnapshot, String>`（后验 A/B 基面四合一：HEAD / MERGE_HEAD 在场性 / porcelain 状态 / ls-files 索引）；`commit_merge -> Result<(), String>`（`commit --no-edit`——链代收口）；`abort_merge -> Result<(), String>`（lean 收口面）。清单 / 快照解析全部 `-z`（NUL 分隔零引号形态，R6） | 拆独立探针（`conflict_files` / `abort_merge` 两方法 + `merge_branch` 维持 Err）会让「冲突判定」与「冲突读取」两步之间存在无主窗口（清单读失败时态归谁不清）；枚举返回把冲突判定原子化在单次 spawn 序列内。快照四合一避免链侧四次 port 往返与半截快照形态 |
| D4 | 后验机械面（待决 3：命令族与快照时机） | **A = 冲突即时（agent 前）、B = agent 收口后各取一次 `worktree_snapshot` 对比**，规则以纯函数 `verify_resolution(&A, &B, conflicts) -> Result<(), String>` 落地（驻 orchestration）：① `B.merge_head` 缺席 → Err（agent 违约自行收口 / 中止了 merge——串附 `A.head` 供 `git reset --hard` 引导，链不自动改写历史）；② 冲突路径在 B 的索引仍有 stage>0 条目 → Err「残留冲突」；③ 冲突路径在 B 必须 worktree 干净（Y=' '）且索引单条 stage-0（或已删除缺席）→ 全量 staged 才可收口；④ 非 C 路径的 porcelain 状态与索引条目（mode/hash/stage）A/B 逐字一致、且 B 无 A 缺席的新路径、消失路径 ⊆ 冲突清单 → 否则 Err「清单外新改动」（捕获：agent 新建 / 新 add 文件、staged 用户文件、编辑 merge-staged 文件后 add——hash 对比面、动用户未跟踪文件）。残留标记另以 `residual_markers(root, conflicts)` fs 扫描冲突清单文件行首 `<<<<<<< ` / `>>>>>>> ` 形态（worktree 与 staged 同内容——③已保证；locale 免疫、范围窄、假阳性后果 = lean 安全收口）。**bound 留痕**：后验是**状态面**对比非内容 forensics——对 A 时点已在案的未提交内容做就地篡改（不 add、状态不变）不可检测，属 R2 已知假阴面，机械红线由「吞并不发生」（收口只 commit 索引内既定内容）兜底 | 时机（A/B 各一次）是 proposal R2 缓解的指定形态；规则纯函数化使分支覆盖在单测层穷尽（真实 git 只锚定「解析形态」——`-z` / rename 双段 / stage 号 / 非 ASCII 路径——与既有 git_test tempdir 纪律同源）；`--cached --check` 方案因 git 输出 locale 依赖被否决（fs 扫描免疫） |
| D5 | 解冲突 agent 会话定式（待决 2） | provenance：`source="change"`、**`source_ref = "<change>/archive/merge-conflict"`**（spec-sync `<change>/archive/spec-sync` 同族定式——归档语义段命名，无 attempt 段）；`model_level = High`（裁决质量面，spec-sync 同档）；`permission = BypassPermissions`；`role = Executor`（`KernelWorkerPort` 不读 role，零枚举扩展——D4 先例）；**cwd = 主 workspace root**（爆炸半径即主仓——后验机械拦截是唯一的面，不信任 agent 自律）；`ArchiveControl.session` **单槽复用**：归档链两会话（冲突解算 → spec 同步）严格串行，槽位由 `ArchiveSink` 首事件覆写即「当前会话」，停止寻址语义天然成立；快照 `session_id` 同义。进行面转录只呈现当前会话；已收口会话经密封转录按 source_ref 反查回放（可审计性既有面，前端不设会话切换器——V1 呈现边界留痕） | 多会话槽 + 前端切换器是 AC 外的呈面扩张；串行单槽在停止 / 快照 / 转录三个面语义都正确，零 IPC 迁移 |
| D6 | merge commit 收口归属与提交信息派生（待决 4 + 7 合并定稿） | **链后台代收，不信任 agent 收口**：后验通过后链调 `commit_merge`（`git commit --no-edit`——采用 git 默认 merge 信息 `Merge branch 'change/<name>'`，含 `Conflicts:` 段落为 git 默认形态）。agent 的 git 面收缩为「编辑清单文件 + 逐文件 `git add`」——prompt 明令禁 `commit` / `merge` / `rebase` / `reset` / `stash` / `add -A`。**机械安全性**：冲突只发生在 non-ff 三方合并，git 此时已拒绝带 staged 条目的发起（desktop-archive-change D9① 实验定锚）→ 冲突态在场即证 index 起点干净；`commit --no-edit`（无 -a、merge 态禁 pathspec）只提交索引内既定内容 = merge 结果，物理上吞并不了未 staged 的用户未提交 / untracked 内容。提交信息三定式维持不分化：worktree 提交 `archive: <name>`；merge commit（自动合入与冲突解后**同一形态**）git 默认；落盘提交 `archive: move <name> to archive`（扩围内容不换词汇——描述归档落盘主旨） | agent 自行 commit 的违约为 D4① 捕获（MERGE_HEAD 缺席）且无法安全回滚（需改史）——把收口收归链侧使违约面只剩「编辑 / add 越界」，两者均被 D4④ / 标记扫描捕获；信息不分化避免 merge commit 语义与 git 原生形态漂移 |
| D7 | 冲突摘要呈现载体（待决 5） | **零新增 IPC 字段 / data-testid**：lean 咨询面 = 既有失败面（`FailureFace`），载体 = `fail_stage` 单点铸造的 lean 串（`Merge` 段 failed detail 与 `Finished.error` 同串——既有同源机制）。串模板（词汇单点驻 `archive_flow.rs`）：`合入冲突无法自动裁决（<原因>）。冲突文件 <n> 个：<逐行清单>。已执行 git merge --abort 恢复主仓干净态<abort 失败附注>。请手动将分支 change/<name> 合入主仓并解冲突后重试归档——重试将识别已合入并续走收口。`；原因词 ∈ {agent 会话失败 / agent 会话被停止 / 残留冲突未解 / 清单外新改动 / 后验探测失败 / agent 违约自行收口 merge}。**被停止特例**：冲突 agent 被 stop 停止时不用 `TXT_STOPPED` 收敛词——同样走 lean 串（附「（归档链已停止）」注记），满足 spec scenario「被停止也呈现冲突摘要与引导」；`TXT_STOPPED` 保留给非冲突段停止面。Merge 段 running 期间追加一次 detail 更新 `合入冲突，解冲突 agent 裁决中`（同段后写覆盖既有机制，转录面板自动呈现当前会话） | 冲突摘要是停链错误的一部分而非独立交互态（rich 档已明确留后续）——专属字段会预支 rich 档的 IPC 面；失败面已含重试按钮（幂等续走即 lean 闭环的桌面半边） |
| D8 | SpecSync 段位、cwd 与溯源摘除 | 段位移至合入后（R4 失败面变宽由全阶段幂等吸收：重试经已合入跳过 + 同步幂等 + 双写续半边——既有机制零新增）；**cwd 恒 = 主 workspace root**（`record.worktree` 在场的 worktree-cwd 分支删除——delta specs 已随合入进入主仓 active 树，legacy 本就在主仓，两类记录同锚）；链内 delta 探测（`detect_delta_specs`）随段位移至合入后执行——`locate_change` 主仓优先的解析链在合入后必然主仓命中（merge 整树带入 / 已合入跳过时亦在主仓 / legacy 原生在主仓），preflight 探测仅为确认面对话呈现；`spec_sync_prompt` 溯源摘除重写（首行括注「openspec-archive-change skill 步骤 2 的桌面化执行」删除，增量合并语义自持——模板逐字见「数据模型」，语义单源 = spec「spec 同步 agent 与归档链语义自持」requirement） | 段位与 cwd 是死法 B 的结构性消灭面（产物生在主仓无处搁浅）+ 冻结基线旧病消除（合并基线恒为主仓当前 specs）；prompt 摘除是 AC-9 的实现位 |
| D9 | Finalize pathspec 扩围 | pathspec 集 = 归档改名两路径（active 残迹 + archive 新目录，既有）**+ 本链 SpecSync 段探测的 delta capability 清单各 `{domain}/specs/<capability>`**（`domain_dir_name()` 单点拼相对 POSIX 串；`commit_paths` 既有 `/**` glob 纪律直接覆盖子树）。扩围条件 = delta specs 在场（**用户跳过同步时同样扩围**——该子树此时通常干净，`add -A -- <paths>` 无事可做，语义上无害且免去第二套条件）；脏探测门 = 两路径与扩围集的**并集** dirty（既有单探测门语义自然扩为并集）。全树 `{domain}/specs/**` 通配为禁区（proposal 裁定） | 扩围集合与同步段探测同源（不二次探测）；R5 缓解：子树圈定 + 同步 agent 以主仓当前内容为合并基线（用户编辑保留在合并结果内被一并提交——内容不丢） |
| D10 | 无法裁决 lean 收敛面 | 收敛序：agent `Failed` / `Running`（失败收敛）、`Stopped`（被停止）、后验任一 Err（D4 各面 / 残留标记）、`commit_merge` 自身 Err（git 语境——串附「主仓处于 merge 态：可手动 `git commit --no-edit` 收口或 `git merge --abort` 回退后重试」引导，重试前半截态须人工清）→ 一律：`abort_merge` 尽力执行（自身失败则附注「abort 未成功，请手动核验主仓 git 状态」，不掩盖原 Err）→ `fail_stage(Merge, lean 串)`。db 零变化（双写未触）、已成功阶段零回滚（提交段产物在分支上天然保留）——重试经已合入判定幂等续走（用户手动 merge 即完成该段） | 单一收敛序避免各失败面各造文案；abort 尽力 + 附注是既有 `merge_branch` Err 面的同构纪律（不静默吞二次失败） |
| D11 | 前端呈现随动（待决 6：合入段子阶段呈现） | `ARCHIVE_STAGES` 呈现序改为 `['preflight', 'commit', 'merge', 'specSync', 'seal', 'finalize']`（`archive-state.ts` 单点——panel 与测试引用同源）；`STAGE_LABEL` 文案零改动。合入段子阶段呈现 = **Merge 单行内的状态与 detail 演进**（不设子阶段行）：merge 执行中（running 无 detail）→ 冲突解算中（running，detail `合入冲突，解冲突 agent 裁决中`——D7 后写覆盖）→ passed（detail `已解冲突 <n> 文件`）或 failed（detail = lean 串）；转录区随当前会话切换（D5）。`archive-panel.tsx` 逻辑零改动（detail 透传既有）仅注释随动；data-testid 零新增——AC-6 / AC-7 断言挂 `archive-stage-merge[data-status]` 与 detail 文本、`archive-error` 文本 | 子阶段行是 DTO + reducer + panel 三面扩张（枚举值域红线）；单行 detail 演进在既有信封内完整表达进度与终态，R7 以呈现序对齐 + 既有断言形态消解 |
| D12 | 版本与守线 | `packages/desktop` 0.4.25 → **0.4.26**（`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 2.10.44 零改动（SKILL.md 零引用——摘除后归档链语义自持，AC-9 的 grep 面：spec / prompt / 源码零 `openspec-archive-change` 溯源措辞）；DTO 零新增 → `bindings.ts` 预期零语义 diff（specta 枚举 union 声明序可能文字性重排——`bindings:check` 守线复核，不手改生成物） | spec 数值与现状一致直接执行；生成物零手改是既有纪律 |

### 阶段机（drive() 重整定形）

```
preflight（重校验）：不变（建档 + status=active + worktree 缺失且未合入 Err 引导 + 警告清单）
commit：worktree=None → skipped（legacy 无 worktree）；
        目录缺失 → skipped（已合入——前置已拦未合入形态，D2）；
        dirty → commit_all("archive: <name>")；干净 → skipped（干净）        ← merged 查询砍除
merge： worktree=None → skipped（legacy 无 worktree）；
        branch_merged → skipped（已合入）；否则 merge_branch：
        · Merged → passed
        · Err → failed（非冲突失败面——AC-8 维持，串即 port Err）
        · Conflicted(list) → 冲突分支（D3–D7）：
            A = worktree_snapshot
            running detail 更新（合入冲突，解冲突 agent 裁决中）
            agent 会话（merge_conflict_prompt / cwd=主 root / D5 定式）
              Completed → 后验：residual_markers 扫描 + B = worktree_snapshot
                          + verify_resolution(A, B, list) → 通过则
                          commit_merge → passed（已解冲突 <n> 文件）
              任一不通过 / Stopped / Failed → D10 lean 收敛
specSync：探测（detect_delta_specs——此时必主仓命中）→
        无 delta → skipped（无 delta specs）；用户跳过 → skipped（用户选择）；
        否则 agent 会话（spec_sync_prompt 重写版 / cwd=主 root / 既有定式）→
        Completed → passed（detail=session_id）；否则 failed 停链（既有面）
seal：  不变（write::archive 双写单点直调）
finalize：pathspec = 归档两路径 + delta capability 各 {domain}/specs/<cap>（D9）；
        并集 dirty → commit_paths（既有 glob 纪律）；假 → skipped（已落盘）
Finished{summary}（名 / 归档目录 / specs 三态 / 警告清单——零改动）
```

legacy（`worktree=None`）：commit / merge 双 skipped，链 = 校验 → 同步（cwd=主 root，与现行 legacy 形态一致）→ 双写 → 落盘。停止面：取消旗检查点 + StopRegistry 寻址当前会话（单槽覆写——D5）均既有零改动；冲突段停止走 D7 特例。

### 验收标准对齐

| AC | 落点 |
|----|------|
| AC-1 | D2（Commit 短路砍除）+ D1（阶段序——Seal 守卫前提交合入必已发生） |
| AC-2 | D2（干净跳过唯一）+ Merge 段 merged 幂等跳过维持 |
| AC-3 | D8（段位 / cwd 恒主 root / 主仓当前基线——合并发生在主仓工作区） |
| AC-4 | D9（扩围 pathspec 与并集脏探测）+ `commit_paths` 既有不吞并纪律 |
| AC-5 | D8 + D9 结构性（产物落主仓工作区经落盘段提交；摘要与磁盘事实同源） |
| AC-6 | D3（Conflicted 原子判定）+ D5（会话定式）+ D4（后验）+ D6（链代收口） |
| AC-7 | D10（lean 收敛序）+ D7（lean 串与重试引导）+ 幂等续走（AC-2 面） |
| AC-8 | D3（非冲突失败 Err 面逐字维持——尽力 abort + git 语境 + 手动引导） |
| AC-9 | D8（prompt 溯源摘除）+ D12（plugin 零触 / grep 面） |
| AC-10 | D12（0.4.26 / 管线绿 / 零豁免） |

### 风险对齐（proposal R1–R8 → 设计落点）

R1 爆炸半径 → D4 后验 + D5/D6 prompt 机械收缩（agent 只编辑 + 逐文件 add）+ D10 abort 兜底；R2 假阳 / 假阴 → D4 精确规则 + 纯函数单测穷尽 + 真实 git tempdir 锚定 + bound 留痕；R3 并发手动编辑 → 既有互斥（run 拒绝 + ArchiveControl 重入）+ D7 串内 abort 告知；R4 同步后移失败面 → 全阶段幂等（零新增）；R5 扩围撞车 → D9 子树圈定 + 同步以主仓当前内容为基线；R6 Windows 解析 → `-z` 全线 + argv 直传既有；R7 前端序 → D11；R8 时长反馈 → 既有 broadcast + 转录流 + 发起幂等（零改动）。

---

## 数据模型

### port 载荷（`orchestration/src/port.rs`，非 IPC 面）

```rust
/// merge 结果（冲突语义：冲突态保留，不 abort——收口归调用方）。
pub enum MergeOutcome {
    /// 合入成功（ff 或 merge commit 已落）。
    Merged,
    /// 冲突（unmerged 清单非空；`-z` 归一路径）。
    Conflicted(Vec<String>),
}

/// `status --porcelain -z` 解析像（XY = index / worktree 状态码；untracked 为 "?","?"；
/// rename 条目记新路径）。
pub struct StatusEntry { pub x: char, pub y: char, pub path: String }

/// `ls-files -s` 解析像（stage 0 = 正常条目；1/2/3 = unmerged 三方）。
pub struct IndexEntry { pub mode: String, pub hash: String, pub stage: u8, pub path: String }

/// 主仓工作区快照（后验 A/B 对比基面，D4）。
pub struct WorktreeSnapshot {
    /// HEAD（A 时点 = merge 前 HEAD——merge 态不前移 HEAD）。
    pub head: String,
    /// MERGE_HEAD 在场性（merge 态判别；缺席 = 已收口 / 已中止）。
    pub merge_head: Option<String>,
    pub status: Vec<StatusEntry>,
    pub index: Vec<IndexEntry>,
}
```

`ArchiveVcsPort` 方法面（sync 签名，`Err` 带引导文案维持）：既有 `dirty` / `commit_all` / `branch_merged` / `current_branch` / `commit_paths` 五方法零改动；`merge_branch` 签名改 `Result<MergeOutcome, String>`；新增 `worktree_snapshot(main_root) -> Result<WorktreeSnapshot, String>`、`commit_merge(main_root) -> Result<(), String>`、`abort_merge(main_root) -> Result<(), String>`。

### DTO 面（零增量注记）

`ArchiveStage` / `ArchiveStageStatus` / `ArchiveStageState` / `ArchiveSpecsStatus` / `ArchiveSummary` / `ArchiveUpdate` / `ArchiveSnapshot` / `ArchivePreflight` 逐字段不动；`ArchiveStage` 声明序对齐执行序（wire 值不变）。冲突语义全部经 `ArchiveStageState.detail`（failed 记因 / running 提示）与 `Finished.error` 既有串面承载——零新 IPC 字段，`bindings.ts` 预期零语义 diff。

### provenance 与提交信息定式

| 面 | 定式 |
|---|---|
| spec 同步 agent | `source="change"`、`source_ref="<change>/archive/spec-sync"`、cwd=主 workspace root、BypassPermissions、ModelLevel::High、role=Executor、SessionRef::New |
| 解冲突 agent（新） | `source="change"`、`source_ref="<change>/archive/merge-conflict"`、cwd=主 workspace root、BypassPermissions、ModelLevel::High、role=Executor、SessionRef::New |
| worktree 提交 | `archive: <name>`（不变） |
| merge commit | `git merge --no-edit` / `git commit --no-edit`——git 默认信息 `Merge branch 'change/<name>'`（含 `Conflicts:` 段为 git 默认形态；自动合入与冲突解后不分化） |
| 归档落盘提交 | `archive: move <name> to archive`（不变），pathspec = `openspec/changes/<name>` + `openspec/changes/archive/<YYYY-MM-DD>-<name>` + delta capability 各 `openspec/specs/<cap>`（`/**` glob 由 `commit_paths` 既有逻辑附加） |

### spec 同步 prompt（`spec_sync_prompt(change)` 重写——溯源摘除版）

```
你执行 change「{change}」归档链的 delta specs 同步段（完成度核对与归档确认已由桌面完成，无需重复）。

对每个 `openspec/changes/{change}/specs/` 下的 `<capability>/spec.md`：
1. 读 delta 与主基线 `openspec/specs/<capability>/spec.md`（主基线可能不存在）。
2. 按增量语义合并——delta 表达意图而非整体替换，保留 delta 未提及的主 spec 内容：
   - `## ADDED Requirements`：requirement 缺席则追加；已存在则更新为与 delta 一致。
   - `## MODIFIED Requirements`：只应用增量——新增 scenario、修改列中的 scenario、修订描述；不复制既有 scenario。
   - `## REMOVED Requirements`：整块移除该 requirement。
   - `## RENAMED Requirements`：把 FROM: requirement 改名为 TO:。
3. capability 主 spec 缺席时创建：简短 `## Purpose`（TBD 可）+ ADDED requirements。
4. 合并幂等：对已同步的主基线重跑本段应零变化。

约束（必须遵守）：
- 只编辑 `openspec/specs/**`；不修改 `openspec/changes/{change}/`（delta 原件由归档收口整体迁移）。
- 禁止调用 MCP 工具（change_list 等）、禁止 __TOOL_ASK_USER__、不读写 workflow.json。
- 不执行任何 git 命令——提交与合入由桌面编排代执行。
- 完成后最终消息简述各 capability 的合并动作（added / modified / removed / renamed）。
```

### 解冲突 prompt（`merge_conflict_prompt(change, conflicts)` 新增）

```
你执行 change「{change}」归档链的合入冲突解算段。主仓当前分支合入分支 change/{change} 时以下文件冲突（冲突态已保留，merge 进行中）：

{conflicts 逐行清单}

对每个冲突文件：
1. 读文件内的冲突标记区块（<<<<<<< / ======= / >>>>>>>），结合两侧语义裁决出正确的合并结果。
2. 以裁决结果编辑该文件，移除全部冲突标记。
3. 完成后对该文件执行 `git add <该文件路径>`（逐文件精确 add）。

约束（必须遵守）：
- 只编辑上面清单内的文件；清单外任何文件一律不动（含未提交 / staged 的无关内容——它们属于用户）。
- 禁止 `git add -A` / `git add .`；禁止 `git commit` / `git merge` / `git rebase` / `git reset` / `git stash` 等收口或改写历史的命令——merge 收口由桌面编排执行。
- 禁止调用 MCP 工具、禁止 __TOOL_ASK_USER__。
- 全部冲突文件解算并 add 后，最终消息简述各文件的裁决要点。
```

### 词汇单点（`archive_flow.rs` 常量随动）

既有跳过因词汇零改动（干净 / 已合入 / 无 delta specs / 用户选择 / legacy 无 worktree / 已落盘）；新增 lean 串模板与原因词（D7）、Merge running detail 提示（`合入冲突，解冲突 agent 裁决中`）、冲突解通过 detail（`已解冲突 <n> 文件`）——铸造收 `archive_flow.rs` 单点（D12 先例：测试断言锚不漂移）。

---

## 变更清单

### 实现文件

- `crates/core/orchestration/src/port.rs`（扩展）— `MergeOutcome` / `StatusEntry` / `IndexEntry` / `WorktreeSnapshot` 类型；`merge_branch` 签名改；`worktree_snapshot` / `commit_merge` / `abort_merge` 方法族；trait 文档注释随动（冲突语义家族）
- `crates/core/orchestration/src/archive_flow.rs`（重整）— 阶段序重排（`ArchiveStage` 声明序 + `drive()` 段序 + check_stop 位点）；Commit 段 merged 短路砍除与目录缺失跳过因归置（D2）；Merge 段冲突分支（A 快照 → agent 会话 → 后验 → `commit_merge` 收口 / D10 lean 收敛）；`verify_resolution` / `residual_markers` 纯函数；SpecSync 段位后移 + cwd 恒主 root + `spec_sync_prompt` 溯源摘除重写；`merge_conflict_prompt` 新增；Finalize pathspec 扩围（D9）；lean 串与提示词汇常量
- `crates/core/orchestration/src/lib.rs` — re-export 面零新增核对（`MergeOutcome` 等经 port 模块既有路径可见性核对，至多注释随动）
- `crates/infra/vcs/src/git.rs`（扩展）— `merge_branch` 冲突不-abort 语义（`-z` 清单归一 + 非冲突失败面维持）；`worktree_snapshot`（HEAD / MERGE_HEAD / porcelain `-z` / ls-files `-z` 解析，rename 双段与 stage 号处理）；`commit_merge`；`abort_merge`
- `crates/infra/vcs/src/lib.rs` — `ProcessArchiveVcs` impl 四面随动（导出面零新增）
- `src/commands/archive_flow/mod.rs` — 模块注释六段序描述随动（零命令签名 / 装配 / 互斥改动）
- `packages/desktop/src/views/changes/flow/archive-state.ts` — `ARCHIVE_STAGES` 呈现序对齐执行序
- `packages/desktop/src/views/changes/flow/archive-panel.tsx` — 注释随动（逻辑零改动：detail 透传 / 失败面即咨询面）
- `packages/desktop/src/types/generated/bindings.ts` — 再生核对（预期零语义 diff；`bindings:check` 守线）
- `packages/desktop/package.json` — 0.4.25 → 0.4.26

### 测试文件（test-design / test-gen 阶段承接 + 实现期编译面机械随动）

- `crates/core/orchestration/src/archive_flow_test.rs` — `FakeVcs` 随动（`merge_branch` 新签名 + `with_merge_conflicts` 编程面 + 三新方法编程与捕获）；既有锚改设：「已合入双跳过」锚 fake worktree 设干净才成立、「merge 冲突停链」锚按新语义分裂（冲突 → agent 路径 / 非冲突 Err → AC-8 面维持）；新增锚（proposal 测试清单全量）：零提交 + 脏 → 提交发生（merged=true 误判形态）；同步段 cwd=主仓；冲突 → 清单透传 + 后验通过 + `commit_merge` 收口；无法裁决各面（agent 失败 / 停止 / 残留标记 / 清单外新改动 / MERGE_HEAD 缺席）→ lean abort + 停链咨询串；`verify_resolution` 纯函数分支穷尽（合成快照）；Finalize 扩围 pathspec 断言（delta 在场含跳过同步形态 / 缺席两路径）
- `crates/infra/vcs/src/git_test.rs` — merge 冲突锚族按不-abort 语义重写（`Conflicted` 清单返回、UU 态保留、`abort_merge` 幂等收口、`commit_merge` 默认信息收口）；`worktree_snapshot` 解析锚（`-z` 形态、rename 双段、stage 号、非 ASCII 路径——R6）；无关 staged / untracked 前后保持机械断言随冲突路径扩族
- `src/commands/archive_flow/mod_test.rs` — 回环核对（预期零改动或注释面随动——命令签名零变）
- `packages/desktop/src/views/changes/flow/archive-state.test.ts` / `archive-panel.test.tsx` — 呈现序断言更新；Merge 段 detail 演进与 lean 咨询面断言（既有 data-testid）
- golden（若 DTO 演进触线）— 预期零触线；`DESKTOP_GOLDEN_REWRITE=1` 流程仅在意外 diff 时启用

### 不要修改（红线对齐 proposal）

`plugins/dev-team` 全部（版本 2.10.44、SKILL.md、三类交付产物）；`crates/core/workflow/src/write/archive.rs` 双写语义；walker / 相位机 / `RunUpdate` / `ChangeRunSnapshot` 事件面；`SessionRecord` / 会话转录 / 槽位记录面（解冲突 agent 走既有会话基建）；`crates/infra/agent/src/worker.rs`；`WorktreePort` 六方法；五命令签名与 `ArchiveSink` 转译逻辑；`openspec/specs/**` 既有基线；worktree / branch 清理边界（用户手动维持）。
