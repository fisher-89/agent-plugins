# 测试设计: workspace-sidebar

> **日期**: 2026-09-22

---

## 验收范围

<!-- 测试框架识别结论(见 test_detect_frameworks):`packages/desktop` 套件为 vite-plus(`vite-plus/test` 运行器 + @testing-library/react + jsdom),与既有 17 个测试文件同一运行器;`src-tauri` rust 套件与本 change 无涉(AC-9 后端零改动)。`package.json` / `styles/global.css` 报 unknown(非可执行源,落不可测试项)。
路径解析结论(见 test_resolve_paths):6 对 source -> test_file(`AppSidebar.test.tsx` 与 `use-mobile.test.ts` 为 resolve 派生的新增落地文件);errors 6 项——`src/components/ui/**` 六件(sidebar/separator/sheet/tooltip/context-menu/sonner)「Not in test config scope」,与 `openspec/config.json` desktop 套件 excludes 预置口径一致(design D6 维持),全部落入不可测试项。
本 change 测试工作主体是**既有测试文件的交互入口改写**:①查询宿主迁移(下拉 combobox/option、header「移除」「刷新列表」按钮 → sidebar 列表项 / 右键菜单 / GroupAction / 清单页头部);②error-note 动作类断言迁移(→ sonner toast 固定文案断言,D8);③IPC 时序/次数/参数断言逐字保留(AC-3「与现状逐字一致」硬约束)。改写用例统一标「新增」并在测试条件内注明改写来源;`src/__tests__/shadcn_controls.test.tsx` 的 header「移除」按钮断言随 header 瘦身一并改写(proposal 测试文件清单未列,属改写波及面,归集成关系「sidebar 列表项交互 → workspace 动作链」承载)。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 壳态渲染 `SidebarProvider` / `AppSidebar` / `SidebarInset`；页面无 `combobox`（`getByRole('combobox')` 抛错）；header 仅折叠钮 + 标题 + 版本/更新指示 | 集成测试 | `packages/desktop/src/App.test.tsx`，见集成测试「壳层布局与折叠形态：App 壳 ↔ ui/sidebar ↔ use-mobile」 |
| AC-2 | root 为 null 时不渲染 `SidebarProvider` / `AppSidebar` DOM，`WelcomeView` 全屏；Toaster 在欢迎态与壳态均挂载 | 集成测试 | `packages/desktop/src/App.test.tsx`，见集成测试「壳层布局与折叠形态：App 壳 ↔ ui/sidebar ↔ use-mobile」 |
| AC-3 | 点击清单项 → `touch_workspace` → 清单重排取第一名为新根，change 选中清空、`list_changes` 以新根重取（IPC 断言与现状逐字一致）；悬停 Tooltip 展示完整 root；同名项副文本经 testid 断言 | 集成测试 | `packages/desktop/src/App.test.tsx`（切换链）＋ `packages/desktop/src/components/AppSidebar.test.tsx`（Tooltip / 副文本单测半边），见集成测试「sidebar 列表项交互 → workspace 动作链」 |
| AC-4 | 「工作区」组标签右侧 `SidebarGroupAction` 内联图标 → 文件夹选择器 → `add_workspace` 入库 → 新记录（第一名）打开；欢迎屏「添加新文件夹」入口保留 | 集成测试 | `packages/desktop/src/App.test.tsx`，见集成测试「sidebar 列表项交互 → workspace 动作链」添加场景 |
| AC-5 | `fireEvent.contextMenu` 打开菜单完成移除；移除当前根 → 切剩余第一名、空则回欢迎屏；移除非当前项 → 当前根不变；无确认弹窗、不删盘上目录 | 集成测试 | `packages/desktop/src/__tests__/workspace_restore.test.tsx` ＋ `packages/desktop/src/App.test.tsx`（另涉 `src/__tests__/shadcn_controls.test.tsx` 移除断言改写），见集成测试「启动恢复链与移除时序改写」与「sidebar 列表项交互 → workspace 动作链」 |
| AC-6 | 清单页头部刷新按钮带 `disabled={loading}`；header 无刷新按钮 | 单元测试 | `packages/desktop/src/views/changes/ChangeListView.test.tsx`（「header 无刷新按钮」半边由集成测试「壳层布局与折叠形态」header 终态用例承载） |
| AC-7 | `collapsible="icon"` + Tooltip 生效；窗口 < 768px 呈现 Sheet 抽屉；Ctrl/Cmd+B 切换折叠 | 集成测试 | `packages/desktop/src/App.test.tsx` ＋ `packages/desktop/src/hooks/use-mobile.test.ts`（断点边界单测半边），见集成测试「壳层布局与折叠形态：App 壳 ↔ ui/sidebar ↔ use-mobile」 |
| AC-8 | add/remove/touch reject → toast 呈现错误文本且 `queryAllByTestId('error-note')` 为 0；`list_workspaces` / `list_changes` / `get_change_detail` / `read_artifact` reject → error-note 持久渲染不消失；全部成功 → 无 toast 无 error-note；updater「重试更新」按钮行为不变 | 集成测试 | `packages/desktop/src/App.test.tsx`（toast 轨）＋ `packages/desktop/src/__tests__/workspace_restore.test.tsx`（查询 inline 保留轨）＋ `packages/desktop/src/hooks/useWorkspaces.test.ts`（hook toast 调用语义单测半边），见集成测试「错误双轨呈现：动作 reject → toast / 查询 reject → inline」 |
| AC-9 | `git diff packages/desktop/src-tauri` 为空；`desktop-workspace-store` spec 无本 change delta | 不可测试 | —（见不可测试项：AC-9 后端零改动） |
| AC-10 | `pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过且 ui/** 无新增豁免条目；`vp test` 全绿；新增 ui 生成件的 mutate 纳入/排除处置与理由成对（沿用 tailwind-migration 口径） | 不可测试 | —（见不可测试项：AC-10 管线守线） |

<!-- 双路由说明:AC-3 / AC-7 / AC-8 主类型按占多数的集成链路标注,其单测半边(Tooltip/副文本、断点边界、hook toast 调用参数)分别路由到 `AppSidebar.test.tsx` / `use-mobile.test.ts` / `useWorkspaces.test.ts` 的单元章节;同一断言在集成关系与单测章节间不重复登记。 -->

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景(vite-plus/test + @testing-library/react 组件/hook 渲染测试)。
迭代类型口径:既有用例的查询入口改写(下拉/按钮 → sidebar 交互、error-note 动作类 → toast)统一标「新增」并在测试条件内注明改写来源与被取代断言;「废弃」仅用于断言语义随控件移除而消亡、无后继用例的行(本设计仅 1 行,见集成测试「壳层布局与折叠形态」);零改写保留的用例不入迭代清单,以 HTML 注释登记。 -->

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

<!-- 源文件和测试文件路径均相对于项目根目录。 -->

#### 待测功能

- App(): 应用壳——壳/欢迎态分流、`SidebarProvider` + `AppSidebar` + `SidebarInset` 壳层、header 终态（`SidebarTrigger` + 标题 + `UpdateIndicator`）、`Toaster` App 根挂载、`pickAndAdd` 添加流（签名不变；内部 `WorkspaceSelect` / `AppHeader` 随壳重排移除）

#### 用例

<!-- App.test.tsx 经本 change 壳重排后是全壳集成套件:jsdom 全挂载 App,进程边界仅 mock IPC / 对话框 / 版本号三处,用例主体是跨模块链路(AppSidebar → useWorkspaces → useChangeList、错误双轨呈现、壳层拓扑与折叠)。
为避免同一用例双表登记,本节不设用例表——该文件的全部改写与新增用例在集成测试四个关系章节承载:「sidebar 列表项交互 → workspace 动作链(切换/添加/移除)」「启动恢复链与移除时序改写」(workspace_restore 套件)「错误双轨呈现:动作 reject → toast / 查询 reject → inline」「壳层布局与折叠形态:App 壳 ↔ ui/sidebar ↔ use-mobile」。
零改写保留的既有用例(启动恢复自动恢复第一名、欢迎屏空态与加载中、恢复进列表后详情进入、添加对话框取消、添加流对话框参数契约)不入迭代清单。 -->

#### Mock策略

<!-- 该文件的进程边界 mock 共四处,与集成测试四个关系章节的 Mock策略表同源,逐项展开口径以各关系章节为准,此处为文件级挂载汇总: -->

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发(`list_workspaces` / `list_changes` / `touch_workspace` / `add_workspace` / `remove_workspace` / `get_change_detail` / `read_artifact`);touch 以单调时钟刷新 `last_opened_at` 并降序重排、按模块级开关切 reject | 全部场景(挂载依赖 + IPC 时序/次数/参数断言,见关系一/二/三/四) |
| `@tauri-apps/plugin-dialog` open | `vi.mock` 按用例 resolve 路径串 / null / reject | 添加流场景(GroupAction 与欢迎屏两入口,见关系一) |
| `@tauri-apps/api/app` getVersion | `vi.mock` 固定 resolve `'0.1.0'`(useUpdater 挂载取版本,与 invoke 调用序列隔离) | 全部场景(UpdateIndicator 挂载依赖) |
| `window.matchMedia` | stub 伪实现(记录 query 串 `(max-width: 767px)`、按用例切换 matches;jsdom 无实现,App 壳态经 SidebarProvider 消费 `useIsMobile`) | 壳层拓扑与折叠形态场景(Sheet 第三态用例 matches=true,见关系四);其余场景默认 false |

<!-- sonner `<Toaster />` 不 mock:App 根真实挂载,toast 断言走 DOM 文案 + `waitFor`(见「错误双轨呈现」关系 Mock策略表)。 -->

---

### packages/desktop/src/components/AppSidebar.tsx -> packages/desktop/src/components/AppSidebar.test.tsx

<!-- 测试文件为新增(proposal 明确的新文件;组件须包裹在 SidebarProvider 内渲染——shadcn sidebar 原语消费 useSidebar context)。 -->

#### 待测功能

- AppSidebar(props: AppSidebarProps): workspace 清单侧栏——「工作区」组渲染、列表项点击切换(`onOpen`)、`SidebarGroupAction` 添加(`onAdd`)、右键 `ContextMenu` 移除(`onRemove`)、Tooltip 完整 root、副文本父目录区分同名

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AppSidebar：清单渲染 | 正向 | workspaces 全量渲染：每项带 `data-testid="workspace-item"` + `data-root`、主文本为 `record.name`、组标签「工作区」在场 | 新增 |
| AppSidebar：清单渲染 | 正向 | `currentRoot` 匹配项呈激活态（isActive 标记），非匹配项不激活 | 新增 |
| AppSidebar：清单渲染 | 边界 | 空清单（`[]`）：组与组标签仍渲染、无列表项、不崩 | 新增 |
| AppSidebar：清单渲染 | 边界 | 超大清单（50 项）：全量渲染无丢失（workspace-item 计数断言） | 新增 |
| AppSidebar：清单渲染 | 异常 | `currentRoot` 引用清单中不存在的 root（移除后刷新间隙的瞬时态）：无激活项、不崩、清单照常渲染 | 新增 |
| AppSidebar：切换回调 | 正向 | 点击列表项 → `onOpen` 以该项 root 调用恰一次 | 新增 |
| AppSidebar：添加入口 | 正向 | `SidebarGroupAction`（`aria-label="添加 workspace"`）点击 → `onAdd` 调用 | 新增 |
| AppSidebar：右键移除 | 正向 | `fireEvent.contextMenu` 列表项 → 菜单出现 → 点击「移除」→ `onRemove` 以该项 root 调用（D4-①：jsdom 下 contextmenu 事件即开） | 新增 |
| AppSidebar：右键移除 | 边界 | 菜单按项作用域隔离：右键非当前项 B 移除的是 B 的 root 而非当前项（`data-root` 定位不串项） | 新增 |
| AppSidebar：Tooltip 与副文本 | 正向 | hover 清单项（`delayDuration=0` 即显，D4-②）→ tooltip 内容为完整 root | 新增 |
| AppSidebar：Tooltip 与副文本 | 正向 | 同名不同父两项（如 `C:\a\plugin` 与 `C:\b\plugin`）：`data-root` 定位后 `within()` 取 `workspace-sub`，副文本各为父目录 `C:\a` / `C:\b`（D3 区分能力） | 新增 |
| AppSidebar：Tooltip 与副文本 | 边界 | root 无分隔符（如 `plugin`）：副文本为空串、`workspace-sub` 节点不渲染（D3） | 新增 |
| AppSidebar：Tooltip 与副文本 | 边界 | 超长 root（>1000 字符）：渲染不崩、完整 root 在 DOM（truncate 承载，供 Tooltip 断言） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `window.matchMedia` | stub 伪实现（记录 query 串、可切 matches；jsdom 无实现，`SidebarProvider` 内部消费 `useIsMobile`） | 全部用例 |
| ResizeObserver | 按需 stub（radix Tooltip / ContextMenu 定位在 jsdom 缺失时兜底；断言按 D4-④ 收敛最终态，不复刻 portal 细节） | Tooltip / 右键菜单用例 |
| 回调 props | `onOpen` / `onAdd` / `onRemove` 以 `vi.fn()` 注入（组件纯回调驱动，无进程边界） | 全部用例 |

---

### packages/desktop/src/hooks/use-mobile.ts -> packages/desktop/src/hooks/use-mobile.test.ts

<!-- 测试文件为新增(colocated 惯例,resolve 派生路径落地):design D6 将 use-mobile 归入 mutate 纳入口径,断点语义须有用例钉住;亦是 AC-7 Sheet 第三态机制的单元半边。 -->

#### 待测功能

- useIsMobile(): `< 768px` 断点 hook——`matchMedia('(max-width: 767px)')` 查询 + change 监听，返回 `isMobile`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useIsMobile | 正向 | matchMedia matches=true（窗口 < 768px）→ 返回 true | 新增 |
| useIsMobile | 正向 | matches=false（窗口 ≥ 768px）→ 返回 false | 新增 |
| useIsMobile | 边界 | 查询串恰为 `(max-width: 767px)`：断点边界 767px 命中、768px 不命中由查询串表达（监听与查询同串） | 新增 |
| useIsMobile | 正向 | change 事件推送新 matches：state 跟随更新（监听回调生效，SidebarProvider 据此切换 Sheet） | 新增 |
| useIsMobile | 边界 | 卸载后 change 事件：监听已移除（removeEventListener 被调）、不更新不泄漏 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `window.matchMedia` | stub 伪实现（捕获 addEventListener 注册的监听、可用例内手动派发 change） | 全部用例（jsdom 无 matchMedia 实现） |

---

### packages/desktop/src/hooks/useWorkspaces.ts -> packages/desktop/src/hooks/useWorkspaces.test.ts

#### 待测功能

- useWorkspaces(): 清单取数收口 + 错误双轨落点——动作失败（add/remove/touch）直调 `toast.error` 固定文案、`error` 仅承载 `list_workspaces` 加载失败（签名不变，D2/D8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useWorkspaces：错误双轨（动作 toast 化） | 异常 | add reject：返回 null、`toast.error` 以「添加 workspace 失败：」前缀 + 错误串调用、`error` 保持 null（改写：原「error 置位」断言废除——双轨后动作失败不再置 error） | 新增 |
| useWorkspaces：错误双轨（动作 toast 化） | 异常 | add 传入空字符串 root：参数原样 invoke、toast 以固定前缀呈现、不崩溃（改写：error 断言 → toast 断言） | 新增 |
| useWorkspaces：错误双轨（动作 toast 化） | 异常 | remove reject：返回 false、`toast.error` 以「移除 workspace 失败：」前缀调用、`error` 保持 null（改写同上） | 新增 |
| useWorkspaces：错误双轨（动作 toast 化） | 异常 | touch reject：返回 false、`toast.error` 以「切换 workspace 失败：」前缀调用、`error` 保持 null、不阻塞后续 add（改写同上） | 新增 |
| useWorkspaces：错误双轨（动作 toast 化） | 边界 | remove resolve(false)（store miss 幂等）：返回 false、不调 toast、不置 error（改写：既有「不置 error」用例补「不 toast」断言，D8 排除项） | 新增 |
| useWorkspaces：错误双轨（动作 toast 化） | 边界 | 启动恢复 touch 失败（fire-and-forget）：toast 以「切换 workspace 失败：」前缀调用、`error` 保持 null、恢复链不阻断（内部刷新照常） | 新增 |
| useWorkspaces：错误双轨（动作 toast 化） | 正向 | add / remove / touch 全部成功：toast 零调用（动作成功面无惊扰） | 新增 |

<!-- 零改写保留(不入迭代清单):挂载自动取数与恢复第一名时序、恢复守卫恰一次(list_workspaces 恰 2 次 / touch 恰 1 次)、add/remove/touch 成功路径与内部刷新、remove resolve(false) 的内部刷新次数、无轮询无 watch、loading 状态转换、卸载取消、list_workspaces reject 置 error(查询轨 inline 语义,D2 保留分支)——既有断言全部原样保留。 -->

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发（沿用既有 mockDispatch 替身与可切换 reject 实现） | 全部用例 |
| sonner `toast` | `vi.mock('sonner', …)` 将 `toast.error` 替换为 `vi.fn`（hook 层不渲染 toast DOM，仅断言调用参数：D8 固定前缀 + `String(err)`） | 动作失败 / store miss / 全成功用例 |

---

### packages/desktop/src/views/changes/ChangeListView.tsx -> packages/desktop/src/views/changes/ChangeListView.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的公共 API 变更(视图组件,签名不变)。修改内容为 design 变更清单所列:顶部新增头部行(flex items-center justify-end,先于 error-note 与数据区,始终渲染)的「刷新列表」Button,onClick={state.refresh}、disabled={state.loading}(AC-6 落点)。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeListView：头部刷新行 | 正向 | 头部行先于 error-note 与数据区渲染，且始终在场：loading / error / 空数据 / 有数据四形态下「刷新列表」按钮均存在（刷新入口迁入后空态可操作的前提） | 新增 |
| ChangeListView：头部刷新行 | 正向 | 点击「刷新列表」→ `state.refresh` 调用恰一次 | 新增 |
| ChangeListView：头部刷新行 | 边界 | `loading=true`：按钮 disabled、点击不触发 refresh（`disabled={state.loading}`） | 新增 |
| ChangeListView：头部刷新行 | 边界 | 空数据态（「暂无数据，点击刷新获取。」文案在场）：按钮仍可操作、点击触发 refresh | 新增 |

<!-- 零改写保留(不入迭代清单):分组渲染、代际徽标、change-row 点击、created 日期、unparsable 标注、列表加载失败 error-note inline 断言(AC-8 查询轨,proposal 明确保留不动)、无错误不渲染错误条、暂无数据三分支形态——既有用例全部原样保留。 -->

#### Mock策略

<!-- 无 Mock:测试以 fixture 直接注入 ChangeListState(取数已在 hooks 层被测),不跨进程边界。 -->

---

### packages/desktop/src/views/WelcomeView.tsx -> packages/desktop/src/views/WelcomeView.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。design 定案无结构性改动:error-note 区块因 hook 语义收窄自动仅呈现清单加载失败,仅 JSDoc 与「入库失败」注释对齐双轨措辞。 -->

#### 用例

<!-- **不新建 `src/views/WelcomeView.test.tsx`**(resolve 派生路径仅登记,不落地),故本节无用例行:欢迎屏行为(空态文案、「添加新文件夹」入口、error-note 呈现、加载中)全部经由 `src/App.tsx` 渲染路径,已被 `App.test.tsx` 与 `__tests__/workspace_restore.test.tsx` 覆盖(error-note 仅清单加载失败呈现的收窄语义由 useWorkspaces.test.ts 的 hook 层断言 + workspace_restore 保留用例承载);本 change 对该文件无行为面增量,新建专属文件仅产重复覆盖。 -->

#### Mock策略

<!-- 不适用:测试文件不落地(理由见「用例」节注释);欢迎屏经 App.test.tsx 渲染路径验证,其进程边界 mock(IPC invoke 按命令分发 / dialog open 按用例切换 / getVersion 固定 resolve / matchMedia stub)由集成测试四个关系章节的 Mock策略表承载,不在本节重复登记。 -->

---

## 集成测试

<!-- 跨模块交互识别结论:本 change 的跨模块交互集中在壳重排后的 App 壳层——sidebar 清单动作链(切换/添加/移除)、启动恢复与移除时序、错误双轨呈现、壳层拓扑与折叠形态,各成独立关系。
本 change 不新增集成测试文件:跨模块链路用例按 proposal 分配承载于既有 `src/App.test.tsx`(壳重排后的全壳集成套件,与源文件同址)与既有 `src/__tests__/workspace_restore.test.tsx`;`src/__tests__/shadcn_controls.test.tsx` 仅作改写波及(见关系一说明),不设独立关系。
查询类失败呈现(list_changes / get_change_detail / read_artifact reject → inline)由 `ChangeListView.test.tsx` / `ChangeDetailView.test.tsx` / `__tests__/ipc_pipeline.test.tsx` 既有用例承载,proposal 明确保留不动,不设关系。 -->

### sidebar 列表项交互 → workspace 动作链（切换/添加/移除） → `packages/desktop/src/App.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/components/AppSidebar.tsx` | 触发方（列表项点击 / GroupAction / 右键菜单三类动作入口） |
| `packages/desktop/src/components/ui/sidebar.tsx`、`ui/context-menu.tsx`、`ui/tooltip.tsx` | 容器与交互原语（provider、菜单、tooltip） |
| `packages/desktop/src/App.tsx` | 接线方（`onOpen`→`touch`、`onAdd`→`pickAndAdd`、`onRemove`→`remove`） |
| `packages/desktop/src/hooks/useWorkspaces.ts` | 动作执行方（IPC 下发 + 清单重排 + toast 双轨落点） |
| `packages/desktop/src/views/changes/ChangeView.tsx`（经 `useChangeList`） | 下游取数方（新根 `list_changes` 重取、change 选中清空） |
| `packages/desktop/src/App.test.tsx` | 验证方（改写承载）；`src/__tests__/shadcn_controls.test.tsx` 为验证方之二（header「移除」按钮断言随瘦身改写为右键菜单语义，「刷新列表」半边迁移至清单页头部后按钮名不变） |

**关联AC**: AC-3, AC-4, AC-5

**关系描述**: 顶部下拉被 sidebar 取代后，App.test.tsx 的三条下拉时代用例（下拉切换、移除当前项、下拉可操作）失去查询宿主，必须整体改写为 sidebar 列表项交互；而 IPC 断言（touch → 新根 `list_changes`、change 选中清空且不以新根重发 `get_change_detail`、移除切剩余第一名）逐字保留是本关系的硬约束——出错模式正是改写查询挂钩时顺手「简化」IPC 时序断言，使交互改写演变为语义漂移。radix 交互在 jsdom 的可测性由 D4 机制定夺（`contextmenu` 事件即开菜单、`delayDuration=0` hover 即显 tooltip、断言收敛 `data-root` / testid 最终态），交互入口以 `fireEvent.contextMenu` 与 `data-root` 作用域定位表达，不在测试里复刻 portal 细节。

#### 场景: 列表项点击切换（下拉切换用例改写）

验证 sidebar 成为唯一切换入口后切换语义与下拉时代逐字一致。前置：mock IPC 双记录启动恢复进入列表视图（FIRST 当前、SECOND 备选）；输入：以 `data-root` 定位 SECOND 列表项并 click；预期：`touch_workspace {root: SECOND.root}` → 清单重排 → 最后一次 `list_changes` 以新根发起、change 选中清空。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 点击非当前清单项：`touch_workspace`(新根) → 清单重排 → `list_changes` 以新根发起 → change 选中清空（详情退出且 `get_change_detail` 不以新根重发）——IPC 断言与现状逐字一致（改写：`fireEvent.change(combobox)` → 列表项 click） | 新增 |
| 正向 | 切换后激活态迁移：原当前项退出 isActive、新当前项进入（改写：option 的 value/title 断言 → workspace-item 的 `data-root` + 激活标记断言） | 新增 |
| 边界 | 对唯一清单项（当前项自身）点击：touch 照常发起、视图状态无抖动（原「下拉未禁用」用例的动作面保留） | 新增 |

#### 场景: GroupAction 添加流（欢迎屏入口保留）

验证添加入口从欢迎屏按钮扩展到「工作区」组标签右侧内联图标后链路一致。前置：壳态（或空清单欢迎态）；输入：点击 `aria-label="添加 workspace"` 图标；预期：文件夹对话框 → `add_workspace` → 新记录（第一名）打开。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 点击 GroupAction 内联图标：`open({directory:true, multiple:false})` → `add_workspace` → 新记录（第一名）打开 → `list_changes` 以返回记录的 canonical root 发起——对话框参数契约断言逐字保留（改写：欢迎屏入口 → GroupAction 图标） | 新增 |
| 正向 | 欢迎屏「添加新文件夹」入口保留：dialog → add_workspace → 打开链路与既有用例一致（零改写保留，登记确认） | 新增 |
| 边界 | 对话框取消（null）：不调用 `add_workspace`、停留当前态——两处入口（欢迎屏 / GroupAction）行为一致 | 新增 |

#### 场景: 右键移除流（App 侧改写）

验证移除入口从 header 按钮迁移到清单项右键菜单后移除语义完整。前置：壳态多记录启动恢复；输入：`fireEvent.contextMenu` 目标项 → 菜单「移除」click；预期：`remove_workspace` 下发、清单收缩、根按剩余第一名/空回落。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 右键当前项 → 点击「移除」：`remove_workspace {root}` → 切剩余第一名 → `list_changes` 以新根发起；无确认弹窗（菜单点击后直接下发，无 dialog 节点插入）（改写：header「移除」按钮 → 右键菜单项） | 新增 |
| 正向 | 右键移除非当前项：当前根不变（`list_changes` 不以他根重发）、该项从清单消失 | 新增 |
| 边界 | 移除至空清单：回欢迎屏（WelcomeView 全屏）且 Toaster 仍挂载 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发（沿用 mockIpc：touch 以单调时钟刷新 last_opened_at 并降序重排，对齐后端 store 语义） | 全部场景 |
| `@tauri-apps/plugin-dialog` open | `vi.mock` 按用例 resolve 路径串 / null / reject | 添加流场景 |
| `@tauri-apps/api/app` getVersion | `vi.mock` 固定 resolve `'0.1.0'`（useUpdater 挂载取版本，与 invoke 调用序列隔离） | 全部场景 |
| `window.matchMedia` | stub 伪实现（壳重排后 App 挂载即经 SidebarProvider 消费 `useIsMobile`；jsdom 无实现） | 全部场景 |

---

### 启动恢复链与移除时序改写 → `packages/desktop/src/__tests__/workspace_restore.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/hooks/useWorkspaces.ts` | 数据链路（恢复 touch 与取数时序、双轨落点） |
| `packages/desktop/src/App.tsx` | 壳接线（动作入口随壳重排迁移到 sidebar） |
| `packages/desktop/src/components/AppSidebar.tsx` | 触发方（右键移除 / 列表项切换的 UI 入口） |
| `packages/desktop/src/__tests__/workspace_restore.test.tsx` | 验证方（invoke 时序 / 次数 / 参数契约断言） |

**关联AC**: AC-3, AC-5, AC-8

**关系描述**: 该套件断言焦点是 invoke 时序契约。本 change 触达它的改写有三类：①两个移除用例改 `fireEvent.contextMenu` 入口（「移除→以剩余第一名重取」「无二次恢复 touch」的时序/次数守卫逐字保留）；②下拉切换用例改列表项点击（IPC 逐字保留）；③`list_workspaces` reject 的 error-note inline 断言保留不动（AC-8 查询轨语义）。出错模式：改写 UI 入口时弱化「恢复恰一次」「移除后恰一次刷新」的次数守卫，使挂钩改写演变为覆盖缩水。

#### 场景: 移除与切换入口改写（invoke 时序守卫逐字保留）

验证动作入口迁移到 sidebar 后时序契约原样保留。前置：mock IPC 多记录启动恢复；输入：`fireEvent.contextMenu` + 菜单项 click / 列表项 click；预期：invoke 时序、次数、参数与改写前逐字一致。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 右键移除当前项：`remove_workspace` → 以剩余第一名为新根重取列表（改写：`getByText('移除')` header 按钮 → `fireEvent.contextMenu` + 菜单项「移除」；时序断言逐字保留） | 新增 |
| 正向 | 移除后内部刷新取得剩余清单：恰 3 次 `list_workspaces`、恰 2 次 `list_changes`、无第二次恢复 touch——次数守卫逐字保留（入口改写） | 新增 |
| 正向 | 切换另一项用例改列表项点击：touch(新根) + 选中清空 + `list_changes` 以新根重取 + `get_change_detail` 不以新根重发（改写：`fireEvent.change(combobox)` → 列表项 click） | 新增 |
| 边界 | 剩余清单枚举断言迁移：`getAllByRole('option')` 的 value 序列 → `getAllByTestId('workspace-item')` 的 `data-root` 序列（清零 combobox/option 查询） | 新增 |

<!-- 零改写保留(不入迭代清单):恢复链三用例(有记录启动的「取数→touch→list_changes」时序、touch reject 不阻断恢复链、空清单停欢迎屏)与 `list_workspaces` reject 的 error-note inline 用例(proposal 明确保留不动,AC-8 查询轨的集成层承载)——断言原样保留。 -->

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令分发，list / touch / add 以模块级开关切失败（沿用既有 mockIpc 方案） | 全部用例 |
| `@tauri-apps/plugin-dialog` open | `vi.mock` resolve 路径串 | 手动添加路径用例 |
| `window.matchMedia` | stub 伪实现（App 壳重排后挂载即消费 `useIsMobile`） | 全部用例 |

---

### 错误双轨呈现：动作 reject → toast / 查询 reject → inline → `packages/desktop/src/App.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/hooks/useWorkspaces.ts` | 双轨落点（动作 catch 直调 `toast.error`；`error` 仅承载 `list_workspaces` 加载失败） |
| `packages/desktop/src/components/ui/sonner.tsx` | toast 呈现方（`<Toaster />` App 根挂载，classNames 映射既有 token） |
| `packages/desktop/src/App.tsx` | 壳层（Toaster 与条件渲染同级、两态覆盖；header error-note 随 AppHeader 移除） |
| `packages/desktop/src/views/WelcomeView.tsx`、`views/changes/ChangeListView.tsx`、`ChangeDetailView.tsx` | 查询轨 inline 呈现方（error-note 保留不动） |
| `packages/desktop/src/App.test.tsx` | 验证方（toast 轨 DOM 文案断言；inline 轨核对） |

**关联AC**: AC-8

**关系描述**: 双轨把同一 hook 的两类失败拆到两种呈现通道——本 change 最核心的跨模块语义变化。toast 轨经真实 sonner DOM 断言（D8 固定文案前缀 + waitFor 异步时序，proposal 确认 jsdom 可 `getByText`）；查询轨 inline 由既有套件承载（workspace_restore 的 `list_workspaces` reject、ChangeListView / ChangeDetailView / ipc_pipeline 的列表 / 详情 / 产物失败呈现，全部保留不动）。出错模式：①动作失败仍残流 error 态（error-note 与 toast 双呈现，违反 AC-8 前半）；②恢复链 touch 失败被误纳入 error 态阻断启动；③remove resolve(false) 幂等 miss 被误 toast（D8 排除）；④sonner 异步渲染时序致 flaky（约定 waitFor + 固定文案）。

#### 场景: 动作失败 toast 轨（error-note 动作类断言迁移）

验证动作类失败经 toast 呈现且 inline 通道归零。前置：mock IPC 以开关注入动作 reject；输入：右键移除 / 添加 / 恢复链 touch 触发；预期：toast DOM 文案含 D8 固定前缀 + 错误串，`queryAllByTestId('error-note')` 为 0。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | remove reject：toast 呈现「移除 workspace 失败：」+ 错误串（waitFor 文案断言），且 `queryAllByTestId('error-note')` 为 0、停留列表视图（改写：原 Header error-note 呈现断言 → toast 文案断言） | 新增 |
| 正向 | 欢迎屏 add reject：toast 呈现「添加 workspace 失败：」、无 error-note、可重试（改写：原「error-note 呈现」分支二断言 → toast 断言；对话框调用 reject 分支的静默保持现状语义保留） | 新增 |
| 正向 | 恢复链 touch 失败（fire-and-forget）：toast 呈现「切换 workspace 失败：」且启动恢复不阻断（`list_changes` 照常发起） | 新增 |
| 边界 | remove resolve(false)（store miss 幂等）：无 toast、无 error-note（D8 排除项在壳层的核对） | 新增 |
| 边界 | 全部动作与查询成功：无 toast、无 error-note（零呈现基线） | 新增 |

#### 场景: 查询失败 inline 轨与 updater 不变（保留性核对）

验证查询类失败仍走 error-note 持久 inline、updater 错误呈现不被双轨波及。前置：`list_workspaces` reject 开关 / updater error 态；输入：启动渲染；预期：error-note 持久渲染不消失、「重试更新」按钮在场。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `list_workspaces` reject：欢迎屏 error-note 持久渲染不消失（文本含「workspace 清单加载失败」前缀语义）——App 层核对与 workspace_restore 保留用例同一呈现形态 | 新增 |
| 正向 | updater error：「重试更新」按钮在场且点击语义不变（不经 toast，`UpdateIndicator` 逐字不动） | 新增 |

<!-- `list_changes` / `get_change_detail` / `read_artifact` reject 的 inline 呈现由 ChangeListView.test.tsx / ChangeDetailView.test.tsx / __tests__/ipc_pipeline.test.tsx 既有用例承载(proposal 明确保留不动),本关系不重复建用例。 -->

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令分发并以开关切 reject（removeReject / addBehavior / touchReject / listReject） | toast 轨与 inline 轨全部用例 |
| sonner Toaster | 不 mock：App 根真实挂载，断言走 toast DOM 文案 + `waitFor`（sonner 异步渲染时序） | toast 轨全部用例 |
| updater 依赖（`@tauri-apps/api/app` getVersion 等） | 沿用既有 getVersion 固定 resolve 与 updater mock 语义 | 「重试更新」用例 |

---

### 壳层布局与折叠形态：App 壳 ↔ ui/sidebar ↔ use-mobile → `packages/desktop/src/App.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/App.tsx` | 分流方（壳态/欢迎态条件渲染、header 终态） |
| `packages/desktop/src/components/ui/sidebar.tsx` | 布局提供方（provider / `collapsible="icon"` 折叠 / mobile Sheet 分流 / inset） |
| `packages/desktop/src/hooks/use-mobile.ts` | 断点判定方（< 768px → Sheet 抽屉第三态） |
| `packages/desktop/src/components/AppSidebar.tsx` | 壳态挂载物（清单侧栏） |
| `packages/desktop/src/views/WelcomeView.tsx` | 欢迎态挂载物（全屏，不挂壳） |
| `packages/desktop/src/components/ui/sonner.tsx` | 两态覆盖方（Toaster 与条件渲染同级） |

**关联AC**: AC-1, AC-2, AC-7

**关系描述**: 壳重排把布局拓扑从「AppHeader + main」换成「SidebarProvider + AppSidebar + SidebarInset」，并在欢迎态整体不挂壳。jsdom 可断言的是拓扑与语义属性而非观感：壳态存在 provider / sidebar / inset 的 DOM 标记、无 combobox、header 三件套；欢迎态无 sidebar DOM；折叠经 window keyDown（Ctrl/Cmd+B，D1 会话内 state）；< 768px 经 matchMedia stub 切 Sheet。出错模式：①欢迎态误挂 provider（空清单渲染空 sidebar）；②Toaster 漏挂某态（动作失败反馈只在壳态出现）；③折叠快捷键被去 cookie 改造（D1）误伤；④断言复刻 portal 细节导致 flaky（D4-④ 收敛最终态）。

#### 场景: 壳态拓扑与 header 终态

验证壳态 DOM 拓扑与 header 瘦身终态。前置：mock IPC 多记录启动恢复进入壳态；输入：渲染断言；预期：三件套标记在场、无 combobox、header 仅折叠钮 + 标题 + 版本/更新指示。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 壳态渲染 `SidebarProvider` / `AppSidebar` / `SidebarInset` 的 DOM 标记（data-sidebar 属性或 testid），1100px 居中容器位于 inset 内（D7：内层为 div 非 main，无嵌套 main） | 新增 |
| 正向 | 页面无 combobox：`queryByRole('combobox')` 为 null（AC-1 硬断言） | 新增 |
| 正向 | header 终态：折叠钮（SidebarTrigger）+ 标题「Desktop Terminal」+ 版本/更新指示在场；无「刷新列表」「移除」按钮 | 新增 |
| 废弃 | 原「Header 下拉在清单非空时保持可操作（未禁用）」：combobox 随 header 瘦身移除、断言语义消亡，由上一行无 combobox 断言取代 | 废弃 |

#### 场景: 欢迎态隔离与 Toaster 两态覆盖

验证 root 为 null 时壳整体不挂载、Toaster 两态均在。前置：mock IPC 空清单启动；输入：渲染断言（壳态对照）；预期：无 provider/sidebar DOM、WelcomeView 全屏、两态 sonner 容器在场。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | root=null（空清单启动）：无 `SidebarProvider` / `AppSidebar` DOM，WelcomeView 全屏文案在场 | 新增 |
| 正向 | Toaster 与条件渲染同级置于 App 根：欢迎态与壳态各验一次 sonner 容器在场 | 新增 |

#### 场景: 折叠形态与 Sheet 第三态

验证 `collapsible="icon"` 折叠、快捷键与 mobile Sheet 三态机制。前置：壳态启动；输入：keyDown（Ctrl/Cmd+B）/ matchMedia stub 切换 / trigger 点击；预期：折叠态标记切换、< 768px 时走 Sheet 抽屉、折叠态不持久化。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `collapsible="icon"`：触发折叠后 sidebar 容器呈 icon 折叠标记（data-collapsible / data-state），清单项 Tooltip 在折叠态可显（D4-② hover 即显） | 新增 |
| 正向 | Ctrl/Cmd+B：对 window 派发 keyDown（ctrlKey 与 metaKey 两形态）在 collapsed/expanded 间切换（D1 会话内 state） | 新增 |
| 边界 | 窗口 < 768px（matchMedia stub matches=true）：sidebar 走 Sheet 抽屉第三态，SidebarTrigger 点击切换 openMobile（SheetContent 出现/消失，断言收敛最终态，D4-④） | 新增 |
| 边界 | 折叠态不持久化（D1）：卸载重挂回到展开态，无 localStorage / cookie 读写 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `window.matchMedia` | stub 伪实现（记录 query 串 `(max-width: 767px)`、按用例切换 matches） | Sheet 第三态用例（matches=true），其余用例默认 false |
| `@tauri-apps/api/core` invoke / `plugin-dialog` open / `app` getVersion | 沿用壳级三 mock（按命令分发 / 按用例切换 / 固定版本号） | 全部用例（挂载依赖） |

---

## 不可测试项

<!-- 含 test_resolve_paths errors 6 项的落账(`src/components/ui/**` 六件「Not in test config scope」)与 proposal 范围内静态/管线/观感类条目。 -->

- AC-9 后端零改动（`git diff packages/desktop/src-tauri` 为空、`desktop-workspace-store` spec 无本 change delta） — **原因**: diff 缺席与 spec 文本静态约束，属 grep / diff 口径验收而非行为测试；间接佐证为变更清单不含 src-tauri 文件、且集成关系一/二的既有 IPC 契约断言（4 命令签名与 reject→Err(String) 契约）逐字保留全绿。
- AC-10 管线守线（`client:check` 通过且 ui/** 无新增豁免、`vp test` 全绿、ui 生成件 mutate 处置与理由成对） — **原因**: fmt / lint / knip 静态管线与 Stryker 变异执行面验收，与自动化测试分属不同验证层，由任务终验命令承接；「vp test 全绿」半边由本设计全部测试用例的全绿状态间接承载。
- `packages/desktop/src/components/ui/{sidebar,separator,sheet,tooltip,context-menu,sonner}.tsx` 专属单测 — **原因**: test_resolve_paths 报「Not in test config scope」六项（`openspec/config.json` desktop 套件 excludes 预置口径，design D6 维持零改动）；其行为经消费方用例间接验证——toast 文案断言（错误双轨关系）、右键菜单与 Tooltip（关系一与 AppSidebar.test.tsx）、折叠 / Sheet / inset 拓扑（壳层布局关系）。
- `packages/desktop/package.json`（新增 6 个运行时依赖） — **原因**: 依赖清单无可执行单元；依赖正确性与可安装性由 `vp test` / `client:check` 全链路通过间接佐证。
- `packages/desktop/src/styles/global.css` — **原因**: design D6 同源定案零新增（Toaster 配色经 sonner.tsx classNames 映射既有 token），「如需追加」条款以实测对比度触发、非预定改动；纯样式声明无可执行单元，jsdom 无布局引擎。
- 折叠动画 / 侧栏宽度过渡 / toast 位置与时长（sonner 默认 bottom-right）/ Tooltip 视觉观感 — **原因**: CSS 布局与动画渲染面，jsdom 无布局引擎无法进程内断言；且属库自带呈现语义，按既有纪律不逐项验证库语义（sonner / radix 行为语义不测，只测自研接线层）。
- 欢迎态全屏布局形态（flex 居中、max-width 容器视觉） — **原因**: 布局观感属视觉口径；其 DOM 存在性与文案断言已由壳层布局关系「欢迎态隔离」用例承载。

