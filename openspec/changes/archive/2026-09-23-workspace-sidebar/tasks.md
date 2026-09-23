# 任务: workspace-sidebar

> 依赖顺序：阶段 1（依赖与生成件）→ 阶段 2（错误双轨）→ 阶段 3（Sidebar 壳重排）→ 阶段 4（管线守线）。
> 后端零改动（AC-9）：全部任务不含 `packages/desktop/src-tauri/**`；测试文件由 test-design / test-gen 阶段承接，此处不列。

## 阶段 1：依赖与 shadcn 生成件地基

- [x] `packages/desktop/package.json` 新增运行时依赖并安装：`pnpm -C packages/desktop add @radix-ui/react-separator @radix-ui/react-dialog @radix-ui/react-tooltip @radix-ui/react-context-menu lucide-react sonner`
- [x] 新增 `packages/desktop/src/hooks/use-mobile.ts`：`useIsMobile(): boolean`（768px 断点，matchMedia + change 监听）
- [x] 新增 `packages/desktop/src/components/ui/tooltip.tsx`：仅 `Tooltip` / `TooltipTrigger` / `TooltipContent` / `TooltipProvider` 四个导出
- [x] 新增 `packages/desktop/src/components/ui/separator.tsx`：`Separator` 薄包装
- [x] 新增 `packages/desktop/src/components/ui/sheet.tsx`：仅保留 `sidebar.tsx` 消费面（`Sheet` / `SheetContent` 含侧向 variant），其余导出删减
- [x] 新增 `packages/desktop/src/components/ui/context-menu.tsx`：仅保留 `ContextMenu` / `ContextMenuTrigger` / `ContextMenuContent` / `ContextMenuItem`，Sub/Checkbox/Radio 等未用导出删减
- [x] 新增 `packages/desktop/src/components/ui/sonner.tsx`：`Toaster` 包装，`toastOptions.classNames` 映射既有 token（bg-card / text-foreground / border-border），去 Next 语境残留
- [x] 新增 `packages/desktop/src/components/ui/sidebar.tsx` 裁剪版：导出集按 design pin（Provider/Sidebar/Trigger/Inset/Group/GroupLabel/GroupAction/GroupContent/Menu/MenuItem/MenuButton；`useSidebar` 不导出）；去 cookie 持久化（折叠态会话内 state，`defaultOpen` 固定 true，design D1）；`collapsible="icon"` + Ctrl/Cmd+B 保留；`TooltipProvider delayDuration={0}`（design D4）；裁掉 MenuSkeleton / MenuSub / Input 等未用子组件（不引入 skeleton.tsx / input.tsx，design D5）；变体集仅保留实际用量
- [x] 中间守线：`pnpm -C packages/desktop run client:check` 通过，生成件过 fmt / lint / knip 且无新增豁免（超 `max-lines-per-function: 50` 的生成函数拆内部组件消化）

## 阶段 2：错误呈现双轨（useWorkspaces）

- [x] `packages/desktop/src/hooks/useWorkspaces.ts`：import `toast` from 'sonner'；`useWorkspacesActions` 的 add/remove/touch catch 分支改为 `toast.error(固定前缀 + String(err))`（前缀：`添加 workspace 失败：` / `移除 workspace 失败：` / `切换 workspace 失败：`，design D8）并移除 `setError` 调用；`useWorkspacesActions` 参数收掉 `setError`
- [x] `packages/desktop/src/hooks/useWorkspaces.ts`：`list_workspaces` 加载失败分支保留 `setError(String(err))`；`remove` resolve(false)（store miss）维持无 error、无 toast；启动恢复 touch 失败走 toast 且 fire-and-forget 不阻断恢复链
- [x] `packages/desktop/src/hooks/useWorkspaces.ts`：`WorkspaceState` shape 不变，JSDoc 更新为双轨语义（`error` 仅承载清单加载失败）与 toast 说明

## 阶段 3：Sidebar 壳与视图重排

- [x] 新增 `packages/desktop/src/components/AppSidebar.tsx`：props（workspaces / currentRoot / onOpen / onAdd / onRemove）；「工作区」组 + `SidebarGroupAction`（Plus 图标，`aria-label="添加 workspace"`，onClick → onAdd）；清单项 = ContextMenu（Trigger asChild）包裹 SidebarMenuItem + SidebarMenuButton（`isActive`、`tooltip={record.root}`、onClick → onOpen）；主文本 `record.name`，副文本父目录（root 末段前缀，无分隔符时空串，design D3）`data-testid="workspace-sub"`；按钮 `data-testid="workspace-item"` + `data-root={record.root}` 作用域属性；ContextMenu「移除」项 onClick → onRemove，无确认弹窗；列表项子组件拆分保线
- [x] `packages/desktop/src/App.tsx`：删除内部组件 `WorkspaceSelect` / `AppHeader`；壳态重排为 `SidebarProvider` → `AppSidebar` + `SidebarInset`，内层 `<main>` 改同 class `<div>`（1100px 居中形态不变，design D7）；新 header 终态 = `SidebarTrigger` + 标题「Desktop Terminal」+ `UpdateIndicator`（保留「重试更新」语义）；`<Toaster />` 置于 App 根与条件渲染同级；root === null 时不渲染 provider/sidebar DOM，`WelcomeView` 全屏；`pickAndAdd` 保留（对话框参数 `{ directory: true, multiple: false }` 不变），`record === null` 注释对齐「toast 已呈现」
- [x] `packages/desktop/src/views/changes/ChangeListView.tsx`：顶部新增头部行（右对齐，始终渲染，先于 error-note 与数据区）：`Button` 文案「刷新列表」，`onClick={state.refresh}`、`disabled={state.loading}`（props 零变化）
- [x] `packages/desktop/src/views/WelcomeView.tsx`：无结构性改动；JSDoc/注释对齐双轨措辞（error-note 仅清单加载失败），「添加新文件夹」入口保留
- [x] `packages/desktop/src/styles/global.css`：按 design 定案不新增 token（sonner 配色走 classNames 映射）；仅当实测对比度不足时追加 `@theme inline` 映射行且以此为限（实测零 diff 属预期）

## 阶段 4：管线守线与范围核对

- [x] 全量 `pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过；`knip.json` / `stryker.config.json` / `vite.config.ts` / `components.json` / `tsconfig` 零改动、无新增豁免条目（AC-10）
- [x] mutate 处置核对（design D6）：`stryker.config.json` 零改动，`src/components/ui/**` 维持排除（理由成对记录于 design）；`AppSidebar.tsx` / `use-mobile.ts` 不在 ui/** glob、自然纳入 mutate
- [x] 范围核对（AC-9）：`git diff packages/desktop/src-tauri` 为空；变更清单内文件与实际改动一致，无越界文件
