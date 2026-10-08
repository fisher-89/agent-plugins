# desktop-change-archive Specification

## ADDED Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 提供归档入口（按钮）：db `ChangeRecord.status=active` 的 change 可见可用；已归档（`source=archive` / status=archived）SHALL NOT 呈现入口；无 db 建档的文档形态 change（存量 CLI change）SHALL NOT 提供归档（写面既有拒绝面沿袭）。点击 SHALL 先呈现确认对话再发起，对话 SHALL 呈现：完成度核对结论（以 db PhaseRecord eval 序列确定性核算——workflow 相位表全部相位存在 non-stale 的 pass / skipped 条目即完成，MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支」的告知。警告 SHALL NOT 阻断（沿 `openspec-archive-change` skill 护栏语义：inform + confirm），用户显式确认后照常发起；无警告时确认为轻量确认。该 change 存在运行中 run（运行注册表复合键在案）时发起归档 SHALL 显式拒绝。

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

归档 SHALL 为确定性编排链（独立于相位 run 状态机），阶段序为：前置校验（建档在案 + status=active + 无运行中 run）→（delta specs 在场）spec 同步 agent →（记录带 worktree）worktree 提交 → 主仓合入 → 写面 `archive` 双写收口。每阶段 SHALL 幂等可重入：worktree 干净则跳过提交；分支已合入（主仓 active / archive 树已命中该 change 目录）则跳过合入；双写半完成重试走既有续半边（仅补 db 翻转）。delta specs 缺席时 SHALL 跳过 agent 阶段（skill「无 delta specs 直接归档」语义对译）。任一阶段失败 SHALL 显式停在该阶段（错误呈现，MUST NOT 静默跳过或降级），重试自最近未完成阶段续走；已成功阶段不重复执行。

#### Scenario: 干净 worktree 跳过提交

- **WHEN** 对 worktree 内无未提交改动的 change 发起归档链（如用户已手动提交）
- **THEN** 提交段零动作（不产生空提交），链直达合入段

#### Scenario: 已合入跳过合入直达收口

- **WHEN** 分支已 merge 回主仓（主仓 active 树已含 `changes/<n>/`）后发起归档链
- **THEN** 合入段零动作，链径直到双写收口

#### Scenario: 阶段失败停等可重试

- **WHEN** 合入段因冲突失败后用户手动解冲突，再次发起归档链
- **THEN** 前置段（含 agent 同步，幂等）按需续走，已成功的提交段不重复执行，链自合入段续走至收口

### Requirement: worktree 提交与主仓合入

带 worktree 记录的 change，归档链 SHALL 在 worktree 内提交本 change 编辑集（git add 范围 = worktree 全域——worktree 为 change 私有执行锚，全域即本 change 编辑集与 spec 同步产物；提交信息派生规则由 design 定稿），worktree 干净时跳过。SHALL 在主仓将 branch `change/<name>` 合入主仓当前分支（HEAD 所在分支）；合入冲突或主仓 git 状态不允许时 SHALL 显式 `Err`（携带 git 语境与手动处置引导），MUST NOT 强制推送、MUST NOT 改写主仓历史、MUST NOT 自动解冲突。主仓无关未提交改动与无关 staged 条目 MUST NOT 被归档链的任何自动提交吞并（主仓侧提交仅可以已知归档路径 pathspec 圈定，且是否自动提交由 design 定稿）。legacy 记录（`worktree=None`）SHALL 跳过提交与合入两段（主仓工作区编辑的提交仍为用户手动面，语义见 desktop-change-worktree「V1 范围与边界留痕」）。

#### Scenario: 提交合入成功

- **WHEN** 对 dirty worktree 的 change 走归档链至合入段成功
- **THEN** worktree 产生一个包含编辑集与 spec 同步产物的提交，主仓当前分支 merge 后 `openspec/changes/<n>/` 目录在场且 HEAD 前移

#### Scenario: 冲突显式失败零破坏

- **WHEN** 主仓与分支对同一文件有不兼容修改时合入段执行
- **THEN** 显式 `Err` 呈现 git 冲突语境与手动处置引导；主仓未落半截 merge 状态（或引导用户 `git merge --abort`），db 零变化，链可重试

#### Scenario: 主仓无关改动不吞并

- **WHEN** 主仓预先存在无关未提交文件与无关 staged 条目，归档链全程收口
- **THEN** 两者保持归档前的 git 状态（未被吞并进任何自动提交），可机械验证

#### Scenario: legacy 跳过 git 段

- **WHEN** 对 `worktree=None` 的存量建档 change 发起归档链
- **THEN** 零提交零合入动作，链径直经（可选 agent 同步后）双写收口

### Requirement: spec 同步 agent 与 skill prompt 桌面化

归档链 SHALL 在 `openspec/changes/<name>/specs/` 存在 delta specs 时发起一个 spec 同步 agent 会话：prompt 语义 SHALL 取自 `plugins/dev-team/skills/openspec-archive-change/SKILL.md` 步骤 2——逐 capability 将 delta spec 增量合并进 `openspec/specs/<capability>/spec.md` 主基线（`ADDED` 追加 / `MODIFIED` 增量应用 / `REMOVED` 整块移除 / `RENAMED` 改名），保留 delta 未提及的主 spec 内容，合并幂等；capability 主 spec 缺席时创建（简 Purpose + ADDED requirements）。change 名与路径上下文 SHALL 由桌面插值；skill 的 change 选择、`__TOOL_ASK_USER__` 确认与 MCP `change_list` 完成度核对步骤 SHALL 由桌面确认面与 db 核算替代——prompt MUST NOT 依赖 MCP 工具、`__TOOL_ASK_USER__` 或 workflow.json。会话 SHALL 走既有 WorkerAgent 通道（compose_turn 新会话、CLI 引擎、bypassPermissions、provenance `source="change"` 且 source_ref 携 change 名与归档语义段——定式由 design 定稿）、转录可观测（实时 + 重放一致）且可停止（StopRegistry 既有终止面）。会话 cwd = 同步对象所在 root（worktree change → worktree 绝对路径；legacy → 主 workspace root），路径沙箱前缀校验将同步编辑囚于该 root 内。worktree change 上同步 SHALL 先于提交与合入（同步产物随本 change 提交合入主仓）。agent 失败或被停止时链 SHALL 停在同步段（零提交、零合入、零归档变更），可重试。

#### Scenario: 同步产物随提交合入

- **WHEN** 带 delta specs 的 worktree change 走归档链，agent 完成同步后续段执行
- **THEN** 主 spec 更新与编辑集同在工作树提交内，merge 后主仓 specs 基线与 change 产物一致

#### Scenario: 无 delta specs 零 agent 会话

- **WHEN** 对无 `specs/` 子树的 change 发起归档链
- **THEN** 全程零归档 agent 会话记录（provenance 反查为空），链直落 git 段与双写收口；结果摘要 specs 行呈现「无 delta specs」语义

#### Scenario: 增量合并保真与幂等

- **WHEN** agent 对含 `## MODIFIED Requirements` 的 delta spec 执行同步，随后同一链重试再同步一次
- **THEN** 主 spec 仅按增量语义变化（delta 未提及的 requirement 与 scenario 原样保留），两次同步结果一致（幂等）

#### Scenario: agent 失败停链可重试

- **WHEN** 同步 agent 会话以失败收场（或运行中被用户停止）
- **THEN** 链停在同步段：worktree 零提交、主仓零合入、db 与归档树零变化；错误呈现后重试自同步段续走成功

### Requirement: 双写收口与结果摘要

归档链末段 SHALL 经写面 `archive` 单点收口（desktop-change-state-store「归档双写」零改动复用：主仓 active→archive 目录改名带 `YYYY-MM-DD-<name>` 前缀 + db status 翻转与 archived_at；改名成功翻转失败的重试走续半边）。收口后 UI SHALL 呈现结果摘要：change 名、归档位置（archive 树目录名）、specs 同步与否（已同步 / 无 delta specs / 跳过）、过程警告清单——`openspec-archive-change` skill「Output On Success」面对译。收口后主仓归档改名的落盘处置（自动以已知归档路径 pathspec 提交 vs 留给用户手动提交）由 design 定稿；无论何者 MUST NOT 吞并主仓无关改动。归档完成后详情页 SHALL 回到已归档形态（入口不再呈现）。

#### Scenario: 收口摘要呈现

- **WHEN** 归档链双写收口成功
- **THEN** 详情页呈现结果摘要（名 / 归档目录 / specs 同步状态 / 警告），db status=archived、主仓目录已带日期前缀改名，归档按钮不再呈现

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

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.20` 升级为 `0.4.21`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`：版本保持 `2.10.44`，`openspec-archive-change/SKILL.md` 为只读参考源（prompt 语义单源引用，不改写插件）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop` version 为 0.4.21 且 `src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` version 仍为 2.10.44，SKILL.md 原文未动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/archive_flow.rs`（新） | 归档编排链运行时 | 阶段序固定（校验 → agent 同步 → worktree 提交 → 主仓合入 → 双写收口）；每阶段幂等可重入；失败停该阶段显式呈现；复用 WorkerAgentPort；零 run 事件面；依赖方向 orchestration → workflow 不变 |
| `crates/core/workflow/src/write/worktree.rs`（扩展，或新 port——design 定稿） | 提交 / 合入 / pathspec 提交 port 缝 | port 属消费者、spawn 不进 core；sync 签名纪律沿 WorktreePort 先例；`Err` 面带引导文案 |
| `crates/infra/vcs/src/git.rs`（扩展） | git 工作面进程执行 | `add`（worktree 全域）/ `commit`（pathspec 形态，不吞无关 staged）/ `merge` / 干净探测子命令族；真实 git tempdir 夹具可测 |
| `crates/core/orchestration/src/prompt.rs`（或归档链模块，design 定稿） | 归档 agent prompt 模板 | 语义单源 = SKILL.md 步骤 2（增量合并 + 幂等 + 保留未提及内容）；变量插值（change 名 / 路径）；零 MCP / workflow.json 依赖 |
| `crates/infra/agent/src/worker.rs`（复用） | WorkerAgentPort 实现 | compose_turn 新会话 + StopRegistry 终止 + 密封转录；provenance `source="change"` + 归档语义 source_ref（定式 design 定稿） |
| `crates/core/workflow/src/write/archive.rs`（复用零改动） | 双写单点收口 | 改名 + 翻转 + 续半边恢复既有语义；归档链末段唯一收口触点 |
| `src/commands/`（归档链 IPC 面，形态 design 定稿） | 薄命令包装 | 发起 / 状态 / 停止面；三件事纪律 `Result<T, String>`；互斥校验；bindings 再生 |
| `packages/desktop/src/views/changes/`（详情页） | 归档入口与确认面 | 归档按钮两态可见性；确认对话（警告清单 + worktree 合入告知）；阶段状态与结果摘要；agent 会话转录入口（复用 AgentTimeline 基建）；data-testid 挂钩 |
| `src/types/generated/bindings.ts` | IPC 类型跟随 | 新命令与 DTO 再生；`bindings:check` 守卫拦截过期生成物 |
