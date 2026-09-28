# 设计: change-detail-react-flow

> **变更**: change-detail-react-flow
> **日期**: 2026-09-24

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 流程图纯函数转换层 | `detail → { nodes, edges }`：三分类事件收集、O(n) 时间序归并、链式边推导、列索引 / 执行序坐标 | `packages/desktop/src/views/changes/flow/graph.ts`（入口）、`flow/types.ts`（模型）、`flow/layout.ts`（常量与坐标） | `types/dto`（`ChangeDetail`）；零 react / 零 @xyflow/react 依赖、零 invoke | TypeScript 纯函数 |
| 素材挂载派生 | 文档→列、eval-checklist→节点、file_log `scope+attempt`→节点、`scope='workflow'` 与未命中条目→图外 | `packages/desktop/src/views/changes/flow/attachments.ts` | `FlowGraph`、`ArtifactEnvelope`、`FileLogEntry` | TypeScript 纯函数 |
| 流程图渲染薄层 | ReactFlow 挂载、subflow 9 列容器、边视觉映射、fitView、点击回调 | `packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx` | 转换层输出、`@xyflow/react` | React 19 + `@xyflow/react` v12 |
| 列容器 / 事件节点组件 | 列头（phase 名 + 过程文档徽章）；eval / active / interrupted 三类节点视觉 | `packages/desktop/src/views/changes/flow/PhaseColumnNode.tsx`、`flow/FlowEventNode.tsx` | `@xyflow/react` `NodeProps`、`FlowMaterials` | React 自定义节点 |
| 右侧抽屉 | 单一交互入口，三分节【本站文档 / eval report+checklist / 文件表】 | `packages/desktop/src/views/changes/flow/DetailDrawer.tsx` | `renderers/registry`（`ArtifactView`）、`FileLogTable`、`FlowMaterials` | React |
| file_log 表格 | workflow 独立面板与抽屉文件表节共用表体 | `packages/desktop/src/views/changes/flow/FileLogTable.tsx` | `components/ui/table` | React |
| 页面组装 | Header + 流程图区（v0 占位）+ workflow 独立面板 + 产物区 + 抽屉状态 | `packages/desktop/src/views/changes/ChangeDetailView.tsx` | `useChangeDetail` 状态、上述全部组件 | React |
| 取数层（不改） | 显式 refresh 触发 `get_change_detail` + 逐产物 `read_artifact` | `packages/desktop/src/views/changes/hooks/useChangeDetail.ts` | Tauri IPC | 既有 hook，零改动 |

命名与分层：flow 子目录与 `renderers/`、`hooks/` 平级，同为 `views/changes/` 按关注点分层；转换层不依赖渲染层，渲染层只做映射与交互（`max-lines-per-function: 50` 约束下组件逐文件拆分，每文件单函数不超限）。

页面布局草图（布局即语义）：

```
col:proposal   col:dev-design    col:test-design  col:implement  ...  col:code-analyze   ← 9 列恒定（未走站空列）
y0: proposal#1──▶dev-design#1 ✗─────────────────▶implement#1 ✓
y1:             dev-design#2 ✓ ◀━━ 回跳(虚线+reason) ━━test-execution#1 ✗
y2:             interrupted 灰显 ┊ active pulse …
```

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  测试文件（flow/*.test.ts、flow/*.test.tsx、ChangeDetailView.test.tsx 断言迁移）由 test-design 阶段承接，
  不列入本清单与 tasks.md。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/views/changes/flow/types.ts` | 流程图视图模型类型：`FlowNodeKind` / `FlowEdgeKind` / `FlowNode` / `FlowEdge` / `FlowColumn` / `FlowGraph` / `FlowMaterials` / `DrawerSelection` 与节点 id 方案注释 |
| `packages/desktop/src/views/changes/flow/layout.ts` | 布局常量（`PIPELINE_PHASES` 前端镜像、`COL_W` / `ROW_H` / `COLUMN_HEADER_H` / `COLUMN_PAD_X` / `COLUMN_PAD_Y`）与坐标纯函数 `nodePosition` |
| `packages/desktop/src/views/changes/flow/graph.ts` | 核心转换纯函数 `buildFlowGraph`：事件三分类收集 → 时间序归并 → 链式边推导 → 列对齐坐标 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 素材挂载纯函数 `mountMaterials` + 模块私有的文档→列静态映射表（`docColumn`，不导出） |
| `packages/desktop/src/views/changes/flow/PhaseColumnNode.tsx` | phase 列容器自定义节点：列头 phase 名 + 该站过程文档徽章，点击打开抽屉 |
| `packages/desktop/src/views/changes/flow/FlowEventNode.tsx` | 事件自定义节点：eval（实心，pass 绿 / fail 红，stale 半透明，backtrack 徽标）/ active（pulse）/ interrupted（dashed 灰显） |
| `packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx` | ReactFlow 薄层：`@xyflow/react` 挂载、subflow（`parentId` + `extent: 'parent'`）、`nodePosition` 映射、边视觉、fitView、`onNodeClick → onSelect` |
| `packages/desktop/src/views/changes/flow/FileLogTable.tsx` | file_log 条目表体（op / scope / attempt / path / at），workflow 面板与抽屉文件表节共用 |
| `packages/desktop/src/views/changes/flow/DetailDrawer.tsx` | 右侧抽屉：三分节【本站文档 / eval report+checklist / 文件表】，列头与节点点击共用同一入口 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/views/changes/ChangeDetailView.tsx` | 拆除文件内线性布局组件（`Station` / `Attempt` / `VerdictBadge` / `DetailSectionInterupted` / `DetailSectionFileList` 及 `INVENTORY_VARIANT` 之外的展示件），改为 Header + 流程图区（v0 空图占位）+ workflow 独立面板 + 产物区 + 抽屉挂载（`selection` 本地 state） | `DetailHeader` / `DetailFallback` / `unparsable` 警示条 / `INVENTORY_VARIANT` 映射 / `formatTime` 原样保留；被拆组件的展示语义（verdict 徽标、backtrack 文案、checklist 条目、中断留档时间、file_log 表格）分别迁入 `FlowEventNode` 与 `DetailDrawer`，信息不丢失 |
| `packages/desktop/package.json` | `dependencies` 新增 `"@xyflow/react": "^12.11.0"`；`version` 由 `0.3.2` 升为 `0.3.3` | 依赖与版本交付（AC-10）；`tauri.conf.json` 经 `../package.json` 引用自动跟随、`src-tauri/Cargo.toml` 不随动 |

### 删除文件

<!-- 无 —— 旧线性布局在 ChangeDetailView.tsx 内就地改造，无独立文件删除，省略此子节 -->

### 公共函数 / API

<!-- 仅列模块级导出；转换层 / 挂载层的内部辅助函数（事件收集、归并、边推导、docColumn 映射）均不导出，经公共入口测试 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `buildFlowGraph` | `flow/graph.ts` | 新增 | `function buildFlowGraph(detail: ChangeDetail): FlowGraph` | 详情聚合 → 流程图模型；`pipeline` 为空（v0）时返回空图（无列无节点） |
| `mountMaterials` | `flow/attachments.ts` | 新增 | `function mountMaterials(graph: FlowGraph, detail: ChangeDetail, envelopes: ArtifactEnvelope[]): FlowMaterials` | 三类素材按挂载规则归位；未命中条目归 `outsideFiles`，不丢信息 |
| `nodePosition` | `flow/layout.ts` | 新增 | `function nodePosition(node: FlowNode): { x: number; y: number }` | react-flow 就绪坐标：列容器为画布绝对坐标，事件节点为相对父列坐标（见数据模型坐标约定） |
| `ChangeFlowGraph` | `flow/ChangeFlowGraph.tsx` | 新增 | `function ChangeFlowGraph(props: { graph: FlowGraph; materials: FlowMaterials; onSelect: (selection: DrawerSelection) => void }): React.JSX.Element` | 流程图渲染薄层；节点点击经 `onSelect` 上抛 |
| `DetailDrawer` | `flow/DetailDrawer.tsx` | 新增 | `function DetailDrawer(props: { selection: DrawerSelection | null; graph: FlowGraph; materials: FlowMaterials; onClose: () => void }): React.JSX.Element \| null` | `selection` 为 null 时不渲染 |
| `FileLogTable` | `flow/FileLogTable.tsx` | 新增 | `function FileLogTable(props: { entries: FileLogEntry[] }): React.JSX.Element` | 空数组渲染「（空）」占位；`attempt` / `at` 空缺渲染「—」 |
| `PhaseColumnNode` | `flow/PhaseColumnNode.tsx` | 新增 | `function PhaseColumnNode(props: NodeProps): React.JSX.Element` | 列容器节点（nodeTypes 键 `column`） |
| `FlowEventNode` | `flow/FlowEventNode.tsx` | 新增 | `function FlowEventNode(props: NodeProps): React.JSX.Element` | 事件节点（nodeTypes 键 `event`），按 `kind` 三分类视觉 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FlowNodeKind` | `flow/types.ts` | 新增 | `'eval' \| 'active' \| 'interrupted'` |
| `FlowEdgeKind` | `flow/types.ts` | 新增 | `'forward' \| 'retry' \| 'backtrack'`，由边两端列索引差 Δ 的符号派生 |
| `FlowNode` | `flow/types.ts` | 新增 | 事件节点：`id` / `kind` / `phase` / `attempt`（缺号 0 兜底）/ `colIndex` / `order` / `parentId` / `record`（eval 独有 `AttemptRecord`）/ `startAt` / `endAt`（interrupted 独有） |
| `FlowColumn` | `flow/types.ts` | 新增 | 列容器：`id` / `phase` / `colIndex` |
| `FlowGraph` | `flow/types.ts` | 新增 | `{ columns: FlowColumn[]; nodes: FlowNode[]; edges: FlowEdge[] }`；v1+ 恒 9 列，`nodes` 列容器在前、事件节点按归并序在后 |
| `FlowEdge` | `flow/types.ts` | 新增 | `{ id; source; target; kind: FlowEdgeKind; label: string \| null }`；回跳边 `label` 取目标节点 `record.backtrackReason` |
| `FlowMaterials` | `flow/types.ts` | 新增 | `{ columnDocs: Record<string, ArtifactEnvelope[]>; nodeChecklists: Record<string, ArtifactEnvelope[]>; nodeFiles: Record<string, FileLogEntry[]>; outsideFiles: FileLogEntry[] }` |
| `DrawerSelection` | `flow/types.ts` | 新增 | `{ scope: 'column'; phase: string } \| { scope: 'node'; nodeId: string }` 判别 union |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.3"`（原 `"0.3.2"`） | AC-10 版本交付；`tauri.conf.json` 引用 `../package.json` 自动跟随，`src-tauri/Cargo.toml`（`0.1.0`）不随动 |
| `dependencies.@xyflow/react` | `packages/desktop/package.json` | 新增 | `"^12.11.0"` | react-flow v12（React 19 + Tailwind 4 兼容线）；不引入 dagre / elk 等第二布局库 |

### 边界与不修改清单

`useChangeDetail.ts`（取数语义：显式 refresh、无 watch / 轮询）、`renderers/`（registry 与既有 renderer，抽屉只消费）、`src-tauri/crates/core/workflow/src/queries/detail.rs`（`ChangeDetail` DTO 与 `PIPELINE_PHASES`）、`plugins/dev-team/bin/src/modules/workflow/files/record.ts`（file_log scope 写入侧）、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`——全部零改动。

---

## 数据模型

流程图模型为运行时派生物（不持久化、不落库），唯一事实源是经 `get_change_detail` 取得的 `ChangeDetail`（DTO 零改动）。

### 节点 id 方案与三分类

| 节点 | id | 数据源 | 视觉 |
|------|----|--------|------|
| 列容器 | `col:<phase>` | `PIPELINE_PHASES`（前端 `flow/layout.ts` 常量，与 Rust 端同序镜像） | 列头：phase 名 + 过程文档徽章 |
| eval | `eval:<phase>:<attempt>` | `detail.pipeline[phase].attempts[]` | 实心；pass 绿 / fail 红；`stale` 半透明；`skipped` / `backtrackTo` / `backtrackReason` 以徽标文案保留（迁移旧 `Attempt` 块语义） |
| active | `active:<phase>:<attempt>` | `detail.activePhase` | pulse 运行中，无 verdict，列内接流末端 |
| interrupted | `interrupted:<phase>:<attempt>` | `detail.interrupted[]` | dashed 边框灰显，展示 startAt ~ endAt；独立成节点，不与同号 eval 合并 |

`attempt` 为 null 时按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款）；eval 事件按 `PIPELINE_PHASES` 站序 + 站内 attempts 序收集（后端已按 attempt 稳定排序，天然为 eval 追加序）。

### 坐标约定（AC-1 公式的 react-flow 落地）

- 列容器（绝对坐标）：`x = colIndex × COL_W`，`y = 0`，宽 `COL_W`，高 `COLUMN_HEADER_H + 列内节点数 × ROW_H + COLUMN_PAD_Y`。常量取值：`COL_W = 260`、`ROW_H = 128`、`COLUMN_HEADER_H = 48`、`COLUMN_PAD_X = 18`、`COLUMN_PAD_Y = 16`（节点宽 = `COL_W − 2 × COLUMN_PAD_X` = 224，9 列总宽 ≈ 2340px，靠 fitView 缩放适配）。
- 事件节点（相对父列坐标，`parentId = col:<phase>` + `extent: 'parent'`）：`x = COLUMN_PAD_X`，`y = COLUMN_HEADER_H + order × ROW_H`。`order` 为该事件在列内归并序；等价绝对坐标的列索引项与执行序项与 AC-1 公式一一对应，`COLUMN_HEADER_H` 为列容器自身的常量偏移。
- `nodePosition` 返回上表坐标；`ChangeFlowGraph` 仅做 `事件节点坐标 − 父列坐标` 的一次机械换算以喂给 subflow，无其他坐标运算。

### 时间序归并与边推导（O(n)）

1. **主序 = eval 追加序**：按站序 + 站内 attempts 序串联为骨干链，不按时间重排（追加序即事实执行序；时间戳乱序的失真风险由夹具回归覆盖）。
2. **interrupted / active 归并插入**：锚点时间取 `startAt`（经 `Date.parse` 为毫秒比较，解析失败视同 null）。插入点 = 骨干中第一个锚点时间严格晚于其 `startAt` 的 eval 事件之前；骨干锚点为 null 的事件不作插入参考；`startAt` 为 null 或无插入点者按列表序（先 `interrupted[]` 后 active）追加链尾。
3. **链式边**：归并后序列头尾相连，每个非首事件恰一条入边（`edge:<source>-><target>`）。`kind` 由 ΔcolIndex 符号派生：`> 0` → `forward`、`= 0` → `retry`、`< 0` → `backtrack`（虚线 + warn 色 + `label = 目标节点 record.backtrackReason`）。边视觉：常规边 border token 色。

### 素材挂载规则（文档挂列、记录挂节点）

| 素材 | 匹配依据 | 去向 |
|------|----------|------|
| `markdown-doc` / `tasks-progress` 信封 | 模块私有静态映射 `docColumn(source)`：`proposal.md`、`specs/**`（前缀 `specs/`）→ proposal；`design.md`、`tasks.md` → dev-design；`test-reports/**` → test-execution | `columnDocs[phase]`（列头徽章，每站一份不随 attempt 重复） |
| 映射表外文档（`explore.md`、旧代际 `phases/**`、其余路径） | 无匹配 | 不入图——产物区全量列表天然兜底，不误挂任何站 |
| `eval-checklist` 信封 | payload `{ phase, attempt }`（以最小形状守卫收窄，flow 层自持、不改 renderers/）→ 按 id 方案定位节点（eval 优先，其次 active、interrupted） | `nodeChecklists[nodeId]` |
| `file_log` 条目 | `scope` 为 phase id 且 `attempt !== null` → 同上定位节点 | `nodeFiles[nodeId]` |
| `file_log` `scope='workflow'` 条目与未命中节点的 phase-scope 条目 | — | `outsideFiles`（图外 workflow 独立面板完整展示，信息不丢） |

`docColumn` 不导出（knip 会把仅测试引用的导出判为未用），文档归属映射经 `mountMaterials` 公共输出覆盖测试。

### 三代际降级映射

| 代际 | 判定 | 呈现 |
|------|------|------|
| v0 | `detail.pipeline.length === 0` | 不挂载 `ChangeFlowGraph`，渲染空图占位（保留现有文案「v0 早期代际：无 workflow.json，仅文档形态」）+ 产物区文档列表照常 |
| v1 | `detail.fileLog === null` | 流程图正常绘制；不渲染 workflow 独立面板；抽屉文件表节呈空态降级（保留现有文案「无 file_log 数据：v1 及更早代际无此字段」） |
| v2 | 其余 | 完整图 + workflow 独立面板 |

`unparsable` 警示条与 error / loading / 未找到降级页沿用现状（workflow 解析失败时 pipeline 各站 attempts 为空 → 9 空列正常呈现）。

### 抽屉内容规则

- **本站文档节**：`columnDocs[phase]` 逐个经 `ArtifactView`（`renderers/registry`）渲染；空则空态。
- **eval report+checklist 节**：eval 节点渲染 `record.report` 文本；checklist 优先渲染挂载的 eval-checklist 信封（`ArtifactView`，复用 registry），无信封挂载（读取失败降级信封 payload 为 null 不可匹配）时回退内联渲染 `record.checklist` 条目（迁移旧 checklist 展示语义）；active / interrupted 与列头选中时该节呈空态。
- **文件表节**：`FileLogTable`（节点选中渲染 `nodeFiles[nodeId]`；v1 呈降级文案；列头选中呈空态——attempt 作用域未定，不虚构聚合）。
- 关闭：遮罩点击或关闭按钮，`selection` 置 null。

### 交互与渲染参数

- `ReactFlow`：`nodesDraggable={false}`、`nodesConnectable={false}`、`fitView`（`fitViewOptions={{ padding: 0.1, maxZoom: 1 }}`、`minZoom={0.15}`）、`<Background variant="dots" />`；显式 refresh 后图数据变化时重新 fitView。
- 样式引入：`import '@xyflow/react/dist/style.css'` 置于 `ChangeFlowGraph.tsx` 组件级引入（`global.css` 是 app token 单一来源，不混入库样式）；节点 / 边颜色一律引用现有 token（pass / fail / warn / border / muted）。
- data-testid 挂钩（供 test-design 采用）：`flow-graph`、`flow-column`、`flow-node`、`flow-empty`、`detail-drawer`、`drawer-docs-section`、`drawer-eval-section`、`drawer-files-section`、`workflow-panel`（面板内表格沿用 `filelog-table`）。

---

## 依赖

### 运行时依赖

- `@xyflow/react` ^12.11.0 — react-flow v12 渲染层（React 19 + Tailwind 4 兼容线，探索已验证）；仅 `ChangeFlowGraph.tsx` 与两个节点组件 import，转换层零依赖

### 构建/测试依赖

- 无新增 — vitest / vite-plus / knip 沿用现状；转换层为纯函数，vitest 直测无需额外依赖

---

## 待决问题

- （无 —— proposal 三轮探索全部定案；dev-design 层三项遗留细节已在本设计定夺：flow 模块目录与文件命名见变更清单、COL_W / ROW_H 与 fitView 策略见坐标约定、react-flow 样式引入位置见交互与渲染参数。）
- 备注（交 test-design 阶段处理，非实现阻塞）：ReactFlow 在 jsdom 下依赖 `ResizeObserver` 等浏览器 API，集成测试可能需在测试环境提供 shim。

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-change-flow-view/spec.md` 已由提案阶段写入并同步，不列入变更清单与任务；本设计不修改提案层产物。

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | `flow/layout.ts` 常量与 `nodePosition` 坐标约定；`buildFlowGraph` 恒输出 9 列（v0 空图除外），`FlowColumn` / `colIndex` / `order` 落定公式 |
| AC-2 | `FlowEventNode` 三分类视觉；`graph.ts` 三分类收集；interrupted 独立 id（`interrupted:<phase>:<attempt>`）不与 eval 合并；stale 半透明 |
| AC-3 | `graph.ts` 归并算法（eval 追加序主链 + startAt 插入）与链式边推导；ΔcolIndex 符号 → forward / retry / backtrack；回跳边虚线 + `backtrackReason` 标签 |
| AC-4 | `flow/attachments.ts` 挂载规则表（文档挂列 / checklist 与 file_log 挂节点 / workflow 与 explore.md 不入图）；`FlowMaterials` 结构 |
| AC-5 | `DetailDrawer` 三分节与单一入口；文档节经 `ArtifactView`（renderers/registry） |
| AC-6 | 三代际降级映射表；`ChangeDetailView` 按 inventory 分支（v0 占位不挂图、v1 无面板 + 抽屉空态） |
| AC-7 | `useChangeDetail.ts` 列入不修改清单；图数据仅随 `state.detail` 显式 refresh 重算 |
| AC-8 | 转换层纯函数无 react / 无 invoke，公共入口 `buildFlowGraph` / `mountMaterials` / `nodePosition` 可直测（测试文件由 test-design 阶段承接） |
| AC-9 | 组件按文件拆分满足 `max-lines-per-function: 50`；`docColumn` 不导出避免 knip 豁免；data-testid 挂钩清单；守线任务仅含 `client:check`（fmt / lint / knip）与零 diff 边界检查（静态，不含测试执行；测试执行归 test-execution 阶段） |
| AC-10 | `packages/desktop/package.json` version `0.3.3` + `@xyflow/react` 依赖；`tauri.conf.json` / `src-tauri/Cargo.toml` 列入不修改清单 |
