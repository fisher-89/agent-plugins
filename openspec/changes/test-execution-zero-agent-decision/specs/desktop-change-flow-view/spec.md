# desktop-change-flow-view Specification (Delta)

## MODIFIED Requirements

### Requirement: 时间序边推导

执行图边 SHALL 由后端边表（`runs[].edges`，desktop-change-state-store「run 执行图落库」）直接读取，**不再由列差 Δ 派生**：`deriveEdges` 按 `colIndex` 差派生 forward（Δ>0）/ retry（Δ=0）/ backtrack（Δ<0）的逻辑 SHALL 退役。边 SHALL 以 `edgeKind ∈ { forward, feedback, retry, backtrack }` 显式呈现（四值词汇为 `RunEdgeKind` 的解码契约，前端类型面不收缩）：

- **forward** 计划序推进（向右或同列计划步）；
- **feedback** gate 失败回到同 phase agent 步的 step 级回边（phase 内回边，新增）；
- **retry** phase fail 后同相位重跑——该语义已随决策协议退役（重做本相位改由回溯到自身表达，入口边为 `backtrack`），存量 `retry` 边由查询层投影屏蔽（desktop-change-queries「change 详情聚合」），投影面 MUST NOT 出现该 kind；前端样式分支保留默认渲染，MUST NOT 视为可达形态；
- **backtrack** 跨相位回跳（虚线 + `backtrackReason` 边标签；目标为失败相位自身即「重做本相位」的重做入口）。

边端点 SHALL 以节点稳定身份键 `(phase, attempt, step, instanceNo)` 寻址（取代「时间序扫描串联 + active 按 startAt 归并插入」）；节点身份键自 `runs[].nodes` 直读（desktop-change-queries「change 详情聚合」），MUST NOT 在前端重新派生节点序。attempt 缺号 SHALL 按 0 兜底（与后端 `detail.rs` 的 `unwrap_or(0)` 同款），startAt 缺失的事件 SHALL 按列表序归并、不丢节点。边端点缺失（历史投影被屏蔽的 `retry` 行、或节点未出线）SHALL 跳过该边且不报错——图面照常渲染其余边。

#### Scenario: 四类边自边表呈现

- **WHEN** 执行史含 proposal#1 → dev-design#1 fail → dev-design#2 pass（决策回溯到自身重做的同列入口边）、test-execution#1 → test-gen#2（跨站 backtrack）、implement 内 static-check fail → executor 修复（phase 内 feedback），且存量库含 `retry` 边
- **THEN** 图边自 `runs[].edges` 直读：同列重做入口呈 `backtrack`（虚线、带 `backtrackReason`）、跨站回跳呈 `backtrack`、phase 内回边呈 `feedback`、forward 边计划序推进；`retry` 边不出现（查询层已屏蔽，端点缺失亦跳过）；无 `deriveEdges` Δ 派生逻辑

#### Scenario: 缺号与缺时间戳兜底

- **WHEN** 某事件 `attempt` 为 null 或 startAt 为 null
- **THEN** 节点不丢失：attempt 以 0 兜底参与列内排序，startAt 缺失者按列表序归并，转换不报错

#### Scenario: retry 边缺席不误渲染

- **WHEN** 以含 `edgeKind: 'retry'` 的边集夹具调用转换函数（模拟旧库未屏蔽或夹具直构）
- **THEN** 该边按默认样式渲染或按端点缺失跳过，转换不 panic、不产生额外派生边；正常运行的图面不出现该 kind（投影面屏蔽为其唯一来源保证）

## Module Contract

| 模块 | 定位 | 关键契约（Component / Function） |
|------|------|------|
| `src/views/changes/flow/graph.ts` | 边转换 | `runs[].edges` 直读、端点缺失 `continue`；零 `deriveEdges` Δ 派生；`retry` 无专属样式分支 |
| `src/views/changes/flow/graph.test.ts` | 转换层用例 | `retry` fixture 改写为「输入含 retry 边 → 输出图不含该边」的投影锚 |
