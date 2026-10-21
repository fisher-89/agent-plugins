# desktop-crate-layout Specification（delta: exec-run-shell）

## MODIFIED Requirements

### Requirement: crate 三层分组与依赖规则

Rust 侧 SHALL 按 cargo workspace 组织为三层分组（core 引擎侧 / infra 基础设施侧 / desktop-app 壳）：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析 + workspace 身份段派生 + 取消原语（刻意极小）
│   ├── config/                # crate: 工作区配置（读 + 校验 + 默认值，核心配置出口）
│   ├── workflow/              # crate: change 域（model / parse / queries / artifacts）
│   └── agent/                 # crate: agent 域中立契约（AgentEvent / AgentRunner / 状态机）
├── infra/                     # 基础设施侧命名空间（纯目录，不是 crate）
│   ├── store/                 # crate: native_db 本地库（db 边界第一成员，底层 redb）
│   └── agent/                 # 目录：agent 边界实现（crate 裸名 agent-runtime，唯一性约束）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → config → foundation`，且 `desktop-app → store`，且 `desktop-app → agent + agent-runtime`、`agent-runtime → agent`、`store → agent`（仅纯类型嵌装载荷）+ `store → foundation`（仅 workspace 身份段派生纯函数，desktop-change-worktree 引入），且 `desktop-app → vcs-runtime`（壳层装配注入 WorktreePort 实现）、`vcs-runtime → workflow`（WorktreePort port 类型）+ `foundation`（身份段派生）。`foundation` MUST NOT 依赖任何其他 crate（含 MUST NOT 依赖 tokio——取消原语 `cancel` 为 std-only 实现，见下）；`config` SHALL 仅依赖 `foundation`，MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 行为，SHALL 仅依赖 `agent` 的纯类型（`AgentEvent` 等 derive-free 类型）作嵌装载荷与 `foundation` 的身份段派生纯函数（store 自身持有模型与版本治理，行为自含纪律不变），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；`agent` MUST NOT 依赖 workspace 内任何 crate；`agent-runtime` SHALL 仅依赖 `agent`，MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；`vcs-runtime` SHALL 仅依赖 `workflow`（port 类型）与 `foundation`，MUST NOT 依赖 Tauri（进程 spawn 是 vcs 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

**工作区配置唯一出口**（desktop-workspace-config 落地）：读取并校验工作区 config.json 属核心功能——`config` crate 是工作区配置的唯一合法出口，desktop-app 及未来任何需要配置的模块（`workflow` 等其他 core/infra crate 出现真实消费需求时）SHALL 经 `config` crate 获取配置项，MUST NOT 自行读取 config.json（由「配置文件名字面量隔离」requirement 机械保证）。本变更不预铺任何 `→ config` 消费依赖：首个真实消费者出现才立该边。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `config` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。agent 两 crate 沿此规则取裸名 `agent`（core）与 `agent-runtime`（infra，因唯一性不得复用 `agent`；原裸名 `agent-cli`，升格引擎门面时随 desktop-agent-execution 改名）。

规则修订留痕：「store MUST NOT 依赖 core 任何 crate（自含模型）」原为其模型平凡前提下的局部特例；infra → core 依赖已有先例（`agent-runtime → agent`），且 store 的事件流类型化建模（native-db-store-upgrade）需要 `agent::AgentEvent` 作嵌装载荷，故修订为「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」。第二例修订（desktop-change-worktree）：store 依赖白名单增 `foundation` 身份段派生纯函数——db 文件名与 worktree 子目录同源消费 foundation 单点（store 内不残留第二份派生实现），口径仍为「禁 core 行为、准 core 纯类型 / 纯函数」，依赖面不扩大到任何 core 行为 crate。第三例下沉（exec-run-shell）：`foundation` 增取消原语 `cancel`（`CancelFlag` / `CancelHandle`）——两消费者同时到场（编排侧 `RunGuard` 写、`exec::run_shell` 读），属「第二个消费者出现才下沉」的正触发；实现 std-only（`Arc` + `AtomicBool`），不引 tokio、不引 workspace 内 crate，故本 requirement 的依赖方向与分层结论不变。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`config`、`foundation`、`store`、`agent`、`agent-runtime`、`vcs-runtime`，`workflow` 依赖 `foundation`，`config` 仅依赖 `foundation`，`agent-runtime` 依赖 `agent`，`store` 依赖 `agent` + `foundation`，`vcs-runtime` 依赖 `workflow` + `foundation`
- **AND** `foundation` 无 workspace 内依赖；`config` 的 workspace 内依赖仅 `foundation` 且无 Tauri 依赖；`store` 的 workspace 内依赖为 `agent` + `foundation`（纯类型与纯函数，零行为）且无 Tauri 依赖；`agent` 无 workspace 内依赖；`agent-runtime` 与 `vcs-runtime` 均无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力（目录常量组与路径推导）、workspace 身份段派生纯函数（desktop-change-worktree 引入的第二消费者既成下沉：db 文件名与 worktree 子目录同源单点）与取消原语 `cancel`（exec-run-shell 引入的第三消费者正触发下沉：`RunGuard` 写 / `exec::run_shell` 读，std-only 零 tokio），不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具；配置语义不落 foundation（读 / 校验 / 默认值归 `config` crate）

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

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/foundation` | 共享地基（极小纪律） | 新增能力第三项 = 取消原语 `cancel`（`CancelFlag` / `CancelHandle`）；无 workspace 内依赖、无 tokio；其余（layout / 身份段派生）零改动 |
| `crates/core/foundation/src/cancel.rs`（新） | 取消原语 | `CancelFlag::{never, is_set}`（`Clone` 共享同一原子）、`CancelHandle::set`；std::sync::atomic + Arc |
| `crates/infra/exec` | 消费方（读） | `run_shell` 以 200 ms 轮询 `CancelFlag::is_set()`；crate 依赖仍为 `foundation` + tokio（特性面 `process` / `io-util` / `time` / `macros`），零新增依赖 |
| `crates/core/orchestration` | 消费方（写） | `RunGuard::cancel_flag()`、`request_stop` 双写；core→core 依赖不变（`orchestration → foundation` 既有边） |

**API 接口**：无。
**CLI 命令**：无。
**前端组件**：无。
