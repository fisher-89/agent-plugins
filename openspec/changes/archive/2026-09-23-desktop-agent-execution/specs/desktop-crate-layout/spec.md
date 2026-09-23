# desktop-agent-execution — desktop-crate-layout 变更集

> agent 执行落地为第五类边界：`core/agent`（契约）+ `infra/agent`（实现）。本 delta 与 `desktop-agent-execution` 能力 spec 配套阅读。

## MODIFIED Requirements

### Requirement: crate 三层分组与依赖规则

Rust 侧 SHALL 按 cargo workspace 组织为三层分组（core 引擎侧 / infra 基础设施侧 / desktop-app 壳）：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析（刻意极小）
│   ├── workflow/              # crate: change 域（model / parse / queries / artifacts）
│   └── agent/                 # crate: agent 域中立契约（AgentEvent / AgentRunner / 状态机）
├── infra/                     # 基础设施侧命名空间（纯目录，不是 crate）
│   ├── store/                 # crate: redb 本地库（db 边界第一成员）
│   └── agent/                 # 目录：agent 边界实现（crate 裸名 agent-cli，唯一性约束）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → store`，且 `desktop-app → agent + agent-cli`、`agent-cli → agent`。`foundation` MUST NOT 依赖任何其他 crate；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 任何 crate（自含模型），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；`agent` MUST NOT 依赖 workspace 内任何 crate；`agent-cli` SHALL 仅依赖 `agent`，MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。agent 两 crate 沿此规则取裸名 `agent`（core）与 `agent-cli`（infra，因唯一性不得复用 `agent`）。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`foundation`、`store`、`agent`、`agent-cli`，`workflow` 依赖 `foundation`，`agent-cli` 依赖 `agent`
- **AND** `foundation` 无 workspace 内依赖；`store` 无 workspace 内 crate 依赖、无 Tauri 依赖；`agent` 无 workspace 内依赖；`agent-cli` 与 `agent` 均无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力，不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具

#### Scenario: store 落位 infra 且 core 依赖不到 store

- **WHEN** 检查 `store` 与 core 各 crate 的 `Cargo.toml` 及目录落位
- **THEN** `store` 落位 `crates/infra/store`，core 三个 crate（foundation / workflow / agent）的依赖中无 `store`
- **AND** `store` 不出现 Tauri 依赖

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力（desktop-agent-execution）扩展为五类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app（通用 shell 执行仅 desktop-app；runner 内部的子进程管理属 agent 边界实现细节，不属于 shell 边界）；
- **db 边界**：redb（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-cli`）；允许进程 spawn，禁 Tauri。

领域核心（change 域读模型、agent 域契约）与应用编排（现为 command）位于各边界之内：core 三 crate 纯依赖；`store` 不依赖 core（自含模型），`agent-cli` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证（既有 requirement 不变）。`infra/` 目录表达 db / api / agent 等边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent 五类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate（含 agent-cli）的依赖声明
- **THEN** 无任何 Tauri 系依赖（tauri、tauri-plugin-* 等），shell 依赖仅存在于 desktop-app

### Requirement: 未来租户归位规则

已规划特性 SHALL 按下表归位；未落地行 MUST NOT 被任何变更据表预建目录或 crate（首个真实成员出现才立）：

| 特性 | 边界 | 归位 | 规则 |
|---|---|---|---|
| workspace 写文件 | 文件 | `commands/exec/` 轨道 | trait 形状等第一条真实执行命令落地时定形（既有决策不变）；写用户 repo 属对 repo 的变更，落地时 SHALL 有 write protection / 路径包含性检查 / 审计 |
| workspace 读写 db | db | `infra/store` 的 workspace 维度 | 落盘策略见 desktop-data-dimensions 能力 |
| 图数据库 | db | `infra/graph`（暂不建） | 暂不考虑；关系查询真实痛点出现再启，届时按 db 边界新成员落位 |
| 后端 api | api | `infra/api`（不建） | 先占位留痕：用途未定，MUST NOT 建目录、MUST NOT 动代码 |
| agent 执行 | agent | `core/agent`（契约）+ `infra/agent`（实现，裸名 `agent-cli`） | 已由 desktop-agent-execution 落地：三租户抽象 MVP 仅 CLI；进程 spawn 属 agent 边界实现细节，非 shell 边界 |

`infra/fs` SHALL 在文件边界长出独立适配（区别于 workflow 内纯读逻辑的、可复用的 fs 基础设施）时再立，不预建。

#### Scenario: 无预建目录

- **WHEN** 检查 `crates/infra/` 目录树
- **THEN** 仅 `store` 与 `agent` 两个成员（后者 crate 裸名 `agent-cli`），不存在 `infra/graph` / `infra/api` / `infra/fs` 空目录

#### Scenario: 图数据库与 api 决策留痕

- **WHEN** 查阅本能力 spec
- **THEN** 图数据库"暂不考虑、痛点驱动再启"与 api "先占位留痕"两条裁决可考，后续变更无需重新辩论是否引入

#### Scenario: exec 轨道由 agent 执行开通

- **WHEN** 检查 `commands/exec/`
- **THEN** 轨道已由 agent 执行命令开通（`agent_start` / `agent_runs` / `agent_run_events`，见 desktop-agent-execution）
- **AND** workspace 写文件仍将落此轨道而非 queries / workspaces 轨道，空壳 Executor trait 纪律不变

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent`（新，裸名 `agent`） | agent 域中立契约 | 零 Tauri、零 spawn、零 workspace 内依赖；不认识 claude |
| `crates/infra/agent`（新，crate 裸名 `agent-cli`） | agent 边界实现 | 仅依赖 `agent`；允许进程 spawn；禁 Tauri |
| 依赖图增边 | crate 图机械保证 | `desktop-app → agent + agent-cli`；`agent-cli → agent`；core 三 crate 均不依赖 store |
| 五类边界分类学 | 落位先声明边界 | shell（仅 desktop-app，runner 进程管理除外）/ db / 文件 / api（占位）/ agent（core 契约 + infra 实现） |
| 租户归位表 agent 行 | 特性 → 归位映射 | agent 执行 → `core/agent` + `infra/agent`（已落地）；graph / api / fs 仍不预建 |
