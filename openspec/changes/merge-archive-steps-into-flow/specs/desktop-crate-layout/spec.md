# desktop-crate-layout Specification (Delta)

## Purpose

约束 `packages/desktop` 桌面端包的落位方式与 Rust crate 分层组织：作为完全独立的包存在于仓库中，cargo workspace 按三层分组组织依赖方向，并在代码命名上隔离 `openspec` 字样、将磁盘路径收口到 foundation 的 layout 解析器。vcs 边界的消费者随归档并入 phase → step 模型定稿为「walker（core/orchestration）第三执行缝」，实现仍驻 `crates/infra/vcs`，七类边界分类学不变。

## MODIFIED Requirements

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力扩展为五类，再经检查域扩展为六类，再经 change worktree 扩展为七类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-runtime`）；允许进程 spawn，禁 Tauri；
- **checks 边界**：外部进程 + pass/fail 结论门禁的检查域（现有成员：static-check、test-execution）——纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；允许进程 spawn，禁 Tauri；runner port 留 `core/orchestration`；
- **vcs 边界**：外部进程 + 版本控制工作面（git worktree 建域 / 脏仓探测 / 依赖 bootstrap 安装 / archive 相位 git 步（commit / rebase / ff_merge / commit_paths）的进程执行）——建域 port（`WorktreePort`）落位 `crates/core/workflow`（port 属于消费者，spawn 不进 core），归档 git 步 port（`ArchiveVcsPort`）落位 `crates/core/orchestration`（walker 第三执行缝），实现落位 `crates/infra/vcs`（crate 裸名 `vcs-runtime`）；允许进程 spawn，禁 Tauri。

领域核心与应用编排位于各边界之内：core 各 crate 纯依赖；`store` 不依赖 core 行为（仅依赖 `agent` 纯类型作嵌装载荷与 `foundation` 身份段派生纯函数），`agent-runtime` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证。`infra/` 目录表达各边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent / checks / vcs 七类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: 检查步归类 checks 边界

- **WHEN** 评审 static-check 与 test-execution 执行步的落位
- **THEN** 归 checks 边界：core/checks 纯层零 spawn 零 Tauri，infra/checks 执行层允许 spawn；检查域成员不出现在 infra/agent

#### Scenario: git 工作面归类 vcs 边界

- **WHEN** 评审 git worktree 建域 / 脏仓探测 / bootstrap 安装 / archive 相位 git 步执行的落位
- **THEN** 归 vcs 边界：core 侧 port（`WorktreePort` 在 core/workflow、`ArchiveVcsPort` 在 core/orchestration）零 spawn 零 Tauri，infra/vcs 执行层允许 spawn；vcs 成员不出现在 infra/agent 或 desktop-app 内联 spawn

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate（含 agent-runtime 与 checks、vcs 各 crate）的依赖声明
- **THEN** 无任何 Tauri 系依赖，shell 依赖仅存在于 desktop-app

### Requirement: 未来租户归位规则

已规划特性 SHALL 按下表归位；未落地行 MUST NOT 被任何变更据表预建目录或 crate（首个真实成员出现才立）：

| 特性 | 边界 | 归位 | 规则 |
|---|---|---|---|
| workspace 写文件 | 文件 | `commands/exec/` 轨道 | trait 形状等第一条真实执行命令落地时定形；写用户 repo 属对 repo 的变更，落地时 SHALL 有 write protection / 路径包含性检查 / 审计 |
| workspace 读写 db | db | `infra/store` 的 workspace 维度 | 落盘策略见 desktop-data-dimensions 能力 |
| 图数据库 | db | `infra/graph`（暂不建） | 暂不考虑；关系查询真实痛点出现再启 |
| 后端 api | api | `infra/api`（不建） | 先占位留痕：用途未定，MUST NOT 建目录、MUST NOT 动代码 |
| agent 执行 | agent | `core/agent`（契约）+ `infra/agent`（实现，裸名 `agent-runtime`） | 已由 desktop-agent-execution 落地：三租户抽象 MVP 落地 CLI 与 SDK 双引擎，远程 API 未预建 |
| git 工作面（worktree 建域 / 脏仓探测 / 依赖 bootstrap / archive 相位 git 步） | vcs | `core/workflow`（WorktreePort）+ `core/orchestration`（ArchiveVcsPort）+ `infra/vcs`（实现，裸名 `vcs-runtime`） | 已由 desktop-change-worktree 落地建域；archive 相位 git 步由本变更定稿为 walker 第三执行缝（`ArchiveVcsPort` 落 core/orchestration，实现不越 `infra/vcs`） |

`infra/fs` SHALL 在文件边界长出独立适配时再立，不预建。

#### Scenario: 无预建目录

- **WHEN** 检查 `crates/infra/` 目录树
- **THEN** 仅 `store`、`agent`（裸名 `agent-runtime`）、`checks`、`watch`、`vcs` 五个已落地成员，不存在 `infra/graph` / `infra/api` / `infra/fs` 空目录

#### Scenario: 图数据库与 api 决策留痕

- **WHEN** 查阅本能力 spec
- **THEN** 图数据库"暂不考虑、痛点驱动再启"与 api "先占位留痕"两条裁决可考

#### Scenario: exec 轨道由 agent 执行开通

- **WHEN** 检查 `commands/exec/`
- **THEN** 轨道已由 agent 执行命令开通（`agent_start` / `agent_runs` / `agent_run_events`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop`（包） | 独立桌面应用 | 自带 lockfile / 构建脚本；不进根 workspace、不进 `dist/` |
| `crates/core/foundation` | 共享地基 | 无 workspace 内依赖；第二消费者出现才下沉新能力 |
| `crates/core/workflow` | change 域纯读库 + 写面 | 依赖 foundation；零 Tauri 依赖；无指令概念；不依赖 store；`WorktreePort` 落此 |
| `crates/core/agent`（裸名 `agent`） | agent 域中立契约 | 零 Tauri、零 spawn、零 workspace 内依赖、derive-free |
| `crates/infra/store` | native_db 本地库 | 行为自含、依赖 `agent` 纯类型 + `foundation` 身份段纯函数；零 Tauri |
| `crates/infra/agent`（裸名 `agent-runtime`） | agent 边界实现 | 仅承载会话租户身份；static_check 移出 checks 边界；允许进程 spawn；禁 Tauri |
| `crates/desktop-app` | Tauri 壳 | 依赖 workflow + foundation + store + agent + agent-runtime + vcs-runtime；command 薄包装 |
| 七类边界分类学 | 落位先声明边界 | shell / db / 文件 / api / agent / checks / vcs；core 与 infra 禁 Tauri |
| `crates/core/checks` + `crates/infra/checks` | 检查域双层落位 | 纯层零 spawn、执行层允许 spawn；两 crate 均零 Tauri |
| `crates/infra/vcs`（裸名 `vcs-runtime`） | git 工作面进程执行 | 依赖：`vcs-runtime` → workflow（WorktreePort 类型）+ orchestration（ArchiveVcsPort 类型，若落此）+ foundation；允许 spawn；禁 Tauri；由 desktop-app 装配注入 |
| `crates/core/orchestration`（ArchiveVcsPort） | walker 第三执行缝（vcs） | `ArchiveVcsPort` 定义落消费者（core/orchestration）；spawn 不进 core；实现驻 infra/vcs |
| `crates/infra/` | 目录表达计划 | store / agent（裸名 agent-runtime）/ checks / watch / vcs 五个已落地成员；graph / api / fs 均不预建 |
| `mod_test.rs` 命名隔离扫描（扩展） | 字面量执法不变量 | `openspec` 目录名 + `config.json` 文件名双禁令；唯一触点 layout/mod.rs 常量组 |
