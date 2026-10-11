# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 时间序边推导

执行图边 SHALL 由后端边表（`runs[].edges`，desktop-change-state-store「run 执行图落库」）直接读取，**不再由列差 Δ 派生**：`deriveEdges` 按 `colIndex` 差派生 forward（Δ>0）/ retry（Δ=0）/ backtrack（Δ<0）的逻辑 SHALL 退役。边 SHALL 以 `edgeKind ∈ { forward, feedback, retry, backtrack }` 显式呈现：

- **forward** 计划序推进（向右或同列计划步）；
- **feedback** gate 失败回到同 phase agent 步的 step 级回边（phase 内回边，新增）；
- **retry** phase fail 后同相位重跑；
- **backtrack** 跨相位回跳（虚线 + `backtrackReason` 边标签）。

边端点 SHALL 以节点稳定身份键 `(phase, attempt, step, instanceNo)` 寻址（取代「时间序扫描串联 + active 按 startAt 归并插入」）；节点身份键自 `runs[].nodes` 直读（desktop-change-queries「change 详情聚合」），MUST NOT 在前端重新派生节点序。attempt 缺号 SHALL 按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款），startAt 缺失的事件 SHALL 按列表序归并、不丢节点。

#### Scenario: 四类边自边表呈现

- **WHEN** 执行史含 proposal#1 → dev-design#1 fail → dev-design#2 pass（同列 retry）、test-execution#1 → test-gen#2（跨站 backtrack）、以及 implement 内 static-check fail → executor 修复（phase 内 feedback）
- **THEN** 图边自 `runs[].edges` 直读：retry 边同列、backtrack 边虚线带 `backtrackReason`、feedback 边为 phase 内回边、forward 边计划序推进；无 `deriveEdges` Δ 派生逻辑

#### Scenario: 缺号与缺时间戳兜底

- **WHEN** 某事件 `attempt` 为 null 或 startAt 为 null
- **THEN** 节点不丢失：attempt 以 0 兜底参与列内排序，startAt 缺失者按列表序归并，转换不报错

### Requirement: 过程素材挂载规则

素材挂载分两层：**文档挂列、记录挂节点**。

- 过程文档（`artifacts` 中 `markdown-doc` / `tasks-progress` 类）SHALL 经 **后端 phase 清单归属**挂到 phase 列（列头徽章，每站一份、不随 attempt 重复）：`proposal.md` 与 `specs/**` → proposal、`design.md` 与 `tasks.md` → dev-design、`test-reports/**` → test-execution——归属 SHALL 来自源码 phase 清单（desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）经 detail 出线，前端 `docColumn(source)` 字符串→列静态映射 SHALL 退役。清单未声明归属的文档 SHALL 落入「未归属」处理（design 定夺展示位置），MUST NOT 误挂到错误站。`explore.md` MUST NOT 入图（探索在流程之外，不属于任何站）。
- eval-checklist 信封（payload 自带 `phase` + `attempt`）SHALL 挂到对应 attempt 节点（PhaseRecord 投影分支，不走相对路径清单）。

file_log 挂载 SHALL 退役：file_log 条目挂节点规则与 `scope='workflow'` 图外独立面板随载体退役一并删除（前端 file-log 面板组件与挂载分支移除）。

归属 SHALL 由后端 phase 清单供给，前端 MUST NOT 保留第二份 docColumn 静态映射；后端 `ChangeDetail` DTO 与 `PIPELINE_PHASES` MUST NOT 因本能力新增 file_log 相关字段。

#### Scenario: 文档按清单归列

- **WHEN** 某 change 的产物含 `proposal.md`、`design.md`、`tasks.md` 与 `test-reports/xxx.md`
- **THEN** proposal 列头徽章含 proposal.md，dev-design 列头徽章含 design.md 与 tasks.md，test-execution 列头徽章含该 test-reports 文档（归属自后端清单，无 docColumn 映射）

#### Scenario: 清单未声明文档不误挂

- **WHEN** change 目录存在清单之外的 `.md`（如 `notes.md`）
- **THEN** 该文档不挂任何 phase 列（落入「未归属」处理），MUST NOT 被 docColumn 或扩展名猜测误挂到错误站

#### Scenario: explore.md 隐身与文件面板退役

- **WHEN** 打开任一 change 详情，产物清单含 `explore.md`
- **THEN** explore.md 不出现在图中，图上无文件徽章挂靠（节点文件区概念已退役），无 workflow 独立面板渲染

### Requirement: 运行态节点与实时执行视图

change 详情流程图 SHALL 升级为实时执行视图：运行期间，当前执行位置的节点 SHALL 呈现运行态（pulse 视觉，与既有 active 节点同款），ToolStep 节点（phase-start / static-check / phase-log / backtrack 相位机步）与 Gate 节点 SHALL 与 WorkerAgent 节点同等上图并可辨（类型可区分）。运行态呈现 SHALL 与既有节点模型并存：eval（实心 pass 绿 / fail 红）、stale 淡化规则不变；`PIPELINE_PHASES` 列布局、边表读取规则与空列恒定 MUST NOT 因运行态呈现改变。

运行收口后，图 SHALL 保持**步节点常驻渲染**（「图回落 ChangeDetail 派生规则」语义退场）：已落库执行图节点（`runs[].nodes` 全词汇，desktop-change-state-store）自统一视图派生常驻上图，与运行中实时面走**同一条派生路径**（同一转换函数消费合并查询结果，而非结果碰巧一样）；运行态视觉（pulse）褪去、终态样式呈现。常驻渲染 SHALL 取**全史叠加**（多次 run 的节点全部保留，非仅最近 run）：attempt 经跨 run max+1 分配不撞号，全史叠加在同一列面、run 归属分层经 run 史可查；在飞 run 的活节点叠加其上。节点身份键 SHALL 为 `(phase, attempt, step, instanceNo)`（自后端直读，见 desktop-change-queries），前端 `runStepNodes` 的「槽位配对 + `:seq` 后缀堆叠」机制 SHALL 退役——节点直接按后端稳定身份键渲染，同一 `(phase, attempt, step)` 的多次执行以 `instanceNo` 区分。

#### Scenario: 运行态与两分类并存

- **WHEN** implement#2 相位运行中（executor 会话活跃）且历史上有 fail→retry 与 active_phase 残留
- **THEN** 图上同时呈现：历史 eval 节点（实心三色）、active_phase 的 active 节点（pulse）与当前 executor 节点（pulse 运行态）及同相位的 ToolStep / Gate 节点（类型可辨、状态如实）

#### Scenario: 收口后常驻不回落

- **WHEN** run 以全部相位 pass 收口（或 fail / stopped 收口）后刷新 / 重开详情
- **THEN** 已落库执行图节点常驻上图（终态样式、无 pulse），派生自统一视图库读史、与运行中同一转换函数；无「回落后步节点消失」的形态跳变

#### Scenario: 稳定身份键渲染与 feedback 多实例

- **WHEN** 某 attempt 内 static-check 失败触发 executor 修复（feedback 重跑）后刷新详情
- **THEN** executor 首次与修复各为 `instanceNo` 递增的独立节点（同一 `(phase, attempt, step)` 不覆盖），feedback 边自边表呈现；无「槽位配对 + `:seq` 堆叠」逻辑残留，同键多实例不串扰

#### Scenario: 全史叠加与在飞叠加

- **WHEN** 某 change 历经两次 run 后第三次 run 运行中
- **THEN** 图上前两 run 的节点全史常驻，第三 run 活节点（pulse）叠加其上；同 phase 节点 attempt 递增不撞键，任一步节点可溯其 run 归属

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/flow/graph.ts` + `types.ts` | 边表直读与节点身份 | `deriveEdges` Δ 派生退役、改读 `runs[].edges`（四 kind）；节点身份键 `(phase, attempt, step, instanceNo)`；纯函数直测保持绿 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | phase 清单归属 | `docColumn` 静态映射退役；文档挂列读后端 phase 清单；eval-checklist 走 PhaseRecord 投影分支 |
| `packages/desktop/src/views/changes/flow/run-state.ts` | 稳定身份渲染 | `runStepNodes` 槽位配对 + `:seq` 堆叠退役；节点按后端稳定身份键直渲染；feedback 多实例以 `instanceNo` 区分 |
| `packages/desktop/src/views/changes/flow/` 节点 / 列组件 | 边与节点渲染 | 四 kind 边呈现（feedback 相位内回边可辨）；WorkerAgent / ToolStep / Gate 类型可辨；pulse 运行态与既有 active 视觉同款 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 拼缝统一 | 执行图数据源 = 统一视图（库读史 ∪ 在飞 run）同一路径；无 deriveEdges / docColumn / 槽位配对残留 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型再生跟随 | nodes / edges / edgeKind / instanceNo 经 export-bindings 再生成；tsc 全量类型检查拦截前端消费漂移 |
