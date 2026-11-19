# desktop-change-worktree Specification

## Purpose

定义 change worktree 双 root 执行能力：写面 create 以主仓 HEAD 为基线为每次创建分配 per-change git worktree（workspace git 根同级 `.worktrees/<仓库目录名>/<name>` + branch `change/<name>`），change run 以 workspace root 为唯一 store 身份锚、worktree 为执行根的双 root 组合执行；配套确定性依赖 bootstrap、脏仓警告不阻止、merge-first 归档引导与 worktree 路径 UI 可见性。

## Requirements

### Requirement: worktree 落位单点（git 根同级）

change worktree SHALL 落位于 **workspace git 根的同级**：`<git 根父目录>/.worktrees/<git 根目录名>/<change-name>`。git 根 SHALL 经 `git rev-parse --show-toplevel` 解析——workspace root 可为仓库子目录（workspace 在 `client/app` 时 worktree 仍落仓库根同级 `.worktrees/<仓库目录名>/`），落位恒锚定 git 根，MUST NOT 以 workspace root 自身为锚。落位派生 SHALL 收口 vcs 边界单点（`vcs-runtime::worktree_root`），并经 `WorktreePort::worktree_root` 缝在写面前置校验时序内消费。worktree 落位与 workspace db 文件名的身份段派生（foundation 单点）SHALL 解耦：worktree 路径 MUST NOT 叠加数据根 / 身份段 hash 段（避免 worktree 路径逼近 Windows MAX_PATH），git 树 MUST NOT 感知 db 文件。worktree 落位与数据目录 SHALL 零耦合（desktop-data-dimensions 零改动）；跨盘（data 目录与 repo 异盘）的 git worktree 合法，SHALL 接受。存量 change 的 worktree 路径以 `ChangeRecord.worktree` 落库为唯一执行锚，本落位变更 MUST NOT 迁移或重算存量地址。

#### Scenario: git 根同级可复现

- **WHEN** 对同一 workspace root 跨进程重启两次派生 worktree 落位
- **THEN** 两次派生结果逐字一致，均为 `<git 根父目录>/.worktrees/<git 根目录名>/<change>`

#### Scenario: workspace 为仓库子目录仍锚定 git 根

- **WHEN** workspace root 为 git 仓库的子目录（如 `<repo>/client/app`）
- **THEN** worktree 落位于 `<repo 父目录>/.worktrees/<repo 目录名>/<change>`，MUST NOT 落在子目录内或其同级

#### Scenario: 异仓库互不冲突

- **WHEN** 两个不同 git 仓库各自创建同名 change 的 worktree
- **THEN** 两个 worktree 落位于各自的 `.worktrees/<仓库目录名>/` 子树下，互不冲突、互不可见

#### Scenario: 子树零耦合

- **WHEN** 审查 worktree 落位与 workspace 库的消费关系
- **THEN** db 路径派生只消费 `workspaces/` 子树（foundation 身份段单点），worktree 落位只消费 git 根，git 工作树内无 db 文件感知

#### Scenario: 存量地址不迁移

- **WHEN** 升级后打开一个存量 change（记录携旧数据根落位的 worktree 路径）
- **THEN** 执行 / 归档 / 清理均以 `ChangeRecord.worktree` 为准，磁盘目录不移动、不重算

### Requirement: create 分配 worktree 与分支基线

写面 `create` 对每次成功创建 SHALL 分配同名 worktree：以主仓 HEAD 为基线 `git worktree add` 落位于 git 根同级 `.worktrees/<仓库目录名>/`，并建 branch `change/<name>`——主仓未提交与 staged 改动 MUST NOT 进入 worktree（基线恒 HEAD）。`openspec/changes/<name>/` 目录树与 `explore.md` SHALL 建于 worktree 内（主仓 active 树在 merge 前 MUST NOT 含该目录）。建档 SHALL 记录 worktree 绝对路径与创建基线（`ChangeRecord.worktree` / `base_commit`，字段语义见 desktop-change-state-store）。前置校验 SHALL 在任何写 IO 之前覆盖：git 可用性（git 不可发现或主仓非 git 仓 → 显式 `Err` 引导，MUST NOT 静默回退主 root 创建）、branch `change/<name>` 已存在、worktree 目标目录已存在——拒绝面零 worktree、零建档、零目录。进程 spawn（git / bootstrap）SHALL 经 core/workflow 定义、vcs 边界（`crates/infra/vcs`，裸名 `vcs-runtime`）实现的 port 缝承载，core/workflow 零 infra 依赖、零进程 spawn 红线不变。

#### Scenario: 成功建域四件套

- **WHEN** 对干净 git 仓 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** worktree 目录存在于 `<git 根父目录>/.worktrees/<git 根目录名>/<name>`、branch `change/<name>` 在案且指向主仓 HEAD、worktree 内 `openspec/changes/<name>/explore.md` 为 goal 原文、db `ChangeRecord` 携 worktree 路径与 base_commit；主仓 active 树无该目录

#### Scenario: git 不可用拒绝

- **WHEN** git 不可发现或主仓不是 git 仓库时调用写面 `create`
- **THEN** 返回显式 `Err`（含引导文案），不产生任何 worktree / branch / 目录 / db 记录，MUST NOT 静默回退主 root 创建

#### Scenario: branch 与目录冲突拒绝

- **WHEN** branch `change/<name>` 已存在或 worktree 目标目录已存在时调用写面 `create`
- **THEN** 返回显式 `Err`（含冲突对象），零 worktree、零建档、零目录改动

#### Scenario: 基线恒 HEAD

- **WHEN** 主仓存在未提交改动（工作区或 staged）时创建 worktree 成功
- **THEN** worktree 内容为主仓 HEAD 检出，不含任何未提交改动；base_commit 记录该 HEAD

### Requirement: 双 root 执行语义与 db 身份不变量

change run SHALL 以双 root 组合执行：

- **workspace root**（用户注册的 canonical root）是唯一 store 身份锚：`for_root` 派生 workspace 库、provider / 会话 / change 建档 / 相位状态全部钉在其上；worktree 路径 MUST NOT 进入 `for_root`（直接喂入会铸出空库）。
- **exec root**（db 记录带 worktree → 该 worktree 绝对路径；legacy 记录 `worktree=None` → 主 workspace root）承担：会话 cwd（经既有 `turn.root → SessionCtx.workspace_root` 通道，CLI `current_dir` 与 SDK `LoopTurn.cwd` 同锚）、工件根（`Layout::resolve`）、git diff 与检查器（static-check / test-execution）执行目录。

`compose_turn` 的 store 半边 SHALL 由命令层预解析的 workspace root 库实例注入；相位状态机零 fs 保持不动；会话 resume 的 cwd 由当次 turn 的 root 重推导（`SessionRecord` 不存 cwd），resume 链零改动。SDK 路径沙箱前缀校验以会话 cwd（即 exec root）为锚（既有机制零改动），worktree change 的文件编辑 SHALL 因此囚于 worktree 内；worktree 内 `git diff HEAD` SHALL 恰为本 change 编辑集（不含主仓无关改动）。会话转录 / 三槽位 / StepRecord SHALL 照旧落 workspace root 库——db 不受 worktree 存在或删除影响是本能力核心不变量。

#### Scenario: worktree run 的 db 身份不变

- **WHEN** worktree change 的 run 走完一相位（executor / evaluator 会话 + phase_log 落账）
- **THEN** 会话记录、槽位与 StepRecord 落在 workspace root 对应的 workspace 库（provider 可解析、与主 root 会话同库可查），compose 全程未以 worktree 路径进 `for_root`

#### Scenario: 编辑囚于 worktree

- **WHEN** worktree change 的 executor 会话（SDK 引擎）尝试编辑 worktree 目录外文件
- **THEN** 路径沙箱前缀校验拒绝（既有机制，锚 = exec root），编辑不落主仓

#### Scenario: diff 面恰为本 change 编辑集

- **WHEN** 主仓存在无关未提交改动、worktree 内 executor 已产生本 change 编辑后组装 executor prompt
- **THEN** git diff 上下文取自 worktree，仅含本 change 编辑集，不含主仓无关改动

#### Scenario: legacy 主 root 执行

- **WHEN** 对 `worktree=None` 的存量建档 change 发起 run
- **THEN** exec root 解析为主 workspace root，既有全链语义（cwd / Layout / diff / 检查器 / 归档）零变化

### Requirement: 确定性依赖 bootstrap

写面 `create` SHALL 在 worktree 就绪后执行确定性依赖引导，按 workspace 配置的 `projects` 分两种模式：

- **未配置项目根**（`projects` 缺失 / 空数组）：探测 **worktree 根**的 lockfile（已知管理器映射：pnpm-lock.yaml / package-lock.json / yarn.lock / Cargo.lock 等，映射表由 design 定稿）→ 首匹配命中则在 worktree 根 spawn 对应安装命令（经 vcs 边界 port）；未知管理器或无 lockfile → 跳过并在警告面注记。
- **配置了项目根**（`projects` 非空）：按 **`depends_on` 的拓扑序**逐个解析 `<worktree>/<root>`（`root` 为相对 worktree 根的项目目录，`.` 等价 worktree 根）——依赖先于依赖方安装，无依赖或同层者保持**配置声明序**（声明序是唯一 tie-break，MUST NOT 改按字典序）；每个项目独立探测同一张映射表，首匹配命中即在该项目目录 spawn 安装（cwd = 项目目录）；每个项目至多一次安装，多个项目可各自安装。**脏依赖数据 MUST NOT 阻断安装**（校验面不做引用校验，消费面天然免疫）：`depends_on` 指向未声明 root 的悬空项直接跳过、自引用与成环经 `visited` 集合收口、同名 `root` 重复项折叠为一次安装（以首次出现为准）——四种情形一律零 `Err`、零新增诊断，产出确定安装序。项目目录不存在 / 无已知 lockfile / 安装失败均在该项目的警告注记（文案含项目根）并继续后续项目；MUST NOT 回退扫描 worktree 根。
- 配置文件 SHALL 从**主 workspace root** 读取（create 时刻的当前配置，含未提交改动）；worktree 内配置副本仍停在 base commit。项目根 SHALL 承载扫描 / 安装锚点、编码语言标注与项目间依赖声明三项语义；**这三项之外 MUST NOT 承载语义**（MUST NOT 沦为通用元数据口袋；字段面与软规则边界见 desktop-workspace-config「项目协议语义边界与消费点纪律」）。

bootstrap 失败 SHALL NOT 回滚 create（建档与 worktree 保留），失败信息 SHALL 立即呈现于 create 产出警告清单（static-check / test-execution 为确定性步骤无 agent 救场，后续大声失败兜底）。

#### Scenario: lockfile 命中执行安装

- **WHEN** 未配置 `projects` 且 worktree 检出含 `pnpm-lock.yaml` 的仓库
- **THEN** create 在 worktree 根 spawn 对应安装命令（cwd = worktree 根），成功后无相关警告

#### Scenario: 未知管理器跳过注记

- **WHEN** 未配置 `projects` 且 worktree 根无任何已知管理器 lockfile
- **THEN** bootstrap 跳过 spawn，警告清单注记「未识别依赖管理器，跳过依赖引导」

#### Scenario: 失败不回滚

- **WHEN** 安装命令非零退出（任一模式）
- **THEN** create 以成功收口（建档与 worktree 保留），警告清单含失败摘要；后续 static-check / test-execution 步以自身失败面兜底

#### Scenario: 多项目按拓扑序安装

- **WHEN** `projects` 为 `[{root: "packages/desktop/src-tauri", depends_on: ["packages/desktop"]}, {root: "packages/desktop"}]`，前者含 `Cargo.lock`、后者含 `pnpm-lock.yaml`
- **THEN** create 执行两次安装，**顺序与声明序相反**：先 `<worktree>/packages/desktop`（`pnpm install --frozen-lockfile`）后 `<worktree>/packages/desktop/src-tauri`（`cargo fetch --locked`）；顺利时警告清单为空

#### Scenario: 无依赖保持声明序

- **WHEN** `projects` 声明三个项目（顺序 A、B、C）且均无 `depends_on`、均命中 lockfile
- **THEN** 安装序逐字等于声明序 A、B、C（退化情形：拓扑序 = 声明序，既有「按序安装」断言零改动）

#### Scenario: 悬空与自引用不阻断

- **WHEN** 某项目 `depends_on` 含未声明的 root（悬空），另一项目 `depends_on` 含自身 root（自引用）
- **THEN** 两项目照常安装、其余项目照常安装；create `Ok`、警告清单未因依赖数据新增条目、诊断面零条目（悬空项静默跳过）

#### Scenario: 成环依赖按声明序落定

- **WHEN** 两个项目互相 `depends_on`（A→B→A），均命中 lockfile
- **THEN** create `Ok`，两项目各安装恰一次，安装序为确定值（按声明序落定，零 `Err`、不死循环）

#### Scenario: 重复 root 折叠安装一次

- **WHEN** `projects` 含两条 `root` 相同的元素（后者另带不同 `depends_on`）
- **THEN** 该项目恰安装一次（以首次出现的元素为准），零诊断、零 `Err`

#### Scenario: 项目内首匹配单安装

- **WHEN** 某项目目录同时含 `pnpm-lock.yaml` 与 `package-lock.json`
- **THEN** 该项目恰执行一次安装（映射表首匹配 `pnpm install --frozen-lockfile`），后续项目继续扫描

#### Scenario: 配置根缺失与项目无 lockfile

- **WHEN** `projects` 含 worktree 内不存在的 root，或项目目录存在但无任何已知 lockfile
- **THEN** 该项目零安装，警告清单含「项目根 "<root>"：在 worktree 内不存在，跳过依赖引导」或「项目根 "<root>"：未识别依赖管理器，跳过依赖引导」；create 照常成功并继续后续项目

#### Scenario: 非空 projects 抑制根扫描

- **WHEN** `projects` 非空且 worktree 根另有 lockfile
- **THEN** worktree 根 lockfile 不参与扫描、零安装；仅按项目根执行

#### Scenario: 单项目失败不中断后续

- **WHEN** 第一个项目安装非零退出、第二个项目命中 lockfile
- **THEN** create 照常成功，第一个项目记失败警告、第二个项目仍执行安装，警告按**安装序**累积

### Requirement: 脏主仓警告不阻止

主仓 `git status --porcelain` 非空时写面 `create` SHALL 照常成功（警告但不阻止），基线恒 HEAD 不变；警告文案 SHALL 含「先提交再开新 change」引导（主仓未提交状态对新 change 不可见）。警告 SHALL 经 `CreateOutcome.warnings` 抵达前端（与 bootstrap 警告同载体），干净仓 SHALL 产出空警告清单。

#### Scenario: 脏仓创建成功且警告引导

- **WHEN** 主仓工作区存在未提交文件时创建 change
- **THEN** 创建成功、worktree 基线为 HEAD，`warnings` 含先提交引导文案；worktree 不含该未提交内容

#### Scenario: 干净仓零警告

- **WHEN** 主仓 `git status --porcelain` 为空时创建 change
- **THEN** 创建成功且 `warnings` 为空清单

### Requirement: merge-first 归档引导

带 worktree 记录的 change 归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该 change 目录时，直接归档 SHALL 显式拒绝并引导「先 merge worktree 分支（change/<name>）回主仓再归档」（MUST NOT 泛化「目录未找到」了事）；主仓出现 `changes/<n>/` 后，既有归档双写（db status 翻转 + 主仓目录改名）SHALL 零改动可用。

满足前置的两条等价路径：**archive 相位自动段**（desktop-change-archive：run 的 archive 相位在 Seal 双写收口前经 Commit / Rebase / ResolveConflicts / Merge 步自动执行 worktree 提交 + 主仓合入，为用户确认进入 archive 相位后的缺省路径）与**用户手动 merge**（既有路径维持，archive 相位的 step 内判断将其视为已完成段）。merge SHALL NOT 被强制：用户可不 merge 弃置 change（V1 弃置 = 手动清 worktree / branch，db 记录保留，见「V1 范围与边界留痕」）。自动合入的冲突处置（worktree 内 rebase 冲突态保留 + 解冲突 agent 裁决，agent 无法裁决时 lean 档停链咨询用户）与主仓状态约束（不吞并无关改动、不改写主仓历史）由 desktop-change-archive 约束。run 收口语义改写：全前序路由相位（一次性 initialize + 内容相位）pass 后停等 archive 前（「等待确认进入归档」），MUST NOT 自动归档、MUST NOT 自动 merge（archive 相位由用户确认进入，非 run 收口的自动动作）；worktree 清理由 archive 相位 Cleanup 步在双写收口前自动执行（尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」——V1 手动边界就此偿还），branch `change/<name>` 保留（删除仍为用户手动边界）。

#### Scenario: archive 相位自动提交合入满足前置

- **WHEN** 带 worktree 记录且主仓两树均未命中目录的 change 经用户确认进入 archive 相位，Commit / Rebase / ResolveConflicts / Merge 步成功（含零提交分支形态——Commit 步不因分支基线可达主仓 HEAD 而跳过）
- **THEN** 主仓出现 `changes/<n>/`，Seal 双写收口零特判走通（无 worktree 路径分支进入收口步）；用户全程无手动 git 操作

#### Scenario: 手动 merge 后 archive 相位跳过合入

- **WHEN** 用户已手动将 worktree 分支 merge 回主仓（主仓出现 `changes/<n>/`）后进入 archive 相位
- **THEN** Commit 步按干净探测跳过、Rebase / Merge 步幂等跳过（零 git 动作），链径直至 Cleanup / Seal / Finalize 收口；spec 同步在主仓当前基线执行（产物不再滞留 worktree）；直接经裸归档命令调用亦零改动可用（既有语义）

#### Scenario: 未 merge 的直接归档仍被引导拒绝

- **WHEN** 绕过 archive 相位对带 worktree 记录且主仓两树均未命中目录的 change 直接调用归档写面
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支；db 与磁盘零变化（既有引导语义不因 archive 相位在场而退役）

#### Scenario: run 收口不自动合入

- **WHEN** 带 worktree 记录的 change run 走完全部前序路由相位（含一次性 initialize）
- **THEN** run 停在 archive 前（零自动 merge、零自动归档、零 archive 相位触发），用户显式确认进入 archive 相位是唯一发起源

### Requirement: worktree 路径 UI 可见性

create 产出与 change 详情 SHALL 暴露 worktree 绝对路径（记录带 worktree 时），供用户 review / 手动 commit / merge 可达（worktree 藏于 home 数据目录下的可见性补偿）；legacy 记录 SHALL 出线 null（前端不呈现该信息面）。create 对话框 SHALL 行内呈现警告清单（脏仓 / bootstrap）。详情出线经 detail DTO 字段演进（wire contract 冻结约束下走 golden 显式重写流程）。

#### Scenario: create 成功呈现路径与警告

- **WHEN** create 成功返回（含或不含警告）
- **THEN** 对话框成功面呈现 worktree 绝对路径与警告清单（有则行内列出）

#### Scenario: 详情信息行可达

- **WHEN** 打开带 worktree 记录的 change 详情
- **THEN** 详情信息面呈现 worktree 路径；legacy change 详情该面为 null 且不渲染

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **branch 清理手动（删除面已偿还）**：worktree 由 archive 相位 Cleanup 步自动移除（两段式回收尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」与本 spec「worktree 回收两段式」）；归档路径下 branch `change/<name>` 仍保留（历史可溯）。需要彻底清理时走清单删除入口并勾选「同时清空 worktree」——该路径同时删除 worktree 目录与 branch（见 desktop-change-delete）。
2. **legacy 主 root 路径**：`worktree=None` 存量记录照旧主 root 执行，零迁移；非 git 仓 workspace 的 create 显式拒绝（不提供 legacy 创建逃生口，真实诉求另立变更）。
3. **`agent_start` 与探索笔记留主仓 root**：exec 轨道手动会话与 `openspec/explores/` 笔记不混入 change worktree。
4. **config / AGENT.md 冻结**：worktree 内配置停在创建时刻（base commit 检出），主仓中途改动不传导，文档化；唯一例外是 create 在创建时读取**主 workspace root** 的当前配置以选择 `projects[].root`（依赖引导锚点），worktree 内配置副本仍冻结。
5. **构建成本接受**：fresh worktree 依赖 / 构建产物全量重建；共享 target 目录类逃生口（`CARGO_TARGET_DIR` 等）仅文档化，不内建机制。
6. **孤儿 worktree 对账（两源）**：create 中途崩溃遗留的 worktree / branch、以及删除失败留下的已注销孤儿，SHALL 以**两源**对账——① `git worktree list`；② 直接检查 worktree 落位根 `<git 根父目录>/.worktrees/<仓库目录名>/` 下的子目录。已注销孤儿不在 git 登记面（`list` 不显示、`prune` 无输出、`remove` 报 `not a working tree`），单靠 git 对账无法定位，故面向用户的残留指引 MUST NOT 只给 git 一侧。有建档记录的残留经清单删除入口勾选「同时清空 worktree」清理（desktop-change-delete——删除面按记录定位，故只覆盖有建档记录的残留），且回收单点自带 OS 层兜底、一次失败不再等于终态（见「worktree 回收两段式」）；无记录的孤儿仍须手动删除其目录。create 前置校验保证下次创建显式报错而非误用；create 前置 worktree 磁盘保底门槛为失控防线（见 desktop-change-create）。
7. **report_dir 绝对路径接受失效**：phase_log 报告文本内嵌的 worktree 绝对路径在 worktree 删除后失效（转录历史，仅展示面），不做相对化；归档 Cleanup 步使删除成为缺省路径，本边界维持。
8. **并行 merge 冲突处置分层**：两 change 改同一文件各自绿、merge 撞时——经 archive 相位自动合入的冲突由 Rebase / ResolveConflicts 步的解冲突 agent 裁决（agent 无法裁决时 lean 档停链转人工，见 desktop-change-archive）；链外用户手动 merge 的冲突走 git 原生冲突处理。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

#### Scenario: 残留指引双源

- **WHEN** 审查三处面向用户的 worktree 残留指引（`remove_worktree` 的 `Err`、create 前置⑨ 门槛文案、create 补偿残留文案）
- **THEN** 三者均同时给出「`git worktree list` 对账」与「直接检查 `.worktrees/<仓库目录名>/` 下的子目录」两源，且均不含「执行 `git worktree remove --force <path>` 清理」单源指引

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.13` 升级为 `0.4.14`（用户可见新功能；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.14，`src-tauri/Cargo.toml` 版本不随动

### Requirement: worktree 回收两段式（git 注销失败即 OS 层兜底）

`crates/infra/vcs` 的 `remove_worktree(main_root, worktree)` SHALL 为 worktree 磁盘回收的唯一单点（create 树写出补偿、删除面「同时清空 worktree」、归档相位的 Cleanup 步三处消费；`WorktreePort` 与 `ArchiveVcsPort` 两缝 SHALL 委托同一实现、同一 `Err` 文案）。执行序 SHALL 为两段式：

1. `git worktree remove --force <path>` 成功 → `Ok`（既有语义零变化）。
2. 该调用失败 → 以 **OS 层删除**兜底（绕过 git 的登记语义）：目录已不在场 → `git worktree prune`（尽力）→ `Ok`；目录仍在场 → `fs::remove_dir_all`（Windows 经 `\\?\` 扩展前缀）→ 成功后 `git worktree prune`（清可能的残留 admin）→ `Ok`。
3. 兜底亦失败 → `Err`（见下「失败面」）。

该两段式的存在依据 SHALL 作为实现理由留痕：git 的删除语义是**先拆登记（admin 目录 + 登记表条目）、再删文件**（`--force` 与否一致），因此任何一次删除失败都留下「git 已注销」的孤儿目录——`git worktree list` 不显示、`prune` 无事可做、`worktree remove` 报 `not a working tree`。**不存在「失败即回滚、可安全重试」的路径**，故兜底是唯一的自愈手段。

约束：

- **幂等**：目录不在场 MUST 以 `Ok` 收口（MUST NOT 以 `not a working tree` 报错），使 create 补偿的「失败面零残留」与 delete / Cleanup 的重试链可达收敛。
- **护栏**：兜底 MUST NOT 施于 `main_root` 自身、`main_root` 的祖先、`main_root` 的子目录与文件系统根（命中即 `Err` 且磁盘零变化）。除此黑名单外 MUST NOT 以「是否 worktree 形态（目录内 `.git` 指针在场）/ 目录是否为空」作白名单——删除中途 abort 时 `.git` 常在 ③ 阶段已被先删，形态判据会在最需要自愈的窗口拒绝自愈。
- **`prune` 次序与安全**：`git worktree prune` SHALL 仅在目标目录已不在场之后调用（登记仍有效且目录在场的活 worktree MUST NOT 被波及）；`prune` 失败 SHALL 静默吞（目标目录已回收即主目标达成）。
- **零重试零退避**：兜底失败即 `Err`，MUST NOT 引入重试、退避或「优雅握手」（实测强杀后立即释放句柄，不存在需要等待的窗口；退避只会把持久占用伪装成偶发失败）。
- **降级留痕**：兜底成功而 git 段失败时，原 git 错误 SHALL 以 `[vcs]` 前缀的 stderr 日志留痕（沿 `commit_all` 既有先例），MUST NOT 借此扩 port 签名、`Result` 形状或 wire DTO。
- **失败面**：`Err` SHALL 携 worktree 绝对路径、原 git 错误、OS 错误与成因清单（终端 cwd / 编辑器打开的文件 / 常驻子进程 / 杀毒实时扫描 / 云同步客户端），MUST NOT 指引用户执行 `git worktree remove --force <path>`（对已注销孤儿恒报 `not a working tree`），MUST NOT 以 `git worktree list` 单源对账收尾。
- **双源对账**：本单点 `Err`、create 前置⑨ 保底门槛文案、create 两处补偿残留文案 SHALL 采用同一双源形态——「① `git worktree list` 对账；② 直接检查 worktree 落位根 `<git 根父目录>/.worktrees/<仓库目录名>/` 下的子目录（删除失败留下的残留不在 git 登记面，直接删除该子目录即可）」。MUST NOT 出现「以磁盘计数引导、以 git 对账收尾」的错配（计数源与对账源须为同一真相源）。branch 侧指引仍为 `git branch -D <branch>`（分支不受本缺陷影响）。
- **平台**：两段式对 Windows 与 POSIX 同构（除扩展路径前缀仅 Windows 生效外零 `cfg` 分叉）；POSIX 允许删除被打开的文件，兜底只会更容易成功。

#### Scenario: 已注销孤儿自愈回收

- **WHEN** 磁盘上存在一个 git 已注销的孤儿目录（登记与 admin 目录已拆、目录仍在场，`git worktree list` 不显示、`git worktree prune` 无输出），对该路径调用 `remove_worktree`
- **THEN** 返回 `Ok`；目录自磁盘消失；`git worktree list` 仍只列主仓；`<main>/.git/worktrees/` 无该 worktree 的残留 admin；全程一次调用、零重试

#### Scenario: 登记与目录一致时原语义零变化

- **WHEN** worktree 由 `add_worktree` 正常铸出（目录与登记一致）后调用 `remove_worktree`
- **THEN** 返回 `Ok`、目录移除、主仓工作树无损、branch `change/<name>` 留存、同路径可再 `add` 成功（既有补偿面语义逐字不变）

#### Scenario: 目录缺席幂等

- **WHEN** 对从未存在（或已被手动删除）的路径调用 `remove_worktree`
- **THEN** 返回 `Ok`（`prune` 尽力后收口），零删除动作、零 panic；重复调用仍 `Ok`

#### Scenario: 主仓护栏拒绝

- **WHEN** 以 `main_root` 自身、`main_root` 的祖先或其子目录调用 `remove_worktree`
- **THEN** 返回显式 `Err`（含路径与拒绝因），磁盘零变化（主仓文件与目录全在）

#### Scenario: prune 不误伤活 worktree

- **WHEN** 两个活 worktree 在案（登记与目录均在场），对其中一个执行两段式的兜底分支
- **THEN** 另一个仍在 `git worktree list` 中且目录在场，其 admin 目录未被清除

#### Scenario: 两段式皆失败携成因

- **WHEN** worktree 内某文件被不允许删除共享的句柄独占（Windows 语义：`share = READ` 不含 delete），两段式均失败
- **THEN** 返回 `Err`，文案含绝对路径、`git worktree remove` 失败原文、OS 错误与成因清单，且**不含**「`git worktree remove --force <path>`」单源指引；句柄释放后重试同一路径 → `Ok`（收敛）

#### Scenario: 零 port 与 wire 变更

- **WHEN** 审查 `WorktreePort` / `ArchiveVcsPort` 的方法签名与 IPC DTO
- **THEN** 二者方法族与 `Result<(), String>` 形状逐字不变，两缝委托同一实现；无新命令、无 DTO 字段演进、生成物零 diff

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/package.json` | 版本交付 | 0.4.13 → 0.4.14；`src-tauri/Cargo.toml` 不随动 |
| `crates/core/foundation`（身份段派生单点，新） | workspace 身份段纯函数 | `{可读段}-{sha256 前 16 字节 32 位 hex}`；仅 workspace db 文件名消费（worktree 落位已解耦）；纯函数可密集测试 |
| `crates/infra/vcs`（新，裸名 `vcs-runtime`，vcs 边界首成员） | git 工作面进程执行 | `git rev-parse --show-toplevel` 落位派生（git 根同级 `.worktrees/<仓库目录名>/`）/ `git worktree add` / `git status --porcelain` / bootstrap 安装 spawn；PATH 探测前置（沿 bash.rs / static_check 先例）；零 Tauri；→ workflow（port 类型）；foundation 仅测试依赖 |
| `crates/core/workflow/src/write/`（WorktreePort，新） | 写面 → vcs 执行的进程内缝 | port 定义留消费者（core/workflow）；spawn 不进 core；sync 签名 |
| `crates/core/workflow/src/write/create.rs` | create 建域组合 | 建档（worktree / base_commit）→ worktree add（HEAD 基线）→ worktree 内目录树 + explore.md → bootstrap；扩展校验全 IO 前置；脏仓警告；bootstrap 按 `projects` 分派（空 → worktree 根单扫描；非空 → 逐项目根扫描安装），失败不回滚 |
| `crates/infra/agent/src/compose.rs` | compose_turn 拆参 | store 半边 = 命令层注入的 workspace root 库实例；cwd 半边经既有 turn 通道；worktree 路径不进 `for_root` |
| `crates/core/orchestration/src/control.rs` | 并行冲突键 | `(workspace root, change)` 复合键；`RunUpdate` / 快照 DTO 零改动 |
| `src/commands/change_flow/mod.rs` | exec root 解析 | db 记录 worktree 字段 → exec root（None → 主 root）；compose 注入装配 |
| `src/commands/changes/mod.rs` | create / detail / artifact 命令面 | `CreateOutcome` 增 `worktree` / `warnings`；`locate_change` worktree 回退供 detail / read_artifact 消费 |
| `crates/core/workflow/src/queries/` | 读面 worktree 感知 | db status 权威归组；detail 出线 worktree 路径（None → null）；产物发现经 exec root |
| `packages/desktop/src/views/changes/` | 前端可见性 | create 对话框路径与警告呈现；详情 worktree 信息行（legacy 不渲染） |
| （零实现触点——本能力清理条款由 desktop-change-archive Cleanup 步承载） | 边界改写 | worktree 清理自动化偿还归属 archive 相位；branch 生命周期仍手动；git 根同级 `.worktrees/<仓库目录名>/` 落位单点零改动（目录数扫描由 desktop-change-create 前置⑨消费） |
| `crates/core/workflow/src/write/create.rs`（零改动面） | worktree 建域 | 基线恒 HEAD、目录树语义零改动；bootstrap 走 `projects` 分派（见前条）；initialize 的 clean 步在既有 worktree 面上恢复基线（不新增建域语义） |
| `crates/infra/vcs`（restore_baseline，新） | 基线恢复 | 经 `ArchiveVcsPort::restore_baseline` 缝执行 `reset --hard` + `clean -fd` 排除面（foundation 名单 + `openspec/`）；仅 worktree 在场时被调用（walker 保证），主仓零重置 |

### 模块增量：worktree 回收两段式（worktree-remove-orphan-defect）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/infra/vcs/src/git.rs`（`remove_worktree`，改） | worktree 回收单点 | `pub(crate) fn remove_worktree(main_root: &Path, worktree: &Path) -> Result<(), String>`：签名不变，体内改两段式（`git worktree remove --force` → 失败按目录在场分流 → OS 层兜底 → `prune` → `Ok`）；`Err` 携路径 / git 原文 / OS 错误 / 成因清单 / 双源指引；兜底成功时 `eprintln!("[vcs] …")` 留痕原 git 错误 |
| `crates/infra/vcs/src/git.rs`（`prune_worktrees`，新私有） | 残留 admin 清理 | `git worktree prune`；仅目标目录不在场后调用；失败静默吞（`Result` 由调用点择取，不外泄为用户面错误） |
| `crates/infra/vcs/src/git.rs`（`delete_tree`，新私有） | OS 层兜底删除 | `fs::remove_dir_all`；Windows 对绝对路径加 `\?\` 扩展前缀（超长路径面），POSIX 直删；零重试零退避 |
| `crates/infra/vcs/src/git.rs`（`is_protected_path`，新私有） | 兜底护栏 | path == `main_root` / 祖先 / 子目录 / 文件系统根 → 拒绝；命中即 `Err` 且零删除 |
| `crates/infra/vcs/src/lib.rs`（`ProcessArchiveVcs::remove_worktree`，改） | 归档缝透传 | 取消自组「请手动执行 `git worktree remove --force {path}` 清理」wrapper，纯透传单点 `Err`（文案单点收口，防两缝漂移）；`ArchiveVcsPort` 签名不变 |
| `crates/infra/vcs/src/lib.rs`（`ProcessWorktree::remove_worktree`） | create / delete 缝 | 逐字不变（已为透传）；两缝同实现同文案 |
| `crates/infra/vcs/src/git_test.rs` / `lib_test.rs` | 回收语义覆盖 | 孤儿回收 / 缺席幂等 / 主仓护栏 / `prune` 不误伤 / 两段式皆失败携成因（Windows 句柄装置）/ 单点文案透传 |
| `packages/desktop/package.json` | 版本交付 | 落地时 `HEAD` 现值 + 1 patch（现值 0.6.42 → 0.6.43）；`src-tauri/Cargo.toml` 不随动 |

### 模块增量：`projects` 依赖声明与拓扑序安装（project-config-protocol-extension）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 类型 | 职责 / 契约 |
|------|------|-------------|
| `crates/core/workflow/src/write/create.rs` | Function | 新增 `pub struct BootstrapProject { pub root: String, pub depends_on: Vec<String> }`（`depends_on` 缺省空，工作流侧不区分「未设」与「空」）；`create` / `bootstrap` 的项目入参由 `project_roots: &[String]` 改为 `projects: &[BootstrapProject]` |
| `crates/core/workflow/src/write/create.rs`（`install_order`，新私有纯函数） | Function | `fn install_order(projects: &[BootstrapProject]) -> Vec<&str>`：后序 DFS（依赖先于依赖方）+ 声明序 tie-break + `visited` 去重/断环 + 悬空依赖跳过；纯内存、零 `Err`、零 IO |
| `crates/core/workflow/src/write/create.rs`（`bootstrap` / `bootstrap_project`，改） | Function | 遍历序改由 `install_order` 产出；`bootstrap_project` 的映射表探测、警告文案、「每项目至多一次安装」「失败继续」语义逐字不变（只换遍历顺序与折叠后的元素集） |
| `crates/core/workflow/src/write/promote.rs` | Function | `promote_explore` 项目入参随 `create` 同形态（纯透传，语义零变化） |
| `crates/core/workflow/src/write/mod.rs` | API | 导出 `BootstrapProject`（命令层构造入参） |
| `src-tauri/src/commands/changes/mod.rs` / `src-tauri/src/commands/explores/mod.rs` | API（命令层） | 配置读取映射改为 `config::ProjectConfig → BootstrapProject`（`root` 原样、`depends_on` 缺省空）；`language` 在安装路径**不消费** |
| `crates/core/workflow/src/write/create_test.rs` / `promote_test.rs` | 测试 | 夹具 `create_with_projects` 收 `&[BootstrapProject]`；新增拓扑序 / 无依赖退化 / 悬空与自引用 / 成环 / 重复 root 五例；既有九例断言结构不变 |
