# 提案: unit-test-mutation-testing

> **变更**: unit-test-mutation-testing
> **日期**: 2026-07-01
> **状态**: 草稿

---

## 问题

现有 `dev-team unit-test` 命令仅通过覆盖率（coverage）衡量测试质量。覆盖率衡量的是"哪些代码被执行了"，但无法衡量"测试是否真正验证了代码行为"——即使测试覆盖了 100% 的代码行，断言可能为空或过于宽松，导致缺陷漏报。在覆盖率达标的情况下，仍可能出现以下情况：

- 测试仅检查了正向路径（happy path），未覆盖边界条件或错误处理
- 测试断言过于宽松（如仅检查返回值不为 null，未检查具体值）
- 测试未覆盖条件分支的每个结果（如 if-else 的 else 分支）

Mutation testing（变异测试）通过对源代码进行微小变异（如将 `>` 改为 `<`、删除语句、翻转条件），检查现有测试能否"杀死"这些变异体。存活下来的变异体直接暴露了测试缺口。

---

## 提案

在 `dev-team unit-test` 命令中集成 **StrykerJS** 变异测试引擎，作为测试执行流水线的一个阶段。变异测试在覆盖率收集之后自动执行，结果写入统一的单元测试执行报告。

### 核心设计

1. **核心工作流集成**：变异测试是测试执行流程的默认环节，在覆盖率收集后自动运行。支持的框架（jest、vitest、vite-plus）自动执行变异测试；不支持的框架（bun、node-test、go、rust、pytest）静默跳过。
2. **StrykerJS 配置策略**：采用"模板 + 覆盖"模式。如果项目根目录存在 `stryker.config.{json,mjs,cjs}`，则直接使用；否则 CLI 从内置模板生成临时配置文件。
3. **变异范围**：根据测试文件路径推导源代码文件，使用 glob 匹配模式限制变异范围——仅变异被测试覆盖的源文件。
4. **阈值覆盖（overrides）**：`config.test.overrides` 支持 `mutation: { score: N }` 字段，用于按目录覆盖变异得分阈值——与现有的 `coverage` overrides 模式一致。
5. **`--no-mutation` 标志**：CLI 新增 `--no-mutation` 选项，允许用户跳过变异测试阶段。

### 报告结构

变异测试结果写入子报告和汇总报告中的 `mutation` 块，包含：

- `pass`：是否达到阈值
- `score`：变异得分百分比
- `threshold`：得分阈值
- `measured`：各类变异体计数（killed、survived、timeout、noCoverage、compileError、runtimeError、ignored、total、detected、undetected）
- `by_framework`：按框架的详细结果
- `overrides`：按 overrides 配置的逐个分组结果

---

## 能力

### 新增能力

- **mutation-testing** — 在 `dev-team unit-test` 命令中集成 StrykerJS 变异测试引擎，支持 jest/vitest/vite-plus 框架

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — 添加 `mutation` 阈值 schema 和 override 中的 `mutation` 字段
- `plugins/dev-team/bin/src/lib/test-framework.ts` — 在 `FrameworkConfig` 中添加 `mutation_framework` 字段，在 registry 中标记支持的框架
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — 在 `PlanEntry` 中添加 mutation 相关字段
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` — 在 `PlanEntry` schema 中添加 mutation 字段
- `plugins/dev-team/bin/src/lib/test-runner.ts` — 在 `executePlanEntry()` 中添加 mutation 执行阶段（覆盖率后执行 StrykerJS）
- `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` — **新建**：解析 StrykerJS JSON 报告输出
- `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` — **新建**：生成临时 StrykerJS 配置文件
- `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` — 添加 `MutationBlock`、`MutationMeasured` schema 定义
- `plugins/dev-team/bin/src/lib/test-report.ts` — 在子报告和汇总报告生成中添加 mutation 块
- `plugins/dev-team/bin/src/cli.ts` — 添加 `--no-mutation` CLI 选项

### 测试文件

- `plugins/dev-team/bin/src/lib/__tests__/test-parser/mutation-parser.test.ts` — mutation 解析器单元测试
- `plugins/dev-team/bin/src/lib/__tests__/test-parser/stryker-config.test.ts` — stryker 配置生成器单元测试
- `plugins/dev-team/bin/src/lib/__tests__/test-report.test.ts` — 报告生成中 mutation 块的测试

### 不要修改

- `plugins/dev-team/bin/src/lib/test-parser/coverage-parser.ts` — 覆盖率解析逻辑不在此变更中修改
- `plugins/dev-team/bin/src/lib/test-parser/index.ts` — parser dispatch 不涉及 mutation
- `plugins/dev-team/bin/src/commands/unit-test.ts` — 上层编排逻辑由 runner 和 report 内部处理，命令入口不做结构性修改（仅 `--no-mutation` 标志传入）
- `plugins/dev-team/agents/unit-test-evaluator.md` — evaluator 的 checklist 修改属于后续变更

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | config.schema.ts 添加 mutation 阈值 | `config.test.mutation.score` 为可选数字，默认值 80；`config.test.overrides[].mutation` 为可选对象，含 `score` 数字字段 |
| AC-2 | test-framework.ts 添加 mutation_framework | `FrameworkConfig` 包含 `mutation_framework: string | null` 字段；jest/vitest/vite-plus 注册为 `"stryker-js"`，其余框架为 `null` |
| AC-3 | test-detect-frameworks.ts 传递 mutation 字段 | `PlanEntry` 包含 `mutation_framework`、`mutation_config`、`mutation_score` 字段，从 registry 和 config 正确填充 |
| AC-4 | test-runner.ts 执行 StrykerJS | 当 `mutation_framework` 非空时，`executePlanEntry` 在覆盖率解析后执行 StrykerJS；输出存入 `ExecutionResult.mutation`；执行时长计入 `durationMs` |
| AC-5 | stryker-config.ts 生成配置 | 项目根目录存在 `stryker.config.*` 时直接使用；否则生成临时配置，执行后清理 |
| AC-6 | mutation-parser.ts 解析 StrykerJS 报告 | 解析 `reports/mutation/mutation.json` 的 StrykerJS JSON 输出，正确提取 score 和各类变异体计数 |
| AC-7 | unit-test-output.schema.ts 添加 mutation schemas | `MutationMeasured` 包含 killed/survived/timeout/noCoverage/compileError/runtimeError/ignored/total/detected/undetected 字段；`MutationBlock` 包含 pass/score/threshold/measured/by_framework 字段 |
| AC-8 | test-report.ts 生成 mutation 块 | 子报告和汇总报告包含 `mutation` 字段（mutation 框架时）；阈值判定基于 `measured.score >= thresholds.score` |
| AC-9 | cli.ts 添加 --no-mutation | `--no-mutation` 选项跳过所有 mutation 执行；mutation 框架检测仍正常进行 |
| AC-10 | 不支持框架静默跳过 | bun/node-test/go/rust/pytest 的 `mutation_framework` 为 null，执行时跳过 mutation 阶段，report 中 mutation 为 null |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| StrykerJS 执行时间过长 | 变异测试可能需要数分钟，远超测试本身的执行时间 | 中 | 设置 StrykerJS `--timeout` 和 `--maxTestRunnerReuse` 限制；变异范围仅限被测试覆盖的源文件；提供 `--no-mutation` 跳过选项 |
| StrykerJS 依赖安装缺失 | 项目未安装 StrykerJS 相关依赖导致执行失败 | 中 | 检测 `node_modules/.bin/stryker` 是否存在，不存在时提示用户安装并跳过；文档中注明依赖要求 |
| 临时配置文件与用户自定义配置冲突 | 用户已有 `stryker.config.*` 但 CLI 生成的临时配置覆盖其设置 | 低 | "模板 + 覆盖"策略优先检测用户自定义配置，仅在无自定义配置时生成临时配置 |
| 变异测试结果不稳定 | 同一份代码多次执行变异测试结果不一致 | 低 | StrykerJS 默认使用确定性种子；如果出现 flaky test，StrykerJS 的 `--timeoutMS` 配置可减少误报 |
| Windows 兼容性 | StrykerJS 在 Windows 上的路径处理可能有差异 | 低 | 使用 `path.resolve()` 和正斜杠规范化路径；CI 环境中通过 `--no-mutation` 跳过 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 变异测试作为独立命令还是集成到 unit-test？ | 集成到 unit-test 命令 | 变异测试是测试质量评估的自然延伸，与现有覆盖率报告共用输出通道；复用已有框架检测、计划生成和报告管道 | 独立 `dev-team mutation-test` 命令（需额外编排，报告结构冗余） |
| 默认执行还是 opt-in？ | 默认执行，支持的框架自动运行 | 变异测试应成为常规质量门禁，而非可选增强；`--no-mutation` 在需要快速反馈时使用 | opt-in 模式（`--mutation` 标志，易被遗忘） |
| 配置策略：模板 vs 完全自定义？ | "模板 + 覆盖"模式 | 减少用户配置负担：90% 场景可用默认模板；需要精细控制时提供 stryker.config.* 覆盖 | 仅模板（缺乏灵活性）；仅自定义（配置负担高） |
| 变异范围策略？ | 范围限定为被测试覆盖的源文件 | 缩小执行范围，减少变异体数量，加速反馈循环 | 全项目变异（执行时间过长）；仅测试文件变异（无意义） |
| overrides 中 mutation 字段模式？ | 与 coverage 一致的 `{ score: N }` 模式 | 保持配置一致性，降低学习成本；复用现有的 overrides 匹配和计算基础设施 | 独立 `mutation_overrides` 数组（冗余配置结构） |

### 待决问题

- 无

---
