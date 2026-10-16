# desktop-corpus-regression Specification (Delta)

## ADDED Requirements

### Requirement: 步级时间与 token 语料维度

run 运行史 db 种子语料 SHALL 覆盖步级时间与 token 维度（承接「run 运行史语料维度」，样本字段随 `ChangeStepState` 四字段扩展）：种子构造器构造 `ChangeStepState` 字面量时 SHALL 以**定值注入** `started_at` / `ended_at` / `token_input` / `token_output`（零 wall-clock 进 corpus——emit 生产时钟不进种子路径，沿「corpus 确定性由测试注入承载」口径）；样本 SHALL 含**时间有无两态**（正常起止双值步 / 瞬步 `started_at == ended_at` / running 收口在途 `ended_at=None`）与**token 有无两态**（WorkerAgent 步带 token 值 / 工具步 None）各至少一个。聚合 golden SHALL 覆盖 detail 线面 nodes 四字段投影（毫秒数口径，见 desktop-change-queries delta）；golden 更新 SHALL 沿显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` → 人工确认 diff 预期范围留痕 → 复核一致），MUST NOT 静默自动重写。种子构造 SHALL 经 store run 域操作面写入（`finish_command` 组装路径），MUST NOT 以裸表插桩绕过操作面。

#### Scenario: 种子定值与维度两态在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器对 `ChangeStepState` 的构造面
- **THEN** 四字段均以定值字面量注入（零 wall-clock 调用）；正常双值步、瞬步同刻、running 在途（ended None）、token 有值与 None 两态样本各至少一个，全部经 store run 域操作面落库

#### Scenario: golden 覆盖四字段投影

- **WHEN** 对含步级时间与 token 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs[].nodes[]` 四字段出线与库内记录逐字一致（毫秒数、None → null），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 确定性回归可复现

- **WHEN** 连续两次运行快照回归测试
- **THEN** 输出与 golden 全等（定值注入下零时间漂移），无 flaky 时间断言

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/`（种子构造器扩展） | 步级维度样本 | `ChangeStepState` 字面量补四字段（定值注入）；时间两态（双值 / 瞬步同刻 / ended None）与 token 两态样本；经 store run 域操作面构造 |
| `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/golden/` | golden 随动 | detail nodes 四字段投影入 golden；diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕；既有二十值词汇与 archive 断言不回归 |
