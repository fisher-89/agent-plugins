# desktop-change-state-store Specification（delta: archi-check-and-gate）

## MODIFIED Requirements

### Requirement: run 执行图落库

run 状态 SHALL 落 workspace 库（`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 三表，`change_id` 归键，全史保留不截 last_run；attempt 经 `phase_start` max+1 分配跨 run 不撞号）。写时序 SHALL 为**每 run 两写**：run 行 SHALL 在 run 发起时以 `status=running` 建立，run 收口时 SHALL 以终态更新 + 该 run 全部节点 + 边整包**单事务**落库；MUST NOT 每步写放大。run 收口同事务处置 `ChangeRecord.active_phase` SHALL 以 `change_id` 定位记录。

执行图节点 SHALL 为**全词汇封闭集单点定义**：`RunNodeRecord` 落全部二十二类步词汇（agent 阶段 `executor` / `evaluator` / `decision`、脚本阶段 `static_check` / `test_execution` / `mutation_test` / `archi_check`、流程面步骤 `phase_start` / `phase_log`、三门 gate `verdict_gate` / `retry_gate` / `whitelist_gate`、归档步 `preflight` / `commit` / `rebase` / `resolve_conflicts` / `merge` / `spec_sync` / `cleanup` / `seal` / `finalize`），MUST NOT 过滤。新增步词 SHALL 一律**尾部追加**（`archi_check` 为 21→22 的尾部追加；`RunNodeRecord.step` 经 bincode 按下标编码，中途插入 / 重排会令存量节点行步词整体错位——本条为白名单式扩展的唯一合法形态，MUST NOT 以「按相位名或步名重排」方式整理词汇序）。节点身份键 SHALL 为 `(run_id, phase, attempt, step, instance_no)`（`instance_no` 每次执行递增，feedback 重跑为独立新节点）。`RunNodeRecord` 的 `session_id` / `detail` / `output_ref` SHALL 随行落库；节点 `session_id` SHALL 为前端会话转录的 sessionId 直查键（WorkerAgent 步在案、非 WorkerAgent 步为空；前端零 provenance `source_ref` 反查，会话 `source_ref` 留痕仅供后端 provenance，见 desktop-change-orchestration「会话挂靠 provenance 与转录 session id 直查」）。

执行图边 SHALL 单独落 `RunEdgeRecord`：`edge_kind ∈ { forward, feedback, retry, backtrack }`，`from_node_key` / `to_node_key` 指向节点身份键；`run_id` 二级索引与 `from_node_key` 二级索引 SHALL 均在案。前端 `deriveEdges` 列差 Δ 派生 SHALL 退役（读边表）。

`RunNodeRecord` / `RunEdgeRecord` SHALL 与既有 `StepRecord` 审计面**职责分立**：StepRecord 为相位机步骤审计行，RunNodeRecord / RunEdgeRecord 为执行图节点史 + 边史；两表 MUST NOT 互为替代。run 中途崩溃（无 finish 落包）SHALL 丢该 run 节点 / 边史——start 行经启动标定可见，为既定权衡留痕。

#### Scenario: 两写时序与零写放大

- **WHEN** 假引擎驱动一相位 run 走完（含 executor / static-check / evaluator 多步与 feedback 边）
- **THEN** 库写事务恰两次：发起时 RunRecord（running）落行、收口时同事务完成终态更新与全部 RunNodeRecord + RunEdgeRecord 整包落库；期间零每步写事务

#### Scenario: 节点与边整包原子

- **WHEN** finish 落包过程中某环节失败（如节点或边行编码错误注入）
- **THEN** 写事务整体回滚：无 RunNodeRecord / RunEdgeRecord 半包、RunRecord 终态与 finished_at 零变化

#### Scenario: 全词汇与稳定身份键

- **WHEN** 一次 run 覆盖全部二十二类步词汇（含 phase_start / phase_log / 三门 gate、mutation_test、archi_check、9 个归档步与 feedback 重跑）后查询 run_nodes / run_edges
- **THEN** 二十二类词汇全落 RunNodeRecord（零过滤）；身份键 `(run_id, phase, attempt, step, instance_no)` 稳定唯一，feedback 重跑为 `instance_no` 递增新节点；边表 feedback / retry / backtrack / forward 四 kind 齐备

#### Scenario: 归档步复用 RunNodeRecord

- **WHEN** archive 相位一次 run 收口后查询
- **THEN** 归档 9 个步词节点（含 cleanup）与 PhaseStart / PhaseLog 全落 RunNodeRecord（零新表、零 archive 独立落库），串行 forward 边落 RunEdgeRecord

#### Scenario: 边表两向查询

- **WHEN** 按 `run_id` 全量读某 run 的边、按 `from_node_key` 读某节点后继
- **THEN** 两二级索引均可达且结果一致；`from_node_key` 后继查询能枚举 feedback / forward / retry / backtrack 出边

#### Scenario: 全史保留与跨 run 不撞号

- **WHEN** 同一 change（同一 id）先后发起两次 run（第二次自续走锚点推进）
- **THEN** 两 run 的 RunRecord 与各自 RunNodeRecord / RunEdgeRecord 全量在库，同 phase 的 attempt 经 max+1 分配递增不撞号

#### Scenario: 崩溃丢节点边史为既定权衡

- **WHEN** run 运行中途进程被 kill（finish 落包未达）后重启查询
- **THEN** 该 run 的 RunRecord 经启动标定为 interrupted、其 RunNodeRecord / RunEdgeRecord 零行；attempt 级评估史仍可达自 PhaseRecord；库中无每步写透补偿痕迹

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/state.rs`（`RunStepKind`） | 库侧步词封闭集 | `ArchiCheck` 尾部追加（21→22，`as_str` = `archi_check`）；与 `StepNode` / `ChangeStepKind` 同词面 1:1 |
| `crates/core/workflow/src/state.rs`（`StepKind`，审计命令包络） | 审计步词封闭集 | `ArchiCheck` 尾部追加（八臂 → 九臂；`as_str` = `archi_check`）；落 `StepRecord.step_kind`（`native_model` 版本链承接，存量审计行解码语义不变） |
| `crates/infra/store/src/model.rs`（`RunNodeRecord.step`） | 落库编码 | bincode 下标编码不变，新变体只追加；表结构零变化、零迁移——存量节点行解码语义逐字不变 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | 种子语料随动 | 全词汇样本扩一条 `archi_check` 节点（见 desktop-corpus-regression「run 运行史语料维度」）；golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕 |
