# 任务: change-detail-react-flow

> 依赖顺序：依赖与版本 → 纯函数转换层 → 渲染层 → 页面组装 → 守线自检。
> 测试文件（`flow/*.test.ts`、`flow/*.test.tsx`、`ChangeDetailView.test.tsx` 断言迁移）由 test-design / test-gen 阶段承接，测试执行归 test-execution 阶段，均不在本列表。

## 阶段一：依赖与版本交付

- [x] `packages/desktop/package.json`：`dependencies` 新增 `"@xyflow/react": "^12.11.0"`，`version` 由 `0.3.2` 升为 `0.3.3`，并安装依赖；确认 `src-tauri/tauri.conf.json`（引用 `../package.json`）与 `src-tauri/Cargo.toml` 零改动

## 阶段二：flow 纯函数转换层（零 react / 零 @xyflow/react 依赖）

- [x] 新增 `packages/desktop/src/views/changes/flow/types.ts`：`FlowNodeKind` / `FlowEdgeKind` / `FlowNode` / `FlowEdge` / `FlowColumn` / `FlowGraph` / `FlowMaterials` / `DrawerSelection` 类型，附节点 id 方案注释（`col:<phase>` / `eval:<phase>:<attempt>` / `active:…` / `interrupted:…`）
- [x] 新增 `packages/desktop/src/views/changes/flow/layout.ts`：`PIPELINE_PHASES` 前端镜像常量（与 Rust 端 9 站同序）、`COL_W = 260`、`ROW_H = 128`、`COLUMN_HEADER_H = 48`、`COLUMN_PAD_X = 18`、`COLUMN_PAD_Y = 16`，导出 `nodePosition(node: FlowNode): { x, y }`（列容器绝对坐标、事件节点相对父列坐标）
- [x] 新增 `packages/desktop/src/views/changes/flow/graph.ts`：导出 `buildFlowGraph(detail: ChangeDetail): FlowGraph`——三分类事件收集（eval 按站序 + attempts 序、active、interrupted）→ O(n) 时间序归并（eval 追加序为主链，interrupted / active 按 `startAt` 插入首个锚点更晚的骨干事件之前，时间戳 `Date.parse` 失败或缺失按链尾列表序追加）→ 链式边推导（ΔcolIndex 符号 → `forward` / `retry` / `backtrack`，回跳边 label 取目标节点 `record.backtrackReason`）；`attempt` 缺号 0 兜底；`pipeline` 为空（v0）返回空图
- [x] 新增 `packages/desktop/src/views/changes/flow/attachments.ts`：导出 `mountMaterials(graph, detail, envelopes): FlowMaterials`；模块私有 `docColumn(source)` 静态映射（`proposal.md`、`specs/**` → proposal；`design.md`、`tasks.md` → dev-design；`test-reports/**` → test-execution；`explore.md` 与映射表外不入图）；eval-checklist 按 payload `{ phase, attempt }` 最小守卫挂节点；file_log `scope`=phase id 且带 `attempt` 挂节点（eval 优先，其次 active、interrupted）；`scope='workflow'` 与未命中条目归 `outsideFiles`

## 阶段三：流程图渲染层

- [x] 新增 `packages/desktop/src/views/changes/flow/PhaseColumnNode.tsx`：列容器自定义节点（nodeTypes 键 `column`）——列头 phase 名 + `columnDocs` 过程文档徽章，点击经 `onSelect({ scope: 'column', phase })`
- [x] 新增 `packages/desktop/src/views/changes/flow/FlowEventNode.tsx`：事件自定义节点（nodeTypes 键 `event`）——eval 实心（pass 绿 / fail 红 token，`stale` 半透明，`skipped` / backtrack 徽标文案）、active pulse、interrupted dashed 灰显（startAt ~ endAt），点击经 `onSelect({ scope: 'node', nodeId })`
- [x] 新增 `packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx`：组件级 `import '@xyflow/react/dist/style.css'`；`nodes`/`edges` 经 `nodePosition` 与父子坐标换算映射为 xyflow 节点（`parentId` + `extent: 'parent'`，列容器在前）；边视觉（回跳 dashed + warn 色 + label）；`nodesDraggable` / `nodesConnectable` 关闭；`fitView`（`padding: 0.1`、`maxZoom: 1`、`minZoom: 0.15`）+ `<Background variant="dots" />`；`onNodeClick` → `onSelect`；data-testid：`flow-graph` / `flow-column` / `flow-node`
- [x] 新增 `packages/desktop/src/views/changes/flow/FileLogTable.tsx`：op / scope / attempt / path / at 五列表体（沿用 `components/ui/table` 与 `filelog-table` 挂钩；空数组渲染「（空）」，`attempt` / `at` 空缺渲染「—」）
- [x] 新增 `packages/desktop/src/views/changes/flow/DetailDrawer.tsx`：右侧抽屉（固定宽 + 遮罩关闭 + 关闭按钮，`selection` 为 null 不渲染），三分节 data-testid `drawer-docs-section` / `drawer-eval-section` / `drawer-files-section`——本站文档节经 `ArtifactView`（renderers/registry）渲染 `columnDocs[phase]`；eval 节渲染 `record.report` + checklist（有挂载信封走 `ArtifactView`，无则内联 `record.checklist`）；文件表节按选中域渲染 `nodeFiles[nodeId]` 或空态（v1 降级文案保留）

## 阶段四：页面组装

- [x] 改造 `packages/desktop/src/views/changes/ChangeDetailView.tsx`：拆除 `Station` / `Attempt` / `VerdictBadge` / `DetailSectionInterupted` / `DetailSectionFileList`，改为 Header + （v0 空图占位（`flow-empty`，保留「v0 早期代际」文案）/ `ChangeFlowGraph`）+ workflow 独立面板（`fileLog !== null` 时渲染 `outsideFiles`，`workflow-panel` 挂钩）+ `DetailSectionArtifacts` 产物区 + `DetailDrawer`（`selection` 本地 state）；`DetailHeader` / `DetailFallback` / `unparsable` 警示条 / `INVENTORY_VARIANT` / `formatTime` 保留；error / loading / 未找到降级页不变

## 阶段五：守线自检（静态，不含测试执行）

- [x] `pnpm -C packages/desktop run client:check` 全绿：fmt / lint（含 `max-lines-per-function: 50`）/ knip 无新增豁免条目（`docColumn` 保持模块私有，无仅测试引用的导出）
- [x] 确认取数边界零改动（`useChangeDetail.ts`、`renderers/`、`detail.rs`、`tauri.conf.json`、`src-tauri/Cargo.toml` 无 diff）
