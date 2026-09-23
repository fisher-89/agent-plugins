# desktop-agent-execution — desktop-app-shell 变更集

> exec 轨道第一条真实命令（`agent_start`）落地，翻转信号 #2 触发并重新裁决：仍不抽独立 app crate。本 delta 与 `desktop-agent-execution` 能力 spec 配套阅读。

## MODIFIED Requirements

### Requirement: Tauri command 查询双轨

dev-team SHALL 按 queries / exec 双轨组织 Tauri command：

- `commands/queries/`：MVP 实现三个命令——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）
- `commands/exec/`：执行轨道。空轨道状态由 desktop-agent-execution 结束，首批命令为 `agent_start`（agent 执行）与 `agent_runs` / `agent_run_events`（run 重放查询）。轨道纪律升级为：MUST NOT 出现空壳 Executor 类 trait（agent 执行的抽象由 `core/agent` 的 `AgentRunner` 承担）；后续 workspace 写文件等执行命令落此轨道时按各自 proposal 定形

每个查询 command SHALL 是无状态薄包装：参数 → core 函数 → DTO 返回，MUST NOT 在 command 层持有或缓存 workspace 状态；所有 workspace 状态访问 SHALL 只经 workflow / foundation 的 core 函数。DTO SHALL 区分 Query Result 与 Command Result 形态。

#### Scenario: 查询命令薄包装

- **WHEN** 前端 invoke `list_changes` / `get_change_detail` / `read_artifact`
- **THEN** command 仅做参数转换并调用 core 查询函数，返回 DTO，自身无状态

#### Scenario: exec 轨道承载 agent 首批命令

- **WHEN** 检查 `commands/exec/`
- **THEN** `agent_start` / `agent_runs` / `agent_run_events` 三命令落地，无空壳 Executor trait
- **AND** workspace 写文件未在此轨道实现（仍待后续变更定形）

#### Scenario: 状态访问收敛 core

- **WHEN** 审查 command 层代码
- **THEN** 无直接文件系统访问，workspace 状态一律经 core 函数获取

### Requirement: Sidebar 壳层布局

壳态（存在已打开 workspace）SHALL 以 shadcn Sidebar 块承载导航壳：`SidebarProvider` 包裹 `AppSidebar`（workspace 清单侧栏）与 `SidebarInset`（主内容区）；main 区 max-width 1100px 居中布局 SHALL 在 `SidebarInset` 内维持（从全宽 body 移入 inset，形态不变）。

header SHALL 瘦身为终态：折叠钮 + 标题（Desktop Terminal）+ 版本/更新指示（`UpdateIndicator` 含「重试更新」按钮语义不变）；workspace `select`、移除、刷新控件 MUST NOT 留在 header。

侧栏 SHALL 采用 `collapsible="icon"`：折叠态仅图标并经 Tooltip 补足信息；SHALL 接受窗口 < 768px 时 `use-mobile` 触发的 Sheet 抽屉第三态与内建 Ctrl/Cmd+B 折叠快捷键；折叠态持久化方式（localStorage vs 会话内 state）由 design 定夺。sidebar MUST NOT 引入路由：列表 ↔ 详情视图切换维持 `ChangeView` 本地 state。

侧栏 SHALL 增设页面导航组：[变更] [Agent 调试] 两个页面入口（AppSidebar 首次出现非 workspace 入口语义）；点击切换顶层视图，切换维持本地 state、MUST NOT 引入路由；workspace 清单组语义不变。Agent 调试页不依赖 change 选中状态，切换页面 MUST NOT 触发 change 取数。

欢迎态（root 为 null）MUST NOT 挂载 `SidebarProvider` / `AppSidebar`：`WelcomeView` 维持全屏现状；Toaster（sonner）SHALL 在 App 根挂载一次，欢迎态与壳态都覆盖。

#### Scenario: 壳态挂载与 header 终态

- **WHEN** workspace 清单非空、应用处于壳态
- **THEN** `SidebarProvider` / `AppSidebar` / `SidebarInset` 渲染，main 区 max-width 1100px 居中位于 inset 内
- **AND** header 仅含折叠钮、标题、版本/更新指示，无 `combobox`、移除、刷新控件

#### Scenario: 页面导航组切换

- **WHEN** 用户点击侧栏页面导航组的「Agent 调试」
- **THEN** 主内容区切至 AgentDebugView，无 router 依赖，workspace 清单组仍在
- **AND** 点回「变更」恢复 change 视图（选中状态保持策略 design 定夺）

#### Scenario: 欢迎态隔离

- **WHEN** `root` 为 null（清单为空或加载失败后无根）
- **THEN** 页面无 `SidebarProvider` / `AppSidebar` DOM，`WelcomeView` 全屏呈现
- **AND** Toaster 已挂载（欢迎态下添加失败同样可 toast）

#### Scenario: 折叠交互

- **WHEN** 用户点击折叠钮或按下 Ctrl/Cmd+B
- **THEN** 侧栏进入 icon-only 态，悬停清单项出 Tooltip；窗口宽度 < 768px 时侧栏以 Sheet 抽屉呈现

#### Scenario: 不引入路由

- **WHEN** 检查 desktop 依赖与视图代码
- **THEN** 无 router 依赖，列表 ↔ 详情仍为 `ChangeView` 本地 state（`selectedChange`），Agent 调试页切换同为本地 state

### Requirement: React 前端刷新取数模型

前端 SHALL 以 React + TS 实现，取数收在 hooks（`useChangeList` / `useChangeDetail` / `useWorkspaces`）内：由用户显式刷新动作触发 invoke；`useWorkspaces` 为唯一例外——启动时自动触发一次以支撑自动恢复，此后仍由用户动作触发。MUST NOT 实现文件 watch、后台轮询或事件订阅。例外（desktop-agent-execution）：agent 调试页的运行事件经 Tauri Channel 实时订阅——该订阅属执行流通道（`agent_start` 命令作用域流），不属于查询取数模型；agent 域查询取数（历史 run 列表 / 事件重放）仍由用户显式动作触发 invoke。刷新 SHALL 覆盖两个层级：workspace 级（重取列表）与 change 级（重取当前详情）。未来替换为推送时 SHALL 仅改动 hooks 内部实现，视图层不感知取数方式。

视图 SHALL 至少呈现：change 列表（代际标注、按月分组）、phase 流水线（attempt / verdict / checklist 展开）、经 renderer 注册表渲染的产物区（含 markdown 文档与 tasks 进度）、workspace 清单（sidebar 侧栏）、Agent 调试页（事件时间线）。列表刷新入口 SHALL 位于清单页（`ChangeListView`）头部并保留 `disabled={loading}` 语义，MUST NOT 回迁 header。

#### Scenario: 刷新按钮触发重取

- **WHEN** 用户点击清单页头部的刷新按钮
- **THEN** hooks 重新 invoke 对应查询命令并更新视图，期间无自动轮询发生

#### Scenario: agent 实时流例外

- **WHEN** `agent_start` 运行中，调试页时间线逐事件更新
- **THEN** 事件经 Tauri Channel 推送到达；无文件 watch、无定时轮询
- **AND** 历史 run 列表与事件重放仍为用户显式触发的 invoke 查询

#### Scenario: 取数收口 hooks

- **WHEN** 审查视图组件代码
- **THEN** 组件不直接 invoke，取数统一经 hooks（change / workspace 域既有 hooks 与 agent 域新增 hooks）

#### Scenario: 无 watch 依赖

- **WHEN** 检查 desktop 依赖与前端代码
- **THEN** 无文件系统监听（notify 等）依赖、无定时器轮询逻辑

### Requirement: command body 纪律与 app 层微形态

dev-team SHALL NOT 抽独立 app 层 crate：command 即应用服务，维持既有双轨（queries / workspaces）加 exec 轨道（已由 agent 执行命令开通）的组织不变。作为补偿纪律，任何 Tauri command body SHALL 只允许三件事：

1. **参数转换**（IPC 入参 → 领域/store 入参）；
2. **调用**（core 函数或 store 操作）；
3. **错误映射**（领域/Store 错误 → `Err(String)`）。

command 层 MUST NOT 实现领域解释（属领域解释的编排 SHALL 下推 core）或跨边界协调（属跨边界协调的编排 SHALL 触发 app crate 决策，见下一 requirement），MUST NOT 以"先塞进命令里"的方式消化编排增长。

`commands/workspaces` 的 `*_inner(&Store)` 纯函数模式 SHALL 视为 app 层微形态（IPC 适配与纯逻辑的函数级分层）予以保留：将来抽 app crate 时 SHALL 将 inner 函数平移复用（函数边界升 crate 边界），MUST NOT 重写。

`commands/exec` 的 `run_agent()` 编排函数 SHALL 与 `*_inner` 同列 app 层微形态（`*_inner` 先例的进化）：`agent_start` 的 tee 协调（组装 runner → 事件流双 sink：Tauri Channel + store 落库 → run 状态收敛）收在编排函数内，命令体仍只做三件事。将来抽 app crate 时编排函数与 inner 函数 SHALL 一并平移复用，MUST NOT 重写。

#### Scenario: 新命令符合三件事

- **WHEN** 新增任一 Tauri command
- **THEN** body 为参数转换 + 调用 + 错误映射三段，无编排逻辑、无领域解释、无跨 store / fs 协调

#### Scenario: run_agent 编排缝保留

- **WHEN** 审查 `commands/exec` 实现
- **THEN** `agent_start` 命令体三段式，runner 组装与 tee 在 `run_agent()` 内，未被内联回命令体

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

信号 #2 已由 desktop-agent-execution 触发（exec 轨道首条真实命令 `agent_start` 落地），重新裁决为：**仍不抽独立 app crate**——编排以 `run_agent()` 微形态承接（编排单点、仍在一屏内，无跨 store + fs 多点协调），裁决落痕于本 spec；后续信号再次触发时按清单重议，MUST NOT 沿袭本次结论。

#### Scenario: 信号可机械判定

- **WHEN** 审查翻转信号清单
- **THEN** 每条信号可由代码状态直接判定（特定命令是否出现 / 命令体行数 / 复用需求是否提出），无需主观架构评价

#### Scenario: 信号 #2 裁决落痕

- **WHEN** 查阅本 spec 的翻转信号清单
- **THEN** #2 已由 desktop-agent-execution 触发与「仍不抽」裁决及其理由（`run_agent()` 微形态、协调单点）可考

#### Scenario: 信号触发即重议

- **WHEN** 任一翻转信号再次出现（如 phase lifecycle 命令进入 design）
- **THEN** 对应变更的 proposal 显式回应"是否抽 app crate"的重新决策，而不是默认沿袭"不建"

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `dev-team::commands::exec` | 执行轨道（已开通） | `agent_start`（三件事，编排收 `run_agent()`）/ `agent_runs` / `agent_run_events`（薄包装）；`Result<T, String>`；无空壳 Executor trait |
| `run_agent()` 编排函数 | app 层微形态（`*_inner` 先例进化） | 组装 runner → tee 双 sink（Channel + store）→ 状态收敛；将来抽 crate 平移复用不重写 |
| `packages/desktop/src/App.tsx` | 顶层视图切换 | changes \| agent 本地 state，无路由；壳布局既有契约不变 |
| `packages/desktop/src/components/AppSidebar.tsx` | 页面导航组 + workspace 清单 | [变更] [Agent 调试] 页面入口；workspace 清单组语义不变 |
| agent 域前端 hooks（新） | 执行流订阅 + 查询 | Channel 实时订阅（查询取数模型的显式例外）；历史 run 列表 / 重放为显式 invoke |
| app crate 翻转信号 | 决策触发器 | 五条信号；#2 已触发、裁决「仍不抽」落痕；后续触发按清单重议 |
