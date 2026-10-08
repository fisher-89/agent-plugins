# 提案: desktop-archive-change

> **变更**: desktop-archive-change
> **日期**: 2026-10-08
> **状态**: draft
> **探索**: `openspec/changes/desktop-archive-change/explore.md`（一句话定向：详情页归档按钮；有 worktree 时提交、合入主干；参考 `plugins/dev-team/skills/openspec-archive-change` 的 prompt 调动 agent 完成归档）

---

## 问题

desktop 的归档链路今天只有半条：写面 `archive` 双写（主仓目录改名 + db status 翻转）与 IPC 薄命令 `archive_change` 已在案（desktop-workflow-db-state D11 明确「IPC 薄命令先行，本轮无前端入口；详情页归档按钮留后续变更」），但三个断点让「完成即归档」走不通：

1. **无前端入口**：归档只能经 db-inspector 级别的命令触达，用户在详情页看到「Ready for archiving」后没有下一步动作面——按钮是 D11 显式挂账的欠款。
2. **worktree change 归档前置全手动**：带 worktree 记录的 change，其目录树与全部编辑囚于 worktree（branch `change/<name>`）；归档写面以主仓目录在场为前置（merge-first 引导），用户必须手动 `git add/commit` worktree、手动 merge 回主仓，才够得着归档按钮背后的双写——「隔离执行」的收益被「手动收口」的摩擦抵消（worktree 变更 R2 留痕的「完成即归档即提交」期望未兑现）。
3. **delta specs 同步无承担者**：`openspec-archive-change` skill 的步骤 2（把 `openspec/changes/<n>/specs/**` 增量合并进 `openspec/specs/**` 主基线——ADDED/MODIFIED/REMOVED/RENAMED 增量语义、保留 delta 未提及内容、幂等）是 CLI 工作流专属；desktop 归档双写只改名，主规格基线永不同步——归档后的 specs 树相对 change 产物是失忆的。

explore 核实的四个有利事实：**写面 archive 单点已含半完成续走分支**（改名成功而翻转失败 → 重试仅补翻转；agent 先行改名的形态亦可被「archive 树定位命中 + db 仍 active」分支吸收）；**vcs 边界在案**（`crates/infra/vcs` git 子命令族 + `WorktreePort` 六方法，commit / merge 只是同族新增子命令，无新边界）；**WorkerAgent 通道可复用**（compose_turn 新会话 + provenance + StopRegistry 终止 + 密封转录——归档 agent 与 executor 同构，非相位 run 成员）；**完成度可确定性核算**（db PhaseRecord eval 序列即 skill `workflow_done` 的等价单源，无需 MCP `change_list` / workflow.json）。

三个硬点：主仓自动 merge 有冲突与吞并风险（主仓可能有无关未提交 / staged 改动——自动提交 MUST 以已知路径圈定）；spec 同步是智能段（delta 合并需 LLM 判断，不能确定性编码）；归档链不得污染相位 run 的事件面与状态机（「终态不自动归档」红线不动，归档唯一发起源是用户点击）。

---

## 提案

把归档补成一条「用户点击 → 确定性编排链」的完整收口链，智能段交 agent、机械段留进程内：

1. **归档入口与确认面**：change 详情页提供归档按钮（db status=active 可见；已归档 / 文档形态不呈现）。点击先出确认对话：完成度核对结论（相位表全相位 non-stale pass/skipped = workflow_done 的 db 等价核算）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓」的告知；警告不阻断（skill 护栏对译：inform + confirm），运行中 run 显式拒绝发起。
2. **归档编排链（确定性、幂等、可重入）**：前置校验 →（有 delta specs）spec 同步 agent →（带 worktree）worktree 提交 → 主仓合入 → 写面 `archive` 双写收口。每阶段幂等：worktree 干净跳过提交；分支已合入（主仓目录已在场）跳过合入；双写半完成走既有续半边。任一阶段失败停在该阶段、显式呈现、重试自最近未完成阶段续走。
3. **worktree 提交与主仓合入（vcs 边界扩展）**：worktree 内提交本 change 编辑集（add 范围 = worktree 全境，天然只圈本 change）；主仓将 `change/<name>` 合入主仓当前分支；冲突 / 主仓状态不允许 → 显式 Err 带 git 语境引导手动，MUST NOT 强推、MUST NOT 改写主仓历史、MUST NOT 吞并主仓无关改动（pathspec 纪律）。legacy（worktree=None）跳过 git 段（主仓编辑提交仍为用户手动面）。
4. **spec 同步 agent（skill prompt 桌面化）**：delta specs 在场时发起一个归档 agent 会话，prompt 语义取 `openspec-archive-change` SKILL.md 步骤 2（增量合并 + 幂等），change 名与路径由桌面插值；skill 的 change 选择 / ask-user / `change_list` 步骤由桌面确认面与 db 核算替代（MUST NOT 依赖 MCP 工具与 workflow.json）。会话走既有 WorkerAgent 通道（CLI 引擎、provenance `source="change"`、转录可观测可停止、cwd = 同步对象 root，路径沙箱囚禁同步编辑）。worktree change 上同步先于提交合入——spec 同步产物随本 change 的提交合入主仓，主仓 aftermath 最小化。
5. **双写单点收口**：链末经既有写面 `archive` 收口（零改动复用：目录改名 + status 翻转 + 日期前缀 + 续半边恢复）。收口后呈现结果摘要（change 名 / 归档位置 / specs 同步与否 / 警告——skill Output On Success 面对译）；主仓归档改名的落盘处置（自动 pathspec 提交 vs 留给用户）由 design 定稿，无论何者 MUST NOT 吞并无关改动。
6. **run 面隔离**：归档链独立于相位 run 状态机——零 `RunUpdate` / `ConfirmWait` 事件，walker 与相位机零改动；run 收口「Ready for archiving」照常停等，不自动触发归档（用户点击是唯一发起源）；归档链进行中拒绝该 change 的 run 发起（互斥）。
7. **spawn 落位**：commit / merge / pathspec 提交为 vcs 边界既有家族扩展（`WorktreePort` 增方法或新 port 缝——消费者归属，design 定稿），进程执行留 `crates/infra/vcs`，core 零进程 spawn 红线不变；归档编排链落 core/orchestration（walker 的姊妹运行时，复用 WorkerAgentPort 与端口纪律）。

---

## 能力

### 新增能力

- **desktop-change-archive** — change 归档编排与入口：详情页归档按钮与确认面（完成度警告不阻断）、确定性编排链（校验 → spec 同步 agent → worktree 提交 → 主仓合入 → 双写收口）、阶段幂等与失败续走、worktree 提交 / 主仓合入的 vcs 面（不吞无关改动、冲突显式引导）、spec 同步 agent（skill prompt 桌面化、provenance 与转录可观测）、run 面隔离与互斥、结果摘要、V1 边界留痕（worktree / branch 清理仍手动、legacy 提交面手动）。

### 修改的能力

- **desktop-change-worktree** — 「merge-first 归档引导」requirement 修订：归档链（desktop-change-archive）承担自动提交 + 合入（用户手动 merge 仍为等价满足前置的路径）；run 收口「不自动归档不自动 merge」语义不变（归档链由用户点击发起，非 run 收口自动动作）；V1 边界「worktree / branch 清理手动」与「并行 merge 冲突手动解」维持（归档链冲突失败走 git 原生手动处置）。
- **desktop-change-state-store** — 「归档双写」requirement 条款收窄：双写操作本体 MUST NOT 触碰 worktree / branch（清理仍为用户手动边界）；归档链前置段（提交 / 合入，见 desktop-change-archive）不属双写本体——消除「归档不触碰 worktree」与归档链自动提交合入的表面冲突。
- **desktop-crate-layout** — vcs 边界词汇扩展：git 工作面家族增「归档提交 / 合入 / pathspec 提交」进程执行成员（租户归位表行随动）；七类边界分类学与依赖规则零变化。

### 引用沿用（零 delta）

- desktop-change-orchestration — WorkerAgentPort / compose_turn 会话通道、StopRegistry 终止面复用；walker、相位机、RunUpdate 事件面零改动（归档会话非 walker 成员，provenance 定式扩展由 desktop-change-archive 自述）。
- desktop-change-flow-view — 详情页为归档入口宿主（组件落位与 data-testid 由 design 定夺）；会话转录呈现复用 AgentTimeline / 会话重放基建，无第二套时间线。
- desktop-change-queries — detail / locate_change / 产物发现零改动（worktree 路径出线既有面直接供归档链消费）。
- desktop-ipc-type-bindings — 新 IPC 命令随 bindings 再生纪律（`bindings:check` 守卫）零例外。
- desktop-corpus-regression / desktop-data-dimensions — 零触点（无 db 模型演进、无新数据维度载体）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs`（新）— 归档编排链（阶段推进 / 幂等跳过 / 失败停等；复用 WorkerAgentPort，vcs 缝经 port）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/worktree.rs`（或新 port，design 定稿）— 提交 / 合入 / pathspec 提交 port 方法扩展（消费者归属；sync 签名纪律不变）
- `packages/desktop/src-tauri/crates/infra/vcs/src/git.rs` — `add` / `commit`（pathspec 形态）/ `merge` / 干净探测子命令族扩展（真实 git 语义；引导文案沿既有风格）
- `packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs`（或归档链模块内）— 归档 agent prompt 模板（skill 步骤 2 语义桌面化 + 变量插值）
- `packages/desktop/src-tauri/src/commands/changes/`（或新归档命令组，design 定稿）— 归档链 IPC 入口（发起 / 状态 / 停止面形态由 design 定稿；三件事纪律）
- `packages/desktop/src/views/changes/` — 归档按钮（详情页头部 / 运行控制面板区，design 定夺）、确认对话（警告清单 + worktree 告知）、阶段状态与结果摘要呈现、agent 会话转录入口
- `packages/desktop/src/types/generated/bindings.ts` — 随 IPC / DTO 演进再生
- `packages/desktop/package.json` — `version` 0.4.20 → 0.4.21

### 测试文件

- `crates/core/orchestration/src/archive_flow_test.rs`（新）— 假 vcs port + 假引擎直驱：阶段序、幂等跳过（干净 worktree / 已合入）、失败停等与续走、agent 失败停链、无 delta specs 跳过 agent、run 互斥拒绝
- `crates/infra/vcs/src/git_test.rs` — 真实 git tempdir 夹具：提交 / 合入成功、merge 冲突失败面、pathspec 提交不吞无关 staged、干净探测
- `src/commands/changes/mod_test.rs`（或新命令组测试）— IPC 薄包装回环、DTO 字段面
- `packages/desktop/src/views/changes/`（vitest）— 按钮可见性两态、确认对话警告呈现、阶段 / 结果摘要断言（data-testid 挂钩）
- golden（若 DTO 演进触线）— `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程

### 删除文件

- 无。

### 不要修改

- `plugins/dev-team` 全部（`openspec-archive-change/SKILL.md` 为只读参考源；版本 2.10.44、三类交付产物零改动）
- `crates/core/workflow/src/write/archive.rs` 双写语义（单点收口复用；至多注释面随动）
- walker / 相位机 / `RunUpdate` / `ChangeRunSnapshot` 事件面（run 面隔离红线）
- `SessionRecord` / 会话转录 / 轮统计行记录面（零迁移零改形）
- `openspec/specs/**` 既有基线 spec（本变更只写 delta，归档时经本能力自证合并）
- workflow.json 双向墙（归档链零 workflow.json 触点）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 归档入口与确认面 | db status=active 的 change 详情呈现归档按钮；点击出确认对话（完成度结论 + delta specs 在场情况 + worktree 提交合入告知）；已归档（source=archive）不呈现；文档形态（无 db 记录）无入口；运行中 run（复合键在案）发起归档显式拒绝 |
| AC-2 | 完成度警告不阻断 | 构造有 fail 相位 / 缺产物 change：确认对话呈现警告清单，用户确认后链照常发起并走通；全 pass 且产物齐备 change 无警告直接确认 |
| AC-3 | worktree 提交与合入 | dirty worktree（含 agent 同步产物）→ 归档链产生一个提交（编辑集 + spec 同步产物同commit）；主仓 merge 后 `changes/<n>/` 在场且主仓 HEAD 前移；干净 worktree 跳过提交；分支已合入（目录已在场）跳过合入直达收口 |
| AC-4 | 不吞并无关改动 | 主仓预先构造无关未提交文件与无关 staged 条目，归档链全程收口后两者保持原状态（未被任何自动提交吞并；pathspec 纪律可机械验证） |
| AC-5 | spec 同步 agent | 有 delta specs：归档 agent 会话发起（provenance 在案、转录可达、cwd = worktree / 主 root），主 spec 按 ADDED/MODIFIED 增量语义更新且保留 delta 未提及内容，重跑幂等；无 delta specs：零 agent 会话，链直落 git 段 / 收口 |
| AC-6 | agent 失败停链可重试 | agent 失败或被停止 → 链停在同步段（零提交 / 零合入 / 零归档变更），错误呈现；重试自同步段续走成功 |
| AC-7 | 双写收口与摘要 | 链末经写面 archive 收口：主仓目录改名带日期前缀 + db status=archived + archived_at；结果摘要呈现（名 / 归档位置 / specs 同步与否 / 警告）；半完成（改名后翻转失败）重试仅补翻转（续半边既有回归） |
| AC-8 | merge 冲突显式失败 | 构造主仓与分支改同一文件：合入段显式 Err（git 语境 + 手动处置引导），db 与主仓零破坏，链可重试（用户手动解冲突后再发起直达收口） |
| AC-9 | legacy 与 run 面隔离 | worktree=None change：跳过 git 段，主 root 同步 + 双写收口走通；既有 `archive_change` 裸双写语义零回归；归档链全程零 RunUpdate / ConfirmWait 事件；run 收口「Ready for archiving」不自动触发归档；归档链进行中该 change 发起 run 被拒 |
| AC-10 | 版本与管线 | `packages/desktop` version 0.4.21；`plugins/dev-team` 2.10.44 零改动（SKILL.md 原文未动）；`vp test` / `client:check` / knip / `bindings:check` 全绿，无新增豁免 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 主仓自动 merge 冲突（分支落后主仓 / 并行 change 撞文件） | 归档链中断，用户需手动解冲突 | 中 | 冲突显式 Err + git 语境引导（沿 worktree V1 边界 8 的 git 原生处置）；幂等续走使解冲突后重发直达收口；确认面告知将执行合入 |
| R2 自动提交吞并主仓无关改动（未提交 / staged 混入） | 用户工作区被污染（高危） | 中 | add 范围 = worktree 全域（天然只圈本 change）；主仓侧提交仅以已知归档路径 pathspec 圈定且由 design 定夺是否自动；AC-4 机械断言 |
| R3 spec 同步质量漂移（delta 合并错漏 / 非幂等） | 主规格基线被错误改写 | 中 | prompt 语义单源取 SKILL.md 步骤 2（ADDED/MODIFIED/REMOVED/RENAMED + 保留未提及内容 + 幂等）；同步产物在 worktree 提交内可 diff 可回滚（git 面）；转录可审计 |
| R4 合入目标分支漂移（用户在主仓切到非预期分支后归档） | change 合入错误分支 | 低 | 合入目标 = 主仓当前分支并在确认面呈现；design 定夺额外防护（如确认对话展示当前分支名） |
| R5 归档链与 run 并发竞态（归档中发起 run / run 中发起归档） | db 与 worktree 状态互相踩踏 | 低 | 双向互斥：运行中 run 拒绝归档发起（AC-1）；归档链进行中拒绝 run 发起（AC-9）；互斥登记载体由 design 定稿 |
| R6 半完成态扩散（merge 成功后收口失败 / agent 停止后重试语义） | 归档链卡中间态 | 低 | 全阶段幂等设计（AC-3/6/7）：干净跳过提交、已合入跳过合入、双写续半边既有、agent 同步幂等 |
| R7 Windows git 面新 spawn 族（merge/commit 输出语境解析、长路径、引号形态） | 误判成功 / 失败面不可读 | 低 | vcs 边界真实 git tempdir 夹具先例（PATH 隔离互斥）；raw_arg 引号教训留痕（0.4.18 先例）纳入测试形态 |
| R8 归档链时长（agent 同步分钟级）无进度反馈 | 用户以为死机重复点击 | 中 | 阶段状态呈现 + agent 会话转录入口（实时流）；发起幂等防重入（design 定稿命令面形态） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 归档入口位置 | change 详情页按钮 + 确认对话（完成度警告不阻断） | explore 定向；desktop-workflow-db-state D11 挂账偿还；skill 护栏（don't block on warnings, inform and confirm）对译 | 菜单项 / 命令行 only（discoverability 差，与 D11 欠款不符） |
| agent 职责切分 | 智能段（delta spec 同步）归 agent；机械段（提交 / 合入 / 双写）进程内确定性执行 | 延续「零 CLI 子进程写通道」哲学：状态变更唯一经写面、确定性步骤不烧 agent；双写单点与续半边零改动复用 | agent 全程执行 skill（含 mv 改名）→ 落库翻转脱离桌面掌控、机械段不可测，弃 |
| 同步与提交合入排序 | worktree change：同步先于提交合入（产物随 merge 入主仓，主仓 aftermath 最小） | spec 同步产物与本 change 编辑集同一提交合入，主仓收口后仅剩归档改名待落盘 | 先合并后主仓同步（同步产物落主仓未提交区，aftermath 更脏，弃） |
| 合入目标 | 主仓当前分支（HEAD 所在分支） | worktree 基线自主仓 HEAD 派生，「主干」即主仓当前分支；确认面呈现告知 | 固定 main / master（命名假设不可靠） |
| 双写收口 | 既有写面 archive 单点复用（零改动） | 半完成续走分支已吸收中间态；归档链只在其前加段 | 归档链内联改名 + 翻转（复制语义、丢续半边，弃） |
| 完成度核对载体 | db PhaseRecord eval 序列确定性核算（全相位 non-stale pass/skipped） | workflow_done 的 db 等价单源；零 workflow.json / MCP 依赖（双向墙不动） | agent 内调 MCP change_list（依赖 CLI 面 + workflow.json，违双向墙，弃） |
| spawn 落位 | vcs 边界既有家族扩展（port 缝消费者归属，进程执行留 infra/vcs） | commit / merge 与 worktree 建域同族；七类分类学零变化；core 零 spawn 红线不变 | 新边界 / 寄居壳层（分类学噪音，弃） |
| 归档编排链落位 | core/orchestration 姊妹运行时（复用 WorkerAgentPort 与端口纪律），命令层薄包装 | 编排属运行时域；与 walker 共享 agent 缝与可观测基建而互不侵入 | command 层编排（违「命令体薄包装」纪律）；core/workflow（写面无 agent 面，弃） |
| worktree / branch 清理 | 仍手动（V1 边界维持），归档链不 remove / 不删 branch | explore 未覆盖；清理补偿语义独立（worktree 变更 D5 哲学不变）；后续变更偿还 | 归档成功即清理（超范围，弃） |
| 版本交付 | desktop 0.4.20 → 0.4.21；dev-team 零改动 | 用户可见新功能沿 0.4.x patch 先例（0.4.13→0.4.14 worktree 同型）；SKILL.md 为只读参考源 | minor bump（无 breaking，过度） |

### 待决问题

- IPC 命令面形态：既有 `archive_change` 升级为归档链入口 vs 新增流程命令（发起 / 状态 / 停止）+ 裸双写命令保留——design 定稿。
- 归档 agent 的 provenance source_ref 定式（`<change>/archive/<n>` 或 role 词汇扩展）与 prompt 模板归属（orchestration prompt.rs vs 归档链模块内单点）——design 定稿。
- 提交信息派生规则（change 名 / goal 截断 / 固定前缀）与合入形态（默认 merge commit vs `--ff-only` 优先回退）——design 定稿。
- 归档收口后主仓改名落盘：自动 pathspec 提交（已知归档路径圈定）vs 留给用户手动提交——design 定稿缺省值（本 proposal 倾向自动，AC-4 已约束不吞并）。
- 归档链状态呈现载体：一次性命令 await + 阶段轮询/事件 vs 运行状态 Channel 同构流（agent 执行先例例外）——design 定稿。
- 归档进行中的互斥登记载体（run 注册表复合键复用 vs 归档链自带登记）与 run 发起侧反向校验——design 定稿。
- 确认面对话的可选项面（是否提供「跳过 spec 同步仅归档」——skill "Archive without syncing" 对译）与合入目标分支名呈现——design 定稿。
- 无 delta specs 时是否仍提供结果摘要中的 specs 行（"No delta specs" 对译）与警告词汇单点——design 定稿。
