# desktop-change-archive Specification

## Purpose

定义桌面端 change 归档编排链：详情页归档入口与前置确认面（db PhaseRecord 确定性完成度核算，不读 workflow.json / MCP），确定性六阶段链（前置校验 → worktree 提交 → 主仓合入（worktree 内 rebase 重放 + 主仓 ff-only 快进——线性历史零 merge commit）→ spec 同步 agent → 双写收口 → 归档落盘提交）幂等可重入；spec 同步 agent 在主仓合入后的当前基线上合并 delta（增量合并语义由本 spec 单源定义，零 CLI skill 溯源），合入冲突由解冲突 agent 裁决（冲突态囚于 change 私有 worktree，无法裁决时 lean 档停链咨询用户），全程零 run 事件面与用户点击唯一发起源互斥。

## Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 以 run 流程的 archive 相位为归档入口：db `ChangeRecord.status=active` 的 change 在 run 走完 8 个路由相位后 SHALL 停在 archive 相位之前（stop-before-archive 停等面），该停等面 SHALL 呈现归档前置确认数据——完成度核对结论（以该 change id 的 db PhaseRecord eval 序列确定性核算——workflow 相位表全部路由相位存在 non-stale 的 pass / skipped 条目即完成，MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支」的告知。警告 SHALL NOT 阻断（inform + confirm 护栏为本能力自持语义），用户显式确认后 walker 才进入 archive 相位；未确认时 run 停在 archive 前（不自动归档）。已归档（`source=archive` / status=archived）SHALL NOT 再进入 archive 相位（`phase_next` 已路由 done）；无 db 建档的 change（id 不可达）SHALL NOT 提供归档（写面既有拒绝面沿袭）。运行中 run 已在该 run 内，归档相位为该 run 的尾部（零独立归档链互斥面）。

#### Scenario: 未完成警告确认后照常进入归档相位

- **WHEN** 一个存在 fail 相位（无全相位 non-stale pass）的 active change 的 run 到达 archive 前停等面，用户在停等面选择确认
- **THEN** 停等面呈现「工作流未全部通过」警告，确认后 walker 进入 archive 相位并走通收口；警告不构成阻断

#### Scenario: 未确认不自动归档

- **WHEN** run 走完 8 个路由相位到达 archive 前停等面而用户未确认
- **THEN** walker 停在 archive 前（stop-before-archive），不进入 archive 相位、零归档变更；run 处于停等态，用户可确认续走或终止

#### Scenario: 已归档与无建档 id 无入口

- **WHEN** 打开已归档 change（archive 树条目）详情，并审查无 db 记录的 id 情形
- **THEN** 已归档 change 的 `phase_next` 路由 done、不再进入 archive 相位；无建档 id 即使经命令面直调也被写面既有拒绝面拒绝（零落账、零目录改动）

### Requirement: 归档编排链与阶段幂等

归档 SHALL 为 walker 的第 10 个路由相位（`archive`，确定性相位：`PhaseDefinition.executor=None` / `evaluator=None`），10 步有序 step 序列 SHALL 为：`PhaseStart`（开相，attempt max+1 分配）→ `Preflight`（store 读 + vcs 探测的只读校验，detail 携完成度警告 / delta specs 清单 / merge 目标）→ `Commit`（vcs.commit_all，step 内「干净」跳过）→ `Rebase`（vcs.rebase_branch 重放至主仓当前分支，产出冲突清单入 `PhaseCursor.rebase_conflicts`）→ `ResolveConflicts`（条件执行：无冲突 `passed` + `detail` 记「无冲突」；有冲突经解冲突 agent 会话解算）→ `Merge`（vcs.ff_merge 主仓 ff-only 快进）→ `SpecSync`（delta specs 在场且未跳过时 agent 会话，否则 `passed` + `detail` 记跳过）→ `Seal`（经写面 `archive` 双写收口：目录改名 + db status 翻转）→ `Finalize`（vcs.commit_paths 落盘提交，step 内「已落盘」跳过）→ `PhaseLog`（空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord）。每步 SHALL 幂等可重入：**worktree 干净（`status --porcelain` 非空为假）是 Commit 步唯一跳过依据——分支可达性 MUST NOT 构成 Commit 步的跳过分支**；分支已合入则 Rebase / Merge 步幂等跳过（重放已在目标中的提交显式丢弃、ff-only 快进零 merge commit）；双写半完成重试走写面既有续半边（仅补 db 翻转）。delta specs 缺席或用户选择跳过时 SpecSync SHALL 零会话（`passed` + 跳过因）。任一步失败 SHALL 以 `Terminal` 收口（`on_fail=None` 的步失败即 run 失败），MUST NOT 静默跳过或降级；重试靠 step 内判断短路（干净 / 已合入 / 已落盘 / 双写续半边），已成功步不重复执行。

#### Scenario: 10 步按序执行

- **WHEN** 假引擎 + 假 vcs port + 假写面驱动 archive 相位
- **THEN** 10 步按序执行：PhaseStart 开相、Preflight 校验、Commit 提交（或干净跳过）、Rebase 重放、ResolveConflicts 条件解算、Merge 快进、SpecSync 同步（或跳过）、Seal 双写、Finalize 落盘（或跳过）、PhaseLog 空 checklist 落账

#### Scenario: 零提交分支脏工作区照常提交合入

- **WHEN** 桌面执行相位全程未提交（分支零提交、tip = 创建基线）且主仓 HEAD 已前进，worktree 内编辑 / staged / change 目录在场
- **THEN** Commit 步照常产生提交（不因基线可达 HEAD 而跳过）、Rebase 重放 + Merge 快进走通（主仓 HEAD 前移、零 merge commit），写面 merge-first 守卫零触发

#### Scenario: 已合入跳过合入直达收口

- **WHEN** 分支已 merge 回主仓（主仓 active 树已含 `changes/<n>/`）且 worktree 干净
- **THEN** Commit 步零动作（干净跳过）、Rebase / Merge 步幂等跳过（已合入），链径直到 SpecSync / Seal / Finalize 收口

#### Scenario: 步失败即 Terminal 收口

- **WHEN** archive 相位某步失败（commit 失败 / rebase 冲突无法裁决 / seal 半完成 / spec_sync 失败）
- **THEN** 该步以 `Terminal` 收口、run 直接 `Failed`，不唤决策 agent、不触发 phase retry；db 与磁盘保持该步失败前状态；重试经 step 内判断短路续走

### Requirement: worktree 提交与主仓合入

带 worktree 记录的 change，archive 相位 SHALL 经四步完成 worktree 提交与主仓合入：`Commit` 步在 worktree 内提交本 change 编辑集（git add 范围 = worktree 全域——worktree 为 change 私有执行锚，全域即本 change 编辑集；提交信息派生规则由 design 定稿），worktree 脏即提交、干净则跳过，worktree 目录缺失且分支已合入则跳过；`Rebase` 步将 branch `change/<name>` 以 `rebase --empty=drop` 重放至主仓当前分支之上（重放目标 = 主仓当前分支；已在目标中的重放提交显式丢弃），重放冲突时 SHALL 保留冲突态于 worktree（MUST NOT 自动 `rebase --abort`），冲突判据为 rebase 非零退出且冲突清单非空（`--diff-filter=U` 清单），命中时产出冲突清单入 `PhaseCursor.rebase_conflicts`；`ResolveConflicts` 步 SHALL 条件执行——无冲突 `passed` + `detail` 记「无冲突」；有冲突时唤起一个解冲突 agent 会话（cwd = worktree、bypassPermissions、经既有 WorkerAgent 通道），agent SHALL 仅编辑冲突清单内文件并以 pathspec 纪律收口（只 add 冲突清单，禁止 `add -A`），rebase 收口动词（`rebase --continue` / `rebase --abort`）由 walker 代行、agent 无收口权；`Merge` 步 SHALL 在主仓以 `merge --ff-only` 纯快进收口——线性历史零 merge commit。walker SHALL 对解冲突 agent 结果后验（REBASE_HEAD 在场 + 冲突标记复查 + 冲突清单外无新改动）：后验通过则 `rebase --continue` 续走、重放完成后快进；后验不通过（agent 失败 / 被停止 / 残留冲突 / REBASE_HEAD 缺席 / 清单外新改动）SHALL 以 lean 档收口（`rebase --abort` 恢复 worktree 干净态 + 步失败 `Terminal` + 冲突摘要与手动裁决引导）；rich 档留后续变更。非冲突失败（rebase 非零退出且冲突清单为空）SHALL 尽力 `rebase --abort` 后 `Terminal`（携带 git 语境与手动处置引导）。全程 MUST NOT 强制推送、MUST NOT 改写主仓历史；主仓无关未提交改动与无关 staged 条目 MUST NOT 被任何自动提交或解冲突 agent 吞并（主仓全程零冲突态——冲突面囚于 worktree）。legacy 记录（`worktree=None`）SHALL 跳过 Commit / Rebase / ResolveConflicts / Merge 四步（主仓工作区编辑的提交仍为用户手动面，语义见 desktop-change-worktree「V1 范围与边界留痕」）。

#### Scenario: 提交合入成功

- **WHEN** 对 dirty worktree 的 change 走 archive 相位至 Merge 步成功
- **THEN** Commit 步产生一个包含本 change 编辑集的提交（零 spec 同步产物——SpecSync 在 Merge 之后），Rebase 重放完成后主仓 ff-only 快进，`openspec/changes/<n>/` 目录在场且主仓 HEAD 前移（零 merge commit）

#### Scenario: 冲突 agent 解冲突续链

- **WHEN** 分支重放与主仓当前分支对同一文件有不兼容修改时 Rebase / ResolveConflicts 步执行
- **THEN** 冲突态保留于 worktree（零 abort），解冲突 agent 会话发起（cwd = worktree、provenance 在案）；agent 仅编辑冲突清单内文件并以清单 pathspec 收口，后验通过（REBASE_HEAD 在场、零残留标记、清单外零新改动）后 walker 代 `rebase --continue` 收口并快进；主仓无关改动保持归档前状态

#### Scenario: 无法裁决 lean 停链

- **WHEN** 解冲突 agent 失败 / 被停止 / 后验发现残留冲突、REBASE_HEAD 缺席或冲突清单外新改动
- **THEN** walker 执行 `rebase --abort` 收口（worktree 恢复重放前干净态、主仓无任何合入态），该步 `Terminal` 呈现冲突摘要与手动裁决引导；db 零变化；用户手动处置后重发 run，已合入跳过、续走至收口

#### Scenario: 非冲突失败显式引导

- **WHEN** rebase 非零退出且冲突清单为空（worktree git 状态不允许等）
- **THEN** walker 尽力 `rebase --abort` 后 `Terminal` 呈现 git 语境与手动处置引导，零解冲突 agent 会话，db 零变化

#### Scenario: 主仓无关改动不吞并

- **WHEN** 主仓预先存在无关未提交文件与无关 staged 条目，archive 相位全程收口（含解冲突 agent 路径）
- **THEN** 两者保持归档前的 git 状态（未被吞并进任何自动提交或 agent 编辑；主仓全程零冲突态，冲突面囚于 worktree），可机械验证

#### Scenario: legacy 跳过 git 步

- **WHEN** 对 `worktree=None` 的存量建档 change 进入 archive 相位
- **THEN** Commit / Rebase / ResolveConflicts / Merge 四步零动作，链径直经（可选 SpecSync 后）Seal / Finalize 收口

### Requirement: spec 同步 agent 与归档链语义自持

delta specs 在场且用户未选择跳过时，SpecSync 步 SHALL 在 Merge 步之后发起一个 spec 同步 agent 会话：会话 cwd = 主 workspace root（worktree change 与 legacy 同锚——worktree change 的 delta specs 已随合入进入主仓 active 树，legacy 本就在主仓），同步编辑直接落在主仓主基线。prompt SHALL 逐 capability 将 delta spec 增量合并进 `openspec/specs/<capability>/spec.md` 主基线：`ADDED` 追加（requirement 已存在则更新为与 delta 一致）/ `MODIFIED` 增量应用（新增 scenario、修改列中 scenario、修订描述，不复制既有 scenario）/ `REMOVED` 整块移除 / `RENAMED` 改名，保留 delta 未提及的主 spec 内容，合并幂等（对已同步基线重跑零变化）；capability 主 spec 缺席时创建（简 Purpose + ADDED requirements）。以上增量合并语义 SHALL 以本 requirement 为单源定义（归档相位为唯一定义源），spec 与 prompt MUST NOT 引用 CLI skill 文档作为语义或护栏溯源；prompt MUST NOT 依赖 MCP 工具、`__TOOL_ASK_USER__` 或 workflow.json，change 名与路径上下文 SHALL 由桌面插值。会话 SHALL 走既有 WorkerAgent 通道（compose_turn 新会话、CLI 引擎、bypassPermissions、provenance `source="change"` 且 source_ref 携 change id 与归档步语义段——定式由 design 定稿）、转录可观测（实时 + 重放一致）且可停止（StopRegistry 既有终止面）；路径沙箱前缀校验以主仓 root 为锚，同步编辑囚于主仓内。agent 失败或被停止时该步 SHALL `Terminal`（零归档变更：db 零变化、归档树零改名；已完成的提交与合入不回滚），可重试（同步幂等，重试自 SpecSync 步续走）。无 delta specs 或用户选择跳过时 SpecSync SHALL 零会话、`passed` + `detail` 记跳过因。

#### Scenario: 同步基线为主仓当前 specs

- **WHEN** 主仓 `openspec/specs/<capability>/spec.md` 已被先行的其他 change 归档更新（内容超出本 change worktree 创建基线）
- **THEN** 同步以主仓当前内容为合并基线（零冻结基线合并产物、零 git 层 spec 反向冲突）

#### Scenario: 同步产物由落盘步提交

- **WHEN** 带 delta specs 的 worktree change 走 archive 相位至收口
- **THEN** Rebase / Merge 只引入 change 内容（零同步产物——分支重放提交与快进均不含同步产物）；同步产物落主仓工作区，经 Finalize 步扩围 pathspec 提交（见「Seal 与 Finalize 收口」）

#### Scenario: 无 delta specs 零 agent 会话

- **WHEN** 对无 `specs/` 子树的 change 进入 archive 相位
- **THEN** 全程零归档 agent 会话记录（provenance 反查为空），SpecSync 步 `passed` + 跳过因，链直落 Seal / Finalize 收口

#### Scenario: 增量合并保真与幂等

- **WHEN** agent 对含 `## MODIFIED Requirements` 的 delta spec 执行同步，随后同一相位重试再同步一次
- **THEN** 主 spec 仅按增量语义变化（delta 未提及的 requirement 与 scenario 原样保留），两次同步结果一致（幂等）

#### Scenario: agent 失败停链可重试

- **WHEN** 同步 agent 会话以失败收场（或运行中被用户停止）
- **THEN** SpecSync 步 `Terminal`：db 与归档树零变化（已落主仓的提交与合入保留不回滚）；错误呈现后重试自 SpecSync 步续走（已合入跳过、同步幂等）成功至收口

### Requirement: 双写收口与结果摘要

archive 相位 SHALL 经 Seal 步复用写面 `archive` 单点收口（desktop-change-state-store「归档双写」零改动复用：主仓 active→archive 目录改名带 `YYYY-MM-DD-<name>` 前缀 + db status 翻转与 archived_at；改名成功翻转失败的重试走续半边），随后 Finalize 步 SHALL 以 pathspec 圈定自动提交主仓落盘产物：归档改名两路径（active 残迹 + archive 新目录，glob 形态覆盖已删除路径）**与 spec 同步产物路径（本 change delta specs 涉及的 `openspec/specs/<capability>` 子树——全树通配为禁区，无关 specs 的用户未提交编辑 MUST NOT 被吞并）**；脏探测为零则跳过（幂等面）。归档完成后详情页 SHALL 回到已归档形态（`phase_next` 路由 done，归档列全史常驻上图）。

#### Scenario: 收口走通

- **WHEN** archive 相位 Seal / Finalize 步成功
- **THEN** db status=archived、主仓目录已带日期前缀改名、落盘提交覆盖归档两路径与 delta capability 主 specs 路径；详情页回到已归档形态

#### Scenario: 落盘提交扩围含同步产物

- **WHEN** 带 delta specs 的 change archive 相位 Finalize 步执行（主仓 specs 路径脏探测为真）
- **THEN** 归档改名与主 spec 更新同在落盘提交内（pathspec = 归档两路径 + delta capability 主 specs 路径）；用户对无关 specs 文件的未提交编辑与主仓无关改动原样保留（未被吞并）

#### Scenario: 半完成重试仅补翻转

- **WHEN** Seal 步目录改名成功而 db 翻转失败后重试 archive 相位
- **THEN** 前置与 git 步按幂等跳过，Seal 经续半边仅补 db 翻转（不重复改名），Finalize 已落盘跳过

### Requirement: 归档作为 run 相位与手动门

archive 相位 SHALL 是 run 状态机的一部分（第 10 个路由相位），零独立归档链事件面：归档节点 / 边走当前 run 的 `RunNodeRecord` / `RunEdgeRecord` 整包落库，归档实时流经既有 `RunUpdate` / `RunNotice` 变更通知（`archive_flow` 独立 `ArchiveUpdate` / `ArchiveSink` 退役）。「独立手动执行」SHALL 由 stop-before-archive 停等面承载：run 走完 8 个路由相位后 SHALL 停在 archive 前，MUST NOT 自动推进进入 archive 相位；用户显式确认后才进入 archive 相位；archive 相位完成后 run 才 `completed`（"Ready for archiving" 停等语义改写为「等待确认进入归档」）。归档相位的停止 SHALL 复用 run 的停止命令（停止 run 自然停归档相位）。

#### Scenario: 归档节点进 run 执行图

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 10 步节点与串行 forward 边落当前 run 的 `RunNodeRecord` / `RunEdgeRecord`（零新表、零独立归档链事件），详情页 refresh 后归档列全史常驻

#### Scenario: 停等面承载手动门

- **WHEN** run 走完 8 个路由相位到达 archive 前
- **THEN** 停等面呈现，用户确认后进入 archive 相位；run 未确认前不 completed、不归档；停止 run 自然停在 archive 前（零独立归档链停止命令）

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **worktree / branch 清理仍手动**：archive 相位成功后不执行 `git worktree remove` / `git branch -d`（清理为用户手动边界，desktop-change-worktree 既有裁定维持）；桌面 cleanup 按钮留后续变更。
2. **legacy 提交面手动**：`worktree=None` change 的主仓编辑提交仍由用户手动完成，archive 相位不代劳。
3. **主 spec 同步仅归档时**：specs 基线同步只发生在 archive 相位的 SpecSync 步；run 的其余相位不做主基线同步（proposal 相位只写 change 目录内 delta）。
4. **archive 相位不处理主仓无关改动**：主仓无关未提交 / staged 内容原样保留，处置归用户。
5. **无 delta specs 不触碰主 spec**：缺席 delta specs 时主基线零变更（SpecSync 步跳过因注记）。
6. **归档无 evaluator 无 checklist**：archive 相位不落评估 checklist（空 checklist 落账仅为「相位已 pass」的 PhaseRecord 标记，非评估证据面）。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 六条边界均可考，后续变更无需重新论证是否知情

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.36` 升级为 `0.4.37`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44`；CLI skill 的下线处置另行变更承载，本变更仅在 spec / prompt 面摘除对其的语义依赖）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop` version 为 0.4.37 且 `src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` version 仍为 2.10.44，SKILL.md 原文未动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/archive_flow.rs`（键与 provenance） | 归档链 id 寻址 | `ArchiveControl` / `ArchiveRequest` / `ArchiveGuard` 键 `(root, change)` → `(root, id)`；分支判定 `change/<name>` 与 prompt 内文经 id → 记录 → name 供给（git 面零改名）；阶段序固定（校验 → worktree 提交 → 主仓合入（重放 → 冲突 agent 循环 → 快进）→ spec 同步 → 双写收口 → 落盘提交）；提交段跳过依据仅干净探测（可达性不构成跳过分支）；每阶段幂等可重入；失败停该阶段显式呈现；复用 WorkerAgentPort（spec 同步 + 解冲突两会话面）；零 run 事件面；依赖方向 orchestration → workflow 不变 |
| `crates/core/orchestration/src/archive_flow.rs`（agent 两会话） | provenance 身份段换锚 | spec 同步 `source_ref=<id>/archive/spec-sync`、解冲突 `source_ref=<id>/archive/merge-conflict`（形不变、身份段 id；prompt 内文仍插值 name） |
| `crates/core/orchestration/src/port.rs`（ArchiveVcsPort） | 归档链 → vcs 执行的进程内缝 | 消费者 = 归档链；spawn 不进 core，进程执行驻 infra/vcs；方法族 = `rebase_branch`（worktree 重放）/ `ff_merge`（主仓 ff-only 快进）/ `rebase_continue`（链代续走，多停循环）/ `rebase_abort`（lean 收口）/ `worktree_snapshot`（REBASE_HEAD 在场性 A/B 快照）+ 脏探测 / 全域提交 / 已合入判定 / pathspec 提交；`Err` 面带引导文案 |
| `crates/infra/vcs/src/git.rs` | git 工作面进程执行 | `add -A`（worktree 全域）/ `commit`（pathspec 形态，不吞无关 staged）/ `rebase --empty=drop`（冲突态保留 + `--diff-filter=U` 清单读取）/ `rebase --continue` / `rebase --abort` / `merge --ff-only` / `rev-parse -q --verify REBASE_HEAD` / 干净探测子命令族；真实 git tempdir 夹具可测 |
| `crates/core/orchestration/src/prompt.rs`（或归档链模块，design 定稿） | 归档 agent prompt 模板 | spec 同步 prompt（增量合并语义自持——单源为本 spec「spec 同步 agent 与归档链语义自持」，零 skill 溯源）与解冲突 prompt（冲突清单 + 只编辑清单内文件 + pathspec 收口纪律）；变量插值（change 名 / 路径 / 清单）；零 MCP / workflow.json 依赖 |
| `crates/infra/agent/src/worker.rs`（复用） | WorkerAgentPort 实现 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"` + 归档语义 source_ref（定式 design 定稿） |
| `crates/core/workflow/src/write/archive.rs`（复用零改动） | 双写单点收口 | 改名 + 翻转 + 续半边恢复既有语义；归档链末段唯一收口触点 |
| `src/commands/archive_flow/` | 薄命令包装 id 化 | preflight / start / stop / state / watch 定位参数 == change id；互斥校验键 `(root, id)`；缺 id 记录 → 既有拒绝面（`Result<T, String>`）；三件事纪律；冲突摘要呈现载体由 design 定稿；bindings 再生 |
| `packages/desktop/src/views/changes/`（详情页） | 归档入口与链面呈现 | 归档按钮两态可见性；确认对话（警告清单 + worktree 合入告知）；阶段状态（新序 + 冲突子阶段与 lean 咨询面）与结果摘要；agent 会话转录入口（复用 AgentMessages 基建）；data-testid 挂钩 |
| `packages/desktop/src/views/changes/flow/archive-panel.tsx` + `hooks/use-archive-flow.ts` | 面板 id 透传 | 命令携 change id；完成度 / 摘要 / 归档目录（`archivedDir` 含日期前缀）展示面语义不变（name 展示值取自 detail） |
| `src/types/generated/bindings.ts` | IPC 类型跟随 | 新命令与 DTO 再生；`bindings:check` 守卫拦截过期生成物 |
