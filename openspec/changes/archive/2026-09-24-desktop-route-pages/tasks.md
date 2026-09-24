# 任务: desktop-route-pages

> 依赖顺序：阶段 1（依赖与版本）→ 阶段 2（Sidebar NavLink 化）→ 阶段 3（ChangeView 选中态路由化）→ 阶段 4（App 壳路由表）→ 阶段 5（管线守线与范围核对）。
> 零改动边界：全部任务不含 `packages/desktop/src-tauri/**` 与 `packages/desktop/src/hooks/**`（hooks 取数契约不变）；`ChangeListView` / `ChangeDetailView` / `AgentDebugView` / `WelcomeView` 呈现契约零改动；测试文件（App.test.tsx / agent_page_nav.test.tsx / AppSidebar.test.tsx 改造与新增 route_pages.test.tsx）由 test-design / test-gen 阶段承接，此处不列。

## 阶段 1：依赖与版本

- [x] `packages/desktop/package.json` 新增运行时依赖并安装：`pnpm -C packages/desktop add react-router`（v7 `react-router` 单包，declarative 模式；MUST NOT 安装 `react-router-dom` 或任何第二路由库，AC-2）
- [x] `packages/desktop/package.json` `version` `0.2.2` → `0.3.0`（design D5：导航模型功能级 minor 档位）

## 阶段 2：Sidebar NavLink 化

- [x] `packages/desktop/src/components/AppSidebar.tsx` 删除 `export type TopPage`，并从 `AppSidebarProps` 删除 `page?` 与 `onPageChange?` 字段（其余字段 `workspaces` / `currentRoot` / `onOpen` / `onAdd` / `onRemove` 逐字不动）
- [x] `packages/desktop/src/components/AppSidebar.tsx` `PageNavGroup` NavLink 化（design D8 / 实现形态）：删除 `page` / `onPageChange` 参数；`SidebarMenuButton asChild` + 内嵌 `NavLink`（`to="/changes"` / `to="/agent"`），`data-testid="nav-changes"` / `data-testid="nav-agent"` 移至 NavLink 锚点且语义不变；active 经 `useLocation().pathname` 派生传入 `isActive`（变更项：`pathname === '/changes' || pathname.startsWith('/changes/')`；Agent 项：`pathname === '/agent'`），`data-active` 语义与 lucide 图标、`tooltip` 保留
- [x] `packages/desktop/src/components/AppSidebar.tsx` workspace 清单组核对零改动：清单项 `isActive` / `tooltip={record.root}` / 副文本 / `data-root`、`SidebarGroupAction` 添加流、`ContextMenu` 右键移除逐字保持

## 阶段 3：ChangeView 选中态路由化

- [x] `packages/desktop/src/views/changes/ChangeView.tsx` 选中态改路由参数承载（design D1 / 实现形态）：`useParams<'name'>()` 派生 `selected = name ?? null`，删除 `selectedChange` 本地 state（无双轨并存）
- [x] `packages/desktop/src/views/changes/ChangeView.tsx` 导航接线：`openChange` → `navigate(\`/changes/${name}\`)`；`backToList` → `navigate('/changes')`（design D4：MUST NOT 使用 `navigate(-1)`）
- [x] `packages/desktop/src/views/changes/ChangeView.tsx` 根切换过渡抑制与落清单（design D4 / 实现形态）：沿用渲染期 `prevRoot` 调整模式，根切换且带旧选中时置 `resetPending`；过渡轮 `useChangeDetail(root, null)` 抑制「新根 + 旧名」误发 `get_change_detail`；effect 中 `navigate('/changes', { replace: true })` 后清位；`ChangeListView`（`onSelect`）/ `ChangeDetailView`（`onBack`）/ `useChangeDetail` 接线契约与组件 props 签名不变

## 阶段 4：App 壳路由表

- [x] `packages/desktop/src/App.tsx` 删除 `page` state（`useState<TopPage>`）、`TopPage` 导入与传入 `AppSidebar` 的 `page` / `onPageChange` props 接线
- [x] `packages/desktop/src/App.tsx` 壳态分支自含 `<HashRouter>`（design D2）：欢迎态 gate（`root === null` → `WelcomeView`）维持 Router 外、不引入 `/welcome` 路由；壳态 `SidebarProvider` / `AppSidebar` / `SidebarInset` / `ShellHeader` / 1100px 居中容器 / `<Toaster />` App 根挂载逐字不动
- [x] `packages/desktop/src/App.tsx` 新增模块内私有组件 `AppRoutes`（路由表独立成组件，不导出，不超 `max-lines-per-function: 50`）：`/` → `<Navigate replace to="/changes" />`；`/changes` 与 `/changes/:name` → `<ChangeView list root />`；`/agent` → `<AgentDebugView root />`；`*` → `<Navigate replace to="/changes" />`；内容区条件渲染替换为 `<AppRoutes list={list} root={workspaceState.root} />`
- [x] `packages/desktop/src/main.tsx` 核对零改动（design D2：Router 于 App 内自含，保持 `ReactDOM.createRoot(<App />)` 现状，确认无 diff）

## 阶段 5：管线守线与范围核对

- [x] 残留清扫：grep `packages/desktop/src` 实现（非测试）确认无 `TopPage` / `onPageChange` / `selectedChange` 残留（AC-3）
- [x] `pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过：fmt / lint 无违例（`AppRoutes` / `PageNavGroup` / `ChangeView` 均不超 `max-lines-per-function: 50`）；knip 无未用导出残留（AC-2）；`knip.json` / `stryker.config.json` / vite 配置 / tsconfig 零改动、无新增豁免条目
  - 整链已通过（exit 0）。`AppSidebar.test.tsx` 的 `TopPage` / `page` / `onPageChange` 引用已按 design D6 就地适配（MemoryRouter 包裹 + URL 驱动断言）以解除静态检查阻塞。注：`vp test` 另有 5 个运行时失败全部位于 `src/__tests__/`（ipc_pipeline / workspace_restore 等，断言路由化前 UI 流、且受 jsdom hash 跨用例残留影响——design D6 已预见，缓解即 beforeEach 重置 hash），该目录属 test-design / test-gen 阶段范围
- [x] 范围核对：`git diff` 确认 `packages/desktop/src-tauri` 与 `packages/desktop/src/hooks` 为空、`main.tsx` 无 diff；实际改动文件与 design 变更清单一致，无越界文件
