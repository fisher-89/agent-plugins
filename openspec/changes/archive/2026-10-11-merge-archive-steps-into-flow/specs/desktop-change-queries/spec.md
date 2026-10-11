# desktop-change-queries Specification (Delta)

## Purpose

定义 workflow crate 查询层的聚合契约：change 列表为 workspace 库 ChangeRecord 全量（db 单源、零磁盘扫描、按 id 寻址、按月分组），单 change 详情聚合（10 站流水线、PhaseRecord attempt 序列、active_phase、backtrack、会话槽位、run 运行史全量读面），查询层保持纯读且无指令概念。流水线由 9 站扩为 10 站（追加 archive），run 执行图节点全词汇十八值。

## MODIFIED Requirements

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情（**按 id 寻址**），状态面单源自 workspace 库：10 站流水线（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / code-analyze / archive）逐站展开 PhaseRecord 的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（自 ChangeRecord）与 backtrack 字段（backtrack_to / backtrack_reason）；checklist 自 ChecklistItemRecord 子表按打包键序重组，线面聚合形状不变。详情 SHALL 出线 `id`（身份锚）、`name`（恒裸名）与 `title`（恒非空）。详情 SHALL 出线 `worktree` 路径（自 ChangeRecord.worktree 直读透出；None → null）。产物发现与信封读取的路径解析 SHALL 经 **id → 记录 → name / worktree** 分辨率；`worktree=None` 的 change 维持主仓两树解析既有语义。**文档形态分支整体退役**：db 无该 id 记录时详情 SHALL 返回「未找到」（`None`），MUST NOT 回退磁盘目录解析、MUST NOT 读取任何 workflow.json。`id` / `title` 字段为 detail 线面字段演进，SHALL 走 golden 显式重写流程。

详情 SHALL 同时出线 **run 执行图全量读面**：`runs` 数组（自 RunRecord 全史投影），每 run 的 `nodes`（自 RunNodeRecord 投影：phase / attempt / step / instanceNo / stepKind / status / sessionId / detail / outputRef）与 `edges`（自 RunEdgeRecord 投影：fromNodeKey / toNodeKey / edgeKind ∈ forward / feedback / retry / backtrack）。节点身份键 `(phase, attempt, step, instanceNo)` SHALL 为稳定出线键；边 SHALL 显式出线。节点 `stepKind` SHALL 为十八值全词汇（含 8 个归档步词）。时间戳 SHALL 维持既有口径（db i64 毫秒落库、出线 ISO 串 + null）；DTO SHALL 纯 derive 零字段属性，10 站流水线 / 十八值 nodes / edges 字段演进 SHALL 走 golden 显式重写流程，bindings 再生成同步前端类型。详情标示「运行中」SHALL 与实际在飞 run 一致；archive 相位完成落账后 `phase_next` 路由 done，详情 MUST NOT 呈运行中假象。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件按 phase 定义清单驱动发现）；archive 相位无产物清单（不产 proposal / design / tasks 类文档），MUST NOT 为 archive 列挂过程文档。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 PhaseRecord 含同 phase 多次 attempt（fail→retry）与 backtrack_to 记录
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: 归档 change 全状态面（错配修复锚点）

- **WHEN** 打开归档 change 详情（记录 status=archived，磁盘目录名为 `YYYY-MM-DD-<name>`，经 id 寻址）
- **THEN** 出线 `status=archived`、10 站 pipeline 与 runs 全量呈现（archive 列 PhaseRecord 与 step 节点在案），`name` 为裸名、`id` 恒在案；产物清单经带前缀目录命中

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
- **THEN** `runs` 出线全史记录；每 run `nodes` 按稳定身份键出线（十八值全词汇零过滤、archive 8 个步词在案、feedback 重跑为 `instanceNo` 递增新节点），`edges` 显式出线四 kind；无 `seq` 字段、无 `deriveEdges` 派生残留

#### Scenario: 10 站流水线 golden 显式重写

- **WHEN** detail DTO 增 archive 列与十八值 nodes 后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/list.rs` | 列表 db 单源 | 零磁盘扫描；`ChangeSummary` 增 `id`、name 恒裸名；月分组 `archived_at` 唯一权威 |
| `crates/core/workflow/src/queries/mod.rs` | change 目录定位单点 | `locate_change(layout, worktree, name)` 名义签名不变（调用前经 id → 记录 → name 分辨率）；archive 后缀扫描与 `is_single_component_name` 校验保留 |
| `crates/core/workflow/src/queries/detail.rs` | 详情 id 寻址与线面收敛 | `PIPELINE_PHASES` 10 列（追加 archive）；`change_detail(..., id)`：`get_change(id)` → 记录 → `locate_change`；文档形态分支删除；`ChangeDetail` 增 `id`；未知 id → `None`；槽位三列直读透出；runs nodes 十八值全词汇投影；纯 derive 零字段属性口径不变 |
| `crates/infra/store`（workspace 库实例，经 port 缝消费） | 状态读源 | ChangeRecord / PhaseRecord / ChecklistItemRecord / RunNodeRecord / RunEdgeRecord 只读查询；时间转换收 queries 层单点 |
| `foundation::layout` | 路径输入 | 产物发现仅经 `Layout` 取目录，不自行拼磁盘路径 |
| `workflow::artifacts` | 产物发现协作 | 详情返回产物清单，发现机制复用 matcher 注册表；archive 相位零产物清单 |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | 10 站流水线、十八值 nodes、archive 列投影走 golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕） |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeDetail` 10 列 pipeline 与 nodes 十八值经 export-bindings 再生成；一致性守卫拦截漂移 |
