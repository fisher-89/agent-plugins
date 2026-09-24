# 测试设计: desktop-route-pages

> **日期**: 2026-09-23

---

## 验收范围

<!-- 框架识别结论：packages/desktop 前端套件为 vite-plus 工具链（vp test，vite-plus/test 断言 API +
@testing-library/react + jsdom 组件环境），与既有 desktop 测试文件一致。单元测试路径由
test_resolve_paths 解析，errors 为空。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 路由表落地：壳态按 URL 渲染 `/changes`（清单）、`/changes/:name`（详情）、`/agent`（调试页）；`/` 与未知路径均落在 `/changes` | 集成测试 | `packages/desktop/src/__tests__/route_pages.test.tsx`（关系：Sidebar NavLink → HashRouter 路由表 → 页面渲染；`/` 与未知路径 → 重定向 /changes） |
| AC-2 | 依赖收敛：package.json 含 react-router 且无第二路由库；静态检查管线（类型检查 + 死导出扫描）通过，`TopPage` 等未用导出清零 | 不可测试项 | `packages/desktop/package.json`（见「不可测试项」） |
| AC-3 | Sidebar NavLink 化：`nav-changes` / `nav-agent` 点击切换路由，active 态由 URL 派生；代码无 `TopPage` / `onPageChange` 残留 | 单元测试；集成测试 | `packages/desktop/src/components/AppSidebar.test.tsx`；`packages/desktop/src/__tests__/route_pages.test.tsx` |
| AC-4 | 选中态 URL 化：点击 change 行进入 `/changes/:name` 且 `get_change_detail` 以 URL 参数发起；返回操作落在 `/changes` 清单 | 单元测试；集成测试 | `packages/desktop/src/views/changes/ChangeView.test.tsx`；`packages/desktop/src/__tests__/route_pages.test.tsx`（关系：清单行点击 → /changes/:name → useChangeDetail 取数） |
| AC-5 | workspace 切换清选中：切换或移除当前根后视图落在 `/changes`（URL 无 `:name` 段） | 集成测试 | `packages/desktop/src/__tests__/route_pages.test.tsx`（关系：workspace 根切换 → ChangeView 过渡抑制 → navigate('/changes')） |
| AC-6 | 欢迎态路由隔离：`root === null` 时无路由出口 / 壳 DOM，`WelcomeView` 全屏；`nav-changes` / `nav-agent` 不在场 | 集成测试 | `packages/desktop/src/__tests__/route_pages.test.tsx`（关系：欢迎态 gate → Router 子树挂载/卸载）；既有 `packages/desktop/src/App.test.tsx` 欢迎态用例保留回归 |
| AC-7 | 既有语义保留：切页往返不重发 `list_workspaces` / `list_changes`；导航点击不触发 workspace 命令；进入详情 → 切 Agent → 切回显示清单且 `get_change_detail` 不以旧选中重发；既有 testid 不变；前端测试套件全绿 | 集成测试 | `packages/desktop/src/__tests__/route_pages.test.tsx`；既有 `packages/desktop/src/App.test.tsx` 与 `packages/desktop/src/__tests__/agent_page_nav.test.tsx`（行为断言保留、表述按路由化核对改写） |
| AC-8 | spec 一致性：desktop-app-shell spec 归档合并后无「MUST NOT 引入路由」残留；desktop-page-routing spec 与实现一致 | 不可测试项 | `openspec/specs/desktop-app-shell/spec.md`、`openspec/changes/desktop-route-pages/specs/**`（见「不可测试项」） |

---

## 单元测试

<!-- 覆盖进程内可验证的组件级行为。集成用例驱动 App 自含的真实 HashRouter（design D6），
组件级单测以 MemoryRouter 包裹提供 Router 上下文。 -->

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

#### 待测功能

<!-- 来源：design.md「公共函数 / API」中所在文件为 App.tsx 的行；AppRoutes 为模块内私有组件，
design 明确不入表，其行为经 App 挂载后的集成关系覆盖。 -->

- App(): 壳态自含 `<HashRouter>`（D2）；`page` state 与 `onPageChange` 接线删除、内容区换 `AppRoutes`；欢迎态 gate 维持 Router 外

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App（HashRouter 自含挂载） | 正向 | 壳态 `render(<App />)` 不包任何 Router：启动 hash 落 `#/changes`、清单内容渲染、`[data-slot="sidebar-wrapper"]` / header DOM 标记在场（测试无需 Router 包裹即得，D2 收敛点） | 新增 |
| App（HashRouter 自含挂载） | 边界 | 启动前 `window.location.hash` 已为 `#/changes/add-feature`：`render(<App />)` 直出详情视图并以 URL 参数取数，无任何导航点击（深链直达，D7） | 新增 |
| App（HashRouter 自含挂载） | 边界 | 启动前 hash 已为 `#/agent`：直出 AgentDebugView（`agent-run-form` testid 在场）且 `nav-agent` 呈激活态 | 新增 |
| App（HashRouter 自含挂载） | 异常 | 启动前 hash 为未知路径（如 `#/bogus`）：兜底落 `#/changes` 渲染清单，不空白不崩 | 新增 |
| App（欢迎态 gate） | 边界 | `root === null` 空清单启动：无 sidebar-wrapper / header / `nav-*` testid、`WelcomeView` 全屏文案在场（既有用例沿路由化复核保留，AC-6） | 新增 |
| App（page state 条件渲染） | 废弃 | `page === 'changes' ? … : …` 条件渲染相关断言（含「无路由，state 切视图」表述与 `TopPage` 导入）随路由化删除，由路由化断言承接 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发（`list_workspaces` / `list_changes` / `get_change_detail` / `add_workspace` / `remove_workspace`），`vi.fn()` 记录调用序列供次数与参数断言（沿用既有 `mockIpc` 装置） | 全部 describe |
| `@tauri-apps/api/app` getVersion、`@tauri-apps/plugin-dialog` open、`@tauri-apps/plugin-updater` check | `vi.mock` 固定 resolve（版本号 `0.1.0`；对话框按用例 resolve null / 路径串；check 默认 null，更新用例经 `vi.stubEnv` 启用） | 全部 describe |
| `window.location.hash`（jsdom 全局、跨用例存活） | 非 mock：`beforeEach` 重置为空串，防上一用例 hash 残留污染路由初态（design D6） | 深链 / 重定向相关用例 |
| sonner `<Toaster />` | 不 mock：App 根真实挂载，toast 断言走 DOM 文案 + waitFor；`beforeEach` 统一 `toast.dismiss()` 清模块级残留 | 错误呈现相关用例 |

---

### packages/desktop/src/components/AppSidebar.tsx -> packages/desktop/src/components/AppSidebar.test.tsx

#### 待测功能

<!-- 来源：design.md「公共函数 / API」中所在文件为 AppSidebar.tsx 的行；TopPage 为删除的类型、
AppSidebarProps 为字段收缩，不构成待测函数条目。 -->

- AppSidebar(props): props 收缩（去 `page` / `onPageChange`）；「页面」组 NavLink 化（active 经 `useLocation().pathname` 派生、testid 移至锚点保持，D8 组合）；workspace 清单组 / 添加 / 右键移除零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AppSidebar（NavLink 挂载形态） | 正向 | MemoryRouter（initialEntries=['/changes']）+ SidebarProvider 挂载：`nav-changes` / `nav-agent` 渲染为锚点元素（tagName 为 `A`），testid 落在锚点上且文本含「变更」/「Agent 调试」（D8：asChild Slot 合并到锚点） | 新增 |
| AppSidebar（NavLink active 派生） | 正向 | pathname='/changes' → `nav-changes` `data-active='true'`、`nav-agent` 为 'false'；pathname='/agent' → 两项互换（active 值由 URL 派生，非 props） | 新增 |
| AppSidebar（NavLink active 派生） | 边界 | pathname='/changes/add-feature'（详情深链态）→ `nav-changes` 仍 active（`startsWith('/changes/')` 前缀派生，与路由化前 page='changes' 含详情态一致）、`nav-agent` 不激活 | 新增 |
| AppSidebar（NavLink active 派生） | 异常 | initialEntries=['/bogus']（无匹配 pathname，既非 `/changes` 前缀也非 `/agent`）→ 两 nav 均 `data-active='false'`，锚点元素与 workspace 组照常渲染不崩（active 派生对无匹配路径安全降级，不等同 App 层已兜底重定向——组件级须自证不依赖重定向前置） | 新增 |
| AppSidebar（NavLink 导航） | 正向 | 点击 `nav-agent` → 包裹层经 `useLocation` 观察到 pathname 变为 '/agent'，点击 `nav-changes` → 回 '/changes'（导航由 NavLink 承载，无回调 prop） | 新增 |
| AppSidebar（workspace 组不串扰） | 边界 | pathname='/agent' 下：workspace-item 渲染数量与 `data-active` 标记不变，`onOpen` / `onAdd` / `onRemove` 回调语义不受页面导航组改造影响（既有用例回归保留） | 新增 |
| AppSidebar（page / onPageChange props） | 废弃 | 「`mountNav(page)` 驱动激活态」「点击 → `onPageChange('agent')` 恰一次」等 props 驱动用例随 props 删除而废弃 | 废弃 |
| AppSidebar（TopPage 类型导入） | 废弃 | `import { AppSidebar, type TopPage }` 的既有类型导入随 `TopPage` 导出删除而废弃（死导出清零） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ResizeObserver（jsdom 缺口） | `vi.stubGlobal` 以 stub 类兜底（radix Tooltip / ContextMenu 定位依赖），`afterEach` unstub（沿用既有装置） | 全部 describe |
| react-router MemoryRouter | 测试装置（非 mock）：`MemoryRouter` + `initialEntries` 提供 `useLocation` 上下文，控制初态无需真实 hash | NavLink 各 describe |
| IPC / 对话框 | 不涉及：组件纯回调驱动（`onOpen` / `onAdd` / `onRemove` 以 `vi.fn()` 注入），无进程边界 | 不适用 |

---

### packages/desktop/src/views/changes/ChangeView.tsx -> packages/desktop/src/views/changes/ChangeView.test.tsx

#### 待测功能

<!-- 来源：design.md「公共函数 / API」中所在文件为 ChangeView.tsx 的行。本测试文件为新增文件
（test_resolve_paths 解析结果，此前 ChangeView 无组件级单测）。 -->

- ChangeView(props): 选中态改 `useParams` 派生（`selected = name ?? null`）；`openChange` → `navigate('/changes/:name')`；`backToList` → 显式 `navigate('/changes')`（不用 `navigate(-1)`，D4）；根切换 `resetPending` 过渡抑制 + effect 中 replace 导航落清单

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeView（路由参数选中态） | 正向 | MemoryRouter initialEntries=['/changes']（Routes 包装）→ ChangeListView 呈现、useChangeDetail 以 `(root, null)` 调用恰一次 | 新增 |
| ChangeView（路由参数选中态） | 正向 | initialEntries=['/changes/add-feature'] → ChangeDetailView 呈现、useChangeDetail 以 `(root, 'add-feature')` 调用 | 新增 |
| ChangeView（行点击导航） | 正向 | 点击 ChangeListView 项触发 `onSelect('beta-fix')` → location.pathname 变为 '/changes/beta-fix'（`navigate` 接线） | 新增 |
| ChangeView（返回导航） | 正向 | 详情态点击 `onBack`（「← 返回列表」）→ pathname === '/changes'，显式导航而非历史回退（D4：`navigate(-1)` 落点不确定） | 新增 |
| ChangeView（根切换过渡抑制） | 边界 | 详情态（name='add-feature'）以新 root 重渲染 → 过渡轮 useChangeDetail 以 `(新root, null)` 调用（旧名抑制，防「新根 + 旧名」误发），随后 pathname replace 导航至 '/changes' 且抑制位清空（后续渲染恢复 `(root, selected)` 正常取数） | 新增 |
| ChangeView（根切换 · 清单态） | 边界 | 清单态（无 name 段）变更 root prop → pathname 不变、无导航发生、不产生多余 useChangeDetail 调用（`resetPending` 不置位） | 新增 |
| ChangeView（未知 change 深链透传） | 异常 | initialEntries=['/changes/ghost']（清单中不存在）→ 选中态不做校验、useChangeDetail 以 `(root, 'ghost')` 透传调用，呈现交由 detail 态决定（组件不崩） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| useChangeDetail hook（`../hooks/useChangeDetail`） | `vi.mock` 为受控 `vi.fn()`：按入参序列返回 detail 态，暴露调用参数记录——过渡轮 `(新root, null)` 抑制断言需直接观察 hook 入参，mock 为唯一可行观测点（hook 本体契约零改动，不在此重复其内部行为测试） | 全部 describe |
| react-router MemoryRouter | 测试装置（非 mock）：`MemoryRouter` + `Routes` 包装提供 `useParams` / `useNavigate` 上下文，`initialEntries` 控制路由参数初态 | 全部 describe |
| ChangeListView / ChangeDetailView | 不 mock（呈现契约零改动）：经 props 回调（`onSelect` / `onBack`）触发导航断言，复用其既有 DOM 结构定位 | 导航 / 选中态相关 describe |

---

## 集成测试

<!-- 全部路由级集成场景收敛于 design.md 定夺的新增文件 packages/desktop/src/__tests__/route_pages.test.tsx
（D6：集成用例 render(<App />) 驱动 App 自含的真实 HashRouter，零 Router 包裹；beforeEach 重置
window.location.hash）。既有 App.test.tsx 与 __tests__/agent_page_nav.test.tsx 的行为断言语义不变、
表述按路由化核对改写（proposal「测试文件」），在下列关系描述中标注承接关系。 -->

### Sidebar NavLink → HashRouter 路由表 → 页面渲染 → `packages/desktop/src/__tests__/route_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/components/AppSidebar.tsx` | 触发方（NavLink 点击发起导航、active 由 URL 派生） |
| `packages/desktop/src/App.tsx` | 中间件（HashRouter 挂载 + AppRoutes 路由表匹配） |
| `packages/desktop/src/views/changes/ChangeView.tsx` | 渲染方（/changes* route element） |
| `packages/desktop/src/views/agent/AgentDebugView.tsx` | 渲染方（/agent route element） |

**关联AC**: AC-1, AC-3, AC-7

**关系描述**:

页面导航组从 `onPageChange` 回调上抛改为 NavLink 驱动 HashRouter，AppRoutes 按 URL 匹配选 element——这条链路把「导航意图 → 路由状态 → 页面渲染」三段跨模块协作串成一体，任何一段断链都表现为点击无响应、页面空白或 active 标记错位。值得测试的出错模式有三：其一，testid 从 button 迁移到 NavLink 渲染的锚点后，既有选择器若仍按 button 语义定位会静默失效（AC-3 要求 testid 语义不变）；其二，active 派生若只做全等匹配 `/changes`，详情态 `/changes/:name` 下 `nav-changes` 会错误失活（前缀派生遗漏）；其三，NavLink 点击若误带 workspace 语义会违反「导航不触发 workspace 命令」的既有约束（AC-7）。既有 App.test.tsx「顶层页面切换」describe 与 agent_page_nav.test.tsx 承载同一行为面（切页卸载、命令计数不变、清单数据驻留），本关系在 route_pages.test.tsx 以路由化断言（hash 落点 + active 派生）承接，两个既有文件同步核对表述后保留回归。

#### 场景: 侧栏点击切换页面路由

验证壳态内点击 `nav-agent` / `nav-changes` 经路由表切换页面：前置条件为有记录启动、恢复至清单页（`#/changes`）；输入为 NavLink 点击；预期输出为 hash 落点切换、对应 route element 渲染、另一页面内容卸载、active 标记随 URL 迁移。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | restored 后点击 `nav-agent` → hash 变为 '#/agent'、`agent-run-form` 在场、清单内容（add-feature 文案）卸载、`nav-agent` `data-active='true'` | 新增 |
| 正向 | 再点击 `nav-changes` → hash 回 '#/changes'、清单呈现、`agent-run-form` 卸载、`nav-changes` 呈激活态 | 新增 |
| 正向 | 详情态（hash='#/changes/add-feature'）下 `nav-changes` 仍呈激活态（前缀派生）；此时点击 `nav-changes` → 落 '#/changes'（无 `:name` 段）呈现清单 | 新增 |
| 边界 | 详情 → 切 Agent → 切回 changes：清单呈现、`get_change_detail` 总调用次数保持不变（不以旧选中重发，D10 语义路由化保留，改写自 agent_page_nav.test.tsx） | 新增 |

#### 场景: 导航零命令副作用与清单数据驻留

验证导航点击仅改 URL 不触发任何 workspace 命令、`useChangeList` 驻留 App 层切页往返不重发：前置条件为 restored 后记录各命令调用计数基线；输入为 agent ↔ changes 多次往返点击；预期输出为全部命令计数不变、切回后清单数据即时可见。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | agent ↔ changes 往返点击后：`list_workspaces` / `list_changes` / `add_workspace` / `remove_workspace` 调用计数与基线一致（导航零命令，AC-7） | 新增 |
| 边界 | 切往 agent 再切回 changes：清单内容（add-feature）无需重取即时呈现、`list_changes` 不重发（数据驻留 App 层，改写自 agent_page_nav.test.tsx 同名用例） | 新增 |

#### 场景: 深链直达 agent 页

验证启动时 hash 已指向 `/agent` 的直达恢复：前置条件为 render 前 `window.location.hash = '#/agent'`；输入为正常启动取数流；预期输出为 AgentDebugView 直渲染且 `nav-agent` 激活，全程无导航点击。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | hash 预置 '#/agent' 启动 → AgentDebugView 直渲染、`nav-agent` `data-active='true'`、无任何 NavLink 点击发生 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发（清单 / 详情 / workspace 动作命令）并记录调用序列，供次数断言 | 全部场景 |
| `@tauri-apps/api/app` getVersion、`@tauri-apps/plugin-dialog` open、`@tauri-apps/plugin-updater` check | `vi.mock` 固定 resolve，隔离更新与对话框分支 | 全部场景 |
| `window.location.hash` | `beforeEach` 重置为空串（jsdom 跨用例存活），深链场景用例内显式预置 | 深链直达场景 |

---

### 清单行点击 → /changes/:name 路由参数 → useChangeDetail 取数 → `packages/desktop/src/__tests__/route_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/changes/ChangeListView.tsx` | 触发方（行点击 `onSelect(name)` 回调） |
| `packages/desktop/src/views/changes/ChangeView.tsx` | 中间件（`useParams` 派生选中、`useNavigate` 接线行点击与返回） |
| `packages/desktop/src/views/changes/hooks/useChangeDetail.ts` | 读取方（以 `(root, :name)` 发起 `get_change_detail`） |
| `packages/desktop/src/App.tsx` | 路由宿主（/changes/:name 匹配、root / list 下发） |

**关联AC**: AC-4, AC-7

**关系描述**:

选中态从本地 state 迁移为路由参数后，「行点击 → 参数入 URL → 参数回流取数」替代了原来的 setState 直通，选中真相源变为 location。这条链路值得集成验证的原因：参数要经 URL 序列化再经 `useParams` 反序列化回流，任何一段错位（如 name 段编码、路由表 path 拼写）都表现为详情取数参数为 undefined 或清单不响应点击；同时返回操作改为显式 `navigate('/changes')`（D4），若误用历史回退，从 agent 页进入详情再返回会落回 agent 页而非清单。既有 App.test.tsx「列表项点击进入详情视图，返回列表」用例保留行为断言（`get_change_detail` 参数、返回后刷新入口在场），hash 落点断言由本关系新增承接。

#### 场景: 行点击进入详情与返回落清单

验证选中态 URL 化闭环：前置条件为 restored 后处于清单页；输入为清单行点击与详情页返回按钮；预期输出为 hash 与取数参数一致、返回落回无 `:name` 段的清单且清单数据不重取。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 点击 'add-feature' 行 → hash 变为 '#/changes/add-feature'、`get_change_detail` 以 `{root: FIRST.root, change: 'add-feature'}` 恰一次发起、详情 heading 在场 | 新增 |
| 正向 | 详情页点击「← 返回列表」→ hash 回 '#/changes'、清单呈现、`list_changes` 不重发（list 驻留 App 层）、刷新入口重新在场 | 新增 |
| 边界 | 先后进入 change A、返回、进入 change B：两次 `get_change_detail` 参数分别为 A 与 B（URL 参数实时透传，无旧参残留） | 新增 |

#### 场景: 深链直达详情与不存在 change 降级（D7）

验证启动时 URL 即携带选中参数的恢复路径及其异常兜底：前置条件为 render 前 hash 预置目标路径；输入为启动取数流；预期输出为详情直渲染（正常名）或既有降级页（不存在名），全程无点击。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | hash 预置 '#/changes/add-feature' 启动 → 详情直渲染、`get_change_detail` 以 URL 参数 `{root, change: 'add-feature'}` 发起、无行点击发生 | 新增 |
| 异常 | hash 预置 '#/changes/ghost'（不存在的 change）启动 → 既有「未找到该 change」降级呈现，不崩、不发起以 ghost 之外的参数取数 | 新增 |
| 边界 | hash 预置超长 name 段（>1000 字符）启动 → 参数透传不崩、呈现交由 detail 态决定（URL 参数边界鲁棒性） | 新增 |

#### 场景: 畸形多段路径兜底

验证超出 `/changes/:name` 单参数结构的路径不产生未定义渲染：前置条件为壳态启动；输入为多段 hash；预期输出为经 `*` 兜底落回清单页。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | hash 预置 '#/changes/a/b'（双段超出 `:name` 单参）启动 → 兜底落 '#/changes' 渲染清单，不空白不崩 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发：`get_change_detail` 返回固定 detail fixture（不存在名场景返回 null 模拟后端未命中），记录调用参数 | 全部场景 |
| `@tauri-apps/api/app` getVersion、`@tauri-apps/plugin-updater` check | `vi.mock` 固定 resolve，隔离无关分支 | 全部场景 |
| `window.location.hash` | `beforeEach` 重置为空串，深链场景用例内显式预置 | 深链 / 畸形路径场景 |

---

### `/` 与未知路径 → 重定向 /changes → `packages/desktop/src/__tests__/route_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/App.tsx` | 路由宿主（AppRoutes 的 `/` 与 `*` Navigate 元素配置） |
| `packages/desktop/src/views/changes/ChangeView.tsx` | 落点渲染方（重定向后清单呈现） |
| `packages/desktop/src/hooks/useChangeList.ts` | 数据方（落清单后渲染驻留 list，不因重定向重取） |

**关联AC**: AC-1

**关系描述**:

应用启动的初始 hash 为空（即 `/`），壳态 Router 挂载后第一跳就是根路径重定向；未知路径兜底则是路由表对一切未匹配路径的最后防线。这条链路测的是我们的路由表配置是否正确接线（`/` 与 `*` 两个 Navigate 元素是否真的指向 ChangeView 可渲染的 `/changes`），而非 react-router 自身的重定向机制——库语义不重复验证，只断言配置生效后的最终落点。可能的出错模式：路由表漏配 `*` 时未知路径渲染空白；漏配 `/` 时首屏无内容；重定向落点若误指向带参路径会触发一次无意义取数。另有一个跨态接线：欢迎态经添加流首次进入壳态时 Router 初次挂载、起点同样是 `/`，重定向必须同样生效。

#### 场景: 启动根路径重定向与首屏落点

验证常规启动（hash 空）经 `/` 重定向进入清单页：前置条件为有记录启动；输入为正常挂载取数流；预期输出为 hash 落 '#/changes'、清单渲染且取数序列正常（`list_workspaces` → `list_changes` 各一次，无多余 detail 取数）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 常规启动（hash 为空）→ hash 落 '#/changes'、清单内容渲染、取数序列为 `list_workspaces` + `list_changes`（无重定向引发的多余调用） | 新增 |

#### 场景: 未知路径兜底与欢迎态进壳落点

验证未匹配路径与 Router 初次挂载两种进入方式都收敛到清单页：前置条件分别为壳态启动后改写 hash、以及空清单欢迎态；预期输出为两者最终均呈现 '#/changes' 清单。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 壳态内将 hash 改写为 '#/totally-unknown' → 兜底落 '#/changes' 渲染清单，不空白不崩 | 新增 |
| 边界 | 欢迎态添加 workspace 成功进入壳态（Router 初次挂载、起点为 `/`）→ 落 '#/changes' 渲染清单（重定向对 Router 首挂同样生效） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发并记录序列（首屏取数序列断言依赖记录） | 全部场景 |
| `@tauri-apps/plugin-dialog` open | `vi.mock` 按用例 resolve 路径串（欢迎态添加流进壳场景） | 欢迎态进壳场景 |
| `window.location.hash` | `beforeEach` 重置为空串；未知路径场景用例内改写 | 全部场景 |

---

### workspace 根切换 → ChangeView 过渡抑制 → navigate('/changes') → `packages/desktop/src/__tests__/route_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/components/AppSidebar.tsx` | 触发方（onOpen 切换 / ContextMenu onRemove 移除 / GroupAction onAdd 添加） |
| `packages/desktop/src/hooks/useWorkspaces.ts` | 状态方（root 变更为唯一驱动源） |
| `packages/desktop/src/views/changes/ChangeView.tsx` | 抑制与导航方（`resetPending` 过渡轮 + effect replace 导航，D4 单点收口） |
| `packages/desktop/src/views/changes/hooks/useChangeDetail.ts` | 读取方（过渡轮入参置 null 抑制误发） |

**关联AC**: AC-5, AC-7

**关系描述**:

根切换清选中从 ChangeView 内 setState 改为「过渡抑制 + URL 导航」，且必须在 ChangeView 单点收口——添加流（`pickAndAdd`）位于 App 层、Router 之外，无法在动作处导航。这条链路的集成价值在于三条触发路径（切换 / 移除当前根 / 添加新根）共用同一收口点，任何一条遗漏都会留下带旧 `:name` 段的 URL，下一轮渲染就以「新根 + 旧名」发起 `get_change_detail` 误发（AC-7 明确禁止）；而抑制位若未在导航后清位，后续正常选详情会被永久抑制。既有 App.test.tsx「切换后 change 选中清空」用例断言视图回落，本关系新增 URL 落点与误发抑制断言。

#### 场景: 切换根清选中落清单

验证详情态下切换 workspace 后 URL 与视图双双落清单、且不发生误发取数：前置条件为 restored 后进入 add-feature 详情；输入为点击另一 workspace 清单项；预期输出为 hash 落 '#/changes'（无 `:name` 段）、清单呈现、`list_changes` 以新根恰重发一次、detail 无「新根 + 旧名」组合调用。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 详情态点击另一 workspace 项 → hash 落 '#/changes'、清单呈现、`list_changes` 以新根（SECOND.root）发起 | 新增 |
| 异常 | 上述切换全程检查 `get_change_detail` 全部调用：root 均为旧根（FIRST.root），不存在「新根 + 旧名」组合（过渡轮抑制生效，AC-7） | 新增 |
| 边界 | 清单态（无选中）切换根 → hash 保持 '#/changes' 无跳变、仅 `list_changes` 以新根重发（无选中时抑制位不置位、无导航发生） | 新增 |

#### 场景: 移除当前根与添加新根落清单

验证另两条根变更路径同样收口：前置条件为详情态；输入分别为 ContextMenu 移除当前根、GroupAction 添加新根（对话框返回路径入库后以 canonical root 打开）；预期输出为两者最终均落 '#/changes' 清单。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 详情态右键当前根 → 「移除」→ 顺延剩余第一名 → hash 落 '#/changes'、`list_changes` 以顺延根发起 | 新增 |
| 正向 | 详情态经 GroupAction 添加新根（对话框 resolve 新路径、`add_workspace` 返回 canonical root）→ hash 落 '#/changes'、清单以新根呈现（添加流在 Router 外，导航由 ChangeView 单点收口，D4） | 新增 |
| 边界 | 移除当前根至空清单 → 退出壳态回欢迎态（此场景的 Router 卸载面在「欢迎态 gate」关系覆盖，此处仅断言不残留旧 hash 渲染） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发：`remove_workspace` 同步收缩内存清单、`add_workspace` 返回 canonical root 记录、`get_change_detail` 记录全部调用参数（误发检查依赖全量记录） | 全部场景 |
| `@tauri-apps/plugin-dialog` open | `vi.mock` resolve 路径串（添加流场景） | 添加新根场景 |
| `window.location.hash` | `beforeEach` 重置为空串 | 全部场景 |

---

### 欢迎态 gate → Router 子树挂载/卸载 → `packages/desktop/src/__tests__/route_pages.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/App.tsx` | gate 方（`root === null` 条件分支位于 HashRouter 之外，D3：欢迎态不路由化） |
| `packages/desktop/src/views/WelcomeView.tsx` | 渲染方（欢迎态全屏呈现与添加入口） |
| `packages/desktop/src/components/AppSidebar.tsx` | 不在场验证方（壳态专属 DOM，欢迎态必须缺席） |

**关联AC**: AC-6

**关系描述**:

欢迎态 gate 维持在 Router 外意味着 HashRouter 子树随 root 在 null 与非 null 之间完整挂载 / 卸载——这是本变更唯一一处 Router 生命周期整体变迁，跨模块协作面在于：欢迎态下不仅路由出口缺席，侧栏导航（NavLink 需 Router context）也必须整体缺席，若 gate 位置被误移入 Router 内，NavLink 会在无 Router 时崩溃或欢迎态残留壳 DOM；反向从欢迎态添加 workspace 进壳时，Router 初次挂载要正确落 '/changes'（与重定向关系衔接）。既有 App.test.tsx 欢迎态用例（无壳 DOM、WelcomeView 全屏、导航不在场）语义全部保留，本关系补齐「壳 → 欢迎」卸载与「欢迎 → 壳」挂载两个方向的 Router 生命周期断言。

#### 场景: 欢迎态路由隔离与双向迁移

验证 root === null 时 Router 子树整体缺席、以及双向迁移时的挂载/卸载完整性：前置条件分别为空清单启动、壳态移除至空清单、欢迎态添加成功；预期输出为欢迎态无任何壳 DOM 与 nav testid、进壳后路由正常挂载并落清单。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 空清单启动 → `WelcomeView` 全屏文案在场、无 `[data-slot="sidebar-wrapper"]` / header / `nav-changes` / `nav-agent`、`Toaster` 在场（既有用例沿路由化复核保留） | 新增 |
| 边界 | 壳态右键移除唯一根至空清单 → Router 子树卸载回欢迎态：无壳 DOM 残留、`WelcomeView` 呈现、不因 Router 卸载崩溃 | 新增 |
| 边界 | 欢迎态添加 workspace 成功 → 壳 DOM 与 nav testid 在场、Router 挂载落 '#/changes' 渲染清单 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发：`list_workspaces` 返回可控清单（空 / 非空切换承载双向迁移）、`add_workspace` 返回新记录 | 全部场景 |
| `@tauri-apps/plugin-dialog` open | `vi.mock` resolve 路径串（欢迎 → 壳迁移场景） | 进壳迁移场景 |
| `window.location.hash` | `beforeEach` 重置为空串（欢迎态本无 hash 语义，重置防前用例残留） | 全部场景 |

---

## 不可测试项

<!-- test_resolve_paths 的 errors 为空，无解析失败条目。以下为 proposal 范围内但不由单测/集成测试承接的条目。 -->

- AC-2 依赖收敛（`react-router` 唯一路由依赖、无第二路由库、`TopPage` 等未用导出清零、静态检查管线通过）— **原因**: 属构建配置与静态分析门禁（依赖清单核对 + 类型检查 + 死导出扫描），非进程内可断言的运行时行为；由实现阶段的管线检查与验收阶段评估承接，测试代码无法对其断言。
- AC-8 spec 一致性（desktop-app-shell 归档合并后无「MUST NOT 引入路由」残留、desktop-page-routing spec 与实现一致）— **原因**: 文档级归档合并与文本核对，发生在归档阶段，自动化测试不读取 spec 文本；由归档流程与验收评估承接。
- AC-7 中「前端测试套件全绿」整体门禁 — **原因**: 汇总性判定而非单个可设计用例；其实际断言面已由上述各单元 / 集成场景逐条承接。
- Tauri 生产形态下 HashRouter 的真实深链 / 刷新行为（自定义协议静态资源服务、webview 内 history 栈）— **原因**: jsdom 无法模拟 Tauri webview 与自定义协议层，D2 决策的运行时正确性超出前端自动化测试范围；测试侧以真实 HashRouter 挂载（design D6）最大程度逼近，打包形态验证留待人工验收。
- `packages/desktop/package.json` 的 `version` 字段 bump（0.2.2 → 0.3.0，D5）— **原因**: 纯配置元数据，无运行时行为断言面。
