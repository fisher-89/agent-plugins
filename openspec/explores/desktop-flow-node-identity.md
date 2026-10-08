# desktop 流程图节点身份复用探索（根治重建闪没）

日期:2026-10-08 · 状态:分析完成,待开 change

## 动机

修复「进行中的流程,进入详情页后一瞬间流程图消失(DOM 中所有泳道被置 hidden)」时,
以两个定点修复收口(见当次提交):

1. **runNodes 依赖收窄**:`change-detail-view.tsx` 的 runNodes memo 以
   `run.state?.steps` 为依赖(sessionEvent 高频流入不动 steps 引用 → 图零重建);
   `runStepNodes` 入参随之收窄为 steps 表本身。
2. **列声明静态尺寸**:`toChartNodes` 列节点携带 `width`/`height` 节点属性,
   可见性判据 `nodeHasDimensions` 优先读声明尺寸,泳道不依赖测量。

两者掐掉了最高频的触发面,但闪没的根因——**节点对象整体重建即丢 measured**——
仍在,残余场景见下。本文记录根治方向(对象身份复用)的设计探索。

## 根因机制(xyflow v12 受控节点语义)

- NodeWrapper 以 `visibility: hasDimensions ? 'visible' : 'hidden'` 渲染节点包裹层;
  `hasDimensions = nodeHasDimensions(node)`,判据取
  `node.measured?.width ?? node.width ?? node.initialWidth`(height 同)。
- 受控 `nodes` prop 归并走 `adoptUserNodes`:节点对象与上一轮**引用相等**才保留
  internal node(含 measured);引用变了即重建,`measured` 重置为 undefined,
  **不从旧测量继承**。未测量窗口内节点 `visibility: hidden`,等 ResizeObserver
  下一帧重测才恢复——视觉即「整图闪没一瞬间」。
- 我们的 `toChartNodes` memo 依赖 `[graph, materials, onSelect]`,任一变身份就
  从零重建全部节点对象(列 / 事件 / 运行步无一幸免),永远命中不了引用相等快路径。

## 修复 1+2 之后的残余(方向 3 的标的)

| 场景 | 触发 | 现状 |
|------|------|------|
| 产物二段到达 | `useChangeDetail` 先 `setDetail` 后 `setArtifacts` → materials 重建 → 全量节点重建 | 事件/运行步节点闪一帧(泳道凭声明尺寸不闪) |
| 快照到达 | `change_flow_state` 落定 → runNodes 空数组换身份 → graph 重建 | 同上 |
| 真内容变化 | run 步并入活动列(列高增长)、active 按 startAt 插入挤后序 order、节点状态跃迁 | 变化节点各闪一帧(内容本在变,视觉感弱) |

sessionEvent 流入面已被修复 1 掐掉(原先最高频的持续闪没源)。

## 方向 3 设计草图:逐 id 对象复用

核心:**重建时内容未变的节点沿用上一轮对象**,命中 xyflow 引用相等快路径,
measured 全程保留。

- 落点在 `toChartNodes`(graph→chart 映射边界):组件内以 ref 持上一轮
  `Map<id, ChartNode>`,新构建出的候选与旧对象逐内容比
  (id/type/position/parentId/width/height/style/data 深比一层),相等即复用旧引用。
- data 引用不稳定是主要难点:`buildFlowGraph`/`mountMaterials` 每轮全量新建
  (eval 节点 record、columnDocs 数组等),复用判据必须是内容比而非引用比,
  或把身份稳定贯穿 graph → materials 两层先行(改动面更大,收益同)。
- 列已走声明尺寸,复用面只需覆盖事件 / 运行步两类节点。

### 备选:受控回调面

改用 `onNodesChange` + `applyNodeChanges` 维护节点 state(尺寸变更回合回写
`node.measured`),重建对象时天然携带 measured。代价:引入节点 state 与
xyflow change 协议的整条维护面,与「渲染薄层」定位(坐标全部来自
nodePosition,无布局运算)相悖,倾向不做。

## 验收形态

- 进入进行中流程详情页:产物 / 快照 / watch 信封任意时序到达,已呈现节点
  永不闪回 hidden(DOM visibility 断言,同本次两锚点的形态)。
- run 步并入活动列:变化节点与被挤位的节点随内容更新,未变化节点零闪。
- 既有回归锚(`run-state.test` steps 引用、`change-flow-graph.test` 声明尺寸、
  `change-detail-view.test` sessionEvent 零重建)全绿。
