# desktop-archi-canvas Specification（delta: archi-file-tree）

## MODIFIED Requirements

### Requirement: 单一画布组件与两处挂载点

架构画布 SHALL 为**一个共享组件**（`ArchitectureCanvas`，入参 `{ root, mode }`），在仓库内两处挂载，MUST NOT 为两处各写一套渲染：

| 挂载点 | 位置 | `root` | `mode` |
|---|---|---|---|
| A | `/architecture` 页面（侧栏「架构」入口） | 当前 workspace 主仓根 | `full` |
| B | change 详情页「架构」页签 | `detail.worktree`；归档 change 回落主仓根 | `incremental` |

- **观察根就是当前工作根**：命令签名 SHALL 一律显式带 `root`，MUST NOT 引入「当前选中 change」一类隐式上下文。change 侧的 `root` 派生口径 SHALL 与既有会话 `cwd` 口径同源（`detail.worktree`；归档 change 恒回落主仓根）。
- **显式取数**：画布取数 SHALL 只在三种触发下发起——组件挂载、`root` 变化、用户显式刷新；MUST NOT 引入目录监听（watch）或轮询（沿 `desktop-file-watch` 既有红线，本变更不得以任何形式偷加自动刷新）。
- 画布区块 SHALL 为三段：**左栏树**（缺省为文件树——磁盘枚举 + 元素根归属标记；可经分段控件切到元素树——层级由 FQN 前缀派生）、中 xyflow 画布（节点 = element，边 = relation）、右抽屉（见「抽屉内容」requirement）。左栏两种视图的完整契约（命令、标记词表、归属判定、枚举面、呈现与联动）归 `desktop-archi-file-tree`；本能力只登记「三段布局的第一段是左栏树且缺省文件树」。
- 缺省渲染 SHALL 只呈现**聚焦元素的 1-hop 邻域**（入边 + 出边 + 直接子元素），聚焦切换 SHALL 由图内交互完成；全量铺开 MUST NOT 作为缺省形态（防大模型过载）。
- 测试挂钩 SHALL 用 data-testid，MUST NOT 以样式类名作查询挂钩（沿 `desktop-standard-components` 既有纪律）。

#### Scenario: 两处挂载点共用同一组件

- **WHEN** 分别打开 `/architecture` 与某 active change 详情的「架构」页签
- **THEN** 两处呈现同一组件（左栏树 / 画布 / 抽屉三段齐备）；A 的 `root` 为主仓根，B 的 `root` 为该 change 的 `detail.worktree`；两处左栏缺省均为文件树

#### Scenario: 归档 change 回落主仓根

- **WHEN** 打开一个已归档 change 的详情页并切到「架构」页签
- **THEN** 取数以主仓根发起（与归档 change 会话 `cwd` 恒回落同口径），MUST NOT 以已清理的 worktree 路径取数

#### Scenario: 三触发取数与零监听

- **WHEN** 审查画布取数调用点，并观察运行期无任何目录变化
- **THEN** 取数只在挂载 / `root` 变化 / 显式刷新三处发起；源码无 watch 接入、无定时轮询；磁盘外改模型时画布不自动变化（须用户刷新）

#### Scenario: 缺省 1-hop

- **WHEN** 模型含数十元素且无元素被显式聚焦
- **THEN** 画布只渲染聚焦元素（缺省取根元素或首个顶层元素）及其 1-hop 邻域；非邻域元素不出现在图内，全量仅经显式操作展开

### Requirement: 结构错的停摆态（全或无）

模型有**任何**结构错时，画布 SHALL 停摆：不渲染残缺图、不渲染左栏树（文件树与元素树均不挂载）、不降级样式、不留幽灵位置；图形区域 SHALL 改为显示错误面板，逐条列出结构错（`kind` + `message` + `file:line`）并提示「修好后再看」。停摆态下画布数据 SHALL 为零图数据（`elements` / `relations` 皆空），MUST NOT 以「残缺图 + 高亮」替代。

三条策略各管一段且 MUST NOT 互相替代：**读取**照常加载并返回结构化诊断（绝不 panic）；**渲染**停摆（绝不画畸形图）；**门禁**判结构错（归 `desktop-archi-gate`）。

无结构错时 SHALL 渲染完整图，`docNotFound` 一类文档警告 MUST NOT 触发停摆。

#### Scenario: 有结构错即停摆

- **WHEN** 模型含一个环（或未定义引用 / 孤儿层级 / 缺必填字段等任一结构错）时请求画布数据
- **THEN** 返回停摆态：`elements` 与 `relations` 均为空，错误面板逐条呈现诊断（含 `file:line`）；画布零节点零边，左栏树零节点（文件树与元素树均不挂载）

#### Scenario: 无结构错渲染完整图

- **WHEN** 模型零结构错（可含 `docNotFound` 警告）
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
         exempt       灰实线（豁免；抽屉内呈现 reason）
```

- **节点标记不呈现**：节点标记词汇（`none` / `unfulfilled`）仍由图元素 DTO 的 `mark` 字段承载并纳入 `desktop-archi-model` 的词汇对拍，但本能力 MUST NOT 呈现节点级标记——画布节点与元素树节点只呈短名，无状态点 / 无节点标记样式分支。
- **左栏文件树的归属标记不属本能力**：左栏文件树节点的 `FileTreeMark` 五值（`elementRoot` / `ambiguous` / `member` / `containsElements` / `none`）SHALL NOT 与本条目的节点标记词汇混同——其定义、渲染与优先级归 `desktop-archi-file-tree`；本能力 MUST NOT 借文件树呈现漂移违例、MUST NOT 把 `unfulfilled` 映射到 `FileTreeMark` 任一类。
- **结构错不产生标记**：结构错一律以停摆态替代（见上一条），边级与节点级「结构错」标记 MUST NOT 存在——否则等于承认「残缺图 + 红点」这一被否形态。
- **未知标记值** SHALL 退化为中性样式（`none`）并 MUST NOT 崩溃；词汇扩展 SHALL 走「改本 requirement + 同步词汇对拍语料」两处，MUST NOT 静默新增。
- **标记生产源（本变更解锁）**：标记 SHALL 由漂移报告派生——检查报告（`archi_drift` 的 `observations` 与 `suppressed` 段，见 desktop-archi-drift「报告结构与 status 五值」）经**取数侧纯函数**映射为**边**标记：`observations[].kind = unfulfilledDeclaration` 的 `(from, to)` → 该边 `unfulfilled`；`suppressed[]` 的 `(from, to)` → 该边 `exempt`（reason 随抽屉呈现）。映射 MUST 只读报告、MUST NOT 重算归属或判定（判定在 `desktop-archi-drift`）；MUST NOT 派生节点标记——节点标记不呈现（见上一条），画布节点与元素树只呈短名。`marks.ts` 的边四值样式映射与词汇契约零改动（`unfulfilled` / `crossKind` / `exempt` / `none` 四值不变）。两挂载点 SHALL 同源取数、按模式分流：挂载点 A（`/architecture`）取 `full`，挂载点 B（change 详情）取 `incremental`（观察根为 `detail.worktree`，归档 change 回落主仓根）——同一纯函数映射，MUST NOT 出现第二套标记派生。

#### Scenario: 标记驱动边样式

- **WHEN** 图数据中某边标记为 `unfulfilled`、另一条为 `crossKind`、另一条为 `exempt`
- **THEN** 三条分别呈灰虚线 / 橙虚线 / 灰实线；`exempt` 边在抽屉内呈现 reason 文本；无标记（`none`）边呈实线

#### Scenario: 未知标记退化不崩

- **WHEN** 图数据中出现词汇外的边标记值
- **THEN** 该边按中性样式渲染，画布不崩溃、不空白

#### Scenario: 报告驱动标记

- **WHEN** 检查报告含一条 `observations[].kind = unfulfilledDeclaration`（`App.Billing` → `App.Auth`）与一条 `suppressed[]`（`App.Billing` → `App.Legacy`，reason 在位）
- **THEN** 对应两条边分别呈灰虚线（`unfulfilled`）与灰实线（`exempt`，抽屉内呈现 reason）；报告未提及的边保持 `none`

#### Scenario: 零报告或零漂移零标记

- **WHEN** 检查报告状态为 `clean` / `skipped` / `noChanges`，或取数失败
- **THEN** 全部边为 `none`（实线），画布不空白、不报错、不重复取数；取数失败另呈一行内提示（图照常渲染）

#### Scenario: 标记生产不派生节点标记

- **WHEN** 检查报告含一条 `observations[].kind = unfulfilledDeclaration`
- **THEN** 该边呈灰虚线，而 `from` 节点与元素树对应条目只呈短名、无状态点——节点标记词汇仅由 DTO 的 `mark` 字段承载并纳入词汇对拍，本能力 MUST NOT 呈现节点级标记

## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/views/architecture/components/architecture-canvas.tsx` | 共享画布 + 标记接线点（两挂载点共用） | 入参 `{ root, mode: 'full' \| 'incremental' }`；三段布局（**左栏树（缺省文件树，可切元素树）** / xyflow 画布 / 抽屉）；1-hop 缺省；两态切换（正常态 / 停摆态）；`graph = driftMarks(report, graph)` 后交渲染，漂移取数失败降级为「零标记 + 行内提示」且 MUST NOT 阻断图渲染；`mode` 直接决定 `archi_drift` 的调用模式；data-testid 挂钩。左栏分支：分段控件 `data-testid="archi-tree-mode"` 切换 `FileTree`（`desktop-archi-file-tree`）/ `ElementTree`，切换 MUST NOT 触发图或抽屉重取数 |
| `src/views/architecture/components/element-tree.tsx` | 左栏元素视图（保留） | 层级由 FQN 前缀派生；节点只呈短名（节点标记不呈现，叶子留禁用的展开空位）；停摆态零节点；testid `archi-element-tree` / `archi-tree-node` 不变（本变更零改动） |
| `src/views/architecture/components/file-tree.tsx` | 左栏文件树（缺省视图） | 数据面 / 标记 / 点击语义 / 双向联动见 `desktop-archi-file-tree`；本能力只要求「缺省渲它、点击走同一次 `onFocus`」 |
| `src/views/architecture/hooks/use-archi-file-tree.ts` | 文件树取数 | 三触发（挂载 / `root` 变化 / 上层 `key` 重挂）、零 watch 零轮询；失败只落左栏行内错误、MUST NOT 阻断画布 |
