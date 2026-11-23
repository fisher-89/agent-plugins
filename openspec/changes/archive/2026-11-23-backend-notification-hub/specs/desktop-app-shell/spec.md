# desktop-app-shell Specification（delta: backend-notification-hub）

## Purpose

本 delta 只**动两处结构面**加**一处例外登记**，不改命令轨道的三件事纪律、`*_inner` 微形态、`run_agent()` 编排缝、错误呈现双轨、执行线程纪律与其余壳层陈述（逐字不动）：

1. 新增第四条命令轨道 `commands/notifications/`（通知命令面 + 写命令落本轨道而非只读的 db 轨道）；
2. header 终态由「折叠钮 + 标题 + 版本/更新指示」扩为含**通知铃铛**（收件箱呈现面归 `desktop-notification-hub`，此处只登记 header 有它）；
3. 「React 前端刷新取数模型」补一处 Channel 例外（通知订阅，非查询取数），并写明其语义归 `desktop-notification-hub`。

本 delta 不含：任何命令签名 / 返回 / 错误文案变化（除新增五命令本身）、Toaster 单点挂载与路由表的改动、执行线程纪律的适用范围变化。

## MODIFIED Requirements

### Requirement: Tauri command 轨道组织

dev-team SHALL 按 changes / exec / db / notifications 四轨组织 Tauri command（queries 轨道已由 move-queries-command-to-change 消解：三命令全部为 change 域读面，并入 change 域命令组，见下；workspace 注册命令另成一组，见「workspace 注册命令轨道」）：

- `commands/changes/`：change 域命令组（读 + 记录面同组，沿 explores 组先例）——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）三读命令与 `create_change`（新建 change，见 desktop-change-create）共组；`commands/queries/` 轨道 MUST NOT 残留（目录、模块声明或 `all_commands!` 登记路径任一存在即违例）；组内 blank root 双口径并存且 MUST NOT 互换——读命令走空结果语义（空清单 / `None`）、写命令走显式 `Err`（写无空结果语义）
- `commands/exec/`：执行轨道。空轨道状态由 desktop-agent-execution 结束，首批命令为 `agent_start`（agent 执行）与 `agent_runs` / `agent_run_events`（run 重放查询）。轨道纪律升级为：MUST NOT 出现空壳 Executor 类 trait（agent 执行的抽象由 `core/agent` 的 `AgentRunner` 承担）；后续 workspace 写文件等执行命令落此轨道时按各自 proposal 定形
- `commands/db/`：db 查看轨道（native-db-store-upgrade 开通），承载只读 db 检查命令 `db_models`（模型清单与计数）与 `db_records`（按模型分页扫描），经 `State<Store>` 调用 store 的记录信封 API（见 desktop-workspace-store「记录信封 API」）。轨道纪律：只读，MUST NOT 出现任何写命令；命令命名 design 可调
- `commands/notifications/`：通知轨道（desktop-notification-hub 开通），承载通知命令面 `notification_subscribe`（订阅，形参 `Channel<Notification>`）/ `notification_list` / `notification_mark_read` / `notification_mark_all_read` / `notification_record`（前端反馈轨落库）。轨道纪律：**读写同组**——通知是旁路业务面，写命令 MUST NOT 并入只读的 db 查看轨道；各命令的语义、返回形态与过滤口径归 desktop-notification-hub 持有，本条只登记轨道存在与归属

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

#### Scenario: 通知轨道读写同组且不并入 db 轨

- **WHEN** 检查 `src/commands/` 目录树、`all_commands!` 清单与 `commands/db/` 的命令集
- **THEN** 五条通知命令（subscribe / list / mark_read / mark_all_read / record）均实现并登记于 `commands/notifications/`，`all_commands!` 尾部按登记序含这五条；`commands/db/` 内仍只有 `db_models` / `db_records` 两读命令、零写命令

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

### Requirement: Sidebar 壳层布局

壳态（存在已打开 workspace）SHALL 以 shadcn Sidebar 块承载导航壳：`SidebarProvider` 包裹 `AppSidebar`（workspace 清单侧栏）与主内容区 `main`（2026-10-10 回溯修订：`SidebarInset` 包装撤编，main 直承 `flex min-h-0 flex-col` 高度链；路由内容包裹层为 shadcn `ScrollArea`——`mx-auto w-full flex-1` 居中撑满、内容在其视口内滚动）；main 区居中布局 SHALL 在主内容区内维持（从全宽 body 移入，形态不变）。

header SHALL 瘦身为终态：折叠钮 + 标题（Desktop Terminal）+ 版本/更新指示（`UpdateIndicator` 含「重试更新」按钮语义不变）+ **通知铃铛**（未读徽标与抽屉收件箱的呈现面归 desktop-notification-hub，本条只登记 header 有它）；workspace `select`、移除、刷新控件 MUST NOT 留在 header。

侧栏 SHALL 采用 `collapsible="icon"`：折叠态仅图标并经 Tooltip 补足信息；SHALL 支持内建 Ctrl/Cmd+B 折叠快捷键；折叠态持久化方式（localStorage vs 会话内 state）由 design 定夺。系统 PC-only：极小分辨率不适配，窗口最小尺寸 SHALL 由 `tauri.conf.json` 限定（minWidth 900 / minHeight 600）。sidebar 导航 SHALL 路由化：页面导航组与列表 ↔ 详情切换均由路由承载（路由表与选中态 URL 化契约见 desktop-page-routing 能力）。

侧栏 SHALL 以两个导航组组织顶层视图入口：「页面」组承载数据视图入口 [变更]；「系统工具」组承载系统级工具入口 [Agent 调试]（自页面组平移）与 [数据库]（desktop-db-inspector，`TopPage` 增 `db` 变体）（AppSidebar 首次出现非 workspace 入口语义）。点击经路由切换顶层视图（`/changes` ↔ `/agent`，NavLink 化契约见 desktop-page-routing 能力）；workspace 清单组语义不变。系统工具页不依赖 change 选中状态，切换页面 MUST NOT 触发 change 取数。

欢迎态（root 为 null）MUST NOT 挂载 `SidebarProvider` / `AppSidebar`：`WelcomeView` 维持全屏现状——系统工具组随壳整体不挂载，数据库页仅壳态可达。Toaster（sonner）SHALL 在 App 根挂载一次，欢迎态与壳态都覆盖。

#### Scenario: 壳态挂载与 header 终态

- **WHEN** workspace 清单非空、应用处于壳态
- **THEN** `SidebarProvider` / `AppSidebar` / 主内容区 `main`（`sidebar-wrapper` 直子级）渲染，路由内容经 `ScrollArea` 根容器（`data-slot="scroll-area"`）居中撑满承载
- **AND** header 仅含折叠钮、标题、版本/更新指示与通知铃铛，无 `combobox`、移除、刷新控件；铃铛随壳挂载（欢迎态不渲染，见「欢迎态隔离」）

#### Scenario: 导航组切换

- **WHEN** 用户点击「页面」组的「变更」或「系统工具」组的「Agent 调试」/「数据库」
- **THEN** 主内容区切至对应视图（变更页 ChangeListView / ChangeDetailView、AgentDebugView、DbInspectorView），依赖 router，workspace 清单组仍在
- **AND** 点回「变更」经 `/changes` 路由恢复 change 视图；选中不保留——选中随 URL 消失显示清单（desktop-page-routing 能力「选中重置语义保持」）

#### Scenario: 欢迎态隔离

- **WHEN** `root` 为 null（清单为空或加载失败后无根）
- **THEN** 页面无 `SidebarProvider` / `AppSidebar` DOM（含「页面」与「系统工具」两组），`WelcomeView` 全屏呈现，无 数据库入口可达
- **AND** Toaster 已挂载（欢迎态下添加失败同样可 toast）

#### Scenario: 折叠交互

- **WHEN** 用户点击折叠钮或按下 Ctrl/Cmd+B
- **THEN** 侧栏进入 icon-only 态，悬停清单项出 Tooltip

#### Scenario: 路由化导航

- **WHEN** 检查 desktop 依赖与视图代码
- **THEN** 以 react-router HashRouter 承载导航，列表 ↔ 详情为 `/changes` ↔ `/changes/:id` 路由，Agent 调试页为 `/agent`，数据库页为 `/db`
- **AND** 无 `TopPage` 本地 state / `onPageChange` 回调残留，无第二路由库

### Requirement: React 前端刷新取数模型

前端 SHALL 以 React + TS 实现，取数收在 hooks（`useChangeList` / `useChangeDetail` / `useWorkspaces`）内：由用户显式刷新动作触发 invoke；`useWorkspaces` 为唯一例外——启动时自动触发一次以支撑自动恢复，此后仍由用户动作触发。MUST NOT 实现文件 watch、后台轮询或事件订阅。例外（desktop-agent-execution）：agent 调试页的运行事件经 Tauri Channel 实时订阅——该订阅属执行流通道（`agent_start` 命令作用域流），不属于查询取数模型；agent 域查询取数（历史 run 列表 / 事件重放）仍由用户显式动作触发 invoke。例外（desktop-notification-hub）：壳层的通知收件箱经 Tauri Channel 订阅后端通知事件的实时增量（并取回初值未读计数）——该订阅同属执行流通道（与 agent 执行流同列，且与 change run 的变更通知通道分属不同驱动源），不是查询取数；其时序、初值、失败面与过滤口径归该能力持有，本条只登记例外存在；该例外 MUST NOT 被读作放开后台轮询，通知列表仍由抽屉打开时的显式 invoke 拉取。刷新 SHALL 覆盖两个层级：workspace 级（重取列表）与 change 级（重取当前详情）。未来替换为推送时 SHALL 仅改动 hooks 内部实现，视图层不感知取数方式。

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

#### Scenario: 通知订阅是通道例外且非取数

- **WHEN** 壳态挂载时 `notification_subscribe` 建立 `Channel<Notification>` 并取回初值未读计数
- **THEN** 该订阅只承载后端通知事件的实时增量与初值，不是查询取数（历史列表仍由抽屉打开时的 `notification_list` 显式拉取）；订阅失败不阻断壳渲染；全仓仍无定时器轮询与文件系统监听依赖，`hooks/use-file-watch-signal.ts` 零 diff

## Module Contract

### 模块增量：通知轨道与 header 铃铛（backend-notification-hub）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `packages/desktop/src-tauri/src/commands/notifications/`（新轨道） | 通知命令面（读写同组） | `notification_subscribe`（`Channel<Notification>` 形参；先订阅后读未读计数）/ `notification_list` / `notification_mark_read` / `notification_mark_all_read` / `notification_record`；`Result<T, String>`；命令体三件事；语义与过滤口径归 desktop-notification-hub |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 命令登记面 | `pub mod notifications;` + `all_commands!` 尾部按登记序追加五条（既有条目零位移） |
| `packages/desktop/src/app.tsx`（`ShellHeader`） | 页壳顶栏 | 终态 = 折叠钮 + 标题 + 版本/更新指示 + 通知铃铛；欢迎态不渲染 header（`root === null` 走 `WelcomeView`）；`<Toaster />` 与 `AppRoutes` 零改动 |
| `packages/desktop/src/components/notification-bell.tsx`（新） | 顶栏铃铛 + 抽屉收件箱 | 未读徽标（99+）+ `StandardDrawer` 列表；文件落 `App.Frontend.Shell` 的 `path` 之内；跨根点击先 `select` 再 navigate |
| `packages/desktop/src/hooks/use-notifications.ts`（新） | 通知订阅与收件箱取数 | `Channel<Notification>` 构造 1 处 + `notification_subscribe` 调用 1 处；`Info` 不弹 toast；订阅失败静默、列表失败 inline |

### 零 delta 的陈述面

| 面 | 理由 |
|---|---|
| 三件事纪律 / `*_inner` 微形态 / `run_agent()` 编排缝 | 五条新命令同守（薄包装 + 错误映射），无第二形态 |
| 错误呈现双轨 | 动作失败仍走 toast（实现形态经 `lib/notify.ts` 门面，可观察行为不变）；订阅失败静默、列表失败 inline 均按双轨语义分流 |
| 命令执行线程纪律 | 五命令均为毫秒级读写（单行 upsert / 分页扫描 / 计数），不在「批量子系统 IO」适用面内 |
| workspace 选择 / 侧栏导航组 / 命令 body 纪律 | 零改动 |
