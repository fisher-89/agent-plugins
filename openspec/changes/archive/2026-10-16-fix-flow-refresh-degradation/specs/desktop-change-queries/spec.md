# desktop-change-queries Specification (Delta)

## ADDED Requirements

### Requirement: 详情查询错误语义分流（None / Err）

详情查询 SHALL 将两种「详情不可达」形态分流出线，MUST NOT 混同：

- **None = 真「未找到」**：db 无该 id 记录（含 `workflow_type` 不在相位表内的既有 `None` 语义，见「change 详情聚合」）→ `Ok(None)`；前端「未找到」兜底页语义不变。
- **Err = 读失败**：workspace 库读 `Err`（`get_change` 返回 `StoreFault`）SHALL 以 `Err` 原样传播出线（错误串透传到命令层与前端），MUST NOT 经 `.ok()` / `unwrap_or_default` 降级为 `None` 或空面——读失败 MUST NOT 呈「未找到」假象（运行中的 change 因 store 瞬时读错被前端整页塌兜底的竞态由此封堵）。

core `change_detail` 签名 SHALL 由 `Option<ChangeDetail>` 改为 `Result<Option<ChangeDetail>, StoreFault>`；既有其余子表读（`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges`）的 `unwrap_or_default` 单区块空面降级哲学 SHALL 保持不变（单区块读失败不升级为整份 `Err`）。命令面 `get_change_detail` SHALL 随动为 `Result<Option<ChangeDetailUnified>, String>`：blank root 维持空结果 `Ok(None)` 语义；`StoreFault` → 错误串映射收命令层单点。DTO 线面形状零演进：`Result<Option<T>, String>` 经 bindings 出线仍为 `Promise<T | null>`（Err 走 reject 通道），前端类型面零漂移、golden 零重写。前端失败轮的保 last-good 降级行为由 desktop-change-flow-view「通知重查失败降级与有界重试」承载。

#### Scenario: store 读 Err 传播不降级

- **WHEN** mock store 的 `get_change` 返回 `Err(StoreFault)`（该 id 记录实际在库）
- **THEN** `change_detail` 返回 `Err`（fault 错误串透传），MUST NOT 出 `Ok(None)`（未找到假象）、MUST NOT 出部分区块空面的降级 `Some`

#### Scenario: 未知 id 仍 Ok(None)

- **WHEN** 查询 db 无该 id 记录的 id（含磁盘存在同名目录的情形）
- **THEN** `change_detail` 返回 `Ok(None)`（真「未找到」），零磁盘目录解析回退、零 workflow.json 读取，不报错——既有语义回归

#### Scenario: 命令面 Err 出线为 reject

- **WHEN** core `change_detail` 返回 `Err`
- **THEN** IPC `get_change_detail` 以 store 错误串 reject（前端 invoke catch 面可承接），非 `null` 应答、非 panic；blank root 仍 `Ok(None)`（空结果语义不变）

#### Scenario: 子表读降级哲学不变

- **WHEN** `get_change` 读 `Ok(Some)` 而 `list_runs` 读 `Err`
- **THEN** 详情照常 `Ok(Some)` 出线（runs 为空数组），单区块缺失不升级为整份 `Err`——既有 `unwrap_or_default` 哲学保持

#### Scenario: bindings 零漂移

- **WHEN** 签名 Result 化后执行 export-bindings 再生成
- **THEN** `getChangeDetail` TS 类型面不变（`Promise<ChangeDetailUnified | null>`，Err 走 reject 通道），一致性守卫绿、golden 零 diff

## MODIFIED Requirements

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情（**按 id 寻址**），状态面单源自 workspace 库：流水线相位序 SHALL 随 `record.workflow_type` 出线（`phase_table(workflow_type)` 的相位序——requirement 9 列、test-only 6 列；MUST NOT 使用固定 10 站 `PIPELINE_PHASES` 常量）逐站展开 PhaseRecord 的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（自 ChangeRecord）与 backtrack 字段（backtrack_to / backtrack_reason）；checklist 自 ChecklistItemRecord 子表按打包键序重组，线面聚合形状不变。详情 SHALL 出线 `id`（身份锚）、`name`（恒裸名）与 `title`（恒非空）。详情 SHALL 出线 `worktree` 路径（自 ChangeRecord.worktree 直读透出；None → null）。产物发现与信封读取的路径解析 SHALL 经 **id → 记录 → name / worktree** 分辨率；`worktree=None` 的 change 维持主仓两树解析既有语义。**文档形态分支整体退役**：db 无该 id 记录时详情 SHALL 返回「未找到」（`None`），MUST NOT 回退磁盘目录解析、MUST NOT 读取任何 workflow.json。`id` / `title` 字段为 detail 线面字段演进，SHALL 走 golden 显式重写流程。

详情 SHALL 同时出线 **run 执行图全量读面**：`runs` 数组（自 RunRecord 全史投影），每 run 的 `nodes`（自 RunNodeRecord 投影：phase / attempt / step / instanceNo / stepKind / status / sessionId / detail / outputRef）与 `edges`（自 RunEdgeRecord 投影：fromNodeKey / toNodeKey / edgeKind ∈ forward / feedback / retry / backtrack）。节点身份键 `(phase, attempt, step, instanceNo)` SHALL 为稳定出线键；边 SHALL 显式出线。节点 `stepKind` SHALL 为十九值全词汇（含 `mutation_test` 与 8 个归档步词）。时间戳 SHALL 维持既有口径（db i64 毫秒落库、出线 ISO 串 + null）；DTO SHALL 纯 derive 零字段属性，流水线相位序 / 十九值 nodes / edges 字段演进 SHALL 走 golden 显式重写流程，bindings 再生成同步前端类型。详情标示「运行中」SHALL 与实际在飞 run 一致；archive 相位完成落账后 `phase_next` 路由 done，详情 MUST NOT 呈运行中假象（有界降级例外——通知重查失败轮保 last-good 的过渡态由 desktop-change-flow-view「通知重查失败降级与有界重试」承载：有界重试、带「刷新失败」内联提示、收敛保证；本条款禁杀的是无界、静默的假态，强度不变）。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件按 phase 定义清单驱动发现）；archive 与 code-analyze 相位无产物清单（不产 proposal / design / tasks 类文档），MUST NOT 为 archive / code-analyze 列挂过程文档。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 PhaseRecord 含同 phase 多次 attempt（fail→retry）与 backtrack_to 记录
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: 归档 change 全状态面（错配修复锚点）

- **WHEN** 打开归档 change 详情（记录 status=archived，磁盘目录名为 `YYYY-MM-DD-<name>`，经 id 寻址）
- **THEN** 出线 `status=archived`、相位序随 workflow_type 的 pipeline 与 runs 全量呈现（archive 列 PhaseRecord 与 step 节点在案），`name` 为裸名、`id` 恒在案；产物清单经带前缀目录命中

#### Scenario: worktree 路径出线与产物经 worktree 解析

- **WHEN** 查询带 worktree 记录的 change 详情并逐个读取其产物信封
- **THEN** detail DTO 的 `worktree` 与库内记录值逐字一致；产物清单与信封读取命中 worktree 内目录

#### Scenario: legacy 与 null 出线

- **WHEN** 查询 `worktree=None` 的 change 详情
- **THEN** `worktree` 出线 null、产物解析走主仓两树既有语义，聚合不报错

#### Scenario: 未知 id 未找到

- **WHEN** 查询 db 无记录的 id（含磁盘存在同名目录的情形）
- **THEN** 详情返回「未找到」（`None`），零磁盘目录解析回退、零文档形态空面、workflow.json 零读取，不报错

#### Scenario: 运行中状态可见且不撒谎

- **WHEN** 某 change 的 ChangeRecord.`active_phase` 非空（含 archive 相位运行中）
- **THEN** 详情标示该 phase 为运行中；run 死亡后 `active_phase` 已处置，详情 MUST NOT 呈运行中假象

#### Scenario: run 执行图全量出线

- **WHEN** 某 change 历经一次含 archive 相位、feedback 重跑、phase 重试与 backtrack 的 run 后查询详情
- **THEN** `runs` 出线全史记录；每 run `nodes` 按稳定身份键出线（十九值全词汇零过滤、mutation_test 与 archive 8 个步词在案、feedback 重跑为 `instanceNo` 递增新节点），`edges` 显式出线四 kind；无 `seq` 字段、无 `deriveEdges` 派生残留

#### Scenario: 10 站流水线 golden 显式重写

- **WHEN** detail DTO 相位序随 workflow_type 出线后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: title 出线与 golden 显式重写

- **WHEN** 查询 title 非空的 change 详情
- **THEN** `title` 自记录直读逐字一致；detail DTO 增 `title` / nodes / edges 后运行语料回归测试，diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: golden 显式重写留痕

- **WHEN** detail DTO 增 `id` 字段（与列表 `id` 投影）后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: 相位序随 workflow_type 出线

- **WHEN** 分别查询 `workflow_type="requirement"` 与 `"test-only"` 的 change 详情
- **THEN** requirement 的 `pipeline` 为 9 列（proposal…archive，无 code-analyze，archive 第 9 列）、test-only 为 6 列（proposal / code-analyze / test-design / test-gen / test-execution / archive，archive 第 6 列）；`pipeline` 相位序与 `phase_table(workflow_type)` 逐一一致

#### Scenario: 未知 workflow_type 读面不退化

- **WHEN** 查询 `workflow_type` 为不受支持值（如 `"bug-fix"`）的 change 详情
- **THEN** 详情按 design 定稿形态呈现（`None` / 空 pipeline / 显式错误三选一），MUST NOT 静默按 requirement 的 9 列出线；产物清单不误按 requirement 清单发现

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/detail.rs` | 错误语义分流单点 | `change_detail` 签名 `Option<ChangeDetail>` → `Result<Option<ChangeDetail>, StoreFault>`：`get_change` 读 `Err` 传播（`.ok()` 吞错消除）；子表读 `unwrap_or_default` 哲学不变；DTO 线面形状零演进 |
| `src/commands/changes/mod.rs`（`get_change_detail`） | IPC 面分流 | 返回 `Result<Option<ChangeDetailUnified>, String>`：blank root 仍 `Ok(None)`；`StoreFault` → 错误串映射收命令层单点 |
| `crates/core/orchestration/src/snapshot.rs`（机械随动） | 唯一 orchestration 调用点适配 | `Err` 透传 store 错误串（取代误报「change 不存在」）；`None` 仍「change 不存在: {id}」；`WorkflowSnapshotPort::detail` 契约与 walker 消费面不变 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型零漂移 | `Result<Option<T>, String>` 出线 `Promise<T \| null>`（Err 走 reject 通道）；再生成预期零 diff、一致性守卫绿 |
