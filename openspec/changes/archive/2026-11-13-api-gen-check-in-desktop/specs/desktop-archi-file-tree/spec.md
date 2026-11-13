# desktop-archi-file-tree Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改一条 requirement：左栏分段控件由两钮扩为三钮（`file` / `element` / `contract`，缺省仍 `file`），并把契约视图的完整契约转指 `desktop-archi-contract-panel`。命令与 DTO、`FileTreeMark` 五值词表、归属判定与判定单源、枚举面一致性、元素根缺失不静默、树取数状态面六条 requirement 逐字不变。

## MODIFIED Requirements

### Requirement: 左栏文件树呈现、分段切换与双向联动

`/architecture`（与 change 详情「架构」页签共用的同一画布组件）的三段布局第一段 SHALL 提供文件树，并与元素树、契约视图以分段控件切换：

- **缺省文件树**：左栏缺省渲文件树；分段控件（`data-testid="archi-tree-mode"`，三钮 `data-mode="file"` / `data-mode="element"` / `data-mode="contract"`）SHALL 切换左栏组件；切换只换左栏、`focus` 共用，MUST NOT 触发画布或抽屉重新取数。
- **契约视图转指**：`data-mode="contract"` 的视图形态、两轴呈现、生成交互与取数纪律归 `desktop-archi-contract-panel`；本能力只登记「分段控件存在第三值且切换不引发画布 / 抽屉重取数」。
- **元素树保留**：元素树 MUST NOT 删除——容器元素（`path` 空）只在 FQN 树上可见，且「谁依赖谁」在 FQN 树上更顺；其入参、渲染与既有 testid（`archi-element-tree` / `archi-tree-node`）零变化。
- **渲染顺序**：SHALL 目录先于文件、各自按 `name` 排序（后端只按 `path` 字节序，呈现序由前端派生）；缩进表达层级。
- **标记呈现**（图标 + 色 + 名称 + tooltip）：

  | `mark` | 图标 / 色 | 名称 | tooltip |
  |---|---|---|---|
  | `elementRoot` | 实心圆点 + 主色 | 加粗 | `模块 <FQN>` |
  | `ambiguous` | 警示色圆点 | 加粗 | `多归属：A、B` |
  | `member` | 左侧竖线 + 次级色 | 常规（弱化） | `属于 <FQN>` |
  | `containsElements` | 空心圆 + 计数 | 常规 | `含 N 个模块`（尾随小字 `·N`） |
  | `none` | 无 | 常规 | — |

- **点击（树 → 画布）**：`elementRoot` / `member` 行点击 SHALL 调 `onFocus(owners[0])`（即既有 `setFocusOverride`，一次调用、与元素树同语义）；`ambiguous` / `containsElements` / `none` 行点击 SHALL **只切该行展开态、零 `onFocus`**（MUST NOT 为候选选择新增弹层原语；`owners` 已是全量候选，将来放开只改点击处理）。
- **反查（画布 / 抽屉 → 树）**：`focus` 变化时 SHALL 以「条目 `owners` 是否含该 FQN」定位（判据 MUST NOT 依赖元素 `path` 在前端重算），对全部命中条目（多 `path` 元素会有多条）展开其祖先链、滚动到第一条、并让其同时高亮；**零命中**时 SHALL 在树顶部呈一行弱提示「该元素在树上无可定位节点（容器元素不持有文件，或该元素源文件均在判定面之外）」，MUST NOT 滚动。提示文案 MUST NOT 断言「该元素是容器」——`path` 非空的元素若其源文件全被排除矩阵排除也会零命中，两种成因不可在同一句里混说。
- **展开态**：SHALL 为 per-path 翻转集合（沿 `views/info/components/dir-tree.tsx` 既有模式），默认展开前 2 段路径，刷新后按 `path` 保持用户操作态；折叠 SHALL 不渲染子树 DOM。
- **图例**：树底部 SHALL 常驻一行可折叠图例，说明「仅显示判定面源文件（测试文件与架构配置 `exclude` 命中项不在树中；`none` 行 = 无元素认领）」。
- **测试挂钩**：SHALL 用 `data-testid`（`archi-file-tree` / `archi-file-node` / `archi-file-mark` 携 `data-mark` / `archi-tree-mode`），MUST NOT 以样式类名作查询挂钩。
- **不抽公共件**：文件树组件 MUST NOT 与 `views/info/components/dir-tree.tsx` 抽公共抽象——数据面不同（marks / owners vs 行数聚合），抽象需泛型参数化而收益为零。

#### Scenario: 缺省渲文件树且可切回元素树

- **WHEN** 打开 `/architecture`（或 change 详情「架构」页签）
- **THEN** 左栏缺省为文件树（`archi-file-tree` 在场、`archi-element-tree` 不在场）；点分段钮 `data-mode="element"` 后元素树在场且其 testid 与切换前既有断言一致；`archi_graph` / `archi_module` / `archi_drift` 调用次数不因切换增加

#### Scenario: 第三钮切到契约视图

- **WHEN** 点分段钮 `data-mode="contract"`
- **THEN** 左栏渲契约视图（`archi-contract-panel` 在场、`archi-file-tree` 与 `archi-element-tree` 均不在场）；`archi_graph` / `archi_module` / `archi_drift` 调用次数不因切换增加（契约视图自身的检查取数归 `desktop-archi-contract-panel`）

#### Scenario: 点模块行圈定范围

- **WHEN** 点击某 `mark = elementRoot` 或 `member` 行
- **THEN** 触发一次 `onFocus(owners[0])`：画布切到该元素的 1-hop 邻域、抽屉取该元素详情（`archi_module` 恰一次）

#### Scenario: 非归属行只切展开

- **WHEN** 点击 `mark` 为 `ambiguous` / `containsElements` / `none` 的行
- **THEN** 该行展开态翻转，`onFocus` 零调用（`archi_module` 调用次数不增），MUST NOT 出现候选弹层

#### Scenario: 聚焦多 path 元素多行高亮

- **WHEN** 画布或抽屉把 `focus` 切到声明了多条 `path` 的元素（如 `App.Frontend.Shell`）
- **THEN** 其全部对应节点同时高亮（`data-mark="elementRoot"` 且行底为高亮态）、祖先链展开、视图滚动到第一条

#### Scenario: 容器元素聚焦呈提示不滚动

- **WHEN** `focus` 为 `path` 为空的容器元素
- **THEN** 树顶部呈一行「该元素在树上无可定位节点」类弱提示（成因含「容器不持有文件」）；树不滚动、不尝试定位任何节点，且画布与抽屉的 1-hop 呈现不受影响

#### Scenario: 折叠不渲染子树且展开态按 path 保持

- **WHEN** 展开某深层目录后触发一次刷新重挂（同一批 `path`）
- **THEN** 该目录仍为展开态（per-path 翻转集合保持）；折叠态子树的 DOM 零节点

## Module Contract

### Component: `packages/desktop/src/views/architecture/components/file-tree.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| `TreeMode` | 改 | `'file' \| 'element'` → `'file' \| 'element' \| 'contract'` |
| `TreeModeSwitch` | 改 | 选项表加第三钮（`data-mode="contract"`，文案「契约」）；控件 testid、栅格形态与另两钮零变化 |
| `FileTree` 本体 | 零改动 | 节点渲染、标记、点击语义、双向联动、图例逐字不变 |
