# 设计: workspace-sidebar

> **变更**: workspace-sidebar
> **日期**: 2026-09-22

---

## 提案与规格同步状态

`proposal.md` 与 `openspec/changes/workspace-sidebar/specs/desktop-app-shell/spec.md`（ADDED×2 + MODIFIED×3 delta）已由提案阶段写入并通过，不在本 design 变更清单与任务范围内。本文只覆盖提案「变更范围 - 实现文件」的实现设计；测试文件由 test-design / test-gen 阶段承接。

---

## 关键设计决策（proposal 待决问题定夺）

proposal「待决问题」四项 + 实现形态定夺如下，均为本文约束，实现阶段照此落地：

| # | 问题 | 定夺 | 理由 |
|---|------|------|------|
| D1 | 折叠态持久化 | **会话内 state**（SidebarProvider 内 React state，不持久化） | 去 cookie 是内部化纪律硬性要求；localStorage 在 Tauri webview 引入 storage 边界（proposal 风险表低概率项）却只换来「重启后记住折叠」这一弱收益；会话内 state 零持久化面、零新依赖 |
| D2 | 错误双轨 hook 形态 | **hook 内直调 toast**：`useWorkspaces` 的 add/remove/touch catch 分支直接 `toast.error()`，动作路径不再置 error；返回值（`WorkspaceRecord \| null` / `boolean`）保留作流程控制 | 无需视图层 effect 桥接、无新增契约面；`WorkspaceState` shape 不变（`error` 字段保留但语义收窄为「清单加载失败」），既有调用方（App / WelcomeView）改动最小 |
| D3 | 同名区分副文本内容 | **父目录**：root 去掉最后一段（最后一个 `/` 或 `\` 之前）的完整前缀；无分隔符时副文本为空串不显示 | name 已是 root 尾段，副文本必须提供正交信息；同名项（如 `C:\a\plugin` 与 `C:\b\plugin`）父目录必不同（canonical root 唯一，upsert 去重保证同父同名不存在），区分能力完备；Tooltip 仍展示完整 root，信息层级为「副文本=父目录、Tooltip=全路径」 |
| D4 | jsdom + Radix 交互机制 | ① `ContextMenu` 以 `ContextMenuTrigger asChild` 包裹每个 `SidebarMenuItem`，jsdom 下 `contextmenu` 事件即可打开，菜单项以文案查询；② Tooltip 走 `SidebarMenuButton` 内建 tooltip prop，`TooltipProvider delayDuration={0}`（生成件内部化允许调整）使 hover 即显；③ Sheet 第三态经 `useIsMobile`（`matchMedia('(max-width: 767px)')`），jsdom 需 stub `window.matchMedia`；④ 断言一律收敛 testid 最终态，不在测试里复刻 portal 细节 | proposal 风险表要求 design 先定机制；radix 交互不外泄到视图，测试只碰稳定挂钩 |
| D5 | ui 生成件引入集 | 引入 `sidebar` / `separator` / `sheet` / `tooltip` / `context-menu` / `sonner` 六件；**不引入** `skeleton.tsx` / `input.tsx`——sidebar.tsx 内对应的未用子组件（`SidebarMenuSkeleton` / `SidebarInput` 等）直接裁剪 | knip 未用导出纪律（tailwind-migration 口径）：不留无消费面的生成件与其依赖文件 |
| D6 | mutate 处置（AC-10） | `stryker.config.json` **零改动**：`src/components/ui/**` 维持既有排除（理由成对：无对应测试投入的生成件纳入 mutate 只产噪声，与 tailwind-migration 定案一致）；`AppSidebar.tsx` / `use-mobile.ts` 属应用代码、不在 ui/** glob 内，自然纳入 mutate | AC-10 要求处置与理由成对；排除口径延续既有配置，无新豁免 |
| D7 | 侧栏内层 main 语义 | `SidebarInset` 本身即 `<main>`；原 App.tsx 的内层 `<main className="mx-auto w-full max-w-[1100px] ...">` 改为同 class 的 `<div>`，max-width 1100px 居中形态不变 | 避免嵌套 main；布局冻结条款由「Sidebar 壳层布局」requirement 承接，1100px 居中是长期形态 |
| D8 | toast 文案与形态 | 固定前缀：`添加 workspace 失败：{message}` / `移除 workspace 失败：{message}` / `切换 workspace 失败：{message}`；`<Toaster />` 以 sonner 默认参数挂载（bottom-right、默认时长）；remove resolve(false)（store miss，幂等非错误）**不 toast**；启动恢复 touch 失败同走 toast（fire-and-forget 不阻断恢复链） | 固定文案供断言；幂等 miss 非失败不该惊扰；恢复链时序为 spec 既有语义不动 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| App 壳 | 壳态/欢迎态分流；壳态以 `SidebarProvider` + `AppSidebar` + `SidebarInset` 承载；header 终态（SidebarTrigger + 标题 + UpdateIndicator）；`Toaster` App 根挂载一次；pickAndAdd 添加流 | `packages/desktop/src/App.tsx` | useWorkspaces / useChangeList / useUpdater / AppSidebar / sonner | React 19 + shadcn Sidebar |
| AppSidebar | workspace 清单侧栏：「工作区」组呈现全部清单；列表项点击切换（touch）、`SidebarGroupAction` 添加、右键 `ContextMenu` 移除、Tooltip 完整 root、副文本区分同名 | `packages/desktop/src/components/AppSidebar.tsx`（新） | ui/sidebar / ui/context-menu / lucide-react / `@/lib/utils` | shadcn Sidebar 块 + Radix ContextMenu |
| ui/sidebar | shadcn Sidebar 生成件（裁剪版）：provider/折叠态（icon + Ctrl/Cmd+B）/mobile Sheet 分流/inset/菜单原语 | `packages/desktop/src/components/ui/sidebar.tsx`（新） | ui/button / ui/separator / ui/sheet / ui/tooltip / use-mobile / lucide-react | shadcn vendored 内部化 |
| ui/separator、ui/sheet、ui/tooltip、ui/context-menu | sidebar 与右键菜单的 Radix 薄包装生成件（按消费面裁剪导出） | `packages/desktop/src/components/ui/*.tsx`（新） | @radix-ui/react-separator / react-dialog / react-tooltip / react-context-menu | shadcn vendored 内部化 |
| Toaster 包装 | sonner Toaster；`toastOptions` classNames 映射既有 token（bg-card / text-foreground / border-border），配色身份不引入 shadcn 默认主题 | `packages/desktop/src/components/ui/sonner.tsx`（新） | sonner / use-mobile | shadcn vendored 内部化 |
| use-mobile | 断点 hook：窗口 < 768px → true，SidebarProvider 据 its 切 Sheet 抽屉第三态 | `packages/desktop/src/hooks/use-mobile.ts`（新） | 无 | React + matchMedia |
| useWorkspaces | 清单取数收口 + **错误双轨落点**：动作失败（add/remove/touch）直调 toast；`error` 仅承载 `list_workspaces` 加载失败；切换语义不变（touch → 清单重排 → 恒取第一名） | `packages/desktop/src/hooks/useWorkspaces.ts` | @tauri-apps/api/core / sonner | React hooks |
| ChangeListView | change 列表视图；**新增头部刷新按钮**（`disabled={state.loading}`，语义从 header 迁入）；列表加载失败 error-note inline 保留 | `packages/desktop/src/views/changes/ChangeListView.tsx` | ui/button / ChangeListState | React |
| WelcomeView | 欢迎屏空态 +「添加新文件夹」入口；error-note 因 hook 语义收窄而仅呈现清单加载失败（结构性不动） | `packages/desktop/src/views/WelcomeView.tsx` | ui/button / WorkspaceState | React |
| 样式入口 | Tailwind v4 唯一样式入口；本 change 定案**不新增 token** | `packages/desktop/src/styles/global.css` | tailwindcss | Tailwind v4 `@theme inline` |

组件间数据流（零后端改动，单入口）：

```
AppSidebar 列表项点击 ─▶ useWorkspaces.touch(root) ─▶ 清单重排 ─▶ root 恒取第一名（新根）
AppSidebar GroupAction ＋ ─▶ App.pickAndAdd（文件夹选择器）─▶ add_workspace ─▶ 新记录（第一名）打开
AppSidebar 右键「移除」 ─▶ useWorkspaces.remove(root) ─▶ 刷新清单 ─▶ 剩余第一名 / 空→欢迎屏
 动作 reject ─▶ toast.error（hook 内直调）      查询 reject ─▶ error 态 inline 持久（双轨）
```

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/components/ui/sidebar.tsx` | shadcn Sidebar 裁剪版：导出集 pin 为 `SidebarProvider` / `Sidebar` / `SidebarTrigger` / `SidebarInset` / `SidebarGroup` / `SidebarGroupLabel` / `SidebarGroupAction` / `SidebarGroupContent` / `SidebarMenu` / `SidebarMenuItem` / `SidebarMenuButton`（`useSidebar` 仅内部消费、不导出）；去 cookie 持久化残留（D1：折叠态会话内 state，`defaultOpen` 固定 true）；`collapsible="icon"` + 内建 Ctrl/Cmd+B 快捷键保留；`TooltipProvider delayDuration={0}`（D4）；未用子组件（MenuSkeleton / MenuSub / Input 等）与未用导出删减，超 `max-lines-per-function: 50` 的生成函数拆内部组件消化 |
| `packages/desktop/src/components/ui/separator.tsx` | Radix Separator 薄包装，sidebar.tsx 布局分隔用 |
| `packages/desktop/src/components/ui/sheet.tsx` | Radix Dialog 薄包装（Sheet），sidebar < 768px 抽屉第三态；仅保留 `Sheet` / `SheetContent`（含侧向 variant）等被 sidebar.tsx 消费的导出 |
| `packages/desktop/src/components/ui/tooltip.tsx` | Radix Tooltip 薄包装：`Tooltip` / `TooltipTrigger` / `TooltipContent` / `TooltipProvider`，供 SidebarMenuButton tooltip prop 消费 |
| `packages/desktop/src/components/ui/context-menu.tsx` | Radix ContextMenu 薄包装：仅保留 `ContextMenu` / `ContextMenuTrigger` / `ContextMenuContent` / `ContextMenuItem`（Sub/Checkbox/Radio 等未用导出删减），供 AppSidebar 右键移除消费 |
| `packages/desktop/src/components/ui/sonner.tsx` | sonner Toaster 包装：`toastOptions.classNames` 映射既有 token，去 Next 语境残留 |
| `packages/desktop/src/hooks/use-mobile.ts` | `useIsMobile()` 断点 hook（768px），sidebar.tsx 与 sonner.tsx 消费 |
| `packages/desktop/src/components/AppSidebar.tsx` | workspace 清单侧栏（应用组件，非 ui/**）：结构见下「AppSidebar 形态」 |

**AppSidebar 形态**（实现约束）：

- props：`workspaces: WorkspaceRecord[]`、`currentRoot: string`、`onOpen(root: string): void`、`onAdd(): void`、`onRemove(root: string): void`
- 「工作区」`SidebarGroup`：`SidebarGroupLabel`（文案「工作区」）+ `SidebarGroupAction`（lucide `Plus` 图标，`aria-label="添加 workspace"`，onClick → `onAdd`）
- 清单项 = `ContextMenu`（Trigger asChild 包裹）→ `SidebarMenuItem` → `SidebarMenuButton`：
  - `isActive={record.root === currentRoot}`；`tooltip={record.root}`（完整 root，D4 的 delayDuration=0 生效）
  - 主文本 `record.name`；副文本 block 级小字 span，内容为父目录（D3），`data-testid="workspace-sub"`
  - `data-testid="workspace-item"` + `data-root={record.root}` 作用域属性（同名场景以 `data-root` 定位到具体项后 `within()` 取副文本，不依赖 accessible name）
  - onClick → `onOpen(record.root)`
- ContextMenu 内容：`ContextMenuItem`（文案「移除」）onClick → `onRemove(record.root)`；无确认弹窗、前端仅调 `remove_workspace`，不触盘上目录
- 列表项子组件（如 `WorkspaceItem`）拆分保 `max-lines-per-function` 线

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/package.json` | dependencies 新增 `@radix-ui/react-separator`、`@radix-ui/react-dialog`、`@radix-ui/react-tooltip`、`@radix-ui/react-context-menu`、`lucide-react`、`sonner` | 依赖与用途见「依赖」节；knip.json / stryker.config.json / vite.config.ts / components.json / tsconfig 零改动（D6，无新增豁免） |
| `packages/desktop/src/App.tsx` | ① 删除内部组件 `WorkspaceSelect` 与 `AppHeader`（非独立文件，无删除文件条目）；② 壳态改为 `SidebarProvider` → `AppSidebar` + `SidebarInset`，内层 main 改 div（D7）；③ 新 header 终态：`SidebarTrigger` + 标题「Desktop Terminal」+ `UpdateIndicator`（保留原函数与「重试更新」语义）；④ `<Toaster />` 与条件渲染同级置于 App 根，欢迎态/壳态均覆盖；⑤ 欢迎态（root === null）不渲染 `SidebarProvider` / `AppSidebar` DOM；⑥ `pickAndAdd` 保留（对话框参数 `{ directory: true, multiple: false }` 不变），`record === null` 注释由「error 态已呈现」改为「toast 已呈现」 | AC-1 / AC-2 / AC-4 / AC-9 |
| `packages/desktop/src/hooks/useWorkspaces.ts` | ① import `toast` from 'sonner'；② `useWorkspacesActions` 三个动作的 catch 分支改为 `toast.error(固定前缀 + String(err))` 并移除 `setError` 调用（D8 文案）；参数 `setError` 从 `useWorkspacesActions` 移除；③ `list_workspaces` 加载失败分支保留 `setError(String(err))` 不变；④ `WorkspaceState` 接口 shape 不变，JSDoc 更新 error 语义与双轨说明 | AC-8 核心落点；切换/恢复/移除后重排语义逐字不动 |
| `packages/desktop/src/views/WelcomeView.tsx` | 无结构性改动：error-note 区块因 hook 语义收窄自动仅呈现清单加载失败；JSDoc 与「入库失败」相关注释对齐双轨措辞；「添加新文件夹」入口与 `onAdd` 流保留 | AC-2 / AC-4 / AC-8 |
| `packages/desktop/src/views/changes/ChangeListView.tsx` | 顶部新增头部行（`flex items-center justify-end`，先于 error-note 与数据区，始终渲染）：`Button` 文案「刷新列表」，`onClick={state.refresh}`、`disabled={state.loading}`（props 无变化，`ChangeListState` 已含 refresh/loading） | AC-6；「暂无数据，点击刷新获取」空态由此保持可操作 |
| `packages/desktop/src/styles/global.css` | 定案零新增（D6 同源）：Toaster 配色经 sonner.tsx 的 classNames 映射既有 token，不新增 CSS；仅当实现实测对比度不足时允许追加 `@theme inline` 映射行，追加范围以此为限 | 对齐提案「toast 相关 token（如需）」条款；如最终零 diff 属预期结果 |

<!-- 删除文件：无。proposal 明确「无删除文件」；WorkspaceSelect / AppHeader 为 App.tsx 内部函数组件，随壳重排从该文件移除，不构成文件删除。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `AppSidebar` | `packages/desktop/src/components/AppSidebar.tsx` | 新增 | `function AppSidebar(props: AppSidebarProps): React.JSX.Element` | props 见「新增文件」节；仅壳态挂载 |
| `Toaster` | `packages/desktop/src/components/ui/sonner.tsx` | 新增 | `function Toaster(props: ComponentProps<typeof SonnerToaster>): React.JSX.Element` | App 根挂载一次，两态覆盖 |
| `SidebarProvider` | `packages/desktop/src/components/ui/sidebar.tsx` | 新增 | `function SidebarProvider(props: ComponentProps<'div'> & { defaultOpen?: boolean; open?: boolean; onOpenChange?: (open: boolean) => void }): React.JSX.Element` | 折叠态会话内 state（D1）；去 cookie 写入 |
| `Sidebar` / `SidebarInset` / `SidebarTrigger` | `packages/desktop/src/components/ui/sidebar.tsx` | 新增 | `SidebarTrigger(props: ComponentProps<typeof Button>): React.JSX.Element` 等 | `collapsible="icon"`；Trigger 置于壳态 header |
| `SidebarGroup` / `SidebarGroupLabel` / `SidebarGroupAction` / `SidebarGroupContent` | `packages/desktop/src/components/ui/sidebar.tsx` | 新增 | shadcn 原语签名（`SidebarGroupAction` 含 `tooltip?: string`） | 「工作区」组与内联添加图标 |
| `SidebarMenu` / `SidebarMenuItem` / `SidebarMenuButton` | `packages/desktop/src/components/ui/sidebar.tsx` | 新增 | `SidebarMenuButton(props: React.ComponentProps<'button'> & { isActive?: boolean; tooltip?: string \| ReactElement; asChild?: boolean }): React.JSX.Element` | 变体集按本 app 实际用量裁剪（仅 default，同 ui/button 口径） |
| `Sheet` / `SheetContent` | `packages/desktop/src/components/ui/sheet.tsx` | 新增 | `function SheetContent(props: ComponentProps<typeof SheetPrimitive.Content> & { side?: 'top' \| 'right' \| 'bottom' \| 'left' }): React.JSX.Element` | 仅保留 sidebar.tsx 消费面 |
| `Tooltip` / `TooltipTrigger` / `TooltipContent` / `TooltipProvider` | `packages/desktop/src/components/ui/tooltip.tsx` | 新增 | shadcn 原语签名 | delayDuration=0 由 sidebar.tsx provider 设定（D4） |
| `ContextMenu` / `ContextMenuTrigger` / `ContextMenuContent` / `ContextMenuItem` | `packages/desktop/src/components/ui/context-menu.tsx` | 新增 | shadcn 原语签名 | 右键移除菜单（AC-5） |
| `Separator` | `packages/desktop/src/components/ui/separator.tsx` | 新增 | `function Separator(props: ComponentProps<typeof SeparatorPrimitive.Root>): React.JSX.Element` | sidebar 布局分隔 |
| `useIsMobile` | `packages/desktop/src/hooks/use-mobile.ts` | 新增 | `function useIsMobile(): boolean` | `< 768px` 断点；内部 `isMobile` state + matchMedia change 监听 |
| `useWorkspaces` | `packages/desktop/src/hooks/useWorkspaces.ts` | 修改 | `function useWorkspaces(): WorkspaceState`（签名不变） | 动作失败直调 toast、`error` 语义收窄为清单加载失败（D2） |
| `App` | `packages/desktop/src/App.tsx` | 修改 | `export default function App(): React.JSX.Element`（签名不变） | 壳重排；内部 `WorkspaceSelect` / `AppHeader` 移除，`UpdateIndicator` 保留 |

<!-- 说明：导出 class 无；CLI 子命令 / HTTP 端点无。组件导出的子原语（如 SheetHeader 等）按消费面裁剪后不在导出集内。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `WorkspaceState` | `packages/desktop/src/hooks/useWorkspaces.ts` | 修改 | shape 不变（`root` / `workspaces` / `loading` / `error` / `add` / `remove` / `touch`）；`error` 语义收窄为「`list_workspaces` 加载失败」，JSDoc 同步 |
| `AppSidebarProps` | `packages/desktop/src/components/AppSidebar.tsx` | 新增 | `interface AppSidebarProps { workspaces: WorkspaceRecord[]; currentRoot: string; onOpen: (root: string) => void; onAdd: () => void; onRemove: (root: string) => void }` |
| `WorkspaceRecord` | `packages/desktop/src/types/dto.ts` | 引用（零改动） | 副文本/tooltip 的数据来源（`name` / `root`）；后端 DTO 镜像不动（AC-9） |

<!-- 配置：无配置文件键变更。依赖新增记录于 package.json（见「依赖」节）；knip.json / stryker.config.json / vite.config.ts / components.json 本 change 明确零改动、无新增豁免条目（AC-10）。 -->

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | App.tsx 壳态 `SidebarProvider` + `AppSidebar` + `SidebarInset`（变更清单-修改文件）；header 终态 = `SidebarTrigger` + 标题 + `UpdateIndicator`，`select`/移除/刷新控件全部移除（无 combobox 可查）；main 1100px 居中移入 inset 内 div（D7） |
| AC-2 | App 根条件渲染：root === null 时不挂 provider/sidebar；`<Toaster />` 与条件渲染同级置于 App 根，两态覆盖（变更清单-App.tsx ④⑤） |
| AC-3 | AppSidebar 列表项 onClick → `onOpen` → `workspaceState.touch` → 清单重排恒取第一名；`SidebarMenuButton tooltip={record.root}`（完整 root）；副文本父目录 + `data-testid="workspace-sub"`、`data-root` 作用域（D3/D4、AppSidebar 形态） |
| AC-4 | `SidebarGroupAction`（Plus 图标，aria-label「添加 workspace」）→ `App.pickAndAdd`（文件夹选择器参数不变）→ `add_workspace` 入库 → 刷新后第一名打开；WelcomeView「添加新文件夹」保留 |
| AC-5 | `ContextMenu`（Trigger asChild 包裹清单项）→「移除」→ `onRemove` → `remove_workspace`；移除当前根经 hook 刷新自动切剩余第一名、空则 root=null 回欢迎屏；无确认弹窗、无盘上目录操作 |
| AC-6 | ChangeListView 头部行刷新按钮 `onClick={state.refresh}` + `disabled={state.loading}`；App header 终态无刷新按钮 |
| AC-7 | sidebar.tsx `collapsible="icon"` + SidebarMenuButton tooltip + 内建 Ctrl/Cmd+B；`useIsMobile` < 768px 时 provider 切 Sheet 抽屉（D4-③）；折叠态会话内 state（D1） |
| AC-8 | useWorkspaces 双轨（D2/D8）：动作 reject → `toast.error(固定前缀+错误)` 且 error 态不再置位（error-note 为 0）；查询 reject（`list_workspaces`/`list_changes`/`get_change_detail`/`read_artifact`）→ error 态透传 inline 持久（WelcomeView / ChangeListView / ChangeDetailView 既有 error-note 不动）；全成功零呈现；`UpdateIndicator` 与「重试更新」逐字不动 |
| AC-9 | 变更清单不含 `packages/desktop/src-tauri/**` 任何文件；tasks 无后端任务；`desktop-workspace-store` spec 无本 change delta（规格同步状态节） |
| AC-10 | D5/D6：生成件按消费面裁剪导出（knip 守线）、不引入 skeleton/input、超线函数拆分；knip.json / stryker.config.json / vite.config.ts 零改动无新豁免；ui/** mutate 维持排除（理由成对）、AppSidebar / use-mobile 纳入 mutate；`pnpm -C packages/desktop run client:check` 与 `vp test` 全绿为任务终验 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `WorkspaceRecord`（前端 DTO 镜像） | `root` / `name` / `addedAt` / `lastOpenedAt` | 后端 redb WORKSPACES 表经 4 命令（list/add/remove/touch_workspace）访问；本 change 零改动 | redb（现状，AC-9 不动） |
| `WorkspaceState`（React 运行时状态） | `root` / `workspaces` / `loading` / `error` + `add` / `remove` / `touch` | root 恒等清单第一名；error 仅承载清单加载失败（双轨收窄） | 无持久化（组件生命周期内） |
| 侧栏折叠态 | `open` / `openMobile`（SidebarProvider 内部 state） | mobile 态走 openMobile（Sheet），desktop 态走 open（icon 折叠） | **会话内 state，不持久化**（D1：无 localStorage、无 cookie） |
| `ChangeListState` / `ChangeDetailState` | 现状字段 | 列表 ↔ 详情维持 `ChangeView` 本地 state（`selectedChange`），不引入路由 | 无持久化 |

---

<!-- 路由/API 设计：本变更无 HTTP API。Tauri IPC 仅消费既有 4 命令（list_workspaces / add_workspace / remove_workspace / touch_workspace），签名与 reject→Err(String) 契约零改动（AC-9），不新增命令、不改查询轨道。 -->

## 依赖

### 运行时依赖

- `@radix-ui/react-separator`（^1.x）— ui/separator，Sidebar 布局分隔
- `@radix-ui/react-dialog`（^1.x）— ui/sheet，< 768px 抽屉第三态
- `@radix-ui/react-tooltip`（^1.x）— ui/tooltip，折叠态与清单项 Tooltip
- `@radix-ui/react-context-menu`（^2.x）— ui/context-menu，清单项右键移除
- `lucide-react`（最新稳定）— Plus / Folder / 折叠钮等图标（shadcn sidebar 块图标来源）
- `sonner`（^2.x）— 动作失败 toast；经 ui/sonner.tsx 包装挂载

### 构建/测试依赖

- 无新增（knip / stryker / vitest(vp test) / vp check 沿用现有工具链；AC-10 要求其配置零改动）

---

## 待决问题

- 无阻塞性未决。proposal「待决问题」四项均已在本文定夺：折叠态持久化（D1）、错误双轨 hook 形态（D2）、同名副文本内容（D3）、jsdom + Radix 机制（D4）。
- 非阻塞默认约定（实现阶段按默认执行，无需回评）：toast 时长/位置用 sonner 默认（bottom-right）；lucide-react 与 radix 包安装时取最新稳定版本。
