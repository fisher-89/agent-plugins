# desktop-corpus-regression Specification (Delta)

## ADDED Requirements

### Requirement: run 运行史语料维度

workflow crate 的 db 种子语料 SHALL 覆盖 run 运行史维度（RunRecord / RunStepRecord，desktop-change-state-store「run 运行史落库」）：同 change 多 run 全史样本（≥2 run，attempt 跨 run 递增不撞号）、interrupted 标定样本（模拟中途死亡残留）、步词汇两态（五落词汇在场 / 流程面步骤与三门零行的缺席断言）、`session_id` 与 `detail` 有无两态。聚合 golden SHALL 覆盖 detail 线面 `runs` / `steps` 投影（全史出线、emit seq 稳定序、时间戳 ISO 串口径）；golden 更新 SHALL 沿显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` 覆写 → 人工确认 diff 预期范围并留痕 → 复核一致），MUST NOT 静默自动重写。种子构造 SHALL 经 store run 域操作面写入（run_start / run_finish 整包同真实写路径），MUST NOT 以裸表插桩绕过操作面（保证词汇过滤与两写时序在语料路径同样成立）。

#### Scenario: run 维度种子在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器与语料清单
- **THEN** 含多 run 全史、interrupted 标定、步词汇与 sessionId 两态样本各至少一个，全部经 store run 域操作面构造

#### Scenario: golden 覆盖 runs / steps 投影

- **WHEN** 对含 run 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs` / `steps` 出线与库内记录逐字一致（全史不截、seq 稳定序、五词汇封闭集），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 缺席断言防词汇漂移

- **WHEN** 语料样本经含流程面步骤（phase_start / phase_log / 三门）的构造序列落库后快照
- **THEN** run_steps 投影零流程面词汇行（过滤词汇单点生效），断言随快照回归常驻

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/`（run 维度种子构造器扩展） | db 种子语料 run 维度 | 经 store run 域操作面构造（run_start / run_finish 整包）；多 run 全史 / interrupted / 步词汇与 sessionId 两态样本；golden 覆盖 `runs` / `steps` 两投影（字段语义见 desktop-change-state-store / desktop-change-queries） |
