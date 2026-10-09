# 提案: archive-merge-first

> **变更**: archive-merge-first
> **日期**: 2026-10-08
> **状态**: draft
> **探索**: `openspec/explores/archive-merge-first.md`（一句话定向：归档链地基缺陷修复——Commit 段 merged 可达性短路在零提交分支上必误判；链形重整为「提交 → 合入 → 主仓同步」，冲突 agent 化，skill 溯源摘除）

---

## 问题

desktop-archive-change（已归档）落地的归档链存在一个地基缺陷与两个结构性死法，其自身的归档收场就是现场：

1. **地基缺陷：Commit 段 merged 短路**。spec 意图是「worktree **干净**则跳过提交；**分支已合入**则跳过合入」，代码实际（`archive_flow.rs` Commit 段）是**先查 merged、true 即跳过，脏不脏不看**。而 `branch_merged` = `merge-base --is-ancestor`（纯可达性判定）。桌面执行相位从不提交（提交是归档时动作）→ 分支零提交是**常态**；零提交分支 tip = 创建基线，主仓只要前进过哪怕一个 commit，基线必为 HEAD 祖先 → merged=true 误判。
2. **死法 A（纯桌面路径）**：Commit+Merge 双跳过 → Seal 撞写面 merge-first 守卫 → `Err` 引导「请先手动 merge」，但 worktree 里还有未提交内容——用户连 merge 都没得 merge（merge 无从携带未提交编辑）。
3. **死法 B（手动 merge 后重试，静默失败）**：用户手动 merge → 点归档 → SpecSync 在 worktree 改 specs → Commit 段 merged=true 跳过 → **spec 同步产物静默搁浅在 worktree**，主仓 specs 永不同步，摘要还报「已同步」。

dogfood 考古（desktop-archive-change 自身收场，2026-10-08 探索核实）：branch `change/desktop-archive-change` 零提交（tip = 基线）；worktree 脏（代码 + change 目录 staged，3 份主 specs 同步产物 unstaged）；master 同批内容 staged 未提交且领先基线 4 个 commit——正是死法 A 的完整现场。

两个附带缺陷：

4. **旧序 SpecSync 骑在冻结基线上合并**：同步在 worktree 内先于合入执行，合并基线是 worktree 检出时刻的旧主 specs；主仓 specs 被其他 change 更新过时，同步产出旧基线结果，再在 git 层反向撞 spec 文件冲突。
5. **冲突面全手动 + 溯源耦合**：merge 冲突 → `merge --abort` + `Err` 一刀切（摩擦大）；spec 把 spec 同步 prompt 语义单源钉在 `openspec-archive-change` SKILL.md 步骤 2，而 CLI skill 即将下线——归档链应为唯一定义源。

---

## 提案

链形重整 + 地基修复 + 冲突 agent 化 + 溯源摘除，四件事一个变更偿还（用户已裁定方向）：

1. **链形重整（读法 A：merge 顺带搬运）**：阶段序由「Preflight → SpecSync → Commit → Merge → Seal → Finalize」重整为「**Preflight → Commit → Merge → SpecSync → Seal → Finalize**」。SpecSync 挪到合入之后、cwd=主仓：delta specs 随 merge 进入主仓 active 树，「specs 进主仓」由 merge 顺带完成，**不设专门搬运动作**（归档诚实性：db 翻 archived 时实现代码必须已在主仓）。结构性收益：死法 B 消灭（sync 产物生在主仓，无处搁浅）；同步合并基线永远是主仓**当前** specs（冻结基线旧病消除）；merge commit 只含 change 内容，更干净。
2. **地基修复（Commit 段砍 merged 短路）**：提交段跳过依据收敛为**干净探测唯一**——脏即提交，分支可达性 MUST NOT 构成提交段跳过分支（零提交分支的可达性命中正是死法 A 根源）。Merge 段保留 merged 幂等跳过（此时可达性判定安全：提交段已保证分支携带内容）。
3. **冲突 agent 化**：合入冲突判据 = merge 非零退出 + 冲突标记在场（`diff-filter=U` 清单非空）；冲突态**保留**（不再 `merge --abort`），唤起解冲突 agent 会话（cwd=主仓、bypassPermissions、既有 WorkerAgent 通道）。爆炸半径由链**后验**机械拦截：解后仍残留冲突标记、或冲突清单之外冒出新的工作区 / staged 改动 → 判「无法裁决」。无法裁决（agent 失败 / 被停止 / 后验不通过）→ **lean 档**（V1）：`merge --abort` 收口 + 停链显式咨询（冲突摘要 + 手动裁决引导）；用户自行 merge 后重发归档，merged 判定幂等续走直达收口。rich 档（待裁决态 + theirs/ours/手改交互咨询面）需新 IPC 面与状态机，留后续变更。
4. **Finalize pathspec 扩围**：sync 产物落主仓工作区后由落盘段提交——pathspec 集合 = 归档改名两路径（active 残迹 + archive 新目录）**+ 本 change delta specs 涉及的 `openspec/specs/<capability>` 子树**。全树 `openspec/specs/**` 通配为禁区（会吞并用户对无关 specs 的未提交编辑，违既有吞并红线）。
5. **溯源摘除**：spec 与 prompt 面零 `openspec-archive-change` skill 溯源措辞（「skill 步骤 2 桌面化」「skill 护栏语义对译」「skill Output On Success 面对译」全部摘除）；spec 同步 prompt 的增量合并语义（ADDED / MODIFIED / REMOVED / RENAMED、保留未提及内容、幂等、capability 缺席创建）由 desktop-change-archive 单源定义，桌面自持。CLI skill 的下线处置另行变更承载，本变更不动 `plugins/dev-team`。
6. **版本**：用户可见缺陷修复，desktop 0.4.25 → 0.4.26（0.4.25 为 desktop-archive-change 自身的未提交 bump）。

---

## 能力

### 新增能力

- 无（既有能力的缺陷修复与语义修订）。

### 修改的能力

- **desktop-change-archive** — 六处 requirement 修订：①「归档编排链与阶段幂等」（阶段序重整为 提交 → 合入 → 同步 → 双写 → 落盘；提交段跳过依据收敛为干净探测唯一）；②「worktree 提交与主仓合入」（提交段可达性短路废除；冲突 agent 化——冲突态保留、解冲突 agent、后验、lean 档无法裁决收口；rich 档留后续）；③「spec 同步 agent」**改名**为「spec 同步 agent 与归档链语义自持」（段位后移、cwd=主仓、合并基线=主仓当前 specs、语义单源桌面自持、失败面随段位重述）；④「双写收口与结果摘要」（落盘段 pathspec 扩围含 delta capability 主 specs 子树；skill 面对译措辞摘除）；⑤「归档入口与前置确认面」（skill 护栏溯源措辞摘除，语义自持）；⑥「版本交付」（0.4.26）。另 Purpose 与 Module Contract 随链形重写。
- **desktop-change-worktree** — 「merge-first 归档引导」措辞对齐新链形（自动合入冲突处置指向解冲突 agent 语义）；「V1 范围与边界留痕」边界 8 对齐（链内冲突 agent 裁决、agent 无法裁决时人工兜底；链外手动 merge 仍走 git 原生冲突处理）。

### 引用沿用（零 delta）

- desktop-change-state-store — 归档双写零改动（改名 + 翻转 + 续半边复用；Finalize 扩围不触写面）。
- desktop-crate-layout — vcs 边界零演进：合入冲突判读（不-abort + 冲突清单）与 `merge --abort` 收口仍属既有「主仓合入」家族成员职责，无新边界成员、无租户迁移。
- desktop-change-orchestration — WorkerAgentPort / StopRegistry / 密封转录基建复用（解冲突 agent 是归档链的第二个会话面，非 walker 成员）。
- desktop-change-queries — `locate_change` / detail 出线零改动（合入后主仓 active 树命中，既有回退序天然成立）。
- desktop-ipc-type-bindings — 既有命令面零新命令（DTO 若随动走 bindings 再生纪律零例外）。
- desktop-corpus-regression / desktop-data-dimensions — 零触点（无 db 模型演进、无新数据维度载体）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs` — 阶段序重整（Commit 提前、SpecSync 后移且 cwd=主仓）；Commit 段 merged 短路砍除（脏即提交）；Merge 段冲突分支（冲突判读 → 解冲突 agent 唤起 → 后验 → lean abort 收口）；Finalize pathspec 扩围（delta capability 主 specs 路径）
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — `ArchiveVcsPort` 签名随动（merge 冲突语义不-abort + 冲突清单读取 + abort 面；形态由 design 定稿）
- `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` — `merge_branch` 冲突语义改不-abort、冲突清单读取（`diff --name-only --diff-filter=U`）、`merge --abort` 收口面
- `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs`（或归档链模块）— spec 同步 prompt 溯源措辞摘除（语义自持重写）；解冲突 agent prompt 模板（新：冲突清单 + 只编辑清单内文件 + pathspec 收口纪律）
- `packages/desktop/src-tauri/src/commands/archive_flow/` — 阶段状态 / 会话事件面随动（解冲突 agent 会话透传；冲突摘要呈现；形态 design 定稿）
- `packages/desktop/src/views/changes/` — 阶段面板顺序随动；lean 档冲突咨询面（停链错误呈现）
- `packages/desktop/src/types/generated/bindings.ts` — 随 DTO 演进再生（若触线）
- `packages/desktop/package.json` — `version` 0.4.25 → 0.4.26

### 测试文件

- `crates/core/orchestration/src/archive_flow_test.rs` — 既有「已合入双跳过直达收口」锚改设 fake worktree 干净才成立；新增锚：零提交 + 脏 → 提交发生（merged=true 误判形态下）；同步段会话 cwd=主仓；冲突 → agent 路径（冲突清单透传 + 后验）；无法裁决 → lean abort + 停链咨询；非冲突失败 Err 面维持；Finalize 扩围 pathspec 断言
- `crates/infra/vcs/src/git_test.rs` — merge 冲突锚随不-abort 语义重写（冲突清单返回、UU 态保留、abort 幂等收口）
- `src/commands/archive_flow/mod_test.rs` — 命令面回环随 DTO 随动
- `packages/desktop/src/views/changes/`（vitest）— 阶段面板新序与冲突咨询面断言（data-testid 挂钩）
- golden（若 DTO 演进触线）— `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（CLI skill 下线另行变更承载；版本 2.10.44、三类交付产物零改动）
- `crates/core/workflow/src/write/archive.rs` 双写语义（单点收口复用零改动）
- walker / 相位机 / `RunUpdate` / `ChangeRunSnapshot` 事件面（run 面隔离红线）
- `SessionRecord` / 会话转录 / 槽位记录面（解冲突 agent 走既有会话基建，零迁移零改形）
- `openspec/specs/**` 既有基线 spec（本变更只写 delta，归档时经本能力自证合并）
- worktree / branch 清理边界（仍为用户手动，desktop-change-worktree 既有裁定维持）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 死法 A 消亡（地基修复） | 零提交分支（tip=创建基线）+ 主仓 HEAD 已前进 + worktree 脏（编辑 / staged / change 目录）：发起归档链 → 提交段照常产生提交（不因基线可达 HEAD 跳过）、合入段执行合入（merge commit 落主仓）、链续走同步与双写收口；Seal merge-first 守卫零触发 |
| AC-2 | 已合入幂等跳过维持 | 分支真已合入（用户手动 merge / 前次链半完成）且 worktree 干净：提交段零动作（干净跳过）、合入段零动作（已合入跳过），链直达同步段与收口；重试语义与既有锚等价 |
| AC-3 | SpecSync 段位与 cwd | 带 delta specs 的 worktree change：同步段在合入段之后执行、会话 cwd = 主 workspace root；主仓 specs 以合入后的**主仓当前**内容为合并基线（构造主仓 specs 先行更新场景，零冻结基线合并、零 git 层 spec 反向冲突）；同步产物落主仓工作区（零 worktree 滞留） |
| AC-4 | Finalize 扩围不吞并 | 收口后落盘段 pathspec = 归档改名两路径 + delta 涉及 capability 的主 specs 路径：归档改名与主 spec 更新同 commit 落盘；主仓无关未提交 / staged / 无关 specs 编辑原样保留（机械可验证）；merge commit 只含 change 内容 |
| AC-5 | 死法 B 消亡（搁浅消灭） | 手动 merge 后再点归档：同步产物不再滞留 worktree——主仓 specs 被更新且随落盘段提交；摘要「已同步」与磁盘事实一致（重放 dogfood 场景验证） |
| AC-6 | 冲突 agent 解冲突续链 | 构造主仓与分支对同一文件不兼容修改：合入段冲突态保留（零 abort）、解冲突 agent 会话发起（cwd=主仓、provenance 在案）；agent 仅编辑冲突清单文件并以清单 pathspec 收口，后验通过 → 链续走至收口；主仓无关改动零吞并 |
| AC-7 | 无法裁决 lean 停链 | agent 失败 / 被停止 / 后验不通过（残留冲突标记或冲突清单外新改动）→ 链执行 `merge --abort` 收口（主仓无半截 merge 态、db 零变化）、停合入段呈现冲突摘要与手动裁决引导；用户手动 merge 后重发归档 → 合入段已合入跳过、链续走收口 |
| AC-8 | 非冲突失败面维持 | merge 非零退出且无冲突标记（主仓状态不允许等）：显式 `Err`（git 语境 + 手动处置引导）、零解冲突 agent 会话、db 零变化、链可重试 |
| AC-9 | 溯源摘除 | spec 与 prompt 面零 `openspec-archive-change` skill 溯源措辞；增量合并语义单源 = desktop-change-archive 自述；`plugins/dev-team` 零改动（版本 2.10.44、SKILL.md 原文未动） |
| AC-10 | 版本与管线 | `packages/desktop` version 0.4.26（`src-tauri/Cargo.toml` 版本不随动）；`vp test` / `client:check` / knip / `bindings:check` 全绿，无新增豁免 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 解冲突 agent 在主仓的爆炸半径（bypassPermissions + 用户主仓，比 worktree 会话大） | agent 越界编辑 / 吞并无关改动（高危） | 中 | 后验机械拦截（残留冲突复查 + 冲突清单外新改动探测）→ lean abort 兜底恢复干净态；prompt 约束只编辑清单内文件 + 只 add 清单（禁 `add -A`）；转录可审计；AC-6/7 机械断言 |
| R2 后验判据的假阳 / 假阴（merge 自身产生的非冲突文件变化 vs agent 越界难辨） | 误判无法裁决（链多停）或漏判越界（吞并发生） | 中 | 冲突清单快照（`diff-filter=U`）在 agent 前后各取一次对比；判定命令族与时机由 design 定稿并真实 git tempdir 夹具锚定 |
| R3 lean 收口 `merge --abort` 丢弃冲突态期间的并发手动编辑 | 用户在主仓冲突态下手动改的内容被回滚 | 低 | 冲突态期间归档链持有该 change 的互斥面（run 发起被拒）；停链文案显式告知 abort 已执行、后续处置归用户 |
| R4 SpecSync 后移的失败面变宽（merge 已落、同步失败停链） | 归档未收口但 change 内容已在主仓 | 中 | 全阶段幂等：重试经已合入跳过 + 同步幂等 + 双写续半边，自同步段续走；阶段面如实呈现停等位置（AC-2/AC-3 锚） |
| R5 Finalize 扩围与用户无关 specs 编辑撞车（pathspec 覆盖同 capability 的用户未提交编辑） | 用户编辑被并入落盘提交（内容不丢但被提交） | 低 | 扩围收敛到 delta 涉及 capability 子树（全树通配禁区）；该子树内容经同步 agent 以当前文件为合并基线，用户编辑保留在合并结果内 |
| R6 Windows git 冲突面解析（porcelain UU 码位、路径引号 / 非 ASCII 形态） | 冲突判读误判（漏判走 Err / 误判走 agent） | 低 | vcs 边界真实 git tempdir 夹具先例；raw_arg 引号教训（0.4.18）纳入测试形态 |
| R7 前端阶段序与冲突子阶段呈现（枚举执行序变、merge → agent → 后验子段状态面） | 阶段面板与实际进度错位 | 低 | 阶段枚举值域不变仅执行序变；冲突子阶段呈现形态由 design 定稿，data-testid 挂钩断言 |
| R8 冲突解算时长（agent 分钟级）无进度反馈 | 用户以为死机重复点击 / 手动 abort | 低 | 既有归档链广播面 + agent 转录实时流复用（spec-sync 同构）；发起幂等防重入（ArchiveControl 既有） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| SpecSync 段位 | 读法 A：合入后、cwd=主仓，deltas 随 merge 进主仓，不设专门搬运动作 | 归档诚实性：db 翻 archived 时实现代码必须已在主仓；死法 B 结构性消灭（sync 产物生在主仓无处搁浅）；合并基线恒为主仓当前 specs | 读法 B（fs 搬运解耦：sync 仍在 worktree、产物 fs 搬运进主仓）——sync 仍骑在旧基线上合并且搬运语义游离 git 之外，弃 |
| Commit 段跳过依据 | 砍 merged 短路，脏即提交（干净探测为唯一跳过依据） | `branch_merged` 是纯可达性判定，零提交分支 tip=基线、主仓前进即误判——死法 A 根源；砍除后 Merge 段的可达性跳过恢复安全（提交段已保证分支携带内容） | 保留短路 + 补「分支零提交」特判——特判面更宽、幂等语义更脆，弃 |
| 冲突处置 | agent 化：冲突态保留 + 解冲突 agent（cwd=主仓）+ 链后验 + lean 档收口 | 用户裁定；abort+Err 全手动摩擦大（探索否决现状）；爆炸半径由后验机械拦截而非信任 agent | 维持 abort+Err（现状）；agent 无后验信任（主仓红线不可裸奔，弃） |
| 无法裁决档位 | V1 lean：abort 收口 + 停链 + 冲突摘要咨询；rich 留后续变更 | explore 第五节裁定；用户自行 merge 后重发即幂等续走（merged 判定天然跳过），lean 已闭环 | rich（待裁决态 + theirs/ours/手改交互面）——需新 IPC 面与状态机，超本变更范围 |
| 溯源摘除 | spec / prompt 面零 skill 溯源措辞，语义单源 = desktop-change-archive 自述 | CLI skill 即将下线，归档链成为唯一定义源；本变更不动 plugin（skill 下线另行承载） | 摘除推迟到 skill 下线变更一并做——两变更间语义单源悬空，弃 |
| Finalize 扩围范围 | delta 涉及 capability 的主 specs 子树圈定（非全树 glob） | 吞并红线：全树 `openspec/specs/**` 会吞并用户对无关 specs 的未提交编辑；delta capability 清单 preflight 已有 | 全树通配（实现省事但违红线精神，弃） |
| 版本交付 | desktop 0.4.25 → 0.4.26 patch | 用户可见缺陷修复沿 0.4.x patch 先例；0.4.25 为 desktop-archive-change 自身未提交 bump | minor bump（无 breaking，过度） |

### 待决问题

- `ArchiveVcsPort` 签名形态：`merge_branch` 返回冲突清单（如 `Result<MergeOutcome, String>` 枚举 Merged / Conflicted(list)）vs 拆 `conflict_files` / `abort_merge` 独立方法——design 定稿。
- 解冲突 agent 会话定式：provenance source_ref 定式（spec-sync 先例 `<change>/archive/spec-sync`，冲突路径对应形）、model level、与 spec-sync 会话的 `ArchiveControl.session` 槽复用（一段一会话 vs 多会话槽）——design 定稿。
- 后验机械面：残留冲突复查与「冲突清单外新改动」探测的命令族与快照时机（merge 冲突后 vs agent 收口后各取一次对比）——design 定稿。
- merge commit 的收口归属：agent 内 `git add <清单>` + `commit` 收口 vs 链后验通过后台代收（pathspec 纪律两态都可守）——design 定稿。
- 冲突摘要呈现载体：Merge 段 failed detail 面词汇 vs 专属咨询面字段——design 定稿。
- 前端阶段面板对合入段子阶段（merge → 解冲突 agent → 后验）的呈现形态与 data-testid——design 定稿。
- 提交信息派生规则（现役 `archive: <change>`）是否随冲突路径分化（merge commit 信息形态）——design 定稿。
- worktree 目录缺失但分支已合入形态下提交段的跳过因词汇（与「干净」/「已合入」既有词汇面的归置）——design 定稿。

### 遗留（本变更不动，留痕）

- dogfood 现场（desktop-archive-change 的 worktree / branch 清理与 master staged 0.4.25 批次提交）由用户手动处置（探索第一节命令）。
- rich 交互咨询面（待裁决态 + theirs/ours/手改）留后续变更。
- CLI skill（`openspec-archive-change`）的下线处置另行变更；本变更仅摘除 spec / prompt 面对其的语义依赖。
