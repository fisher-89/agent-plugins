# desktop-page-routing Specification

## Purpose

定义 desktop 前端壳态的页面路由契约：react-router（HashRouter）路由表、change 选中态 URL 化、Sidebar 页面导航 NavLink 化、欢迎态路由隔离，以及路由化对既有集成语义（取数模型、workspace 命令、选中重置）的保留与管线合规。

## Requirements

### Requirement: 路由表与 Router 选型

desktop 前端壳态页面导航 SHALL 以 react-router（v7 declarative 模式，`react-router` 单包）+ HashRouter 承载。路由表 SHALL 为：

- `/` → 重定向 `/changes`（replace 语义由 design 定夺）
- `/changes` → 变更清单页
- `/changes/:name` → 变更详情页（`name` 为路由参数）
- `/agent` → Agent 调试页
- 其余未知路径 → 重定向 `/changes` 兜底

Router 类型 SHALL 为 HashRouter：Tauri 生产构建经自定义协议以静态资源服务，BrowserRouter 的深链 / 刷新 SHALL NOT 采用（无协议层 fallback 配置）。MUST NOT 引入第二路由库。路由表声明 SHALL 独立成组件，MUST NOT 突破 `max-lines-per-function: 50` 管线约束。

#### Scenario: 未知路径回退清单

- **WHEN** 应用处于壳态且 URL 命中路由表之外的路径
- **THEN** 视图落在变更清单页，不渲染空白或崩溃

#### Scenario: 根路径直达清单

- **WHEN** 壳态启动且 URL 为 `/`
- **THEN** 视图落在 `/changes` 变更清单页

#### Scenario: 详情按路由参数渲染

- **WHEN** URL 为 `/changes/<name>` 且该 change 存在
- **THEN** 渲染 ChangeDetailView，`get_change_detail` 以 URL 中的 `name` 参数发起 invoke

### Requirement: change 选中态 URL 化

壳态下 change 选中状态 SHALL 由路由参数承载，MUST NOT 保留独立的 `selectedChange` 本地 state 与路由双轨并存：

- 清单行点击 SHALL 导航至 `/changes/<name>`
- 详情页数据参数 SHALL 来自路由参数（`useParams` 或等价 API），`useChangeDetail` hook 本体契约不变
- 详情返回 SHALL 显式导航至 `/changes`，MUST NOT 使用 `navigate(-1)` 类历史栈回退（落点不确定）
- workspace 切换或移除当前根 SHALL 导航至 `/changes`（URL 无 `:name` 段），与现行「切换清空 change 选中」语义一致

#### Scenario: 清单行点击进入详情路由

- **WHEN** 用户点击清单中名为 `add-feature` 的 change 行
- **THEN** URL 变为 `/changes/add-feature`，详情视图渲染且 `get_change_detail` 恰以 `{ change: 'add-feature' }` 参数发起一次

#### Scenario: 返回落清单

- **WHEN** 用户在详情页触发返回操作
- **THEN** URL 变为 `/changes`，清单视图呈现，无 `:name` 段残留

#### Scenario: workspace 切换清选中

- **WHEN** 用户在 `/changes/<name>` 详情页点击 sidebar 另一 workspace 清单项
- **THEN** URL 落在 `/changes`（无 `:name` 段），清单以新根重取，旧 change 详情不呈现

### Requirement: Sidebar 页面导航 NavLink 化

`AppSidebar` 页面导航组 SHALL 以 `NavLink`（或等价路由感知组件）承载，active 态 SHALL 由当前 URL 派生，MUST NOT 经独立 `page` state 或 `onPageChange` 回调同步。`TopPage` 类型导出 SHALL 删除。测试挂钩 `data-testid="nav-changes"` / `data-testid="nav-agent"` SHALL 保持不变。

#### Scenario: active 态由 URL 派生

- **WHEN** URL 为 `/agent`
- **THEN** `nav-agent` 呈现 active 态，`nav-changes` 非 active；反向同理
- **AND** 代码中无 `TopPage` / `onPageChange` 残留（knip 通过）

#### Scenario: 导航 testid 稳定

- **WHEN** 检查 `AppSidebar` 渲染产物
- **THEN** `nav-changes` / `nav-agent` testid 存在且语义与路由化前一致

### Requirement: 欢迎态路由隔离

Router SHALL 仅在壳态（`root` 非 null）挂载：欢迎态（`root === null`）MUST NOT 渲染路由出口与壳 DOM（`SidebarProvider` / `AppSidebar`），`WelcomeView` 维持全屏。MUST NOT 引入 `/welcome` 路由将欢迎态纳入路由树。

#### Scenario: 欢迎态无路由出口

- **WHEN** `root === null`（清单为空或加载失败后无根）
- **THEN** 页面无路由出口 / `SidebarProvider` / `AppSidebar` DOM，`WelcomeView` 全屏呈现
- **AND** `nav-changes` / `nav-agent` 不在场

### Requirement: 既有集成语义保留

路由化 MUST NOT 改变既有可观察行为：

- 切页往返 MUST NOT 重发 `list_workspaces` / `list_changes`（清单数据驻留 App 层，查询仍显式触发）
- 导航点击 MUST NOT 触发任何 workspace 命令（`add_workspace` / `remove_workspace` 等）
- 进入详情 → 切 Agent 页 → 切回：选中随 URL 消失，显示清单，`get_change_detail` MUST NOT 以旧选中重发（现行 D10 语义）
- 刷新取数模型不变：无文件 watch、无定时轮询，取数收口 hooks
- `desktop-app-shell` 的错误呈现双轨、header 终态、折叠交互、workspace 选择契约不受影响

#### Scenario: 切页不重发查询

- **WHEN** 用户在变更页与 Agent 页之间往返切换
- **THEN** `list_workspaces` 与 `list_changes` 调用次数不增加，清单数据照常呈现

#### Scenario: 导航零 workspace 命令

- **WHEN** 用户点击 `nav-agent` / `nav-changes`
- **THEN** `list_workspaces` / `add_workspace` / `remove_workspace` 调用次数均不变

#### Scenario: 选中重置语义保持

- **WHEN** 用户进入 change 详情后切至 Agent 页再切回变更页
- **THEN** 显示清单而非旧详情，`get_change_detail` 不以旧选中再次发起

### Requirement: 管线合规

路由化后 desktop 前端 SHALL 维持全管线通过：`vp check --fix`（fmt / lint，含 `max-lines-per-function: 50`）、knip（无 `TopPage` 等未用导出残留）、`vp test` 全绿。测试 SHALL 不以样式类名作为查询挂钩（沿用 data-testid 纪律）；路由测试的挂载方式（MemoryRouter 包裹 vs 驱动真实 HashRouter）由 design 定夺并记录理由。

#### Scenario: 全管线通过

- **WHEN** 运行 `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test`
- **THEN** fmt / lint / knip / 测试全部通过，无路由化引入的豁免条目（vite.config.ts 与 knip.json 无新增 ignorePatterns）

#### Scenario: 无类名查询回潮

- **WHEN** 检索新增 / 改造的 `*.test.tsx`
- **THEN** 无 `querySelector` / `getElementsByClassName` 样式类名查询，挂钩均为 data-testid

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `react-router`（新依赖，`packages/desktop/package.json`） | 路由运行时 | v7 declarative 模式（`react-router` 单包）；HashRouter；无第二路由库 |
| `packages/desktop/src/App.tsx` | 路由表挂载 + 壳布局 | 路由表 `/`→`/changes` 重定向、`/changes`、`/changes/:name`、`/agent`、`*` 兜底；欢迎态 gate 在 Router 外；壳态 DOM 契约不变 |
| `packages/desktop/src/components/AppSidebar.tsx` | NavLink 页面导航组 | active 由 URL 派生；`TopPage` / `onPageChange` 删除；testid `nav-changes` / `nav-agent` 保持 |
| `packages/desktop/src/views/changes/ChangeView.tsx` | 选中态 ↔ 路由参数接线（溶解与否 design 定） | `useParams` 承载选中；返回显式 `navigate('/changes')`；workspace 切换落 `/changes` |
| `packages/desktop/src/hooks/useChangeDetail.ts` 等 hooks | 取数契约不变 | 入参来源由 state 改为路由参数，hook 本体不动；显式刷新模型不变 |
| 路由级测试（新，如 `src/__tests__/route_pages.test.tsx`） | 路由表 / 选中态 / 语义保留断言 | data-testid 挂钩；MemoryRouter vs HashRouter 挂载 design 定夺有据 |
