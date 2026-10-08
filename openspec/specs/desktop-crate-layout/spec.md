# desktop-crate-layout Specification

## Purpose

约束 `packages/desktop` 桌面端包的落位方式与 Rust crate 分层组织：作为完全独立的包存在于仓库中，cargo workspace 按三层分组组织依赖方向，并在代码命名上隔离 `openspec` 字样、将磁盘路径收口到 foundation 的 layout 解析器。

## Requirements

### Requirement: packages/desktop 完全独立包落位

桌面端 SHALL 落位为 `packages/desktop`，作为完全独立的包存在：自带 lockfile 与构建脚本，MUST NOT 引入仓库根 `package.json` 或 `pnpm-workspace.yaml`，MUST NOT 进入 `dist/` 插件分发链。CLAUDE.md「改插件源码后 bump `plugins/<name>/package.json` version 并 rebuild 产物」规则 SHALL NOT 适用于 `packages/desktop`。

#### Scenario: 独立包不污染主仓

- **WHEN** 检查仓库根目录与 `dist/`
- **THEN** 根目录无新增 `package.json` / `pnpm-workspace.yaml`
- **AND** `dist/` 不含 desktop 产物

#### Scenario: 不受插件 bump 规则约束

- **WHEN** `packages/desktop` 源码变更
- **THEN** 无任何 `plugins/<name>/package.json` 版本 bump 或 rebuild 步骤被要求

### Requirement: crate 三层分组与依赖规则

Rust 侧 SHALL 按 cargo workspace 组织为三层分组（core 引擎侧 / infra 基础设施侧 / desktop-app 壳）：

```
crates/
├── core/                      # 引擎侧命名空间（纯目录，不是 crate）
│   ├── foundation/            # crate: layout 解析 + workspace 身份段派生（刻意极小）
│   ├── config/                # crate: 工作区配置（读 + 校验 + 默认值，核心配置出口）
│   ├── workflow/              # crate: change 域（model / parse / queries / artifacts）
│   └── agent/                 # crate: agent 域中立契约（AgentEvent / AgentRunner / 状态机）
├── infra/                     # 基础设施侧命名空间（纯目录，不是 crate）
│   ├── store/                 # crate: native_db 本地库（db 边界第一成员，底层 redb）
│   └── agent/                 # 目录：agent 边界实现（crate 裸名 agent-runtime，唯一性约束）
└── desktop-app/               # crate: Tauri 壳
```

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → config → foundation`，且 `desktop-app → store`，且 `desktop-app → agent + agent-runtime`、`agent-runtime → agent`、`store → agent`（仅纯类型嵌装载荷）+ `store → foundation`（仅 workspace 身份段派生纯函数，desktop-change-worktree 引入），且 `desktop-app → vcs-runtime`（壳层装配注入 WorktreePort 实现）、`vcs-runtime → workflow`（WorktreePort port 类型）+ `foundation`（身份段派生）。`foundation` MUST NOT 依赖任何其他 crate；`config` SHALL 仅依赖 `foundation`，MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 行为，SHALL 仅依赖 `agent` 的纯类型（`AgentEvent` 等 derive-free 类型）作嵌装载荷与 `foundation` 的身份段派生纯函数（store 自身持有模型与版本治理，行为自含纪律不变），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；`agent` MUST NOT 依赖 workspace 内任何 crate；`agent-runtime` SHALL 仅依赖 `agent`，MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；`vcs-runtime` SHALL 仅依赖 `workflow`（port 类型）与 `foundation`，MUST NOT 依赖 Tauri（进程 spawn 是 vcs 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

**工作区配置唯一出口**（desktop-workspace-config 落地）：读取并校验工作区 config.json 属核心功能——`config` crate 是工作区配置的唯一合法出口，desktop-app 及未来任何需要配置的模块（`workflow` 等其他 core/infra crate 出现真实消费需求时）SHALL 经 `config` crate 获取配置项，MUST NOT 自行读取 config.json（由「配置文件名字面量隔离」requirement 机械保证）。本变更不预铺任何 `→ config` 消费依赖：首个真实消费者出现才立该边。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `config` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。agent 两 crate 沿此规则取裸名 `agent`（core）与 `agent-runtime`（infra，因唯一性不得复用 `agent`；原裸名 `agent-cli`，升格引擎门面时随 desktop-agent-execution 改名）。

规则修订留痕：「store MUST NOT 依赖 core 任何 crate（自含模型）」原为其模型平凡前提下的局部特例；infra → core 依赖已有先例（`agent-runtime → agent`），且 store 的事件流类型化建模（native-db-store-upgrade）需要 `agent::AgentEvent` 作嵌装载荷，故修订为「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」。第二例修订（desktop-change-worktree）：store 依赖白名单增 `foundation` 身份段派生纯函数——db 文件名与 worktree 子目录同源消费 foundation 单点（store 内不残留第二份派生实现），口径仍为「禁 core 行为、准 core 纯类型 / 纯函数」，依赖面不扩大到任何 core 行为 crate。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`config`、`foundation`、`store`、`agent`、`agent-runtime`、`vcs-runtime`，`workflow` 依赖 `foundation`，`config` 仅依赖 `foundation`，`agent-runtime` 依赖 `agent`，`store` 依赖 `agent` + `foundation`，`vcs-runtime` 依赖 `workflow` + `foundation`
- **AND** `foundation` 无 workspace 内依赖；`config` 的 workspace 内依赖仅 `foundation` 且无 Tauri 依赖；`store` 的 workspace 内依赖为 `agent` + `foundation`（纯类型与纯函数，零行为）且无 Tauri 依赖；`agent` 无 workspace 内依赖；`agent-runtime` 与 `vcs-runtime` 均无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力（目录常量组与路径推导）与 workspace 身份段派生纯函数（desktop-change-worktree 引入的第二消费者既成下沉：db 文件名与 worktree 子目录同源单点），不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具；配置语义不落 foundation（读 / 校验 / 默认值归 `config` crate）

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

### Requirement: 代码命名隔离 openspec 字样

`packages/desktop` 内的 crate 名、模块名、类型名、函数名与 DTO 字段 SHALL NOT 含 `openspec` 字样（该名称未来要改，代码概念上此目录树是 change 域而非 openspec 域）。磁盘真实路径（当前为 `openspec/changes/`）SHALL 全部收进 foundation 的 layout 解析器，除 `resolve` 一处外，desktop 源码 MUST NOT 出现硬编码的该路径字符串。

#### Scenario: 命名扫描通过

- **WHEN** 对 `packages/desktop` 源码做标识符扫描（crate 名、模块名、类型名）
- **THEN** 无标识符含 `openspec` 子串

#### Scenario: 路径硬编码唯一触点

- **WHEN** 在 `packages/desktop` 源码中搜索当前磁盘目录名字符串
- **THEN** 仅 foundation 的 `resolve` 实现处出现，model / parse / queries / 前端均经 `Layout` 结构取路径

### Requirement: 配置文件名字面量隔离

`packages/desktop` 的产品源码（全部 `.rs` 文件，排除 `*_test.rs`）MUST NOT 出现 `config.json` 文件名字面量；唯一合法触点为 `foundation/src/layout/mod.rs`（常量组，见 workspace-layout-resolution）。config.json 路径解析 SHALL 只经 `foundation::layout` 提供的能力取得——绕开 `config` crate 直接读配置文件的代码连文件名都拼不出。执法 SHALL 由 `mod_test.rs` 的命名隔离扫描扩展承载：`config.json` 禁令与既有 `openspec` 目录名禁令并列执行（大小写敏感全文匹配，含注释；产品源码全扫、`*_test.rs` 排除、layout/mod.rs 唯一例外 + 在场断言）。

标识符与 DTO 命名沿用「代码命名隔离 openspec 字样」requirement：配置类型 SHALL 取 `WorkspaceConfig` / `ConfigReport` 类命名，MUST NOT 取 `openspec_config` 类命名。

#### Scenario: 扫描双禁令并列

- **WHEN** 运行 `mod_test` 命名隔离扫描
- **THEN** `openspec` 与 `config.json` 双字面量在产品源码中均仅出现于 layout/mod.rs，任一他处出现即 panic 指认违例文件，且 layout/mod.rs 在场断言通过

#### Scenario: 注释与字符串同受禁

- **WHEN** 在非 layout/mod.rs 产品源码的注释或字符串中写 `config.json` 字样
- **THEN** 扫描同样 panic（全文匹配口径与 openspec 禁令一致），禁令无注释豁免

#### Scenario: 配置类型命名合规

- **WHEN** 审查 `config` crate 与命令轨的 DTO 命名
- **THEN** 无标识符含 `openspec` 子串，配置类型为 `WorkspaceConfig` 类命名

### Requirement: 四类边界分类学

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力（desktop-agent-execution）扩展为五类，再经检查域（desktop-checks-domain）扩展为六类，再经 change worktree（desktop-change-worktree）扩展为七类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app（通用 shell 执行仅 desktop-app；runner 内部的子进程管理属 agent 边界实现细节，不属于 shell 边界）；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员（落位 `infra/` 下新 crate）；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-runtime`）；允许进程 spawn，禁 Tauri；
- **checks 边界**：外部进程 + pass/fail 结论门禁的检查域（现有成员：static-check、test-execution）——纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；允许进程 spawn，禁 Tauri；runner port 留 `core/orchestration`（port 属于消费者，spawn 不进 core）；
- **vcs 边界**：外部进程 + 版本控制工作面（git worktree 建域 / 脏仓探测 / 依赖 bootstrap 安装 / 归档提交与主仓合入的进程执行——desktop-archive-change 扩族）——port 定义落位 `crates/core/workflow`（port 属于消费者，spawn 不进 core；归档链消费的 port 缝归属由该变更 design 定稿，允许落 `core/orchestration`），实现落位 `crates/infra/vcs`（crate 裸名 `vcs-runtime`）；允许进程 spawn，禁 Tauri。

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
| git 工作面（worktree 建域 / 脏仓探测 / 依赖 bootstrap / 归档提交与主仓合入） | vcs | `core/workflow`（WorktreePort）+ `infra/vcs`（实现，裸名 `vcs-runtime`） | 已由 desktop-change-worktree 落地：port 属消费者、spawn 不进 core；归档提交 / 合入由 desktop-archive-change 扩族（port 缝归属由其 design 定稿，实现不越 `infra/vcs`） |

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop`（包） | 独立桌面应用 | 自带 lockfile / 构建脚本；不进根 workspace、不进 `dist/` |
| `crates/core/foundation` | 共享地基（今天仅 layout） | 无 workspace 内依赖；第二消费者出现才下沉新能力 |
| `crates/core/workflow` | change 域纯读库 | 依赖 foundation；零 Tauri 依赖；无指令概念；不依赖 store |
| `crates/core/agent`（裸名 `agent`） | agent 域中立契约 | 零 Tauri、零 spawn、零 workspace 内依赖、derive-free；不认识 claude；被 `agent-runtime` 与 `store` 双向消费（契约 / 嵌装载荷） |
| `crates/infra/store` | native_db 本地库（db 边界第一成员，底层 redb） | 行为自含、依赖 `agent` 纯类型作嵌装载荷；零 Tauri 依赖；db 路径由 desktop-app 注入 |
| `store → agent` 依赖（新） | 依赖规则修订落痕 | 「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」；修订理由与先例（agent-runtime → agent）落痕于「crate 三层分组与依赖规则」 |
| `crates/infra/agent`（裸名 `agent-runtime`，收窄） | agent 边界实现 | 仅承载会话租户身份（cli / sdk / worker / compose / store_port）；static_check 移出 checks 边界；允许进程 spawn；禁 Tauri |
| `crates/desktop-app` | Tauri 壳 | 依赖 workflow + foundation + store + agent + agent-runtime；command 薄包装 |
| 七类边界分类学 | 落位先声明边界 | shell（仅 desktop-app，runner 进程管理除外）/ db（infra 成员）/ 文件（workspace 读写）/ api（占位留痕）/ agent（core 契约 + infra 实现）/ checks（core/checks 纯层 + infra/checks 执行）/ vcs（core/workflow port + infra/vcs 执行）；core 与 infra 禁 Tauri |
| `crates/core/checks` + `crates/infra/checks`（新，checks 边界首批成员） | 检查域双层落位 | 依赖：`checks` → config + foundation；`checks-runtime` → orchestration + checks + config + foundation；纯层零 spawn、执行层允许 spawn；两 crate 均零 Tauri（详见 desktop-checks-domain） |
| 未来租户归位表 | 特性 → 归位映射 | 写文件 → exec 轨道；读写 db → store workspace 维度；图数据库 → 暂不考虑；api → 不建目录；agent 执行 → core/agent + infra/agent（已落地） |
| `crates/infra/vcs`（新，裸名 `vcs-runtime`，vcs 边界首成员） | git 工作面进程执行 | 依赖：`vcs-runtime` → workflow（WorktreePort 类型）+ foundation；允许 spawn；禁 Tauri；由 desktop-app 装配注入（`desktop-app → vcs-runtime`） |
| `store → foundation` 依赖（新） | 依赖规则第二例修订落痕 | 仅身份段派生纯函数（db 文件名与 worktree 子目录同源单点，store 内零第二份派生实现）；store 行为自含、零 Tauri 不变 |
| `crates/infra/` | 目录表达计划 | store / agent（裸名 agent-runtime）/ checks / watch / vcs 五个已落地成员；graph / api / fs 均不预建，成员出现才立 |
| `crates/core/config`（新，裸名 `config`） | 工作区配置唯一合法出口（读 + 校验 + 默认值） | 仅依赖 `foundation` + serde 系 + specta；禁 Tauri；未来模块取配置仅准经此，MUST NOT 自行读 config.json；首个真实消费者出现才立 `→ config` 边 |
| `mod_test.rs` 命名隔离扫描（扩展） | 字面量执法不变量 | `openspec` 目录名 + `config.json` 文件名双禁令；唯一触点 layout/mod.rs 常量组；`*_test.rs` 排除、全文匹配含注释 |
