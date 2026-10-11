# desktop-change-queries Specification (Delta)

## MODIFIED Requirements

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情（**按 id 寻址**），状态面单源自 workspace 库：9 站流水线（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / code-analyze）逐站展开 PhaseRecord 的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（自 ChangeRecord）与 backtrack 字段（backtrack_to / backtrack_reason）；checklist 自 ChecklistItemRecord 子表按打包键序重组，线面聚合形状不变（存储级拆分，出线不动）。详情 SHALL 出线 `id`（身份锚）、`name`（自记录直读，恒裸名——归档 change 的日期前缀仅存在于磁盘目录名，MUST NOT 进入出线值）与 `title`（人类可读标题，自记录直读，恒非空）。详情 SHALL 出线 `worktree` 路径（自 ChangeRecord.worktree 直读透出；None → null，纯 derive 零派生改写口径），作为用户 review / 手动 commit / merge 的可达锚（见 desktop-change-worktree「worktree 路径 UI 可见性」）。产物发现与信封读取的路径解析 SHALL 经 **id → 记录 → name / worktree** 分辨率：`locate_change` 在记录带 worktree 时于主仓 active / archive 两树未命中回退 worktree 路径（目录解析单点收口）；`worktree=None` 的 change 维持主仓两树解析既有语义（archive 树含日期前缀后缀扫描）。**文档形态分支整体退役**：db 无该 id 记录时详情 SHALL 返回「未找到」（`None`），MUST NOT 回退磁盘目录解析、MUST NOT 返回空流水线文档形态、MUST NOT 读取任何 workflow.json。fileLog 区块与 unparsable 状态 SHALL 保持退役；`id` / `title` 字段为 detail 线面字段演进，SHALL 走 golden 显式重写流程（wire contract 冻结记忆，`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）。

详情 SHALL 同时出线 **run 执行图全量读面**：`runs` 数组（自 RunRecord 全史投影，不截 last_run：runId / status / reason / 起止时刻），每 run 的 `nodes`（自 RunNodeRecord 投影：phase / attempt / step / instanceNo / stepKind / status / sessionId / detail / outputRef）与 `edges`（自 RunEdgeRecord 投影：fromNodeKey / toNodeKey / edgeKind ∈ forward / feedback / retry / backtrack），供前端执行图常驻渲染与运行史审计消费（见 desktop-change-state-store「run 执行图落库」、desktop-change-flow-view）。节点身份键 `(phase, attempt, step, instanceNo)` SHALL 为稳定出线键（取代原 `seq` emit 序），边 SHALL 显式出线（取代前端 `deriveEdges` 列差 Δ 派生）。时间戳 SHALL 维持既有口径（db i64 毫秒落库、出线 ISO 串 + null，转换收 queries 层单点）；DTO SHALL 纯 derive 零字段属性，runs / nodes / edges / title 字段演进 SHALL 走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕），bindings 再生成同步前端类型。详情标示「运行中」SHALL 与实际在飞 run 一致：active_phase 悬挂杀除后（desktop-change-state-store「启动标定与 active_phase 悬挂处置」），MUST NOT 出现无运行 run 而呈运行中的假象；在飞 run 的并入见 desktop-change-orchestration「运行控制命令面」读路统一。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件按 **phase 定义清单驱动**发现，见 `desktop-artifact-plugins`），供前端按信封逐个请求；产物发现 SHALL 对带 worktree 记录与 legacy（worktree=None）两类 change 同等可用（均经 id → 记录定位，文档形态类别已整体退役）。phase 归属 SHALL 来自源码 phase 清单（desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），前端 `docColumn` 静态映射退役后 MUST NOT 在 queries 层重建第二份归属表。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 PhaseRecord 含同 phase 多次 attempt（fail→retry）与 backtrack_to 记录
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: 归档 change 全状态面（错配修复锚点）

- **WHEN** 打开归档 change 详情（记录 status=archived，磁盘目录名为 `YYYY-MM-DD-<name>`，经 id 寻址）
- **THEN** 出线 `status=archived`、9 站 pipeline 与 runs 全量呈现（MUST NOT 落文档形态空面），`name` 为裸名、`id` 恒在案；产物清单经带前缀目录命中

#### Scenario: worktree 路径出线与产物经 worktree 解析

- **WHEN** 查询带 worktree 记录的 change 详情并逐个读取其产物信封
- **THEN** detail DTO 的 `worktree` 与库内记录值逐字一致；产物清单与信封读取命中 worktree 内目录（主仓两树未命中不致 detail / read_artifact 落空）

#### Scenario: legacy 与 null 出线

- **WHEN** 查询 `worktree=None` 的 change 详情
- **THEN** `worktree` 出线 null、产物解析走主仓两树既有语义，聚合不报错

#### Scenario: 未知 id 未找到

- **WHEN** 查询 db 无记录的 id（含磁盘存在同名目录的情形）
- **THEN** 详情返回「未找到」（`None`），零磁盘目录解析回退、零文档形态空面、workflow.json 零读取，不报错

#### Scenario: 运行中状态可见且不撒谎

- **WHEN** 某 change 的 ChangeRecord.`active_phase` 非空（phase 正在运行）
- **THEN** 详情标示该 phase 为运行中（含 attempt 序号与 start_at）；run 死亡后（含中途停止与重启标定）`active_phase` 已处置，详情 MUST NOT 呈运行中假象

#### Scenario: run 执行图全量出线

- **WHEN** 某 change 历经一次含 feedback 重跑、phase 重试与 backtrack 的 run 后查询详情
- **THEN** `runs` 出线全史记录；每 run `nodes` 按稳定身份键出线（全词汇零过滤、feedback 重跑为 `instanceNo` 递增新节点），`edges` 显式出线四 kind（forward / feedback / retry / backtrack）；无 `seq` 字段、无 `deriveEdges` 派生残留

#### Scenario: title 出线与 golden 显式重写

- **WHEN** 查询 title 非空的 change 详情
- **THEN** `title` 自记录直读逐字一致；detail DTO 增 `title` / nodes / edges 后运行语料回归测试，diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

#### Scenario: golden 显式重写留痕

- **WHEN** detail DTO 增 `id` 字段（与列表 `id` 投影）后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/detail.rs` | 详情 id 寻址与执行图线面 | `change_detail(..., id)`：`get_change(id)` → 记录 → `locate_change`（记录 name / worktree）；文档形态分支删除；`runs[].nodes` / `runs[].edges` 取代 `runs[].steps`（稳定身份键、边 kind 四值）；`inventory` / `fileLog` / `unparsable` 字段删；槽位三列直读透出（None → null）；纯 derive 零字段属性口径不变 |
| `crates/infra/store`（workspace 库实例，经 port 缝消费） | 状态读源 | ChangeRecord / PhaseRecord / ChecklistItemRecord / RunRecord / RunNodeRecord / RunEdgeRecord 只读查询；时间转换收 queries 层单点（i64 millis → ISO 串 + null） |
| `foundation::layout` | 路径输入 | 产物发现仅经 `Layout` 取目录，不自行拼磁盘路径 |
| `workflow::artifacts` | 产物发现协作 | 详情返回产物清单，发现机制改 phase 清单驱动（见 desktop-artifact-plugins）；带 worktree 记录与 legacy 两类 change 同等可用 |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | 列表 db 单源与 nodes / edges 字段演进走 golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕）；语料见 desktop-corpus-regression |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeDetail.nodes` / `edges` 经 export-bindings 再生成；一致性守卫拦截漂移 |
