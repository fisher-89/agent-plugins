# desktop-crate-layout Specification (Delta)

## MODIFIED Requirements

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力（desktop-agent-execution）扩展为五类，再经检查域（desktop-checks-domain）扩展为六类，再经 change worktree（desktop-change-worktree）扩展为七类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app（通用 shell 执行仅 desktop-app；runner 内部的子进程管理属 agent 边界实现细节，不属于 shell 边界）；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-runtime`）；允许进程 spawn，禁 Tauri；
- **checks 边界**：外部进程 + pass/fail 结论门禁的检查域（现有成员：static-check、test-execution）——纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；允许进程 spawn，禁 Tauri；runner port 留 `core/orchestration`（port 属于消费者，spawn 不进 core）；
- **vcs 边界**：外部进程 + 版本控制工作面（git worktree 建域 / 脏仓探测 / 依赖 bootstrap 安装 / 归档提交与主仓合入的进程执行——desktop-archive-change 扩族）——port 定义落位 `crates/core/workflow`（port 属于消费者，spawn 不进 core；归档链消费的 port 缝归属由该变更 design 定稿，允许落 `core/orchestration`），实现落位 `crates/infra/vcs`（crate 裸名 `vcs-runtime`）；允许进程 spawn，禁 Tauri；既有 `infra/agent` 内 git_diff 迁入 vcs 留后续变更。

领域核心（change 域读模型、agent 域契约、检查域纯层）与应用编排（现为 command）位于各边界之内：core 各 crate 纯依赖；`store` 不依赖 core 行为（仅依赖 `agent` 纯类型作嵌装载荷与 `foundation` 身份段派生纯函数，规则修订见「crate 三层分组与依赖规则」），`agent-runtime` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证（既有 requirement 不变）。`infra/` 目录表达 db / api / agent / checks / vcs 等边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent / checks / vcs 七类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: 检查步归类 checks 边界

- **WHEN** 评审 static-check 与 test-execution 执行步的落位
- **THEN** 归 checks 边界：core/checks 纯层零 spawn 零 Tauri，infra/checks 执行层允许 spawn；检查域成员不出现在 infra/agent

#### Scenario: git 工作面归类 vcs 边界

- **WHEN** 评审 git worktree 建域 / 脏仓探测 / bootstrap 安装 / 归档提交与主仓合入执行的落位
- **THEN** 归 vcs 边界：core 侧 port（WorktreePort 或归档链 port 缝）零 spawn 零 Tauri，infra/vcs 执行层允许 spawn；vcs 成员不出现在 infra/agent 或 desktop-app 内联 spawn

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate（含 agent-runtime 与 checks、vcs 各 crate）的依赖声明
- **THEN** 无任何 Tauri 系依赖（tauri、tauri-plugin-* 等），shell 依赖仅存在于 desktop-app

### Requirement: 未来租户归位规则

已规划特性 SHALL 按下表归位；未落地行 MUST NOT 被任何变更据表预建目录或 crate（首个真实成员出现才立）：

| 特性 | 边界 | 归位 | 规则 |
|---|---|---|---|
| workspace 写文件 | 文件 | `commands/exec/` 轨道 | trait 形状等第一条真实执行命令落地时定形（既有决策不变）；写用户 repo 属对 repo 的变更，落地时 SHALL 有 write protection / 路径包含性检查 / 审计 |
| workspace 读写 db | db | `infra/store` 的 workspace 维度 | 落盘策略见 desktop-data-dimensions 能力 |
| 图数据库 | db | `infra/graph`（暂不建） | 暂不考虑；关系查询真实痛点出现再启，届时按 db 边界新成员落位 |
| 后端 api | api | `infra/api`（不建） | 先占位留痕：用途未定，MUST NOT 建目录、MUST NOT 动代码 |
| agent 执行 | agent | `core/agent`（契约）+ `infra/agent`（实现，裸名 `agent-runtime`） | 已由 desktop-agent-execution 落地：三租户抽象 MVP 落地 CLI 与 SDK（rig 进程内）双引擎，远程 API 未预建；进程 spawn 属 agent 边界实现细节，非 shell 边界 |
| git 工作面（worktree 建域 / 脏仓探测 / 依赖 bootstrap / 归档提交与主仓合入） | vcs | `core/workflow`（WorktreePort）+ `infra/vcs`（实现，裸名 `vcs-runtime`） | worktree 建域已由 desktop-change-worktree 落地：port 属消费者、spawn 不进 core；归档提交 / 合入由 desktop-archive-change 扩族（port 缝归属由其 design 定稿，实现不越 `infra/vcs`）；既有 `infra/agent` 内 git_diff 迁入 vcs 留后续变更 |

`infra/fs` SHALL 在文件边界长出独立适配（区别于 workflow 内纯读逻辑的、可复用的 fs 基础设施）时再立，不预建。

#### Scenario: 无预建目录

- **WHEN** 检查 `crates/infra/` 目录树
- **THEN** 仅 `store`、`agent`（裸名 `agent-runtime`）、`checks`、`watch`、`vcs` 五个已落地成员，不存在 `infra/graph` / `infra/api` / `infra/fs` 空目录

#### Scenario: 图数据库与 api 决策留痕

- **WHEN** 查阅本能力 spec
- **THEN** 图数据库"暂不考虑、痛点驱动再启"与 api "先占位留痕"两条裁决可考，后续变更无需重新辩论是否引入

#### Scenario: exec 轨道由 agent 执行开通

- **WHEN** 检查 `commands/exec/`
- **THEN** 轨道已由 agent 执行命令开通（`agent_start` / `agent_runs` / `agent_run_events`，见 desktop-agent-execution）
- **AND** workspace 写文件仍将落此轨道而非 queries / workspaces 轨道，空壳 Executor trait 纪律不变
