# desktop-crate-layout Specification

## MODIFIED Requirements

### Requirement: crate 三层分组与依赖规则

Rust 侧 SHALL 按 cargo workspace 组织为三层分组（core 引擎侧 / infra 基础设施侧 / desktop-app 壳）：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析（刻意极小）
│   ├── config/                # crate: 工作区配置（读 + 校验 + 默认值，核心配置出口）
│   ├── workflow/              # crate: change 域（model / parse / queries / artifacts）
│   └── agent/                 # crate: agent 域中立契约（AgentEvent / AgentRunner / 状态机）
├── infra/                     # 基础设施侧命名空间（纯目录，不是 crate）
│   ├── store/                 # crate: native_db 本地库（db 边界第一成员，底层 redb）
│   └── agent/                 # 目录：agent 边界实现（crate 裸名 agent-cli，唯一性约束）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → config → foundation`，且 `desktop-app → store`，且 `desktop-app → agent + agent-cli`、`agent-cli → agent`、`store → agent`（仅纯类型嵌装载荷）。`foundation` MUST NOT 依赖任何其他 crate；`config` SHALL 仅依赖 `foundation`，MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 行为，SHALL 仅依赖 `agent` 的纯类型（`AgentEvent` 等 derive-free 类型）作嵌装载荷（store 自身持有模型与版本治理，行为自含纪律不变），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；`agent` MUST NOT 依赖 workspace 内任何 crate；`agent-cli` SHALL 仅依赖 `agent`，MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

**工作区配置唯一出口**（desktop-workspace-config 落地）：读取并校验工作区 config.json 属核心功能——`config` crate 是工作区配置的唯一合法出口，desktop-app 及未来任何需要配置的模块（`workflow` 等其他 core/infra crate 出现真实消费需求时）SHALL 经 `config` crate 获取配置项，MUST NOT 自行读取 config.json（由「配置文件名字面量隔离」requirement 机械保证）。本变更不预铺任何 `→ config` 消费依赖：首个真实消费者出现才立该边。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `config` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。agent 两 crate 沿此规则取裸名 `agent`（core）与 `agent-cli`（infra，因唯一性不得复用 `agent`）。

规则修订留痕：「store MUST NOT 依赖 core 任何 crate（自含模型）」原为其模型平凡前提下的局部特例；infra → core 依赖已有先例（`agent-cli → agent`），且 store 的事件流类型化建模（native-db-store-upgrade）需要 `agent::AgentEvent` 作嵌装载荷，故修订为「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`config`、`foundation`、`store`、`agent`、`agent-cli`，`workflow` 依赖 `foundation`，`config` 仅依赖 `foundation`，`agent-cli` 依赖 `agent`，`store` 依赖 `agent`
- **AND** `foundation` 无 workspace 内依赖；`config` 的 workspace 内依赖仅 `foundation` 且无 Tauri 依赖；`store` 的 workspace 内依赖仅 `agent` 且无 Tauri 依赖；`agent` 无 workspace 内依赖；`agent-cli` 与 `agent` 均无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力（目录常量组与路径推导），不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具；配置语义不落 foundation（读 / 校验 / 默认值归 `config` crate）

#### Scenario: store 落位 infra 且 core 依赖不到 store

- **WHEN** 检查 `store` 与 core 各 crate 的 `Cargo.toml` 及目录落位
- **THEN** `store` 落位 `crates/infra/store`，core 各 crate（foundation / config / workflow / agent）的依赖中无 `store`
- **AND** `store` 不出现 Tauri 依赖

#### Scenario: store 嵌装仅纯类型

- **WHEN** 审查 store 对 `agent` 的使用点
- **THEN** 仅 `AgentEvent` 等纯类型作嵌装载荷（derive-free 原样引用），无 agent 域行为（runner / 状态机）被 store 调用

#### Scenario: config 落位 core 且依赖最小

- **WHEN** 检查 `config` crate 的目录落位与 `Cargo.toml`
- **THEN** `config` 落位 `crates/core/config`，依赖仅 `foundation` + serde 系（serde / serde_json）+ specta，无 Tauri、无 workflow / agent / infra 依赖
- **AND** crate 图中不存在任何「既有 crate → config」边（首个真实消费者出现才立）

## ADDED Requirements

### Requirement: 配置文件名字面量隔离

`packages/desktop` 的产品源码（全部 `.rs` 文件，排除 `*_test.rs`）MUST NOT 出现 `config.json` 文件名字面量；唯一合法触点为 `foundation/src/layout.rs`（常量组，见 workspace-layout-resolution）。config.json 路径解析 SHALL 只经 `foundation::layout` 提供的能力取得——绕开 `config` crate 直接读配置文件的代码连文件名都拼不出。执法 SHALL 由 `layout_test.rs` 的命名隔离扫描扩展承载：`config.json` 禁令与既有 `openspec` 目录名禁令并列执行（大小写敏感全文匹配，含注释；产品源码全扫、`*_test.rs` 排除、layout.rs 唯一例外 + 在场断言）。

标识符与 DTO 命名沿用「代码命名隔离 openspec 字样」requirement：配置类型 SHALL 取 `WorkspaceConfig` / `ConfigReport` 类命名，MUST NOT 取 `openspec_config` 类命名。

#### Scenario: 扫描双禁令并列

- **WHEN** 运行 `layout_test` 命名隔离扫描
- **THEN** `openspec` 与 `config.json` 双字面量在产品源码中均仅出现于 layout.rs，任一他处出现即 panic 指认违例文件，且 layout.rs 在场断言通过

#### Scenario: 注释与字符串同受禁

- **WHEN** 在非 layout.rs 产品源码的注释或字符串中写 `config.json` 字样
- **THEN** 扫描同样 panic（全文匹配口径与 openspec 禁令一致），禁令无注释豁免

#### Scenario: 配置类型命名合规

- **WHEN** 审查 `config` crate 与命令轨的 DTO 命名
- **THEN** 无标识符含 `openspec` 子串，配置类型为 `WorkspaceConfig` 类命名

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/config`（新，裸名 `config`） | 工作区配置唯一合法出口（读 + 校验 + 默认值） | 仅依赖 `foundation` + serde 系 + specta；禁 Tauri；未来模块取配置仅准经此，MUST NOT 自行读 config.json；首个真实消费者出现才立 `→ config` 边 |
| `layout_test.rs` 命名隔离扫描（扩展） | 字面量执法不变量 | `openspec` 目录名 + `config.json` 文件名双禁令；唯一触点 layout.rs 常量组；`*_test.rs` 排除、全文匹配含注释 |
