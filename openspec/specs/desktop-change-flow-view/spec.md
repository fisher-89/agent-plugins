# desktop-change-flow-view Specification

## Purpose

定义 desktop change 详情页的 react-flow 流程图视图：attempt 级三分类节点（eval / active / interrupted）、9 列手工坐标布局、时间序边推导、过程素材两层挂载、右侧抽屉单一交互入口与三代际降级；转换层为纯函数、渲染薄层复用 react-flow v12。

## Requirements

### Requirement: 流程图节点模型与三分类

change 详情页 SHALL 以 react-flow 流程图承载执行史，节点粒度 SHALL 为 attempt 级。转换层 SHALL 将 `ChangeDetail` 的事件归并为三类节点：

- **eval 节点**：来自 `pipeline` 各站的 `attempts[]`（verdict / checklist / report / skipped / stale），实心呈现——pass 绿、fail 红；`stale: true` 的节点 SHALL 半透明淡化（废弃支线）；
- **active 节点**：来自 `activePhase`（运行中，无 verdict），pulse 运行中视觉，位于所在列的接流末端；
- **interrupted 节点**：来自 `interrupted[]`（startAt ~ endAt 留档），dashed 灰显。

interrupted 节点 SHALL 独立成节点，MUST NOT 与后续同号 eval 节点合并（CLI 语义：interrupted 永不写 eval、与 eval 无 id 关联）。9 站顺序 SHALL 以 `PIPELINE_PHASES` 固定序列为准（proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → code-analyze），不依赖 eval 排列。

#### Scenario: 三分类节点呈现

- **WHEN** 打开一个含多 attempt、backtrack 与 interrupted 留档的 v2 change 详情（v2-b 夹具形态：`interrupted` 的 test-gen#1 与 `active_phase` 的 implement#2 均不在 eval 序列内）
- **THEN** 图上同时呈现 eval 节点（pass 绿 / fail 红）、pulse 的 active 节点与 dashed 灰显的 interrupted 节点，且 interrupted 独立成节点不与同号 eval 合并

#### Scenario: stale 淡化

- **WHEN** 某站的最新 pass eval 被 backtrack 标记 `stale: true`
- **THEN** 该节点呈半透明淡化，表示废弃支线，但仍保留在图上

### Requirement: 9 列手工坐标布局

布局 SHALL 采用手工坐标，MUST NOT 引入 dagre / elk 等自动布局引擎：`x = PIPELINE_PHASES.indexOf(phase) × COL_W`（9 列恒定，未走过的站呈现空列），`y = 列内执行序 × ROW_H`。同 phase 的全部事件 SHALL 纵向对齐成一列。phase 列 SHALL 以 react-flow v12 subflow 承载（`parentId` + `extent: 'parent'`），列头展示 phase 名与该站过程文档徽章。COL_W / ROW_H 常量取值由 design 定夺。视口 SHALL 默认 100% 缩放（MUST NOT 初始 fitView 适配），滚轮 SHALL 平移画布（滚轮缩放 MUST NOT 触发），显式刷新重绘时 SHALL 保留当前视口。

#### Scenario: 列对齐与空列恒定

- **WHEN** 某 change 只走到 dev-design 站（proposal 有 eval 记录）
- **THEN** proposal 与 dev-design 事件各自纵向对齐成列，后续 7 站仍以空列呈现，列总数恒为 9

#### Scenario: backtrack 重做自动向左

- **WHEN** test-execution#1 fail 后 backtrack 回 test-gen 并重做 test-gen#2
- **THEN** 重做的 test-gen#2 节点位于 test-gen 列（Δx < 0），无需显式边分类即呈现为向左回跳

### Requirement: 时间序边推导

边 SHALL 由 O(n) 扫描推导：eval 事件按追加序（即时间序）串联，interrupted / active 事件按 startAt 归并插入；每个事件的入边 SHALL 指向时间上紧邻的前一事件。边的 Δx 符号 SHALL 表达类型：向右=前进、同列（Δx=0）=重试、向左=回跳；回跳边 SHALL 为虚线并以 `backtrackReason` 作边标签。attempt 缺号 SHALL 按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款），startAt 缺失的事件 SHALL 按列表序归并、不丢节点。

#### Scenario: 前进 / 重试 / 回跳三类边

- **WHEN** 执行史为 proposal#1 → dev-design#1 fail → dev-design#2 pass（回跳重做）
- **THEN** proposal#1→dev-design#1 为向右前进边，dev-design#1→dev-design#2 为同列重试边；若失败边源自跨站 backtrack（如 test-execution#1 → test-gen#2），该边呈虚线且 `backtrackReason` 可读

#### Scenario: 缺号与缺时间戳兜底

- **WHEN** 某事件 `attempt` 为 null 或 startAt 为 null
- **THEN** 节点不丢失：attempt 以 0 兜底参与列内排序，startAt 缺失者按 eval 追加序归并，转换不报错

### Requirement: 过程素材挂载规则

素材挂载分两层：**文档挂列、记录挂节点**。

- 过程文档（`artifacts` 中 `markdown-doc` / `tasks-progress` 类）SHALL 经前端静态映射表归属到 phase 列（列头徽章，每站一份、不随 attempt 重复）：`proposal.md` 与 `specs/**` → proposal、`design.md` 与 `tasks.md` → dev-design、`test-reports/**` → test-execution；映射表外的文档 SHALL 落入「未归属」处理（design 定夺展示位置），MUST NOT 误挂到错误站。`explore.md` MUST NOT 入图（探索在流程之外，不属于任何站）。
- eval-checklist 信封（payload 自带 `phase` + `attempt`）SHALL 挂到对应 attempt 节点。
- file_log 条目 `scope` 为 phase id 且带 `attempt` 者 SHALL 精确挂到对应 attempt 节点；`scope='workflow'` 的条目 MUST NOT 入图，SHALL 在图外独立面板展示。

归属 SHALL 全部在前端派生，后端 `ChangeDetail` DTO 与 `PIPELINE_PHASES` MUST NOT 因本能力修改。

#### Scenario: 文档按映射表归列

- **WHEN** 某 change 的产物含 `proposal.md`、`design.md`、`tasks.md` 与 `test-reports/xxx.md`
- **THEN** proposal 列头徽章含 proposal.md，dev-design 列头徽章含 design.md 与 tasks.md，test-execution 列头徽章含该 test-reports 文档

#### Scenario: workflow scope 独立面板与 explore.md 隐身

- **WHEN** file_log 仅含 `scope='workflow'` 条目（存量数据常态），且产物清单含 `explore.md`
- **THEN** 图上无任何文件徽章挂靠（节点文件区为空），workflow 条目在图外独立面板完整呈现，explore.md 不出现在图中

### Requirement: 右侧抽屉单一交互入口

phase 列头与 attempt 节点 SHALL 提供同一交互入口：点击打开右侧抽屉，抽屉按【本站文档 | eval report+checklist | 文件表】分节——本站文档节 SHALL 复用 `renderers/registry`（经 `ArtifactView` 渲染，新增产物 kind 仍只需在 registry 追加一行）；eval 节展示该 attempt 的 report 与 checklist 条目；文件表节展示挂靠该节点的 file_log 条目（op / path / at）。列头点击与节点点击 MUST NOT 出现两套并存的展开交互。

#### Scenario: 节点点击抽屉内容

- **WHEN** 点击 implement#2（fail → retry pass 的第二个 attempt）节点
- **THEN** 抽屉展示该站文档、attempt#2 的 report 与 checklist、attempt#2 名下的 file_log 文件表

#### Scenario: 列头点击同入口

- **WHEN** 点击 dev-design 列头
- **THEN** 打开与节点点击相同的右侧抽屉，本站文档节含 design.md / tasks.md

### Requirement: 三代际降级

视图 SHALL 按 inventory 降级：

- **v0**（无 workflow.json）：SHALL 呈现空图占位，产物区照常呈现（图区 MUST NOT 挤掉产物区）；
- **v1**（无 file_log，`fileLog === null`）：SHALL 正常绘制流程图（eval / active / interrupted 节点照常），节点文件区为空、抽屉文件表节呈空态；
- **v2**：完整图（含 file_log 挂载与 workflow 独立面板）。

`unparsable`（workflow.json 损坏）警示与 error / loading / 未找到降级页 SHALL 沿用现状。

#### Scenario: v0 纯文档形态

- **WHEN** 打开 inventory 为 v0 的 change 详情
- **THEN** 图区呈现空图占位，产物区照常展示，页面不报错

#### Scenario: v1 无文件区

- **WHEN** 打开 inventory 为 v1 的 change 详情（fileLog 为 null）
- **THEN** 流程图正常绘制各节点与边，任一节点抽屉的文件表节为空态，无 file_log 报错

### Requirement: 取数模型与管线合规不变

视图改造 MUST NOT 改变取数契约：详情更新 SHALL 仅由显式 refresh（或选中 change 变化）触发，MUST NOT 引入 watch 订阅、定时轮询或事件订阅（live 刷图留作后续独立 change）。desktop 前端 SHALL 维持全管线通过：`vp check --fix`（fmt / lint，含 `max-lines-per-function: 50`）、knip（无未用导出残留）、`vp test` 全绿；测试 SHALL 以 data-testid 为查询挂钩，MUST NOT 以样式类名查询。

#### Scenario: 显式刷新仍是唯一更新途径

- **WHEN** 流程图呈现期间 change 的 workflow.json 被外部进程修改
- **THEN** 图不自动变化；点击「刷新详情」后经 `get_change_detail` 重取并重绘

#### Scenario: 全管线通过

- **WHEN** 运行 `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test`
- **THEN** fmt / lint / knip / 测试全部通过，`package.json` 与 knip.json 无新增豁免条目

### Requirement: 纯函数转换层与渲染薄层

`detail → { nodes, edges }` 转换 SHALL 为独立纯函数模块（`views/changes/` 下，目录与命名由 design 定夺），不依赖 react 宿主组件状态、不发起任何 invoke，vitest SHALL 直测其边界：前进 / 重试 / 回跳 / stale 淡化 / interrupted 插入 / active 接流末端 / attempt 缺号 / 时间戳缺失 / 文档归属映射 / workflow scope 剔除。react-flow SHALL 仅作渲染薄层，其库内布局与交互语义 MUST NOT 做逐项断言（符合「不逐项验证库语义」约定）。

#### Scenario: 转换层直测

- **WHEN** 以 v2-a / v2-b 夹具形状的 `ChangeDetail` 数据（含 fail→retry、backtrack、stale、interrupted）调用转换函数
- **THEN** 输出的节点坐标符合列索引 / 执行序公式，边集合与方向符合时间序推导，无需挂载 react 组件即可断言

### Requirement: desktop 版本升级交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.3.2` 升级为 `0.3.3`。`src-tauri/tauri.conf.json` SHALL 维持 `../package.json` 引用（版本自动跟随，MUST NOT 引入硬编码版本号）；`src-tauri/Cargo.toml` 的 `[package].version`（`0.1.0`）MUST NOT 随 desktop 版本变动（既有惯例：0.3.0 / 0.3.1 两次升级均只改 package.json）。

#### Scenario: 版本号升级与引用跟随

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` 的 `version` 为 `0.3.3`，`tauri.conf.json` 无版本号硬编码改动，`src-tauri/Cargo.toml` 版本保持 `0.1.0`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/`（新，命名 design 定） | 纯函数转换层 | `detail → { nodes, edges }`：x = `PIPELINE_PHASES` 列索引 × COL_W、y = 列内执行序 × ROW_H；节点三分类；O(n) 时间序边推导；attempt 缺号 0 兜底；文档→列静态映射表；无 invoke、无 react 状态依赖 |
| ReactFlow 渲染薄层组件（新） | 图呈现 | `@xyflow/react` v12 subflow（`parentId` + `extent: 'parent'`）；9 列恒定 + 默认 100% 视口 + 滚轮平移（禁滚轮缩放）；eval 实心 / active pulse / interrupted dashed / stale 半透明 |
| phase 列头 + 自定义节点组件（新） | 列容器与节点 | 列头：phase 名 + 过程文档徽章；节点：verdict 视觉 + attempt 序号；点击统一进抽屉 |
| 右侧抽屉组件（新） | 素材详情 | 三分节【本站文档 / eval report+checklist / 文件表】；文档节复用 `renderers/registry`（`ArtifactView`）；单一交互入口 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 页面组装 | Header + 流程图 + workflow 独立面板（`scope='workflow'` file_log）+ 产物区；v0 空图占位 / v1 无文件区降级；`unparsable` 警示与降级页保留 |
| `packages/desktop/package.json` | 新依赖与版本交付 | `@xyflow/react` v12（React 19 + Tailwind 4 兼容）；无第二图布局库（无 dagre / elk）；`version` 0.3.2 → 0.3.3（`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动） |
| 后端 `workflow::queries::detail`（不改） | 数据来源 | `ChangeDetail` DTO 与 `PIPELINE_PHASES` 不动；归属全部前端派生 |
