# desktop-archi-canvas Specification

## Purpose

定义架构画布的渲染契约：单一共享组件在两处挂载（`/architecture` 全量只读 / change 详情「架构」页签增量可改档）、结构错的停摆态（全或无）、漂移标记呈现契约（只认标记不认判定）、`full` 与 `incremental` 的能力差异、抽屉内容与本期边界。

## Requirements

### Requirement: 单一画布组件与两处挂载点

架构画布 SHALL 为**一个共享组件**（`ArchitectureCanvas`，入参 `{ root, mode }`），在仓库内两处挂载，MUST NOT 为两处各写一套渲染：

| 挂载点 | 位置 | `root` | `mode` |
|---|---|---|---|
| A | `/architecture` 页面（侧栏「架构」入口） | 当前 workspace 主仓根 | `full` |
| B | change 详情页「架构」页签 | `detail.worktree`；归档 change 回落主仓根 | `incremental` |

- **观察根就是当前工作根**：命令签名 SHALL 一律显式带 `root`，MUST NOT 引入「当前选中 change」一类隐式上下文。change 侧的 `root` 派生口径 SHALL 与既有会话 `cwd` 口径同源（`detail.worktree`；归档 change 恒回落主仓根）。
- **显式取数**：画布取数 SHALL 只在三种触发下发起——组件挂载、`root` 变化、用户显式刷新；MUST NOT 引入目录监听（watch）或轮询（沿 `desktop-file-watch` 既有红线，本变更不得以任何形式偷加自动刷新）。
- 画布区块 SHALL 为三段：左元素树（层级由 FQN 前缀派生）、中 xyflow 画布（节点 = element，边 = relation）、右抽屉（见「抽屉内容」requirement）。
- 缺省渲染 SHALL 只呈现**聚焦元素的 1-hop 邻域**（入边 + 出边 + 直接子元素），聚焦切换 SHALL 由图内交互完成；全量铺开 MUST NOT 作为缺省形态（防大模型过载）。
- 测试挂钩 SHALL 用 data-testid，MUST NOT 以样式类名作查询挂钩（沿 `desktop-standard-components` 既有纪律）。

#### Scenario: 两处挂载点共用同一组件

- **WHEN** 分别打开 `/architecture` 与某 active change 详情的「架构」页签
- **THEN** 两处呈现同一组件（元素树 / 画布 / 抽屉三段齐备）；A 的 `root` 为主仓根，B 的 `root` 为该 change 的 `detail.worktree`

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

模型有**任何**结构错时，画布 SHALL 停摆：不渲染残缺图、不渲染元素树节点、不降级样式、不留幽灵位置；图形区域 SHALL 改为显示错误面板，逐条列出结构错（`kind` + `message` + `file:line`）并提示「修好后再看」。停摆态下画布数据 SHALL 为零图数据（`elements` / `relations` 皆空），MUST NOT 以「残缺图 + 高亮」替代。

三条策略各管一段且 MUST NOT 互相替代：**读取**照常加载并返回结构化诊断（绝不 panic）；**渲染**停摆（绝不画畸形图）；**门禁**判结构错（归 `archi-check-and-gate`）。

无结构错时 SHALL 渲染完整图，`docNotFound` 一类文档警告 MUST NOT 触发停摆。

#### Scenario: 有结构错即停摆

- **WHEN** 模型含一个环（或未定义引用 / 孤儿层级 / 缺必填字段等任一结构错）时请求画布数据
- **THEN** 返回停摆态：`elements` 与 `relations` 均为空，错误面板逐条呈现诊断（含 `file:line`）；画布零节点零边，元素树零节点

#### Scenario: 无结构错渲染完整图

- **WHEN** 模型零结构错（可含 `docNotFound` 警告）
- **THEN** 返回完整图数据，元素树与画布正常渲染，警告不改变渲染态

#### Scenario: 停摆态不给部分形状

- **WHEN** 审查停摆态的渲染产物
- **THEN** 不存在「部分节点 + 高亮」「悬空边丢弃」「缺字段节点灰化」等任意降级形态（零降级样式类、零占位节点）

### Requirement: 漂移标记呈现契约（只认标记，不认判定）

画布 SHALL 接受一组**标记**并按标记渲染，标记的**产生**（何时算漂移、如何判定）MUST NOT 在本能力定义。标记词汇 SHALL 为封闭集：

```
边标记   none         实线（无标记；本变更恒此值）
         unfulfilled  灰虚线（声明未兑现）
         crossKind    橙虚线（跨 kind 向上，提示价值而非约束）
         exempt       灰实线（豁免；抽屉内呈现 reason）
节点标记 none         ⚪ 正常
         unfulfilled  🟡 存在未兑现声明
```

- **结构错不产生节点标记**：结构错一律以停摆态替代（见上一条），节点级「结构错」标记 MUST NOT 存在——否则等于承认「残缺图 + 红点」这一被否形态。
- **未知标记值** SHALL 退化为中性样式（`none`）并 MUST NOT 崩溃；词汇扩展 SHALL 走「改本 requirement + 同步词汇对拍语料」两处，MUST NOT 静默新增。
- 本期标记生产者为空（恒 `none`），渲染契约与词汇先立：check 侧接入时 UI 零改动。

#### Scenario: 标记驱动边样式

- **WHEN** 图数据中某边标记为 `unfulfilled`、另一条为 `crossKind`、另一条为 `exempt`
- **THEN** 三条分别呈灰虚线 / 橙虚线 / 灰实线；`exempt` 边在抽屉内呈现 reason 文本；无标记（`none`）边呈实线

#### Scenario: 未知标记退化不崩

- **WHEN** 图数据中出现词汇外的标记值
- **THEN** 该边 / 节点按中性样式渲染，画布不崩溃、不空白

#### Scenario: 本期零标记

- **WHEN** 以本变更产出的图数据渲染画布
- **THEN** 全部边为 `none`（实线）、全部节点为 `none`（⚪ 正常）——标记生产不在本期射程

### Requirement: 能力差异（全量只读 / 增量可改档）

`mode = full` 与 `mode = incremental` 的能力差异 SHALL 是**数据语义差异**而非 UI 差异：

| 能力 | `full`（workspace 级） | `incremental`（change 级） |
|---|---|---|
| 结构校验 / 图渲染 / 文档关联 | ✅ | ✅ |
| 改 `gate` 档 | ❌ 只读视图，呈现「改档需要开一个 change」提示 | ✅ 写 `detail.worktree` 内模型，随该 change 归档合回 |

- `full` 模式 MUST NOT 呈现任何可提交的 `gate` 编辑入口（按钮 / 下拉 / 行内编辑），MUST NOT 发起写命令。
- `incremental` 模式的改档 SHALL 经**一个显式写命令**（服务端写前校验 + 渲染自证 + 原子写回）完成，MUST NOT 由前端直接拼写 / 重写 TOML 文本。
- 改档成功 SHALL 触发一次显式重取（图与抽屉同步刷新）；失败 SHALL 以行内错误呈现且磁盘零变化。
- `incremental` 模式 MUST NOT 提供「新增 / 删除元素」「连边」一类结构编辑入口（本期只开放 `gate` 单字段写入）。

#### Scenario: 全量模式零写入口

- **WHEN** 在 `/architecture`（`mode = full`）审查元素抽屉
- **THEN** 无任何可提交的 `gate` 编辑控件，呈现「改档需要开一个 change」提示；审查源码零写命令调用

#### Scenario: 增量模式改档落 worktree

- **WHEN** 在 change 详情「架构」页签把 `App.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 写命令只改该 worktree 内模型文件的目标行（注释与其余内容保留），成功后图与抽屉显示新档位；主仓模型零变化

#### Scenario: 改档失败零写入

- **WHEN** 改档命令因元素缺席 / 值非法 / 渲染自证失败返回 `Err`
- **THEN** 抽屉呈现行内错误，磁盘模型字节零变化

### Requirement: 抽屉内容与本期边界

元素抽屉 SHALL 呈现四类信息：① 元素概览（`kind` / `path` / `gate` / `modelFile:line`，位置可读且可直接用于 `edit`）；② `docs` 关联列表（三源条目，按强度序）；③ ADR 条目列表（`kind = "adr"` 的关联项，点击按需取正文并渲染 markdown，正文取数只接受关联集内路径）；④ `incremental` 模式下 `gate` 的编辑入口。

以下内容 MUST NOT 在本期出现（归 `archi-check-and-gate`，防"半成品字段"）：

- 「相邻报告」区块（最近一次检查的 errors / warnings）——本期抽屉零该区块、零占位框；
- 漂移统计 / 门禁结论 / 豁免 reason 编辑入口。

正文取数命令 SHALL 只接受**关联集内**的文档路径（越界显式 `Err`），MUST NOT 成为任意文件读取通道。

#### Scenario: 概览含可定位声明位置

- **WHEN** 打开某元素抽屉
- **THEN** 呈现 `kind` / `path` / `gate` 与 `modelFile:line`（该文件该行为该元素表头），agent 与人可据此直接定位声明

#### Scenario: ADR 正文按需取

- **WHEN** 抽屉内 ADR 条目被展开
- **THEN** 以该条目 `path` 取正文并渲染 markdown；取数路径不在关联集内时命令返回 `Err`，前端呈现行内错误

#### Scenario: 本期无相邻报告区块

- **WHEN** 审查抽屉渲染产物
- **THEN** 无「相邻报告」区块、无 `errors` / `warnings` 计数字段、无空占位框

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src/views/architecture/components/architecture-canvas.tsx` | 共享画布 | 入参 `{ root, mode: 'full' \| 'incremental' }`；三段布局（元素树 / xyflow 画布 / 抽屉）；1-hop 缺省；两态切换（正常态 / 停摆态）；data-testid 挂钩 |
| `src/views/architecture/components/element-tree.tsx` | 左元素树 | 层级由 FQN 前缀派生；节点状态点取节点标记词汇（`none` / `unfulfilled`）；停摆态零节点 |
| `src/views/architecture/components/structure-error-panel.tsx` | 停摆态 | 逐条诊断（`kind` + `message` + `file:line`）+「修好后再看」；零图数据、零降级样式 |
| `src/views/architecture/components/element-drawer.tsx` | 右抽屉 | 概览（`kind` / `path` / `gate` / `modelFile:line`）+ `docs` 列表 + ADR 条目与按需正文；`incremental` 下 `gate` 编辑入口；本期零「相邻报告」区块 |
| `src/views/architecture/lib/marks.ts` | 标记 → 样式映射 | 边标记四值（`none` 实线 / `unfulfilled` 灰虚线 / `crossKind` 橙虚线 / `exempt` 灰实线）、节点标记两值（`none` ⚪ / `unfulfilled` 🟡）；未知值退化为中性；封闭词汇 |
| `src/views/architecture/hooks/use-archi-graph.ts` | 画布取数 | 挂载 / `root` 变化 / 手动刷新三触发；零 watch 零轮询；停摆态与正常态两形状透出 |
| `src/views/architecture/hooks/use-archi-module.ts` | 抽屉取数 | 元素详情（`archi_module`）+ ADR 正文（`archi_read_doc`，仅关联集内路径） |
| `src/views/architecture/architecture-view.tsx` | 挂载点 A | `/architecture` 全量只读；显式刷新入口 |
| `src/views/changes/change-detail-view.tsx`（架构页签） | 挂载点 B | 以 `detail.worktree` 为根、归档回落主仓根；`incremental` 可改档；页签切换不重复取数 |
| `src/routes.tsx` + `src/components/app-sidebar.tsx` | 入口 | `/architecture` 路由 + 页面组「架构」项（`data-testid="nav-architecture"`，active 由 URL 派生） |
