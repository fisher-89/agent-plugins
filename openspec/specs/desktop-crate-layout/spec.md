# desktop-crate-layout Specification

## Purpose

约束 `packages/desktop` 桌面端包的落位方式与 Rust crate 分层组织：作为完全独立的包存在于仓库中，cargo workspace 按三层分组组织依赖方向，并在代码命名上隔离 `openspec` 字样、将磁盘路径收口到 foundation 的 layout 解析器；工具 crate（`export-bindings` 与模块 API 契约生成器 `archi-api-gen`）同样登记落位、依赖方向、允许写盘的理由与边界归类。

## Requirements

### Requirement: packages/desktop 完全独立包落位

桌面端 SHALL 落位为 `packages/desktop`，作为完全独立的包存在：自带 lockfile 与构建脚本，MUST NOT 引入仓库根 `package.json` 或 `pnpm-workspace.yaml`，MUST NOT 进入 `dist/` 分发链（`dist/` 的三份产物目录已随插件退役整体删除，仓库自此只有这一个包）。仓库根曾有的「改插件源码后 bump 插件 `package.json` version 并 rebuild 产物」规则随插件退役整条消失；`packages/desktop` 的版本 bump SHALL 只由「版本交付纪律」（见 desktop-release-pipeline）承载，MUST NOT 由任何产物重建步骤驱动、MUST NOT 要求重建第二份产物。

#### Scenario: 独立包不污染主仓

- **WHEN** 检查仓库根目录与 `dist/`
- **THEN** 根目录无新增 `package.json` / `pnpm-workspace.yaml`
- **AND** `dist/` 不含 desktop 产物

#### Scenario: 版本 bump 只走桌面发版纪律

- **WHEN** `packages/desktop` 源码变更
- **THEN** 只需 `packages/desktop/package.json` patch 版本 +1（`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 不随动）
- **AND** 无任何第三方产物 bump 或 rebuild 步骤被要求（仓库内不存在第二个可分发包）

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

依赖规则 SHALL 为：`desktop-app → workflow → foundation`，且 `desktop-app → config → foundation`，且 `desktop-app → store`，且 `desktop-app → agent + agent-runtime`、`agent-runtime → agent`、`store → agent`（仅纯类型嵌装载荷）+ `store → foundation`（仅 workspace 身份段派生纯函数，desktop-change-worktree 引入），且 `desktop-app → vcs-runtime`（壳层装配注入 WorktreePort 实现）、`vcs-runtime → workflow`（WorktreePort port 类型；`foundation` 仅为测试依赖——取消原语夹具）。`foundation` MUST NOT 依赖任何其他 crate（含 MUST NOT 依赖 tokio——取消原语 `cancel` 为 std-only 实现，见下）；`config` SHALL 仅依赖 `foundation`，MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；`workflow` MUST NOT 依赖 `desktop-app`；`store` MUST NOT 依赖 core 行为，SHALL 仅依赖 `agent` 的纯类型（`AgentEvent` 等 derive-free 类型）作嵌装载荷与 `foundation` 的身份段派生纯函数（store 自身持有模型与版本治理，行为自含纪律不变），MUST NOT 依赖 Tauri（Tauri 只属于 desktop-app）；`agent` MUST NOT 依赖 workspace 内任何 crate；`agent-runtime` SHALL 仅依赖 `agent`，MUST NOT 依赖 Tauri（进程 spawn 是 agent 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；`vcs-runtime` SHALL 仅依赖 `workflow`（port 类型），MUST NOT 依赖 Tauri（进程 spawn 是 vcs 边界自身的实现细节，允许 spawn，Tauri 仍然只属于 desktop-app）；core 任何 crate MUST NOT 依赖 `store`（由 crate 图机械保证有状态持久化进不了纯读域）。未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖；未来 db 边界新成员（如图数据库）SHALL 落位 `infra/` 下新 crate。

**工作区配置唯一出口**（desktop-workspace-config 落地）：读取并校验工作区 config.json 属核心功能——`config` crate 是工作区配置的唯一合法出口，desktop-app 及未来任何需要配置的模块（`workflow` 等其他 core/infra crate 出现真实消费需求时）SHALL 经 `config` crate 获取配置项，MUST NOT 自行读取 config.json（由「配置文件名字面量隔离」requirement 机械保证）。本变更不预铺任何 `→ config` 消费依赖：首个真实消费者出现才立该边。

`core/` 与 `infra/` SHALL 仅作目录名，内部 crate 使用 `foundation` / `config` / `workflow` / `archi` / `store` 等裸名（workspace 内唯一即可），MUST NOT 存在名为 `core` 的 crate（与 Rust 内置 core 撞名）。agent 两 crate 沿此规则取裸名 `agent`（core）与 `agent-runtime`（infra，因唯一性不得复用 `agent`；原裸名 `agent-cli`，升格引擎门面时随 desktop-agent-execution 改名）。

规则修订留痕：「store MUST NOT 依赖 core 任何 crate（自含模型）」原为其模型平凡前提下的局部特例；infra → core 依赖已有先例（`agent-runtime → agent`），且 store 的事件流类型化建模（native-db-store-upgrade）需要 `agent::AgentEvent` 作嵌装载荷，故修订为「禁依赖 core 行为，可依赖 core 纯类型作嵌装载荷」。第二例修订（desktop-change-worktree）：store 依赖白名单增 `foundation` 身份段派生纯函数——db 文件名消费 foundation 单点（store 内不残留第二份派生实现），口径仍为「禁 core 行为、准 core 纯类型 / 纯函数」，依赖面不扩大到任何 core 行为 crate。第三例下沉（exec-run-shell）：`foundation` 增取消原语 `cancel`（`CancelFlag` / `CancelHandle`）——两消费者同时到场（编排侧 `RunGuard` 写、`exec::run_shell` 读），属「第二个消费者出现才下沉」的正触发；实现 std-only（`Arc` + `AtomicBool`），不引 tokio、不引 workspace 内 crate，故本 requirement 的依赖方向与分层结论不变。第四例修订（worktree 落位 git 根同级）：worktree 落位改由 `vcs-runtime` 经 `git rev-parse --show-toplevel` 派生（`<git 根父目录>/.worktrees/<仓库目录名>/`），不再消费 foundation 身份段——身份段仅 workspace db 文件名消费；`vcs-runtime` 的 foundation 依赖降为 dev-dependencies（测试取消原语夹具），生产依赖仅 `workflow`。

#### Scenario: 依赖方向符合分层

- **WHEN** 检查各 crate 的 `Cargo.toml` 依赖声明
- **THEN** `desktop-app` 依赖 `workflow`、`config`、`foundation`、`store`、`agent`、`agent-runtime`、`vcs-runtime`，`workflow` 依赖 `foundation`，`config` 仅依赖 `foundation`，`agent-runtime` 依赖 `agent`，`store` 依赖 `agent` + `foundation`，`vcs-runtime` 依赖 `workflow`（`foundation` 仅 dev-dependencies）
- **AND** `foundation` 无 workspace 内依赖；`config` 的 workspace 内依赖仅 `foundation` 且无 Tauri 依赖；`store` 的 workspace 内依赖为 `agent` + `foundation`（纯类型与纯函数，零行为）且无 Tauri 依赖；`agent` 无 workspace 内依赖；`agent-runtime` 与 `vcs-runtime` 均无 Tauri 依赖

#### Scenario: foundation 保持极小

- **WHEN** 实现 `foundation` crate
- **THEN** 它只包含 layout 解析能力（目录常量组与路径推导）、workspace 身份段派生纯函数（仅 workspace db 文件名消费；worktree 落位已改为 git 根同级派生，不再同源）与取消原语 `cancel`（exec-run-shell 引入的第三消费者正触发下沉：`RunGuard` 写 / `exec::run_shell` 读，std-only 零 tokio），不预铺 fs 助手 / 错误类型等尚无第二个消费者的通用工具；配置语义不落 foundation（读 / 校验 / 默认值归 `config` crate）

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

Desktop 后端的外部依赖 SHALL 按边界分类组织，任何新特性落位时 SHALL 先声明所属边界。分类学经 agent 执行能力扩展为五类，再经检查域扩展为六类，再经 change worktree 扩展为七类：

- **shell 边界**：Tauri IPC、dialog、通知、单实例——只属于 desktop-app；
- **db 边界**：redb / native_db（现有 `crates/infra/store`）及未来数据库成员；
- **文件边界**：workspace 文件读（现有，经 workflow/foundation 纯读域）与未来 workspace 文件写；
- **api 边界**：未来后端服务（用途未定，先占位留痕）；
- **agent 边界**：外部执行 agent 的接入与运行（本机 CLI / 进程内 SDK / 远程 API 三租户）——契约落位 `core/agent`，实现落位 `infra/`（现有成员 `agent-runtime`）；允许进程 spawn，禁 Tauri；
- **checks 边界**：外部进程 + pass/fail 结论门禁的检查域（现有成员：static-check、test-execution、mutation-test）——纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；允许进程 spawn，禁 Tauri；runner port 留 `core/orchestration`；
- **vcs 边界**：外部进程 + 版本控制工作面（git worktree 建域 / 脏仓探测 / 依赖 bootstrap 安装 / archive 相位 git 步（commit / rebase / ff_merge / commit_paths）/ 变更文件清单（`change_edit_set`，突变范围唯一入口）的进程执行）——建域 port（`WorktreePort`）落位 `crates/core/workflow`（port 属于消费者，spawn 不进 core），归档 git 步 port（`ArchiveVcsPort`）落位 `crates/core/orchestration`（walker 第三执行缝），实现落位 `crates/infra/vcs`（crate 裸名 `vcs-runtime`）；允许进程 spawn，禁 Tauri。

领域核心与应用编排位于各边界之内：core 各 crate 纯依赖；`store` 不依赖 core 行为（仅依赖 `agent` 纯类型作嵌装载荷与 `foundation` 身份段派生纯函数），`agent-runtime` 仅依赖其契约 `core/agent`；infra 各 crate 均不依赖 Tauri，依赖方向由 crate 图机械保证。`infra/` 目录表达各边界的计划落位，但目录 MUST NOT 在首个成员出现前创建。

#### Scenario: 边界归类完备

- **WHEN** 评审任一新增特性（外部依赖）的落位设计
- **THEN** 其声明了 shell / db / 文件 / api / agent / checks / vcs 七类边界之一，并按对应规则归位；无特性落在分类之外

#### Scenario: 检查步归类 checks 边界

- **WHEN** 评审 static-check、test-execution 与 mutation-test 执行步的落位
- **THEN** 三者归 checks 边界：core/checks 纯层零 spawn 零 Tauri，infra/checks 执行层允许 spawn；检查域成员不出现在 infra/agent

#### Scenario: git 工作面归类 vcs 边界

- **WHEN** 评审 git worktree 建域 / 脏仓探测 / bootstrap 安装 / archive 相位 git 步 / 变更文件清单的落位
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

### Requirement: 平台条件依赖的落位例外（infra 的 Windows 系统 API 绑定）

infra crate 声明平台 API 绑定依赖时 SHALL 落 `[target.'<cfg>'.dependencies]` 条件段，MUST NOT 落无条件的 `[dependencies]`——使非目标平台（含 CI 的 Linux / macOS 构建）零新增编译依赖。首个消费者为 `crates/infra/exec` 的锁占用诊断面：

- `crates/infra/exec` SHALL 以 `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }` 引入 Restart Manager 绑定（占用者点名）；feature 面 MUST 为**按需最小集**（MUST NOT 引 `Win32` 全量 / MUST NOT 引与用途无关的 feature 族），且每个 feature MUST 在 `Cargo.toml` 注释其用途。
- 版本 SHALL 与 workspace 锁内既有版本一致（`0.61.2`），MUST NOT 为同一 crate 追加第二个直接依赖版本行。
- 精确锁探针的「DELETE 访问权 + 全共享打开」SHALL 用 std `std::os::windows::fs::OpenOptionsExt`（`access_mode` / `share_mode`）表达，MUST NOT 为访问权 / 共享模式常量单独引入 `Win32_Storage_FileSystem` / `Win32_Security` feature。
- 本例外 MUST NOT 改变既有分层与依赖方向结论：core 各 crate 仍零平台依赖、零 Tauri；`infra/exec` 的 workspace 内依赖仍仅 `foundation`；非 Windows 目标的依赖面与编译产物 MUST 与本变更前一致。

#### Scenario: 条件段落位审查

- **WHEN** 审查 `crates/infra/exec/Cargo.toml`
- **THEN** `windows-sys` 仅出现在 `[target.'cfg(windows)'.dependencies]` 段，feature 仅含 `Win32_System_RestartManager`，且该行有用途注释；无条件 `[dependencies]` 段无平台绑定 crate

#### Scenario: 非 Windows 零新增

- **WHEN** 以非 Windows 目标解析依赖（如 Linux CI）
- **THEN** 依赖图与编译输入不含 `windows-sys` 新增直接依赖，`crates/infra/exec` 的 crate 依赖仍为 `foundation` + tokio（特性面不变）

#### Scenario: 分层结论不变

- **WHEN** 复核 `desktop-crate-layout` 既有「依赖方向符合分层」与「core 零 Tauri」断言
- **THEN** core 各 crate 依赖面零变化、零 Tauri；`infra/exec` 不新增 workspace 内依赖边；平台绑定依赖不被任何 core crate 引入

### Requirement: glob 方言单点复用与 agent-runtime → checks 依赖边

`agent-runtime` SHALL 以 workspace 依赖 `checks = { workspace = true }` 引入一条 infra → core 纯层的依赖边，其**唯一用途**是复用 `checks::globmatch::match_glob` 作写入保护的规则匹配：

- `agent-runtime`（写入保护判定面）MUST NOT 自造第二份 glob 匹配实现——花括号展开、无通配模式的目录前缀语义、`**/` 零段可命中、反斜杠归一四项语义 SHALL 只存在于 `checks::globmatch` 一处（MUST NOT 在 protect 模块内直接以 `glob::Pattern` + 自定 `MatchOptions` 拼第二套方言）。
- `checks::globmatch` 的公开签名与语义 MUST NOT 变化：`match_glob(path, pattern) -> bool` 与 `try_expand_braces(pattern) -> Result<Vec<String>, String>` 保持不变，既有两个消费点（`core/checks/src/aggregate.rs`、`infra/checks/src/testexec/detect.rs`）零改动。
- **边方向合法性**：infra → core 已有先例（`agent-runtime → agent` / `config` / `orchestration` / `foundation` / `exec` / `store`）；`checks` MUST NOT 反向依赖 `agent-runtime`（crate 图机械保证无环）；`checks` 的纯层性质 MUST NOT 变化（零进程 spawn、零 Tauri，边界归类仍为 checks 边界的纯层半边）；`agent-runtime` 仍 MUST NOT 依赖 Tauri。
- **替代方案被否留痕**：把 `match_glob` 下沉 `foundation` 被否——`foundation` 的「零其他 crate 依赖（含零 tokio）」是刻意不变量（取消原语 `cancel` 即为此以 std-only 实现），引入 `glob` crate 会破该不变量；故取「登记新增依赖边 + 方言单点」而非「下沉到最小 crate」。
- **现行口径声明**：本条款 SHALL 为 `agent-runtime` workspace 内依赖集合的现行登记。既有「`agent-runtime` SHALL 仅依赖 `agent`」表述已随多轮演进（config / exec / orchestration / foundation / store 逐次引入）陈旧，本条款只登记新增边与方言理由，MUST NOT 被解读为放宽任何分层结论：core 各 crate 仍不得依赖 infra、`config` 仍仅依赖 `foundation`、`store` 仍不依赖 core 行为、`foundation` 仍零 workspace 内依赖。

#### Scenario: 依赖边审查

- **WHEN** 审查 `crates/infra/agent/Cargo.toml` 与 `crates/core/checks/Cargo.toml`
- **THEN** `agent-runtime` 的 `[dependencies]` 含 `checks = { workspace = true }` 且有用途注释（glob 方言单点复用）；`checks` 的依赖中不出现 `agent-runtime` 或任何 infra crate；`agent-runtime` 无 Tauri 依赖

#### Scenario: 方言单点生效

- **WHEN** 审查写入保护的规则匹配实现
- **THEN** 匹配只经 `checks::globmatch::match_glob`；protect 模块内无第二份花括号展开、无自建 `glob::Pattern` + 自定义 `MatchOptions` 的匹配路径（`glob` crate 在 `agent-runtime` 内的既有用途仅为 glob 工具的目录扫描，不参与保护判定）

#### Scenario: foundation 不变量守住

- **WHEN** 审查 `crates/core/foundation/Cargo.toml`
- **THEN** 依赖面与本变更前一致（无 `glob` crate、无任何 workspace 内依赖）——方言未下沉，极小 crate 的零依赖不变量不变

#### Scenario: 分层结论不变

- **WHEN** 复核既有「依赖方向符合分层」「core 零 Tauri」「store 不依赖 core 行为」断言
- **THEN** 全部结论原样成立；新增边只发生在 infra → core 纯层方向，checks 边界的成员（进程执行）仍不出现在 `infra/agent`

### Requirement: core/archi crate 落地登记

既有「crate 三层分组与依赖规则」中「未来 `core/archi` crate 落地时 SHALL 满足 `desktop-app → archi → foundation` 且 `workflow` 与 `archi` 互不依赖」的预告 SHALL 由本条兑现为落地登记（该 requirement 的 crate 树列举与规则文本本身零改写，本条为其增补登记）：

- **落位**：`crates/core/archi`，crate 裸名 `archi`（workspace 内唯一）；承载模型解析、内存图、结构校验、文档关联、结构 diff、gate 写回纯函数与三面共用 DTO。
- **workspace 内依赖**：`archi → foundation`（架构域路径派生）**唯一**。`foundation` 仍零 workspace 内依赖。
- **两条新增入边**：`desktop-app → archi`（命令面薄包装）与 `agent-runtime → archi`（agent 工具执行体复用同一 DTO 投影）。后者的理由 SHALL 与本仓既有 `agent-runtime → checks`（glob 方言单点）同构：**同一投影的第二个消费方直接消费同源实现**，MUST NOT 各写一份 JSON 投影——否则「三方一致性」纪律（见 `desktop-archi-model`）在机制上失效。两条边 MUST 在 `Cargo.toml` 处有用途注释。
- **零依赖面**：`archi` MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `workflow`（`workflow` ⇄ `archi` 互不依赖为硬红线，双向均须机械可验）、MUST NOT 依赖 `orchestration`、MUST NOT 进程 spawn（零 `std::process` / 零 `tokio::process`）。传参面纯函数化（gate 写回不触文件系统、文件读写归命令层）。
- **外部依赖归类**：`serde` / `serde_json` / `specta`（DTO 三件）+ `toml` / `toml_edit`（格式与渲染）+ `petgraph`（图算法）+ `ignore`（目录遍历，自动尊重 gitignore）；版本 SHALL 取自 workspace 锁内既有版本（零新增包）。
- **边界分类学不增类**：架构模型文件读面归既有「文件边界」的 workspace 文件读（经 core 纯读域），不在七类边界之外新增类别；`archi` MUST NOT 引入任何外部进程或 shell 触点。
- **消费者登记**：`foundation::layout` 的架构域派生函数（`archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`）以本 crate 为第二个消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」。
- **配置出口纪律不变**：`archi` MUST NOT 依赖 `config` crate、MUST NOT 读取工作区 `config.json`（架构域配置是另一个文件，且本期不产出其读取面）；`desktop-crate-layout` 既有「工作区配置唯一出口」结论零变化。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/core/archi/Cargo.toml` 与 `crates/infra/agent/Cargo.toml`
- **THEN** `archi` 的 workspace 内依赖恰为 `foundation`，无 Tauri / 无 `store` / 无 `workflow` / 无 `orchestration`；`agent-runtime` 含 `archi = { workspace = true }` 且带用途注释；`workflow` 的依赖中不出现 `archi`

#### Scenario: foundation 不变量守住

- **WHEN** 审查 `crates/core/foundation/Cargo.toml` 与既有「foundation 保持极小」断言
- **THEN** 依赖面与变更前一致（无新增 workspace 内依赖、无新增外部依赖），新增内容仅为 layout 常量组与四个派生 / 判定函数

#### Scenario: 零 spawn 与边界归类

- **WHEN** 审查 `archi` 的进程触点与边界归类
- **THEN** 无 `std::process` / `tokio::process` / shell 调用；边界归既有「文件边界」读面，七类边界分类学零增类，`infra/` 目录不新增成员

#### Scenario: 图层结论全量不变

- **WHEN** 复核既有「core 零 Tauri」「store 不依赖 core 行为」「config 仅依赖 foundation」三条断言
- **THEN** 全部原样成立；新增依赖边只发生在 `desktop-app → archi`（壳 → core）与 `agent-runtime → archi`（infra → core 纯层）两个既有合法方向



### Requirement: 工具 crate `archi-api-gen` 落位与契约面依赖扩展

模块 API 契约的**CLI 驱动面** SHALL 落位独立工具 crate `crates/tools/archi-api-gen`（crate 裸名 `archi-api-gen`，workspace 内唯一）。本能力只登记其落位、依赖方向、边界归类与写盘实现的外置；契约格式、判据、生成语义与写盘单点分别归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-contract-panel`。

- **落位与职责**：承载 CLI 三子命令（`generate` / `--check` / `bootstrap`）与三子命令的**编排**（`generate` = 渲染 + 写盘单点；`--check` = 比对 + 打印；`bootstrap` = 草稿文本）。判定（契约解析 / `$defs` 规则 / 十条判据 / 渲染出字符串）MUST NOT 在本 crate 实现——全部在 `crates/core/archi/src/api/**`；**写盘实现同样 MUST NOT 在本 crate**——单点驻 `crates/infra/archi`。它存在的唯一理由是「CLI 入口 + 子命令编排」：`archi` 的零写盘纪律不动，写盘单点唯一性由 `infra/archi` 承担。
- **workspace 内依赖**：恰为 `archi`（契约解析 / 装配 / 判据 / 渲染 / 比对）+ `foundation`（契约目录与生成物路径派生）+ `archi-runtime`（**唯一用途**：调生成物写盘单点 `write_artifacts`；该依赖 MUST 在 `Cargo.toml` 处带用途注释），依赖方向为 tools → core / infra 单向，MUST NOT 出现 core / infra → tools。
- **写盘经单点（不再是本 crate 的工具边界属性）**：本 crate MUST NOT 出现 `fs::write` / `create_dir_all` / `File::create`——写盘经 `archi-runtime` 的单点函数完成；该纪律 MUST NOT 被用来稀释 `archi` 的零写盘纪律，也 MUST NOT 让 app / core / infra 任何 crate 依赖本 crate。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `config`、MUST NOT 依赖 `workflow`、MUST NOT 依赖壳（`dev-team`）；**MUST NOT 直接依赖 `orchestration`**——`archi-runtime` 自带 `orchestration` 依赖（port 与产出类型），该传递边不可避免且在案留痕，本 crate 源码 MUST NOT `use orchestration::…`。进程面：本 crate 自身为 CLI 二进制，MUST NOT 派生任何子进程（零 `std::process::Command` / 零 shell）。
- **入边**：`archi-api-gen` MUST NOT 被 app / core / infra 依赖，仅经 `cargo run -p archi-api-gen`（`package.json` 的 `api:export` / `api:check`）驱动——与 `export-bindings`「不被 app 构建拖带」的裁定同式（避免 `tauri build` 把工具 bin 编进 release 产物）。新增入边 SHALL 恰为 `archi-api-gen → archi-runtime`（tools → infra 单向，唯一 tools → infra 边）。
- **`archi` crate 零新增外部依赖**：契约面所需的 `syn`（Rust 声明抽取）/ `oxc_ast` / `oxc_parser` / `oxc_span` / `oxc_syntax`（TS 声明抽取）/ `sha2`（生成物头摘要）/ `globset` / `serde_json` / `specta` 全部已在 `Cargo.lock` 且已是 `archi` 的直接依赖 ⇒ 本变更 MUST NOT 引入新外部包。`archi` 的零 `workflow` 依赖与 `workflow` ⇄ `archi` 互不依赖两条红线零变化。
- **边界分类学不增类**：生成物写盘归既有「文件边界」的工具写面（与 bindings 导出同族），MUST NOT 新增第八类边界；`infra/` 目录不新增成员（新 crate 落 `tools/`）。
- **段名字面量纪律扩展**：生成器与契约面 MUST NOT 行内拼 `<archi_dir>/api` 段——一律经 `foundation::layout::archi_api_dir`（见 `workspace-layout-resolution`）。该纪律与既有 `openspec` / `config.json` 双禁令同源，机械可验。
- **消费者登记**：`foundation::layout` 的 `archi_api_dir` / `archi_api_rel_dir` 以本 crate 与 `archi` 为消费者而落地，登记见 `workspace-layout-resolution`「架构域路径派生（archi_dir 族）」与「架构域 repo 相对路径派生（会话知识用）」。

#### Scenario: 依赖方向与红线

- **WHEN** 审查 `crates/tools/archi-api-gen/Cargo.toml`、`crates/core/archi/Cargo.toml`、`crates/infra/archi/Cargo.toml` 与壳 / core / infra 各 `Cargo.toml`
- **THEN** `archi-api-gen` 的 workspace 内依赖恰为 `archi` + `foundation` + `archi-runtime`（后者带用途注释）；零 Tauri / 零 `store` / 零 `config` / 零 `workflow` / 零壳 / 零直接 `orchestration`；壳与 core / infra 各 crate 的依赖中均不出现 `archi-api-gen`；`workflow` 的依赖中不出现 `archi`

#### Scenario: 写盘单点驻 infra/archi

- **WHEN** 审查 `crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/src/**` 与 `crates/infra/archi/src/api_artifacts.rs` 的文件系统调用
- **THEN** `archi` 侧零写盘调用（`std::fs::write` / `create_dir*` / `File::create` 全无）；`archi-api-gen` 侧零写盘调用（三子命令只做编排）；写盘恰在 `infra/archi` 的 `write_artifacts` 一处（含 `starts_with(root)` 越根复核）；`archi` 与 `archi-api-gen` 两侧仍零 `std::process` / 零 `tokio::process`

#### Scenario: 零新外部依赖与零进程

- **WHEN** 比对变更前后的 `Cargo.lock` 与 `crates/core/archi/Cargo.toml`
- **THEN** `archi` 的依赖清单零新增条目（`syn` / `oxc_*` / `sha2` / `globset` / `serde_json` 均已在案）；`archi-api-gen` 自身零子进程派生（仅 CLI 入口读 argv 并调库）；workspace `members` 与 `[workspace.dependencies]` 零新增项（无新 crate）

#### Scenario: 生成器不被 app 构建拖带

- **WHEN** 审查 `package.json` 脚本与 `tauri.conf.json`
- **THEN** 生成器只经 `cargo run -p archi-api-gen` 驱动（`api:export` / `api:check`）；`tauri build` 的产物不含该 bin；`src-tauri/Cargo.toml` 的壳 `[dependencies]` 不含 `archi-api-gen`

### Requirement: infra/archi crate 落地登记与 archi 依赖扩展

架构漂移判定域的执行面 SHALL 落位 `crates/infra/archi`（crate 裸名 `archi-runtime`），并在本能力登记其落位、依赖与边界归类：

- **落位与职责**：承载 `ArchiCheckRunner` 实现（取输入 → 调 `archi` 判定纯函数 → 落 `reports/archi` 报告 → 派生 outcome）与报告落盘单点；并承载**生成物写盘单点**（`write_artifacts`：越根复核 + 建目录 + 写文件，与报告落盘并列为本 crate 两处写盘单点，两处都只是 dumb IO，零判定）。领域判定（扫描 / 归属 / 定级 / 报告结构 / 配置读取 / 契约判据 / 生成物渲染与比对）MUST NOT 在此重复——全部在 `crates/core/archi`（见 `desktop-archi-drift` / `desktop-archi-contract-panel`）。
- **workspace 内依赖**：`archi`（判定 / 报告结构 / 生成物渲染与比对）+ `orchestration`（port 与产出类型，与 `checks-runtime` 依赖 `orchestration` 同构）+ `foundation`（报告目录派生）。依赖方向 SHALL 为 infra → core 单向，MUST NOT 出现 core → infra。依赖清单零新增（写盘单点不引入新依赖）。
- **零依赖面**：MUST NOT 依赖 Tauri、MUST NOT 依赖 `store`、MUST NOT 依赖 `vcs-runtime`（变更文件清单经 port 注入，见 `desktop-archi-gate`「门禁输入口径与观察根」）、MUST NOT 依赖 `config`、MUST NOT 依赖 `workflow`、MUST NOT 创建任何进程（零 `std::process` / 零 `tokio::process` / 零 shell）。
- **入边**：新增入边 SHALL 为两条——`desktop-app → archi-runtime`（壳层装配注入 runner + 两条契约面命令调写盘单点，MUST 在 `Cargo.toml` 处带用途注释）与 `archi-api-gen → archi-runtime`（tools 驱动面调写盘单点，同样带用途注释）；`archi-runtime` MUST NOT 被 `workflow` / `archi` / `agent-runtime` / `checks-runtime` 依赖（agent 侧只经 `core/archi` 的只读面消费契约事实，见 `desktop-archi-agent`）。
- **`archi` crate 依赖扩展**：`crates/core/archi` SHALL 新增外部依赖 `oxc_parser` / `oxc_allocator` / `oxc_resolver`（TS / JS 词法与模块解析）与 `syn`（Rust 语法与 `use` 树），并把 `globset` 由传递依赖提为直接依赖（排除匹配）；这些依赖 MUST NOT 引入进程、Tauri、`config` 或 `workflow` 触点，`archi` 仍保持零 `workflow` 依赖（`workflow` ⇄ `archi` 互不依赖为硬红线，双向机械可验）。
- **边界分类学不增类**：本执行面零新外部依赖（`serde_json` 为锁内既有），归既有「文件边界」的读盘面 + 检查结论落盘（报告树）+ 生成物写盘（工具同族写面），MUST NOT 新增第八类边界；`infra/` 目录不新增除 `archi` 外的成员。
- **依赖成本护栏**：`Cargo.lock` 净新增包数 SHALL ≤ 40（软线 25）；超出即视为特性开关引入面失控，先收紧 `oxc` 特性再复测（成本实测与回退序见 `desktop-archi-drift`「运行期预算与语料」）。

#### Scenario: 依赖面与方向审查

- **WHEN** 审查 `crates/infra/archi/Cargo.toml`、`crates/core/archi/Cargo.toml` 与壳 `Cargo.toml`
- **THEN** `archi-runtime` 的 workspace 内依赖恰为 `archi` / `orchestration` / `foundation`（本变更零新增）；`archi` 的 workspace 内依赖仍仅 `foundation`；壳与 `archi-api-gen` 各含 `archi-runtime = { workspace = true }` 且带用途注释；`workflow` 依赖中不出现 `archi`，`archi` 依赖中不出现 `workflow`

#### Scenario: 零进程零 Tauri 审查

- **WHEN** 审查 `crates/infra/archi` 与 `crates/core/archi` 的进程触点与框架依赖
- **THEN** 两者均无进程创建、无 shell 调用、无 Tauri 依赖；`archi-runtime` 零 `store` / 零 `config` / 零 `vcs-runtime` 依赖；`agent-runtime` 的依赖中不出现 `archi-runtime`（写盘单点对 agent 面不可达）

#### Scenario: 依赖成本不越硬线

- **WHEN** 完成 `oxc_*` / `syn` 引入并执行成本实测（锁包数 / 安装包 / 主二进制 / 编译用时）
- **THEN** `Cargo.lock` 净增 ≤ 40 包；任一指标越硬线时按回退序（减特性 → 自算路径 → 正则 import）处置并留痕，MUST NOT 直接放宽阈值

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop`（包） | 仓库内唯一可分发包 | 自带 lockfile 与构建脚本；不进仓库根 workspace；不进 `dist/` 分发链（`dist/` 三份产物目录已随插件退役删除） |
| `packages/desktop/src-tauri/Cargo.toml` | cargo workspace 三层分组 | core / infra / desktop-app 三层依赖方向不变；`crates/core/config/config.schema.json` 为 crate 内资产（不入 bundle） |
| `packages/desktop/package.json` | 版本权威 | patch +1 纪律（见 desktop-release-pipeline「版本交付纪律」）；MUST NOT 由产物重建驱动 |
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
| `crates/infra/vcs`（新，裸名 `vcs-runtime`，vcs 边界首成员） | git 工作面进程执行 | 依赖：`vcs-runtime` → workflow（WorktreePort 类型）；`foundation` 仅 dev-dependencies（测试取消原语夹具）；允许 spawn；禁 Tauri；由 desktop-app 装配注入（`desktop-app → vcs-runtime`） |
| `store → foundation` 依赖（新） | 依赖规则第二例修订落痕 | 仅身份段派生纯函数（仅 workspace db 文件名消费；worktree 落位已解耦，store 内零第二份派生实现）；store 行为自含、零 Tauri 不变 |
| `crates/infra/` | 目录表达计划 | store / agent（裸名 agent-runtime）/ checks / watch / vcs 五个已落地成员；graph / api / fs 均不预建，成员出现才立 |
| `crates/core/config`（新，裸名 `config`） | 工作区配置唯一合法出口（读 + 校验 + 默认值） | 仅依赖 `foundation` + serde 系 + specta；禁 Tauri；未来模块取配置仅准经此，MUST NOT 自行读 config.json；首个真实消费者出现才立 `→ config` 边 |
| `mod_test.rs` 命名隔离扫描（扩展） | 字面量执法不变量 | `openspec` 目录名 + `config.json` 文件名双禁令；唯一触点 layout/mod.rs 常量组；`*_test.rs` 排除、全文匹配含注释 |
| `crates/core/foundation` | 共享地基（极小纪律） | 新增能力第三项 = 取消原语 `cancel`（`CancelFlag` / `CancelHandle`）；无 workspace 内依赖、无 tokio；其余（layout / 身份段派生）零改动 |
| `crates/core/foundation/src/cancel.rs`（新） | 取消原语 | `CancelFlag::{never, is_set}`（`Clone` 共享同一原子）、`CancelHandle::set`；std::sync::atomic + Arc |
| `crates/infra/exec` | 消费方（读） | `run_shell` 以 200 ms 轮询 `CancelFlag::is_set()`；crate 依赖仍为 `foundation` + tokio（特性面 `process` / `io-util` / `time` / `macros`），非 Windows 目标零新增依赖（Windows 平台绑定经条件段，见下行） |
| `crates/infra/exec/Cargo.toml` | 平台条件依赖 | `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }`（按需最小 feature + 用途注释）；非目标平台零新增 |
| `crates/infra/exec/src/lockprobe.rs` | 平台 API 消费点 | 访问权 / 共享模式常量经 std `OpenOptionsExt`；Restart Manager 经 `cfg(windows)` 模块；非 Windows 臂零 windows-sys 引用 |
| `crates/core/**` | 分层边界 | 零平台绑定依赖、零 Tauri（结论不变） |
| `crates/core/orchestration` | 消费方（写） | `RunGuard::cancel_flag()`、`request_stop` 双写；core→core 依赖不变（`orchestration → foundation` 既有边） |
| `crates/infra/agent/Cargo.toml` | 新增 infra → core 纯层依赖边 | `checks = { workspace = true }`（唯一用途：`checks::globmatch::match_glob`；带用途注释） |
| `crates/core/checks/src/globmatch.rs`（既有，零改） | 唯一 glob 方言实现 | `match_glob` / `try_expand_braces` 签名与语义不变；花括号展开上限 `EXPANSION_LIMIT`、`MatchOptions{ require_literal_separator: true, require_literal_leading_dot: false }` 单点持有 |
| `crates/infra/agent/src/sdk/protect/**` | 依赖边消费方 | 匹配只调 `checks::globmatch::match_glob`；MUST NOT 自建第二套方言（含自定 `MatchOptions`） |
| `crates/core/foundation/**` | 本变更零改动 | 依赖面零变化（零 workspace 内依赖、零 `glob` crate）；`layout::config_path` 仅为内置 glob 派生源，不承载匹配语义 |
| `crates/core/checks/src/aggregate.rs`、`crates/infra/checks/src/testexec/detect.rs` | 本变更零改动（既有两消费点） | 调用点与断言不变，方言演进仍单点 |
| `crates/core/archi`（新，裸名 `archi`） | 架构模型纯读域 + 纯函数写渲染 | workspace 内依赖仅 `foundation`；外部依赖 serde 系 + `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`（锁内既有版本）；零 Tauri / 零 store / 零 spawn / 零 `workflow` |
| `crates/infra/agent`（依赖登记，`archi` 边） | 工具执行体复用同一投影 | `archi = { workspace = true }` + 用途注释；理由与 `agent-runtime → checks`（glob 方言单点）同构 |
| `crates/desktop-app`（依赖登记，`archi` 边） | 命令面薄包装 | `archi = { workspace = true }`；命令层零领域规则（投影全在 `archi::dto`） |
| `crates/core/foundation`（消费者登记，archi_dir 族） | 架构域路径派生 | 四个函数以 `archi` 为第二个消费者落地；`foundation` 依赖面零变化 |
| `crates/infra/archi`（新，裸名 `archi-runtime`） | 架构门禁执行面 + 报告落盘 + 生成物写盘单点 | workspace 内依赖 `archi` + `orchestration` + `foundation`；外部依赖 `serde_json`；写盘单点两处（`report::write_report` / `api_artifacts::write_artifacts`，均 dumb IO）；零 Tauri / 零 spawn / 零 store / 零 vcs / 零 config / 零 workflow |
| `crates/core/archi`（依赖扩展） | TS / JS 与 Rust 解析面 | 外部依赖新增 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn`（+ `globset` 提为直接依赖）；`foundation` 仍为唯一 workspace 内依赖；零 spawn / 零 Tauri / 零 store / 零 workflow |
| `crates/desktop-app`（依赖登记） | 装配注入 | `archi-runtime = { workspace = true }` + 用途注释；装配层零判定（runner 构造与注入） |
| `src-tauri/Cargo.toml`（workspace 清单） | 成员与版本收敛 | `members` 增 `crates/infra/archi`；`[workspace.dependencies]` 增 `archi-runtime` 与 `oxc_parser` / `oxc_allocator` / `oxc_resolver` / `syn` 收敛项（版本取实测选定值，MUST NOT 各 crate 分散声明） |
| 边界分类学 | 七类不增类 | 零新外部依赖 ⇒ 归既有文件边界读盘面 + 报告落盘；`infra/` 仅新增 `archi` 一个成员 |
| `crates/tools/archi-api-gen/Cargo.toml`（新） | 工具 crate 落位与依赖 | 裸名 `archi-api-gen`；workspace 内依赖恰为 `archi` + `foundation` + `archi-runtime`（后者唯一用途 = 调写盘单点 `write_artifacts`，带用途注释）；外部 `serde_json`；零 Tauri / 零 store / 零 config / 零直接 orchestration / 零 workflow / 零壳 |
| `crates/tools/archi-api-gen/src/main.rs`（新） | CLI 唯一入口 | 三子命令 `generate` / `--check` / `bootstrap`；`--check` 零写入、有差异即非零退出并列文件；目录创建经 `foundation::layout` 派生；零判定逻辑 |
| `src-tauri/Cargo.toml`（workspace 成员登记） | 工具 crate 成员 | `members` 增 `crates/tools/archi-api-gen`；`[workspace.dependencies]` 增 `archi-api-gen = { path = "crates/tools/archi-api-gen" }`（入口注释说明「CLI 工具，不被 app 依赖」）；壳 `[dependencies]` MUST NOT 增该 crate |
| `packages/desktop/package.json`（脚本面） | 驱动入口 | `api:export` = `cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- generate`（写盘）；`api:check` = 同一调用加 `--check`（零写入、工具内比对，覆盖四类生成物：`<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs` / `<root>.i.ts`）；`check` 链在 `bindings:export` 之后插 `api:check` |
| `crates/core/archi/Cargo.toml`（契约面依赖面） | 依赖面零新增 | 契约面无需新外部依赖（`syn` / `oxc_*` / `sha2` / `globset` / `serde_json` / `specta` 均已在案）；零 Tauri / 零 store / 零 spawn / 零 `workflow` 四条结论零变化 |

### CLI: `crates/tools/archi-api-gen`

| 项 | 变化 | 描述 |
|---|---|---|
| `Cargo.toml` | 改 | `[dependencies]` 加 `archi-runtime = { workspace = true }`（用途注释：调生成物写盘单点）；零新增外部包；零子进程 |
| `src/main.rs` | 收窄 | 只剩 argv 解析 + 退出码 + 三子命令编排；零 `fs::write` / 零本地 walker |
| 不被依赖 | 零改动 | 壳 / core / infra 各 `Cargo.toml` 均不出现 `archi-api-gen` |

### API: `crates/infra/archi`

| 项 | 变化 | 描述 |
|---|---|---|
| `src/api_artifacts.rs` | 新增 | `write_artifacts(root, &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`：越根复核 + 建目录 + 读盘比对后写；不删文件、不回滚 |
| `src/report.rs` | 零改动 | 报告落盘单点逐字不变（两处写盘单点并列） |
| 依赖面 | 零改动 | `archi` / `orchestration` / `foundation` + `serde_json`；零 Tauri / 零 spawn / 零 `store` / 零 `config` / 零 `workflow` |
| 入边 | 追加一条 | `archi-api-gen → archi-runtime`（tools 驱动面）；既有 `desktop-app → archi-runtime` 保留 |
