# desktop-change-flow-view Specification (Delta)

## ADDED Requirements

### Requirement: 步级耗时与 token 节点呈现

runtime step 节点（`RunStepNode`）SHALL 在标题行下新增**常驻行**呈现步级耗时与 token 消耗（非 HoverCard，决策拍板）：耗时 = `endedAt - startedAt` 前端计算（后端不落 duration）；token 行仅 `group === 'workerAgent'` 节点渲染（`tokenInput` / `tokenOutput` 两值缩写呈现，缺席 "—"）。`RuntimeFlowNode` SHALL 透传 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput` 四字段（`runStepNodes` / `stepFromRecord` 直读透传，词汇归一单点口径不变）。

呈现语义 SHALL 为：

- **静态耗时**：已收口步（`endedAt !== null` 且 `startedAt > 0`）显格式化耗时；`phaseStart` 等瞬步 0ms 不显耗时缀；
- **running 走秒**：**仅活面（在飞 run）节点**且 `status === 'running'` 时以 `Date.now() - startedAt` 每秒刷新呈现（节点组件自持 interval 局部重渲染——前端全库首例，xyflow 节点独立渲染不惊动画布；节点离开 running 态或卸载即清 timer，收口后回落静态耗时）；库读史 `status=running` 残留节点（run 已死）MUST NOT 走秒，恒静态呈现；
- **缺席兜底**：`startedAt <= 0`（旧库行）或 `endedAt == null` 的非 running 态显 "-"（决策拍板：旧数据不从 turn 行回填）；token 缺席显 "—"。

毫秒 → 人读格式化 SHALL 为独立纯函数模块（新写，`1.2s` / `3m20s` 类分级；无既有先例可复用），vitest 直测边界；判定「缺席 → "-"」逻辑 SHALL 收格式化侧单点（MUST NOT 散落 `0` 魔数比较）。布局（10 列手工坐标 / subflow）、边表读取、节点点击判据与抽屉交互 MUST NOT 因常驻行改变；测试挂钩 SHALL 使用 data-testid。

#### Scenario: 常驻行静态耗时与 token

- **WHEN** 打开含已收口 executor 步（`startedAt=T0` / `endedAt=T1` / `tokenInput=12000` / `tokenOutput=3000`）的 change 详情
- **THEN** 该节点标题下常驻行呈现格式化耗时 `T1 - T0` 与 tokens in / out 缩写；无 HoverCard 依赖、点击判据不变

#### Scenario: token 行仅 WorkerAgent 组

- **WHEN** 图上同时呈现 executor 节点与 static-check 工具步节点（后者四字段中 token 恒 null）
- **THEN** 工具步 / Gate / Git 节点不渲染 token 行（仅耗时）；workerAgent 组节点渲染 tokens in / out 两值

#### Scenario: running 走秒与收口回落

- **WHEN** run 运行中某 WorkerAgent 节点 running（活面，`startedAt` 在案、`endedAt=null`）
- **THEN** 该节点耗时以每秒 tick 的走秒值呈现；该步收口（状态离开 running）后 timer 卸载、回落静态耗时；同一时刻其余节点零重渲染

#### Scenario: 库读史残留与旧数据兜底

- **WHEN** 打开详情呈现本变更前落库的 run 节点（`startedAt=0` / `endedAt=null`）或库读史 status=running 残留节点
- **THEN** 耗时显 "-"（不走秒、不报错）；token 显 "—"；其余节点呈现不受影响

#### Scenario: 格式化纯函数直测

- **WHEN** 以边界值（0ms / 999ms / 1s 级 / 分秒级 / 时分级）调用格式化函数
- **THEN** 输出符合分级规则（0ms 由调用侧省显、缺席入参出 "-"），无需挂载 react 组件即可断言

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/types.ts` + `run-state.ts` | 透传 | `RuntimeFlowNode` 增 `startedAt` / `endedAt` / `tokenInput` / `tokenOutput`；`stepFromRecord` / `runStepNodes` 直读透传（`UnifiedRunStep` 单一形状） |
| `packages/desktop/src/views/changes/flow/run-step-node.tsx` | 常驻行与走秒 | 标题下常驻行（静态耗时 + workerAgent token 行）；running 走秒 interval 仅活面 running 节点挂载、卸载即清；缺席 / 旧数据 "—" 兜底 |
| `packages/desktop/src/views/changes/flow/`（格式化模块，新） | 毫秒 → 人读 | 纯函数分级格式化 + 缺席判定单点；vitest 直测边界 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeStepState` / `ChangeRunNodeRecord` 四字段再生成，tsc 全量类型检查拦截漂移 |
