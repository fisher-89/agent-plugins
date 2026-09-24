# desktop-crate-layout 变更提案

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
│   ├── store/                 # crate: native_db 本地库（db 边界第一成员，底层 redb）
│   └── agent/                 # 目录：agent 边界实现（crate 裸名 agent-cli，唯一性约束）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → store`，且 `desktop-app → agent + agent-cli`、`agent-cli → agent`、`store → agent`（仅纯类型嵌装载荷）。`foundation` MUST NOT 依赖任何其他 crate；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 行为，SHALL 仅依赖 `agent` 的纯类型（`AgentEvent` 等 derive-free 类型）作嵌装载荷（store 自身持有模型与版本治理，行为自含纪律不变），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；`agent` MUST NOT 依赖 workspace 内任何 crate；`agent-cli` SHALL 仅依赖 `agent`，MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。agent 两 crate 沿此规则取裸名 `agent`（core）与 `agent-cli`（infra，因唯一性不得复用 `agent`）。

规则修订留痕：「store MUST NOT 依赖 core 任何 crate（自含模型）」原为其模型平凡前提下的局部特例；infra → core 依赖已有先例（`agent-cli → agent`），且 store 的事件流类型化建模（native-db-store-upgrade）需要 `agent::AgentEvent` 作嵌装载荷，故修订为「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`foundation`、`store`、`agent`、`agent-cli`，`workflow` 依赖 `foundation`，`agent-cli` 依赖 `agent`，`store` 依赖 `agent`
- **AND** `foundation` 无 workspace 内依赖；`store` 的 workspace 内依赖仅 `agent` 且无 Tauri 依赖；`agent` 无 workspace 内依赖；`agent-cli` 与 `agent` 均无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力，不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具

#### Scenario: store 落位 infra 且 core 依赖不到 store

- **WHEN** 检查 `store` 与 core 各 crate 的 `Cargo.toml` 及目录落位
- **THEN** `store` 落位 `crates/infra/store`，core 三个 crate（foundation / workflow / agent）的依赖中无 `store`
- **AND** `store` 不出现 Tauri 依赖

#### Scenario: store 嵌装仅纯类型

- **WHEN** 审查 store 对 `agent` 的使用点
- **THEN** 仅 `AgentEvent` 等纯类型作嵌装载荷（derive-free 原样引用），无 agent 域行为（runner / 状态机）被 store 调用

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力（desktop-agent-execution）扩展为五类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app（通用 shell 执行仅 desktop-app；runner 内部的子进程管理属 agent 边界实现细节，不属于 shell 边界）；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-cli`）；允许进程 spawn，禁 Tauri。

领域核心（change 域读模型、agent 域契约）与应用编排（现为 command）位于各边界之内：core 三 crate 纯依赖；`store` 不依赖 core 行为（仅依赖 `agent` 纯类型作嵌装载荷，规则修订见「crate 三层分组与依赖规则」），`agent-cli` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证（既有 requirement 不变）。`infra/` 目录表达 db / api / agent 等边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent 五类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: shell 依赖不越界

- **WHEN** 检查 core 与 infra 各 crate（含 agent-cli）的依赖声明
- **THEN** 无任何 Tauri 系依赖（tauri、tauri-plugin-* 等），shell 依赖仅存在于 desktop-app

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store` | native_db 本地库（db 边界第一成员，底层 redb） | 行为自含、依赖 `agent` 纯类型作嵌装载荷；零 Tauri 依赖；db 路径由 desktop-app 注入 |
| `crates/core/agent` | agent 域中立契约 | 零 Tauri、零 spawn、零 workspace 内依赖、derive-free；被 `agent-cli` 与 `store` 双向消费（契约 / 嵌装载荷） |
| `store → agent` 依赖（新） | 依赖规则修订落痕 | 「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」；修订理由与先例（agent-cli → agent）落痕于「crate 三层分组与依赖规则」 |
