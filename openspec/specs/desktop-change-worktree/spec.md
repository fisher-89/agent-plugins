# desktop-change-worktree Specification

## Purpose

定义 change worktree 双 root 执行能力：写面 create 以主仓 HEAD 为基线为每次创建分配 per-change git worktree（全局数据目录 `worktrees/{身份段}/<name>` + branch `change/<name>`，身份段与 workspace db 文件名同源派生），change run 以 workspace root 为唯一 store 身份锚、worktree 为执行根的双 root 组合执行；配套确定性依赖 bootstrap、脏仓警告不阻止、merge-first 归档引导与 worktree 路径 UI 可见性。

## Requirements

### Requirement: worktree 落位与身份派生单点

change worktree SHALL 落位于全局数据目录下 `worktrees/` 子树：`<data_root>/worktrees/{身份段}/<change-name>`。身份段 SHALL 为 workspace canonical root 的确定性派生 `{可读段}-{sha256 前 16 字节 32 位小写 hex}`（可读段清洗与截断规则沿既有 `readable_segment` 语义），与 workspace db 文件名（`workspace_db_file_name`）**同源同算法**（去 `.redb` 后缀）——身份段派生 SHALL 收口 foundation 单点纯函数，db 文件名与 worktree 子目录 SHALL 同源消费该单点（同根恒同名、跨重启可复现、异根必不同名；store 内不得残留第二份派生实现）。`worktrees/` 子树与 `workspaces/` 子树 SHALL 相邻而零耦合：worktree 是 git 管理的执行锚，MUST NOT 进入任何 db 维度载体语义（desktop-data-dimensions 零改动），git 树 MUST NOT 感知 db 文件。数据目录根 SHALL 由 desktop-app 注入（沿 store 注入式打开纪律）；跨盘（data 目录与 repo 异盘）的 git worktree 合法，SHALL 接受。

#### Scenario: 同根恒同名可复现

- **WHEN** 对同一 workspace root 跨进程重启两次派生 worktree 子目录路径
- **THEN** 两次派生结果逐字一致，且与该 workspace db 文件名的身份段一致（仅后缀 `.redb` 之差）

#### Scenario: 异根必不同名

- **WHEN** 两个不同 workspace 各自创建同名 change 的 worktree
- **THEN** 两个 worktree 落位于不同身份段子目录下，互不冲突、互不可见

#### Scenario: 子树零耦合

- **WHEN** 审查 worktrees 子树与 workspace 库的消费关系
- **THEN** db 路径派生只消费 `workspaces/` 子树，worktree 目录派生只消费 `worktrees/` 子树，git 工作树内无 db 文件感知

### Requirement: create 分配 worktree 与分支基线

写面 `create` 对每次成功创建 SHALL 分配同名 worktree：以主仓 HEAD 为基线 `git worktree add` 落位于身份派生目录，并建 branch `change/<name>`——主仓未提交与 staged 改动 MUST NOT 进入 worktree（基线恒 HEAD）。`openspec/changes/<name>/` 目录树与 `explore.md` SHALL 建于 worktree 内（主仓 active 树在 merge 前 MUST NOT 含该目录）。建档 SHALL 记录 worktree 绝对路径与创建基线（`ChangeRecord.worktree` / `base_commit`，字段语义见 desktop-change-state-store）。前置校验 SHALL 在任何 IO 之前覆盖：git 可用性（git 不可发现或主仓非 git 仓 → 显式 `Err` 引导，MUST NOT 静默回退主 root 创建）、branch `change/<name>` 已存在、worktree 目标目录已存在——拒绝面零 worktree、零建档、零目录。进程 spawn（git / bootstrap）SHALL 经 core/workflow 定义、vcs 边界（`crates/infra/vcs`，裸名 `vcs-runtime`）实现的 port 缝承载，core/workflow 零 infra 依赖、零进程 spawn 红线不变。

#### Scenario: 成功建域四件套

- **WHEN** 对干净 git 仓 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** worktree 目录存在于 `worktrees/{身份段}/<name>`、branch `change/<name>` 在案且指向主仓 HEAD、worktree 内 `openspec/changes/<name>/explore.md` 为 goal 原文、db `ChangeRecord` 携 worktree 路径与 base_commit；主仓 active 树无该目录

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

写面 `create` SHALL 在 worktree 就绪后执行确定性依赖引导：探测 worktree 内 lockfile（已知管理器映射：pnpm-lock.yaml / package-lock.json / yarn.lock / Cargo.lock 等，映射表由 design 定稿）→ 命中则在 worktree 内 spawn 对应安装命令（经 vcs 边界 port）；未知管理器或无 lockfile → 跳过并在警告面注记。bootstrap 失败 SHALL NOT 回滚 create（建档与 worktree 保留），失败信息 SHALL 立即呈现于 create 产出警告清单（static-check / test-execution 为确定性步骤无 agent 救场，后续大声失败兜底）。

#### Scenario: lockfile 命中执行安装

- **WHEN** worktree 检出含 `pnpm-lock.yaml` 的仓库
- **THEN** create 在 worktree 内 spawn 对应安装命令（cwd = worktree），成功后无相关警告

#### Scenario: 未知管理器跳过注记

- **WHEN** worktree 内无任何已知管理器 lockfile
- **THEN** bootstrap 跳过 spawn，警告清单注记「未识别依赖管理器，跳过依赖引导」

#### Scenario: 失败不回滚

- **WHEN** 安装命令非零退出
- **THEN** create 以成功收口（建档与 worktree 保留），警告清单含失败摘要；后续 static-check / test-execution 步以自身失败面兜底

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

满足前置的两条等价路径：**归档链自动段**（desktop-change-archive：归档编排链在双写收口前自动执行 worktree 提交 + 主仓合入，为用户点击归档按钮后的缺省路径）与**用户手动 merge**（既有路径维持，归档链的幂等跳过将其视为已完成段）。merge SHALL NOT 被强制：用户可不 merge 弃置 change（V1 弃置 = 手动清 worktree / branch，db 记录保留，见「V1 范围与边界留痕」）。自动合入的冲突与主仓状态约束（显式失败引导手动、不吞并无关改动、不改写主仓历史）由 desktop-change-archive 约束。run 收口语义不变：全相位 pass 照常停等「Ready for archiving」，MUST NOT 自动归档、MUST NOT 自动 merge（归档链由用户点击归档入口发起，非 run 收口的自动动作）；worktree / branch 清理仍为用户手动边界（归档链成功后不 remove worktree、不删 branch）。

#### Scenario: 归档链自动提交合入满足前置

- **WHEN** 带 worktree 记录且主仓两树均未命中目录的 change 经归档入口发起归档链，提交与合入段成功
- **THEN** 主仓出现 `changes/<n>/`，双写收口零特判走通（无 worktree 路径分支进入收口段）；用户全程无手动 git 操作

#### Scenario: 手动 merge 后归档链跳过合入

- **WHEN** 用户已手动将 worktree 分支 merge 回主仓（主仓出现 `changes/<n>/`）后经归档入口发起归档链
- **THEN** 合入段幂等跳过（零 git 动作），链径直至双写收口；直接经裸归档命令调用亦零改动可用（既有语义）

#### Scenario: 未 merge 的直接归档仍被引导拒绝

- **WHEN** 绕过归档链对带 worktree 记录且主仓两树均未命中目录的 change 直接调用归档写面
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支；db 与磁盘零变化（既有引导语义不因归档链在场而退役）

#### Scenario: run 收口不自动合入

- **WHEN** 带 worktree 记录的 change run 以全相位 pass 收口
- **THEN** run 照常停等「Ready for archiving」，零自动 merge、零自动归档、零归档链触发（用户点击归档入口是唯一发起源）

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

1. **worktree / branch 清理手动**：`git worktree remove` + `git branch -d` 由用户手动执行（与「手动提交」同哲学）；桌面 cleanup 按钮留后续。
2. **legacy 主 root 路径**：`worktree=None` 存量记录照旧主 root 执行，零迁移；非 git 仓 workspace 的 create 显式拒绝（不提供 legacy 创建逃生口，真实诉求另立变更）。
3. **`agent_start` 与探索笔记留主仓 root**：exec 轨道手动会话与 `openspec/explores/` 笔记不混入 change worktree。
4. **config / AGENT.md 冻结**：worktree 内配置停在创建时刻（base commit 检出），主仓中途改动不传导，文档化。
5. **构建成本接受**：fresh worktree 依赖 / 构建产物全量重建；共享 target 目录类逃生口（`CARGO_TARGET_DIR` 等）仅文档化，不内建机制。
6. **孤儿 worktree 对账文档化**：create 中途崩溃遗留的 worktree / branch 由 `git worktree list` 对账、幂等采纳或手动清理（前置校验保证下次创建显式报错而非误用），不内建自动清理。
7. **report_dir 绝对路径接受失效**：phase_log 报告文本内嵌的 worktree 绝对路径在 worktree 删除后失效（转录历史，仅展示面），不做相对化。
8. **并行 merge 冲突手动解**：两 change 改同一文件各自绿、merge 撞时走 git 原生冲突处理。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.13` 升级为 `0.4.14`（用户可见新功能；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 2.10.44 与三类交付产物）。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.14，`plugins/dev-team/package.json` version 仍为 2.10.44

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/foundation`（身份段派生单点，新） | workspace 身份段纯函数 | `{可读段}-{sha256 前 16 字节 32 位 hex}`；db 文件名与 worktree 子目录同源消费；纯函数可密集测试 |
| `crates/infra/vcs`（新，裸名 `vcs-runtime`，vcs 边界首成员） | git 工作面进程执行 | `git worktree add` / `git status --porcelain` / bootstrap 安装 spawn；PATH 探测前置（沿 bash.rs / static_check 先例）；零 Tauri；→ workflow（port 类型）+ foundation |
| `crates/core/workflow/src/write/`（WorktreePort，新） | 写面 → vcs 执行的进程内缝 | port 定义留消费者（core/workflow）；spawn 不进 core；sync 签名 |
| `crates/core/workflow/src/write/create.rs` | create 建域组合 | 建档（worktree / base_commit）→ worktree add（HEAD 基线）→ worktree 内目录树 + explore.md → bootstrap；扩展校验全 IO 前置；脏仓警告；bootstrap 失败不回滚 |
| `crates/infra/agent/src/compose.rs` | compose_turn 拆参 | store 半边 = 命令层注入的 workspace root 库实例；cwd 半边经既有 turn 通道；worktree 路径不进 `for_root` |
| `crates/core/orchestration/src/control.rs` | 并行冲突键 | `(workspace root, change)` 复合键；`RunUpdate` / 快照 DTO 零改动 |
| `src/commands/change_flow/mod.rs` | exec root 解析 | db 记录 worktree 字段 → exec root（None → 主 root）；compose 注入装配 |
| `src/commands/changes/mod.rs` | create / detail / artifact 命令面 | `CreateOutcome` 增 `worktree` / `warnings`；`locate_change` worktree 回退供 detail / read_artifact 消费 |
| `crates/core/workflow/src/queries/` | 读面 worktree 感知 | db status 权威归组；detail 出线 worktree 路径（None → null）；产物发现经 exec root |
| `packages/desktop/src/views/changes/` | 前端可见性 | create 对话框路径与警告呈现；详情 worktree 信息行（legacy 不渲染） |
