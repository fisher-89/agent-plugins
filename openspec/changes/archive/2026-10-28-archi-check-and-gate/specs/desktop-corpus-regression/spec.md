# desktop-corpus-regression Specification（delta: archi-check-and-gate）

## MODIFIED Requirements

### Requirement: run 运行史语料维度

workflow crate 的 db 种子语料 SHALL 覆盖 run 执行图维度（RunRecord / RunNodeRecord / RunEdgeRecord）：同 change 多 run 全史样本（≥2 run，attempt 跨 run 递增不撞号）、interrupted 标定样本、**节点全词汇样本**（二十二类步词汇在场，含 phase_start / phase_log / 三门 gate、mutation_test、archi_check、initialize 基线步 clean 与 9 个归档步（含 cleanup））、**initialize 相位样本**（5 步节点（phase_start / clean / static_check / test_execution / phase_log）+ 机械 checklist PhaseRecord + 串行 forward 边，含 baseline 槽位报告树工件样本）、**archive 相位样本**（11 步节点 + 空 checklist PhaseRecord + 串行 forward 边）、**feedback 边样本**、**边 kind 四值样本**（forward / feedback / retry / backtrack 各至少一条）、`session_id` / `detail` / `output_ref` 有无两态。聚合 golden SHALL 覆盖 detail 线面 `runs` / `nodes` / `edges` 投影（全史出线、稳定身份键、二十二值全词汇、边 kind 四值、时间戳 ISO 串口径）；golden 更新 SHALL 沿显式重写流程。种子构造 SHALL 经 store run 域操作面写入，MUST NOT 以裸表插桩绕过操作面。

#### Scenario: run 维度种子在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器与语料清单
- **THEN** 含多 run 全史、interrupted 标定、节点二十二值全词汇（含 `archi_check`）、initialize 相位、archive 相位、feedback 边、边 kind 四值、sessionId / detail / outputRef 两态样本各至少一个，全部经 store run 域操作面构造

#### Scenario: golden 覆盖 nodes / edges 投影

- **WHEN** 对含 run 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs` / `nodes` / `edges` 出线与库内记录逐字一致（全史不截、稳定身份键、边 kind 四值、零 seq / 零过滤），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 全词汇与 archive 断言防漂移

- **WHEN** 语料样本经含流程面步骤（phase_start / phase_log / 三门）、mutation_test、archi_check、initialize 基线步（clean / static_check / test_execution）、9 个归档步（含 cleanup）与 feedback 重跑的构造序列落库后快照
- **THEN** run_nodes 投影二十二类词汇全在案（零过滤）、initialize 相位 5 步节点以 forward 边串行、archive 相位 11 步节点以 forward 边串行、feedback 重跑为 `instanceNo` 递增独立节点、run_edges 投影含 feedback 边；断言随快照回归常驻

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/corpus_golden_test.rs` + 种子构造器 | 语料维度随动 | 全词汇样本补 `archi_check` 节点（implement 相位，feedback 边样本可复用既有边）；golden 显式重写留痕 |
| `tests/fixtures/`（db 种子） | 构造面不变 | 种子仍全经 store run 域操作面写入，MUST NOT 裸表插桩 |
