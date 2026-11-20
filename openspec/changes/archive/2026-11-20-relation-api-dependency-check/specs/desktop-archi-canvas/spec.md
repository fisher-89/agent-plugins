# desktop-archi-canvas Specification (Delta)

## ADDED Requirements

### Requirement: 关系信息面板（RelationInfo）与边点击

画布的边 SHALL **可点**：点击一条边后，右栏 `CardHeader` 由元素态（`ModelInfo`）切到**关系态**（`RelationInfo`），两者共用同一块面板位——MUST NOT 新开抽屉、MUST NOT 用边旁浮卡（`EdgeLabelRenderer`）呈现详情。

- **两态互斥且可回来**：同一时刻 `CardHeader` 恰呈一种面态；点边 → 关系态，点节点 → 元素态（并清空选中边）；`root` 变化、用户显式刷新、取数失败降级时选中边 SHALL 清空（回到元素态或空态），MUST NOT 保留指向旧图的选中。`CardContent` 的 xyflow 画布与既有 1-hop / 分组容器 / 入边分区渲染逐字不变。
- **边身份 = `(from, to)`**：选中态 SHALL 以元素对为身份（与 `ArchiRelation` / 报告聚合同一身份，沿用既有 edge `id = '<from>-><to>'`），MUST NOT 以像素 / 边序号为身份。
- **两段取数分工（零重复 invoke）**：**声明清单**取图数据里已有的 `ArchiRelation.apis`（模型拷贝，点开即有，**零取数**）；**实际使用 / 两桶 / 命中点 / 模型行号**取自按需查询 `archi_relation(root, from, to)`（见 `desktop-archi-model`「IPC 命令面契约（九命令）」）。面板 MUST NOT 自行求差、MUST NOT 读 `archi_drift` 报告重算——两桶是命令返回值。
- **面板七段（顺序固定）**：① 关系（`from` → `to` + 标记徽标，取 `ArchiRelation.mark` / `reason`）；② 说明（`description`，`null` 时不占文案）；③ 声明 API（`apis` 逐名，空则空态①）；④ 实际使用（`used` 逐名）；⑤ 两桶（`usedNotDeclared` / `declaredNotUsed` 两行，空则显「—」）；⑥ 命中点（`occurrences` 逐条 `file:line` + `specifier`）；⑦ 模型（`modelFile:modelLine` + 「查看」，经通用 `read_file` 打开该模型文件——与入口文档同一通路，MUST NOT 新增专用读取命令）。
- **四种空态必须彼此可辨（各自的文案与 testid 不同）**：① **未声明 `apis`**（「未声明依赖 API（选填）」）——此态 **MUST NOT 发起 `archi_relation` 取数**（没有可比对的清单，取了也只是空转）；② **扫描面无该关系的 import**（「扫描面无该关系的 import」= `unfulfilledDeclaration` 的事实面）；③ **边被豁免**（「已豁免」+ `via`（`config` / `inline`）+ `reason`）；④ **取数失败**（行内错误 + 「重试」）。四态 MUST NOT 合并成一句「无数据」。
- **`declaredNotUsed` 的文案 MUST 并列两种成因**：可能已不再依赖 / 也可能名字写错或该依赖以方法调用、内联限定路径、glob 形态存在而扫描看不见（与 `desktop-archi-drift`「依赖 API 两桶对账与求差单点」同一措辞口径）。面板 MUST NOT 把它呈现为「已失效」一类断言。
- **失败隔离**：`archi_relation` 失败只落本面板行内错误 + 重试入口——画布图、左栏三视图与元素态面板照常渲染，MUST NOT 空白、MUST NOT 触发重取图。
- **取数纪律（三触发的显式开窗）**：边点击的按需查询由**显式用户动作**发起，不计入「挂载 / `root` 变化 / 显式刷新」三触发；其触发面 SHALL 恰为「选中边变化（且该元素对有 `apis`）」与「用户点重试」。MUST NOT 引入目录监听、定时轮询或选中态自动重取（同一元素对重复选中不重复 invoke）。
- **词汇零扩张**：本面板 MUST NOT 引入新的边标记值 / 节点标记值（`EdgeMark` 四值、节点标记仍不呈现），MUST NOT 在边上渲染色块徽标——标记只经既有 `mark` / `reason` 以文本徽标呈现。
- **面板不承载报告统计**：MUST NOT 呈现全仓 `errors` / `warnings` 计数或门禁结论；呈现的只是**当前这一条边**的声明、事实与两桶（与元素面板「本期无相邻报告区块」同口径）。

#### Scenario: 点边切关系态、点节点回元素态

- **WHEN** 在画布上依次点击一条边、随后点击一个节点
- **THEN** 第一次点击后 `CardHeader` 呈 `RelationInfo`（关系 / 说明 / 声明 API / 实际使用 / 两桶 / 命中点 / 模型七段），选中边高亮；第二次点击后回到 `ModelInfo`（元素概览 / 契约段 / 入口文档 / 改档入口照旧），选中边清空——同一时刻只呈一种面态

#### Scenario: 未声明 apis 不发起取数

- **WHEN** 点击一条 `apis` 为空的边
- **THEN** 面板呈空态①「未声明依赖 API（选填）」，实际使用 / 两桶 / 命中点 / 模型四段整段不渲染；`archi_relation` **零 invoke**（断言命令调用计数为 0）

#### Scenario: 声明与实际一致

- **WHEN** 点击一条 `apis = ["AgentInput", "SessionPanel"]` 且扫描事实同名的边
- **THEN** 声明 API 与「实际使用」两段各两行同名；两桶两行皆「—」；命中点至少一条（带 `file:line` 与 `specifier`）；不呈现任何空态文案

#### Scenario: 用了没声明与声明没用各呈一行

- **WHEN** 点击一条 `apis = ["AgentInput", "SessionPanel"]` 而事实为 `["AgentInput", "FormShell"]` 的边
- **THEN** 两桶段呈「用了没声明：FormShell」与「声明没用：SessionPanel」两行；后者同行附「可能已不再依赖，也可能名字写错或扫描看不见」的并列措辞（MUST NOT 只写「没用」）

#### Scenario: 扫描面无 import 的空态

- **WHEN** 点击一条 `apis` 非空、但扫描面零命中的边（该元素对为 `unfulfilledDeclaration`）
- **THEN** 「实际使用」段为空、两桶段「用了没声明」为空、命中点段呈空态②「扫描面无该关系的 import」；边在画布上仍为 `unfulfilled` 的既有样式（标记面零改动）

#### Scenario: 豁免边的空态

- **WHEN** 点击一条报告里 `suppressed[]` 命中的边（`via = config`，reason 在位）
- **THEN** 面板呈空态③「已豁免」+ `via` + `reason` 原文；徽标沿用既有 `exempt` 呈现，MUST NOT 新增标记值

#### Scenario: 取数失败只落行内错误

- **WHEN** `archi_relation` 返回 `Err`（如模型在两次调用之间被改坏）
- **THEN** 面板呈空态④（行内错误 + 「重试」），画布图与左栏三视图照常渲染、元素态面板零影响；点「重试」才发起第二次 invoke

#### Scenario: 模型行号可定位

- **WHEN** 点击任一条边并查看面板「模型」段
- **THEN** 呈 `modelFile:modelLine`（如 `.dev-team/architecture/graph/model/06-frontend.toml:512`），点「查看」经通用 `read_file` 打开该文件；MUST NOT 新增架构域专用文档读取命令

#### Scenario: 零监听零轮询（按需查询不计入三触发）

- **WHEN** 审查 `use-archi-relation` 与画布的取数调用点，并让选中边保持不动一段时间
- **THEN** 源码无 watch 接入、无 `setInterval`、无自动重取；同一元素对重复选中不产生第二次 invoke；除挂载 / `root` 变化 / 显式刷新三处外，唯一新增的 invoke 来源是「用户点边（或点重试）」

## MODIFIED Requirements

### Requirement: 单一画布组件与两处挂载点

架构画布 SHALL 为**一个共享组件**（`ArchitectureCanvas`，入参 `{ root, mode }`），在仓库内两处挂载，MUST NOT 为两处各写一套渲染：

| 挂载点 | 位置 | `root` | `mode` |
|---|---|---|---|
| A | `/architecture` 页面（侧栏「架构」入口） | 当前 workspace 主仓根 | `full` |
| B | change 详情页「架构」页签 | `detail.worktree`；归档 change 回落主仓根 | `incremental` |

- **观察根就是当前工作根**：命令签名 SHALL 一律显式带 `root`，MUST NOT 引入「当前选中 change」一类隐式上下文。change 侧的 `root` 派生口径 SHALL 与既有会话 `cwd` 口径同源（`detail.worktree`；归档 change 恒回落主仓根）。
- **显式取数**：画布取数 SHALL 只在三种触发下发起——组件挂载、`root` 变化、用户显式刷新；MUST NOT 引入目录监听（watch）或轮询（沿 `desktop-file-watch` 既有红线，本变更不得以任何形式偷加自动刷新）。
- **按需查询不计入三触发（本变更显式开窗）**：边点击取关系详情（`archi_relation`，见「关系信息面板（RelationInfo）与边点击」）由**显式用户动作**发起，既非 watch 也非轮询，**不计入上述三触发**；其触发面 SHALL 恰为「选中边变化（且该元素对有 `apis`）」与「用户点重试」。除此之外 MUST NOT 有任何自动重取（含选中态保持期间的任何重取、`root` 未变时的后台刷新）。本条是本 requirement 三触发纪律的唯一开窗，MUST NOT 被援引为引入 watch / 轮询 / 定时刷新的先例。
- 画布区块 SHALL 为**两栏横向可调宽布局**（`ResizablePanelGroup`）：**左栏**为 `ResizablePanel`（`defaultSize={300}` / `minSize={200}`；面板内 base-ui `Tabs` 在**一个栏位内三种视图**间切换——缺省**文件树**（磁盘枚举 + 元素根归属标记）/ **元素树**（层级由 FQN 前缀派生）/ **契约视图**（生成物差异 + 契约判据两轴，见 `desktop-archi-contract-panel`）；三视图共用同一面板，栏宽可拖拽、MUST NOT 随视图切换变化）；**右栏**为 `ResizablePanel` 内的 `Card`——`CardHeader` 承载**两态面板**（元素态 `ModelInfo`，测试挂钩沿用 `archi-drawer`；关系态 `RelationInfo`，见「关系信息面板（RelationInfo）与边点击」），`CardContent` 承载 xyflow 画布（节点 = element，边 = relation，`data-testid="archi-canvas"`）。左栏三种视图的完整契约分别归 `desktop-archi-file-tree`（文件 / 元素两视图与分段切换语义）与 `desktop-archi-contract-panel`（契约视图）；本能力只登记「左栏是画布区第一栏且缺省文件树」。
- 缺省渲染 SHALL 只呈现**聚焦元素的 1-hop 邻域**（入边 + 出边 + 直接子元素），聚焦切换 SHALL 由图内交互完成；全量铺开 MUST NOT 作为缺省形态（防大模型过载）。
- 直接子元素 SHALL 渲染在聚焦节点**区域内**：聚焦元素有直接子元素时呈分组容器节点（标题栏 + 内容区），子节点以 xyflow `parentId` + `extent: 'parent'` 挂在容器下、逐行排在内容区内；邻域入 / 出边节点分布在容器左右两侧之外，MUST NOT 落进容器区域。
- 入边列（`dependedBy`：依赖当前元素的节点）SHALL 以**虚线分区**框选：分区完整包住整列入边节点，作为画布背景装饰——MUST NOT 计入节点集、MUST NOT 响应点击 / 聚焦；无入边时不渲分区。
- 测试挂钩 SHALL 用 data-testid，MUST NOT 以样式类名作查询挂钩（沿 `desktop-standard-components` 既有纪律）。

#### Scenario: 两处挂载点共用同一组件

- **WHEN** 分别打开 `/architecture` 与某 active change 详情的「架构」页签
- **THEN** 两处呈现同一组件（可调宽左栏面板 / 右栏元素信息与画布齐备，左栏分段控件三视图齐备）；A 的 `root` 为主仓根，B 的 `root` 为该 change 的 `detail.worktree`；两处左栏缺省均为文件树

#### Scenario: 归档 change 回落主仓根

- **WHEN** 打开一个已归档 change 的详情页并切到「架构」页签
- **THEN** 取数以主仓根发起（与归档 change 会话 `cwd` 恒回落同口径），MUST NOT 以已清理的 worktree 路径取数

#### Scenario: 三触发取数与零监听

- **WHEN** 审查画布取数调用点，并观察运行期无任何目录变化
- **THEN** 取数只在挂载 / `root` 变化 / 显式刷新三处发起；源码无 watch 接入、无定时轮询；磁盘外改模型时画布不自动变化（须用户刷新）

#### Scenario: 按需查询的唯一开窗

- **WHEN** 审查右栏两态面板的取数来源
- **THEN** 除三触发外，唯一额外 invoke 是「用户点边 / 点重试 → `archi_relation`」；无第四类触发（无选中态自动重取、无后台刷新）；把「点边可查」写进 spec 不改变其余三触发纪律

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

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/views/architecture/components/architecture-canvas.tsx` | 共享画布（加边点击 + 面板两态） | `onEdgeClick` 置 `selectedEdge = { from, to }`；`onNodeClick` 置 `focus` 并清 `selectedEdge`；`CardHeader` 按 `selectedEdge` 两态切换（`ModelInfo` ⇄ `RelationInfo`）；`root` / 显式刷新 / 停摆态时清 `selectedEdge`；`toEdges` / `toNodes` / `marks` / `drift-marks` 逐字不变（`id` 仍 `<from>-><to>`） |
| `src/views/architecture/components/relation-info.tsx`（新） | 关系信息面板 | 七段渲染 + 四空态（各有 testid）+ 行内错误与重试；`apis` 为空时不触发取数（hook 入参为 `null`）；两桶文案并列两种成因；testid 根 `archi-relation-info` |
| `src/views/architecture/hooks/use-archi-relation.ts`（新） | 按需取数 | `(root, edge) -> { detail, loading, error, retry }`：无根 / 无选中 / `apis` 为空 → 不取数；同一元素对重复选中零重复 invoke；失败保留 `error` 供重试；零 watch 零轮询零 `setInterval` |
| `src/views/architecture/components/model-info.tsx` | 元素态面板（零改动） | 概览 / 契约段 / 入口文档 / 改档入口逐字不变；两态切换由父级决定，面板本身零感知 |
| `src/views/architecture/lib/marks.ts` / `lib/drift-marks.ts` | 标记面（零改动） | `EdgeMark` 四值样式与报告 → 标记派生逐字不变；本变更不引入第五值、不派生节点标记 |
| `src/views/architecture/hooks/use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts` | 既有取数 | 三触发纪律与返回形状零改动（新 hook 与它们并列，不共享缓存） |

### Component: `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| `CanvasBody` 右栏 | 改 | `CardHeader` 两态分支：`selectedEdge === null` → 既有 `ModelInfo`；否则 `RelationInfo`（收 `root` / `edge` / `relation`（图里的 `ArchiRelation`，供 `apis` 与标记）/ `onRetry`）；`CardContent` 零改动 |
| 交互 | 改 | `ReactFlow` 增 `onEdgeClick`；`onNodeClick` 追加清选中；边选中态以 `style` 高亮（只用既有 stroke 变量，不新增标记词汇） |
| 渲染与标记面 | 零改动 | `toEdges` / `toNodes` / `neighborhoodPositions` / 入边分区 / 停摆分支逐字不变 |

### Component: 零改动面

| 项 | 描述 |
|---|---|
| `model-info.tsx` / `element-tree.tsx` / `file-tree.tsx` / `contract-panel.tsx` / `structure-error-panel.tsx` | 渲染、交互与 testid 逐字不变 |
| `hooks/use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts` / `use-archi-api-check.ts` | 三触发纪律与返回形状零改动 |
