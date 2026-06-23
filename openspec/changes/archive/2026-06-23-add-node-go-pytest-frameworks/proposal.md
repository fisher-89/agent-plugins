# 提案: add-node-go-pytest-frameworks

> **变更**: add-node-go-pytest-frameworks
> **日期**: 2026-06-23
> **状态**: draft

---

## 问题

dev-team 插件的测试基础设施当前仅支持五种框架（jest、vitest、vite-plus、bun、rust）。使用 Node.js 内置 `node:test`、Go 内置 `go test` 或 Python pytest 的项目无法被框架检测、命令注册表和覆盖率解析链路正确识别，导致：

1. **config.json 无法声明新框架**：`testFrameworkSchema` 枚举不包含 `node-test`、`go`、`pytest`，配置验证失败
2. **MCP 工具返回未知框架错误**：`test_get_framework_config` 和 `test_detect_frameworks` 的注册表与 schema 均缺少新框架条目
3. **覆盖率维度语义不匹配**：Go 无原生分支覆盖率、coverage.py 无原生函数覆盖率，现有 schema 要求三维度均为 `number`，无法表达"框架不支持"
4. **Executor/Evaluator 缺少解析规则**：`unit-test-executor.md` 仅定义 `istanbul` 和 `llvm-cov` 两种 `coverage_format` 的解析路径；Evaluator 未定义 null 维度的门控行为
5. **Python 工具链未对齐**：`test-framework.py` 未检测 Go 项目；`test-runner.py` 缺少 `go test` 和 `node --test` 运行器

---

## 提案

在现有测试框架注册表、MCP schema、Agent 解析规则和 Python 工具上扩展三种新框架，并引入 nullable 覆盖率维度语义：

1. **框架注册表扩展**：在 `FRAMEWORK_REGISTRY` 中新增 `node-test`、`go`、`pytest` 三条配置，含 `test_cmd`、`coverage_cmd`、`coverage_format`、`coverage_output`、`default_glob` 等字段
2. **node:test 覆盖率两步脚本（D1）**：Step 1 运行 `node --test --experimental-test-coverage` 并将 stdout tee 到文件；Step 2 调用 `parse-node-test-coverage.mjs` 解析文本表格并写入 `coverage/coverage-summary.json`
3. **Nullable 覆盖率维度（D2）**：`coverage.measured` 和 `by_framework[].measured` 的 `branches`、`functions` 类型扩展为 `number | null`；null 维度跳过阈值比较（自动通过），不参与加权平均；Evaluator findings 标注 "N/A (框架不支持)"
4. **coverage_format 枚举扩展**：`'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py'`
5. **Agent 与 Python 工具同步**：更新 executor/evaluator agent 的 Step 2（测试输出解析）、Step 4（覆盖率解析）和 Step 5（加权平均/null 门控）；扩展 `test-framework.py` Go 检测和 `test-runner.py` 运行器

---

## 能力

### 新增能力

（无 — 本变更扩展既有测试基础设施能力，不引入新的 OpenSpec capability）

### 修改的能力

- **config-schema** — `testFrameworkSchema` 枚举新增 `node-test`、`go`、`pytest`；更新 Module Contract 中的有效框架名列表
- **test-execution-diagnostics** — 扩展 `coverage_format` 枚举；FRAMEWORK_REGISTRY 新增三条框架配置；新增 `parse-node-test-coverage.mjs` 解析器；`coverage.measured` 类型允许 null 维度；加权平均与 `coverage.pass` 增加 null 维度规则；新增三种格式的覆盖率解析策略
- **phase-agents** — 更新 `unit-test-executor.md` 和 `unit-test-evaluator.md`：新增框架测试输出解析、三种 `coverage_format` 解析路径、null 维度门控与 findings 标注规则

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/schemas/config/config.schema.ts` — `testFrameworkSchema` 枚举 +3
- `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` — `coverage_format` 类型扩展 + REGISTRY +3
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` — `coverage_format` 枚举扩展
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` — `PlanEntry` 类型同步
- `plugins/dev-team/scripts/parse-node-test-coverage.mjs` — **新增**：node:test stdout 覆盖率表格解析器
- `plugins/dev-team/agents/unit-test-executor.md` — Step 2 测试输出解析 + Step 4 覆盖率解析 + Step 5 null 维度加权平均
- `plugins/dev-team/agents/unit-test-evaluator.md` — null 维度门控与 findings 标注
- `plugins/dev-team/utils/test-framework.py` — Go 项目检测（`go.mod`）
- `plugins/dev-team/utils/test-runner.py` — `go test` 和 `node --test` 运行器

### 测试文件

- `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` — 新框架名 schema 验证
- `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` — REGISTRY 三条新框架配置
- `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` — 输出 schema 新 `coverage_format` 值
- `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.test.ts` — plan 输出 schema 新枚举值
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` — plan 生成含新框架
- `plugins/dev-team/scripts/parse-node-test-coverage.mjs` 对应单元测试（若项目惯例要求）

### 不要修改

- 既有五框架（jest/vitest/vite-plus/bun/rust）的命令配置与解析逻辑
- 集成测试覆盖率（08-integration-test）—— 后续复用，本次不涉及
- `test-gen-generator.md` 框架语法表 —— 不在本次范围（可后续独立变更）
- HTML 覆盖率报告相关逻辑 —— 已废弃，保持 JSON-only
- 非测试相关的 phase agent 文件

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `testFrameworkSchema` 接受 `node-test`、`go`、`pytest` 作为有效框架名 | `config.schema.test.ts` 验证通过；无效名如 `"mocha"` 仍被拒绝 |
| AC-2 | `test_get_framework_config({"framework": "go"})` 返回完整配置，含 `coverage_format: "go-cover"` | 单元测试断言 REGISTRY 字段 |
| AC-3 | `test_get_framework_config({"framework": "node-test"})` 的 `coverage_cmd` 为两步脚本（native coverage + parser） | 单元测试断言命令包含 `--experimental-test-coverage` 和 `parse-node-test-coverage.mjs` |
| AC-4 | `test_get_framework_config({"framework": "pytest"})` 返回 `coverage_format: "coverage-py"`、`coverage_output: "coverage.json"` | 单元测试断言 |
| AC-5 | `test_detect_frameworks` 输出 schema 的 `coverage_format` 枚举含五个值 | schema 测试通过 |
| AC-6 | `parse-node-test-coverage.mjs` 从 node:test stdout 文本表格提取 lines/branches/functions 并写入 istanbul 兼容的 `coverage-summary.json` | 脚本测试或手动 fixture 验证 |
| AC-7 | Go 框架解析后 `measured.branches` 和 `measured.functions` 为 `null`，`measured.lines` 为数值 | executor agent 规则 + 集成验证 |
| AC-8 | pytest 框架解析后 `measured.functions` 为 `null`，`measured.lines` 和 `measured.branches` 为数值 | executor agent 规则 + 集成验证 |
| AC-9 | null 维度跳过阈值比较：`coverage.pass` 计算时不因 null 维度失败 | 构造 Go-only 项目，lines 达标 → `coverage.pass` 为 true |
| AC-10 | 加权平均计算时 null 维度不参与：`measured.branches` 仅由提供 branches 值的框架贡献 | 多框架混合场景单元/集成测试 |
| AC-11 | Evaluator 对 null 维度在 findings 中标注 "N/A (框架不支持)" | 检查 evaluator agent 规则 |
| AC-12 | `test-framework.py` 检测到 `go.mod` 时返回 `("go", "go test ./...")` | Python 测试或手动验证 |
| AC-13 | `test-runner.py` 支持 `go` 和 `node-test` 框架运行 | Python 测试或手动验证 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `node --experimental-test-coverage` 为实验性 API，输出格式可能变更 | node-test 覆盖率解析失败 | 中 | 解析器独立脚本 + fixture 测试；解析失败时该框架 measured 设为 0 并记录 findings |
| Go `go tool cover -func` 输出格式因 Go 版本差异 | lines 百分比提取失败 | 低 | 解析规则匹配 `total:` 行；失败时 lines 设为 0 |
| pytest 需安装 `pytest-cov` 插件 | 覆盖率命令非零退出 | 中 | Executor 捕获失败、记录 findings、不阻塞报告；文档注明依赖 |
| null 维度语义与现有 ALL 门控逻辑冲突 | 误报 pass/fail | 低 | 明确规则：null 跳过比较、不参与加权平均；Evaluator 单独标注 |
| 多框架项目加权平均分母变化 | 覆盖率数值与预期偏差 | 低 | 仅非 null 框架参与该维度计算；全 null 时该维度 overall 也为 null |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| node:test 覆盖率方案 | **D1 — 两步脚本**（native coverage + parser script） | Node 无 JSON reporter；stdout 文本表格可稳定解析 | A: 仅 lines 无 branches/functions；B: 依赖第三方 c8/istanbul |
| 缺失覆盖率维度表达 | **D2 — 使用 null** | 语义清晰：null = 框架不支持，与 0% 区分 | A: 省略字段；B: 使用 -1 哨兵值 |
| 框架命名 | **D3 — `node-test`**（连字符风格，与 `vite-plus` 一致） | 与现有命名惯例一致 | `nodetest`、`node_test` |
| Go 覆盖率格式 | `go-cover` + 文本 `func-summary.txt` | Go 原生工具链，无 JSON summary | cargo-llvm-cov（Go 不适用） |
| pytest 覆盖率格式 | `coverage-py` + `coverage.json` | pytest-cov 标准 JSON 输出 | XML/HTML reporter |

### 待决问题

- `parse-node-test-coverage.mjs` 的插件内路径引用方式：使用相对于项目根的 `plugins/dev-team/scripts/...` 还是通过 MCP 工具解析插件根目录？（实现阶段按现有脚本引用惯例确定）
- node-test 的 `coverage_artifacts` 是否仅包含 parser 产出的 `coverage/coverage-summary.json`，还是同时移动中间 stdout 文件？（建议仅移动 JSON 摘要，中间文件列入 `coverage_cleanup`）

---
