# 测试设计: change-detail-react-flow

> **日期**: 2026-09-24

---

## 验收范围

<!-- 测试框架：vite-plus（vp test，vitest API + jsdom 环境，测试文件与源文件同目录共置）。
   集成测试文件放置于测试区域 src/__tests__/（沿 ipc_pipeline.test.tsx 既有进程边界 mock 模式）。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 转换层坐标公式：x = `PIPELINE_PHASES` 列索引 × COL_W，y = 列内执行序 × ROW_H；列数恒为 9，未走的站呈现空列 | 单元测试 | `packages/desktop/src/views/changes/flow/graph.test.ts`、`packages/desktop/src/views/changes/flow/layout.test.ts` |
| AC-2 | 节点三分类：eval 实心（pass 绿 / fail 红、stale 半透明）、active pulse 运行中、interrupted dashed 灰显且独立成节点不与同号 eval 合并 | 单元测试 | `packages/desktop/src/views/changes/flow/graph.test.ts`、`packages/desktop/src/views/changes/flow/FlowEventNode.test.tsx` |
| AC-3 | 边推导与「布局即语义」：每节点入边来自时间上紧邻的前一事件（eval 追加序主序，interrupted / active 按 start_at 归并）；向右=前进、同列=重试、向左=回跳（虚线 + backtrack_reason 边标签） | 单元测试 | `packages/desktop/src/views/changes/flow/graph.test.ts` |
| AC-4 | 素材挂载：过程文档按静态映射表挂列；eval-checklist 按 payload phase+attempt 挂节点；file_log `scope=phase+attempt` 挂 attempt 节点；`scope='workflow'` 与 `explore.md` 不入图 | 单元测试 | `packages/desktop/src/views/changes/flow/attachments.test.ts` |
| AC-5 | 抽屉单一交互入口：列头与节点点击打开同一右侧抽屉，分节【本站文档 / eval report+checklist / 文件表】，文档经 renderers/registry 渲染 | 集成测试 | `flow/graph.ts` + `flow/attachments.ts` + `flow/ChangeFlowGraph.tsx` + `flow/DetailDrawer.tsx`（→ `src/__tests__/change_flow_pipeline.test.tsx`） |
| AC-6 | 三代际降级：v0 空图占位 + 产物区文档列表；v1 图正常画、节点无文件区（fileLog=null）；v2 完整图 | 集成测试 | `ChangeDetailView.tsx` + `flow/ChangeFlowGraph.tsx` + `flow/FileLogTable.tsx` + `flow/DetailDrawer.tsx`（→ `src/__tests__/change_flow_generations.test.tsx`） |
| AC-7 | 取数模型不变：详情更新仅由显式 refresh 触发，无 watch / 轮询 / 事件订阅 | 集成测试 | `hooks/useChangeDetail.ts` + `ChangeDetailView.tsx` + `flow/graph.ts` + `flow/ChangeFlowGraph.tsx`（→ `src/__tests__/change_flow_refresh.test.tsx`） |
| AC-8 | 转换层测试覆盖：纯函数直测覆盖前进 / 重试 / 回跳 / stale / interrupted / active / attempt 缺号 / 时间戳缺失 / 归属映射边界；react-flow 渲染语义不做逐项断言 | 单元测试 | `packages/desktop/src/views/changes/flow/graph.test.ts`、`packages/desktop/src/views/changes/flow/layout.test.ts`、`packages/desktop/src/views/changes/flow/attachments.test.ts` |
| AC-9 | 管线合规：client:check（fmt / lint 含 max-lines-per-function: 50 / knip）与测试全绿；无新增豁免条目；测试挂钩均为 data-testid | 不可测试 | —（静态守线门禁，见不可测试项） |
| AC-10 | `packages/desktop/package.json` version 升为 0.3.3；tauri.conf.json 经引用自动跟随（无改动）；src-tauri/Cargo.toml 不随动（保持 0.1.0） | 不可测试 | —（静态配置交付约束，见不可测试项） |

---

## 单元测试

<!-- 覆盖前端 Vitest 纯函数 / 组件 mock 测试（框架 vite-plus，jsdom 环境）。每个源文件对应一个独立章节。 -->

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能

- buildFlowGraph(): 详情聚合 → 流程图模型（9 列容器 + 三分类事件节点 + 链式边）；`pipeline` 为空（v0）时返回空图（无列无节点）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| buildFlowGraph：列与坐标骨架 | 正向 | v2 完整详情输出恒 9 列容器（columns 长度 9、colIndex 0..8、id `col:<phase>`、phase 与 `PIPELINE_PHASES` 前端镜像同序），且列容器在前、事件节点按归并序在后 | 新增 |
| buildFlowGraph：列与坐标骨架 | 正向 | 单站单 attempt：该站恰 1 个事件节点（order=0），其余 8 站为空列（列容器存在、无事件节点） | 新增 |
| buildFlowGraph：列与坐标骨架 | 边界 | `pipeline` 为空数组（v0 形态）→ 返回空图：columns / nodes / edges 全空 | 新增 |
| buildFlowGraph：列与坐标骨架 | 异常 | unparsable 详情（workflow 损坏的异常形态，各站 attempts 全空）→ 恒 9 空列、零事件节点、零边，不抛错 | 新增 |
| buildFlowGraph：列与坐标骨架 | 边界 | attempt 为 null 的 eval 记录 → 节点 id 与 attempt 字段按 0 兜底（与后端 `unwrap_or(0)` 同款） | 新增 |
| buildFlowGraph：节点三分类 | 正向 | eval 记录 → `kind='eval'` 节点携带 record（AttemptRecord 原样）、`parentId='col:<phase>'` | 新增 |
| buildFlowGraph：节点三分类 | 正向 | activePhase 非空 → `kind='active'` 节点（无 verdict），列内接流末端（该列 order 最大） | 新增 |
| buildFlowGraph：节点三分类 | 正向 | interrupted 非空 → `kind='interrupted'` 独立节点携带 startAt / endAt，与同号 eval 节点并存不合并（v2-b 夹具形态） | 新增 |
| buildFlowGraph：节点三分类 | 边界 | interrupted 与 activePhase 同时存在 → 两类节点共存、各自恰一条入边 | 新增 |
| buildFlowGraph：时间序归并与边推导 | 正向 | 多站 eval 主链：每个非首事件恰一条入边（id `edge:<source>-><target>`），kind 按端点列差派生——列差 >0 为 forward（前进）、=0 为 retry（多 attempt 重试）、<0 为 backtrack（回跳） | 新增 |
| buildFlowGraph：时间序归并与边推导 | 正向 | backtrack 边 label = 目标节点 `record.backtrackReason`；reason 为 null 的回跳边 label 为 null（不渲染字符串 "null"） | 新增 |
| buildFlowGraph：时间序归并与边推导 | 正向 | interrupted / active 按 startAt（Date.parse 毫秒）插入骨干中第一个锚点严格晚于它的 eval 事件之前（时间序归并） | 新增 |
| buildFlowGraph：时间序归并与边推导 | 边界 | 归并后序列头（首个事件）无入边 | 新增 |
| buildFlowGraph：时间序归并与边推导 | 边界 | startAt 为 null 的 interrupted / active → 按列表序（先 interrupted[] 后 active）追加链尾 | 新增 |
| buildFlowGraph：时间序归并与边推导 | 异常 | startAt 为非法时间串（Date.parse 产出 NaN）→ 视同 null 处理（追加链尾），不产生 NaN 比较、不抛错 | 新增 |
| buildFlowGraph：时间序归并与边推导 | 边界 | 骨干 eval 事件锚点时间戳为 null → 不作插入参考，startAt 插入点跳过 null 锚点 | 新增 |
| buildFlowGraph：时间序归并与边推导 | 异常 | eval 时间戳乱序（追加序 ≠ 时间序）→ 仍按站序 + 站内 attempts 序串联主链，不按时间重排（追加序即事实执行序） | 新增 |
| buildFlowGraph：时间序归并与边推导 | 边界 | startAt 早于全部 eval 锚点 → 插入为序列头（此时该节点无入边） | 新增 |

#### Mock策略

<!-- 无 Mock：转换层为零 react / 零 @xyflow/react / 零 invoke 的纯函数，用例以手工构造的 ChangeDetail DTO fixture（v2-a / v2-b 夹具形态）直接喂入公共入口。 -->

### packages/desktop/src/views/changes/flow/layout.ts -> packages/desktop/src/views/changes/flow/layout.test.ts

#### 待测功能

- nodePosition(): react-flow 就绪坐标——列容器为画布绝对坐标，事件节点为相对父列坐标

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| nodePosition | 正向 | 列容器节点 x = colIndex × COL_W、y = 0（colIndex 0 / 4 / 8 抽样验证公式） | 新增 |
| nodePosition | 正向 | 事件节点 x = COLUMN_PAD_X、y = COLUMN_HEADER_H + order × ROW_H（常量取值 18 / 48 / 128，与 design 坐标约定一致） | 新增 |
| nodePosition | 边界 | order=0（列内首个事件）→ y 恰为 COLUMN_HEADER_H | 新增 |
| nodePosition | 边界 | 末列 colIndex=8 → x 恰为 8 × COL_W（画布总宽边界，≈2340px） | 新增 |
| nodePosition | 异常 | 超大 order（如 1000）→ 坐标按公式线性外推，无钳制、无 NaN、不抛错 | 新增 |

#### Mock策略

<!-- 无 Mock：布局常量（COL_W / ROW_H / COLUMN_HEADER_H / COLUMN_PAD_X / COLUMN_PAD_Y）与纯坐标函数直测；常量取值经 nodePosition 输出间接断言，不单列导出断言。 -->

### packages/desktop/src/views/changes/flow/attachments.ts -> packages/desktop/src/views/changes/flow/attachments.test.ts

#### 待测功能

- mountMaterials(): 三类素材（文档→列、eval-checklist→节点、file_log→节点）按挂载规则归位；未命中条目归 outsideFiles，不丢信息

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mountMaterials：文档挂列 | 正向 | 静态映射全命中：`proposal.md` 与 `specs/` 前缀（含深路径 `specs/x/spec.md`）→ proposal 列；`design.md`、`tasks.md` → dev-design 列；`test-reports/` 前缀 → test-execution 列（columnDocs 键 `col:<phase>`，markdown-doc 与 tasks-progress 两 kind 均走映射） | 新增 |
| mountMaterials：文档挂列 | 异常 | 映射表外文档（`explore.md`、旧代际 `phases/**`、未知路径）→ 不出现在任何 columnDocs（不入图，产物区全量列表兜底） | 新增 |
| mountMaterials：文档挂列 | 边界 | 每站一份不随 attempt 重复：同源文档单信封 → 该列 columnDocs 恰 1 条 | 新增 |
| mountMaterials：记录挂节点 | 正向 | eval-checklist 信封 payload `{ phase, attempt }` → 按节点 id 方案定位 eval 优先节点，入 nodeChecklists[nodeId] | 新增 |
| mountMaterials：记录挂节点 | 正向 | file_log 条目 scope 为 9 站 phase id 且 attempt 非 null → 入 nodeFiles[nodeId] | 新增 |
| mountMaterials：记录挂节点 | 异常 | eval-checklist payload 形状不合规（payload 为 null、缺 phase / attempt 字段、字段类型不符）→ 不挂载任何节点（最小形状守卫收窄，不误挂） | 新增 |
| mountMaterials：记录挂节点 | 异常 | file_log `scope='workflow'` 条目 → outsideFiles，不出现在任何 nodeFiles | 新增 |
| mountMaterials：记录挂节点 | 异常 | file_log scope 为 phase id 但 attempt=null → outsideFiles | 新增 |
| mountMaterials：记录挂节点 | 边界 | 同号仅 active / 仅 interrupted（eval 缺席）时 checklist 定位依次降级：eval → active → interrupted | 新增 |
| mountMaterials：记录挂节点 | 边界 | scope 为非 9 站字符串（未知 phase）或该站无任何事件节点 → outsideFiles | 新增 |
| mountMaterials：聚合与空态 | 正向 | 同列多文档、同节点多素材 → 数组聚合保序不覆盖 | 新增 |
| mountMaterials：聚合与空态 | 边界 | 全空入参（envelopes=[]、fileLog=[]）→ FlowMaterials 四键齐全且全空，不抛错 | 新增 |
| mountMaterials：聚合与空态 | 异常 | fileLog=null（字段缺失形态）→ 与空数组同型全空输出，不抛错 | 新增 |

#### Mock策略

<!-- 无 Mock：挂载层为纯函数；文档→列映射表 docColumn 为模块私有（不导出，避免 knip 判未用），其归属规则全部经 mountMaterials 公共输出覆盖测试。 -->

### packages/desktop/src/views/changes/flow/FileLogTable.tsx -> packages/desktop/src/views/changes/flow/FileLogTable.test.tsx

#### 待测功能

- FileLogTable(): file_log 条目表体（op / scope / attempt / path / at），workflow 独立面板与抽屉文件表节共用

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| FileLogTable | 正向 | 条目逐行渲染 op / scope / attempt / path / at 五列矩阵，沿用 `filelog-table` testid 与 row / cell 语义查询断言 | 新增 |
| FileLogTable | 边界 | 空数组 → 「（空）」占位，无 `filelog-table` 表格 | 新增 |
| FileLogTable | 异常 | attempt=null 与 at=null 的条目 → 「—」占位符（不渲染 "null" 字面量） | 新增 |

#### Mock策略

<!-- 无 Mock：纯展示组件，shadcn/ui Table 直接渲染。 -->

### packages/desktop/src/views/changes/flow/FlowEventNode.tsx -> packages/desktop/src/views/changes/flow/FlowEventNode.test.tsx

#### 待测功能

- FlowEventNode(): 事件自定义节点（nodeTypes 键 `event`），按 kind 三分类视觉：eval 实心（pass 绿 / fail 红、stale 半透明、backtrack 徽标）/ active（pulse）/ interrupted（dashed 灰显）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| FlowEventNode：eval 节点 | 正向 | verdict pass → 实心 + pass 徽标文案；verdict fail → fail 徽标文案（迁移旧 VerdictBadge 语义） | 新增 |
| FlowEventNode：eval 节点 | 正向 | stale=true → 半透明淡化标记（stale 类名 / 视觉标记存在），节点信息仍完整可读 | 新增 |
| FlowEventNode：eval 节点 | 边界 | 全缺省 eval（无 skipped / stale / backtrack、无 startAt / timestamp）→ 无任何徽标与淡化（负分支，不渲染空占位） | 新增 |
| FlowEventNode：eval 节点 | 异常 | attempt 为 0（缺号兜底形态）→ 节点渲染占位标识不抛错（迁移旧「attempt —」语义按兜底值呈现） | 新增 |
| FlowEventNode：eval 节点 | 边界 | skipped / backtrackTo / backtrackReason 存在 → 徽标文案保留（「↩ 回跳至 X：reason」，to 为 null 时「?」，reason 为 null 时不拼接「：null」——四种组合迁移旧 Attempt 语义） | 新增 |
| FlowEventNode：interrupted / active 节点 | 正向 | interrupted → dashed 边框灰显标记 + startAt ~ endAt 文案（迁移旧中断留档语义） | 新增 |
| FlowEventNode：interrupted / active 节点 | 正向 | active → pulse 运行中标记且无 verdict 徽标 | 新增 |
| FlowEventNode：interrupted / active 节点 | 边界 | interrupted startAt 与 endAt 双 null → 两侧均「—」占位（迁移旧 formatTime 语义） | 新增 |
| FlowEventNode：interrupted / active 节点 | 异常 | 以最小 NodeProps 形态直渲染三种 kind（interrupted / active 无 record 字段）→ 各自取用独有字段不抛错 | 新增 |

#### Mock策略

<!-- 无 Mock：自定义节点为纯渲染组件，以最小 NodeProps 形态（data 注入 FlowNode 载荷）直接 render；不挂载 ReactFlow 本体，无 ResizeObserver 依赖。 -->

### packages/desktop/src/views/changes/flow/PhaseColumnNode.tsx -> packages/desktop/src/views/changes/flow/PhaseColumnNode.test.tsx

#### 待测功能

- PhaseColumnNode(): phase 列容器节点（nodeTypes 键 `column`）：列头 phase 名 + 该站过程文档徽章，点击打开抽屉

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| PhaseColumnNode | 正向 | 列头渲染 phase 名，点击触发 onSelect 回调且载荷为 `{ scope: 'column', phase }` | 新增 |
| PhaseColumnNode | 正向 | 该站有过程文档（FlowMaterials.columnDocs 命中）→ 徽章呈现该站文档计数 | 新增 |
| PhaseColumnNode | 边界 | 空列（无事件节点、无文档）→ 列头照常渲染、无徽章（不渲染空占位堆叠） | 新增 |
| PhaseColumnNode | 异常 | materials 中该列 `columnDocs` 键缺失（undefined 引用）或 onSelect 回调未传入 → 列头照常渲染不抛错，徽章按零态处理、点击为 no-op | 新增 |

#### Mock策略

<!-- 无 Mock：直渲染 + spy 回调；与 ReactFlow subflow 的挂载关系由集成测试覆盖。 -->

### packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx -> packages/desktop/src/views/changes/flow/ChangeFlowGraph.test.tsx

#### 待测功能

- ChangeFlowGraph(): ReactFlow 薄层——挂载、subflow（parentId + extent: 'parent'）、nodePosition 映射、边视觉、fitView、onNodeClick → onSelect 上抛

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeFlowGraph | 正向 | 输入真实 buildFlowGraph 产出 → 渲染全部列容器与事件节点（`flow-graph` / `flow-column` / `flow-node` testid 计数与 graph 一致） | 新增 |
| ChangeFlowGraph | 正向 | 点击事件节点 → onSelect 收到 `{ scope: 'node', nodeId }`；点击列头 → `{ scope: 'column', phase }`（DrawerSelection 判别 union 两分支） | 新增 |
| ChangeFlowGraph | 边界 | 空 graph（columns / nodes 为空数组）→ 渲染空画布不抛错 | 新增 |
| ChangeFlowGraph | 边界 | graph props 引用变化（显式 refresh 后新 graph）→ 重渲染并重新 fitView，无残留旧节点 | 新增 |
| ChangeFlowGraph | 异常 | label 为 null 的常规边与带 label 的回跳边混存 → 均正常渲染不抛错（label null 不渲染标签节点） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ResizeObserver（运行环境） | `vi.stubGlobal('ResizeObserver', stub 类)`（沿 AppSidebar.test.tsx 既有 stub 模式），teardown 时 `vi.unstubAllGlobals()` | 全部用例（ReactFlow 在 jsdom 下挂载依赖该浏览器 API，design 已备案） |
| 其余 jsdom 缺失浏览器 API | 如挂载报错缺 DOMMatrixReadOnly 等，按需补 stub——环境垫片而非业务 mock | ReactFlow 挂载用例 |

### packages/desktop/src/views/changes/flow/DetailDrawer.tsx -> packages/desktop/src/views/changes/flow/DetailDrawer.test.tsx

#### 待测功能

- DetailDrawer(): 右侧抽屉三分节【本站文档 / eval report+checklist / 文件表】，列头与节点点击共用同一入口；selection 为 null 时不渲染

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| DetailDrawer | 正向 | selection 为 eval 节点 → 三分节齐备（`drawer-docs-section` / `drawer-eval-section` / `drawer-files-section` testid）：文档节经 ArtifactView 渲染该列文档、eval 节渲染 record.report、文件表节经 FileLogTable 渲染 nodeFiles | 新增 |
| DetailDrawer | 正向 | selection 为列头（scope column）→ 本站文档节渲染该列文档，eval 节与文件表节呈空态 | 新增 |
| DetailDrawer | 正向 | 遮罩点击或关闭按钮 → onClose 回调触发 | 新增 |
| DetailDrawer | 异常 | selection=null → 组件返回 null 不渲染（无 `detail-drawer` testid） | 新增 |
| DetailDrawer | 边界 | eval 节点无挂载信封（读取降级 payload null 不可匹配）→ 回退内联渲染 record.checklist 条目（item / evidence / pass 徽标，迁移旧 checklist 展示语义） | 新增 |
| DetailDrawer | 边界 | active / interrupted 节点选中 → eval 节空态（无 report / checklist 内容） | 新增 |
| DetailDrawer | 边界 | v1 代际（无 file_log 数据）→ 文件表节呈「无 file_log 数据：v1 及更早代际无此字段」降级文案（迁移旧文案） | 新增 |

#### Mock策略

<!-- 无跨进程 Mock：ArtifactView（renderers/registry）与 FileLogTable 均以真实实现消费——链路内模块间调用不 mock；信封与 file_log 条目以 fixture 构造。 -->

### packages/desktop/src/views/changes/flow/types.ts -> packages/desktop/src/views/changes/flow/types.test.ts

<!-- design.md 未声明该文件的公共 API 变更：八个导出（FlowNodeKind / FlowEdgeKind / FlowNode / FlowEdge / FlowColumn / FlowGraph / FlowMaterials / DrawerSelection）均为编译期类型，无运行时行为。本章节不设用例，且不应创建 types.test.ts 文件（零用例测试文件会导致运行器报错）——类型正确性由消费方模块（graph.ts / attachments.ts / 组件层）的用例在编译期覆盖。 -->

### packages/desktop/src/views/changes/ChangeDetailView.tsx -> packages/desktop/src/views/changes/ChangeDetailView.test.tsx

<!-- design.md 公共函数 / API 表未声明该文件的新增导出：ChangeDetailView 为既有导出，行为变更见 design「变更清单 · 修改文件」行（Header + 流程图区 + workflow 独立面板 + 产物区 + 抽屉挂载；DetailHeader / DetailFallback / unparsable 警示 / INVENTORY_VARIANT / formatTime 原样保留）。以下按该行描述的页面组装语义列条目，既有断言逐条迁移而非删除。 -->

#### 待测功能

- ChangeDetailView(): 页面组装——Header + 流程图区（v0 空图占位）+ workflow 独立面板 + 产物区 + 抽屉挂载（selection 本地 state）
- ChangeDetailView().降级: error / loading / 未找到三态 DetailFallback 与 unparsable 警示条原样保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeDetailView：页面组装 | 正向 | v2 详情 → `flow-graph` 图区、`workflow-panel` 面板（内含 `filelog-table`）、产物区三者并存 | 新增（迁移） |
| ChangeDetailView：页面组装 | 正向 | Header 元信息迁移断言：name / inventory 徽标（INVENTORY_VARIANT 域）/ source（进行中 · 已归档）/ created（null 不渲染空节点）/ activePhase 运行中 badge（startAt null 不拼时间与占位） | 新增（迁移） |
| ChangeDetailView：页面组装 | 正向 | 列头 / 节点点击 → `detail-drawer` 挂载（selection 状态在本组件）；关闭后卸载 | 新增 |
| ChangeDetailView：页面组装 | 正向 | unparsable 警示条正负两例（`warn-note` 有 / 无，文案「workflow.json 无法解析」） | 新增（迁移） |
| ChangeDetailView：页面组装 | 正向 | 产物区按信封顺序渲染 ArtifactView 列表；空清单「（未发现可读产物）」占位 | 新增（迁移） |
| ChangeDetailView：页面组装 | 正向 | 点击「← 返回列表」回调触发；点击「刷新详情」触发 refresh 回调 | 新增（迁移+补） |
| ChangeDetailView：页面组装 | 边界 | v0（inventory v0 + pipeline 空）→ 不挂 `flow-graph`，渲染 `flow-empty` 占位文案「v0 早期代际：无 workflow.json，仅文档形态」，产物区不受图区影响照常 | 新增（替代旧 v0 用例） |
| ChangeDetailView：页面组装 | 边界 | v1（fileLog null）→ `flow-graph` 正常渲染且无 `workflow-panel` | 新增 |
| ChangeDetailView：页面组装 | 异常 | error / loading（无 detail）/ 未找到三态降级页与 `detail-note` / `error-note` 挂钩归属正确 | 新增（迁移） |
| ChangeDetailView：页面组装 | 边界 | loading 中已有 detail 不回落加载占位、刷新按钮禁用 | 新增（迁移） |
| ChangeDetailView：旧线性布局断言退役 | 异常 | 旧线性布局断言整体退出——「N 次尝试」/「（无记录）」/ attempt-meta · attempt-verdict · checklist · backtrack 页面级挂钩 / 独立「中断留档」区块 / 页面级 filelog-table 矩阵 / 「（无评估记录）」空流水线文案不再在本文件断言，语义分别迁往 graph.test.ts、FlowEventNode.test.tsx、FileLogTable.test.tsx 与 `__tests__` 集成用例 | 废弃 |

#### Mock策略

<!-- 无跨进程 Mock：组件以 ChangeDetailState 手工 fixture 直渲染（既有文件同款模式）；Tauri invoke 的进程边界 mock 仅在 __tests__ 集成用例中使用，单测层 state 由 fixture 直供。 -->

---

## 集成测试

<!-- 聚焦模块组合时才暴露的行为；链路内模块间调用一律真实实现，仅进程边界（Tauri invoke）与运行环境（ResizeObserver）mock。集成测试文件放置于 packages/desktop/src/__tests__/。 -->

### buildFlowGraph + mountMaterials → ChangeFlowGraph → DetailDrawer 抽屉链路 → `packages/desktop/src/__tests__/change_flow_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/changes/flow/graph.ts` | 转换方（detail → FlowGraph） |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 挂载方（graph + detail + envelopes → FlowMaterials） |
| `packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx` | 渲染方 / 触发方（ReactFlow 挂载，点击 → onSelect） |
| `packages/desktop/src/views/changes/flow/DetailDrawer.tsx` | 消费方（DrawerSelection → 三分节内容） |

**关联AC**: AC-2, AC-3, AC-4, AC-5

**关系描述**:

页面核心链路是同一份 ChangeDetail fixture 经 buildFlowGraph 得图、经 mountMaterials 挂素材，整包喂给 ChangeFlowGraph 渲染，用户点击列头 / 节点产生 DrawerSelection，DetailDrawer 按选中对象组装三分节内容。四个模块各自单测隔离了内部逻辑，但三处接缝只有组合才暴露：mountMaterials 的 nodeChecklists / nodeFiles 键必须与图节点 id 方案逐字一致（id 拼写漂移会让素材静默失挂且单测各自全绿）、onSelect 上抛的 nodeId 必须能在 materials 中反查、DrawerSelection 的 scope 判别字段必须与抽屉分支对齐。典型出错模式是「各自单测全绿、链路不通」：素材挂到不存在的节点键、点击回调上抛载荷与抽屉消费的形状不符、stale / interrupted 节点在真实渲染树中不可交互。

#### 场景: v2 完整链路——素材上墙与抽屉取材

以 v2-b 形态 fixture（eval 多 attempt + backtrack + stale + interrupted + active，文档信封 + eval-checklist 信封，phase-scope 与 workflow-scope file_log 混合）构造 detail 与 envelopes，经真实 buildFlowGraph / mountMaterials 得 graph + materials 后整包渲染。点击 proposal 列 eval fail 节点：预期图上三分类节点与边形态正确（AC-2 / AC-3），抽屉三分节分别呈现该列文档（AC-4 挂列，不随 attempt 重复）、record.report 与挂载 checklist 信封、该 attempt 文件表（AC-4 挂节点 → AC-5 取材）；workflow-scope 条目不出现在任何抽屉文件表节。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 点击 eval 节点 → 抽屉三分节内容与该节点 phase + attempt 素材精确对应（文档挂列、checklist 与 file_log 挂节点、经 registry 渲染） | 新增 |
| 正向 | 点击列头 → 抽屉呈该站文档节，eval 节与文件表节空态（同一抽屉入口，`detail-drawer` 恰一实例） | 新增 |
| 边界 | backtrack 边（向左、虚线标记、backtrackReason 标签）与 retry / forward 边在真实渲染树中可查询，方向与列差一致 | 新增 |
| 边界 | stale eval 节点呈淡化标记且仍可点击打开抽屉（废弃支线信息不丢）；interrupted 节点与同号 eval 并存可分别选中 | 新增 |
| 异常 | 点击无素材对象（空站列容器、无挂载信封的 eval）→ 抽屉三分节全空态不抛错 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| ResizeObserver（运行环境） | `vi.stubGlobal('ResizeObserver', stub 类)`，teardown unstub | 全部场景（ReactFlow jsdom 挂载依赖） |

#### 场景: workflow 独立面板与图外素材

同一 fixture 下，scope='workflow' 与未命中条目只应出现在图外消费面，任何列徽章与节点文件表节都不得包含它们。前置：ChangeDetailView 整页渲染（含 workflow-panel）。预期：`workflow-panel` 内 `filelog-table` 行集等于 outsideFiles 全集；图内任意抽屉文件表节不含 workflow scope 条目——「不入图」与「信息不丢」同时成立。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | workflow 面板逐行渲染 outsideFiles 全集（v2 workflow-scope 条目不缺行） | 新增 |
| 边界 | 未命中节点的 phase-scope 条目（attempt null / 未知 phase）落入面板而非图内任何节点 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| ResizeObserver（运行环境） | `vi.stubGlobal('ResizeObserver', stub 类)`，teardown unstub | 涉及图区挂载的场景 |

### ChangeDetailView 页面组装 → 三代际降级分支 → `packages/desktop/src/__tests__/change_flow_generations.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/changes/ChangeDetailView.tsx` | 组装方 / 分支方（inventory / pipeline / fileLog 三处判定） |
| `packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx` | 图渲染方（v0 不挂载，v1 / v2 挂载） |
| `packages/desktop/src/views/changes/flow/FileLogTable.tsx` | 面板表体（workflow 面板与抽屉文件表节共用） |
| `packages/desktop/src/views/changes/flow/DetailDrawer.tsx` | 抽屉（v1 文件表节降级文案） |

**关联AC**: AC-6

**关系描述**:

三代际降级是页面级分支：判定依据分散在 DTO 三处（inventory 值、pipeline 空数组、fileLog null），落地却横跨「挂不挂图、显不显 workflow 面板、抽屉文件表节形态」三个组件的联动。v1 的判定信号必须穿透 ChangeDetailView 到 DetailDrawer 的 props / 状态传递才生效，单测各自验证组件局部，出错模式是判定条件与组件分支错位——v0 误挂空图、v1 误渲染空面板或抽屉缺降级文案、产物区在降级分支中被误伤。

#### 场景: v0 / v1 / v2 三态与 unparsable 退化

同一 fixture 骨架分别裁出三态经真实链路渲染整页（invoke mock 返回对应 DTO）：v0 为 inventory='v0' + pipeline=[]；v1 为 inventory='v1' + fileLog=null；v2 完整。预期 v0 无 flow-graph、有 `flow-empty` 文案「v0 早期代际：无 workflow.json，仅文档形态」且产物区文档列表照常；v1 图正常绘制、无 `workflow-panel`、抽屉文件表节呈降级文案；v2 全量并存。另验 unparsable 详情（各站 attempts 空）→ 9 空列正常呈现不白屏。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | v2 完整呈现：图区 + workflow 面板 + 产物区并存 | 新增 |
| 正向 | v1 图正常绘制、无 workflow 面板、抽屉文件表节呈 v1 降级文案 | 新增 |
| 边界 | v0 空图占位（flow-empty）且产物区不受图区缺位影响照常渲染 | 新增 |
| 边界 | unparsable 详情 → 9 空列流程图正常呈现（列容器 9、事件节点 0）不白屏 | 新增 |
| 异常 | v0 与 fileLog null 叠加（更早代际混合形态）→ 空图占位 + 无面板双降级互不冲突 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri IPC（invoke） | `vi.mock('@tauri-apps/api/core')` invoke mock，按代际返回对应 ChangeDetail DTO 与信封数组（沿 ipc_pipeline.test.tsx 既有模式） | 全部场景 |
| ResizeObserver（运行环境） | `vi.stubGlobal('ResizeObserver', stub 类)`，teardown unstub | 涉及图区挂载的场景 |

### useChangeDetail 显式 refresh → ChangeDetailView 图区重算 → `packages/desktop/src/__tests__/change_flow_refresh.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/changes/hooks/useChangeDetail.ts` | 取数方（显式 refresh 触发 IPC，零改动基线） |
| `packages/desktop/src/views/changes/ChangeDetailView.tsx` | 消费方 / 触发方（刷新按钮 → refresh → state 更新） |
| `packages/desktop/src/views/changes/flow/graph.ts` | 下游（detail → graph 重算） |
| `packages/desktop/src/views/changes/flow/ChangeFlowGraph.tsx` | 下游渲染（graph props 变化 → 重渲染 + 重新 fitView） |

**关联AC**: AC-7

**关系描述**:

AC-7 是不变式验收：详情更新只由显式 refresh 触发，无 watch / 轮询 / 事件订阅。取数语义归 useChangeDetail（本变更零改动），但保证在新视图下不回归必须走整页链路：挂载后恰一次 get_change_detail，静置不产生新调用（排除轮询回归），点击「刷新详情」恰一次新调用且新 DTO 穿透转换层到图区。出错模式是新布局引入由渲染周期触发的隐式取数（如抽屉打开时重新拉取）、节点点击回调误触 refresh、或组件重复挂载导致调用翻倍。

#### 场景: 调用计数不变式与图区穿透

invoke mock 返回 v2 DTO，渲染完整 ChangeDetailView（含图区）。预期：挂载稳定后 get_change_detail 恰调用 1 次；推进虚拟时间静置多个周期计数仍为 1（无轮询 / watch 回归）；点击「刷新详情」后恰 +1 次，且返回更新后的 DTO 时 `flow-node` 集合随之变化（证明 detail 已穿透到转换层重算）；抽屉打开 / 关闭与列头 / 节点点击等交互全程不产生任何新 invoke。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 挂载恰一次 get_change_detail；点击「刷新详情」恰 +1 次且图区随新 DTO 重算（节点集合变化） | 新增 |
| 边界 | 抽屉打开 / 关闭、列头与节点点击等交互不触发任何 invoke | 新增 |
| 异常 | fake timers 推进静置周期 → invoke 调用计数不增长（无轮询 / 事件订阅回归） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri IPC（invoke） | `vi.mock('@tauri-apps/api/core')` invoke mock：可控行为（返回 / 延迟 resolve 更新前后两份 DTO）+ 调用计数断言 | 全部场景 |
| 定时器 | `vi.useFakeTimers` 推进静置期 | 异常路径（无轮询回归）用例 |
| ResizeObserver（运行环境） | `vi.stubGlobal('ResizeObserver', stub 类)`，teardown unstub | 涉及图区挂载的场景 |

---

## 不可测试项

<!-- test_resolve_paths 返回 errors 为空，以下为模块性质与 AC 属性导致的不可自动化项。 -->

- AC-9 管线合规（client:check 含 fmt / lint（max-lines-per-function: 50）/ knip 全绿、无新增豁免条目） — **原因**: 静态守线门禁由检查工具在守线任务中裁定，非运行时用例可断言；其中「测试挂钩均为 data-testid」的可验证面已由上述单测与集成用例全部以 data-testid 查询落实。
- AC-10 版本交付（`packages/desktop/package.json` version 0.3.3、tauri.conf.json 经 `../package.json` 引用自动跟随、src-tauri/Cargo.toml 保持 0.1.0 不随动） — **原因**: 静态配置交付约束，由验收阶段零 diff 边界检查核对；为版本号写运行时断言无回归价值。
- `flow/types.ts`（test_resolve_paths 解析出 `flow/types.test.ts`，不建） — **原因**: 纯类型模块，八个导出均为编译期类型无运行时行为；类型正确性由消费方（graph.ts / attachments.ts / 组件层）用例在编译期覆盖，生成零用例测试文件会导致运行器报错。
- `packages/desktop/package.json` 依赖声明（`@xyflow/react` ^12.11.0 新增） — **原因**: 依赖解析属包管理器与构建期职责（test_detect_frameworks 对该文件判 unknown），非进程内可测单元。
- react-flow 库自身渲染语义（fitView 缩放数值、subflow extent 约束、边线路径绘制、Background dots） — **原因**: 库自带语义不逐项验证（项目约定），测试只覆盖自研层：nodePosition 换算、DrawerSelection 上抛、testid 与视觉标记。
- fitView 在真实窗口尺寸下的可读性（9 列 ≈ 2340px 总宽在最小窗口 900px 的缩放表现） — **原因**: 依赖真实窗口布局尺寸，jsdom 无布局引擎；属人工视觉验收，自动化仅能断言 testid 与回调语义。
