# desktop-corpus-regression Specification (Delta)

## MODIFIED Requirements

### Requirement: run 运行史语料维度

workflow crate 的 db 种子语料 SHALL 覆盖 run 执行图维度（RunRecord / RunNodeRecord / RunEdgeRecord）：同 change 多 run 全史样本（≥2 run，attempt 跨 run 递增不撞号）、interrupted 标定样本、**节点全词汇样本**（二十二类步词汇在场，含 phase_start / phase_log / 三门 gate、mutation_test、archi_check、initialize 基线步 clean 与 9 个归档步（含 cleanup））、**initialize 相位样本**（3 步节点（clean / static_check / test_execution）+ 机械 checklist PhaseRecord + 串行 forward 边，含 baseline 槽位报告树工件样本）、**archive 相位样本**（9 步节点 + 空 checklist PhaseRecord + 串行 forward 边）、**feedback 边样本**、**边 kind 四值样本**（forward / feedback / retry / backtrack 各至少一条）、`session_id` / `detail` / `output_ref` 有无两态。聚合 golden SHALL 覆盖 detail 线面 `runs` / `nodes` / `edges` 投影（全史出线、稳定身份键、二十二值全词汇、边 kind 四值、时间戳 ISO 串口径）；golden 更新 SHALL 沿显式重写流程。种子构造 SHALL 经 store run 域操作面写入，MUST NOT 以裸表插桩绕过操作面。

**全词汇样本与真实相位形态 SHALL 分两条口径**（辅助步退场后二者不再一致）：`phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` 五词转**存量解码专用**（新写面零产出），但仍 MUST 有语料样本——它们的存在本身是「旧库行可解码」的回归面，词面 MUST NOT 因「新 run 不产」而从语料中删除。真实相位形态样本（initialize / archive / test-execution 等）SHALL 按退场后的步数构造（3 / 9 / 2），二者在语料集内并列、互不替换。

#### Scenario: run 维度种子在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器与语料清单
- **THEN** 含多 run 全史、interrupted 标定、节点二十二值全词汇（含 `archi_check` 与三辅助步 / 三门 gate 的存量解码样本）、initialize 相位、archive 相位、feedback 边、边 kind 四值、sessionId / detail / outputRef 两态样本各至少一个，全部经 store run 域操作面构造

#### Scenario: golden 覆盖 nodes / edges 投影

- **WHEN** 对含 run 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs` / `nodes` / `edges` 出线与库内记录逐字一致（全史不截、稳定身份键、边 kind 四值、零 seq / 零过滤），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 全词汇与 archive 断言防漂移

- **WHEN** 语料样本经含流程面步骤（phase_start / phase_log / 三门）、mutation_test、archi_check、initialize 基线步（clean / static_check / test_execution）、9 个归档步（含 cleanup）与 feedback 重跑的构造序列落库后快照
- **THEN** run_nodes 投影二十二类词汇全在案（零过滤）、initialize 相位样本 3 步节点以 forward 边串行、archive 相位样本 9 步节点以 forward 边串行、feedback 重跑为 `instanceNo` 递增独立节点、run_edges 投影含 feedback 边；断言随快照回归常驻

#### Scenario: 解码专用词汇仍有语料样本

- **WHEN** 审查语料集对 `phase_start` / `phase_log` / `verdict_gate` / `retry_gate` / `whitelist_gate` 五词的覆盖
- **THEN** 五词各至少一个节点样本与对应边（旧库行解码回归面），且样本不与「真实相位形态样本」混同注释（标注为存量形态）；MUST NOT 因新写面零产出而删除这些样本

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/corpus_golden_test.rs` | 种子构造与断言 | initialize 相位样本 5 节点 → 3 节点、archive 相位样本 11 节点 → 9 节点；三辅助步 / 三门 gate 节点样本保留（标注存量解码形态）；全词汇与边 kind 四值断言不变 |
| `crates/core/workflow/tests/golden/*.json` | 入仓 golden | 随种子变更经 `DESKTOP_GOLDEN_REWRITE=1` 重写，人工确认 diff 后入库；重写后不带环境变量复跑零 diff |
| `crates/core/workflow/tests/fixtures/README.md` | 语料矩阵文档 | 补「解码专用词汇样本 vs 真实相位形态样本」两条目说明 |
