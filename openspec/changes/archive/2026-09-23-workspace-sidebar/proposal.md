# 提案: workspace-sidebar

> **变更**: workspace-sidebar
> **日期**: 2026-09-22
> **状态**: 草案

---

## 问题

desktop 壳态的全部 workspace 操作（切换、移除、刷新）挤压在顶部 header：原生 `select` 下拉既无法呈现多 workspace 全貌，也不可扩展；移除与刷新按钮并排悬在全局 header 上，控件语义与数据归属错位（刷新属列表数据，不属全局壳层）。

错误呈现为单一 `error` 态混装两类失败：`useWorkspaces.error` 同时承载清单加载失败与 add/remove/touch 动作失败，AppHeader 与 WelcomeView 渲染同一 `error-note`（`App.tsx:94` / `WelcomeView.tsx:16`）。动作失败的瞬时错误以持久 inline 形式常驻 header，既无时效性又造成布局跳动；而若一律改 toast，启动 `list_workspaces` 失败时错误一闪即逝，用户面对「最近的 workspace (0)」空态会误以为没有 workspace。

前置依赖 `tailwind-migration` 已归档（2026-09-22）：Tailwind v4 + shadcn 地基、data-testid 测试挂钩、`src/components/ui/**` 内部化纪律均已落地，本 change 在其之上引入 shadcn Sidebar 块。

---

## 提案

引入 shadcn/ui Sidebar 块，为 desktop 前端提供独立的 workspace 清单侧栏与管理面板，**后端零改动**（仅现有 4 命令 `list_workspaces` / `add_workspace` / `remove_workspace` / `touch_workspace`，`desktop-workspace-store` 不动）：

1. **Sidebar 壳**：壳态（存在已打开 workspace）以 `SidebarProvider` + `AppSidebar` + `SidebarInset` 承载布局，main 区 max-width 1100px 居中在 `SidebarInset` 内维持；欢迎态（root 为 null）不挂载壳，`WelcomeView` 全屏维持现状；Toaster（sonner）在 App 根挂载一次，两态均覆盖。
2. **清单管理**：sidebar「工作区」组呈现全部清单项——点击切换（`touch` → 清单重排 → 恒取第一名，spec 既有语义仅 UI 面收敛）、组标签右侧 `SidebarGroupAction` 内联图标走文件夹选择器添加流、右键 `ContextMenu` 移除（仅清单位移除，不删盘上目录、无确认弹窗）、悬停 Tooltip 展示完整 root、`SidebarMenuButton` 副文本区分同名项。
3. **header 瘦身**：移除顶部 `select`，sidebar 为唯一 workspace 列表与切换入口；header 终态仅折叠钮 + 标题 + 版本/更新指示；刷新从 header 迁入清单页（ChangeListView）头部，`disabled={loading}` 语义随行。
4. **错误呈现双轨**：动作类接口（add/remove/touch）失败统一 toast（sonner）；查询类接口（启动 `list_workspaces` / `list_changes` / `get_change_detail` / `read_artifact`）失败保留 error 态 inline 持久呈现；`WorkspaceState.error` 语义收窄为「清单加载失败」；updater 错误保留「重试更新」按钮不变。
5. **折叠形态**：`collapsible="icon"` + Tooltip；接受窗口 < 768px 时 `use-mobile` 触发的 Sheet 抽屉第三态与内建 Ctrl/Cmd+B 折叠快捷键；不引入路由（列表 ↔ 详情维持 `ChangeView` 本地 state）。

不做（维持现状）：重命名（name 自动取目录名最后一段）、置顶/固定（排序维持 `last_opened_at` 降序）、移除确认弹窗、全屏欢迎态改版。

---

## 能力

### 新增能力

- 无（sidebar 壳作为新 requirement 并入既有 `desktop-app-shell` 能力，不单开能力）

### 修改的能力

- `desktop-app-shell` — 新增「Sidebar 壳层布局」与「错误呈现双轨」两个 requirement；改写「workspace 选择」（下拉切换 → sidebar 列表项切换、Header 移除 → 右键菜单移除、title → Tooltip）、「React 前端刷新取数模型」（清单呈现形态与刷新入口位置）、「workspace 注册命令轨道」（reject 呈现子句指向双轨语义）、「Tailwind v4 单一样式体系」（布局冻结子句收窄为迁移期约束）

---

## 变更范围

### 实现文件

- `packages/desktop/package.json` — 新增依赖：`@radix-ui/react-separator` / `@radix-ui/react-dialog` / `@radix-ui/react-tooltip` / `@radix-ui/react-context-menu` / `lucide-react` / `sonner`
- `packages/desktop/src/components/ui/sidebar.tsx`（新）及其依赖生成件 `separator.tsx` / `sheet.tsx` / `tooltip.tsx` / `skeleton.tsx` / `input.tsx` / `context-menu.tsx` / `sonner.tsx`（按需引入，含去 Next 语境残留）；内部化纪律沿用：过 fmt / lint / knip 全管线、无新增豁免
- `packages/desktop/src/hooks/use-mobile.ts`（新，shadcn 随附断点 hook）
- `packages/desktop/src/components/AppSidebar.tsx`（新）— workspace 清单组：列表项点击切换、`SidebarGroupAction` 添加、右键 `ContextMenu` 移除、Tooltip 完整 root、副文本区分同名
- `packages/desktop/src/App.tsx` — 壳重排（`SidebarProvider` / `SidebarInset`）、移除 `WorkspaceSelect` / `AppHeader`、header 终态、Toaster App 根挂载
- `packages/desktop/src/hooks/useWorkspaces.ts` — 错误双轨：动作失败转 toast、error 态收窄为加载失败（hook 形态 design 定）
- `packages/desktop/src/views/WelcomeView.tsx` — error-note 收窄为仅清单加载失败呈现；添加流保留
- `packages/desktop/src/views/changes/ChangeListView.tsx` — 头部刷新按钮迁入（`disabled={loading}`）
- `packages/desktop/src/styles/global.css` — toast 相关 token（如需）

### 测试文件

- `packages/desktop/src/App.test.tsx` — 下拉相关 3 用例改写为 sidebar 列表项交互；`.error-note` 动作失败类断言改 toast 文本断言（sonner 在 jsdom 可 `getByText`）；IPC 断言（`touch_workspace` → 新根 `list_changes`）逐字保留
- `packages/desktop/src/__tests__/workspace_restore.test.tsx` — 同类改写
- `packages/desktop/src/hooks/useWorkspaces.test.ts` — 动作失败返回值 / toast 语义
- `packages/desktop/src/views/changes/ChangeListView.test.tsx` — 刷新入口迁入用例（列表加载失败 inline 断言保留不动）
- `packages/desktop/src/components/AppSidebar.test.tsx`（新）— 列表渲染、右键菜单（`fireEvent.contextMenu` + radix jsdom 模拟）、副文本 testid
- 查询类 inline 断言（`ChangeDetailView.test.tsx`、`__tests__/ipc_pipeline.test.tsx` 中列表/详情加载失败呈现）保留不动

### 删除文件

- 无删除文件（`WorkspaceSelect` / `AppHeader` 组件随 `App.tsx` 重排移除）

### 不要修改

- `packages/desktop/src-tauri/**` 全部（commands / store / redb 表零改动）
- `openspec/specs/desktop-workspace-store/spec.md` 既有语义（含「MUST NOT 静默吞掉失败」承诺）
- 切换底层语义（`touch` 重排、恒取第一名、移除后切剩余第一名、切换清空 change 选中）
- `useUpdater` 错误呈现（「重试更新」按钮保留）
- `ChangeView` 本地 state 视图切换模型（不引入路由）
- 插件产物目录（`plugins/` / `claude-plugins/` / `cursor-plugins/`）——本 change 不涉插件源

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `App.tsx` 壳重排 + `AppSidebar.tsx` 新建 | 壳态渲染 `SidebarProvider` / `AppSidebar` / `SidebarInset`；页面无 `combobox`（`getByRole('combobox')` 抛错）；header 仅折叠钮 + 标题 + 版本/更新指示 |
| AC-2 | 欢迎态隔离 | root 为 null 时不渲染 `SidebarProvider` / `AppSidebar` DOM，`WelcomeView` 全屏；Toaster 在欢迎态与壳态均挂载 |
| AC-3 | sidebar 切换流 | 点击清单项 → `touch_workspace` → 清单重排取第一名为新根，change 选中清空、`list_changes` 以新根重取（IPC 断言与现状逐字一致）；悬停 Tooltip 展示完整 root；同名项副文本经 testid 断言 |
| AC-4 | 添加入口 | 「工作区」组标签右侧 `SidebarGroupAction` 内联图标 → 文件夹选择器 → `add_workspace` 入库 → 新记录（第一名）打开；欢迎屏「添加新文件夹」入口保留 |
| AC-5 | 右键移除 | `fireEvent.contextMenu` 打开菜单完成移除；移除当前根 → 切剩余第一名、空则回欢迎屏；移除非当前项 → 当前根不变；无确认弹窗、不删盘上目录 |
| AC-6 | 刷新迁移 | 清单页头部刷新按钮带 `disabled={loading}`；header 无刷新按钮 |
| AC-7 | 折叠形态 | `collapsible="icon"` + Tooltip 生效；窗口 < 768px 呈现 Sheet 抽屉；Ctrl/Cmd+B 切换折叠 |
| AC-8 | 错误双轨 | add/remove/touch reject → toast 呈现错误文本且 `queryAllByTestId('error-note')` 为 0；`list_workspaces` / `list_changes` / `get_change_detail` / `read_artifact` reject → error-note 持久渲染不消失；全部成功 → 无 toast 无 error-note；updater「重试更新」按钮行为不变 |
| AC-9 | 后端零改动 | `git diff packages/desktop/src-tauri` 为空；`desktop-workspace-store` spec 无本 change delta |
| AC-10 | 管线守线 | `pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过且 ui/** 无新增豁免条目；`vp test` 全绿；新增 ui 生成件的 mutate 纳入/排除处置与理由成对（沿用 tailwind-migration 口径） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| jsdom + Radix（Tooltip / Sheet / ContextMenu 的 portal 与 pointer 事件）测试脆弱 | 用例 flaky 阻塞 `vp test` | 中 | design 先定 waitFor / pointer 模拟细节；radix 交互收敛到薄包装、以 testid 断言最终态 |
| sonner toast 异步渲染时序 | toast 断言 flaky | 中 | 测试内挂载 Toaster、固定文案、约定 waitFor 用法 |
| `useWorkspaces` 契约 shape 变更波及 | `WorkspaceState` 及既有 hook 测试连锁改 | 中 | design 先定 hook 形态（直调 toast vs 返回值判定）再动视图 |
| 折叠态持久化引入 localStorage 边界 | Tauri webview storage 行为差异 | 低 | design 可选会话内 state 规避 storage 依赖 |
| 新增生成件触发 knip 未用导出或超线函数 | `client:check` 失败 | 低 | 沿内部化纪律：未用导出删减、超线拆文件消化，不加豁免 |
| 新增 ui 交互逻辑稀释 mutation 分数 | 跌破 break 50 | 低 | 交互逻辑落测试，或按既有口径处置并记录理由 |
| 与 tailwind-migration 的「先迁后删」代价 | error-note 样式两段式改动 | 低 | 前置已归档；仅动作类改 toast，加载类 inline 断言不动 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 「配置」的含义与后端面 | 纯前端清单管理，只用现有 4 命令 | 4 命令 + redb 两表已覆盖全部清单动作 | 后端新增排序 / 重命名字段 |
| workspace 切换入口 | 移除顶部 select，sidebar 唯一入口 | 双入口状态同步成本高、header 过载 | 下拉与 sidebar 并存 |
| 欢迎态是否挂 sidebar | 不挂（WelcomeView 全屏），Toaster 例外挂 App 根 | 空清单无可列项；toast 需覆盖两态的失败反馈 | 全程挂载空 sidebar |
| 添加入口形态 | 组标签右侧 `SidebarGroupAction` 内联图标 | 贴近清单语义，免底部冗余按钮 | 底部固定「添加」按钮 |
| 移除入口形态 | 清单项右键 `ContextMenu` | 避免 hover 双控件拥挤；天然支持移除任一清单项 | hover ⋮；仅当前项按钮 |
| 刷新入口 | 迁入清单页头部 | 刷新语义属列表数据，不属全局壳层 | 保留 header 刷新 |
| 错误呈现 | 双轨：动作 toast / 查询 inline 持久 | 启动清单失败 toast 一闪即逝，会被空态误读为无 workspace | 全部 toast；全部 error-note |
| updater 错误呈现 | 保留「重试更新」按钮 | 恢复动作非纯呈现，toast 无处安放重试 | 改 toast |
| 重命名 / 置顶 / 排序 | 不做 | name 自动取目录尾段、排序维持 `last_opened_at` 降序既有语义 | 增加重命名与固定排序字段 |

### 待决问题

- 折叠态持久化：去 cookie 后换 localStorage vs 会话内 state（#14 遗留）
- 错误双轨的 hook 形态：hook 内直调 toast vs 保留返回值由视图 effect 触发（影响 `WorkspaceState` 契约 shape）
- 同名区分副文本的具体内容：父目录 vs root 尾段（#17 遗留）
- jsdom + Radix（Tooltip / Sheet / ContextMenu）的 waitFor 与 pointer 模拟细节

---
