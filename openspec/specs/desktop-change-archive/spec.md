# desktop-change-archive Specification

## Purpose

定义桌面端 change 归档编排链：详情页归档入口与前置确认面（db PhaseRecord 确定性完成度核算，不读 workflow.json / MCP），确定性六阶段链（前置校验 → worktree 提交 → 主仓合入 → spec 同步 agent → 双写收口 → 归档落盘提交）幂等可重入；spec 同步 agent 在主仓合入后的当前基线上合并 delta（增量合并语义由本 spec 单源定义，零 CLI skill 溯源），合入冲突由解冲突 agent 裁决（无法裁决时 lean 档停链咨询用户），全程零 run 事件面与用户点击唯一发起源互斥。

## Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 提供归档入口（按钮）：db `ChangeRecord.status=active` 的 change 可见可用；已归档（`source=archive` / status=archived）SHALL NOT 呈现入口；无 db 建档的文档形态 change（存量 CLI change）SHALL NOT 提供归档（写面既有拒绝面沿袭）。点击 SHALL 先呈现确认对话再发起，对话 SHALL 呈现：完成度核对结论（以 db PhaseRecord eval 序列确定性核算——workflow 相位表全部相位存在 non-stale 的 pass / skipped 条目即完成，MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支」的告知。警告 SHALL NOT 阻断（inform + confirm 护栏为本能力自持语义），用户显式确认后照常发起；无警告时确认为轻量确认。该 change 存在运行中 run（运行注册表复合键在案）时发起归档 SHALL 显式拒绝。

#### Scenario: 未完成警告确认后照常归档

- **WHEN** 打开一个存在 fail 相位（无全相位 non-stale pass）的 active change，点击归档并在确认对话中选择确认
- **THEN** 对话呈现「工作流未全部通过」警告，确认后归档链照常发起并走通收口；警告不构成阻断

#### Scenario: 已归档与文档形态无入口

- **WHEN** 打开已归档 change（archive 树条目）与无 db 记录的存量 CLI change 详情
- **THEN** 两者的详情页均无归档按钮；对后者即使经命令面直调也被写面既有拒绝面拒绝

#### Scenario: 运行中 run 拒绝归档

- **WHEN** 某 change 的 run 运行中（复合键在案）时点击归档发起
- **THEN** 显式拒绝（呈现运行中原因），归档链零阶段执行、db 与磁盘零变化

### Requirement: 归档编排链与阶段幂等

归档 SHALL 为确定性编排链（独立于相位 run 状态机），阶段序为：前置校验（建档在案 + status=active + 无运行中 run）→（记录带 worktree）worktree 提交 → 主仓合入 →（delta specs 在场且未被用户跳过）spec 同步 agent → 写面 `archive` 双写收口 → 归档落盘提交。每阶段 SHALL 幂等可重入：**worktree 干净（`status --porcelain` 非空为假）是提交段唯一跳过依据——分支可达性 MUST NOT 构成提交段的跳过分支**（桌面执行相位从不提交，零提交分支 tip = 创建基线，主仓前进即令基线可达 HEAD；以此跳过提交是既有的死法根源）；分支已合入（branch `change/<name>` 已可达主仓 HEAD——用户手动 merge 或前次链半完成的等价吸收）则跳过合入；双写半完成重试走既有续半边（仅补 db 翻转）。delta specs 缺席或用户选择跳过时 SHALL 零同步会话。任一阶段失败 SHALL 显式停在该阶段（错误呈现，MUST NOT 静默跳过或降级），重试自最近未完成阶段续走；已成功阶段不重复执行（合入段失败停链时已落主仓的 merge 结果不回滚，重试经已合入跳过吸收）。

#### Scenario: 干净 worktree 跳过提交

- **WHEN** 对 worktree 内无未提交改动的 change 发起归档链（如用户已手动提交）
- **THEN** 提交段零动作（不产生空提交），链直达合入段

#### Scenario: 零提交分支脏工作区照常提交合入

- **WHEN** 桌面执行相位全程未提交（分支零提交、tip = 创建基线）且主仓 HEAD 已前进，worktree 内编辑 / staged / change 目录在场，发起归档链
- **THEN** 提交段照常产生提交（不因基线可达 HEAD 而跳过）、合入段执行合入（merge commit 落主仓），链续走同步与双写收口；写面 merge-first 守卫零触发

#### Scenario: 已合入跳过合入直达收口

- **WHEN** 分支已 merge 回主仓（主仓 active 树已含 `changes/<n>/`）且 worktree 干净后发起归档链
- **THEN** 提交段零动作（干净跳过）、合入段零动作（已合入跳过），链径直到同步与双写收口

#### Scenario: 阶段失败停等可重试

- **WHEN** 合入段冲突经解冲突 agent 无法裁决停链（或用户手动处置）后，再次发起归档链
- **THEN** 已成功的提交段不重复执行（干净跳过），合入段按已合入判定续走（手动 merge 形态幂等吸收），链自最近未完成阶段续走至收口

### Requirement: worktree 提交与主仓合入

带 worktree 记录的 change，归档链 SHALL 先在 worktree 内提交本 change 编辑集（git add 范围 = worktree 全域——worktree 为 change 私有执行锚，全域即本 change 编辑集；提交信息派生规则由 design 定稿）：worktree 脏即提交，干净则跳过；worktree 目录缺失且分支已合入（前置校验已拦未合入形态）则跳过。SHALL 随后在主仓将 branch `change/<name>` 合入主仓当前分支（HEAD 所在分支），分支已合入则幂等跳过。合入冲突时 SHALL 保留冲突态（MUST NOT 自动 `merge --abort`）：冲突判据为 merge 非零退出且冲突标记在场（`--diff-filter=U` 清单非空），命中时 SHALL 唤起一个解冲突 agent 会话（cwd = 主 workspace root、bypassPermissions、经既有 WorkerAgent 通道）——agent SHALL 仅编辑冲突清单内文件并以 pathspec 纪律收口（只 add 冲突清单，禁止 `add -A`）。链 SHALL 对 agent 结果后验：残留冲突标记、或冲突清单之外冒出新的工作区 / staged 改动，即判无法裁决。无法裁决（agent 失败 / 被停止 / 后验不通过）SHALL 以 lean 档收口：`merge --abort` 恢复主仓干净态 + 链停合入段显式咨询（冲突摘要 + 手动裁决引导；用户自行 merge 后重发归档，已合入判定幂等续走）；rich 档（待裁决态 + 交互咨询面）留后续变更。非冲突失败（merge 非零退出且无冲突标记——主仓 git 状态不允许等）SHALL 显式 `Err`（携带 git 语境与手动处置引导）。全程 MUST NOT 强制推送、MUST NOT 改写主仓历史；主仓无关未提交改动与无关 staged 条目 MUST NOT 被归档链任何自动提交或解冲突 agent 吞并。legacy 记录（`worktree=None`）SHALL 跳过提交与合入两段（主仓工作区编辑的提交仍为用户手动面，语义见 desktop-change-worktree「V1 范围与边界留痕」）。

#### Scenario: 提交合入成功

- **WHEN** 对 dirty worktree 的 change 走归档链至合入段成功
- **THEN** worktree 产生一个包含本 change 编辑集的提交（零 spec 同步产物——同步段在合入之后），主仓当前分支 merge 后 `openspec/changes/<n>/` 目录在场且 HEAD 前移

#### Scenario: 冲突 agent 解冲突续链

- **WHEN** 主仓与分支对同一文件有不兼容修改时合入段执行
- **THEN** 冲突态保留（零 abort），解冲突 agent 会话发起（cwd = 主仓、provenance 在案）；agent 仅编辑冲突清单内文件并以清单 pathspec 收口，后验通过后链续走同步与收口；主仓无关改动保持归档前状态

#### Scenario: 无法裁决 lean 停链咨询

- **WHEN** 解冲突 agent 失败 / 被停止 / 后验发现残留冲突或冲突清单外新改动
- **THEN** 链执行 `merge --abort` 收口（主仓无半截 merge 态），停合入段呈现冲突摘要与手动裁决引导；db 零变化；用户手动 merge 后重发归档，合入段已合入跳过、链续走至收口

#### Scenario: 非冲突失败显式引导

- **WHEN** merge 非零退出且无冲突标记（主仓 git 状态不允许等）
- **THEN** 显式 `Err` 呈现 git 冲突语境与手动处置引导，零解冲突 agent 会话，db 零变化，链可重试

#### Scenario: 主仓无关改动不吞并

- **WHEN** 主仓预先存在无关未提交文件与无关 staged 条目，归档链全程收口（含解冲突 agent 路径）
- **THEN** 两者保持归档前的 git 状态（未被吞并进任何自动提交或 agent 编辑），可机械验证

#### Scenario: legacy 跳过 git 段

- **WHEN** 对 `worktree=None` 的存量建档 change 发起归档链
- **THEN** 零提交零合入动作，链径直经（可选 agent 同步后）双写收口

### Requirement: spec 同步 agent 与归档链语义自持

delta specs 在场且用户未选择跳过时，归档链 SHALL 在主仓合入段之后发起一个 spec 同步 agent 会话：会话 cwd = 主 workspace root（worktree change 与 legacy 同锚——worktree change 的 delta specs 已随合入进入主仓 active 树，legacy 本就在主仓），同步编辑直接落在主仓主基线。prompt SHALL 逐 capability 将 delta spec 增量合并进 `openspec/specs/<capability>/spec.md` 主基线：`ADDED` 追加（requirement 已存在则更新为与 delta 一致）/ `MODIFIED` 增量应用（新增 scenario、修改列中 scenario、修订描述，不复制既有 scenario）/ `REMOVED` 整块移除 / `RENAMED` 改名，保留 delta 未提及的主 spec 内容，合并幂等（对已同步基线重跑零变化）；capability 主 spec 缺席时创建（简 Purpose + ADDED requirements）。以上增量合并语义 SHALL 以本 requirement 为单源定义（归档链为唯一定义源），spec 与 prompt MUST NOT 引用 CLI skill 文档作为语义或护栏溯源；prompt MUST NOT 依赖 MCP 工具、`__TOOL_ASK_USER__` 或 workflow.json，change 名与路径上下文 SHALL 由桌面插值。会话 SHALL 走既有 WorkerAgent 通道（compose_turn 新会话、CLI 引擎、bypassPermissions、provenance `source="change"` 且 source_ref 携 change 名与归档语义段——定式由 design 定稿）、转录可观测（实时 + 重放一致）且可停止（StopRegistry 既有终止面）；路径沙箱前缀校验以主仓 root 为锚，同步编辑囚于主仓内。agent 失败或被停止时链 SHALL 停在同步段：零归档变更（db 零变化、归档树零改名；已完成的提交与合入不回滚），可重试（同步幂等，重试自同步段续走）。

#### Scenario: 同步基线为主仓当前 specs

- **WHEN** 主仓 `openspec/specs/<capability>/spec.md` 已被先行的其他 change 归档更新（内容超出本 change worktree 创建基线）
- **THEN** 同步以主仓当前内容为合并基线（零冻结基线合并产物、零 git 层 spec 反向冲突）

#### Scenario: 同步产物由落盘段提交

- **WHEN** 带 delta specs 的 worktree change 走归档链至收口
- **THEN** merge commit 只含 change 内容（零同步产物）；同步产物落主仓工作区，经落盘段扩围 pathspec 提交（见「双写收口与结果摘要」）

#### Scenario: 无 delta specs 零 agent 会话

- **WHEN** 对无 `specs/` 子树的 change 发起归档链
- **THEN** 全程零归档 agent 会话记录（provenance 反查为空），链直落 git 段与双写收口；结果摘要 specs 行呈现「无 delta specs」语义

#### Scenario: 增量合并保真与幂等

- **WHEN** agent 对含 `## MODIFIED Requirements` 的 delta spec 执行同步，随后同一链重试再同步一次
- **THEN** 主 spec 仅按增量语义变化（delta 未提及的 requirement 与 scenario 原样保留），两次同步结果一致（幂等）

#### Scenario: agent 失败停链可重试

- **WHEN** 同步 agent 会话以失败收场（或运行中被用户停止）
- **THEN** 链停在同步段：db 与归档树零变化（已落主仓的提交与合入保留不回滚）；错误呈现后重试自同步段续走（合入段已合入跳过、同步幂等）成功至收口

### Requirement: 双写收口与结果摘要

归档链 SHALL 经写面 `archive` 单点收口（desktop-change-state-store「归档双写」零改动复用：主仓 active→archive 目录改名带 `YYYY-MM-DD-<name>` 前缀 + db status 翻转与 archived_at；改名成功翻转失败的重试走续半边）。收口后 UI SHALL 呈现结果摘要：change 名、归档位置（archive 树目录名）、specs 同步与否（已同步 / 无 delta specs / 跳过）、过程警告清单。收口后归档链 SHALL 以 pathspec 圈定自动提交主仓落盘产物：归档改名两路径（active 残迹 + archive 新目录，glob 形态覆盖已删除路径）**与 spec 同步产物路径（本 change delta specs 涉及的 `openspec/specs/<capability>` 子树——全树通配为禁区，无关 specs 的用户未提交编辑 MUST NOT 被吞并）**；脏探测为零则跳过（幂等面）。归档完成后详情页 SHALL 回到已归档形态（入口不再呈现）。

#### Scenario: 收口摘要呈现

- **WHEN** 归档链双写收口成功
- **THEN** 详情页呈现结果摘要（名 / 归档目录 / specs 同步状态 / 警告），db status=archived、主仓目录已带日期前缀改名，归档按钮不再呈现

#### Scenario: 落盘提交扩围含同步产物

- **WHEN** 带 delta specs 的 change 归档链收口后落盘段执行（主仓 specs 路径脏探测为真）
- **THEN** 归档改名与主 spec 更新同在落盘提交内（pathspec = 归档两路径 + delta capability 主 specs 路径）；用户对无关 specs 文件的未提交编辑与主仓无关改动原样保留（未被吞并）

#### Scenario: 半完成重试仅补翻转

- **WHEN** 收口段目录改名成功而 db 翻转失败后重试归档链
- **THEN** 前置与 git 段按幂等跳过，收口经续半边仅补 db 翻转（不重复改名），摘要照常呈现

### Requirement: run 面隔离与互斥

归档链 SHALL 独立于相位 run 状态机：全程零 `RunUpdate` / `ConfirmWait` / ask 事件，walker、相位机与运行控制命令面零改动；run 收口「Ready for archiving」停等语义不变，MUST NOT 自动触发归档（用户点击是归档唯一发起源）。归档链进行中 SHALL 拒绝该 change 的 run 发起（互斥登记载体由 design 定稿）；反向（运行中 run 拒绝归档发起）见「归档入口与前置确认面」。

#### Scenario: 归档链零 run 事件

- **WHEN** 归档链全程执行（含 agent 同步段）
- **THEN** run 状态通道零新事件，流程图与运行控制面板不因归档链变化；run 收口文案与停等语义逐字不变

#### Scenario: 归档中 run 发起被拒

- **WHEN** 归档链进行中对该 change 调运行控制命令面发起 run
- **THEN** 显式拒绝（呈现归档进行中原因），不进入运行态、零落账

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **worktree / branch 清理仍手动**：归档链成功后不执行 `git worktree remove` / `git branch -d`（清理为用户手动边界，desktop-change-worktree 既有裁定维持）；桌面 cleanup 按钮留后续变更。
2. **legacy 提交面手动**：`worktree=None` change 的主仓编辑提交仍由用户手动完成，归档链不代劳。
3. **主 spec 同步仅归档时**：specs 基线同步只发生在归档链内；run 相位期间不做主基线同步（proposal 相位只写 change 目录内 delta）。
4. **归档链不处理主仓无关改动**：主仓无关未提交 / staged 内容原样保留，处置归用户。
5. **无 delta specs 不触碰主 spec**：缺席 delta specs 时主基线零变更（摘要注明）。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 五条边界均可考，后续变更无需重新论证是否知情

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.25` 升级为 `0.4.26`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44`；CLI skill 的下线处置另行变更承载，本变更仅在 spec / prompt 面摘除对其的语义依赖）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop` version 为 0.4.26 且 `src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` version 仍为 2.10.44，SKILL.md 原文未动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/archive_flow.rs` | 归档编排链运行时 | 阶段序固定（校验 → worktree 提交 → 主仓合入（含冲突 agent 分支与后验）→ spec 同步 → 双写收口 → 落盘提交）；提交段跳过依据仅干净探测（可达性不构成跳过分支）；每阶段幂等可重入；失败停该阶段显式呈现；复用 WorkerAgentPort（spec 同步 + 解冲突两会话面）；零 run 事件面；依赖方向 orchestration → workflow 不变 |
| `crates/core/orchestration/src/port.rs`（ArchiveVcsPort） | 归档链 → vcs 执行的进程内缝 | 消费者 = 归档链；spawn 不进 core，进程执行驻 infra/vcs；方法族含冲突语义演进（merge 不-abort + 冲突清单读取 + abort 收口面——签名形态由 design 定稿）；`Err` 面带引导文案 |
| `crates/infra/vcs/src/git.rs` | git 工作面进程执行 | `add`（worktree 全域）/ `commit`（pathspec 形态，不吞无关 staged）/ `merge`（冲突态保留 + 冲突清单读取）/ `merge --abort` / 干净探测子命令族；真实 git tempdir 夹具可测 |
| `crates/core/orchestration/src/prompt.rs`（或归档链模块，design 定稿） | 归档 agent prompt 模板 | spec 同步 prompt（增量合并语义自持——单源为本 spec「spec 同步 agent 与归档链语义自持」，零 skill 溯源）与解冲突 prompt（冲突清单 + 只编辑清单内文件 + pathspec 收口纪律）；变量插值（change 名 / 路径 / 清单）；零 MCP / workflow.json 依赖 |
| `crates/infra/agent/src/worker.rs`（复用） | WorkerAgentPort 实现 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"` + 归档语义 source_ref（定式 design 定稿） |
| `crates/core/workflow/src/write/archive.rs`（复用零改动） | 双写单点收口 | 改名 + 翻转 + 续半边恢复既有语义；归档链末段唯一收口触点 |
| `src/commands/archive_flow/` | 薄命令包装 | 发起 / 状态 / 停止面；三件事纪律 `Result<T, String>`；互斥校验；冲突摘要呈现载体由 design 定稿；bindings 再生 |
| `packages/desktop/src/views/changes/`（详情页） | 归档入口与链面呈现 | 归档按钮两态可见性；确认对话（警告清单 + worktree 合入告知）；阶段状态（新序 + 冲突子阶段与 lean 咨询面）与结果摘要；agent 会话转录入口（复用 AgentTimeline 基建）；data-testid 挂钩 |
| `src/types/generated/bindings.ts` | IPC 类型跟随 | 新命令与 DTO 再生；`bindings:check` 守卫拦截过期生成物 |
