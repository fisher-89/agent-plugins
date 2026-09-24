# 设计: desktop-route-pages

> **变更**: desktop-route-pages
> **日期**: 2026-09-23

---

## 提案与规格同步状态

`proposal.md` 与 `openspec/changes/desktop-route-pages/specs/`（desktop-page-routing ADDED + desktop-app-shell MODIFIED delta）已由提案阶段写入并通过评估，不在本 design 变更清单与任务范围内。本文覆盖提案「变更范围 - 实现文件」的实现设计；提案「测试文件」节所列各文件（含新增路由级测试，名称于此定夺为 `packages/desktop/src/__tests__/route_pages.test.tsx`）由 test-design / test-gen 阶段承接，本 design 变更清单不列测试文件。

---

## 关键设计决策（proposal 待决问题定夺）

proposal「待决问题」四项 + 实现形态定夺如下，均为本文约束，实现阶段照此落地：

| # | 问题 | 定夺 | 理由 |
|---|------|------|------|
| D1 | `ChangeView` 保留 vs 溶解为两个 route element | **保留**（内部改路由参数接线，不删文件） | surgical：`ChangeListView` / `ChangeDetailView` 呈现契约零改动；无文件删除、无新增视图文件；`/changes` 与 `/changes/:name` 复用同一 element，选中态派生（`name ?? null`）单点收敛于一个组件；溶解只多出两个组件壳，无行为收益 |
| D2 | Router 挂载位置 | **App 内自含 `<HashRouter>`**（壳态分支内挂载），`main.tsx` 零改动 | 欢迎态 gate 天然留在 Router 外（提案 D3）；`main.tsx` 保持 `createRoot(<App />)` 现状；测试 `render(<App />)` 无需任何 Router 包裹，改造面收敛为断言核对（提案风险表预期） |
| D3 | `/` 与未知路径重定向语义 | **`<Navigate to="/changes" replace />`** | 重定向非用户意图产生的导航历史项，replace 避免浏览器回退键困在重定向环；根切换清理（D4）同理用 replace |
| D4 | workspace 切换清选中的实现位置与过渡抑制 | **ChangeView 内单点处理**：沿用渲染期 `prevRoot` 调整模式，根切换且带旧选中时置 `resetPending`（过渡轮 `useChangeDetail` 入参置 null），effect 中 `navigate('/changes', { replace: true })` 后清位 | 单点覆盖 select / 移除当前根 / GroupAction 添加三条根变更路径（添加流 `pickAndAdd` 在 App 层、位于 Router 外，无法在动作处导航，故必须在 ChangeView 收口）；过渡轮抑制避免「新根 + 旧名」误发 `get_change_detail`（AC-7 / D10 语义）；`/agent` 无 `:name` 段无需处理，切页保持现状（agent 页不因切根跳页） |
| D5 | desktop 版本 bump 档位 | **0.2.2 → 0.3.0（minor）** | 导航模型变更为功能级（新增 URL 寻址能力 + 新依赖 react-router），非纯内部重构；对齐 0.2.0（agent 执行功能）minor 档位先例；0.2.3 patch 低估行为面变化 |
| D6 | 测试挂载方式（desktop-page-routing「管线合规」requirement 委托 design 定夺） | **集成用例驱动 App 自含的真实 HashRouter**（`render(<App />)` 即得，零包裹）；**AppSidebar 组件级单测以 `MemoryRouter` 包裹**（NavLink 需 Router context）；集成用例 `beforeEach` 重置 `window.location.hash`（jsdom location 跨用例存活） | App 自含 Router（D2）使集成侧改造收敛；MemoryRouter 挂载最轻、无 hash 跨用例残留副作用；用例撰写与断言细节由 test-design / test-gen 承接，此处仅定挂载策略 |
| D7 | hash 深链恢复场景是否纳入测试范围 | **纳入**（建议 test-design 覆盖：启动前 hash 已为 `#/changes/:name` → 详情直渲染并以 URL 参数取数） | URL 寻址是本变更核心收益，启动直达是其最小验证；不存在该 change 时「未找到该 change」降级页兜底（既有语义），深链无崩溃面 |
| D8 | NavLink 与 `SidebarMenuButton` 组合方式 | **`SidebarMenuButton asChild` + 内嵌 `NavLink`**（testid 置于 NavLink 渲染的锚点）；active 值经 `useLocation().pathname` 派生后传入 `isActive`（`data-active` 语义不变） | 已核实 vendored `SidebarMenuButton` 经 `Button` 支持 `asChild`（Slot 合并到锚点，`data-active` / `tooltip` / 样式全保留）；避免 button 嵌套 anchor 的非法结构；NavLink 承载导航与 `aria-current`，`isActive` 与 NavLink 均由 URL 派生（AC-3「active 态由 URL 派生」） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| App 壳 | 欢迎态 gate（Router 外）；壳态分支自含 `<HashRouter>`（D2）内挂 `SidebarProvider` + `AppSidebar` + `SidebarInset`；header 终态 / 1100px 容器 / `Toaster` / `pickAndAdd` 逐字不动；`page` state 与 `onPageChange` 接线删除 | `packages/desktop/src/App.tsx` | useWorkspaces / useChangeList / useUpdater / AppSidebar / react-router（HashRouter） / sonner | React 19 + react-router v7 declarative |
| AppRoutes（App.tsx 内私有组件） | 路由表独立成组件（spec 硬性要求）：`/` 与 `*` 重定向、`/changes`、`/changes/:name`、`/agent`；向 route element 下发 `root` / `list` | `packages/desktop/src/App.tsx`（不导出） | react-router（Routes / Route / Navigate） / ChangeView / AgentDebugView | react-router v7 |
| AppSidebar | 「页面」导航组 NavLink 化（active 由 URL 派生，D8 组合）；workspace 清单组 / 添加 / 右键移除逐字不动；`TopPage` 导出与 `page` / `onPageChange` props 删除 | `packages/desktop/src/components/AppSidebar.tsx` | ui/sidebar / ui/context-menu / lucide-react / react-router（NavLink / useLocation） | shadcn Sidebar + react-router NavLink |
| ChangeView | 选中态改路由参数承载（`useParams`）；行点击 / 返回显式导航；根切换过渡抑制 + 落 `/changes`（D4）；`/changes` 与 `/changes/:name` 共用本组件 | `packages/desktop/src/views/changes/ChangeView.tsx` | ChangeListView / ChangeDetailView / useChangeDetail / react-router（useParams / useNavigate） | React + react-router hooks |
| 取数 hooks | 契约零改动：`useChangeList` 驻留 App 层（切页不丢数据）、`useChangeDetail(root, change)` 入参来源改为路由参数、`useWorkspaces` / `useUpdater` 不动 | `packages/desktop/src/hooks/**`、`packages/desktop/src/views/changes/hooks/useChangeDetail.ts` | @tauri-apps/api/core | React hooks（显式刷新模型不变） |
| 呈现视图 | 契约零改动：`ChangeListView`（`onSelect` 接线不变）、`ChangeDetailView`（`onBack` 接线不变）、`AgentDebugView`、`WelcomeView` | `packages/desktop/src/views/**` | ui/**、types/dto | React |
| 入口 | 挂载点零改动（D2）：`createRoot(<App />)` | `packages/desktop/src/main.tsx` | App / global.css | ReactDOM |
| react-router 运行时 | HashRouter history、路由匹配、NavLink 导航与 active、导航 hooks | `packages/desktop/node_modules/react-router`（新依赖） | react / react-dom | react-router v7（`react-router` 单包，declarative 模式，不引入 loader/action 数据抽象） |

组件间数据流（零后端改动，单入口）：

```
AppSidebar NavLink（nav-changes / nav-agent）─▶ HashRouter 路由表 ─▶ /changes 或 /agent route element
ChangeListView 行点击 onSelect(name) ─▶ navigate(`/changes/${name}`) ─▶ ChangeView(:name) ─▶ useChangeDetail(root, :name)
ChangeDetailView「← 返回列表」onBack ─▶ navigate('/changes') ─▶ ChangeView(无 :name) ─▶ 清单（list 驻留 App 层不重取）
workspace 切换 / 移除当前根 / 添加新根 ─▶ useWorkspaces.root 变更 ─▶ ChangeView 过渡轮（detail 入参置 null 抑制误发）
                                          └▶ effect: navigate('/changes', { replace: true })（D4）
root === null ─▶ 欢迎态 gate（Router 外）：卸载整个 HashRouter 子树，WelcomeView 全屏
```

---

## 变更清单

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/package.json` | dependencies 新增 `react-router`（`^7`）；`version` `0.2.2` → `0.3.0`（D5） | 唯一路由依赖、无第二路由库（AC-2）；knip.json / stryker.config.json / vite.config / tsconfig 零改动、无新增豁免 |
| `packages/desktop/src/main.tsx` | **零改动**（D2 定夺：Router 于 App 内自含，`main.tsx` 保持 `ReactDOM.createRoot(<App />)` 现状） | proposal「实现文件」清单覆盖项；实现形态确认无 diff，非本变更实际修改面 |
| `packages/desktop/src/App.tsx` | ① 删除 `page` state（`useState<TopPage>`）与传入 `AppSidebar` 的 `page` / `onPageChange` props、`TopPage` 导入；② 壳态分支以 `<HashRouter>` 自含包裹（D2）：`SidebarProvider` → `AppSidebar` + `SidebarInset`，`ShellHeader` / 1100px 居中容器 / `<Toaster />` App 根挂载逐字不动；③ 内容区 `page === 'changes' ? … : …` 条件渲染替换为 `<AppRoutes list={list} root={workspaceState.root} />`；④ 新增模块内私有组件 `AppRoutes`（路由表独立成组件，见「实现形态」，不超 `max-lines-per-function: 50`） | AC-1 / AC-2 / AC-6；欢迎态 gate 维持在 Router 外（root === null 不渲染路由出口与壳 DOM）；`pickAndAdd` / `UpdateIndicator` 不动 |
| `packages/desktop/src/components/AppSidebar.tsx` | ① 删除 `export type TopPage` 与 `AppSidebarProps` 的 `page?` / `onPageChange?` 字段；② `PageNavGroup` NavLink 化（D8 组合，形态见「实现形态」）：`SidebarMenuButton asChild` + `NavLink`，`data-testid="nav-changes"` / `data-testid="nav-agent"` 移至 NavLink 锚点且语义不变，active 经 `useLocation().pathname` 派生传入 `isActive`；`page` / `onPageChange` 参数从 `PageNavGroup` 删除 | AC-3；workspace 清单组（isActive / tooltip / 副文本 / data-root）、`SidebarGroupAction` 添加、`ContextMenu` 右键移除逐字不动 |
| `packages/desktop/src/views/changes/ChangeView.tsx` | ① `selectedChange` 本地 state 删除，选中改 `useParams<'name'>()` 派生（`selected = name ?? null`）；② `openChange` → `navigate(\`/changes/${name}\`)`；`backToList` → `navigate('/changes')`（D4：不用 `navigate(-1)`）；③ 根切换过渡抑制：渲染期 `prevRoot` + `resetPending` 调整，过渡轮 `useChangeDetail(root, null)` 抑制误发，effect 中 `navigate('/changes', { replace: true })` 后清位（D4）；④ 组件 props 签名不变（`{ root: string \| null; list: ChangeListState }`） | AC-4 / AC-5 / AC-7；`ChangeListView`（`onSelect`）/ `ChangeDetailView`（`onBack`）/ `useChangeDetail`（hook 本体）呈现与取数契约零改动；`useChangeDetail` 挂载点留在本组件（切页卸载语义同现状） |

<!-- 新增文件：无。D1 定夺 ChangeView 保留、AppRoutes / PageNavGroup 为模块内私有组件（不构成新文件）；新增路由级测试文件（src/__tests__/route_pages.test.tsx）由 test-design / test-gen 阶段承接，不入实现清单。 -->

<!-- 删除文件：无。D1 定夺 ChangeView 不溶解，文件保留。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `App` | `packages/desktop/src/App.tsx` | 修改 | `export default function App(): React.JSX.Element`（签名不变） | 壳态自含 `<HashRouter>`（D2）；`page` state 删除；内容区换 `AppRoutes`；欢迎态 gate 位置不变 |
| `AppSidebar` | `packages/desktop/src/components/AppSidebar.tsx` | 修改 | `export function AppSidebar(props: AppSidebarProps): React.JSX.Element` | props 收缩（去 `page` / `onPageChange`）；「页面」组 NavLink 化；workspace 组零改动 |
| `ChangeView` | `packages/desktop/src/views/changes/ChangeView.tsx` | 修改 | `export function ChangeView(props: { root: string \| null; list: ChangeListState }): React.JSX.Element`（签名不变） | 选中态改路由参数承载；行点击 / 返回 / 根切换导航语义见「实现形态」 |

<!-- 说明：`AppRoutes` / `PageNavGroup` 为模块内私有组件（不导出，knip 纪律），不入表；本变更无新增模块级导出、无导出删除之外的 class / CLI 子命令 / HTTP 端点变更；`useChangeDetail` 等 hooks 签名与行为零改动（proposal「不要修改」）。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `TopPage` | `packages/desktop/src/components/AppSidebar.tsx` | 删除 | `type TopPage = 'changes' \| 'agent'` 随 page state 一并移除；实现代码无残留（AC-2 knip 清零 / AC-3） |
| `AppSidebarProps` | `packages/desktop/src/components/AppSidebar.tsx` | 修改 | 删除 `page?: TopPage` 与 `onPageChange?: (page: TopPage) => void`；其余字段（`workspaces` / `currentRoot` / `onOpen` / `onAdd` / `onRemove`）逐字不动 |

<!-- 说明：`ChangeListState` / `ChangeDetailState` / `WorkspaceState` / `UpdateState` / `types/dto.ts` 各类型零改动（引用不动）；ChangeView props 为内联类型、签名不变。 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.0"`（原 `"0.2.2"`） | desktop 版本 bump，minor 档位（D5：导航模型功能级变更） |
| `dependencies.react-router` | `packages/desktop/package.json` | 新增 | `"^7"`（semver range） | v7 declarative 单包（`react-router-dom` 已并入，不另装）；无第二路由库（AC-2） |

<!-- 说明：knip.json / stryker.config.json / vite.config / components.json / tsconfig.json 零改动、无新增豁免条目（对齐 desktop-page-routing「管线合规」scenario「无路由化引入的豁免条目」）。 -->

---

## 实现形态（约束）

**App.tsx 壳态结构**（欢迎态 gate 位置不变，`HashRouter` 只包壳态）：

```tsx
{workspaceState.root === null ? (
  <WelcomeView state={workspaceState} onAdd={pickAndAdd} />
) : (
  <HashRouter>
    <SidebarProvider>
      <AppSidebar currentRoot={…} onAdd={pickAndAdd} onOpen={workspaceState.select}
                  onRemove={workspaceState.remove} workspaces={workspaceState.workspaces} />
      <SidebarInset>
        <ShellHeader update={update} />
        <div className="mx-auto w-full max-w-[1100px] flex-1 px-4 py-4">
          <AppRoutes list={list} root={workspaceState.root} />
        </div>
      </SidebarInset>
    </SidebarProvider>
  </HashRouter>
)}
<Toaster />
```

**AppRoutes 路由表**（App.tsx 内私有组件，无 hooks、无副作用，约 10 行）：

```tsx
function AppRoutes({ root, list }: { root: string; list: ChangeListState }) {
  return (
    <Routes>
      <Route path="/" element={<Navigate replace to="/changes" />} />
      <Route path="/changes" element={<ChangeView list={list} root={root} />} />
      <Route path="/changes/:name" element={<ChangeView list={list} root={root} />} />
      <Route path="/agent" element={<AgentDebugView root={root} />} />
      <Route path="*" element={<Navigate replace to="/changes" />} />
    </Routes>
  );
}
```

**PageNavGroup NavLink 化**（AppSidebar.tsx 内私有组件，active 由 URL 派生）：

```tsx
const { pathname } = useLocation();
// 变更项：/changes 与 /changes/:name 均 active（与路由化前 page='changes' 含详情态一致）
<SidebarMenuButton asChild isActive={pathname === '/changes' || pathname.startsWith('/changes/')} tooltip="变更">
  <NavLink data-testid="nav-changes" to="/changes">
    <GitBranch />
    <span>变更</span>
  </NavLink>
</SidebarMenuButton>
// Agent 项：仅 /agent active
<SidebarMenuButton asChild isActive={pathname === '/agent'} tooltip="Agent 调试">
  <NavLink data-testid="nav-agent" to="/agent">…</NavLink>
</SidebarMenuButton>
```

**ChangeView 路由接线**（选中态单点 + 过渡抑制，约 30 行，不超 50 行线）：

```tsx
const { name } = useParams<'name'>();
const navigate = useNavigate();
const selected = name ?? null;
const [prevRoot, setPrevRoot] = useState(root);
const [resetPending, setResetPending] = useState(false);
// 渲染期调整（沿用既有模式）：根切换且带旧选中 → 置待导航标记
if (prevRoot !== root) {
  setPrevRoot(root);
  if (selected !== null) setResetPending(true);
}
// 过渡轮以 null 取数：抑制「新根 + 旧名」误发 get_change_detail（AC-7 / D10）
const detail = useChangeDetail(root, resetPending ? null : selected);
useEffect(() => {
  if (resetPending) {
    setResetPending(false);
    navigate('/changes', { replace: true }); // workspace 切换落清单，URL 无 :name 段（AC-5）
  }
}, [resetPending, navigate]);
const openChange = useCallback((n: string) => navigate(`/changes/${n}`), [navigate]);
const backToList = useCallback(() => navigate('/changes'), [navigate]); // 显式返回，不用 navigate(-1)（D4）
return selected === null ? (
  <ChangeListView state={list} onSelect={openChange} />
) : (
  <ChangeDetailView state={detail} onBack={backToList} />
);
```

行为要点：① 两条 `/changes*` 路由渲染同一组件类型，`/changes` ↔ `/changes/:name` 间导航不引发重挂载；② 深链启动（hash 已为 `#/changes/:name`）时 `prevRoot === root`、`resetPending === false`，直接以 URL 参数取数（D7）；③ hash 指向不存在的 change → `useChangeDetail` 返回 null → 既有「未找到该 change。」降级页兜底；④ change 名为 kebab-case（OpenSpec 约束），URL 不做额外编码，畸形多段路径落 `*` 兜底重定向。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 路由表落地 | AppRoutes 路由表（`/changes` / `/changes/:name` / `/agent` + `/` 与 `*` 重定向 `/changes`，实现形态）；HashRouter 壳态挂载（D2） |
| AC-2 依赖收敛 | `react-router ^7` 唯一路由依赖（配置表）；`TopPage` 导出删除 + `AppSidebarProps` 收缩（类型定义）→ knip 无未用导出；路由表 / PageNavGroup / ChangeView 独立小组件不超 `max-lines-per-function: 50`；`client:check` 为阶段任务终验 |
| AC-3 Sidebar NavLink 化 | PageNavGroup NavLink 化（D8 组合）：testid 移锚点保持、active 经 `useLocation` 派生、`TopPage` / `onPageChange` 删除（修改文件 / 实现形态） |
| AC-4 选中态 URL 化 | `useParams<'name'>` 承载选中；行点击 `navigate('/changes/<name>')` → `get_change_detail` 以 URL 参数发起；返回显式 `navigate('/changes')`（ChangeView 接线） |
| AC-5 workspace 切换清选中 | D4：`resetPending` 过渡 + effect `navigate('/changes', { replace: true })`，单点覆盖 select / 移除当前根 / 添加新根 |
| AC-6 欢迎态路由隔离 | 欢迎态 gate 维持在 HashRouter 外：`root === null` 不渲染路由出口 / `SidebarProvider` / `AppSidebar`，`WelcomeView` 全屏；无 `/welcome` 路由（提案 D3） |
| AC-7 既有语义保留 | `useChangeList` 驻留 App 层、`useChangeDetail` 挂载点不变（切页卸载不重取）；NavLink 点击仅导航（零 workspace 命令）；切 Agent 后 `/changes` 无 `:name` 段 → 清单呈现、detail 不以旧选中重发；过渡轮抑制防「新根 + 旧名」误发（D4）；全部 testid 不变（`nav-changes` / `nav-agent` 移锚点保持，其余组件零改动）；`src-tauri/**` 与 hooks 零改动 |
| AC-8 spec 一致性 | specs/ delta（desktop-page-routing ADDED + desktop-app-shell MODIFIED）已随提案写入，归档合并后无「MUST NOT 引入路由」残留；本文 D1~D8 与 spec「design 定夺」条款一一对应（D6 = 挂载方式、D1/D2/D3 = replace 语义等） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| 路由位置（HashRouter location） | `pathname`（`/changes` / `/changes/:name` / `/agent`）+ history 栈 | 顶层页面与 change 选中的唯一真相源；`/` 与未知路径 replace 重定向 `/changes`（D3） | 无持久化（内存 history；应用重启回到 `/` → 清单页） |
| change 选中态 | 路由参数 `name`（`useParams` 派生 `selected`） | 取代原 `selectedChange` 本地 state（无双轨并存，spec 硬性要求）；`useChangeDetail(root, selected)` 入参来源 | 无 |
| 过渡抑制态（ChangeView `resetPending`） | boolean | 仅根切换且带旧选中的过渡轮置位：detail 入参置 null 抑制误发，导航完成后清位 | 无 |
| `WorkspaceState` / `ChangeListState` / `ChangeDetailState` | 现状字段 | 零改动：list 驻留 App 层（切页不丢、不重发）、detail 在 ChangeView 内按路由参数取数、workspace 语义不变 | 无 |
| 侧栏折叠态 | `SidebarProvider` 内部 state | 现状不变（前 change D1：会话内 state，不持久化） | 无 |

---

## 路由/API 设计

本变更不涉及 HTTP API 与 Tauri 命令变更：`src-tauri/**` 零改动，既有 invoke 命令（`list_workspaces` / `add_workspace` / `remove_workspace` / `list_changes` / `get_change_detail` / `read_artifact` 等）签名与语义不动。下表为前端壳态路由表（HashRouter，地址栏 hash 形如 `#/changes/<name>`）：

| 路径 | 渲染 | 行为 | URL 参数 |
|------|------|------|----------|
| `/` | `<Navigate replace to="/changes" />` | 根路径重定向清单页（D3：replace） | 无 |
| `/changes` | `<ChangeView list root />` | 变更清单页；行点击导航至详情 | 无 |
| `/changes/:name` | `<ChangeView list root />` | 变更详情页；`get_change_detail` 以 `:name` 发起；不存在时既有降级页兜底 | `name`（change 目录名，kebab-case） |
| `/agent` | `<AgentDebugView root />` | Agent 调试页 | 无 |
| `*`（未知路径） | `<Navigate replace to="/changes" />` | 兜底回清单页，不渲染空白或崩溃 | 无 |

---

## 依赖

### 运行时依赖

- `react-router`（^7.x）— 路由运行时：`HashRouter` / `Routes` / `Route` / `Navigate` / `NavLink` / `useNavigate` / `useParams` / `useLocation`；v7 declarative 模式（`react-router` 单包，不引入 `react-router-dom`，不使用 loader/action 数据抽象）

### 构建/测试依赖

- 无新增（vp check / knip / vp test 沿用现有 vite-plus 工具链；knip.json 等配置零改动、无新增豁免）

---

## 待决问题

- 无阻塞性未决。proposal「待决问题」四项均已在本文定夺：ChangeView 保留 vs 溶解（D1）、Router 挂载位置与测试挂载方式（D2 / D6）、版本 bump 档位（D5）、深链恢复场景（D7）。
- 非阻塞默认约定（实现阶段按默认执行，无需回评）：react-router 安装取 `^7` 范围内最新稳定版本；路由级测试的具体用例拆分与断言由 test-design / test-gen 阶段承接。
