# desktop-archi-file-tree Specification（delta: archi-file-tree-lazy-load）

## Purpose

本 delta 只换**缺省视图与取数时机**，文件树的数据面逐字不动：

1. **缺省由文件树翻为元素树**：左栏分段控件的声明序改为 元素 / 文件 / 契约，缺省值即首项。效果是首屏不再无条件付一次全仓 walk（实测 42,847 文件 → 59,771 节点 → 约 10.4 MB → 3.1 s），文件树取数与漂移取数彻底解耦。
2. **首屏零 `archi_file_tree`**：文件视图仍遵守「非活动面板不挂载」，于是「首次挂载 = 用户显式切到文件」；hash 直入 / 侧栏进入 / 切走再切回三种入口的调用数均为 0。
3. **零语义漂移**：条目投影七字段、`FileTreeMark` 五值、归属判定、枚举与排除面、点击与反查、展开态、三触发纪律、失败面**全部零改动**；`archi_file_tree` 的命令形态（`async fn` + `spawn_blocking`）随执行线程纪律翻面，IPC 签名与错误文案不变。

本 delta 不含：逐层懒加载 / 深度截断（收益已被缺省切换清零，且真做必须收窄遍历根并保 gitignore + `ExcludeSet` 语义）、DTO 字段增删、文件树组件抽象。

## MODIFIED Requirements

### Requirement: 左栏文件树呈现、分段切换与双向联动

`/architecture`（与 change 详情「架构」页签共用的同一画布组件）的两栏布局左栏 SHALL 提供文件树，并与元素树、契约视图以分段控件切换：

- **缺省元素树**：左栏缺省渲元素树；文件视图 SHALL NOT 参与首屏——缺省进入页面时 `FileTree` MUST NOT 挂载、`archi_file_tree` 调用数 MUST 为 **0**。分段控件（`Tabs` / `TabsList` / `TabsTrigger` / `TabsContent`，栏位 `data-testid="archi-tree-mode"` 落在 `TabsList`）SHALL 切换左栏组件；三钮的声明序 SHALL 为「元素」/「文件」/「契约」（缺省值即首项，`treeMode` 初值与 `TabsTrigger` 首项同为「元素」——两处 MUST NOT 分叉），以 `role="tab"` + 文案寻址，选中态由 base-ui 的 `aria-selected` / `data-active` 表达（MUST NOT 依赖自绘 `aria-pressed` 与 `data-mode`）。切换只换左栏、`focus` 共用，MUST NOT 触发画布或元素信息面板重新取数。
- **非活动面板不挂载**：`TabsContent` 的非活动面板 SHALL 不挂载（base-ui 缺省 `keepMounted = false`），故文件视图的**首次挂载 = 用户显式切到「文件」**，切回「文件」时 `FileTree` 重挂、其自身取数随之重发一次；该重取属于文件树的挂载触发，MUST NOT 连带 `archi_graph` / `archi_module` / `archi_drift`。
- **栏宽可调且跨视图恒定**：左栏 SHALL 由 `ResizablePanel` 承载（`defaultSize={300}` / `minSize={200}`，Tabs 以 `w-full h-full` 铺满），三视图共用同一面板；栏宽可拖拽调整，MUST NOT 随视图切换变化。
- **契约视图转指**：`value="contract"` 的视图形态、两轴呈现、生成交互与取数纪律归 `desktop-archi-contract-panel`；本能力只登记「分段控件存在第三值且切换不引发画布 / 元素信息面板重取数」。
- **元素树保留**：元素树 MUST NOT 删除——容器元素（`path` 空）只在 FQN 树上可见，且「谁依赖谁」在 FQN 树上更顺；其入参、渲染与既有 testid（`archi-element-tree` / `archi-tree-node`）零变化。缺省视图换成它 MUST NOT 附带任何取数变化（它消费画布已取的 `elements`，零新增 invoke）。
- **渲染顺序**：SHALL 目录先于文件、各自按 `name` 排序（后端只按 `path` 字节序，呈现序由前端派生）；缩进表达层级。
- **标记呈现**（标记节点 / 名称 / tooltip；`member` 不渲标记节点，仅名称弱化）：

  | `mark` | 标记节点 | 名称 | tooltip |
  |---|---|---|---|
  | `elementRoot` | 次色按钮「模块 <短名>」+ 尾随 chevron（聚焦命中时转主色） | 加粗 | `切换至模块 <FQN>` |
  | `ambiguous` | 警示色徽章「多归属」 | 加粗 | `多归属：A、B` |
  | `member` | 无（仅名称弱化） | 常规（弱化） | — |
  | `containsElements` | 次色徽章（加号图标 + 计数 `N`） | 常规 | `含 N 个模块` |
  | `none` | 无 | 常规 | — |

- **点击（树 → 画布）**：`elementRoot` / `member` 行点击 SHALL 调 `onFocus(owners[0])`（即既有 `setFocusOverride`，一次调用、与元素树同语义）；`ambiguous` / `containsElements` / `none` 行点击 SHALL **只切该行展开态、零 `onFocus`**（MUST NOT 为候选选择新增弹层原语；`owners` 已是全量候选，将来放开只改点击处理）。
- **反查（画布 / 元素信息面板 → 树）**：`focus` 变化时 SHALL 以「条目 `mark = elementRoot` 且 `owners` 含该 FQN」定位锚点（判据 MUST NOT 依赖元素 `path` 在前端重算；多 `path` 元素会有多条锚点），展开全部锚点的祖先链与锚点自身（根节点的直接子行可见）、滚动到第一条，并让全部 `owners` 命中条目（含 `member` 内部文件）同时高亮；锚点之下的子节点 MUST NOT 强制展开（不下钻，保持缺省 / 用户翻转态）；**零锚点**时 SHALL 静默——零提示节点、零高亮、MUST NOT 滚动（`path` 为空的容器元素即此形态）。
- **展开态**：SHALL 为 per-path 翻转集合（沿 `views/info/components/dir-tree.tsx` 既有模式），默认展开前 3 段路径，刷新后按 `path` 保持用户操作态；折叠 SHALL 不渲染子树 DOM。
- **测试挂钩**：SHALL 用 `data-testid`（`archi-file-tree` / `archi-file-node` / `archi-file-mark` / `archi-tree-mode` / `archi-file-tree-error` / `archi-file-tree-loading` / `archi-file-tree-empty`），MUST NOT 以样式类名作查询挂钩；`archi-file-mark` 只出现在 `elementRoot` / `ambiguous` / `containsElements` 三值的标记节点上并携 `data-mark`（`member` / `none` 无标记节点）；分段钮按 `role="tab"` + 文案寻址。
- **不抽公共件**：文件树组件 MUST NOT 与 `views/info/components/dir-tree.tsx` 抽公共抽象——数据面不同（marks / owners vs 行数聚合），抽象需泛型参数化而收益为零。

#### Scenario: 缺省渲元素树且切到文件树

- **WHEN** 打开 `/architecture`（或 change 详情「架构」页签）
- **THEN** 左栏缺省为元素树（`archi-element-tree` 在场、`archi-file-tree` 不在场）且 `archi_file_tree` 调用数为 0；点分段钮「文件」后文件树在场（其 testid 与切换前既有断言一致）且 `archi_file_tree` 恰 1 次；`archi_graph` / `archi_module` / `archi_drift` 调用次数不因切换增加

#### Scenario: 三种缺省入口均零文件树取数

- **WHEN** 分别以 hash 直入 `#/architecture`、点侧栏「架构」进入、切走再切回三次路径进入页面
- **THEN** 每次挂载的 `archi_file_tree` 调用数均为 0、`archi_graph` 各恰 1 次；文件树既不挂载也不处加载态（`archi-file-tree-loading` 零出现）

#### Scenario: 分段钮声明序与缺省选中

- **WHEN** 审查分段控件 DOM 或按 `role="tab"` 逐个取文案
- **THEN** 声明序为「元素」/「文件」/「契约」；缺省选中「元素」（`aria-selected` / `data-active` 之一为真）；三视图挂在同一 `ResizablePanel` 内、切换前后栏宽不变

#### Scenario: 切回文件树只重取文件树

- **WHEN** 在「文件」/「元素」两视图间往返两次后回到「文件」
- **THEN** 文件视图每次挂载各发一次 `archi_file_tree`（首次切到「文件」亦恰 1 次）；非活动面板不挂载语义不变，而 `archi_graph` / `archi_module` / `archi_drift` 计数不变

#### Scenario: 第三钮切到契约视图

- **WHEN** 点分段钮「契约」
- **THEN** 左栏渲契约视图（`archi-contract-panel` 在场、`archi-file-tree` 与 `archi-element-tree` 均不在场）；`archi_graph` / `archi_module` / `archi_drift` 调用次数不因切换增加（契约视图自身的检查取数归 `desktop-archi-contract-panel`）

#### Scenario: 点模块行圈定范围

- **WHEN** 点击某 `mark = elementRoot` 或 `member` 行
- **THEN** 触发一次 `onFocus(owners[0])`：画布切到该元素的 1-hop 邻域、元素信息面板取该元素详情（`archi_module` 恰一次）

#### Scenario: 非归属行只切展开

- **WHEN** 点击 `mark` 为 `ambiguous` / `containsElements` / `none` 的行
- **THEN** 该行展开态翻转，`onFocus` 零调用（`archi_module` 调用次数不增），MUST NOT 出现候选弹层

#### Scenario: 聚焦多 path 元素多行高亮

- **WHEN** 画布或元素信息面板把 `focus` 切到声明了多条 `path` 的元素（如 `App.Frontend.Shell`）
- **THEN** 其全部对应节点同时高亮（`data-mark="elementRoot"` 且行底为高亮态）、各锚点及其祖先链展开、视图滚动到第一条；锚点内部的下级目录不强制展开

#### Scenario: 选中模块只展开根节点不下钻

- **WHEN** `focus` 切到某目录型元素，其 `path` 目录内还有多级子目录与 `member` 文件
- **THEN** 模块根节点行及其祖先链展开（根节点的直接子行可见），更深的子目录保持缺省 / 用户翻转态（MUST NOT 因内部文件的 `member` 命中被强制展开）；用户手动展开后，`member` 行照常高亮

#### Scenario: 容器元素聚焦静默不滚动

- **WHEN** `focus` 为 `path` 为空的容器元素
- **THEN** 树上零节点命中：不渲提示节点、不滚动、不尝试定位任何节点，且画布与元素信息面板的 1-hop 呈现不受影响

#### Scenario: 折叠不渲染子树且展开态按 path 保持

- **WHEN** 展开某深层目录后触发一次刷新重挂（同一批 `path`）
- **THEN** 该目录仍为展开态（per-path 翻转集合保持）；折叠态子树的 DOM 零节点

## Module Contract

### 模块增量：缺省视图换面（archi-file-tree-lazy-load）

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 左栏分段控件与缺省视图 | `treeMode` 初值 `'element'`；`options` 声明序 元素 / 文件 / 契约（与 `TabsTrigger` 同源）；三视图共用同一 `ResizablePanel`（`defaultSize={300}` / `minSize={200}`）；切换零额外取数 |
| `packages/desktop/src/views/architecture/components/file-tree.tsx` | 文件树（第二视图） | `TreeMode` 三值与全部渲染 / 标记 / 点击 / 反查 / 展开态语义**零改动**；`TreeMode` 上方注释不再陈述缺省（缺省值单点在 `architecture-canvas.tsx`） |
| `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts` | 文件树取数 | **零改动**：挂载 / `root` 变化 / 上层 `key` 重挂三触发；缺省不挂载 ⇒ 零调用（纪律不变，只是触发次数变少） |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs::archi_file_tree` | 命令面形态 | `pub async fn` + `spawn_blocking(move \|\| archi_file_tree_inner(…))`，`JoinError` → `Err`；IPC 签名（`root: String → Result<ArchiFileTree, String>`）、参数、返回与四条非法 root 文案逐字不变；线程形态与判据的唯一持有者是 `desktop-app-shell`「命令执行线程纪律」 |
| `packages/desktop/src-tauri/crates/core/archi/src/tree.rs` / `tree_test.rs` | 树投影 | **零改动**（条目七字段、标记五值、归属 / 枚举 / 排除 / 目录合成 / `exists` 全不动） |

### 零改动面（本 delta）

| 模块 | 说明 |
|---|---|
| `components/file-tree.test.tsx` | 直测 `FileTree`，不经缺省视图 ⇒ 零改动 |
| `hooks/use-archi-file-tree.test.ts` | 直测钩子（不渲染左栏）⇒ 三触发与返回形状零改动 |
| `components/element-tree.tsx` | 成为缺省视图，入参 / 渲染 / testid 零改动（零新增 invoke） |
