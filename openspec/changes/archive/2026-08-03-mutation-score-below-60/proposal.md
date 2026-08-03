# 提案: mutation-score-below-60

> **变更**: mutation-score-below-60
> **日期**: 2026-07-31
> **状态**: draft
> **工作流**: test-only

---

## 问题

`plugins/dev-team` 在归档变更 `2026-07-31-test-execution-result-file` 的 Stryker 报告（`reports/test/plugins_dev-team_vite-plus/mutation.json`，阈值 `low: 60`）中，有 7 个源文件 mutation score 低于 60%。Survived / NoCoverage 突变体偏多，说明现有 colocated 单测对分支、字面量、正则与路径归一化等细节的断言不够尖锐，变异体存活后无法被发现。

| 文件 | Score | Killed | Survived | NoCoverage |
|------|------:|-------:|---------:|-----------:|
| `bin/src/lib/test-framework.ts` | 11.49 | 20 | 133 | 21 |
| `bin/src/lib/test-runner.ts` | 31.19 | 121 | 189 | 78 |
| `bin/src/lib/test-parser/stryker-config.ts` | 32.81 | 21 | 24 | 19 |
| `bin/src/lib/test-report.ts` | 45.06 | 283 | 311 | 34 |
| `bin/src/lib/test-parser/go-parser.ts` | 50.00 | 71 | 53 | 18 |
| `bin/src/lib/test-parser/text-parser.ts` | 53.19 | 200 | 173 | 3 |
| `bin/src/commands/test-detect-frameworks.ts` | 58.85 | 113 | 46 | 33 |

已 ≥ 60 的同包文件（`coverage-parser.ts`、`js-parser.ts`、`index.ts`、`mutation-parser.ts`、`test-execution.ts` 等）不在本次范围内。

---

## 提案

在 **test-only** 工作流下，针对上表 7 个文件**优先补强/新增 colocated 单元测试**，使每个文件的 Stryker mutation score（`killed / (killed + survived + noCoverage)`）达到 **≥ 60**。

策略：

1. **以归档 `mutation.json` 为缺口清单** — 优先消灭 Survived，其次覆盖 NoCoverage；按 mutator 类型（StringLiteral、ConditionalExpression、LogicalOperator、Regex、BlockStatement、EqualityOperator 等）设计尖锐断言。
2. **只测公共 API 与可观测行为** — 通过已导出函数（`getFrameworkConfig`、`detectFrameworkVersion`、`executePlanEntry`、`resolveStrykerConfig`、`generateSubReport` / `generateSummaryReport`、`parseGoOutput`、`parseTextOutput`、`runTestDetectFrameworks`）及必要 mock（`fs`、`execCommand`、subprocess）杀变异体；不新增仅为测试暴露的 `export`。
3. **不改生产逻辑为默认** — 除非测试明确暴露真实 bug；届时最小修复并在风险中记录。
4. **验证方式** — 单元测试通过后，对 `plugins/dev-team` 再跑 Stryker（或等价 `dev-team test-execution` 含 mutation），确认上述 7 文件各自 score ≥ 60。

---

## 能力

### 新增能力

- **mutation-score-gap-fill** — 针对 `plugins/dev-team` 中 mutation score < 60 的源文件，通过补强 colocated 单测将各文件分数提升至 ≥ 60；约定目标文件清单、阈值计算、测试放置方式与生产代码改动边界。

### 修改的能力

- （无）本次不改变 mutation 执行链路、框架检测或报告生成的产品行为；既有能力 `mutation-testing`、`test-detect-frameworks`、`unit-test-executor` 等仅作为被测面，不新增/修改其需求条文。

---

## 变更范围

### 实现文件

- （默认无）生产源码不在范围内；仅当单测证实真实 bug 时，允许对下列文件做**最小**修复：
  - `plugins/dev-team/bin/src/lib/test-framework.ts`
  - `plugins/dev-team/bin/src/lib/test-runner.ts`
  - `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts`
  - `plugins/dev-team/bin/src/lib/test-report.ts`
  - `plugins/dev-team/bin/src/lib/test-parser/go-parser.ts`
  - `plugins/dev-team/bin/src/lib/test-parser/text-parser.ts`
  - `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`

### 测试文件

- `plugins/dev-team/bin/src/lib/test-framework.test.ts`
- `plugins/dev-team/bin/src/lib/test-runner.test.ts`
- `plugins/dev-team/bin/src/lib/test-parser/stryker-config.test.ts`
- `plugins/dev-team/bin/src/lib/test-report.test.ts`
- `plugins/dev-team/bin/src/lib/test-parser/go-parser.test.ts`
- `plugins/dev-team/bin/src/lib/test-parser/text-parser.test.ts`
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`
- 若某场景更适合集成夹具，可增量扩展 `plugins/dev-team/bin/__tests__/` 下既有 mutation/执行流测试（仍优先单测）

### 不要修改

- 已 ≥ 60 的源文件及其测试（如 `coverage-parser`、`js-parser`、`mutation-parser`、`test-execution` 命令主体等），除非共享夹具/类型被迫联动
- Stryker 阈值配置本身、归档报告路径约定、生产 mutation 执行产品行为（`mutation-testing` 能力）
- 插件版本 bump / dual-product build（无生产交付变更时）
- OpenSpec 工作流引擎、agent 定义、非 `plugins/dev-team` 包
- 仅为测试而新增的生产 `export`（违反 knip / 项目「No test-only exports」规则）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `test-framework.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-2 | `test-runner.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-3 | `stryker-config.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-4 | `test-report.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-5 | `go-parser.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-6 | `text-parser.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-7 | `test-detect-frameworks.ts` 补测 | 对该文件重跑 Stryker 后 mutation score ≥ 60 |
| AC-8 | 既有单测回归 | 目标包既有相关单测全部通过；新增用例 colocated 且不引入 test-only export |
| AC-9 | 生产改动边界 | 无未说明的生产逻辑变更；若有 bugfix，须在变更说明中单列文件与原因 |

分数定义与归档报告一致：`killed / (killed + survived + noCoverage)`；证据为 `plugins/dev-team` 下更新后的 Stryker JSON（或等价 test-execution mutation 产物）中对应 `files` 条目。

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 等价变异体（Equivalent mutants）无法被合理测试杀死 | 个别文件卡在阈值附近 | 中 | 优先杀非等价 Survived；对明显等价变异体在 test-design 中标注并依赖覆盖其他分支拉高整体分数 |
| `test-framework` / `test-runner` Survived 数量极大，补测成本高 | 工期拉长 | 高 | 按 mutator 与热点行分批；先覆盖 registry 字面量与执行分支，再处理边缘路径 |
| Regex 变异体（尤其 `text-parser`）难用正向用例全杀 | score 提升有限 | 中 | 补充负向/边界输入（缺空格、错位标记、部分匹配）与分层 fallback 断言 |
| 补测时发现真实生产 bug | 被迫改生产代码 | 低 | 最小修复 + 回归用例；不扩大重构范围 |
| 全量 Stryker 耗时长 | 反馈慢 | 中 | 验证阶段可对 7 文件 `mutate` 收窄；最终以整包或同配置报告为准 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 是否改生产代码以「方便测」？ | 否，除非真实 bug | 符合 test-only 与「No test-only exports」 | 抽取纯函数并 export（拒绝，污染 knip） |
| 能力建模方式 | 新增 `mutation-score-gap-fill`，不修改既有产品能力条文 | 本次是质量门禁补测，非功能变更 | 在 `mutation-testing` 上 ADDED（混淆产品功能与自测质量） |
| 基线报告 | 使用归档 `2026-07-31-test-execution-result-file` 的 `mutation.json` | explore 指定且可复现 | 重新跑一次仅作验证，不改目标清单 |
| 阈值 | 每文件 ≥ 60（与报告 `low` 一致） | 与缺口定义一致 | 抬到 70/80（超出本次 explore 范围） |

### 待决问题

- 最终验证是「仅 mutate 这 7 个文件」还是「整包 vite-plus suite 全量 mutation」——验收以各文件 score ≥ 60 为准，执行策略可在 test-execution 阶段按耗时选择。
