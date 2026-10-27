# desktop-change-archive Specification（delta: archi-graph-doc-agent）

## MODIFIED Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 以 run 流程的 archive 相位为归档入口：db `ChangeRecord.status=active` 的 change 在 run 走完全部前序路由相位（一次性 initialize + 内容相位）后 SHALL 停在 archive 相位之前（stop-before-archive 停等面），该停等面 SHALL 呈现归档前置确认数据——完成度核对结论（以该 change id 的 db PhaseRecord eval 序列确定性核算——workflow 相位表全部路由相位存在 non-stale 的 pass / skipped 条目即完成（initialize 的一次性 pass / skipped 条目含于核算面），MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支 `change/<name>` 保留）」的告知（清理语义见「Cleanup 步与 worktree 自动清理」——删除磁盘目录为用户可感知动作，确认在先）。警告 SHALL NOT 阻断（inform + confirm 护栏为本能力自持语义），用户显式确认后 walker 才进入 archive 相位；未确认时 run 停在 archive 前（不自动归档）。已归档（`source=archive` / status=archived）SHALL NOT 再进入 archive 相位（`phase_next` 已路由 done）；无 db 建档的 change（id 不可达）SHALL NOT 提供归档（写面既有拒绝面沿袭）。运行中 run 已在该 run 内，归档相位为该 run 的尾部（零独立归档链互斥面）。

**架构变更区块（本期增补）**：确认面 SHALL 另呈现一个独立的「🏛 架构变更」区块（**不是新的确认门**，只提高可见性——确认仍是既有的一个 proceed 动作），数据源为**独立取数命令**（前端在确认面出现时显式取数；MUST NOT 扩 `ArchivePreflight` 字段面、MUST NOT 让 walker 的 Preflight 步依赖 `archi` crate）：

- 内容 SHALL 为：本 change 相对主仓基线的模型结构 diff 摘要——元素新增 / 删除（计数 + FQN 清单）、关系新增 / 删除计数、`gate` 变更清单（FQN + 旧值 → 新值）；另有「查看模型 diff」与「查看新图预览」两个入口（后者以改动后的模型渲染画布）。
- **模型无改动时该区块零占位**（不出现标题、不出现空框）；模型有改动而代码无改动时区块照常出现（模型改动本身即需人工确认的变更面）。
- 模型**结构错**时区块内 SHALL 以红字列出诊断（含 `file:line`）并提示「修复后再归档」，MUST NOT 阻断确认（结构性错误由门禁与 `Preflight` 各处独立处置，本区块只负责可见性）。
- 该区块 MUST NOT 呈现「跨语言依赖」「符号级契约」等本期射程外信息。

#### Scenario: 未完成警告确认后照常进入归档相位

- **WHEN** 一个存在 fail 相位（无全相位 non-stale pass）的 active change 的 run 到达 archive 前停等面，用户在停等面选择确认
- **THEN** 停等面呈现「工作流未全部通过」警告，确认后 walker 进入 archive 相位并走通收口；警告不构成阻断

#### Scenario: 未确认不自动归档

- **WHEN** run 走完全部前序路由相位（含一次性 initialize）到达 archive 前停等面而用户未确认
- **THEN** walker 停在 archive 前（stop-before-archive），不进入 archive 相位、零归档变更；run 处于停等态，用户可确认续走或终止

#### Scenario: 已归档与无建档 id 无入口

- **WHEN** 打开已归档 change（archive 树条目）详情，并审查无 db 记录的 id 情形
- **THEN** 已归档 change 的 `phase_next` 路由 done、不再进入 archive 相位；无建档 id 即使经命令面直调也被写面既有拒绝面拒绝（零落账、零目录改动）

#### Scenario: worktree 清理告知在场

- **WHEN** 带 worktree 记录的 active change 的 run 到达 archive 前停等面
- **THEN** 确认面呈现「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支保留）」告知；legacy（`worktree=None`）change 的确认面零清理告知

#### Scenario: 模型有改动即出现架构变更区块

- **WHEN** change 的 worktree 内模型相对主仓基线有改动（例如新增 `App.Billing`、删除 `App.Legacy`、`App.Billing` 的 `gate` 由 `off` 改 `strict`），run 到达 archive 前停等面
- **THEN** 确认面出现「架构变更」区块并逐项呈现元素 +2 / −1（含 FQN）、关系增删计数、`gate` 变更一条；提供「查看模型 diff」与「查看新图预览」入口；确认动作仍沿用既有单一 proceed（零额外确认门）

#### Scenario: 模型无改动零占位

- **WHEN** change 的 worktree 模型与主仓基线等价（或该 change 从未触碰模型），run 到达 archive 前停等面
- **THEN** 确认面不出现「架构变更」区块（无标题、无空框、无 0 计数行）

#### Scenario: 模型结构错红字提示不阻断

- **WHEN** worktree 模型含结构错（如环 / 未定义引用）时到达 archive 前停等面
- **THEN** 「架构变更」区块内以红字列出诊断（含 `file:line`）并提示修复；用户仍可确认归档（区块零阻断能力）

### Requirement: 归档编排链与阶段幂等

归档 SHALL 为 walker 的第 11 个路由相位（`archive`，确定性相位：`PhaseDefinition.executor=None` / `evaluator=None`——initialize 表头相位之后的末相位），11 步有序 step 序列 SHALL 为：`PhaseStart`（开相，attempt max+1 分配）→ `Preflight`（store 读 + vcs 探测的只读校验，detail 携完成度警告 / delta specs 清单 / merge 目标）→ `Commit`（vcs.commit_all，step 内「干净」跳过）→ `Rebase`（vcs.rebase_branch 重放至主仓当前分支，产出冲突清单入 `PhaseCursor.rebase_conflicts`）→ `ResolveConflicts`（条件执行：无冲突 `passed` + `detail` 记「无冲突」；有冲突经解冲突 agent 会话解算）→ `Merge`（vcs.ff_merge 主仓 ff-only 快进）→ `SpecSync`（delta specs 在场且未跳过时 agent 会话，否则 `passed` + `detail` 记跳过）→ `Cleanup`（vcs worktree 移除尽力步——语义见「Cleanup 步与 worktree 自动清理」）→ `Seal`（经写面 `archive` 双写收口：目录改名 + db status 翻转）→ `Finalize`（vcs.commit_paths 落盘提交，step 内「已落盘」跳过）→ `PhaseLog`（空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord）。Cleanup 与 Finalize 的相对座位由 design 定稿（Finalize 语义不变：主仓归档两路径 + delta capability 主 specs 的脏探测落盘）。每步 SHALL 幂等可重入：**worktree 干净（`status --porcelain` 非空为假）是 Commit 步唯一跳过依据——分支可达性 MUST NOT 构成 Commit 步的跳过分支**；分支已合入则 Rebase / Merge 步幂等跳过（重放已在目标中的提交显式丢弃、ff-only 快进零 merge commit）；Cleanup 幂等面见其单点 requirement（worktree 已不在场即跳过）；双写半完成重试走写面既有续半边（仅补 db 翻转）。delta specs 缺席或用户选择跳过时 SpecSync SHALL 零会话（`passed` + 跳过因）。除 Cleanup（尽力步，步恒 `passed`）外任一步失败 SHALL 以 `Terminal` 收口（`on_fail=None` 的步失败即 run 失败），MUST NOT 静默跳过或降级；重试靠 step 内判断短路（干净 / 已合入 / 已落盘 / worktree 已不在场 / 双写续半边），已成功步不重复执行。

**Preflight 的 rebase 半程挡回（本期增补）**：`Preflight` SHALL 在任何写动作之前（`Commit` 之前）以既有 `worktree_snapshot()` 的 `rebase_head` 在场性检出「上一次归档失败后留下的 rebase 半程」，命中即以 `Terminal` 收口挡回重跑，MUST NOT 继续走 `Commit`：

- **理由（防污染）**：`ResolveConflicts` 的模型冲突分流（见「worktree 提交与主仓合入」）**保留** rebase 半程（MUST NOT 自动 `rebase --abort`，冲突面囚于 worktree）。用户直接重跑归档时，`Commit` 步会以 `git add -A` + `git commit` 在 rebase 中间态产生提交——这类污染极难排查。
- **文案三要素**：`Terminal` 文案 SHALL 含 ① worktree **绝对路径**；② 出路一「在该 worktree 内解完模型冲突后 `git rebase --continue`，再重跑归档」；③ 出路二「放弃本次重放：`git rebase --abort`，再重跑归档（会重新 rebase 一次）」。两条命令 SHALL 可直接照抄。
- **零新 git 能力**：检出只经既有 port 方法，MUST NOT 新增 vcs 能力、MUST NOT 在 `Preflight` 内 spawn git。
- **放行态**：半程不在场（含用户已 `--continue` 或已 `--abort` 后重跑）SHALL 照常放行，后续步按既有幂等短路续走。
- **连带成本（知情接受）**：半程期间该 change 持续占用 active 名额（上限 5）且 worktree 不可被 `Cleanup` 删除；前端失败面 MUST 呈现同一文案（worktree 绝对路径 + 两条命令）。

#### Scenario: 11 步按序执行

- **WHEN** 假引擎 + 假 vcs port + 假写面驱动 archive 相位
- **THEN** 11 步按序执行：PhaseStart 开相、Preflight 校验、Commit 提交（或干净跳过）、Rebase 重放、ResolveConflicts 条件解算、Merge 快进、SpecSync 同步（或跳过）、Cleanup 清理（或幂等跳过）、Seal 双写、Finalize 落盘（或跳过）、PhaseLog 空 checklist 落账

#### Scenario: 零提交分支脏工作区照常提交合入

- **WHEN** 桌面执行相位全程未提交（分支零提交、tip = 创建基线）且主仓 HEAD 已前进，worktree 内编辑 / staged / change 目录在场
- **THEN** Commit 步照常产生提交（不因基线可达 HEAD 而跳过）、Rebase 重放 + Merge 快进走通（主仓 HEAD 前移、零 merge commit），写面 merge-first 守卫零触发

#### Scenario: 已合入跳过合入直达收口

- **WHEN** 分支已 merge 回主仓（主仓 active 树已含 `changes/<n>/`）且 worktree 干净
- **THEN** Commit 步零动作（干净跳过）、Rebase / Merge 步幂等跳过（已合入），链径直到 SpecSync / Cleanup / Seal / Finalize 收口

#### Scenario: 步失败即 Terminal 收口

- **WHEN** archive 相位某步失败（commit 失败 / rebase 冲突无法裁决 / seal 半完成 / spec_sync 失败）
- **THEN** 该步以 `Terminal` 收口、run 直接 `Failed`，不唤决策 agent、不触发 phase retry；db 与磁盘保持该步失败前状态；重试经 step 内判断短路续走（Cleanup 步除外——其失败以 detail 警告呈现、永不 Terminal，见「Cleanup 步与 worktree 自动清理」）

#### Scenario: rebase 半程挡回重跑

- **WHEN** 上一次归档在 `ResolveConflicts` 以模型冲突失败（rebase 半程保留于 worktree，`REBASE_HEAD` 在场），用户未处置直接重跑归档
- **THEN** `Preflight` 步以 `Terminal` 收口挡回：文案含 worktree 绝对路径与两条处置命令（`git rebase --continue` / `git rebase --abort` 后各自重跑）；`Commit` 步零执行（不产生 rebase 中间态提交），db 与磁盘零变化

#### Scenario: 半程已处置则放行

- **WHEN** 用户在 worktree 内已 `git rebase --continue`（或已 `git rebase --abort`）后重跑归档
- **THEN** `Preflight` 因 `REBASE_HEAD` 不在场而放行；`Commit` 按干净或脏两态处置、`Rebase` / `Merge` 按已合入与否幂等短路，链续走至收口

### Requirement: worktree 提交与主仓合入

带 worktree 记录的 change，archive 相位 SHALL 经四步完成 worktree 提交与主仓合入：`Commit` 步在 worktree 内提交本 change 编辑集（git add 范围 = worktree 全域——worktree 为 change 私有执行锚，全域即本 change 编辑集；提交信息派生规则由 design 定稿），worktree 脏即提交、干净则跳过，worktree 目录缺失且分支已合入则跳过；`Rebase` 步将 branch `change/<name>` 以 `rebase --empty=drop` 重放至主仓当前分支之上（重放目标 = 主仓当前分支；已在目标中的重放提交显式丢弃），重放冲突时 SHALL 保留冲突态于 worktree（MUST NOT 自动 `rebase --abort`），冲突判据为 rebase 非零退出且冲突清单非空（`--diff-filter=U` 清单），命中时产出冲突清单入 `PhaseCursor.rebase_conflicts`；`ResolveConflicts` 步 SHALL 条件执行——无冲突 `passed` + `detail` 记「无冲突」；有冲突时唤起一个解冲突 agent 会话（cwd = worktree、bypassPermissions、经既有 WorkerAgent 通道），agent SHALL 仅编辑冲突清单内文件并以 pathspec 纪律收口（只 add 冲突清单，禁止 `add -A`），rebase 收口动词（`rebase --continue` / `rebase --abort`）由 walker 代行、agent 无收口权；`Merge` 步 SHALL 在主仓以 `merge --ff-only` 纯快进收口——线性历史零 merge commit。walker SHALL 对解冲突 agent 结果后验（REBASE_HEAD 在场 + 冲突标记复查 + 冲突清单外无新改动）：后验通过则 `rebase --continue` 续走、重放完成后快进；后验不通过（agent 失败 / 被停止 / 残留冲突 / REBASE_HEAD 缺席 / 清单外新改动）SHALL 以 lean 档收口（`rebase --abort` 恢复 worktree 干净态 + 步失败 `Terminal` + 冲突摘要与手动裁决引导）；rich 档留后续变更。非冲突失败（rebase 非零退出且冲突清单为空）SHALL 尽力 `rebase --abort` 后 `Terminal`（携带 git 语境与手动处置引导）。全程 MUST NOT 强制推送、MUST NOT 改写主仓历史；主仓无关未提交改动与无关 staged 条目 MUST NOT 被任何自动提交或解冲突 agent 吞并（主仓全程零冲突态——冲突面囚于 worktree）。legacy 记录（`worktree=None`）SHALL 跳过 Commit / Rebase / ResolveConflicts / Merge 四步（主仓工作区编辑的提交仍为用户手动面，语义见 desktop-change-worktree「V1 范围与边界留痕」）。

**模型冲突不进 agent 解算（本期增补）**：worktree 模型（`<archi_dir>/` 下任何路径）与主仓冲突时，`ResolveConflicts` SHALL 在任何 agent 会话发起之前按**路径分流** `PhaseCursor.rebase_conflicts`：

```
pending = cursor.rebase_conflicts
   │
   ├─ 含 <archi_dir>/ 下任何路径 → Terminal::failed（零 agent 会话、不 abort）
   │     文案：模型冲突需人工裁决 + worktree 绝对路径 + 「解完模型冲突后重跑归档」
   │
   └─ 全部为代码路径 → 既有 agent 解算流程（逐字零改动）
```

- **判据单点**：路径归属 SHALL 经 `foundation::layout::is_archi_rel_path` 判定（`/` 与 `\` 分隔均归一），MUST NOT 在 core 侧比较 `architecture` 字面量。
- **不 abort**：该臂 MUST NOT 调用 `rebase --abort`——rebase 半程 SHALL 保留给人工处置（既有「冲突态保留于 worktree」语义不破）；由此产生的重入风险由 `Preflight` 的半程挡回兜住（见「归档编排链与阶段幂等」）。
- **理由**：模型是设计权威，agent 自动解算等于让 LLM 替人做架构决策（"哪个模块该依赖哪个"），解错无人能察。
- **混合冲突**：同一冲突清单同时含模型路径与代码路径时 SHALL 走模型臂（整体交人），MUST NOT 先解代码再解模型的半自治形态。
- 该分流 MUST NOT 改变既有代码冲突臂的任何可观察行为（后验三查 / pathspec 纪律 / lean 与 rich 分档 / 非冲突失败处理逐字不变）。

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

#### Scenario: 模型冲突不进 agent 解算

- **WHEN** rebase 冲突清单含 `openspec/architecture/models/02-auth.toml`（单条或与代码路径混合）
- **THEN** `ResolveConflicts` 步以 `Terminal::failed` 收口：零解冲突 agent 会话（run 节点无会话槽位）、零 `rebase --abort` 调用、rebase 半程保留于 worktree；文案含 worktree 绝对路径与「解完模型冲突后重跑归档」引导

#### Scenario: 纯代码冲突路径零回归

- **WHEN** 冲突清单全部为代码路径（无架构域路径）
- **THEN** 行为与本次变更前逐字一致（唤起解冲突 agent、仅编辑清单内文件、pathspec 收口、后验三查、成功续走 / lean 收口两臂均不变）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/views/changes/flow/run-control-panel.tsx`（`ArchiveConfirmCard`） | 归档确认面增区块 | 「架构变更」区块：元素 / 关系增删计数 + `gate` 变更清单 + 「查看模型 diff」/「查看新图预览」入口；模型无改动零占位；结构错红字不阻断；数据经独立取数命令（`archi_change_summary`），零 `ArchivePreflight` 字段扩展 |
| `crates/core/orchestration/src/steps.rs`（`run_archive_preflight`） | Preflight 半程检出 | 以既有 `ArchiveVcsPort::worktree_snapshot` 的 `rebase_head` 在场性判定；命中即 `Terminal`（worktree 绝对路径 + 两条处置命令），零新 vcs 能力、零 spawn |
| `crates/core/orchestration/src/walker.rs`（`step_resolve_conflicts`） | 冲突路径分流 | 冲突清单按 `foundation::layout::is_archi_rel_path` 分流：含架构域路径即 `Terminal::failed` 且零 agent 会话、零 abort；纯代码路径走既有解算（逐字零改动） |
| `foundation::layout::is_archi_rel_path` | 分流判据 | 唯一路径判据（`\` 归一 + 祖先判定）；core 侧零 `architecture` 字面量 |
| `crates/core/orchestration/src/state.rs`（`ArchivePreflight`） | DTO 零扩展 | 字段面逐字不变（架构变更摘要不经 walker 传递，走独立前端取数；golden wire 零 diff） |
