# desktop-page-routing Specification（delta: archi-graph-doc-agent）

## MODIFIED Requirements

### Requirement: 路由表与 Router 选型

desktop 前端壳态页面导航 SHALL 以 react-router（v7 declarative 模式，`react-router` 单包）+ HashRouter 承载。路由表 SHALL 为：

- `/` → 重定向 `/changes`（replace 语义由 design 定夺）
- `/changes` → 变更清单页
- `/changes/:id` → 变更详情页（`id` 为 change id 路由参数——身份锚，见 desktop-change-state-store「change 身份锚与 name 属性分离」）
- `/info` → 工作区基础信息页（当前根的代码统计，desktop-workspace-code-stats）
- `/config` → 工作区配置页（当前根的 config.json 只读解析，desktop-workspace-config）
- `/agent` → Agent 调试页
- `/explores` → 探索清单页
- `/explores/:name` → 探索详情页（`name` 为路由参数）
- `/db` → 数据库检查页（desktop-db-inspector，现状补记）
- `/agents` → 全局 Agent 管理页（desktop-agent-management，不依赖当前 workspace root）
- `/architecture` → 架构页（当前工作根的模型图与模块文档关联，desktop-archi-canvas「单一画布组件与两处挂载点」；根不来自 URL 参数）
- 其余未知路径 → 重定向 `/changes` 兜底

Router 类型 SHALL 为 HashRouter：Tauri 生产构建经自定义协议以静态资源服务，BrowserRouter 的深链 / 刷新 SHALL NOT 采用（无协议层 fallback 配置）。MUST NOT 引入第二路由库。路由表声明 SHALL 独立成组件，MUST NOT 突破 `max-lines-per-function: 50` 管线约束。

#### Scenario: 未知路径回退清单

- **WHEN** 应用处于壳态且 URL 命中路由表之外的路径
- **THEN** 视图落在变更清单页，不渲染空白或崩溃

#### Scenario: 根路径直达清单

- **WHEN** 壳态启动且 URL 为 `/`
- **THEN** 视图落在 `/changes` 变更清单页

#### Scenario: 详情按路由参数渲染

- **WHEN** URL 为 `/changes/<id>` 且该 id 在 db 有记录
- **THEN** 渲染 ChangeDetailView，`get_change_detail` 以 URL 中的 `id` 参数发起 invoke（归档 change 照常全状态面，见 desktop-change-queries）

#### Scenario: 探索详情按路由参数渲染

- **WHEN** URL 为 `/explores/<name>` 且该 explore 记录存在
- **THEN** 渲染探索详情双栏视图，读取与会话还原以 URL 中的 `name` 参数发起

#### Scenario: 基础信息页按当前根渲染

- **WHEN** URL 为 `/info` 且壳态存在当前工作区根
- **THEN** 渲染基础信息页，`code_stats` 以当前根与默认深度发起解析；根不来自 URL 参数（workspace 根为 sidebar 本地态，路由不新增根段）

#### Scenario: 配置页按当前根渲染

- **WHEN** URL 为 `/config` 且壳态存在当前工作区根
- **THEN** 渲染配置页，`workspace_config` 以当前根发起解析；根不来自 URL 参数（路由不新增根段），只读语义随 desktop-workspace-config 承载

#### Scenario: 管理页不依赖当前根

- **WHEN** URL 为 `/agents`
- **THEN** 渲染全局 Agent 管理页（Providers / Agents 两栏），页面取数不携带 workspace root 参数（全局语义）；导航至该页 MUST NOT 触发任何 workspace 命令

#### Scenario: 架构页按当前根渲染

- **WHEN** URL 为 `/architecture` 且壳态存在当前工作区根
- **THEN** 渲染架构页，画布数据与元素详情均以当前根发起取数；根不来自 URL 参数（路由不新增根段），只读与增量两种能力差异由挂载点决定（见 desktop-archi-canvas）

### Requirement: Sidebar 页面导航 NavLink 化

`AppSidebar` 页面导航组 SHALL 以 `NavLink`（或等价路由感知组件）承载，active 态 SHALL 由当前 URL 派生，MUST NOT 经独立 `page` state 或 `onPageChange` 回调同步。「页面」组 SHALL 为 [基础信息] [变更] [探索] [架构] [配置] 五项（五项同属 workspace 域内容页；`/explores` 与 `/explores/:name` 均使探索项 active，`/info` 使基础信息项 active，`/architecture` 使架构项 active，`/config` 使配置项 active；组内排序由 design 定夺）。「系统工具」组 SHALL 为 [Agent 调试] [Agent 管理] [数据库] 三项（[Agent 管理] 项 `data-testid="nav-agents"`，`/agents` 使其 active；组内排序由 design 定夺）。`TopPage` 类型导出 SHALL 保持删除状态。测试挂钩 `data-testid="nav-changes"` / `data-testid="nav-agent"` SHALL 保持不变，`data-testid="nav-explores"` 随探索页变更新增，`data-testid="nav-info"` 随基础信息页变更新增，`data-testid="nav-config"` 随配置页变更新增，`data-testid="nav-agents"` 随管理页变更新增，`data-testid="nav-architecture"` 随架构页变更新增。

#### Scenario: active 态由 URL 派生

- **WHEN** URL 为 `/agent`
- **THEN** `nav-agent` 呈现 active 态，`nav-changes` 非 active；反向同理
- **AND** 代码中无 `TopPage` / `onPageChange` 残留（knip 通过）

#### Scenario: 探索入口 active 态

- **WHEN** URL 分别为 `/explores` 与 `/explores/<name>`
- **THEN** `nav-explores` 均呈现 active 态，`nav-changes` 非 active

#### Scenario: 基础信息入口 active 态

- **WHEN** URL 为 `/info`
- **THEN** `nav-info` 呈现 active 态，`nav-changes` / `nav-explores` / `nav-config` 非 active

#### Scenario: 配置入口 active 态

- **WHEN** URL 为 `/config`
- **THEN** `nav-config` 呈现 active 态，`nav-changes` / `nav-explores` / `nav-info` 非 active

#### Scenario: 管理入口 active 态

- **WHEN** URL 为 `/agents`
- **THEN** `nav-agents` 呈现 active 态，`nav-agent` / `nav-changes` / `nav-config` 非 active

#### Scenario: 导航 testid 稳定

- **WHEN** 检查 `AppSidebar` 渲染产物
- **THEN** `nav-changes` / `nav-agent` testid 存在且语义与路由化前一致，`nav-explores` / `nav-info` / `nav-config` 在「页面」组内可达，`nav-agents` 在「系统工具」组内可达

#### Scenario: 架构入口 active 态

- **WHEN** URL 为 `/architecture`
- **THEN** `nav-architecture` 呈现 active 态，`nav-changes` / `nav-info` / `nav-explores` / `nav-config` 非 active；「系统工具」组各项亦非 active

#### Scenario: 导航 testid 稳定（含架构项）

- **WHEN** 检查 `AppSidebar` 渲染产物
- **THEN** `nav-changes` / `nav-agent` testid 存在且语义与路由化前一致，`nav-explores` / `nav-info` / `nav-config` / `nav-architecture` 在「页面」组内可达，`nav-agents` 在「系统工具」组内可达

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/routes.tsx` | 路由表增一行 | `/architecture` → `ArchitectureView`（根取自壳态当前根，URL 不携根段）；既有路由与兜底重定向零改动 |
| `src/components/app-sidebar.tsx` | 「页面」组增一项 | 「架构」项 `data-testid="nav-architecture"`，active 由 URL 派生；既有五项 testid 与组语义不变 |
