# desktop-archi-canvas Specification

## Purpose

定义架构画布的渲染契约：单一共享组件在两处挂载（`/architecture` 全量只读 / change 详情「架构」页签增量可改档）、结构错的停摆态（全或无）、漂移标记呈现契约（只认标记不认判定，标记由漂移报告经取数侧纯函数派生）、`full` 与 `incremental` 的能力差异、元素信息面板与本期边界。判定本身不在本能力：扫描 / 定级 / 报告结构见 `desktop-archi-drift`。

## Requirements

### Requirement: 单一画布组件与两处挂载点

架构画布 SHALL 为**一个共享组件**（`ArchitectureCanvas`，入参 `{ root, mode }`），在仓库内两处挂载，MUST NOT 为两处各写一套渲染：

| 挂载点 | 位置 | `root` | `mode` |
|---|---|---|---|
| A | `/architecture` 页面（侧栏「架构」入口） | 当前 workspace 主仓根 | `full` |
| B | change 详情页「架构」页签 | `detail.worktree`；归档 change 回落主仓根 | `incremental` |

- **观察根就是当前工作根**：命令签名 SHALL 一律显式带 `root`，MUST NOT 引入「当前选中 change」一类隐式上下文。change 侧的 `root` 派生口径 SHALL 与既有会话 `cwd` 口径同源（`detail.worktree`；归档 change 恒回落主仓根）。
- **显式取数**：画布取数 SHALL 只在三种触发下发起——组件挂载、`root` 变化、用户显式刷新；MUST NOT 引入目录监听（watch）或轮询（沿 `desktop-file-watch` 既有红线，本变更不得以任何形式偷加自动刷新）。
- **按需查询不计入三触发（本变更显式开窗）**：边点击取关系详情（`archi_relation`，见「关系信息面板（RelationInfo）与边点击」）由**显式用户动作**发起，既非 watch 也非轮询，**不计入上述三触发**；其触发面 SHALL 恰为「选中边变化（且该元素对有 `apis`）」与「用户点重试」。除此之外 MUST NOT 有任何自动重取（含选中态保持期间的任何重取、`root` 未变时的后台刷新）。本条是本 requirement 三触发纪律的唯一开窗，MUST NOT 被援引为引入 watch / 轮询 / 定时刷新的先例。
- 画布区块 SHALL 为**两栏横向可调宽布局**（`ResizablePanelGroup`）：**左栏**为 `ResizablePanel`（`defaultSize={300}` / `minSize={200}`；面板内 base-ui `Tabs` 在**一个栏位内三种视图**间切换——缺省**元素树**（层级由 FQN 前缀派生）/ **文件树**（磁盘枚举 + 元素根归属标记）/ **契约视图**（生成物差异 + 契约判据两轴，见 `desktop-archi-contract-panel`）；三视图共用同一面板，栏宽可拖拽、MUST NOT 随视图切换变化）；**右栏**为 `ResizablePanel` 内的 `Card`——`CardHeader` 承载**两态面板**（元素态 `ModelInfo`，测试挂钩沿用 `archi-drawer`；关系态 `RelationInfo`，见「关系信息面板（RelationInfo）与边点击」），`CardContent` 承载 xyflow 画布（节点 = element，边 = relation，`data-testid="archi-canvas"`）。左栏三种视图的完整契约分别归 `desktop-archi-file-tree`（文件 / 元素两视图、分段切换语义与缺省视图）与 `desktop-archi-contract-panel`（契约视图）；本能力只登记「左栏是画布区第一栏、其缺省视图为元素树且文件树不参与首屏」。
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

### Requirement: 结构错的停摆态（全或无）

模型有**任何**结构错时，画布 SHALL 停摆：不渲染残缺图、不渲染左栏树（文件树与元素树均不挂载）、不降级样式、不留幽灵位置；图形区域 SHALL 改为显示错误面板，逐条列出结构错（`kind` + `message` + `file:line`）并提示「修好后再看」。停摆态下画布数据 SHALL 为零图数据（`elements` / `relations` 皆空），MUST NOT 以「残缺图 + 高亮」替代。

三条策略各管一段且 MUST NOT 互相替代：**读取**照常加载并返回结构化诊断（绝不 panic）；**渲染**停摆（绝不画畸形图）；**门禁**判结构错（归 `desktop-archi-gate`）。

无结构错时 SHALL 渲染完整图，入口文档面的判据（`apiFindings` 内的 `docMissing` / `docNotFound` / `apiDocMissing`）MUST NOT 触发停摆。

#### Scenario: 有结构错即停摆

- **WHEN** 模型含一个环（或未定义引用 / 孤儿层级 / 缺必填字段等任一结构错）时请求画布数据
- **THEN** 返回停摆态：`elements` 与 `relations` 均为空，错误面板逐条呈现诊断（含 `file:line`）；画布零节点零边，左栏树零节点（文件树与元素树均不挂载）

#### Scenario: 无结构错渲染完整图

- **WHEN** 模型零结构错（可含 `apiFindings` 内的入口文档判据）
- **THEN** 返回完整图数据，左栏树与画布正常渲染，警告不改变渲染态

#### Scenario: 停摆态不给部分形状

- **WHEN** 审查停摆态的渲染产物
- **THEN** 不存在「部分节点 + 高亮」「悬空边丢弃」「缺字段节点灰化」等任意降级形态（零降级样式类、零占位节点）

### Requirement: 漂移标记呈现契约（只认标记，不认判定）

画布 SHALL 接受一组**标记**并按标记渲染，标记的**产生**（何时算漂移、如何判定）MUST NOT 在本能力定义。**可呈现**的标记词汇 SHALL 为封闭集（边）：

```
边标记   none         实线（无标记）
         unfulfilled  灰虚线（声明未兑现）
         crossKind    橙虚线（跨 kind 向上，提示价值而非约束）
         exempt       灰实线（豁免；元素信息面板内呈现 reason）
```

- **节点标记不呈现**：节点标记词汇（`none` / `unfulfilled`）仍由图元素 DTO 的 `mark` 字段承载并纳入 `desktop-archi-model` 的词汇对拍，但本能力 MUST NOT 呈现节点级标记——画布节点与元素树节点只呈短名，无状态点 / 无节点标记样式分支。
- **左栏文件树的归属标记不属本能力**：左栏文件树节点的 `FileTreeMark` 五值（`elementRoot` / `ambiguous` / `member` / `containsElements` / `none`）SHALL NOT 与本条目的节点标记词汇混同——其定义、渲染与优先级归 `desktop-archi-file-tree`；本能力 MUST NOT 借文件树呈现漂移违例、MUST NOT 把 `unfulfilled` 映射到 `FileTreeMark` 任一类。
- **结构错不产生标记**：结构错一律以停摆态替代（见上一条），边级与节点级「结构错」标记 MUST NOT 存在——否则等于承认「残缺图 + 红点」这一被否形态。
- **未知标记值** SHALL 退化为中性样式（`none`）并 MUST NOT 崩溃；词汇扩展 SHALL 走「改本 requirement + 同步词汇对拍语料」两处，MUST NOT 静默新增。
- **标记生产源（本变更解锁）**：标记 SHALL 由漂移报告派生——检查报告（`archi_drift` 的 `observations` 与 `suppressed` 段，见 desktop-archi-drift「报告结构与 status 五值」）经**取数侧纯函数**映射为**边**标记：`observations[].kind = unfulfilledDeclaration` 的 `(from, to)` → 该边 `unfulfilled`；`suppressed[]` 的 `(from, to)` → 该边 `exempt`（reason 随元素信息面板呈现）。映射 MUST 只读报告、MUST NOT 重算归属或判定（判定在 `desktop-archi-drift`）；MUST NOT 派生节点标记——节点标记不呈现（见上一条），画布节点与元素树只呈短名。`marks.ts` 的边四值样式映射与词汇契约零改动（`unfulfilled` / `crossKind` / `exempt` / `none` 四值不变）。两挂载点 SHALL 同源取数、按模式分流：挂载点 A（`/architecture`）取 `full`，挂载点 B（change 详情）取 `incremental`（观察根为 `detail.worktree`，归档 change 回落主仓根）——同一纯函数映射，MUST NOT 出现第二套标记派生。

#### Scenario: 标记驱动边样式

- **WHEN** 图数据中某边标记为 `unfulfilled`、另一条为 `crossKind`、另一条为 `exempt`
- **THEN** 三条分别呈灰虚线 / 橙虚线 / 灰实线；`exempt` 边在元素信息面板内呈现 reason 文本；无标记（`none`）边呈实线

#### Scenario: 未知标记退化不崩

- **WHEN** 图数据中出现词汇外的边标记值
- **THEN** 该边按中性样式渲染，画布不崩溃、不空白

#### Scenario: 报告驱动标记

- **WHEN** 检查报告含一条 `observations[].kind = unfulfilledDeclaration`（`App.Billing` → `App.Auth`）与一条 `suppressed[]`（`App.Billing` → `App.Legacy`，reason 在位）
- **THEN** 对应两条边分别呈灰虚线（`unfulfilled`）与灰实线（`exempt`，元素信息面板内呈现 reason）；报告未提及的边保持 `none`

#### Scenario: 零报告或零漂移零标记

- **WHEN** 检查报告状态为 `clean` / `skipped` / `noChanges`，或取数失败
- **THEN** 全部边为 `none`（实线），画布不空白、不报错、不重复取数；取数失败另呈一行内提示（图照常渲染）

#### Scenario: 标记生产不派生节点标记

- **WHEN** 检查报告含一条 `observations[].kind = unfulfilledDeclaration`
- **THEN** 该边呈灰虚线，而 `from` 节点与元素树对应条目只呈短名、无状态点——节点标记词汇仅由 DTO 的 `mark` 字段承载并纳入词汇对拍，本能力 MUST NOT 呈现节点级标记

### Requirement: 能力差异（全量只读 / 增量可改档）

`mode = full` 与 `mode = incremental` 的能力差异 SHALL 是**数据语义差异**而非 UI 差异：

| 能力 | `full`（workspace 级） | `incremental`（change 级） |
|---|---|---|
| 结构校验 / 图渲染 / 入口文档段 | ✅ | ✅ |
| 改 `gate` 档 | ❌ 只读视图，呈现「改档需要开一个 change」提示 | ✅ 写 `detail.worktree` 内模型，随该 change 归档合回 |
| 生成契约生成物（写盘） | ❌ 只显示三类差异 + 呈现「生成需开 change（或让左侧会话执行本仓的生成命令）」提示 | ✅ 二次确认后写 `detail.worktree` 内的生成物，随该 change 归档合回 |

- `full` 模式 MUST NOT 呈现任何可提交的 `gate` 编辑入口（按钮 / 下拉 / 行内编辑），MUST NOT 发起写命令；同理 MUST NOT 呈现任何可提交的**生成物生成**入口，MUST NOT 发起 `archi_api_generate`。
- `incremental` 模式的改档 SHALL 经**一个显式写命令**（服务端写前校验 + 渲染自证 + 原子写回）完成，MUST NOT 由前端直接拼写 / 重写 TOML 文本。
- 改档成功 SHALL 触发一次显式重取（图与元素信息面板同步刷新）；失败 SHALL 以行内错误呈现且磁盘零变化。
- `incremental` 模式的生成 SHALL 经**一个显式写命令**（`archi_api_generate`，只增只改、从不删文件）并在二次确认后发起（交互细节归 `desktop-archi-contract-panel`）；成功后 SHALL 重跑一次检查，失败 SHALL 以行内错误如实呈现（写盘非原子）。
- `incremental` 模式 MUST NOT 提供「新增 / 删除元素」「连边」一类结构编辑入口（本期只开放 `gate` 单字段写入与生成物写盘）。
- **只读约束的对象 = 画布的人类可提交入口**：`full` 模式的只读裁定 MUST NOT 被解读为「workspace 级模型不可被 agent 修改」。`/architecture` 页可同时挂载通用 agent 会话（`desktop-archi-chat`），该会话在主仓根直改模型属**会话能力**，不是画布能力；两者并存：MUST NOT 以「会话能写」为理由为画布新增任何写入口或写命令，亦 MUST NOT 以「画布只读」为理由禁止该会话写。会话写盘后画布不自动更新（零 watch 零轮询的必然推论），用户点刷新即可见新图或停摆态诊断。该裁定 SHALL 同样覆盖契约视图的生成按钮——它也是「画布的人类可提交入口」，故 `full` 模式无生成入口；`full` 模式下想清理过期生成物，其合法路径是让页内会话执行本仓的生成命令（或开一个 change）。

#### Scenario: 全量模式零写入口

- **WHEN** 在 `/architecture`（`mode = full`）审查元素信息面板与契约视图
- **THEN** 无任何可提交的 `gate` 编辑控件、无任何可提交的生成按钮，分别呈现「改档需要开一个 change」与「生成需开 change（或让左侧会话执行本仓的生成命令）」提示；审查源码零写命令调用（`archi_set_gate` / `archi_api_generate` 均不在 `full` 分支）

#### Scenario: 增量模式改档落 worktree

- **WHEN** 在 change 详情「架构」页签把 `App.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 写命令只改该 worktree 内模型文件的目标行（注释与其余内容保留），成功后图与元素信息面板显示新档位；主仓模型零变化

#### Scenario: 改档失败零写入

- **WHEN** 改档命令因元素缺席 / 值非法 / 渲染自证失败返回 `Err`
- **THEN** 元素信息面板呈现行内错误，磁盘模型字节零变化

#### Scenario: 增量模式可生成生成物

- **WHEN** 在 change 详情「架构」页签、契约视图存在三类差异时点生成并确认
- **THEN** `archi_api_generate` 写该 worktree 内生成物（主仓零变化），随后检查差异清空、面板刷新；孤儿生成物仍在盘（生成不删文件）

#### Scenario: 画布只读与页内会话写权并存

- **WHEN** 在 `/architecture` 页展开左栏通用 agent 会话并让 agent 改主仓模型（或执行本仓的生成命令）
- **THEN** 元素信息面板仍无任何可提交 `gate` 入口、契约视图无生成按钮、页面零画布写命令；模型或生成物改动由会话写入，画布与面板在用户点刷新前保持旧数据，刷新后呈现新图 / 新差异（两条能力互不代理）

### Requirement: 元素信息面板内容与本期边界

元素信息面板（`ModelInfo`，位于右栏 `CardHeader`；测试挂钩沿用 `data-testid="archi-drawer"`）SHALL 呈现四类信息：① 元素概览（FQN / 摘要 / `kind` 与 `gate` 徽标 / 源码路径逐条，无源码时呈「（未配置）」占位；`modelFile` / `modelLine` 仅数据面，MUST NOT 在概览呈现）；② **入口文档单条**（仅 `api = none` 元素且 `doc` 非空时呈现，值为 `ArchiModuleDetail.doc` 的路径串，经 `FileViewer` + 通用 `read_file` 打开；`api = declared` 元素不渲本段——其入口文档由契约段的 `ContractPathRow("契约", api.contractPath)` 承担，**MUST NOT 出现两条入口文档**）；③ 契约段（`api.status` + 三路径 + 本元素判据，见 `desktop-archi-contract-panel`「元素信息面板契约段」）；④ `incremental` 模式下 `gate` 的编辑入口。

以下内容 MUST NOT 在本能力出现（防"半成品字段"；漂移判定的消费面止于边 / 节点标记，面板不承载报告细节）：

- 「相邻报告」区块（最近一次检查的 errors / warnings）——本期面板零该区块、零占位框；
- 漂移统计 / 门禁结论 / 豁免 reason 编辑入口；
- **`docs` 关联列表与其中的 ADR 条目**——`docs` 三源关联（含 `kind = "adr"` 的 `text` 源）已随 `desktop-archi-module-docs` 整体退役；面板 MUST NOT 保留文档列表位、MUST NOT 保留「按强度序」排序语义、MUST NOT 保留 ADR 正文按需展开的交互（ADR 改经入口文档内链接 / 左栏文件树 / 通用文件搜索抵达）。

`doc` 的路径取数 SHALL 与契约路径同一通路（通用 `read_file`，绝对路径入参）：本能力 MUST NOT 新增专用文档读取命令、MUST NOT 使该通路成为任意文件读取面的**新**风险点——`read_file` 本来就是通用绝对路径读面，本变更只是把「点开一个模型声明的路径」这一个动作接到它上面。

#### Scenario: 概览呈现元素事实

- **WHEN** 打开某元素信息面板
- **THEN** 呈现 FQN、摘要（`null` 时不占文案）、`kind` / `gate` 徽标与源码路径逐条；`path` 为空时呈「（未配置）」；声明位置（`modelFile:line`）MUST NOT 在概览呈现（仅经 `archi_module` 数据面供 agent 定位）

#### Scenario: api = none 元素的入口文档单条

- **WHEN** 聚焦一个 `api = none` 且 `doc` 非空的元素打开元素信息面板
- **THEN** 面板内呈一条「入口文档」行，点「查看」经 `read_file` 打开该路径；同一面板内零文档列表、零 ADR 条目

#### Scenario: declared 元素只有契约段一条入口文档

- **WHEN** 聚焦契约在案的元素（如 `Backend.Infra.ArchiRuntime`）打开元素信息面板
- **THEN** 入口文档只出现在契约段的 `ContractPathRow("契约")` 一行；面板内 MUST NOT 出现第二条入口文档行（`doc` 为 `null`）；段序为「概览 → 契约 → 入口文档 → 豁免」，`declared` 与 `api = none` 未声明 `doc` 时「入口文档」段整段不渲染

#### Scenario: 空态可辨

- **WHEN** 聚焦 `api = none` 且未声明 `doc` 的元素
- **THEN** 「入口文档」段呈一条空态文案（「无入口文档」，testid 随字段面更名）；不报错、不显示失效路径、不产生额外 invoke

#### Scenario: 本期无相邻报告区块

- **WHEN** 审查元素信息面板渲染产物
- **THEN** 无「相邻报告」区块、无 `errors` / `warnings` 计数字段、无空占位框

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/views/architecture/components/architecture-canvas.tsx` | 共享画布 + 标记接线点（两挂载点共用） | 入参 `{ root, mode: 'full' \| 'incremental' }`；两栏可调宽布局（**左栏 `ResizablePanel`（`treeMode` 初值 `'element'`，可切文件树 / 契约视图）** / 右栏 `Card`：`CardHeader` 元素信息 `ModelInfo` + `CardContent` xyflow 画布）；1-hop 缺省；两态切换（正常态 / 停摆态）；`graph = driftMarks(report, graph)` 后交渲染，漂移取数失败降级为「零标记 + 行内提示」且 MUST NOT 阻断图渲染；`mode` 直接决定 `archi_drift` 的调用模式；data-testid 挂钩。左栏为 base-ui `Tabs`（`TabsList` 携 `data-testid="archi-tree-mode"`，三枚 `TabsTrigger` 声明序**元素 / 文件 / 契约**，`options` 与 `TabsTrigger` 同源）切换 `ElementTree` / `FileTree`（`desktop-archi-file-tree`）/ `ContractPanel`（`desktop-archi-contract-panel`），左栏 `defaultSize={300}` / `minSize={200}`，切换 MUST NOT 改变栏宽或触发图 / 元素信息面板重取数；缺省渲元素树 ⇒ `FileTree` 不挂载、`archi_file_tree` 零调用。画布节点视图取自 `components/graph/model-node.tsx` 的 `ModelNode`（叶子短名徽章）与 `components/graph/model-group-node.tsx` 的 `ModelGroupNode`（分组容器：聚焦元素有直接子元素时，子节点以 `parentId` 渲染在其区域内）；入边列以 `ViewportPortal` 虚线分区框选（`data-testid="archi-canvas-zone"`，背景装饰不入节点集） |
| `src/views/architecture/lib/drift-marks.ts` | 报告 → 边标记纯函数 | `driftMarks(report, graph) -> ArchiGraph`：只读 `observations` / `suppressed` 两段、按 `(from, to)` 命中边；MUST NOT 派生节点标记（节点标记不呈现）；零判定、零 IO、无渲染依赖 |
| `src/views/architecture/hooks/use-archi-drift.ts` | 漂移取数 | 经 `archi_drift(root, mode)` 显式取数（挂载 / 根切换 / 手动刷新三触发，零 watch 零轮询）；`mode` 由挂载点给定（A=`full` / B=`incremental`）；取数失败同时清空 `report` 与已载键（旧报告 MUST NOT 冒充本次结论）；返回 `report` / `loading` / `error` |
| `src/views/architecture/components/element-tree.tsx` | 左栏元素视图（缺省视图） | 层级由 FQN 前缀派生；节点只呈短名（节点标记不呈现，叶子留禁用的展开空位）；停摆态零节点；testid `archi-element-tree` / `archi-tree-node` 不变（入参 / 渲染零改动，消费画布已取的 `elements` ⇒ 零新增 invoke） |
| `src/views/architecture/components/file-tree.tsx` | 左栏文件树（第二视图） | 数据面 / 标记 / 点击语义 / 双向联动见 `desktop-archi-file-tree`；本能力只要求「缺省不挂载故不取数、点击走同一次 `onFocus`」 |
| `src/views/architecture/hooks/use-archi-file-tree.ts` | 文件树取数 | 三触发（挂载 / `root` 变化 / 上层 `key` 重挂）、零 watch 零轮询；失败只落左栏行内错误、MUST NOT 阻断画布 |
| `src/views/architecture/components/structure-error-panel.tsx` | 停摆态 | 逐条诊断（`kind` + `message` + `file:line`）+「修好后再看」；零图数据、零降级样式 |
| `src/views/architecture/components/model-info.tsx` | 元素信息面板（右栏 `CardHeader`） | 导出 `ModelInfo`；概览（FQN / 摘要 / `kind` / `gate` 徽标 / 源码路径逐条，无 path 时「（未配置）」）+ 契约段 + **入口文档单条**（仅 `api = none` 且 `doc` 非空；空态文案随字段面更名）+ `incremental` 下 `gate` 编辑入口；`DocsSection` / `DocRow` 删除；本期零「相邻报告」区块 |
| `src/views/architecture/lib/marks.ts` | 标记 → 样式映射（边） | 边标记四值（`none` 实线 / `unfulfilled` 灰虚线 / `crossKind` 橙虚线 / `exempt` 灰实线）；未知值退化为中性；封闭词汇（节点标记不做视觉映射）；标记生产由 `drift-marks.ts` 承担，本件 MUST NOT 参与判定 |
| `src/views/architecture/hooks/use-archi-graph.ts` | 画布取数 | 挂载 / `root` 变化 / 手动刷新三触发；零 watch 零轮询；停摆态与正常态两形状透出 |
| `src/views/architecture/hooks/use-archi-module.ts` | 元素信息面板取数 | 仍单次 `archi_module`；载荷的 `docs` / `warnings` 位次由 `doc` 取代（取数逻辑与触发条件零改动）；入口文档正文由 `components/file` 的 `FileViewer` 经通用 `read_file` 自取 |
| `src/views/architecture/architecture-view.tsx` | 挂载点 A | `/architecture` 全量只读；显式刷新入口 |
| `src/views/changes/change-detail-view.tsx`（架构页签） | 挂载点 B | 以 `detail.worktree` 为根、归档回落主仓根；`incremental` 可改档；页签切换不重复取数 |
| `src/routes.tsx` + `src/components/app-sidebar.tsx` | 入口 | `/architecture` 路由 + 页面组「架构」项（`data-testid="nav-architecture"`，active 由 URL 派生） |

### Component: `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| `CanvasBody` 左栏 | 改 | 左栏为 `ResizablePanel` 内的 base-ui `Tabs` 三视图分支（`value` 受控于 `treeMode`，初值 `'element'`；三枚 `TabsTrigger` 声明序 元素 / 文件 / 契约）：`value="element"` → `ElementTree`（缺省，不挂载其余两视图）、`value="file"` → `FileTree`（收 `root` 自取数）、`value="contract"` → `ContractPanel`；三视图共用同一面板（`defaultSize={300}` / `minSize={200}`，Tabs 以 `w-full h-full` 铺满）；`ElementTree` / `ContractPanel` 的入参与 testid 零改动 |
| 数据传递 | 改 | `report.apiFindings` 与按元素过滤结果传给契约面板与元素信息面板；`mode` 决定生成按钮可用性；零新增取数（判据轴复用既有 `use-archi-drift`） |
| 渲染与标记面 | 改 | `toEdges` / `defaultFocus` / 结构错停摆分支、`lib/marks.ts` / `lib/drift-marks.ts` 逐字不变；`toNodes` / `neighborhoodPositions` 按「直接子元素渲染在聚焦节点区域内」重写（子节点挂 `parentId`，聚焦节点成组时入 / 出边列让出容器）；节点视图拆为 `components/graph/model-node.tsx` 的 `ModelNode` 与 `components/graph/model-group-node.tsx` 的 `ModelGroupNode`，testid `archi-canvas-node` 不变 |

### Component: 零改动面

| 项 | 描述 |
|---|---|
| `structure-error-panel.tsx` / `element-tree.tsx` / `file-tree.tsx`（行渲染与标记面） | 行渲染、标记呈现与点击语义逐字不变；`file-tree.tsx` 的入参由 `entries` 改为 `root`（组件内联 `useArchiFileTree` 自取数），`TreeModeSwitch` 退役（分段控件移至本能力的 `Tabs`） |
| `hooks/use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts` | 三触发纪律与返回形状零改动 |

### 模块增量：元素信息面板入口文档单条换面（module-doc-field-docs-retirement）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|------|
| `packages/desktop/src/views/architecture/lib/drift-marks.ts` | 报告 → 边标记纯函数 | 零改动（只读 `observations` / `suppressed`；`apiFindings` 仍不参与节点 / 边标记） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 共享画布 | 零改动（两栏布局、左栏三视图、元素信息面板接线、`mode` 语义逐字不变） |
| `packages/desktop/src/views/architecture/components/model-info.test.tsx` | 面板语料 | `docsFixture` 删；`docs: []` fixture → `doc: null`；`archi-docs-empty` 用例改「入口文档空态」；新增两条否定断言（`declared` 元素零入口文档行 / 面板内零 `docs` 列表） |

### 模块增量：关系面板与边点击（relation-api-dependency-check）

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
