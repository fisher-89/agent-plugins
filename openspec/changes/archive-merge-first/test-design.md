# 测试设计: archive-merge-first

> **变更**: archive-merge-first
> **日期**: 2026-10-08
> **依据**: proposal.md（AC-1..AC-10）+ design.md（D1–D12 决策编号）；tasks.md 阶段一「编译面机械随动」只保编译不写断言，本文件展开全部新增 / 重写锚（test-gen / test-execution 阶段承接）

---

## 测试边界与框架识别

<!-- 逐文件推导自 design.md 变更清单（实现文件 + 测试文件两节）；实现阶段零清单外测试文件。 -->

- **Rust 面**：src-tauri cargo workspace 套件，共置 `*_test.rs` 模块（orchestration / vcs / commands 先例）。**零新增测试文件**——扩展 `crates/core/orchestration/src/archive_flow_test.rs`（本变更最大断言面）、`crates/infra/vcs/src/git_test.rs`，适配 `src/commands/archive_flow/mod_test.rs`；orchestration / vcs dev 依赖零新增（tokio / tempfile 既有）。
- **前端面**：packages/desktop vite-plus 套件，共置 `.test.ts` / `.test.tsx`。扩展 `flow/archive-state.test.ts`、`flow/archive-panel.test.tsx`；`hooks/use-archive-flow`、`change-detail-view`、`session-transcript-panel` 等零触点（design 不变组件——逻辑零改动仅注释随动）。
- **进程边界策略**：三层分工沿 desktop-archive-change 先例——① **vcs 真件**：git 冲突语义族（不-abort + `-z` 清单 / 快照四连 / commit_merge / abort_merge）以真实 git + tempdir 仓锚定（design D3/D4「真实 git 只锚定解析形态」——rename 双段、stage 号、非 ASCII 裸路径为 R6 面；`git -C <root>` 显式寻址、`TEST_PATH_LOCK` 互斥纪律沿 crate 既有锁）；② **core 假件直驱**：归档链编排经进程内脚本化假件（`ArchiveVcsPort` 扩展冲突编程面 + 快照队列 + 三新方法捕获 / `WorkerAgentPort` 双会话捕获 / `ChangeStateStore` 假件）+ 真实 tempdir fs——后验纯函数（`verify_resolution` / `residual_markers`）以合成快照与真实 tempdir 文件穷尽分支（D4「规则纯函数化使分支覆盖在单测层穷尽」）；③ **命令层回环**：MockRuntime 既有装置——命令签名 / 装配 / 互斥面 / ArchiveSink 零改动，仅阶段信封流序随执行序适配。前端 `vi.mock` hooks 注入（面板纯呈现面）。
- **迭代类型词表**：**新增**（新用例）/ **重写**（既有用例语义演进改写）/ **适配**（机械传参或 fixture 补齐，既有断言面随动）/ **持衡（沿用）**。本变更是既有归档链的重整——阶段序、提交段跳过依据、SpecSync 段位三面既有锚**重写**为主；run 面（walker / steps / snapshot / state / control）与写面（write/archive）、worker.rs、SessionRecord 面既有测试**零触点零适配**（design 不变组件核对结论）。
- **golden 零触点注记**：DTO 面零增量（`ArchiveStage` 值域不变仅声明序、`ArchiveUpdate` / `ArchiveSummary` / `ArchivePreflight` 逐字不动——design D1/D12），corpus golden 零重写；`bindings.ts` 预期零语义 diff（specta 枚举 union 声明序可能文字性重排，`bindings:check` 守线复核，不手改生成物）——proposal「golden（若 DTO 演进触线）」条件不成立，`DESKTOP_GOLDEN_REWRITE=1` 流程不启用。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 10 条 AC。被测文件或模块为承载用例的测试文件（单值）；
     跨半边 AC 以「（同上——…半边）」行展开到各自测试文件；纯静态 / 流程性半边落
     「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 死法 A 消亡：零提交分支（基线可达 HEAD）+ 脏 worktree → 提交照常、合入执行、链续走收口、Seal 守卫零触发 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（Commit 短路砍除与调用序半边——D2/D1） |
| AC-1 | （同上——可达性判定真件基线半边：merge-base 三态持衡） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-2 | 已合入幂等跳过维持：干净跳过提交 + 已合入跳过合入，重试语义等价 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（既有锚改设 fake worktree 干净——重写行） |
| AC-2 | （同上——branch_merged 三态真件半边持衡） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-3 | SpecSync 段位与 cwd：同步在合入后、cwd=主 root、主仓当前基线、产物落主仓工作区 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（段位信封序 / cwd / 探测源恒主仓半边——D8） |
| AC-4 | Finalize 扩围不吞并：pathspec 含 delta capability 子树、无关改动原样保留、merge commit 只含 change 内容 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（pathspec 集断言半边——D9） |
| AC-4 | （同上——pathspec 纪律真件半边：三 pathspec 含 specs 子树 + 无关 capability specs 不吞并） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-5 | 死法 B 消亡：同步产物不滞留 worktree、摘要与磁盘事实同源 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（结构性半边：sync 后移 + cwd + finalize 扩围 + summary 同源——与 AC-3/AC-4 行同锚）；真仓 dogfood 重放 → 不可测试项 5 |
| AC-6 | 冲突 agent 解冲突续链：冲突态保留、会话定式、后验通过、链代收口、无关改动零吞并 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（冲突分支编排 + 会话定式 + 后验编排半边——D3–D6） |
| AC-6 | （同上——冲突态真件半边：Conflicted 清单 / UU 保留 / commit_merge 默认信息） | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs |
| AC-7 | 无法裁决 lean 停链：各失败面 abort 收口 + 冲突摘要 + 手动引导 + 重试幂等续走 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（lean 收敛族参数化半边——D7/D10） |
| AC-7 | （同上——lean 咨询面呈现半边：archive-error 承载冲突摘要与引导） | packages/desktop/src/views/changes/flow/archive-panel.test.tsx |
| AC-8 | 非冲突失败面维持：显式 Err（git 语境 + 引导）、零 agent 会话、db 零变化、可重试 | packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs（Err 面真件维持半边——D3 既有面零加工） |
| AC-8 | （同上——编排停等半边：Err 透传停 Merge 段、零冲突分支调用） | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs |
| AC-9 | 溯源摘除：spec / prompt 面零 skill 措辞、语义单源自持 | packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（prompt 负断言半边——D8/D12）；grep 守线 → 不可测试项 2 |
| AC-10 | 版本与管线：desktop 0.4.26、dev-team 2.10.44 零改动、管线全绿零豁免 | —（见不可测试项 1 / 3） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] / #[tokio::test]、前端 vite-plus）。
     门面 / 纯类型 / 生成物 / 装配模块（port.rs trait 声明面与新载荷类型、lib.rs
     re-export、Cargo.toml、bindings.ts 生成物）不建独立测试文件，统一落「不可测试
     项」声明（含行为去向）。 -->

### packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs

<!-- 既有文件扩展节（本变更最大断言面）。挂 AC-1（提交照常）、AC-2（幂等跳过改设）、
     AC-3/AC-5（段位 / cwd / 探测源）、AC-4（pathspec 扩围）、AC-6（冲突分支编排）、
     AC-7（lean 收敛族）、AC-8（Err 停等）、AC-9（prompt 溯源摘除）。design D2 干净
     探测唯一 / D3 Conflicted 原子判定 / D4 后验纯函数 / D5 会话定式 / D6 链代收口 /
     D7 lean 串与 detail 演进 / D8 段位后移 / D9 扩围 / D10 收敛序——纯函数经
     pub(crate) 可见性直测（同 crate 测试模块），零 test-only export。 -->

#### 待测功能

- verify_resolution(baseline: &WorktreeSnapshot, after: &WorktreeSnapshot, conflicts: &[String]) -> Result<(), String>（pub(crate) 纯函数，D4 四规则）：① after.merge_head 缺席 → Err（违约收口，串附 baseline.head 的 `git reset --hard` 引导）；② 冲突路径在 after 索引仍有 stage>0 条目 → Err 残留冲突；③ 冲突路径 worktree 干净（y==' '）且索引单条 stage-0（或删除缺席）；④ 非 C 路径 porcelain 状态与索引条目（mode/hash/stage/path）A/B 逐字一致、B 无新路径、消失路径 ⊆ 冲突清单 → 否则 Err 清单外新改动
- residual_markers(root: &Path, conflicts: &[String]) -> Vec<String>（pub(crate) fs 扫描，D4）：冲突清单文件逐行扫描行首 `<<<<<<< ` / `>>>>>>> ` 形态，返回 `文件:行` 命中；文件缺席 / 读取失败按无标记处理
- merge_conflict_prompt(change: &str, conflicts: &[String]) -> String（pub(crate) 单点模板，design 数据模型逐字）：冲突清单插值 + 只编辑清单内文件 + 逐文件 `git add` + 禁 `add -A` / commit / merge / rebase / reset / stash + 禁 MCP / `__TOOL_ASK_USER__`
- spec_sync_prompt(change: &str) -> String（pub(crate) 重写版）：增量合并语义自持（ADDED / MODIFIED / REMOVED / RENAMED、保留未提及、幂等、capability 缺席创建）+ 零 skill 溯源措辞（AC-9）
- run_archive_flow / drive：阶段序 Preflight → Commit → Merge（冲突分支）→ SpecSync → Seal → Finalize；Commit 段跳过依据仅干净探测（branch_merged 查询砍除——目录缺失 → skipped「已合入」归置）；Merge 段冲突分支（A 快照 → running detail 更新 → agent 会话 → residual_markers + B 快照 + verify_resolution → commit_merge 收口 / D10 lean 收敛：abort_merge 尽力 + fail_stage lean 串）；SpecSync 段位后移且 cwd 恒主 root；Finalize pathspec = 归档两路径 + delta capability 各 `{domain}/specs/<cap>`（并集脏探测）
- lean 串与提示词汇单点（D7，驻 archive_flow.rs 常量）：模板 `合入冲突无法自动裁决（<原因>）…` + 原因词族（agent 会话失败 / agent 会话被停止 / 残留冲突未解 / 清单外新改动 / 后验探测失败 / agent 违约自行收口 merge）+ Merge running detail `合入冲突，解冲突 agent 裁决中` + passed detail `已解冲突 <n> 文件`——经 Merge 段信封与 Finished error 断言（零 test-only export）
- port 载荷类型消费面：MergeOutcome / WorktreeSnapshot / StatusEntry / IndexEntry 经 FakeVcs 编程与合成快照 fixture 构造（见不可测试项 8 的声明面归属）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_flow_test · 全链正向（worktree + delta specs） | 正向 | 假 vcs（dirty=true / merged=false / merge Ok 且到场 / finalize dirty=true）+ 假 worker（Completed）→ 阶段序恰 Preflight→Commit→Merge→SpecSync→Seal→Finalize 全 passed；vcs 调用序 `dirty(worktree)` → `commit_all` → `branch_merged` → `merge_branch` → `dirty(主 pathspec)` → `commit_paths`（**Commit 段零 branch_merged——D2 砍除的调用序锚**）；worker 恰一次 spec-sync：cwd = 主 root（D8——worktree 在场仍主 root，旧 worktree-cwd 断言反转）、provenance `source="change"` / `source_ref="<change>/archive/spec-sync"`、BypassPermissions / High / Executor / 新会话；finalize pathspec 三拼（active + archive + `{domain}/specs/<cap>`——D9）；worktree 提交信息 `archive: <name>`、落盘信息 `archive: move <name> to archive` 维持；seal 落盘 + summary 四字段（specs=synced） | 重写 |
| archive_flow_test · 零提交分支误判形态提交照常（死法 A） | 正向 | 假 vcs merged=**true**（基线可达 HEAD 的误判现场）+ dirty=**true**（脏 worktree）→ Commit 段照常 `commit_all` 恰一次（调用序首探 `dirty` 非 `branch_merged`——可达性不构成跳过分支）；Merge 段已合入跳过（幂等维持）；主仓目录预置 → 链续走 SpecSync→Seal→Finalize 收口（Seal passed = merge-first 守卫零触发——AC-1 字面） | 新增 |
| archive_flow_test · 已合入幂等双跳过直达收口 | 边界 | 既有锚**改设**：fake worktree 必须干净（`with_dirty([false])`——merged=true + 干净才成立，脏则提交已由上行锚定）；merged=true → Commit skipped「干净」/ Merge skipped「已合入」；`commit_all` / `merge_branch` 零调用；链直达收口（AC-2 / 手动 merge 等价路径） | 重写 |
| archive_flow_test · 干净 worktree 提交跳过 | 边界 | 既有断言面零改动（skipped detail「干净」、`commit_all` 零调用、merge 照常推进——逐段终态断言不涉段序） | 持衡 |
| archive_flow_test · 无 delta specs / 用户跳过两行 | 边界 | 既有断言面零改动（skipped 词汇「无 delta specs」/「用户选择」、零 agent 会话、summary.specs none/skipped、commit 照常） | 持衡 |
| archive_flow_test · worktree 目录缺失且已合入归置 | 边界 | record worktree=Some(缺失路径) + merged=true + 主仓目录预置 → Preflight 通过（缺失+已合入不 Err——既有前置分支）；Commit skipped detail「已合入」（D2 归置：目录缺失跳过因复用词汇，**不设第二次 merged 查询**——branch_merged 总调用数恰 Preflight + Merge 各一次）；`commit_all` 零调用；链续走收口 | 新增 |
| archive_flow_test · merge 冲突 agent 解冲突续链 | 正向 | 假 vcs `with_merge_conflicts(["src/a.txt","src/b.txt"])` + 快照队列 [A, B（由 A 派生的通过形态）] + 主仓冲突文件预置无标记 + 假 worker Completed → Merge 段信封流：running（无 detail）→ running（detail `合入冲突，解冲突 agent 裁决中`——D7 同段后写覆盖）→ passed（detail `已解冲突 2 文件`）；worker 恰一次：prompt = `merge_conflict_prompt(change, list)` 逐字、provenance `source_ref="<change>/archive/merge-conflict"`、cwd = 主 root（D5）、BypassPermissions / High / Executor / 新会话；vcs 调用面：`worktree_snapshot` 恰两次（A 先于 agent、B 后于 agent——请求捕获序对拍）、`commit_merge` 恰一次、`abort_merge` **零调用**；链续走 SpecSync→Seal→Finalize 收口；Finished summary 成功（AC-6 编排半边） | 新增 |
| archive_flow_test · lean 收敛族（参数化九形态） | 异常 | 共同装置：`with_merge_conflicts(["src/a.txt"])` + 各面注入 → Merge failed detail = lean 串（含冲突文件逐行清单 + 「已执行 git merge --abort 恢复主仓干净态」+ 「请手动将分支 change/<name> 合入主仓」引导 + 重试幂等说明）且 Finished error 同串；`abort_merge` 恰一次尽力执行；store 零翻转 / seal 零执行 / fs 零改名。九形态：① worker outcome Failed → 原因词「agent 会话失败」；② worker outcome Stopped → 原因词「agent 会话被停止」+ 串附「（归档链已停止）」注记——**error ≠ 裸「归档链已停止」**（D7 特例：TXT_STOPPED 保留给非冲突段）；③ 主仓冲突文件内容行首 `<<<<<<< ` 在场 → 「残留冲突未解」；④ B 快照非冲突路径 porcelain 漂移（新 untracked 路径）→ 「清单外新改动」；⑤ B.merge_head=None → 「agent 违约自行收口」+ 串附 A.head 与 `git reset --hard` 引导；⑥ B 快照 Err → 「后验探测失败」；⑦ A 快照 Err → lean（后验探测失败）；⑧ `abort_merge` 注入 Err → lean 串附「abort 未成功，请手动核验主仓 git 状态」且原原因词不被掩盖（D10 不静默吞二次失败）；⑨ `commit_merge` 注入 Err → lean 收敛（abort 尽力）+ 串含主仓 merge 态人工收口引导（`git commit --no-edit` / `git merge --abort`——D10「一律」收敛序 + 重试前半截态须人工清） | 新增 |
| archive_flow_test · lean 后重试幂等续走 | 边界 | lean 停链（形态①）后重试：用户手动 merge（换新假件 merged=true + 主仓目录预置）→ Commit skipped（干净）/ Merge skipped（已合入）→ 链续走 SpecSync→收口；已成功段零重复（`commit_all` 零调用——AC-7 scenario 尾半边） | 新增 |
| archive_flow_test · 非冲突失败 Err 停等 | 异常 | 既有「merge 冲突停链」锚的 Err 半边独立：`merge_branch` 注入 Err（git 语境串）→ Merge failed detail = port Err 串逐字（AC-8 零加工）；worker 零会话 / `worktree_snapshot` 零调用 / `abort_merge` 零调用（链侧不二次 abort——port 内尽力已执行）；seal 零执行、store 零写；重试 merged=true 续走（既有后半边保留） | 重写 |
| archive_flow_test · SpecSync 段位与探测源 | 正向 | 阶段信封序断言：Merge 终态信封先于 SpecSync running 信封（段位后移锚——D8）；delta specs 仅布置于主仓 active 树（模拟已随合入进入主仓——locate_change 主仓优先命中）→ worker 恰一次 spec-sync 且 cwd = 主 root；worktree 树零 specs 布置（旧形态「worktree 内探测」的否定锚——死法 B 结构面） | 新增 |
| archive_flow_test · spec 同步 agent 失败停链可重试 | 异常 | 既有锚语义随段位演进**改写**：worker（spec-sync 会话）Failed → SpecSync failed；此时 Commit / Merge 已成功执行（信封在场且 passed——已落主仓产物保留不回滚，R4 半面）；store 零写 / seal 零执行；重试：merged=true + worker Completed → 自同步段续走收口（specs=synced） | 重写 |
| archive_flow_test · 停止收敛（非冲突段） | 异常 | stop_handle 在 spec-sync 会话置取消旗 → Finished error = 「归档链已停止」（TXT_STOPPED 保留面——D7）；既有断言「Commit/Merge 零信封」**反转**为「Seal/Finalize 零信封」（新段序下 Commit/Merge 已过）；vcs 零后续调用 | 重写 |
| archive_flow_test · Finalize 扩围三形态 | 边界 | ① delta 在场 + 同步执行 → finalize 脏探与 `commit_paths` pathspec 集 = [active, archive, `{domain}/specs/<cap>`…]（多 capability 字母序逐个拼接——D9 `domain_dir_name()` 单点）；② delta 在场 + 用户跳过（sync_specs=false）→ **同样扩围**（D9：该子树通常干净，`add -A --` 无事可做，语义无害——跳过不收缩 pathspec 集）；③ delta 缺席 → 两 pathspec 既有形态（既有 finalize 行持衡承载）；落盘信息三形态均维持 `archive: move <name> to archive` | 新增 |
| archive_flow_test · verify_resolution 纯函数分支穷尽 | 边界 | 合成 `WorktreeSnapshot` 直驱（零 FakeVcs——D4「分支覆盖在单测层穷尽」）：Ok 基线（冲突路径单条 stage-0 + y=' '、非冲突路径 status/index A/B 逐字一致）；① after.merge_head=None → Err（违约收口语境）；② 冲突路径索引残留 stage 1/2/3 条目 → Err 残留冲突；③ 冲突路径 y≠' '（worktree 脏）→ Err；③' 冲突路径删除缺席（status 与 index 双缺席）→ Ok；④ 非冲突路径 porcelain 漂移 / B 新增路径 / 消失路径 ⊄ 冲突清单 / 索引条目 mode / hash / stage 任一漂移 → 各自 Err 清单外新改动 | 新增 |
| archive_flow_test · residual_markers 扫描 | 边界 | tempdir 主仓布置冲突清单文件：行首 `<<<<<<< ` 与 `>>>>>>> ` 各命中（返回 `文件:行` 形态）；缩进（非行首）形态不命中；多文件多命中清单聚合；文件缺席 / 读取失败 → 空（无标记处理——D4 容错面） | 新增 |
| archive_flow_test · merge_conflict_prompt 语义锚 | 正向 | 模板断言：change 名插值、冲突清单逐行在场；「只编辑清单内文件 / 清单外一律不动（含未提交 / staged 无关内容）」逐字；逐文件 `git add` 指令；禁令逐字——`git add -A` / `git add .` / `commit` / `merge` / `rebase` / `reset` / `stash` / MCP 工具 / `__TOOL_ASK_USER__`（design 数据模型逐字——AC-6 prompt MUST 面的文本锚） | 新增 |
| archive_flow_test · spec_sync_prompt 溯源摘除锚 | 正向 | 既有 prompt 语义锚随动改写：增量语义四行（ADDED / MODIFIED / REMOVED / RENAMED）、「保留 delta 未提及」「合并幂等」、capability 缺席创建、禁 MCP / `__TOOL_ASK_USER__` / workflow.json / git 各维持在场；**新增负断言**：整串不含 `openspec-archive-change` 且不含 `skill`（AC-9 溯源摘除的行为半边） | 重写 |
| archive_flow_test · 快照与信封形态 | 边界 | 既有断言面零改动：每段 running→终态成对（逐段断言不涉段序）、tag `ipc` 三变体、`ArchiveStage` 六线词（specSync / commit / merge / …——serde 按名序列化，声明序变而线词不变——D1 wire 零迁移锚）、camelCase 字段、Finished 后除名、同段后写覆盖 | 持衡 |
| archive_flow_test · preflight 读面族 | 边界 | preflight 聚合全字段 / None 三态 / 完成度核算与 `phase_next` 对拍 / workflow_type 不支持 / 警告与摘要词汇——读面零触点，既有断言面零改动 | 持衡 |
| archive_flow_test · ArchiveControl 控制面 | 正向 | 登记 / 重入 / 订阅 / 会话槽 / 快照——控制面零触点，既有断言面零改动 | 持衡 |
| archive_flow_test · seal 续半边 / finalize 已落盘 | 边界 | 双写续半边仅补翻转 / 脏探 false 跳过——写面与幂等面零触点，既有断言面零改动 | 持衡 |
| archive_flow_test · legacy 跳 git 段 | 边界 | Commit / Merge 双 skipped「legacy 无 worktree」+ git 段零调用 + agent cwd = 主 root——既有断言面零改动（legacy 链 = 校验 → 同步（cwd=主 root）→ 双写 → 落盘，与 D8 legacy 形态一致） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ArchiveVcsPort（注入依赖入参） | 既有 FakeVcs 扩展：`merge_branch` 返回 `Result<MergeOutcome, String>`（`merge_error` → Err / **`with_merge_conflicts(list)`** → `Conflicted(list)` / 其余 `Merged` + `merge_arrives_from` fs 到场既有）；**`worktree_snapshot` 结果队列**（`Result<WorktreeSnapshot, String>` 按调用序消费——A/B 两拍编程）+ 调用捕获；`commit_merge` / `abort_merge` 调用捕获 + Err 注入；既有 dirty 队列 / merged / branch / commit 捕获保留 | 本节全部链级行（阶段机编排 / 冲突分支 / lean 收敛 / pathspec 参） |
| 合成快照 fixture | `StatusEntry { x, y, path }` / `IndexEntry { mode, hash, stage, path }` / `WorktreeSnapshot` 直构助手（A 基线 → B 通过形态派生；逐规则破坏变体）——纯函数行的零假件直驱 | verify_resolution 分支穷尽行 |
| WorkerAgentPort（注入依赖入参） | 既有 FakeWorker：outcomes 队列按会话序消费（冲突 + 同步两会话串行定式——D5 单槽复用的断言面）+ `WorkerTurnRequest` 全字段捕获（prompt / provenance / cwd 断言面）+ stop_handle 可编程停止注入 | 冲突分支 / 同步段 / 停止面各行 |
| ChangeStateStore（注入依赖入参） | 既有 FakeStore（record + 相位行 + set_archived 故障注入）零扩展 | 本节全部用例 |
| 文件系统 | 真实 tempdir 主仓树 + worktree 树（delta specs / 产物 / 冲突文件内容按需布置——residual_markers 的行首标记 fixture） | 本节全部用例 |
| tokio 运行时 | `#[tokio::test]`（async 阶段机直驱；订阅经真实 broadcast） | 本节全部链级行 |

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

<!-- 既有文件扩展节。挂 AC-6（冲突态真件半边：Conflicted / UU 保留 / commit_merge
     默认信息）、AC-8（非冲突 Err 面维持）、AC-4（pathspec 纪律含 specs 子树）。
     design D3（-z 归一 / 不-abort）+ D4（快照四连）+ R6（-z 全线消除引号 / 非
     ASCII 形态——0.4.18 raw_arg 教训的读取面对偶）。真实 git tempdir 夹具与
     TEST_PATH_LOCK 互斥纪律沿既有。 -->

#### 待测功能

- ProcessArchiveVcs::merge_branch(main_root, branch) -> Result<MergeOutcome, String>: `merge --no-edit` 成功 → `Merged`；非零退出先 `diff --name-only --diff-filter=U -z` 读 unmerged 清单（NUL 分隔归一）——非空 → **不 abort**，返回 `Conflicted(list)`；清单空（非冲突失败）→ 既有面：尽力 `merge --abort` 后 Err 带 git stderr 语境与手动处置引导（AC-8 逐字语义不变）
- ProcessArchiveVcs::worktree_snapshot(main_root) -> Result<WorktreeSnapshot, String>: 四连——`rev-parse HEAD`（A 时点 = merge 前 HEAD）；`rev-parse -q --verify MERGE_HEAD`（退出 0/1 映射 Some/None、其余 Err）；`status --porcelain -z` 解析（XY 两字符 + NUL 分隔路径、untracked "?","?"、rename（R/C）双段跳过原路径记新路径、零引号裸形态）；`ls-files -s -z` 解析（`<mode> <hash> <stage>\t<path>` 逐字段切分）
- ProcessArchiveVcs::commit_merge(main_root) -> Result<(), String>: `commit --no-edit`——采用 MERGE_MSG 默认 merge 信息（`Merge branch 'change/<name>'`，含 `Conflicts:` 段为 git 默认形态——D6 自动合入与冲突解后不分化）
- ProcessArchiveVcs::abort_merge(main_root) -> Result<(), String>: `merge --abort`；Err 上抛（链侧附注——D10）
- 既有五方法（dirty / commit_all / branch_merged / current_branch / commit_paths）零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| git_test · merge 冲突 Conflicted 与冲突态保留 | 异常 | 双方改同一文件 → `Ok(Conflicted([<路径>]))` 清单精确；**冲突态保留**：`.git/MERGE_HEAD` 在场、`status --porcelain` 含 UU 条目、工作区文件含冲突标记（与旧锚「Err + abort 收口」**反转**——D3 不-abort）；非 ASCII 路径（CJK 文件名）冲突 → 清单裸路径零引号零转义（R6 读取面锚） | 重写 |
| git_test · merge 成功 Merged 两形态 | 正向 | ff 形态（主仓居基线）与 non-ff merge commit 形态（主仓前进）各 → `Ok(Merged)`；无关 staged / untracked 前后 `status --porcelain` 逐字一致（既有行返回型 `Ok(())` → `Ok(Merged)` 机械改设，断言面持衡——AC-4 机械断言随冲突路径扩族的基线半边） | 适配 |
| git_test · 非冲突失败 Err 面维持 | 异常 | non-ff + 预置无关 staged（"local changes would be overwritten" 类）→ Err 含 git 语境 + 手动处置引导；仓未落半截 merge 态（无 MERGE_HEAD）；staged 原样保留（既有 merge_nonff 行——Err 返回型零改动，AC-8 真件半边持衡） | 适配 |
| git_test · worktree_snapshot 冲突态四字段 | 正向 | 冲突进行中：`head` = merge 前 HEAD（**HEAD 不前移锚**——D4 注记）；`merge_head` = Some(分支 tip sha)；`status` 含 UU 条目（x='U' y='U'）；`index` 含冲突路径 stage 1/2/3 三方条目（mode / hash / stage / path 逐字段与 `ls-files -s` 原始输出对拍） | 新增 |
| git_test · worktree_snapshot 解析形态族 | 边界 | 干净仓：merge_head=None、status 空、index 全 stage-0；构造混合态——untracked（XY "?","?"）、modified 未 staged（XY " M"）、staged rename（R 码 + `-z` NUL 双段——**新路径记录、原路径段跳过**）、CJK 路径裸形态（零引号——R6 对偶锚）逐条目断言解析像 | 新增 |
| git_test · commit_merge 默认信息收口 | 正向 | 冲突解算（编辑移除标记 + `git add`）后 `commit_merge` → Ok；MERGE_HEAD 消失、index 干净；`log -1 --format=%s` = `Merge branch 'change/<name>'`（git 默认——D6 不分化）；无关 staged / untracked 前后保持（吞并红线的冲突路径扩族半边——AC-6「主仓无关改动零吞并」真件锚） | 新增 |
| git_test · abort_merge 收口与 Err 面 | 边界 | 冲突态 `abort_merge` → Ok：无 UU、无 MERGE_HEAD、工作区内容回冲突前（零破坏）；**非 merge 态再 abort → Err**（git 语境「no merge to abort」类——链侧「尽力 + 附注」面的 Err 来源锚，D10⑧ 的真件前提） | 新增 |
| git_test · commit_paths 三 pathspec 含 specs 子树 | 正向 | 落盘形态夹具：归档改名（旧删新增）+ delta capability 主 specs 子树更新 + **无关 capability specs 用户未提交编辑** + 无关 staged 条目 → `commit_paths([active, archive, {domain}/specs/<cap>])` → 新提交同含改名两路径与 specs 更新（`/**` glob 既有纪律）；无关 capability specs 编辑与无关 staged 前后 `status --porcelain` 逐字一致（**全树通配禁区的机械反面**——AC-4 真件半边） | 新增 |
| git_test · PATH 隔离 Err 面随族扩展 | 异常 | 既有 PATH 隔离行 + 三新方法（`worktree_snapshot` / `commit_merge` / `abort_merge`）各 Err 含「git 不可用」引导（`git()` 执行器既有面随族） | 适配 |
| git_test · 既有归档族行 | 正向 | dirty 全域 / gitignore / pathspec 圈定、commit_all 两行、branch_merged 三态（AC-2 可达性判定的真件基线）、current_branch 两态、构造锚——五既有方法零改动，断言面零改动 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 git 子进程 + tempfile 仓（装置沿既有 `init_repo` / `git` 断言助手 / `worktree_slot` 分支铸造 / `commit_all_fixture` 身份注入）；rename / unmerged / CJK 路径夹具新增；PATH 隔离窗口沿 crate `TEST_PATH_LOCK` 互斥、测毕恢复 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/archive_flow/mod.rs -> packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs

<!-- 既有文件适配节。命令签名 / 装配 / 互斥面 / ArchiveSink 转译零改动（design
     D7「零新 IPC 字段」+ 不变组件核对）——唯一随动面是回环行的阶段信封流序。 -->

#### 待测功能

- 五命令（preflight / start / stop / state / watch）与 ArchiveSink 转译——签名与逻辑零改动，仅模块注释随动（不可测试项 8 归属）；行为面经既有回环行承载

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · start 受理与事件流回环（legacy） | 正向 | 既有回环行的六段线词序断言随执行序适配：`preflight → commit → merge → specSync → seal → finalize`（执行序重整的信封流半边——legacy 双 skipped 下六段仍全到）；其余断言（提前 resolve / summary specs=none / db 翻转 / 目录改名 / 终态除名）零改动 | 适配 |
| mod_test · 其余既有行 | 边界 | preflight 读面 / blank 与 None 口径 / start 拒绝面（blank / 未建档 / 已归档 / run 在案）/ 重入防护 / agent 失败收敛（CliMissing）/ stop / watch / state 快照 / ArchiveSink 转译 / DTO 线面——命令签名零变 + DTO 零增量 → 断言面零改动（agent 失败收敛行：SpecSync failed 时 Commit / Merge 已过为新序常态，该行断言不涉段序零改动） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / Tauri（进程边界） | 既有 MockRuntime 四态托管装置 + capturing_channel / wait_for 既有助手零扩展；PATH 隔离窗口既有装置（CliMissing 合成收敛） | 本节全部用例 |

### packages/desktop/src/views/changes/flow/archive-state.ts -> packages/desktop/src/views/changes/flow/archive-state.test.ts

<!-- 既有文件适配节。挂 D11（ARCHIVE_STAGES 呈现序单点）与 D7（Merge 段 running
     detail 演进的 reducer 归并半边）。reducer 逻辑零改动——`applyArchiveUpdate` /
     `seedArchiveState` 零触点。 -->

#### 待测功能

- ARCHIVE_STAGES: 阶段清单呈现序常量（D11 单点——`['preflight','commit','merge','specSync','seal','finalize']`）
- applyArchiveUpdate: 既有归并逻辑零改动——本变更新锚仅「running 信封携 detail」的同段覆盖链（D7 后写覆盖机制）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive-state · ARCHIVE_STAGES 呈现序 | 边界 | 六值序恰 `preflight / commit / merge / specSync / seal / finalize`（执行序对齐——D11 单点，panel 与本文件同源引用） | 重写 |
| archive-state · Merge 段 detail 演进归并 | 正向 | 同段三连信封：running（detail=null）→ running（detail=`合入冲突，解冲突 agent 裁决中`）→ passed（detail=`已解冲突 2 文件`）→ 单槽终值 passed 携 detail（running 中态被后写覆盖——D7 机制的前端半边） | 新增 |
| archive-state · 既有 reducer / seed / 冻结行 | 边界 | Stage 归并（passed / skipped / failed 各终态）/ SessionEvent 分槽去重 / Finished 两态与冻结 / seedArchiveState / initialArchiveState——逻辑零改动，断言面零改动 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯 reducer 内存直驱（DTO 字面量构造，零 react 零 IPC） | 本节全部用例 |

### packages/desktop/src/views/changes/flow/archive-panel.tsx -> packages/desktop/src/views/changes/flow/archive-panel.test.tsx

<!-- 既有文件适配节。挂 AC-7（lean 咨询面呈现半边）与 D11（呈现序 + Merge 单行
     detail 演进 + 转录区随当前会话）。panel 逻辑零改动（detail 透传 / 失败面即
     咨询面）——data-testid 零新增，断言挂既有挂钩。 -->

#### 待测功能

- 阶段清单呈现：`archive-stage-list` 六行按 ARCHIVE_STAGES 序渲染；`archive-stage-merge` 行的 status 与 detail 透传（D11 单行 detail 演进——不设子阶段行）
- lean 咨询面：`archive-error` 呈现 Finished error（lean 串——冲突摘要 + 引导经既有失败面承载，零新 IPC 字段 / data-testid）
- 转录区：随 `state.sessionId` 当前会话（两会话串行——D5 呈现边界）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| panel · 阶段清单呈现序 | 正向 | 进行态 fixture → `archive-stage-list` 六行 DOM 序 = preflight / commit / merge / specSync / seal / finalize（commit / merge 前置于 specSync——D11 呈现面对齐执行序）；既有行（specSync / commit 状态与 detail 随 state 呈现）fixture 序随动 | 适配 |
| panel · Merge 段子阶段单行呈现 | 正向 | 三 fixture 各驱 `archive-stage-merge[data-status]`：running + detail=`合入冲突，解冲突 agent 裁决中` → 行内呈现该 detail；passed + detail=`已解冲突 2 文件` → 呈现；failed + detail=lean 串 → 呈现（**单行内状态与 detail 演进，零子阶段行**——D11 / R7 呈现面） | 新增 |
| panel · lean 咨询面 | 异常 | Finished error = lean 串 fixture（含原因词 + 冲突文件逐行 + abort 告知 + 手动裁决引导 + 重试说明）→ `archive-error` 呈现该串（冲突摘要与引导完整可见——AC-7 scenario「停合入段呈现冲突摘要与手动裁决引导」的 UI 半边；既有失败面即咨询面零新挂钩） | 新增 |
| panel · 转录区随当前会话 | 边界 | state.sessionId = 冲突会话 id + liveEvents 该会话事件 → `archive-transcript` 呈现且 AgentTimeline 渲染该会话事件；sessionId 切至 spec-sync 会话（两会话串行后半）→ 转录区随最新会话（D5 单槽——进行面只呈现当前会话的呈现边界锚） | 新增 |
| panel · 既有行 | 正向 | 确认对话数据面（警告行 / delta 清单 / 合入告知含目标分支名）/ runActive 拒绝卡 / 跳过同步 checkbox / 轻量确认与取消 / 停止与转录入口 / 结果摘要三态 / warnings 清单 / 既有错误面 fixture——`STAGE_LABEL` 零改动（D11）、逻辑零改动 → 断言面零改动 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC / hooks（进程边界） | 既有 `vi.mock` hooks/use-archive-flow（state / 动作注入）与 use-session-transcript 装置零扩展；AgentTimeline 真实渲染（复用基建） | 本节全部用例 |

---

## 不可测试项

1. **AC-10 版本交付静态半边**：`packages/desktop/package.json` 0.4.25 → 0.4.26（`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）、`plugins/dev-team` 保持 2.10.44 且 SKILL.md 原文未动 — **原因**: 静态版本声明无行为面；由 tasks 阶段四对账（红线自查 grep 的 `plugins/dev-team` 零 diff 复核）承载。
2. **AC-9 grep 守线半边**：spec / prompt / 源码零 `openspec-archive-change` skill 溯源措辞的全仓 grep — **原因**: 静态文本守线属 tasks 阶段四明定动作，无进程内行为断言面；行为半边经 archive_flow_test「spec_sync_prompt 溯源摘除锚」行（prompt 负断言）承载。
3. **AC-10 工具链门半边**：`server:check` / `client:check`（knip 零新增豁免）/ `bindings:check` / `vp test` 全绿 — **原因**: 工具链级套件门非单一测试文件可承载，归 test-execution 阶段承接验证（tasks 阶段四同口径）；`bindings.ts` 预期零语义 diff（specta 枚举 union 声明序文字性重排由 `bindings:check` 守线复核，不手改生成物——design D12），bindings/mod_test 既有线词断言为 set 面与声明序无关，持衡零改动。
4. **crate 图与 grep 守线**：`orchestration` 零依赖新增、git spawn 触点（merge / commit / status / ls-files / rev-parse）仅 `crates/infra/vcs`、walker / steps / snapshot / state / control / write/archive / worker.rs / SessionRecord 面零 diff — **原因**: Cargo 依赖图与全源码 grep 属编译期 / 静态守线（tasks 阶段四明定）；行为半边经 archive_flow_test 假件行（`ArchiveVcsPort` 唯一缝——快照 / 收口 / abort 全经 port，core 零 spawn的类型面证据）承载。
5. **AC-5 dogfood 真仓重放半边**：desktop-archive-change 自身收场现场（零提交分支 + master staged 批次）的完整重放验证 — **原因**: 真仓历史现场重放是集成 / 验收面（需真实 CLI 引擎与真实 agent，非确定）；结构性半边已由编排锚承载——AC-1 行（零提交 + 脏 → 提交照常）、AC-3 行（sync 后移 + cwd + 探测源恒主仓）、Finalize 扩围行（产物经落盘段提交 + summary 同源），四行合取即死法 B 的结构性消灭证明。
6. **AC-6 LLM 实际裁决保真半边**（agent 对冲突标记区块的语义裁决质量、最终消息裁决要点） — **原因**: 冲突解算是智能段（非确定），假引擎不产生真实文件编辑；测试面承载为——merge_conflict_prompt 文本锚（清单 + 只编辑清单内 + 逐文件 add + 禁令逐字）、后验纯函数分支穷尽（`verify_resolution` / `residual_markers`——D4「机械拦截而非信任 agent」的可测化）、会话定式捕获（provenance / cwd / 权限 / 模型档）；真实裁决质量属验收 / 集成面。
7. **R2 已知假阴 bound（D4 留痕）**：后验是状态面对比非内容 forensics——对 A 时点已在案的未提交内容做就地篡改（不 add、状态不变）不可检测 — **原因**: 设计显式接受并留痕（机械红线由「吞并不发生」——收口只 commit 索引内既定内容——兜底，该兜底经 git_test commit_merge 行「无关 staged / untracked 前后保持」承载）；假阴面自身无断言可写。
8. **门面 / 纯类型 / 生成物 / 装配模块不建独立测试文件**：`port.rs` 的 `MergeOutcome` / `WorktreeSnapshot` / `StatusEntry` / `IndexEntry` 载荷声明与 trait 方法面、`orchestration/src/lib.rs` 与 `infra/vcs/src/lib.rs` re-export、`src/commands/archive_flow/mod.rs` 模块注释、`archive-panel.tsx` 注释、`bindings.ts` 生成物 — **原因**: 纯类型 / re-export / 注释 / 生成物无自有行为；行为去向：port 载荷经 archive_flow_test（FakeVcs 编程 + 合成快照 fixture 构造消费）与 git_test（真件解析像对拍）两节承载；命令面注释随动经 mod_test 回环行持衡承载。
9. **D2 微秒级 TOCTOU**（Preflight 与 Commit 段间用户恰在该窗口删 worktree 目录） — **原因**: 设计显式留痕（退化为 dirty 探测失败 → 干净跳过，非破坏性）；无确定性并发断言面；周边行为由「worktree 目录缺失且已合入归置」行（缺失 + 已合入 skipped）与既有「worktree 缺失且未合入 preflight Err」行（前置拦截）两头锚定。
10. **golden 语料零触点注记**：DTO 面零增量（值域不变仅声明序、`ArchiveUpdate` / `ArchiveSummary` / `ArchivePreflight` 逐字不动——design D1/D12），corpus golden 与 fixtures 矩阵零重写零扩展 — **原因**: proposal「golden（若 DTO 演进触线）」条件不成立；`DESKTOP_GOLDEN_REWRITE=1` 流程不启用，留痕防实现期误触。
11. **run 面与会话基建零触点组件的持衡声明**：walker / steps / snapshot / state / control / prompt（相位面）/ write/archive、worker.rs、SessionRecord / 转录面、`change_flow` / `changes` / `bindings` mod_test、use-archive-flow / change-detail-view / session-transcript-panel 前端测试零触点零适配 — **原因**: design 不变组件核对结论明定零 diff（run 面隔离与会话基建复用红线）；「两会话经既有通道透明转译」的命令面半边由 mod_test ArchiveSink 转译行持衡承载（SessionEvent 直译与槽位覆写零改动即单槽语义成立的行为证据）。
12. **环境依赖面**：真实 git 在测试环境的可用性（vcs 真件行前提）、Windows / Unix 平台分支互斥覆盖 — **原因**: 与产品硬依赖同口径的环境前提（git 缺失路径本身有 PATH 隔离 Err 行覆盖）；`-z` 非 ASCII / rename 形态锚覆盖 R6 的解析面，平台差异经 CI 矩阵承载（单平台单测只触达本机分支）。
