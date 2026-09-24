# desktop-page-routing Specification（delta）

## MODIFIED Requirements

### Requirement: 路由表与 Router 选型

desktop 前端壳态页面导航 SHALL 以 react-router（v7 declarative 模式，`react-router` 单包）+ HashRouter 承载。路由表 SHALL 为：

- `/` → 重定向 `/changes`（replace 语义由 design 定夺）
- `/changes` → 变更清单页
- `/changes/:name` → 变更详情页（`name` 为路由参数）
- `/agent` → Agent 调试页
- `/explores` → 探索清单页
- `/explores/:name` → 探索详情页（`name` 为路由参数）
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

#### Scenario: 探索详情按路由参数渲染

- **WHEN** URL 为 `/explores/<name>` 且该 explore 记录存在
- **THEN** 渲染探索详情双栏视图，读取与会话还原以 URL 中的 `name` 参数发起

### Requirement: Sidebar 页面导航 NavLink 化

`AppSidebar` 页面导航组 SHALL 以 `NavLink`（或等价路由感知组件）承载，active 态 SHALL 由当前 URL 派生，MUST NOT 经独立 `page` state 或 `onPageChange` 回调同步。「页面」组 SHALL 为 [变更] [探索] 两项（探索与变更同属 workspace 域内容页；`/explores` 与 `/explores/:name` 均使探索项 active）。`TopPage` 类型导出 SHALL 保持删除状态。测试挂钩 `data-testid="nav-changes"` / `data-testid="nav-agent"` SHALL 保持不变，`data-testid="nav-explores"` 随本变更新增。

#### Scenario: active 态由 URL 派生

- **WHEN** URL 为 `/agent`
- **THEN** `nav-agent` 呈现 active 态，`nav-changes` 非 active；反向同理
- **AND** 代码中无 `TopPage` / `onPageChange` 残留（knip 通过）

#### Scenario: 探索入口 active 态

- **WHEN** URL 分别为 `/explores` 与 `/explores/<name>`
- **THEN** `nav-explores` 均呈现 active 态，`nav-changes` 非 active

#### Scenario: 导航 testid 稳定

- **WHEN** 检查 `AppSidebar` 渲染产物
- **THEN** `nav-changes` / `nav-agent` testid 存在且语义与路由化前一致，`nav-explores` 在「页面」组内可达

### Requirement: 既有集成语义保留

路由化 MUST NOT 改变既有可观察行为：

- 切页往返 MUST NOT 重发 `list_workspaces` / `list_changes`（清单数据驻留 App 层，查询仍显式触发）
- 导航点击 MUST NOT 触发任何 workspace 命令（`add_workspace` / `remove_workspace` 等）
- 进入详情 → 切 Agent 页 → 切回：选中随 URL 消失，显示清单，`get_change_detail` MUST NOT 以旧选中重发（现行 D10 语义）
- 刷新取数模型不变：取数收口 hooks、无定时轮询；文件 watch 为 desktop-file-watch 认可的唯一推送例外（失效信号通道，非数据通道），既有页面的取数语义不变
- `desktop-app-shell` 的错误呈现双轨、header 终态、折叠交互、workspace 选择契约不受影响

#### Scenario: 切页不重发查询

- **WHEN** 用户在变更页与 Agent 页之间往返切换
- **THEN** `list_workspaces` 与 `list_changes` 调用次数不增加，清单数据照常呈现

#### Scenario: 导航零 workspace 命令

- **WHEN** 用户点击 `nav-agent` / `nav-changes` / `nav-explores`
- **THEN** `list_workspaces` / `add_workspace` / `remove_workspace` 调用次数均不变

#### Scenario: 选中重置语义保持

- **WHEN** 用户进入 change 详情后切至 Agent 页再切回变更页
- **THEN** 显示清单而非旧详情，`get_change_detail` 不以旧选中再次发起
