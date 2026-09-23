# 探索笔记：shadcn Sidebar 壳与 workspace 管理面板

> 日期：2026-09-22 ｜ 状态：探索完成，待起 proposal
> 拟议 change 名：`workspace-sidebar`
> **前置依赖：`tailwind-migration`**（Tailwind v4 + shadcn 地基、data-testid 挂钩、
> 生成件内部化纪律 #13 均由其先行落地）。
> 本文件由 `shadcn-sidebar-workspace.md` 拆分而来（第三轮审计拍板 #16）。

## 目标

引入 shadcn/ui Sidebar，为 desktop 前端提供独立的 workspace 列表与管理面板；
header 瘦身；错误呈现双轨化。**后端零改动**（仅现有 4 命令，`desktop-workspace-store`
不动）。

> 2026-09-22 修订：初版要求「保留 header 下拉入口」，sidebar 设计成型后改为
> **移除顶部 select**，sidebar 为唯一切换入口（决策 #5）。

## 已锁定决策

| # | 问题 | 决定 |
|---|------|------|
| 1 | 「配置」含义 | **清单管理**：纯前端 + 现有 4 命令（list/add/remove/touch），不动 store |
| 3 | 欢迎态 | **全屏** WelcomeView 维持现状，sidebar 只在有 workspace 打开时挂载（不挂 SidebarProvider；Toaster 例外——App 根挂载一次，欢迎态与壳态都覆盖） |
| 4 | 重命名 | **不做**，`WorkspaceRecord.name` 维持目录名最后一段自动取 |
| 5 | 顶部 select | **移除**，sidebar 为唯一 workspace 切换入口 |
| 6 | 添加工作区 | 「工作区」组标签右侧内联小图标（`SidebarGroupAction`），文件夹选择器添加流，不要底部按钮 |
| 7 | 刷新 change list | 从 header 迁入清单页 panel 头部（`disabled={loading}` 语义随行） |
| 8 | 移除动作 | 列表项**右键上下文菜单**（shadcn `ContextMenu`），非 hover ⋮ |
| 9 | 其他 shadcn 组件 | 按需一并引入 |
| 10+11 | 错误呈现 | **双轨**：动作类接口（add/remove/touch）hook 统一 toast（sonner）；查询类接口（启动 `list_workspaces` / `list_changes` / `get_change_detail` / `read_artifact`）保留 error 态透传，视图持久渲染 inline（#11 收窄 #10 一刀切）。updater 错误保留「重试更新」按钮（恢复动作非纯呈现，不改 toast） |
| 14 | 折叠形态 | `collapsible="icon"` + Tooltip；**接受**窗口 < 768px 自动 Sheet 抽屉第三态（`use-mobile`）与内建 Ctrl/Cmd+B 折叠快捷键；折叠态持久化方式（去 cookie 后 localStorage vs 会话内）design 定 |
| 17 | 同名区分 | `SidebarMenuButton` **副文本**（sub）显示父目录 / root 尾段（具体内容 design 定）；测试同名场景用 testid 承载，不靠 accessible name |

### 补充默认假设（proposal 阶段可否决）

- 置顶/固定不做：排序维持 `last_opened_at` 降序现状
- 移除仅清单移除（不删盘上目录），无需确认弹窗
- main 区保留 max-width 1100px 居中，只是从全宽 body 改为 SidebarInset 内布局

## 现状关键事实（sidebar 相关）

- 视图切换是 ChangeView 本地 state（selectedChange）：**sidebar 不引入路由**，只做壳层
- 后端仅 4 命令 + redb 两表（WORKSPACES / META，`commands/workspaces/mod.rs`）：
  清单管理无后端面，本 change 后端零改动
- workspace 切换 = `touch` → 清单重排 → 恒取第一名（spec 既有语义，仅 UI 面收敛）

## 布局

当前布局：

```
┌──────────────────────────────────────────────────────┐
│ AppHeader: [标题] [workspace <select>] [移除] (error) │
│                    ...spacer... [版本/更新] [刷新]     │
├──────────────────────────────────────────────────────┤
│ app-main (max-width 1100px 居中)                      │
│   ChangeView: 列表 ↔ 详情（selectedChange 本地 state） │
└──────────────────────────────────────────────────────┘
```

目标布局：

```
┌─────────────┬───────────────────────────────────────────────┐
│             │ [▤ 折叠钮] Desktop Terminal    v0.1.0 [更新到…] │
│ 工作区   [＋]├───────────────────────────────────────────────┤
│ ─────────── │ ┌─ 清单页（ChangeListView）─────────────────┐  │
│  ▣ proj-a   │ │ 进行中 (1)                    [↻ 刷新]  │  │
│  ▢ proj-b   │ │ ───────────────────────────────────────  │  │
│   (副文本)   │ │  ▸ add-feature               v2          │  │
│  ▢ proj-c   │ └──────────────────────────────────────────┘  │
└─────────────┴───────────────────────────────────────────────┘
   - 列表项：点击 = touch 切换；右键 = 上下文菜单（移除）；悬停 tooltip = 完整 root
   - [＋] = SidebarGroupAction 内联小图标（文件夹选择器添加流）
   - header 终态：折叠钮 + 标题 + 版本/更新（select / 移除 / 刷新全部迁出）
   - 可折叠（icon-only + Tooltip）；欢迎态不渲染此壳
```

数据流（单入口，零后端改动）：

```
sidebar 列表项点击 ──▶ useWorkspaces.touch(root) ──▶ 清单重排 ──▶ 第一名即新根
                       （spec 既有语义，仅 UI 面收敛）
```

## 依赖面（在 tailwind-migration 地基之上）

```
Sidebar (block)
 ├── Button ────── @radix-ui/react-slot
 ├── Separator ─── @radix-ui/react-separator
 ├── Sheet ─────── @radix-ui/react-dialog     ← 窗口 < 768px 的抽屉（#14 接受）
 ├── Tooltip ───── @radix-ui/react-tooltip    ← icon-only 模式悬停提示
 ├── Skeleton / Input
 ├── use-mobile (hook)
 ├── cn() / cva（地基已有）
 └── lucide-react ── ＋、↻、折叠等图标（sidebar 示例图标全靠它）

context-menu（@radix-ui/react-context-menu）── 列表项右键移除
sonner + shadcn Toaster ──────────────────── 动作失败 toast（#10+11）
```

生成件纪律沿用 `tailwind-migration.md` #13：`src/components/ui/**` 视为内部组件，
正常过检查管线，可自由改动（含去 cookie 持久化残留）。

## 错误双轨落地细节（#10+11）

- **useWorkspaces.error 态语义变化**：现状单一 error 态同时承载加载与动作失败
  （AppHeader 与 WelcomeView 渲染同一 error，`App.tsx:119` / `WelcomeView.tsx:14`）；
  双轨后 error 态只承载加载失败（保留 inline 持久呈现），动作失败转 toast。
- **hook 形态**（design 待定）：hook 内直接调用 toast 并从 actions 移除 error 置位，
  还是保留 `null/false` 返回值由视图 effect 触发 toast——影响 `WorkspaceState`
  契约 shape。
- **toast 边界**：动作失败 = add/remove/touch；查询失败 inline 常驻的实证理由——
  启动 `list_workspaces` 失败时若 toast 一闪而过，用户面对「最近的 workspace (0)」
  空态会误以为无 workspace（WelcomeView 现渲染持久 error-note 即为防此）。
- 覆盖面：`list_changes` / `get_change_detail` / `read_artifact` 失败同样保留
  视图 inline。
- **与 tailwind-migration 的先后**：地基 change 先行时 error-note 尚未双轨化，
  将全量迁 utilities / testid；本 change 再将动作失败类改 toast（先迁后删是
  拆分两个 change 的已知代价，加载失败类 inline 保留不动）。

## Spec 面预览（desktop-app-shell）

### 场景改写

| 现有场景 | 改写为 |
|----------|--------|
| Header 提供 workspace 下拉切换 | sidebar 列表项点击切换 |
| 悬停以 title 展示完整 path | 悬停 tooltip 展示完整 root（`SidebarMenuButton` 内建 tooltip prop） |
| 移除经 Header 入口 | 经列表项右键上下文菜单 |
| 刷新经 Header 入口 | 经清单页头部入口 |
| hook 以 error 态接住呈现（命令轨道 requirement） | 双轨语义：动作失败 toast / 加载失败 inline 持久 |

底层语义（touch 重排、移除后切剩余第一名、切换清空 change 选中）不动；
workspace-store 的「MUST NOT 静默吞掉失败」承诺不变。

另需 delta：布局形态（sidebar 壳 / header 定位 / SidebarProvider 与欢迎态关系）。

## 测试改写

- `getByRole('combobox')` / `option` 查询 → sidebar 项 `getByRole('button', { name: 'proj-a' })`
  （同名场景以 testid 承载，#17）
- IPC 断言（`touch_workspace` → 新根 `list_changes`）逐字保留
- 右键菜单：`fireEvent.contextMenu` + radix jsdom 模拟
- **error-note 断言拆两类**：动作失败类改 toast 文本断言（sonner 渲染可直接
  getByText），「无错误时不渲染」断言改为 toast 不存在；加载失败类保留视图内
  断言（类查询已在 tailwind-migration 换 testid，本 change 不再动查询方式）
- 波及：`App.test.tsx` 下拉相关 3 用例、`workspace_restore.test.tsx` 同类、
  `.error-note` 动作失败类断言（App.test / workspace_restore / ChangeListView /
  ChangeDetailView 中的一部分）

## 待 design 阶段解决

- 折叠态持久化：去 cookie 后换 localStorage vs 会话内 state（#14 遗留）
- 错误双轨的 hook 形态（hook 直调 toast vs 返回值判定，影响契约 shape）
- 副文本具体内容：父目录 vs root 尾段（#17 遗留）
- jsdom + Radix（Tooltip/Sheet/ContextMenu）的 waitFor / pointer 模拟细节
