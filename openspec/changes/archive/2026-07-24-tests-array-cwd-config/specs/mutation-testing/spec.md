## MODIFIED Requirements

### Requirement: config.schema.ts 的 mutation 阈值配置

**ID**: REQ-MT-8
**Priority**: MUST
**Description**: `schemas/config/config.schema.ts` SHALL 在每个 **suite**（`tests[]` 元素）上定义可选 `mutation` 对象，包含可选 `score` 数字字段，默认值来自 schema 目录常量（`TEST_MUTATION_SCORE_DEFAULT`，当前为 70）。SHALL NOT 再在顶层旧 `test` 对象上提供全局 `mutation` 块，亦 SHALL NOT 再提供 `test.overrides[].mutation` 级联模型。

#### Scenario: suite mutation 阈值默认值

**WHEN** suite 未在 config.json 中设置 `mutation`
**THEN** 经 schema parse 后该 suite 的 `mutation.score` SHALL 等于常量默认值

#### Scenario: suite 显式 mutation 阈值

**WHEN** suite 为 `{ "root": "src", "framework": "vitest", "mutation": { "score": 90 } }`
**THEN** 该 suite 的 mutation score 阈值 SHALL 为 `90`

#### Scenario: 无全局 mutation 级联

**WHEN** 配置仅含 `tests` 数组、无旧 `test.mutation`
**THEN** 各 suite 阈值互相独立
**AND** 实现 MUST NOT 再读取 `config.test.mutation` 作为回退

### Requirement: PlanEntry 传递 mutation 字段

**ID**: REQ-MT-9
**Priority**: MUST
**Description**: `PlanEntry` 接口和 schema SHALL 包含 `mutation_framework: string | null` 和 `mutation_score: number | null` 字段。构建 PlanEntry 时 SHALL 从 `getFrameworkConfig(framework).mutation_framework` 获取 mutation_framework，从**对应 suite** 解析后的 `mutation.score` 获取 mutation_score（不再读全局 `config.test.mutation.score`）。

#### Scenario: 支持框架的 PlanEntry 含 mutation_framework

**WHEN** plan 由 vitest suite 构建
**THEN** 生成的 `PlanEntry` 的 `mutation_framework` SHALL 为 `"stryker-js"`
**AND** `mutation_score` SHALL 为该 suite 的 `mutation.score`

#### Scenario: 不支持框架的 PlanEntry 的 mutation_framework 为 null

**WHEN** plan 由 go suite 构建
**THEN** 生成的 `PlanEntry` 的 `mutation_framework` SHALL 为 `null`

### Requirement: mutation overrides 计算

**ID**: REQ-MT-11
**Priority**: MUST
**Description**: `lib/test-report.ts` 中按分组计算 mutation 结果的逻辑 SHALL 适配 `tests[]` 模型。每个声明了自定义 `mutation.score` 的 suite（或每个 plan 条目）可作为一组，基于该 suite scope（`root` ∩ `includes` ∩ ¬`excludes`）匹配的源文件在 StrykerJS 报告 `files` 中的数据计算实际 score，并与该 suite 阈值比较。

SHALL NOT 再依赖 `config.test.overrides[].file` + `overrides[].mutation` 作为配置来源。

由于 StrykerJS 报告按文件提供变异体数据，计算方式为：
1. 根据 suite scope 匹配源文件
2. 从 StrykerJS 报告的 `files` 字段中提取匹配文件的变异体数据
3. 计算匹配文件的加权平均 mutation score
4. 对比该 suite 阈值生成 `pass` 结果

#### Scenario: suite 匹配部分源文件

**WHEN** suite 配置 `{ "root": "src/core", "framework": "vitest", "mutation": { "score": 90 } }`
**AND** StrykerJS 报告中匹配文件的加权平均 score 为 83.3
**THEN** 该分组的 `pass` SHALL 为 `false`（83.3 < 90）
**AND** `file_count` SHALL 为匹配的源文件数

#### Scenario: suite 使用默认 mutation 阈值时仍可分组展示

**WHEN** suite 未显式写 `mutation`（使用 schema 默认 score）
**THEN** 报告仍可按 suite/plan 展示 mutation 结果
**AND** MUST NOT 要求旧 overrides 数组存在

### Requirement: 子报告生成中包含 mutation 块

**ID**: REQ-MT-5
**Priority**: MUST
**Description**: `lib/test-report.ts` 的 `generateSubReport` SHALL 在 `ExecutionResult` 包含 `mutation` 字段时，生成 mutation 块写入子报告。mutation 块 SHALL 包含：
- `pass` — `score >= threshold` 的计算结果
- `score` — 从 parser 解析的变异得分
- `threshold` — 从**对应 suite / PlanEntry.mutation_score** 读取的阈值（非全局 `test.mutation`）
- `measured` — 各类变异体计数
- `by_framework` — 当前框架的 mutation 结果
- `overrides` — 按 suite 分组的结果（若报告 schema 仍使用该字段名）
- `error` — 仅当 mutation 执行失败时存在

#### Scenario: vitest 子报告包含 mutation 块

**WHEN** `ExecutionResult.mutation` 包含 `{ score: 85, measured: { killed: 20, survived: 3, total: 25, ... } }`
**AND** 对应 suite / plan entry 的 `mutation_score` 为 `70`
**THEN** 子报告 `mutation` 块的 `pass` SHALL 为 `true`（85 >= 70）
**AND** `score` SHALL 为 `85`
**AND** `threshold` SHALL 为 `70`

#### Scenario: mutation 不达标时 pass 为 false

**WHEN** `ExecutionResult.mutation.score` 为 `65`
**AND** suite `mutation_score` 为 `70`
**THEN** 子报告 `mutation.pass` SHALL 为 `false`

#### Scenario: mutation 执行失败时 mutation 块含 error

**WHEN** `ExecutionResult.mutation` 包含 `{ error: "StrykerJS not installed" }`
**AND** `score` 为 `null`
**THEN** 子报告 `mutation` 块的 `pass` SHALL 为 `false`
**AND** `error` SHALL 为 `"StrykerJS not installed"`

#### Scenario: mutation 框架跳过时 mutation 为 null

**WHEN** `entry.mutation_framework` 为 `null`（不支持的框架）
**THEN** `ExecutionResult.mutation` SHALL 为 `null`
**AND** 子报告 `mutation` 字段 SHALL 为 `null`

---

## ADDED Requirements

### Requirement: Stryker 工作目录与产物落在 absCwd

**ID**: REQ-MT-CWD-1
**Priority**: MUST
**Description**: 变异阶段执行时，Stryker 的工作根（`rootPath` / `cwd`）SHALL 为当前 plan entry 的 absCwd（即 `projectRoot / plan.directory`）。临时配置（`stryker.config.*`）、`.stryker-tmp/`、`reports/mutation/` 等产物 SHALL 默认落在该 absCwd 下（第一版不引入独立 `artifacts` 字段）。

传入 `resolveStrykerConfig` 的 `mutate` / sourceFiles 路径 SHALL 重写为相对 absCwd 的 POSIX 路径。

#### Scenario: mutation 在 suite absCwd 下执行

**WHEN** plan entry `directory` 为 `"plugins/dev-team/bin"`
**AND** mutation 阶段启动
**THEN** Stryker 命令的 cwd / rootPath SHALL 解析为 `projectRoot/plugins/dev-team/bin`
**AND** 临时 `stryker.config.*` SHALL 创建于该目录下（当需要生成临时配置时）

#### Scenario: mutate 路径相对 absCwd

**WHEN** sourceFiles 含项目相对路径 `"plugins/dev-team/bin/src/foo.ts"`
**AND** absCwd 为 `plugins/dev-team/bin`
**THEN** 写入 Stryker 配置的 mutate 条目 SHALL 为 `"src/foo.ts"`（相对 absCwd）

### Requirement: 覆盖率阈值读取改为 suite 维度

**ID**: REQ-MT-COV-1
**Priority**: MUST
**Description**: `lib/test-report.ts` 中读取覆盖率阈值的逻辑（原 `readCoverageThresholds` / `computeOverrides`）SHALL 改为使用各 suite / plan entry 的 `coverage` 阈值，MUST NOT 再合成全局 `test.coverage` + `test.overrides[].coverage`。

#### Scenario: 子报告 threshold 来自 suite coverage

**WHEN** suite 配置 `coverage: { lines: 90, branches: 70, functions: 75 }`
**AND** 生成该框架子报告
**THEN** 报告中的 coverage thresholds SHALL 使用 lines=90（及该 suite 的 branches/functions）

#### Scenario: 未显式 coverage 时使用 schema 默认

**WHEN** suite 省略 `coverage` 块
**THEN** thresholds SHALL 为 schema 常量默认值 80/70/75

---

## Module Contract

### Module: lib/test-report.ts

| Aspect | Before | After |
|--------|--------|-------|
| Threshold source | `config.test.coverage` + overrides | `tests[i].coverage` / plan entry |
| Mutation threshold | `config.test.mutation` + overrides | `tests[i].mutation` / `plan.mutation_score` |
| Grouping key | `overrides[].file` | suite scope / plan entry |

### Module: lib/test-runner.ts / stryker-config.ts

| Aspect | Detail |
|--------|--------|
| rootPath / cwd | absCwd (`projectRoot / plan.directory`) |
| Artifacts | under absCwd（v1） |
| mutate paths | relative to absCwd |
