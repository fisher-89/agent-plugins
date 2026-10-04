# move-queries-command-to-change — desktop-app-shell 变更集

> queries 轨道消解：三命令全部为 change 域读面（explore / workspace / db 查询均早已独立成组），并入 change 域命令组 `commands/changes/`（读 + 记录面同组，沿 explores 组先例）。本 delta 修改轨道组织、workspace 注册轨道、command body 纪律三处 requirement；Module Contract 的 `dev-team::commands::queries` 行随归档同步为 `dev-team::commands::changes`（四命令组）。

## MODIFIED Requirements

### Requirement: Tauri command 轨道组织

dev-team SHALL 按 queries / exec / db 三轨组织 Tauri command（queries 轨道已由 move-queries-command-to-change 消解：三命令全部为 change 域读面，并入 change 域命令组，见下）：

- `commands/changes/`：change 域命令组（读 + 记录面同组，沿 explores 组先例）——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）三读命令与 `create_change`（新建 change，见 desktop-change-create）共组；`commands/queries/` 轨道 MUST NOT 残留（目录、模块声明或 `all_commands!` 登记路径任一存在即违例）；组内 blank root 双口径并存且 MUST NOT 互换——读命令走空结果语义（空清单 / `None`）、写命令走显式 `Err`（写无空结果语义）
- `commands/exec/`：执行轨道。空轨道状态由 desktop-agent-execution 结束，首批命令为 `agent_start`（agent 执行）与 `agent_runs` / `agent_run_events`（run 重放查询）。轨道纪律升级为：MUST NOT 出现空壳 Executor 类 trait（agent 执行的抽象由 `core/agent` 的 `AgentRunner` 承担）；后续 workspace 写文件等执行命令落此轨道时按各自 proposal 定形
- `commands/db/`：db 查看轨道（native-db-store-upgrade 开通），承载只读 db 检查命令 `db_models`（模型清单与计数）与 `db_records`（按模型分页扫描），经 `State<Store>` 调用 store 的记录信封 API（见 desktop-workspace-store「记录信封 API」）。轨道纪律：只读，MUST NOT 出现任何写命令；命令命名 design 可调

每个查询 command SHALL 是无状态薄包装：参数 → core 函数或 store 操作 → DTO 返回，MUST NOT 在 command 层持有或缓存 workspace 状态；所有 workspace 状态访问 SHALL 只经 workflow / foundation 的 core 函数。DTO SHALL 区分 Query Result 与 Command Result 形态。

#### Scenario: 查询命令薄包装

- **WHEN** 前端 invoke `list_changes` / `get_change_detail` / `read_artifact`
- **THEN** command 仅做参数转换并调用 core 查询函数，返回 DTO，自身无状态

#### Scenario: queries 轨道消解不留空壳

- **WHEN** 检查 `src/commands/` 目录树与 `all_commands!` 清单
- **THEN** 无 `queries` 模块（目录与 `pub mod` 声明均不存在）；`list_changes` / `get_change_detail` / `read_artifact` 三命令实现于 `commands/changes/mod.rs`，经 `$crate::commands::changes::*` 路径登记
- **AND** 三命令函数名、签名与返回类型与搬迁前一致（IPC 面零变化，重导出 bindings 零 diff）

#### Scenario: 组内 blank root 双口径不互换

- **WHEN** 分别以空白 root invoke 读命令（`list_changes` / `get_change_detail` / `read_artifact`）与写命令（`create_change`）
- **THEN** 读命令返回空结果语义（空清单 / `None`，不报错）；`create_change` 返回显式 `Err`，不进入写面链路

#### Scenario: exec 轨道承载 agent 首批命令

- **WHEN** 检查 `commands/exec/`
- **THEN** `agent_start` / `agent_runs` / `agent_run_events` 三命令落地，无空壳 Executor trait
- **AND** workspace 写文件未在此轨道实现（仍待后续变更定形）

#### Scenario: db 轨道只读薄包装

- **WHEN** 前端 invoke `db_models` / `db_records`
- **THEN** 命令经 `State<Store>` 调用信封 API 返回 DTO，自身无状态且无任何写操作

#### Scenario: 状态访问收敛 core

- **WHEN** 审查 command 层代码
- **THEN** 无直接文件系统访问，workspace 状态一律经 core 函数获取

### Requirement: workspace 注册命令轨道

dev-team SHALL 新增 `commands/workspaces` 命令轨道，承载 workspace 注册表（shell 记忆，非 change 域查询）三命令：`list_workspaces`（默认序清单：表主键 canonical root 升序）、`add_workspace`（canonicalize + upsert）、`remove_workspace`。命令 SHALL 经 Tauri `State<Store>` 访问 store（`main.rs` 启动时打开并 `.manage()`），自身 MUST NOT 直接操作 redb 或 db 文件。既有 `commands/changes/` 组读命令的无状态语义与 `commands/exec/` 轨道纪律 SHALL 保持不变。MUST NOT 存在「按打开时间刷新/排序」类命令（`touch_workspace` 已移除）。

本轨道 SHALL 确立后续可失败命令的错误约定模板：命令返回 `Result<T, String>`，`Err` 由 Tauri 转为前端 reject；MUST NOT 静默吞掉 db 打开或读写失败。db 打开失败 SHALL 使应用启动失败并报错，MUST NOT 静默降级为空清单。reject 抵达前端后的呈现 SHALL 按本能力「错误呈现双轨」requirement 分流：动作类命令（add / remove）失败走 toast，查询类命令失败保留 inline error 态持久呈现。

#### Scenario: 命令经 State 访问 store

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** 三命令均以 `State<Store>` 取得 store，命令体为参数转换 + store 调用 + DTO 返回，签名中无 redb 类型

#### Scenario: 错误以 reject 传达前端

- **WHEN** store 操作失败（如 db 文件损坏、磁盘写失败）
- **THEN** 命令返回 `Err(String)`，前端收到错误字符串并按双轨语义呈现（动作失败 toast / 查询失败 inline 持久），无静默成功

#### Scenario: db 打开失败快速失败

- **WHEN** 启动时 db 文件无法打开
- **THEN** 应用启动失败并给出错误信息，不进入空清单的静默降级

#### Scenario: 既有轨道不受影响

- **WHEN** 检查 `commands/changes` 与 `commands/exec`
- **THEN** 三个读命令语义不变（搬迁仅改落位，不改命令面），exec 轨道已由 agent 执行命令开通、无空壳 Executor trait

### Requirement: command body 纪律与 app 层微形态

dev-team SHALL NOT 抽独立 app 层 crate：command 即应用服务，维持既有命令组（changes / workspaces）加 exec 轨道（已由 agent 执行命令开通）的组织不变。作为补偿纪律，任何 Tauri command body SHALL 只允许三件事：

1. **参数转换**（IPC 入参 → 领域/store 入参）；
2. **调用**（core 函数或 store 操作）；
3. **错误映射**（领域/Store 错误 → `Err(String)`）。

command 层 MUST NOT 实现领域解释（属领域解释的编排 SHALL 下推 core）或跨边界协调（属跨边界协调的编排 SHALL 触发 app crate 决策，见下一 requirement），MUST NOT 以"先塞进命令里"的方式消化编排增长。

`commands/workspaces` 的 `*_inner(&Store)` 纯函数模式 SHALL 视为 app 层微形态（IPC 适配与纯逻辑的函数级分层）予以保留：将来抽 app crate 时 SHALL 将 inner 函数平移复用（函数边界升 crate 边界），MUST NOT 重写。

`commands/exec` 的 `run_agent()` 编排函数 SHALL 与 `*_inner` 同列 app 层微形态（`*_inner` 先例的进化）：`agent_start` 的 tee 协调（组装 runner → 事件流双 sink：Tauri Channel + store 落库 → run 状态收敛）收在编排函数内，命令体仍只做三件事。将来抽 app crate 时编排函数与 inner 函数 SHALL 一并平移复用，MUST NOT 重写。
