# desktop-corpus-regression Specification (Delta)

## Purpose

定义 workflow crate 的语料回归防线：db 种子夹具 + 磁盘产物树入仓管理，以全量聚合 + golden 快照对比的回归测试捕获 db 聚合与 detail 线面投影的静默劣化。语料随归档并入 phase → step 模型扩围：archive 列、8 个归档步词节点、git 步分类、空 checklist PhaseRecord 与 stop-before-archive 收口语义入样本与 golden。

## MODIFIED Requirements

### Requirement: 三代代表性 fixture 语料

workflow crate SHALL 在 `tests/fixtures/` 维护语料夹具，载体 SHALL 为 **db 种子夹具**（经 store change 域操作面写入 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` / `RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 的种子构造器——种子携带铸出 id 归键）+ 磁盘产物树。语料 SHALL 覆盖：完整执行史（多 attempt、fail→retry、backtrack 与 stale 标记、skipped 条目）、会话槽位全/缺两态、坏行样本、**归档 change 样本**（db 记录 name 恒裸名 + 磁盘目录带日期前缀 + status=archived + **archive 列空 checklist PhaseRecord 与 10 步节点样本**）、worktree 维度两态、title 维度两态。原 workflow.json 语料随 parse 退役失去解析主体，处置形态由 design 定稿，MUST NOT 静默遗留在默认语料路径参与回归。

#### Scenario: 语料覆盖状态面形态

- **WHEN** 检查 `tests/fixtures/` 语料清单与种子构造器
- **THEN** 含完整执行史（含 backtrack / stale / skipped）、槽位全缺两态、归档前缀样本（含 archive 列空 checklist 与 10 步节点）、坏行样本、worktree 两态建档样本、title 两态样本各至少一个，全部经 db 种子构造、id 归键

#### Scenario: 归档样本全状态面入 golden

- **WHEN** 对归档样本（db name 裸名 + 磁盘目录 `YYYY-MM-DD-<name>` 前缀 + archive 列在案）运行列表与详情聚合
- **THEN** list golden 该条目 name 为裸名、status=archived、月分组取自 `archived_at`；detail golden status=archived、10 站 pipeline 与 runs 全量出线（archive 列 PhaseRecord 与 step 节点在案）；差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

### Requirement: run 运行史语料维度

workflow crate 的 db 种子语料 SHALL 覆盖 run 执行图维度（RunRecord / RunNodeRecord / RunEdgeRecord）：同 change 多 run 全史样本（≥2 run，attempt 跨 run 递增不撞号）、interrupted 标定样本、**节点全词汇样本**（十八类步词汇在场，含 phase_start / phase_log / 三门 gate 与 8 个归档步）、**archive 相位样本**（10 步节点 + 空 checklist PhaseRecord + 串行 forward 边）、**feedback 边样本**、**边 kind 四值样本**（forward / feedback / retry / backtrack 各至少一条）、`session_id` / `detail` / `output_ref` 有无两态。聚合 golden SHALL 覆盖 detail 线面 `runs` / `nodes` / `edges` 投影（全史出线、稳定身份键、十八值全词汇、边 kind 四值、时间戳 ISO 串口径）；golden 更新 SHALL 沿显式重写流程。种子构造 SHALL 经 store run 域操作面写入，MUST NOT 以裸表插桩绕过操作面。

#### Scenario: run 维度种子在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器与语料清单
- **THEN** 含多 run 全史、interrupted 标定、节点十八值全词汇、archive 相位、feedback 边、边 kind 四值、sessionId / detail / outputRef 两态样本各至少一个，全部经 store run 域操作面构造

#### Scenario: golden 覆盖 nodes / edges 投影

- **WHEN** 对含 run 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs` / `nodes` / `edges` 出线与库内记录逐字一致（全史不截、稳定身份键、边 kind 四值、零 seq / 零过滤），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 全词汇与 archive 断言防漂移

- **WHEN** 语料样本经含流程面步骤（phase_start / phase_log / 三门）、8 个归档步与 feedback 重跑的构造序列落库后快照
- **THEN** run_nodes 投影十八类词汇全在案（零过滤）、archive 相位 10 步节点以 forward 边串行、feedback 重跑为 `instanceNo` 递增独立节点、run_edges 投影含 feedback 边；断言随快照回归常驻

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/`（种子构造器扩展） | db 种子语料 archive 维度 | archive 相位 10 步节点 + 空 checklist PhaseRecord 样本；节点十八值全词汇；经 store run 域操作面写入 |
| `crates/core/workflow/tests/golden/` | 快照对账 | detail golden 10 站 pipeline、十八值 nodes、archive 列投影；`DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → diff 人工确认留痕 → 复核一致 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | 回归防线 | archive 样本入列；十八值全词汇与 archive 断言常驻；db 种子 → 聚合 → golden 比对；显式行为约束不豁免 |
