# desktop-app-shell Specification

## Purpose

定义 desktop-app Tauri 壳层的组织契约：command 按 queries / exec 双轨组织（MVP 仅实现三个查询命令、exec 为预留空轨道），workspace 经文件夹选择器选定，React 前端以显式刷新取数模型收口在 hooks 内。

## Requirements

### Requirement: Tauri command 查询双轨

desktop-app SHALL 按 queries / exec 双轨组织 Tauri command：

- `commands/queries/`：MVP 实现三个命令——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）
- `commands/exec/`：预留空轨道，MUST NOT 实现任何真实命令，MUST NOT 引入空壳 trait（trait 定形等第一条真实执行命令落地）

每个查询 command SHALL 是无状态薄包装：参数 → core 函数 → DTO 返回，MUST NOT 在 command 层持有或缓存 workspace 状态；所有 workspace 状态访问 SHALL 只经 workflow / foundation 的 core 函数。DTO SHALL 区分 Query Result 与 Command Result 形态。

#### Scenario: 查询命令薄包装

- **WHEN** 前端 invoke `list_changes` / `get_change_detail` / `read_artifact`
- **THEN** command 仅做参数转换并调用 core 查询函数，返回 DTO，自身无状态

#### Scenario: exec 轨道为空

- **WHEN** 检查 `commands/exec/`
- **THEN** 无任何已实现命令、无预定义 Executor trait 空壳

#### Scenario: 状态访问收敛 core

- **WHEN** 审查 command 层代码
- **THEN** 无直接文件系统访问，workspace 状态一律经 core 函数获取

### Requirement: workspace 选择

App SHALL 提供 workspace 选择与恢复能力：

- 启动时 SHALL 调用 `list_workspaces`，存在记录时自动恢复最近打开的 workspace（`last_opened_at` 降序第一名）并直接进入列表视图；无任何记录时 SHALL 停留在欢迎屏
- 当前根 SHALL 恒等于清单第一名（`list_workspaces` 按 `last_opened_at` 降序）：清单非空即选中第一名，清单为空（或被移除一空）时停留在欢迎屏；欢迎屏 SHALL 呈现空态与“添加新文件夹”入口；文件夹选择器 SHALL 仍是添加入口，选定后经 `add_workspace` 入库，刷新后新记录（`last_opened_at` 最新）即第一名并打开
- Header SHALL 提供 workspace 下拉切换；清单项 name 仅取目录名最后一段，悬停 SHALL 以 title 展示完整 path；切换 SHALL 清空当前 change 选中并以新根重新取数
- 选中 workspace（含自动恢复与下拉切换）SHALL 触发 `touch_workspace` 刷新 `last_opened_at`
- 移除当前打开的 workspace SHALL 经 Header 入口调用 `remove_workspace`；移除后 SHALL 切换到剩余清单第一名，清单为空时 SHALL 回到欢迎屏

选定 workspace 根后，列表与详情取数均以该根为基准。

#### Scenario: 启动自动恢复

- **WHEN** 应用启动且库中已有 workspace 记录
- **THEN** App 恢复 `last_opened_at` 最新的一条为当前根并进入列表视图，无需用户操作

#### Scenario: 无记录停留欢迎屏

- **WHEN** 应用启动且 `list_workspaces` 返回空清单
- **THEN** App 停留在欢迎屏，呈现“添加新文件夹”入口

#### Scenario: 欢迎屏添加

- **WHEN** 用户经文件夹选择器添加新目录
- **THEN** App 经 `add_workspace` 入库，刷新后以新记录（清单第一名）为根调用 `list_changes` 进入列表视图

#### Scenario: Header 下拉切换

- **WHEN** 用户在 Header 下拉中选择另一 workspace
- **THEN** 当前 change 选中被清空，`touch_workspace` 刷新 `last_opened_at` 后列表以新根（刷新后第一名）重取，悬停清单项可看到完整路径

#### Scenario: 移除当前打开的 workspace

- **WHEN** 用户经 Header 移除当前打开的 workspace
- **THEN** 该项从库中删除，视图切换到剩余清单第一名并以其为根重取列表；无剩余项时回到欢迎屏

### Requirement: React 前端刷新取数模型

前端 SHALL 以 React + TS 实现，取数收在 hooks（`useChangeList` / `useChangeDetail` / `useWorkspaces`）内：由用户显式刷新动作触发 invoke；`useWorkspaces` 为唯一例外——启动时自动触发一次以支撑自动恢复，此后仍由用户动作触发。MUST NOT 实现文件 watch、后台轮询或事件订阅。刷新 SHALL 覆盖两个层级：workspace 级（重取列表）与 change 级（重取当前详情）。未来替换为推送时 SHALL 仅改动 hooks 内部实现，视图层不感知取数方式。

视图 SHALL 至少呈现：change 列表（代际标注、按月分组）、phase 流水线（attempt / verdict / checklist 展开）、经 renderer 注册表渲染的产物区（含 markdown 文档与 tasks 进度）、workspace 清单（Header 下拉）。

#### Scenario: 刷新按钮触发重取

- **WHEN** 用户点击刷新
- **THEN** hooks 重新 invoke 对应查询命令并更新视图，期间无自动轮询发生

#### Scenario: 取数收口 hooks

- **WHEN** 审查视图组件代码
- **THEN** 组件不直接 invoke，取数统一经 `useChangeList` / `useChangeDetail` / `useWorkspaces`

#### Scenario: 无 watch 依赖

- **WHEN** 检查 desktop 依赖与前端代码
- **THEN** 无文件系统监听（notify 等）依赖、无定时器轮询逻辑

### Requirement: workspace 注册命令轨道

desktop-app SHALL 新增 `commands/workspaces` 命令轨道，承载 workspace 注册表（shell 记忆，非 change 域查询）四命令：`list_workspaces`（`last_opened_at` 降序清单）、`add_workspace`（canonicalize + upsert + touch）、`remove_workspace`、`touch_workspace`。命令 SHALL 经 Tauri `State<Store>` 访问 store（`main.rs` 启动时打开并 `.manage()`），自身 MUST NOT 直接操作 redb 或 db 文件。既有 `commands/queries/` 三命令的无状态语义与 `commands/exec/` 空轨道 SHALL 保持不变。

本轨道 SHALL 确立后续可失败命令的错误约定模板：命令返回 `Result<T, String>`，`Err` 由 Tauri 转为前端 reject，前端 hook 以 error 态接住呈现；MUST NOT 静默吞掉 db 打开或读写失败。db 打开失败 SHALL 使应用启动失败并报错，MUST NOT 静默降级为空清单。

#### Scenario: 命令经 State 访问 store

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** 四命令均以 `State<Store>` 取得 store，命令体为参数转换 + store 调用 + DTO 返回，签名中无 redb 类型

#### Scenario: 错误以 reject 传达前端

- **WHEN** store 操作失败（如 db 文件损坏、磁盘写失败）
- **THEN** 命令返回 `Err(String)`，前端 hook 的 error 态收到错误字符串并呈现，无静默成功

#### Scenario: db 打开失败快速失败

- **WHEN** 启动时 db 文件无法打开
- **THEN** 应用启动失败并给出错误信息，不进入空清单的静默降级

#### Scenario: 既有轨道不受影响

- **WHEN** 检查 `commands/queries` 与 `commands/exec`
- **THEN** 三个查询命令语义不变，exec 轨道仍无实现、无空壳 trait

### Requirement: command body 纪律与 app 层微形态

desktop-app SHALL NOT 抽独立 app 层 crate：command 即应用服务，维持既有双轨（queries / workspaces）加 exec 空轨道的组织不变。作为补偿纪律，任何 Tauri command body SHALL 只允许三件事：

1. **参数转换**（IPC 入参 → 领域/store 入参）；
2. **调用**（core 函数或 store 操作）；
3. **错误映射**（领域/Store 错误 → `Err(String)`）。

command 层 MUST NOT 实现领域解释（属领域解释的编排 SHALL 下推 core）或跨边界协调（属跨边界协调的编排 SHALL 触发 app crate 决策，见下一 requirement），MUST NOT 以"先塞进命令里"的方式消化编排增长。

`commands/workspaces` 的 `*_inner(&Store)` 纯函数模式 SHALL 视为 app 层微形态（IPC 适配与纯逻辑的函数级分层）予以保留：将来抽 app crate 时 SHALL 将 inner 函数平移复用（函数边界升 crate 边界），MUST NOT 重写。

#### Scenario: 新命令符合三件事

- **WHEN** 新增任一 Tauri command
- **THEN** body 为参数转换 + 调用 + 错误映射三段，无编排逻辑、无领域解释、无跨 store / fs 协调

#### Scenario: 编排增长被下推而非上塞

- **WHEN** 某命令需要新增一段编排逻辑（解析、校验、多步写等）
- **THEN** 领域解释部分落入 core crate，命令层仅保留三件事；跨边界协调则触发 app crate 决策，而非继续膨胀命令体

#### Scenario: inner 函数缝保留

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** IPC 命令为薄包装并经 `*_inner(&Store)` 纯函数调用 store，错误映射在命令层完成；该模式未被"内联回命令体"的重构破坏

### Requirement: app crate 翻转信号

出现下列任一信号时 SHALL 重新决策是否抽独立 app 层 crate（决策触发，不等架构回顾）：

1. Rust 侧引入 phase lifecycle 类命令（parse + validate + mutate + write 的真编排；TS 端 phase_start / phase_log 已落地，Rust 对齐时即触发）；
2. exec 轨道第一条真实命令落地（写 repo：保护检查 + 审计 + 执行 + touch 协调）；
3. 命令出现跨 store + fs 协调（如 add workspace 后自动 rescan）；
4. CLI / headless 复用桌面后端逻辑的需求出现；
5. 机械判据兜底：单命令非 IPC 样板逻辑超过一屏（约 30 行），或 `commands/` 非样板逻辑合计持续增长。

翻转信号清单 SHALL 保持可机械判定（命令类型可枚举、行数可数、复用需求可证），SHALL 随本能力 spec 存档，供代码评审与后续变更 proposal 引用。

#### Scenario: 信号可机械判定

- **WHEN** 审查翻转信号清单
- **THEN** 每条信号可由代码状态直接判定（特定命令是否出现 / 命令体行数 / 复用需求是否提出），无需主观架构评价

#### Scenario: 信号触发即重议

- **WHEN** 任一翻转信号出现（如 exec 第一条真实命令进入 design）
- **THEN** 对应变更的 proposal 显式回应"是否抽 app crate"的重新决策，而不是默认沿袭"不建"

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `desktop-app::commands::queries` | 三个查询命令 | list_changes / get_change_detail / read_artifact；无状态薄包装；返回 DTO |
| `desktop-app::commands::workspaces` | workspace 注册命令轨道 | list_workspaces / add_workspace / remove_workspace / touch_workspace；`State<Store>`；`Result<T, String>` |
| `desktop-app::commands::exec` | 预留空轨道 | 无实现、无空壳 trait |
| 前端 `hooks/` | 取数收口 | useChangeList / useChangeDetail / useWorkspaces；显式刷新触发（useWorkspaces 启动自动一次） |
| 前端视图 | 列表 / 流水线 / 产物区渲染 + 欢迎屏空态 / Header 下拉 | 消费 DTO 与 ArtifactEnvelope；未注册 kind 由 Fallback 兜底 |
| `desktop-app::commands::*`（全体命令） | 应用服务（command 即应用服务） | body 三件事：参数转换 / 调用 / 错误映射；无独立 app crate；编排下推 core 或触发翻转 |
| `commands::workspaces::{list,add,remove,touch}_workspace_inner` | app 层微形态 | 纯函数 + `&Store` 入参；将来抽 crate 时平移复用，不重写 |
| app crate 翻转信号 | 决策触发器 | 五条信号（phase lifecycle 命令 / exec 首命令 / 跨 store+fs 协调 / CLI 复用 / 行数机械判据），任一出现即重议 |
