# desktop-page-routing Specification (Delta)

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
- `/explores/:name` → 探索详情页（`name` 为路由参数；explore 域零改动）
- `/db` → 数据库检查页（desktop-db-inspector，现状补记）
- `/agents` → 全局 Agent 管理页（desktop-agent-management，不依赖当前 workspace root）
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

### Requirement: change 选中态 URL 化

壳态下 change 选中状态 SHALL 由路由参数承载，MUST NOT 保留独立的 `selectedChange` 本地 state 与路由双轨并存：

- 清单行点击 SHALL 导航至 `/changes/<id>`（行数据携 id——归档条目同样；name 仅作展示）
- 详情页数据参数 SHALL 来自路由参数（`useParams` 或等价 API），`useChangeDetail(root, id)` hook 本体契约不变
- 详情返回 SHALL 显式导航至 `/changes`，MUST NOT 使用 `navigate(-1)` 类历史栈回退（落点不确定）
- workspace 切换或移除当前根 SHALL 导航至 `/changes`（URL 无 `:id` 段），与现行「切换清空 change 选中」语义一致

#### Scenario: 清单行点击进入详情路由

- **WHEN** 用户点击清单中某 change 行（active 或归档组，行携该 change 的 id）
- **THEN** URL 变为 `/changes/<id>`，详情视图渲染且 `get_change_detail` 恰以 `{ root, id }` 参数发起一次

#### Scenario: 归档行经 id 进详情全状态面

- **WHEN** 用户点击归档组中某 change 行（列表 name 为该 change 裸名，磁盘目录带日期前缀）
- **THEN** URL 为 `/changes/<该 change 的 id>`，详情呈现完整状态面（status=archived、pipeline 与 runs 在案），MUST NOT 出现「文档形态」空面

#### Scenario: 返回落清单

- **WHEN** 用户在详情页触发返回操作
- **THEN** URL 变为 `/changes`，清单视图呈现，无 `:id` 段残留

#### Scenario: workspace 切换清选中

- **WHEN** 用户在 `/changes/<id>` 详情页点击 sidebar 另一 workspace 清单项
- **THEN** URL 落在 `/changes`（无 `:id` 段），清单以新根重取，旧 change 详情不呈现

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/routes.tsx` | 路由表 id 化 | `/changes/:id`（原 `:name`）；表声明独立组件不破函数行数约束 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | 行键 / 导航 id 化 | 行 `key` 与 `navigate('/changes/<id>')` 均取 `summary.id`；行展示文案取裸名 name；归档组 / 月分组结构不变 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 选中态 id 化 | `useParams<'id'>`；`useChangeDetail(root, id)` 按路由参数取数；workspace 切换过渡轮抑制与 replace 落 `/changes` 语义不变 |
| `packages/desktop/src/app.test.tsx` + 路由级测试 | 深链与选中断言 id 化 | 深链 `#/changes/<id>` → `get_change_detail` 携 `{ root, id }`；切页 / 切根重置语义不变（data-testid 挂钩） |
