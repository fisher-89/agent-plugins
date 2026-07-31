# 提案: test-execution-result-file

> **变更**: test-execution-result-file
> **日期**: 2026-07-31
> **状态**: 草稿

---

## 问题

`dev-team test-execution` 通过子进程 **stdout** 采集测试结果。子进程打印噪声或运行报错会使 stdout 非标准格式，导致解析失败。主痛点在 **jest**：`js-parser` 对整段 stdout 做 `JSON.parse`，前后缀噪声即触发 `Failed to parse JSON output`。

对照：覆盖率与 mutation 已走文件通道；仅测试用例结果仍绑定 stdout。同时报告布局仍为扁平文件（`reports/test-execution.json` + `reports/test-execution/<planId>.json`），agent 常误用 `<framework>.json` 拼路径，下游缺少可靠的 plan 目录索引。

---

## 提案

将测试结果采集改为 **文件通道优先**，并统一 plan 产物目录布局；CLI / phase 名仍为 `test-execution`，仅报告路径缩短为 `reports/test/`。

### 采集通道（优先级）

1. **框架原生「输出到文件」**（如 jest `--json --outputFile=`、vitest reporter outputFile、llvm-cov `--output-path`、pytest `--cov-report=json:path`）——优先，避免脏 stdout。
2. **无原生能力时**再壳层 **段级 `>`** 重定向到垂直定义的结果文件。
3. **不做** tee；**不做**脏文本括号扫描抽 JSON。

### 统一 plan 产物目录

| 路径 | 含义 |
|------|------|
| `reports/test/summary.json` | 聚合报告（有 `--change` 时在 `openspec/changes/<change>/reports/test/summary.json`） |
| `reports/test/<planId>/report.json` | 原子报告（原 sub_report） |
| `reports/test/<planId>/<vertical artifacts>` | 测试结果 / 覆盖率 / mutation（文件名由各框架垂直定义） |

`planId` 复用现有消毒算法，但返回**目录 id**（去掉 `.json` 后缀）：

- `directory === '.'` → `planId = "<framework>"`（无前缀、无前导 `_`）
- 例：`plugins/dev-team/bin` + `vite-plus` → `plugins_dev-team_bin_vite-plus`

### summary.plans[] 索引

`summary.json` 保留既有聚合字段（`conclusion` / `problems` / coverage 等），并新增 `plans[]`：

```json
{
  "id": "plugins_dev-team_bin_vite-plus",
  "framework": "vite-plus",
  "directory": "plugins/dev-team/bin",
  "path": "reports/test/plugins_dev-team_bin_vite-plus"
}
```

- `path` 相对 **project root**。
- 每个尝试执行的 plan 都进索引（含半失败）；`plans[]` **不**携带 status——成败落在 `report.json` 与 summary `problems`。

### execute 管线：`preparePlanArtifacts`

在 `executePlanEntry` 中、跑命令前：

1. 决议 `reportDir`；`mkdir`；清空**该** plan 目录（不影响其他 plan）。
2. 调用 `preparePlanArtifacts`：填占位符（`{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等）；按需生成临时 config。
3. 展开占位符；若 `redirectStdoutToResults` 则条件追加 `>`。
4. 跑命令 → 垂直 parser 从 `reportDir` 读产物 → 写 `report.json` → 投影进 `summary.plans[]`。
5. best-effort 清理临时文件。

**detect** 仍生成带占位符的 script；**不**在 detect 烤死 reportDir 相关路径。

**临时 config 原则（C3/C4）**：能 CLI 解决的不做临时测试 config；默认仅 bun（coverageDir / lcov）需要临时 bunfig；临时文件落 suite cwd、用后删除；不改用户长期 config。

### 覆盖率与 mutation

- 有 coverage 侧车文件时由垂直 parser 从 **plan 目录**读；无则 `coverage=null`。
- **不**从 jest 结果内 `coverageMap` fallback；**不**读 suite cwd 旧路径；**不**双读 / 迁移历史产物。
- Stryker `jsonReporter.fileName` → `reportDir/mutation.json`（现行唯一 mutation 工具约定名，非跨工具强制名）。
- bun：纠正为临时 bunfig + `lcov`（`coverage_format` → `lcov`）；不做 post-copy 正式策略。

### Jest 目标形态（主痛点）

```
npx jest --json --outputFile=<planDir>/results.json \
  --coverage --coverageDirectory=<planDir> --coverageReporters=json-summary \
  --silent ...
→ 垂直 parser 直接读 results.json + coverage-summary.json
```

本轮范围：**全部八框架**（实现可切片先落地路径层 + jest/vitest）。

---

## 能力

### 新增能力

- （无）

### 修改的能力

- **cli-unit-test-execute** — 报告根改为 `reports/test/`；原子报告为 `<planId>/report.json`；summary 为 `summary.json` 并含 `plans[]`；execute 引入 `preparePlanArtifacts`；文件通道采集 + 垂直 parser；扩展 `ExecutionResult`
- **test-detect-frameworks** — detect 产出带 `{results_file}` / `{coverage_file}` / `{report_dir}` / `{config_args}` 等占位符的 script（不烤死 report 路径）；`coverage_output` 改为相对 reportDir 的垂直约定名；删除 suite cwd `coverage_cleanup` 语义
- **test-get-framework-config** — registry：各框架模板/coverage 路径对齐文件通道；bun 改为 lcov + 支持显式 config；覆盖率格式枚举增加 `lcov`
- **mutation-testing** — mutation 报告写入 `reportDir/mutation.json`；解析只认 plan 目录；临时配置仍 cwd + 用后删
- **unit-test-executor** — executor agent 读 `reports/test/summary.json`；经 `plans[]` 定位各 `<planId>/report.json`；禁止再写旧路径 / `<framework>.json`
- **test-execution-diagnostics** — 诊断输入路径改为 `reports/test/summary.json`
- **phase-agents** — agent Module Contract / 文案中的报告路径迁移到 `reports/test/`

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/test-runner.ts` — `preparePlanArtifacts`、条件 `>`、垂直 parse 调度、`ExecutionResult` 扩展
- `plugins/dev-team/bin/src/lib/test-report.ts` — `derivePlanId` 改为目录 id；布局 `reports/test/`；写 `summary.json` + `plans[]`；原子报告 `report.json`
- `plugins/dev-team/bin/src/lib/test-framework.ts` — 各框架模板占位符、coverage 约定、bun lcov/`config_flag`
- `plugins/dev-team/bin/src/lib/test-parser/**` — 垂直 parser（按框架读 plan 目录文件）；横向 coverage-parser 保留为库
- `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` — `jsonReporter.fileName` → planDir
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — detect 推迟展开 report 相关占位符；移除 suite cwd cleanup 注入
- `plugins/dev-team/bin/src/commands/test-execution.ts` — 报告根路径与 summary 落盘
- `plugins/dev-team/bin/src/schemas/test-execution-output.schema.ts` — `plans[]` 与路径相关 schema
- `plugins/dev-team/agents/test-execution-executor.md` — 读 `summary.json` + `plans[]`
- `plugins/dev-team/agents/test-execution-evaluator.md` — 聚合报告路径
- `plugins/dev-team/skills/phase-test-execution/SKILL.md` — 路径文案（若引用旧路径）

### 测试文件

- `plugins/dev-team/bin/src/lib/test-report.test.ts` — planId、目录布局、`plans[]`
- `plugins/dev-team/bin/src/lib/test-runner.test.ts` — prepare / 文件通道 / 半失败
- `plugins/dev-team/bin/src/lib/test-framework.test.ts` — registry 与 bun lcov
- `plugins/dev-team/bin/src/lib/test-parser/**/*.test.ts` — 八框架各至少 1 条「产物在 reportDir」
- `plugins/dev-team/bin/src/commands/test-execution.test.ts` — 新报告路径
- `plugins/dev-team/bin/__tests__/cli-test-execution-execute/**` — 端到端路径与采集
- Windows cmd / Git Bash：链式框架段级重定向敏感用例

### 不要修改

- CLI / phase 名称（仍为 `test-execution`）
- OpenSpec 技能路由与 phase 编排逻辑（非路径文案部分）
- 用户仓库内长期 config / bunfig（禁止就地改写）
- 旧路径兼容层 / 双读 / 历史产物迁移
- tee、脏 JSON 括号扫描、coverageMap fallback
- bun post-copy 作为正式策略
- 非 test-execution 相关的 MCP / hook / architecture 能力

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 报告布局 | 有/无 `--change` 时分别写入 `…/reports/test/summary.json` 与 `…/reports/test/<planId>/report.json`；不再写 `reports/test-execution.json` 或扁平 `<planId>.json` |
| AC-2 | planId | `directory === '.'` → 目录名为 `<framework>`；非根 suite → `sanitize(directory)_<framework>`；均无 `.json` 后缀 |
| AC-3 | plans[] | summary 含 `plans[]`，字段 `id/framework/directory/path`；`path` 相对 project root；半失败 plan 仍出现且带 path |
| AC-4 | jest 文件通道 | jest 使用 `--outputFile` 写入 planDir；parser 直接读文件成功；**不**对整段 stdout 做 `JSON.parse`；**不**追加壳层 `>` |
| AC-5 | 原生优先 / 条件 `>` | 有原生文件输出的框架不追加 `>`；无原生能力的测试段才段级重定向；不做 tee |
| AC-6 | 覆盖率权威源 | 只从 plan 目录读 coverage；无侧车则为 null；不读 suite cwd 旧路径；无 coverageMap fallback |
| AC-7 | preparePlanArtifacts | execute 前决议路径/占位符；detect script 仍含未展开的 report 占位符；临时文件 cwd + 用后删 |
| AC-8 | bun | 临时 bunfig 直写 `lcov.info` 到 planDir；`coverage_format` 为 lcov；不改用户 bunfig；不做 post-copy 默认策略 |
| AC-9 | mutation | Stryker 报告落在 `reportDir/mutation.json`；解析不读旧 `reports/mutation/` |
| AC-10 | agent / specs 路径 | executor/evaluator 文案使用 `reports/test/summary.json` 与 `plans[]`→`report.json`；禁止 `<framework>.json` |
| AC-11 | 八框架单测 | 每个框架至少 1 条「约定产物在 reportDir」单测（可 mock fs） |
| AC-12 | `>` 族噪声 AC | 对段级重定向框架保留噪声/前缀场景验收；原生 outputFile 族以读 plan 文件成功为主 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| jest/vitest CLI 无法覆盖用户 config 内输出路径 | 产物仍落旧路径，解析失败 | 中 | C3 降级：该框架临时 config overlay；§9 清单在 design 前实证 |
| bun `--config=` 加载/参数位置失败 | bun 覆盖率不可用 | 中 | 降级：测试结果 `>` + `coverage=null`；不阻断其它框架 |
| 全框架同轮改动面大 | 回归成本高 | 高 | 实现切片：先路径层 + jest/vitest，再 C 类与 bun；各切片可独立验收 |
| Windows cmd 段级重定向与 errorlevel | 链式框架结果/覆盖率丢失 | 中 | 与现有 rust 模式对齐；加 cmd/Git Bash 敏感单测 |
| agent 仍写旧路径 | 诊断读错文件 | 中 | AC-10 强制文案迁移；`plans[]` 降低靠公式拼路径的依赖 |
| 硬 breaking 旧报告路径 | 外部脚本失效 | 高（已知） | 明确不做兼容；文档与 agent 同步切换 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 采集通道 | 原生 outputFile 优先，否则段级 `>` | 避免脏 stdout；与 coverage/mutation 文件通道一致 | 统一只用 `>`；脏 JSON 扫描（已否决） |
| 是否 tee | 不做 | 复杂度高；诊断改读 plan 产物即可 | 默认 tee |
| 报告布局 | `reports/test/<planId>/` + `summary.json` | 路径更短；原子报告与原始产物同目录 | 保留 `test-execution` 目录名；真路径嵌套 |
| planId | 消毒算法去 `.json`；`.` → `<framework>` | 与现实现一致（修正 JSDoc `_vitest` 误导） | `_vitest` 前缀变体 |
| plans[] | 要；仅路径索引 | executor/下游定位可靠；与消毒规则解耦 | 仅公式/扫目录 |
| 临时 config | CLI 优先；默认仅 bun | 少碰用户 config；bun coverageDir 无稳定 CLI | jest/vitest 默认也写临时 config |
| 旧路径兼容 | 不做 | 权威源唯一，避免双读歧义 | 双读 fallback |
| 范围 | 本轮全框架 | 路径搬家与垂直 parser 同轮交付 | Jest-first 分批（已推翻） |

### 待决问题

- design 前实证：jest CLI `--coverageDirectory` / `--outputFile` 是否稳定覆盖用户 config 同名键
- vitest / vite-plus：json reporter `outputFile` 与 coverage `reportsDirectory` 的确切 CLI 形态
- bun：`bun --config=` 参数位置与加载失败降级细节
- 各垂直模块最终文件名表（explore 草案可微调，原则已定）
- go：`func-summary.txt` → 现有 coverage 聚合字段的映射细节
