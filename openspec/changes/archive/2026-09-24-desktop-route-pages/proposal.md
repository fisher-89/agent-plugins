# 提案: desktop-route-pages

> **变更**: desktop-route-pages
> **日期**: 2026-09-23
> **状态**: draft

---

## 问题

`packages/desktop`（Tauri 桌面壳）的页面导航目前由两层手工本地 state 承载，URL 不承载任何导航状态：

1. **顶层页面切换**：`App.tsx` 持有 `useState<TopPage>('changes')`（`TopPage = 'changes' | 'agent'`），经条件渲染在 `ChangeView` 与 `AgentDebugView` 之间切换；`AppSidebar` 的页面导航组（`nav-changes` / `nav-agent`）经 `onPageChange` 回调上抛。
2. **页内层级切换**：`ChangeView` 持有 `useState<string | null>(selectedChange)`，在清单 ↔ 详情间条件渲染，`onBack` 置空回到清单。

这一模型的代价随页面与页内层级增长而放大：每新增一个顶层页面或页内层级都要再添一对 state + 条件分支 + 回调 prop（样板扩散）；导航状态不可寻址（无法经 URL 直达某 change 详情）；导航行为的断言只能穿透渲染产物间接验证。现行 `desktop-app-shell` spec 明文规定「sidebar MUST NOT 引入路由」「切换维持本地 state、MUST NOT 引入路由」，本变更按用户需求（页面路由化）翻转该决策，需以 MODIFIED delta 修订既有条款，并新增能力承接路由契约。

---

## 提案

引入 **react-router v7**（declarative 模式，`react-router` 单包——v7 起 `react-router-dom` 已并入）+ **HashRouter**，将壳态导航收口为路由表：

- **路由表**：`/` → 重定向 `/changes`；`/changes` → 变更清单页；`/changes/:name` → 变更详情页（name 为路由参数）；`/agent` → Agent 调试页；未知路径 → 回退重定向 `/changes`。
- **选中态 URL 化**：`ChangeView` 的 `selectedChange` 本地 state 由路由参数 `:name` 取代——清单行点击 `navigate('/changes/<name>')`，详情参数来自 `useParams`，`onBack` 显式 `navigate('/changes')`（不用 `navigate(-1)`，避免回退落点不确定）。
- **workspace 切换清选中**：切换 / 移除当前根后导航到 `/changes`（无 `:name` 段），语义与现行「切换清空 change 选中」一致。
- **Sidebar 导航 NavLink 化**：页面导航组改用 `NavLink`，active 态由当前 URL 派生；删除 `TopPage` 类型与 `onPageChange` 回调；`nav-changes` / `nav-agent` testid 保持不变。
- **欢迎态维持路由外**：`root === null` 的欢迎态 gate 保留在 Router 之外（不引入 `/welcome` 路由），Router 仅在壳态挂载，「欢迎态不挂壳」现状不变。

HashRouter 的选择针对 Tauri 生产形态：前端经自定义协议以静态资源服务，BrowserRouter 的深链 / 刷新会落到不存在的资源路径，Hash 路由无服务端依赖。除导航外零行为变化：取数仍收口 hooks、显式刷新；`src-tauri/**` 后端零改动。

---

## 能力

### 新增能力

- **desktop-page-routing** — desktop 前端壳态页面路由契约：HashRouter + 路由表（`/changes`、`/changes/:name`、`/agent`）、change 选中态 URL 化、Sidebar 导航 NavLink 化、欢迎态路由隔离、既有集成语义保留（切页不重发查询、导航不触发 workspace 命令、testid 稳定）。

### 修改的能力

- **desktop-app-shell** — 「Sidebar 壳层布局」requirement 修订：删除「sidebar MUST NOT 引入路由」「切换维持本地 state、MUST NOT 引入路由」两条禁令及「不引入路由」scenario，改为路由化导航表述（路由表契约由 desktop-page-routing 能力承接）；Module Contract 中 `App.tsx` / `AppSidebar.tsx` 行同步更新。其余条款（header 终态、折叠交互、workspace 选择、刷新取数模型、错误双轨等）不变。

---

## 变更范围

### 实现文件

- `packages/desktop/package.json` — 新增 `react-router` 依赖；desktop 版本号 bump（档位见待决问题）
- `packages/desktop/src/App.tsx` — 路由表替换 `page` state 与条件渲染；壳态内容迁入路由出口
- `packages/desktop/src/main.tsx` — Router 挂载点（`main.tsx` 包裹 App 或 App 内自含 HashRouter，design 定夺）
- `packages/desktop/src/components/AppSidebar.tsx` — `PageNavGroup` 改 `NavLink`；删除 `TopPage` 导出与 `page` / `onPageChange` props
- `packages/desktop/src/views/changes/ChangeView.tsx` — 选中态改路由参数承载（`useParams` / `useNavigate`）；或溶解为 `/changes` 与 `/changes/:name` 两个 route element（design 定夺，见待决问题）

### 测试文件

- `packages/desktop/src/App.test.tsx` — 路由挂载适配（App 自含 HashRouter 时改动极小）；清单 / 详情流断言保持
- `packages/desktop/src/__tests__/agent_page_nav.test.tsx` — 行为语义不变（切页卸载、不触发 workspace 命令、选中重置 D10、清单数据驻留），按路由化实现核对断言表述
- `packages/desktop/src/components/AppSidebar.test.tsx` — `TopPage` props → NavLink 改造后的 props/active 断言更新
- 新增路由级测试（如 `src/__tests__/route_pages.test.tsx`，命名 design 定）— 路由表渲染、未知路径回退、选中态 URL 化、返回回清单、workspace 切换落清单

### 删除文件

- 无（若 design 选 `ChangeView` 溶解方案，则删除 `packages/desktop/src/views/changes/ChangeView.tsx`，见待决问题）

### 不要修改

- `packages/desktop/src-tauri/**` — 纯前端变更，Rust 侧零改动
- `packages/desktop/src/hooks/**` — `useChangeList` / `useChangeDetail` / `useWorkspaces` / `useUpdater` 取数契约不变（`useChangeDetail` 的入参来源由 state 改为路由参数，hook 本体不动）
- `packages/desktop/src/views/agent/**` — `AgentDebugView` 及子组件内部不变
- `packages/desktop/src/views/changes/ChangeListView.tsx` / `ChangeDetailView.tsx` — 呈现契约不变（`onSelect` / `onBack` 接线不变，仅实现来源改为路由）
- `openspec/specs/desktop-workspace-store/**` 等 workspace 注册命令语义 — 不变

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 路由表落地 | 壳态按 URL 渲染 `/changes`（清单）、`/changes/:name`（详情）、`/agent`（调试页）；`/` 与未知路径均落在 `/changes` |
| AC-2 | 依赖收敛 | `package.json` 含 `react-router` 且无第二路由库；`pnpm -C packages/desktop run client:check`（vp check + knip）通过，`TopPage` 等未用导出清零 |
| AC-3 | Sidebar NavLink 化 | `nav-changes` / `nav-agent` 点击切换路由，active 态由 URL 派生；代码无 `TopPage` / `onPageChange` 残留 |
| AC-4 | 选中态 URL 化 | 点击 change 行进入 `/changes/:name` 且 `get_change_detail` 以 URL 参数发起；返回操作落在 `/changes` 清单 |
| AC-5 | workspace 切换清选中 | 切换或移除当前根后视图落在 `/changes`（URL 无 `:name` 段） |
| AC-6 | 欢迎态路由隔离 | `root === null` 时无路由出口 / 壳 DOM，`WelcomeView` 全屏；`nav-changes` / `nav-agent` 不在场 |
| AC-7 | 既有语义保留 | 切页往返不重发 `list_workspaces` / `list_changes`；导航点击不触发 workspace 命令；进入详情 → 切 Agent → 切回显示清单且 `get_change_detail` 不以旧选中重发；既有 testid 不变；`pnpm -C packages/desktop run test` 全绿 |
| AC-8 | spec 一致性 | `openspec/specs/desktop-app-shell/spec.md` 归档合并后无「MUST NOT 引入路由」残留；desktop-page-routing spec 与实现一致 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Tauri 自定义协议下 BrowserRouter 深链 / 刷新 404 | 打包后导航不可用 | 中（若误选） | 决策锁定 HashRouter（D2）；验收含壳态路由渲染 |
| 既有集成测试（App.test.tsx / agent_page_nav.test.tsx）对渲染结构的假设被路由化打破 | 测试改造面扩大 | 中 | testid 全部保持；App 自含 Router 使测试改造收敛为断言核对；行为断言语义不变 |
| 路由化后组件卸载 / 重挂时序变化引起取数漂移（如 detail 重复请求） | 违反「查询显式触发」模型 | 低 | AC-7 保留「不重发查询」断言；hooks 层零改动 |
| knip 检出 `TopPage` 等未用导出、路由表组件超 `max-lines-per-function: 50` | client:check 失败 | 低 | 同步删除弃用导出；路由表独立组件声明 |
| hash URL 中的 change 名非法字符（`/` 等） | 详情路由参数解析错位 | 低 | change 目录名为 kebab-case（OpenSpec 约束）；未知路径统一回退 `/changes` 兜底 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 路由库选型 | react-router v7（declarative 模式，`react-router` 单包） | 生态默认、React 19 兼容；declarative 模式与现有纯组件 + hooks 风格匹配，不引入 loader/action 数据抽象 | TanStack Router（类型能力过剩、抽象更重）；wouter（过轻，嵌套 layout 生态弱）；自研（无必要维护成本） |
| D2 Router 类型 | HashRouter（生产） | Tauri 生产构建经自定义协议静态资源服务，BrowserRouter 深链 / 刷新落到不存在资源；Hash 无服务端依赖、Tauri 社区惯例 | MemoryRouter（丢失 URL 语义，仅测试用）；BrowserRouter（需协议层 fallback 配置） |
| D3 欢迎态是否路由化 | 不路由化，`root === null` gate 维持在 Router 外 | 保持「欢迎态不挂壳」现状；导航语义仅存在于壳态，Router 挂载面最小 | `/welcome` 路由（欢迎态纳入 Router 需壳态条件分支进路由树，复杂化且无收益） |
| D4 选中态迁移与返回 | `selectedChange` → `/changes/:name` 路由参数；返回显式 `navigate('/changes')` | 导航状态可寻址可测；显式返回避免 `navigate(-1)` 经 agent 页回退的落点不确定性 | `navigate(-1)`（回退目标依赖历史栈）；保留本地 state + 路由双轨（两份真相源） |
| D5 既有 spec 决策翻转 | desktop-app-shell「MUST NOT 引入路由」以 MODIFIED delta 修订，路由契约落新能力 desktop-page-routing | 用户需求页面路由化；禁令条款与实现冲突 MUST NOT 静默存续；壳层其余契约不动 | 仅改实现不动 spec（spec 失真） |

### 待决问题

- `ChangeView` 保留（内部改路由参数接线）还是溶解为 `/changes` 与 `/changes/:name` 两个 route element——影响文件删除与测试归属
- Router 挂载位置：`main.tsx` 包裹 `<App />` 还是 `App` 内自含 `<HashRouter>`；测试侧用 `MemoryRouter` 包裹还是直接驱动真实 HashRouter
- desktop 版本 bump 档位：0.2.2 → 0.2.3（patch）或 0.3.0（minor，导航模型变更）
- hash URL 断言是否纳入测试（如启动直达 `/changes/:name` 的深链恢复场景是否值得覆盖）

---
