# desktop-app-shell 变更提案

## RENAMED Requirements

### RENAMED: Tauri command 查询双轨 → Tauri command 轨道组织

- **FROM**: `### Requirement: Tauri command 查询双轨` —— dev-team 按 queries / exec 双轨组织 Tauri command
- **TO**: `### Requirement: Tauri command 轨道组织` —— 内容如下：

dev-team SHALL 按 queries / exec / db 三轨组织 Tauri command：

- `commands/queries/`：MVP 实现三个命令——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）
- `commands/exec/`：执行轨道。空轨道状态由 desktop-agent-execution 结束，首批命令为 `agent_start`（agent 执行）与 `agent_runs` / `agent_run_events`（run 重放查询）。轨道纪律升级为：MUST NOT 出现空壳 Executor 类 trait（agent 执行的抽象由 `core/agent` 的 `AgentRunner` 承担）；后续 workspace 写文件等执行命令落此轨道时按各自 proposal 定形
- `commands/db/`：db 查看轨道（native-db-store-upgrade 开通），承载只读 db 检查命令 `db_models`（模型清单与计数）与 `db_records`（按模型分页扫描），经 `State<Store>` 调用 store 的记录信封 API（见 desktop-workspace-store「记录信封 API」）。轨道纪律：只读，MUST NOT 出现任何写命令；命令命名 design 可调

每个查询 command SHALL 是无状态薄包装：参数 → core 函数或 store 操作 → DTO 返回，MUST NOT 在 command 层持有或缓存 workspace 状态；所有 workspace 状态访问 SHALL 只经 workflow / foundation 的 core 函数。DTO SHALL 区分 Query Result 与 Command Result 形态。

#### Scenario: 查询命令薄包装

- **WHEN** 前端 invoke `list_changes` / `get_change_detail` / `read_artifact`
- **THEN** command 仅做参数转换并调用 core 查询函数，返回 DTO，自身无状态

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

## MODIFIED Requirements

### Requirement: Sidebar 壳层布局

壳态（存在已打开 workspace）SHALL 以 shadcn Sidebar 块承载导航壳：`SidebarProvider` 包裹 `AppSidebar`（workspace 清单侧栏）与 `SidebarInset`（主内容区）；main 区 max-width 1100px 居中布局 SHALL 在 `SidebarInset` 内维持（从全宽 body 移入 inset，形态不变）。

header SHALL 瘦身为终态：折叠钮 + 标题（Desktop Terminal）+ 版本/更新指示（`UpdateIndicator` 含「重试更新」按钮语义不变）；workspace `select`、移除、刷新控件 MUST NOT 留在 header。

侧栏 SHALL 采用 `collapsible="icon"`：折叠态仅图标并经 Tooltip 补足信息；SHALL 接受窗口 < 768px 时 `use-mobile` 触发的 Sheet 抽屉第三态与内建 Ctrl/Cmd+B 折叠快捷键；折叠态持久化方式（localStorage vs 会话内 state）由 design 定夺。sidebar MUST NOT 引入路由：列表 ↔ 详情视图切换维持 `ChangeView` 本地 state。

侧栏 SHALL 以两个导航组组织顶层视图入口：「页面」组承载数据视图入口 [变更]；「系统工具」组承载系统级工具入口 [Agent 调试]（自页面组平移）与 [DB 查看]（desktop-db-inspector，`TopPage` 增 `db` 变体）（AppSidebar 首次出现非 workspace 入口语义）。点击切换顶层视图，切换维持本地 state、MUST NOT 引入路由；workspace 清单组语义不变。系统工具页不依赖 change 选中状态，切换页面 MUST NOT 触发 change 取数。

欢迎态（root 为 null）MUST NOT 挂载 `SidebarProvider` / `AppSidebar`：`WelcomeView` 维持全屏现状——系统工具组随壳整体不挂载，DB 查看页仅壳态可达。Toaster（sonner）SHALL 在 App 根挂载一次，欢迎态与壳态都覆盖。

#### Scenario: 壳态挂载与 header 终态

- **WHEN** workspace 清单非空、应用处于壳态
- **THEN** `SidebarProvider` / `AppSidebar` / `SidebarInset` 渲染，main 区 max-width 1100px 居中位于 inset 内
- **AND** header 仅含折叠钮、标题、版本/更新指示，无 `combobox`、移除、刷新控件

#### Scenario: 导航组切换

- **WHEN** 用户点击「页面」组的「变更」或「系统工具」组的「Agent 调试」/「DB 查看」
- **THEN** 主内容区切至对应视图（ChangeView / AgentDebugView / DbInspectorView），无 router 依赖，workspace 清单组仍在
- **AND** 切换不触发 change 取数，视图状态保持策略 design 定夺

#### Scenario: 欢迎态隔离

- **WHEN** `root` 为 null（清单为空或加载失败后无根）
- **THEN** 页面无 `SidebarProvider` / `AppSidebar` DOM（含「页面」与「系统工具」两组），`WelcomeView` 全屏呈现，无 DB 查看入口可达
- **AND** Toaster 已挂载（欢迎态下添加失败同样可 toast）

#### Scenario: 折叠交互

- **WHEN** 用户点击折叠钮或按下 Ctrl/Cmd+B
- **THEN** 侧栏进入 icon-only 态，悬停清单项出 Tooltip；窗口宽度 < 768px 时侧栏以 Sheet 抽屉呈现

#### Scenario: 不引入路由

- **WHEN** 检查 desktop 依赖与视图代码
- **THEN** 无 router 依赖，列表 ↔ 详情仍为 `ChangeView` 本地 state（`selectedChange`），Agent 调试页与 DB 查看页切换同为本地 state

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `dev-team::commands::db`（新轨道） | db 查看命令轨道 | `db_models` / `db_records` 只读薄包装；`State<Store>` → 信封 API；`Result<T, String>`；命名 design 可调 |
| `packages/desktop/src/App.tsx` | 壳布局 + 顶层视图切换 | changes \| agent \| db 本地 state 切换（`TopPage` 增 `db` 变体），无路由；欢迎态不挂壳 |
| `packages/desktop/src/components/AppSidebar.tsx` | 双导航组 + workspace 清单侧栏 | 「页面」组 [变更]；「系统工具」组 [Agent 调试] [DB 查看]；本地 state 切换；workspace 清单组语义不变 |
