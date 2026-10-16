# desktop-change-queries Specification (Delta)

## ADDED Requirements

### Requirement: 步级时间与 token 出线

detail 线面 `ChangeRunNodeRecord` SHALL 增四字段出线：`startedAt: number` / `endedAt: number | null` / `tokenInput: number | null` / `tokenOutput: number | null`（自 `RunNodeStateRecord` 直读透出）。出线口径 SHALL 为**节点级毫秒数**（与活面 `ChangeStepState` 同形——统一视图 `UnifiedRunStep` 单一形状、`stepFromRecord` 纯复制、running 走秒直减 `Date.now()` 零 parse）；runs 级 `ChangeRunEntry.startedAt` / `finishedAt` 的 ISO 串口径 SHALL 维持不变——两口径并存为有意裁定留痕（节点级时间为节点自有数据而非聚合时刻）。DTO SHALL 保持纯 derive 零字段属性（旧库行缺省值 `0` / `null` 原样出线，MUST NOT 在查询层改写或过滤）；字段演进 SHALL 走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕），bindings 再生成同步前端类型。查询层 SHALL 保持纯读（MUST NOT 回填或派生任何时间 / token 值）。

#### Scenario: 四字段出线与活面同形

- **WHEN** 查询含已收口 WorkerAgent 步（token 有值）与工具步的 run 详情
- **THEN** `runs[].nodes[]` 各行携 `startedAt` / `endedAt`（毫秒数）、`tokenInput` / `tokenOutput`（WorkerAgent 步有值、工具步 null），与活面 `ChangeStepState` 字段形状一致；`stepFromRecord` 无字段转换

#### Scenario: 旧行缺省值原样出线

- **WHEN** 查询含本变更前落库 run 的详情（节点行无步级时间 / token）
- **THEN** 各节点 `startedAt=0` / `endedAt=null` / `tokenInput=null` / `tokenOutput=null` 原样出线（零改写、零回填），聚合不报错、详情其余区块照常

#### Scenario: runs 级 ISO 口径不变

- **WHEN** 对同一详情响应检查 runs 级与节点级时间字段
- **THEN** `runs[].startedAt` / `finishedAt` 仍为 ISO 串 + null；节点级四字段为毫秒数 / null——两口径并存，无相互转换逻辑

#### Scenario: golden 显式重写

- **WHEN** detail DTO 节点级四字段出线后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/detail.rs`（`ChangeRunNodeRecord`） | 线面字段扩展 | 增 `startedAt: number` / `endedAt: number | null` / `tokenInput` / `tokenOutput`（毫秒数口径、纯 derive）；`From<&RunNodeStateRecord>` 直读透出 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `ChangeRunNodeRecord` 四字段再生成；`bindings:check` 守卫拦截漂移 |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | detail nodes 投影四字段入 golden 显式重写流程（语料维度见 desktop-corpus-regression delta） |
