# workspace-sidebar — desktop-app-shell 变更集

> 前置依赖 `tailwind-migration`（已归档 2026-09-22）。后端零改动：`desktop-workspace-store` 无本 change delta。

## ADDED Requirements

### Requirement: Sidebar 壳层布局

壳态（存在已打开 workspace）SHALL 以 shadcn Sidebar 块承载导航壳：`SidebarProvider` 包裹 `AppSidebar`（workspace 清单侧栏）与 `SidebarInset`（主内容区）；main 区 max-width 1100px 居中布局 SHALL 在 `SidebarInset` 内维持（从全宽 body 移入 inset，形态不变）。

header SHALL 瘦身为终态：折叠钮 + 标题（Desktop Terminal）+ 版本/更新指示（`UpdateIndicator` 含「重试更新」按钮语义不变）；workspace `select`、移除、刷新控件 MUST NOT 留在 header。

侧栏 SHALL 采用 `collapsible="icon"`：折叠态仅图标并经 Tooltip 补足信息；SHALL 接受窗口 < 768px 时 `use-mobile` 触发的 Sheet 抽屉第三态与内建 Ctrl/Cmd+B 折叠快捷键；折叠态持久化方式（localStorage vs 会话内 state）由 design 定夺。sidebar MUST NOT 引入路由：列表 ↔ 详情视图切换维持 `ChangeView` 本地 state。

欢迎态（root 为 null）MUST NOT 挂载 `SidebarProvider` / `AppSidebar`：`WelcomeView` 维持全屏现状；Toaster（sonner）SHALL 在 App 根挂载一次，欢迎态与壳态都覆盖。

#### Scenario: 壳态挂载与 header 终态

- **WHEN** workspace 清单非空、应用处于壳态
- **THEN** `SidebarProvider` / `AppSidebar` / `SidebarInset` 渲染，main 区 max-width 1100px 居中位于 inset 内
- **AND** header 仅含折叠钮、标题、版本/更新指示，无 `combobox`、移除、刷新控件

#### Scenario: 欢迎态隔离

- **WHEN** `root` 为 null（清单为空或加载失败后无根）
- **THEN** 页面无 `SidebarProvider` / `AppSidebar` DOM，`WelcomeView` 全屏呈现
- **AND** Toaster 已挂载（欢迎态下添加失败同样可 toast）

#### Scenario: 折叠交互

- **WHEN** 用户点击折叠钮或按下 Ctrl/Cmd+B
- **THEN** 侧栏进入 icon-only 态，悬停清单项出 Tooltip；窗口宽度 < 768px 时侧栏以 Sheet 抽屉呈现

#### Scenario: 不引入路由

- **WHEN** 检查 desktop 依赖与视图代码
- **THEN** 无 router 依赖，列表 ↔ 详情仍为 `ChangeView` 本地 state（`selectedChange`）

### Requirement: 错误呈现双轨

前端错误呈现 SHALL 按接口类别双轨分流：

- 动作类接口（`add_workspace` / `remove_workspace` / `touch_workspace`）失败 SHALL 经 hook 统一以 toast（sonner）呈现，文案 SHALL 含错误信息，MUST NOT 再置入持久 error 态渲染 error-note
- 查询类接口（启动 `list_workspaces` / `list_changes` / `get_change_detail` / `read_artifact`）失败 SHALL 保留 error 态透传，视图 inline 持久渲染（不自动消失）；`WorkspaceState.error` 语义收窄为「清单加载失败」。实证理由：启动 `list_workspaces` 失败时 toast 一闪即逝，用户面对「最近的 workspace (0)」空态会误读为无 workspace，故查询失败必须常驻可见
- updater 错误呈现不变：保留「重试更新」按钮（恢复动作非纯呈现）
- 无错误时 MUST NOT 渲染 error-note，也 MUST NOT 出现 toast

hook 形态（hook 内直调 toast vs 保留返回值由视图 effect 触发）由 design 定夺，本 requirement 只约束可观察行为。

#### Scenario: 动作失败走 toast

- **WHEN** `touch_workspace` / `remove_workspace` / `add_workspace` 任一 reject
- **THEN** toast 呈现含错误信息的文案，`queryAllByTestId('error-note')` 为 0，视图停留当前状态不跳转

#### Scenario: 查询失败 inline 持久

- **WHEN** `list_workspaces`（启动加载）或 `list_changes` / `get_change_detail` / `read_artifact` reject
- **THEN** 对应视图 error-note 持久渲染且不自动消失，无 toast 顶替

#### Scenario: 无错误零呈现

- **WHEN** 全部命令成功返回
- **THEN** `queryAllByTestId('error-note')` 为 0 且无 toast 节点存在

## MODIFIED Requirements

### Requirement: workspace 选择

App SHALL 提供 workspace 选择与恢复能力：

- 启动时 SHALL 调用 `list_workspaces`，存在记录时自动恢复最近打开的 workspace（`last_opened_at` 降序第一名）并直接进入列表视图；无任何记录时 SHALL 停留在欢迎屏
- 当前根 SHALL 恒等于清单第一名（`list_workspaces` 按 `last_opened_at` 降序）：清单非空即选中第一名，清单为空（或被移除一空）时停留在欢迎屏；欢迎屏 SHALL 呈现空态与“添加新文件夹”入口；文件夹选择器 SHALL 仍是添加入口，选定后经 `add_workspace` 入库，刷新后新记录（`last_opened_at` 最新）即第一名并打开；壳态下 sidebar「工作区」组标签右侧的 `SidebarGroupAction` 内联图标 SHALL 提供同一文件夹选择器添加流
- 壳态下 sidebar SHALL 为唯一 workspace 清单与切换入口（顶部 `select` 下拉 MUST 移除）：清单项 SHALL 按 `last_opened_at` 降序呈现全部清单，点击清单项即切换；清单项 name 仅取目录名最后一段，SHALL 以副文本（`SidebarMenuButton` sub）区分同名项（具体内容 design 定，同名场景测试以 testid 承载、不依赖 accessible name），悬停 SHALL 以 Tooltip 展示完整 root；切换 SHALL 清空当前 change 选中并以新根重新取数
- 选中 workspace（含自动恢复与 sidebar 列表项切换）SHALL 触发 `touch_workspace` 刷新 `last_opened_at`
- 移除 SHALL 经清单项右键上下文菜单（shadcn `ContextMenu`）调用 `remove_workspace`，仅从清单移除、MUST NOT 删除盘上目录，MUST NOT 引入确认弹窗；移除的是当前根时 SHALL 切换到剩余清单第一名，清单为空时 SHALL 回到欢迎屏；移除非当前项时当前根与视图 SHALL 保持不变

选定 workspace 根后，列表与详情取数均以该根为基准。

#### Scenario: 启动自动恢复

- **WHEN** 应用启动且库中已有 workspace 记录
- **THEN** App 恢复 `last_opened_at` 最新的一条为当前根并进入列表视图，无需用户操作

#### Scenario: 无记录停留欢迎屏

- **WHEN** 应用启动且 `list_workspaces` 返回空清单
- **THEN** App 停留在欢迎屏，呈现“添加新文件夹”入口

#### Scenario: 欢迎屏添加

- **WHEN** 用户经文件夹选择器添加新目录
- **THEN** App 经 `add_workspace` 入库，刷新后以新记录（清单第一名）为根调用 `list_changes` 进入列表视图

#### Scenario: Sidebar 组动作添加

- **WHEN** 用户点击「工作区」组标签右侧的 `SidebarGroupAction` 内联图标并经文件夹选择器选定目录
- **THEN** 经 `add_workspace` 入库，刷新后以新记录（清单第一名）为根打开列表视图

#### Scenario: Sidebar 列表项切换

- **WHEN** 用户点击 sidebar 中另一 workspace 清单项
- **THEN** 当前 change 选中被清空，`touch_workspace` 刷新 `last_opened_at` 后列表以新根（刷新后第一名）重取；悬停清单项可见 Tooltip 完整 root，同名项以副文本区分（testid 承载）

#### Scenario: 右键菜单移除当前根

- **WHEN** 用户右键当前打开的 workspace 清单项并点击菜单中的移除
- **THEN** 该项从库中删除，视图切换到剩余清单第一名并以其为根重取列表；无剩余项时回到欢迎屏

#### Scenario: 右键菜单移除非当前项

- **WHEN** 用户右键非当前根的清单项并移除
- **THEN** 该项从清单消失，当前根与当前视图保持不变

### Requirement: React 前端刷新取数模型

前端 SHALL 以 React + TS 实现，取数收在 hooks（`useChangeList` / `useChangeDetail` / `useWorkspaces`）内：由用户显式刷新动作触发 invoke；`useWorkspaces` 为唯一例外——启动时自动触发一次以支撑自动恢复，此后仍由用户动作触发。MUST NOT 实现文件 watch、后台轮询或事件订阅。刷新 SHALL 覆盖两个层级：workspace 级（重取列表）与 change 级（重取当前详情）。未来替换为推送时 SHALL 仅改动 hooks 内部实现，视图层不感知取数方式。

视图 SHALL 至少呈现：change 列表（代际标注、按月分组）、phase 流水线（attempt / verdict / checklist 展开）、经 renderer 注册表渲染的产物区（含 markdown 文档与 tasks 进度）、workspace 清单（sidebar 侧栏）。列表刷新入口 SHALL 位于清单页（`ChangeListView`）头部并保留 `disabled={loading}` 语义，MUST NOT 回迁 header。

#### Scenario: 刷新按钮触发重取

- **WHEN** 用户点击清单页头部的刷新按钮
- **THEN** hooks 重新 invoke 对应查询命令并更新视图，期间无自动轮询发生

#### Scenario: 取数收口 hooks

- **WHEN** 审查视图组件代码
- **THEN** 组件不直接 invoke，取数统一经 `useChangeList` / `useChangeDetail` / `useWorkspaces`

#### Scenario: 无 watch 依赖

- **WHEN** 检查 desktop 依赖与前端代码
- **THEN** 无文件系统监听（notify 等）依赖、无定时器轮询逻辑

### Requirement: workspace 注册命令轨道

desktop-app SHALL 新增 `commands/workspaces` 命令轨道，承载 workspace 注册表（shell 记忆，非 change 域查询）四命令：`list_workspaces`（`last_opened_at` 降序清单）、`add_workspace`（canonicalize + upsert + touch）、`remove_workspace`、`touch_workspace`。命令 SHALL 经 Tauri `State<Store>` 访问 store（`main.rs` 启动时打开并 `.manage()`），自身 MUST NOT 直接操作 redb 或 db 文件。既有 `commands/queries/` 三命令的无状态语义与 `commands/exec/` 空轨道 SHALL 保持不变。

本轨道 SHALL 确立后续可失败命令的错误约定模板：命令返回 `Result<T, String>`，`Err` 由 Tauri 转为前端 reject；MUST NOT 静默吞掉 db 打开或读写失败。db 打开失败 SHALL 使应用启动失败并报错，MUST NOT 静默降级为空清单。reject 抵达前端后的呈现 SHALL 按本能力「错误呈现双轨」requirement 分流：动作类命令（add / remove / touch）失败走 toast，查询类命令失败保留 inline error 态持久呈现。

#### Scenario: 命令经 State 访问 store

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** 四命令均以 `State<Store>` 取得 store，命令体为参数转换 + store 调用 + DTO 返回，签名中无 redb 类型

#### Scenario: 错误以 reject 传达前端

- **WHEN** store 操作失败（如 db 文件损坏、磁盘写失败）
- **THEN** 命令返回 `Err(String)`，前端收到错误字符串并按双轨语义呈现（动作失败 toast / 查询失败 inline 持久），无静默成功

#### Scenario: db 打开失败快速失败

- **WHEN** 启动时 db 文件无法打开
- **THEN** 应用启动失败并给出错误信息，不进入空清单的静默降级

#### Scenario: 既有轨道不受影响

- **WHEN** 检查 `commands/queries` 与 `commands/exec`
- **THEN** 三个查询命令语义不变，exec 轨道仍无实现、无空壳 trait

### Requirement: Tailwind v4 单一样式体系

desktop 前端样式 SHALL 以 Tailwind v4 为唯一样式体系:

- 样式入口 SHALL 为正式 CSS 文件(`@import "tailwindcss"`);`index.html` 内联 `<style>` MUST 移除,MUST NOT 长期并存第二套手写 CSS 体系
- 迁移 SHALL 两步走:步骤① 元素级全局样式(`body` / `button` / `#root`)与设计 token 翻入 `@layer base`,与旧类 CSS 共存且视觉零变;步骤② 单 commit 完成类名全量 utilities 化
- 设计 token SHALL 沿 shadcn 结构命名(`--background` / `--card` / `--border` / `--foreground` / `--muted-foreground` / `--primary` 等),值 SHALL 直写现有 hex(如 `--accent #2563eb` → `--primary`),SHALL NOT 换用 shadcn 默认主题色;`--pass/--fail/--warn` 语义色 SHALL 以自定义 `@theme` token 或语义工具类承载(形态 design 定夺);light/dark 双槽 SHALL 预留但仅填 light 值
- 布局冻结 SHALL 仅约束迁移步骤①②:期间 main 区 max-width 1100px 居中、header 信息架构、欢迎态/壳态 DOM 结构(控件替换除外)MUST NOT 变化;迁移完成后布局演化由壳层 requirement 承载(如「Sidebar 壳层布局」),main 区 max-width 1100px 居中作为长期形态延续
- markdown 产物样式 SHALL 经 `@tailwindcss/typography`(`prose`)承载:react-markdown 输出裸元素无类名钩子,MUST NOT 依赖自定义类后代选择器(如 `.markdown-doc pre`)

#### Scenario: 步骤① 共存且视觉零变

- **WHEN** 步骤① 落地
- **THEN** CSS 入口以 `@layer base` 承载元素级样式与 token,旧类 CSS 照常生效
- **AND** `vp build` 成功,前后截图对比布局、配色、结构一致

#### Scenario: 步骤② 内联样式清零

- **WHEN** 步骤② 合入
- **THEN** `index.html` 无 `<style>` 块,组件类名为 utilities
- **AND** `.panel` / `.badge-*` / `.attempt` 等旧自定义样式类不复存在

#### Scenario: token 保持现有配色身份

- **WHEN** 审查 CSS 入口的 token 定义
- **THEN** token 值与原 `index.html` 的 hex 一致(如 `--primary: #2563eb`)
- **AND** 未引入 shadcn 默认 zinc 主题色,深色模式槽位留空未填

#### Scenario: markdown 经 prose 承载

- **WHEN** 渲染含表格、代码块的 markdown 产物
- **THEN** 表格边框、代码底色等样式经 `prose` 生效
- **AND** 样式表中无 `.markdown-doc` 后代选择器

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/components/AppSidebar.tsx`（新） | workspace 清单侧栏 | `SidebarMenuButton` 列表项（点击 touch 切换 / 副文本 testid 区分同名 / Tooltip 完整 root）；`SidebarGroupAction` 添加流；`ContextMenu` 右键移除 |
| `packages/desktop/src/components/ui/sidebar.tsx` 及配套生成件（新） | shadcn 生成件 | sidebar / separator / sheet / tooltip / context-menu / sonner（按需）；内部化纪律适用：过 fmt/lint/knip 无豁免、无 Next 语境残留 |
| `packages/desktop/src/hooks/use-mobile.ts`（新） | 断点 hook | 窗口 < 768px → Sheet 抽屉第三态 |
| `packages/desktop/src/hooks/useWorkspaces.ts` | 错误双轨收口 | 动作失败 toast（add/remove/touch）；error 态收窄为清单加载失败（查询 inline 持久）；切换语义不变（touch → 重排 → 恒取第一名） |
| `packages/desktop/src/App.tsx` | 壳布局 | `SidebarProvider` + `AppSidebar` + `SidebarInset`；header 终态（折叠钮/标题/版本更新）；Toaster App 根挂载一次；欢迎态不挂壳 |
| `packages/desktop/src/views/changes/ChangeListView.tsx` | 刷新入口 | 头部刷新按钮 `disabled={loading}`；列表加载失败 error-note inline 保留 |
| `packages/desktop/src/views/WelcomeView.tsx` | 欢迎态 | error-note 仅清单加载失败；「添加新文件夹」入口保留 |
| `packages/desktop/package.json` | 依赖 | `@radix-ui/react-separator` / `react-dialog` / `react-tooltip` / `react-context-menu`、`lucide-react`、`sonner` |
| `packages/desktop/src-tauri/**` | 不修改 | 后端零改动；`desktop-workspace-store` spec 语义不变 |
| `src/components/AppSidebar.test.tsx`（新）等测试 | 测试挂钩 | data-testid；右键经 `fireEvent.contextMenu`；toast 断言经 sonner 文本；同名场景 testid 承载 |
