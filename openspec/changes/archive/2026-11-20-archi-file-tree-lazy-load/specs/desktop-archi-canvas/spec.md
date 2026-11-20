# desktop-archi-canvas Specification（delta: archi-file-tree-lazy-load）

## Purpose

本 delta 只改**左栏缺省视图这一个事实**及其复述：两栏布局、右栏 `Card` 三段结构、1-hop 缺省渲染、分组容器、入边分区、停摆态、标记面、`mode` 能力差异、元素信息面板**逐字不动**。

1. 左栏三视图的缺省由**文件树**改为**元素树**，两个挂载点（`/architecture` 与 change 详情「架构」页签）同改；分段控件声明序随之为 元素 / 文件 / 契约。
2. 首屏不再挂载文件视图 ⇒ `archi_file_tree` 调用数为 0；文件树的取数、标记与联动语义归 `desktop-archi-file-tree`，本能力不新增任何取数纪律。
3. 本能力不承载「为什么不冻结」——执行线程纪律归 `desktop-app-shell`「命令执行线程纪律」；本能力只持有「左栏是画布区第一栏且缺省元素树」。

## MODIFIED Requirements

### Requirement: 单一画布组件与两处挂载点

架构画布 SHALL 为**一个共享组件**（`ArchitectureCanvas`，入参 `{ root, mode }`），在仓库内两处挂载，MUST NOT 为两处各写一套渲染：

| 挂载点 | 位置 | `root` | `mode` |
|---|---|---|---|
| A | `/architecture` 页面（侧栏「架构」入口） | 当前 workspace 主仓根 | `full` |
| B | change 详情页「架构」页签 | `detail.worktree`；归档 change 回落主仓根 | `incremental` |

- **观察根就是当前工作根**：命令签名 SHALL 一律显式带 `root`，MUST NOT 引入「当前选中 change」一类隐式上下文。change 侧的 `root` 派生口径 SHALL 与既有会话 `cwd` 口径同源（`detail.worktree`；归档 change 恒回落主仓根）。
- **显式取数**：画布取数 SHALL 只在三种触发下发起——组件挂载、`root` 变化、用户显式刷新；MUST NOT 引入目录监听（watch）或轮询（沿 `desktop-file-watch` 既有红线，本变更不得以任何形式偷加自动刷新）。
- 画布区块 SHALL 为**两栏横向可调宽布局**（`ResizablePanelGroup`）：**左栏**为 `ResizablePanel`（`defaultSize={300}` / `minSize={200}`；面板内 base-ui `Tabs` 在**一个栏位内三种视图**间切换——缺省**元素树**（层级由 FQN 前缀派生）/ **文件树**（磁盘枚举 + 元素根归属标记）/ **契约视图**（生成物差异 + 契约判据两轴，见 `desktop-archi-contract-panel`）；三视图共用同一面板，栏宽可拖拽、MUST NOT 随视图切换变化）；**右栏**为 `ResizablePanel` 内的 `Card`——`CardHeader` 承载元素信息面板（`ModelInfo`，测试挂钩沿用 `archi-drawer`），`CardContent` 承载 xyflow 画布（节点 = element，边 = relation，`data-testid="archi-canvas"`）。左栏三种视图的完整契约分别归 `desktop-archi-file-tree`（文件 / 元素两视图、分段切换语义与缺省视图）与 `desktop-archi-contract-panel`（契约视图）；本能力只登记「左栏是画布区第一栏、其缺省视图为元素树且文件树不参与首屏」。
- 缺省渲染 SHALL 只呈现**聚焦元素的 1-hop 邻域**（入边 + 出边 + 直接子元素），聚焦切换 SHALL 由图内交互完成；全量铺开 MUST NOT 作为缺省形态（防大模型过载）。
- 直接子元素 SHALL 渲染在聚焦节点**区域内**：聚焦元素有直接子元素时呈分组容器节点（标题栏 + 内容区），子节点以 xyflow `parentId` + `extent: 'parent'` 挂在容器下、逐行排在内容区内；邻域入 / 出边节点分布在容器左右两侧之外，MUST NOT 落进容器区域。
- 入边列（`dependedBy`：依赖当前元素的节点）SHALL 以**虚线分区**框选：分区完整包住整列入边节点，作为画布背景装饰——MUST NOT 计入节点集、MUST NOT 响应点击 / 聚焦；无入边时不渲分区。
- 测试挂钩 SHALL 用 data-testid，MUST NOT 以样式类名作查询挂钩（沿 `desktop-standard-components` 既有纪律）。

#### Scenario: 两处挂载点共用同一组件

- **WHEN** 分别打开 `/architecture` 与某 active change 详情的「架构」页签
- **THEN** 两处呈现同一组件（可调宽左栏面板 / 右栏元素信息与画布齐备，左栏分段控件三视图齐备）；A 的 `root` 为主仓根，B 的 `root` 为该 change 的 `detail.worktree`；两处左栏缺省均为**元素树**，两处首屏 `archi_file_tree` 调用数均为 0

#### Scenario: 归档 change 回落主仓根

- **WHEN** 打开一个已归档 change 的详情页并切到「架构」页签
- **THEN** 取数以主仓根发起（与归档 change 会话 `cwd` 恒回落同口径），MUST NOT 以已清理的 worktree 路径取数

#### Scenario: 三触发取数与零监听

- **WHEN** 审查画布取数调用点，并观察运行期无任何目录变化
- **THEN** 取数只在挂载 / `root` 变化 / 显式刷新三处发起；源码无 watch 接入、无定时轮询；磁盘外改模型时画布不自动变化（须用户刷新）

#### Scenario: 缺省 1-hop

- **WHEN** 模型含数十元素且无元素被显式聚焦
- **THEN** 画布只渲染聚焦元素（缺省取根元素或首个顶层元素）及其 1-hop 邻域；非邻域元素不出现在图内，全量仅经显式操作展开

#### Scenario: 直接子元素渲染在聚焦节点区域内

- **WHEN** 聚焦一个有直接子元素的元素
- **THEN** 该元素渲染为分组容器节点（标题栏呈短名），直接子元素挂在其下并逐行排在容器内容区内；邻域入边 / 出边节点分布在容器左右两侧之外，不出现在容器区域内

#### Scenario: 入边虚线分区框选

- **WHEN** 聚焦一个有入边（`dependedBy`）的元素
- **THEN** 画布在入边列外围渲染一个虚线矩形分区，完整框住该列全部节点；分区不参与节点点击与聚焦（背景装饰），无入边时分区缺席

## Module Contract

### 模块增量：左栏缺省视图换面（archi-file-tree-lazy-load）

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 共享画布 + 左栏分段控件 | 左栏 `ResizablePanel`（`defaultSize={300}` / `minSize={200}`）内的 base-ui `Tabs`：`value` 受控于 `treeMode`，**初值 `'element'`**；`TabsList`（`data-testid="archi-tree-mode"`）内三枚 `TabsTrigger` 声明序 **元素 / 文件 / 契约**；三枚 `TabsContent` 各渲 `ElementTree` / `FileTree` / `ContractPanel`；缺省渲元素树 ⇒ `FileTree` 不挂载、`archi_file_tree` 零调用。右栏、1-hop、分组容器、入边分区、停摆分支、标记接线、`mode` 语义**逐字不变** |
| `packages/desktop/src/views/architecture/components/element-tree.tsx` | 左栏元素视图（成为缺省） | 入参（`elements` / `focused` / `onFocus`）、渲染、testid（`archi-element-tree` / `archi-tree-node`）零改动；消费画布已取的 `elements` ⇒ 零新增 invoke |
| `packages/desktop/src/views/architecture/components/file-tree.tsx` | 左栏文件树（第二视图） | 数据面 / 标记 / 点击 / 反查 / 展开态见 `desktop-archi-file-tree`；本能力只要求「点击走同一次 `onFocus`、缺省不挂载故不取数」 |
| `packages/desktop/src/views/architecture/hooks/use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts` | 取数纪律 | **零改动**：三触发（挂载 / `root` 变化 / 显式刷新）、零 watch 零轮询、失败面与返回形状逐字不变 |
| `packages/desktop/src/views/architecture/lib/marks.ts` / `lib/drift-marks.ts` | 标记映射 | **零改动**（缺省视图与边标记生产无关） |
