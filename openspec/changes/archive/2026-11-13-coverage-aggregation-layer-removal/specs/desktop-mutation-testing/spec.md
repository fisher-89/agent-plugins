# desktop-mutation-testing Specification（delta: coverage-aggregation-layer-removal）

## MODIFIED Requirements

### Requirement: score 口径与逐 suite 阈值判定

突变得分 SHALL 沿既有公式：`score = (killed + timeout) / (total - ignored - compileError - runtimeError)`（分母排除 ignored + compileError + runtimeError；分母为 0 → 100.0），MUST NOT 引入百分比平均等新口径。阈值 SHALL 取 suite 配置 `mutation.score`（`.dev-team/config.json` 既有字段：desktop 80 / src-tauri 70）。门禁结论 SHALL 为逐 suite 判定、全过即 pass（任一 suite `score < threshold` 即 fail）。多 suite 计数归并 SHALL 沿 `merge_mutation_blocks` 既有计数求和公式（桶块当输入，MUST NOT 新增归并公式）。归并 MutationBlock SHALL **只服务载荷展示**（`MutationTestOutcome.score` / `threshold` / `counts`），MUST NOT 落 `summary.json`——summary 已无 `mutation` 位（原回填面 `backfill_summary` 与 `aggregate_mutation` 一并退役）。载荷 score 是**展示口径**（计数求和后按首块阈值换算），MUST NOT 作门禁判据——门禁判据恒为逐 suite `pass` 布尔（`suite_results.iter().all(|(_, block)| block.pass)`），本变更对该判据零改动。

#### Scenario: 公式与分母排除

- **WHEN** 某 suite 突变计数为 killed=80 / survived=10 / timeout=5 / ignored=3 / compileError=2 / runtimeError=0 / total=100
- **THEN** score = (80+5)/(100-3-2-0) = 89.47（两位舍入），与既有 score 公式逐字一致

#### Scenario: 逐 suite 判定全过即 pass

- **WHEN** 一次 MutationTest 覆盖 desktop（阈值 80）与 src-tauri（阈值 70）两 suite，得分 82 / 68
- **THEN** 门禁结论 fail（src-tauri 未达标），反馈携未达标 suite 的 score / threshold 与报告路径清单

#### Scenario: 归并块只做载荷不落盘

- **WHEN** 一次 MutationTest 覆盖两 suite 且执行完毕，随后审查 `reports/test/summary.json` 与 `MutationTestOutcome`
- **THEN** summary.json 无 `mutation` 键（`backfill_summary` 面不存在）；载荷 `score` / `threshold` / `counts` 仍在且等于归并块的值，逐 suite 明细在 `feedback_text` 与报告树可查

## Module Contract

### Function: `packages/desktop/src-tauri/crates/core/checks/src/aggregate.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `aggregate_mutation` | **删除** | 消费面（子报告携块 → 汇总块）随 `SummaryReport.mutation` 位退役 |
| `build_mutation_block` / `merge_mutation_blocks` | **零改动** | 逐 suite 段块判定与载荷展示归并的唯一实现；score 公式单点不动 |

### Function: `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `backfill_summary` + 其调用点 | **删除** | 二次写盘 summary.json 塞 mutation 块的面退役；summary 无 `mutation` 位 |
| `merge_mutation_blocks(&blocks)` 调用 | 保留 | 仍为 `MutationTestOutcome.score` / `threshold` / `counts` 与 `fail_feedback` 提供归并块（内存态，不落盘） |
| `all_pass`（逐 suite 全过判定） | **零改动** | 门禁判据仍为 `suite_results.iter().all(\|(_, block)\| block.pass)`——归并 score 不参与 |
