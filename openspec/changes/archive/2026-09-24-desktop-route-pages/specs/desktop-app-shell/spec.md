# desktop-route-pages — desktop-app-shell 变更集

> 修订「Sidebar 壳层布局」requirement：翻转「MUST NOT 引入路由」决策为路由化导航。路由表、选中态 URL 化、导航 NavLink 化等新契约由新增能力 desktop-page-routing 承接，本 delta 仅替换壳层 requirement 中的路由相关条款与其 scenario。其余 requirement（workspace 选择、刷新取数模型、错误呈现双轨、命令轨道等）不变。

## MODIFIED Requirements

### Requirement: Sidebar 壳层布局

壳态（存在已打开 workspace）SHALL 以 shadcn Sidebar 块承载导航壳：`SidebarProvider` 包裹 `AppSidebar`（workspace 清单侧栏）与 `SidebarInset`（主内容区）；main 区 max-width 1100px 居中布局 SHALL 在 `SidebarInset` 内维持（从全宽 body 移入 inset，形态不变）。

header SHALL 瘦身为终态：折叠钮 + 标题（Desktop Terminal）+ 版本/更新指示（`UpdateIndicator` 含「重试更新」按钮语义不变）；workspace `select`、移除、刷新控件 MUST NOT 留在 header。

侧栏 SHALL 采用 `collapsible="icon"`：折叠态仅图标并经 Tooltip 补足信息；SHALL 支持内建 Ctrl/Cmd+B 折叠快捷键；折叠态持久化方式（localStorage vs 会话内 state）由 design 定夺。系统 PC-only：极小分辨率不适配，窗口最小尺寸 SHALL 由 `tauri.conf.json` 限定（minWidth 900 / minHeight 600）。sidebar 导航 SHALL 路由化：页面导航组与列表 ↔ 详情切换均由路由承载（路由表与选中态 URL 化契约见 desktop-page-routing 能力）。

侧栏 SHALL 增设页面导航组：[变更] [Agent 调试] 两个页面入口（AppSidebar 首次出现非 workspace 入口语义）；点击经路由切换顶层视图（`/changes` ↔ `/agent`，NavLink 化契约见 desktop-page-routing 能力）；workspace 清单组语义不变。Agent 调试页不依赖 change 选中状态，切换页面 MUST NOT 触发 change 取数。

欢迎态（root 为 null）MUST NOT 挂载 `SidebarProvider` / `AppSidebar`：`WelcomeView` 维持全屏现状；Toaster（sonner）SHALL 在 App 根挂载一次，欢迎态与壳态都覆盖。

#### Scenario: 壳态挂载与 header 终态

- **WHEN** workspace 清单非空、应用处于壳态
- **THEN** `SidebarProvider` / `AppSidebar` / `SidebarInset` 渲染，main 区 max-width 1100px 居中位于 inset 内
- **AND** header 仅含折叠钮、标题、版本/更新指示，无 `combobox`、移除、刷新控件

#### Scenario: 页面导航组切换

- **WHEN** 用户点击侧栏页面导航组的「Agent 调试」
- **THEN** 主内容区切至 AgentDebugView（`/agent` 路由），workspace 清单组仍在
- **AND** 点回「变更」经 `/changes` 路由恢复 change 视图；选中不保留——选中随 URL 消失显示清单（desktop-page-routing 能力「选中重置语义保持」）

#### Scenario: 欢迎态隔离

- **WHEN** `root` 为 null（清单为空或加载失败后无根）
- **THEN** 页面无 `SidebarProvider` / `AppSidebar` DOM，`WelcomeView` 全屏呈现
- **AND** Toaster 已挂载（欢迎态下添加失败同样可 toast）

#### Scenario: 折叠交互

- **WHEN** 用户点击折叠钮或按下 Ctrl/Cmd+B
- **THEN** 侧栏进入 icon-only 态，悬停清单项出 Tooltip

#### Scenario: 路由化导航

- **WHEN** 检查 desktop 依赖与视图代码
- **THEN** 以 react-router HashRouter 承载导航，列表 ↔ 详情为 `/changes` ↔ `/changes/:name` 路由，Agent 调试页为 `/agent`
- **AND** 无 `TopPage` 本地 state / `onPageChange` 回调残留，无第二路由库

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/App.tsx` | 壳布局 + 路由表挂载 | `SidebarProvider` + `AppSidebar` + `SidebarInset`；header 终态（折叠钮/标题/版本更新）；Toaster App 根挂载一次；欢迎态不挂壳；路由表 `/changes` / `/changes/:name` / `/agent`（`page` state 移除，契约见 desktop-page-routing） |
| `packages/desktop/src/components/AppSidebar.tsx` | NavLink 页面导航组 + workspace 清单侧栏 | NavLink 导航（active 由 URL 派生，`TopPage` / `onPageChange` 删除，testid `nav-changes` / `nav-agent` 保持）；`SidebarMenuButton` 清单项（点击本地切换 / 副文本 testid 区分同名 / Tooltip 完整 root）；`SidebarGroupAction` 添加流；`ContextMenu` 右键移除 |
