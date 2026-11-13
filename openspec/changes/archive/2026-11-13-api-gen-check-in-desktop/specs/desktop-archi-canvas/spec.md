# desktop-archi-canvas Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改两条 requirement，且都是**追加**：①「单一画布组件与两处挂载点」的左栏由两视图扩为三视图（文件 / 元素 / 契约，缺省仍是文件树）；②「能力差异（全量只读 / 增量可改档）」的能力差异表加一行「生成生成物」（`full` 不可用、`incremental` 可生成），并把只读约束的对象显式覆盖到契约面板的生成按钮。结构错停摆态、漂移标记呈现、抽屉内容与本期边界三条 requirement 逐字不变。

## MODIFIED Requirements

### Requirement: 单一画布组件与两处挂载点

架构画布 SHALL 为**一个共享组件**（`ArchitectureCanvas`，入参 `{ root, mode }`），在仓库内两处挂载，MUST NOT 为两处各写一套渲染：

| 挂载点 | 位置 | `root` | `mode` |
|---|---|---|---|
| A | `/architecture` 页面（侧栏「架构」入口） | 当前 workspace 主仓根 | `full` |
| B | change 详情页「架构」页签 | `detail.worktree`；归档 change 回落主仓根 | `incremental` |

- **观察根就是当前工作根**：命令签名 SHALL 一律显式带 `root`，MUST NOT 引入「当前选中 change」一类隐式上下文。change 侧的 `root` 派生口径 SHALL 与既有会话 `cwd` 口径同源（`detail.worktree`；归档 change 恒回落主仓根）。
- **显式取数**：画布取数 SHALL 只在三种触发下发起——组件挂载、`root` 变化、用户显式刷新；MUST NOT 引入目录监听（watch）或轮询（沿 `desktop-file-watch` 既有红线，本变更不得以任何形式偷加自动刷新）。
- 画布区块 SHALL 为三段：**左栏**（分段控件在**一个栏位内三种视图**间切换——缺省**文件树**（磁盘枚举 + 元素根归属标记）/ **元素树**（层级由 FQN 前缀派生）/ **契约视图**（生成物差异 + 契约判据两轴，见 `desktop-archi-contract-panel`））、中 xyflow 画布（节点 = element，边 = relation）、右抽屉（见「抽屉内容」requirement）。左栏三种视图的完整契约分别归 `desktop-archi-file-tree`（文件 / 元素两视图与分段切换语义）与 `desktop-archi-contract-panel`（契约视图）；本能力只登记「三段布局的第一段是左栏且缺省文件树」。
- 缺省渲染 SHALL 只呈现**聚焦元素的 1-hop 邻域**（入边 + 出边 + 直接子元素），聚焦切换 SHALL 由图内交互完成；全量铺开 MUST NOT 作为缺省形态（防大模型过载）。
- 测试挂钩 SHALL 用 data-testid，MUST NOT 以样式类名作查询挂钩（沿 `desktop-standard-components` 既有纪律）。

#### Scenario: 两处挂载点共用同一组件

- **WHEN** 分别打开 `/architecture` 与某 active change 详情的「架构」页签
- **THEN** 两处呈现同一组件（左栏 / 画布 / 抽屉三段齐备，左栏分段控件三视图齐备）；A 的 `root` 为主仓根，B 的 `root` 为该 change 的 `detail.worktree`；两处左栏缺省均为文件树

#### Scenario: 归档 change 回落主仓根

- **WHEN** 打开一个已归档 change 的详情页并切到「架构」页签
- **THEN** 取数以主仓根发起（与归档 change 会话 `cwd` 恒回落同口径），MUST NOT 以已清理的 worktree 路径取数

#### Scenario: 三触发取数与零监听

- **WHEN** 审查画布取数调用点，并观察运行期无任何目录变化
- **THEN** 取数只在挂载 / `root` 变化 / 显式刷新三处发起；源码无 watch 接入、无定时轮询；磁盘外改模型时画布不自动变化（须用户刷新）

#### Scenario: 缺省 1-hop

- **WHEN** 模型含数十元素且无元素被显式聚焦
- **THEN** 画布只渲染聚焦元素（缺省取根元素或首个顶层元素）及其 1-hop 邻域；非邻域元素不出现在图内，全量仅经显式操作展开

### Requirement: 能力差异（全量只读 / 增量可改档）

`mode = full` 与 `mode = incremental` 的能力差异 SHALL 是**数据语义差异**而非 UI 差异：

| 能力 | `full`（workspace 级） | `incremental`（change 级） |
|---|---|---|
| 结构校验 / 图渲染 / 文档关联 | ✅ | ✅ |
| 改 `gate` 档 | ❌ 只读视图，呈现「改档需要开一个 change」提示 | ✅ 写 `detail.worktree` 内模型，随该 change 归档合回 |
| 生成契约生成物（写盘） | ❌ 只显示三类差异 + 呈现「生成需开 change（或让左侧会话执行 `pnpm api:export`）」提示 | ✅ 二次确认后写 `detail.worktree` 内的生成物，随该 change 归档合回 |

- `full` 模式 MUST NOT 呈现任何可提交的 `gate` 编辑入口（按钮 / 下拉 / 行内编辑），MUST NOT 发起写命令；同理 MUST NOT 呈现任何可提交的**生成物生成**入口，MUST NOT 发起 `archi_api_generate`。
- `incremental` 模式的改档 SHALL 经**一个显式写命令**（服务端写前校验 + 渲染自证 + 原子写回）完成，MUST NOT 由前端直接拼写 / 重写 TOML 文本。
- 改档成功 SHALL 触发一次显式重取（图与抽屉同步刷新）；失败 SHALL 以行内错误呈现且磁盘零变化。
- `incremental` 模式的生成 SHALL 经**一个显式写命令**（`archi_api_generate`，只增只改、从不删文件）并在二次确认后发起（交互细节归 `desktop-archi-contract-panel`）；成功后 SHALL 重跑一次检查，失败 SHALL 以行内错误如实呈现（写盘非原子）。
- `incremental` 模式 MUST NOT 提供「新增 / 删除元素」「连边」一类结构编辑入口（本期只开放 `gate` 单字段写入与生成物写盘）。
- **只读约束的对象 = 画布的人类可提交入口**：`full` 模式的只读裁定 MUST NOT 被解读为「workspace 级模型不可被 agent 修改」。`/architecture` 页可同时挂载通用 agent 会话（`desktop-archi-chat`），该会话在主仓根直改模型属**会话能力**，不是画布能力；两者并存：MUST NOT 以「会话能写」为理由为画布新增任何写入口或写命令，亦 MUST NOT 以「画布只读」为理由禁止该会话写。会话写盘后画布不自动更新（零 watch 零轮询的必然推论），用户点刷新即可见新图或停摆态诊断。该裁定 SHALL 同样覆盖契约视图的生成按钮——它也是「画布的人类可提交入口」，故 `full` 模式无生成入口；`full` 模式下想清理过期生成物，其合法路径是让页内会话执行 `pnpm api:export`（或开一个 change）。

#### Scenario: 全量模式零写入口

- **WHEN** 在 `/architecture`（`mode = full`）审查元素抽屉与契约视图
- **THEN** 无任何可提交的 `gate` 编辑控件、无任何可提交的生成按钮，分别呈现「改档需要开一个 change」与「生成需开 change（或让左侧会话执行 `pnpm api:export`）」提示；审查源码零写命令调用（`archi_set_gate` / `archi_api_generate` 均不在 `full` 分支）

#### Scenario: 增量模式改档落 worktree

- **WHEN** 在 change 详情「架构」页签把 `App.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 写命令只改该 worktree 内模型文件的目标行（注释与其余内容保留），成功后图与抽屉显示新档位；主仓模型零变化

#### Scenario: 改档失败零写入

- **WHEN** 改档命令因元素缺席 / 值非法 / 渲染自证失败返回 `Err`
- **THEN** 抽屉呈现行内错误，磁盘模型字节零变化

#### Scenario: 增量模式可生成生成物

- **WHEN** 在 change 详情「架构」页签、契约视图存在三类差异时点生成并确认
- **THEN** `archi_api_generate` 写该 worktree 内生成物（主仓零变化），随后检查差异清空、面板刷新；孤儿生成物仍在盘（生成不删文件）

#### Scenario: 画布只读与页内会话写权并存

- **WHEN** 在 `/architecture` 页展开左栏通用 agent 会话并让 agent 改主仓模型（或执行 `pnpm api:export`）
- **THEN** 抽屉仍无任何可提交 `gate` 入口、契约视图无生成按钮、页面零画布写命令；模型或生成物改动由会话写入，画布与面板在用户点刷新前保持旧数据，刷新后呈现新图 / 新差异（两条能力互不代理）

## Module Contract

### Component: `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| `CanvasBody` 左栏 | 改 | 分段控件三视图分支：`file` → `FileTree`、`element` → `ElementTree`、`contract` → `ContractPanel`（w-80）；文件 / 元素两臂的入参、testid 与行为零改动 |
| 数据传递 | 改 | `report.apiFindings` 与按元素过滤结果传给契约面板与抽屉；`mode` 决定生成按钮可用性；零新增取数（判据轴复用既有 `use-archi-drift`） |
| 渲染与标记面 | 零改动 | `toNodes` / `toEdges` / `neighborhoodPositions` / `defaultFocus` / `ArchiNodeView` / 结构错停摆分支、`lib/marks.ts` / `lib/drift-marks.ts` 逐字不变 |

### Component: 零改动面

| 项 | 描述 |
|---|---|
| `structure-error-panel.tsx` / `element-tree.tsx` / `file-tree.tsx`（文件视图渲染） | 逐字不变；`file-tree.tsx` 只在 `TreeMode` 上加一个值 |
| `hooks/use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts` | 三触发纪律与返回形状零改动 |
