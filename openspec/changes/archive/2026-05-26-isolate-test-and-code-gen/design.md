# 设计: isolate-test-and-code-gen

> **变更**: isolate-test-and-code-gen
> **日期**: 2026-05-25
> **基于**: proposal.md, test-design.md

---

## 架构组件

### 组件图

PGE 工作流从 7 阶段扩展为 9 阶段，新增的 06-unit-test 和 08-integration-test 采用独立的 EXEC (Executor->Evaluator) 模式。代码生成阶段的文件读取通过 prompt 约束 + skill 层校验实现隔离。架构工具从 Python 迁移到 TypeScript CLI，集成 `@likec4/core` 作为 C4 解析引擎。

```
+------------------+     +------------------+     +------------------+
| requirements     | --> | test-design      | --> | dev-proposal     |
| DESIGN (P->E)    |     | DESIGN (P->E)    |     | DESIGN (P->E)    |
+------------------+     +------------------+     +------------------+
                                                          |
                                                          v
+------------------+     +------------------+     +------------------+
| test-gen         | --> | implement        | --> | unit-test        |
| EXEC (G->E)      |     | EXEC (G->E+AUTO) |     | EXEC (Ext->Ev)   |
| 读约束: 禁止源码  |     | 读约束: 禁止测试  |     | sonnet executor   |
+------------------+     +------------------+     +------------------+
                                                          |
                                                          v
+------------------+     +------------------+     +------------------+
| code-review      | --> | integration-test | --> | acceptance       |
| EVAL-ONLY (E)    |     | EXEC (Ext->Ev)   |     | EVAL-ONLY (E)    |
| 简化: 单Evaluator|     | sonnet executor  |     |                  |
+------------------+     +------------------+     +------------------+
```

新增的 TypeScript CLI 子命令 `dev-team archi` 替代原有的三个 Python 脚本：

```
+-----------------------------+
| dev-team CLI                |
|  - eval-check               |
|  - eval-log                 |
|  - archi                    |
|    +-- query [--element]    |
|    +-- validate [--source]  |
|    +-- write --path --source|
|    +-- check [--staged|--files] |
+-----------------------------+
         |
         v
+----------------------+     +----------------------+
| @likec4/core         |     | lib/c4-cross-ref.ts  |
| (C4 DSL parsing)     |     | (import 交叉引用)     |
+----------------------+     +----------------------+
```

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| PHASES 常量 | 定义 9 阶段有序列表，提供 `getPhaseIndex` 和 `getPriorPhases` 查询 | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| eval-json 工具 | eval.json 的读写、gate check、条目构建 | `plugins/dev-team/bin/src/lib/eval-json.ts` | 无 | TypeScript |
| eval-check CLI | 前置阶段门控校验、时间戳顺序校验、backtrack 校验、phase state 判定 | `plugins/dev-team/bin/src/commands/eval-check.ts` | workflow.ts, eval-json.ts, change.ts | TypeScript (cac) |
| archi CLI 路由器 | `dev-team archi` 子命令分发（query/validate/write/check） | `plugins/dev-team/bin/src/commands/archi.ts` | `@likec4/core`, c4-cross-ref.ts | TypeScript (cac) |
| c4-cross-ref | import 交叉引用验证：解析 TS/JS/Python import，映射到 C4 元素，生成 violations/warnings 报告 | `plugins/dev-team/bin/src/lib/c4-cross-ref.ts` | `@likec4/core` | TypeScript |
| phase-requirements skill | 9 阶段工作流的入口 skill；调用 requirements-planner + requirements-evaluator | `plugins/dev-team/skills/phase-requirements/SKILL.md` | agents/requirements-*.md | Markdown (skill 定义) |
| phase-unit-test skill | 06-unit-test 阶段的 skill 路由：Executor (sonnet) -> Evaluator；包含 no-op 判定 | `plugins/dev-team/skills/phase-unit-test/SKILL.md` | agents/unit-test-executor.md, agents/unit-test-evaluator.md | Markdown (skill 定义) |
| phase-integration-test skill | 08-integration-test 阶段的 skill 路由：Executor (sonnet) -> Evaluator；包含 no-op 判定 | `plugins/dev-team/skills/phase-integration-test/SKILL.md` | agents/integration-test-executor.md, agents/integration-test-evaluator.md | Markdown (skill 定义) |
| phase-implement skill (修改) | 移除 AUTO 步骤中的单元测试执行，仅保留 static-check | `plugins/dev-team/skills/phase-implement/SKILL.md` | agents/implementation-generator.md, agents/implementation-evaluator.md | Markdown (skill 定义) |
| phase-code-review skill (修改) | 整合为单 Evaluator，移除 integration-test 子步骤 | `plugins/dev-team/skills/phase-code-review/SKILL.md` | agents/code-review-evaluator.md | Markdown (skill 定义) |
| unit-test-executor agent | sonnet 模型，执行单元测试命令，产出结构化报告 | `plugins/dev-team/agents/unit-test-executor.md` | 无（通过 Bash 运行测试） | Markdown (agent prompt) |
| unit-test-evaluator agent | 读取测试执行报告，校验报告完整性，应用诊断决策树 | `plugins/dev-team/agents/unit-test-evaluator.md` | 无（读取 JSON 文件） | Markdown (agent prompt) |
| integration-test-executor agent | sonnet 模型，执行集成测试命令，产出结构化报告 | `plugins/dev-team/agents/integration-test-executor.md` | 无（通过 Bash 运行测试） | Markdown (agent prompt) |
| integration-test-evaluator agent | 读取集成测试执行报告，应用诊断决策树，判断回溯目标 | `plugins/dev-team/agents/integration-test-evaluator.md` | 无（读取 JSON 文件） | Markdown (agent prompt) |
| test-gen-generator agent (修改) | prompt 新增源码文件类型黑名单，skill 层校验读取路径 | `plugins/dev-team/agents/test-gen-generator.md` | 无 | Markdown (agent prompt) |
| implementation-generator agent (修改) | prompt 新增 tests/ 目录黑名单，skill 层校验读取路径 | `plugins/dev-team/agents/implementation-generator.md` | 无 | Markdown (agent prompt) |
| requirements-planner agent (修改) | 增强为识别模块边界并产出结构化接口契约 | `plugins/dev-team/agents/requirements-planner.md` | 无 | Markdown (agent prompt) |
| 模块边界契约生成逻辑 | requirements-planner 在 specs/ 下生成模块接口表格（函数、API、CLI、组件） | `plugins/dev-team/agents/requirements-planner.md`（prompt 约束）+ skill 层 | specs/ 目录下的 spec.md 文件 | Markdown + 结构化表格 |
| no-op 判定逻辑 | skill 层在调用 Executor 前检查文件系统，决定是否跳过 | 内嵌在 phase-unit-test.skill.md 和 phase-integration-test.skill.md 中 | `glob` / `find` 检查测试文件存在性 | Bash + Markdown |
| 诊断决策树逻辑 | Evaluator prompt 内的 IF/THEN 决策链，分析测试报告中的错误类型和文件行号 | 内嵌在 unit-test-evaluator.md 和 integration-test-evaluator.md 中 | 测试执行报告 JSON | Markdown (prompt 约束) |

---

## 数据流

### 流程描述

#### 1. requirements 阶段模块边界契约数据流

```
变更上下文 -> requirements-planner
                 |
                 v
        识别受影响的模块目录
                 |
                 v
        分析每个模块的 public API:
          - public 函数签名
          - API 接口
          - CLI 命令
          - 组件 props/events
                 |
                 v
        写入 specs/<capability>/spec.md
          (结构化表格形式)
                 |
                 v
        test-gen-generator 引用契约
        implementation-generator 引用契约
```

#### 2. 代码生成阶段文件隔离数据流

```
test-gen-generator prompt:
  - 允许读取: test-design.md, 模块边界契约(spec.md), 项目测试模式
  - 禁止读取: *.ts, *.py, *.js, *.rs, *.go, *.java, *.c, *.cpp, *.h, *.hpp

implementation-generator prompt:
  - 允许读取: design.md, tasks.md, proposal.md, 模块边界契约(spec.md), 项目源码模式
  - 禁止读取: tests/, __tests__/, test/ 目录下任何文件

Generator 执行后 -> skill 层读取 Read 工具调用记录
                  -> 校验无违规路径
                  -> 违规则记录到 eval.json 并要求重新生成
```

#### 3. 独立测试执行阶段 (unit-test / integration-test) 数据流

```
Skill 层 no-op 判定:
  - 检查 `**/*.test.*` 或 `**/tests/unit/**` 文件存在
  - 无 -> eval.json 追加 skipped:true 条目，跳过
  - 有 -> 进入 Executor

Executor (sonnet):
  1. 运行测试命令 (npm test / pytest / go test)
  2. 捕获 stdout/stderr, exit code
  3. 提取: total, passed, failed, skipped, coverage%, failure_details, duration
  4. 写入 reports/<phase>-execution.json

Evaluator:
  1. 读取 reports/<phase>-execution.json
  2. 对照 checklist 校验报告字段完整性
  3. 应用诊断决策树:
     - 语法错误在测试行 -> backtrack_to: "04-test-gen"
     - 逻辑错误在实现行 -> backtrack_to: "05-implement"
     - 期望与 test-design 冲突 -> backtrack_to: "02-test-design"
     - 接口签名双方一致 -> backtrack_to: "03-dev-proposal"
     - 无法判断 -> AskUserQuestion (超时 5min -> dev-proposal)
  4. 设置 verdict 和 findings
  5. 追加条目到 eval.json
```

#### 4. 架构工具链数据流

```
模型文件 (*.c4)
    |
    v
dev-team archi validate --source <dsl>:
  LikeC4.fromSource() -> getErrors() -> 诊断结果

dev-team archi query --element <fqn>:
  LikeC4.fromSource() -> LikeC4Model.Computed -> elements/relations JSON

dev-team archi write --path <f> --source <dsl>:
  校验 -> 写入文件

git diff (staged files)
    |
    v
dev-team archi check --staged:
  c4-cross-ref.ts 解析 import 语句
  -> 映射到 C4 元素
  -> 检测 unmodeled_dependency / unmapped_import_target
  -> 输出 violations/warnings 报告 JSON
```

### 数据模型

#### 测试执行报告 JSON Schema

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| TestExecutionReport | `phase: string` — 阶段标识（如 "06-unit-test"） | 每个报告对应一个 Executor 的一次执行 | 文件 `reports/<phase>-execution.json` |
| | `command: string` — 执行的测试命令 | |
| | `timestamp: string` — ISO 8601 执行时间 | |
| | `total: number` — 总用例数 | |
| | `passed: number` — 通过数 | |
| | `failed: number` — 失败数 | |
| | `skipped: number` — 跳过数 | |
| | `coverage: number` — 覆盖率百分比 (0-100) | |
| | `duration_ms: number` — 执行时长（毫秒） | |
| | `failures: FailureDetail[]` — 失败详情列表 | |
| `FailureDetail` | `name: string` — 测试用例名 | 属于 TestExecutionReport | 内嵌于报告 |
| | `file: string` — 失败文件路径 | |
| | `line: number` — 行号 | |
| | `error_type: string` — 错误类型 (SyntaxError/AssertionError/TypeError 等) | |
| | `error_message: string` — 错误消息 | |
| | `stack_trace: string` — 堆栈追踪 | |
| `FailureDetail` (设计冲突) | `expected: string` — 测试期望值 | 属于 TestExecutionReport | 内嵌于报告 |
| | `actual: string` — 实际值 | |
| | `design_ref: string` — 对应 test-design.md 的引用 ID | |

#### eval.json 新增字段

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| EvalEntry (新增 skipped 支持) | `skipped: boolean` — 是否被 no-op 跳过 | 属于 phase | `openspec/changes/<name>/phases/eval.json` |
| EvalEntry (新增 findings 支持) | `findings: string` — 诊断决策树的根因分析过程和结论 | 属于 phase 的 Evaluator 产出 | `openspec/changes/<name>/phases/eval.json` |
| EvalEntry (新增 phase_suffix 支持) | `phase_suffix: string` — 同一 phase 内的子步骤标识（如 "static-check"） | 属于 phase | `openspec/changes/<name>/phases/eval.json` |

#### 模块边界契约数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| FunctionContract | `name: string`, `params: ParamDef[]`, `returns: string`, `description: string` | 属于 ModuleContract | spec.md 中结构化表格 |
| APIContract | `method: string`, `path: string`, `request_schema: object`, `response_schema: object` | 属于 ModuleContract | spec.md 中结构化表格 |
| CLIContract | `command: string`, `args: ArgDef[]`, `flags: FlagDef[]`, `examples: string[]` | 属于 ModuleContract | spec.md 中结构化表格 |
| ComponentContract | `name: string`, `props: PropDef[]`, `events: EventDef[]` | 属于 ModuleContract | spec.md 中结构化表格 |
| ModuleContract | `module_id: string`, `functions: FunctionContract[]`, `apis: APIContract[]`, `clis: CLIContract[]`, `components: ComponentContract[]` | 每个变更可包含多个模块 | `specs/<capability>/spec.md` |

#### archi CLI 输出格式

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| ArchiQueryResult | `elements: C4Element[]`, `relations: C4Relation[]` | query 输出 | stdout JSON |
| ArchiValidateResult | `valid: boolean`, `errors: string[]`, `warnings: string[]` | validate 输出 | stdout JSON |
| ArchiCheckResult | `violations: CrossRefViolation[]`, `warnings: string[]` | check 输出 | stdout JSON + 可选保存到文件 |
| `CrossRefViolation` | `type: string` (unmodeled_dependency / unmapped_import_target / unused_relationship / path_not_found), `source: string`, `target: string`, `file: string`, `description: string` | 属于 ArchiCheckResult | 内嵌于报告 |

---

## CLI 命令设计

| 命令 | 描述 | 输入 | 输出 | 变更说明 |
|------|------|------|------|----------|
| `dev-team archi query [--element <fqn>]` | 查询 C4 模型元素和关系 | `--element` 可选，不传时返回全部 | stdout JSON: `{elements: [...], relations: [...]}` | 新增，替代 `python archi-model.py --command query` |
| `dev-team archi validate [--source <dsl>]` | 校验 C4 DSL 语法 | `--source` DSL 内容，省略时从 stdin 读取 | stdout JSON: `{valid: bool, errors: [...], warnings: [...]}` | 新增，替代 `python archi-model.py --command validate` |
| `dev-team archi write --path <f> --source <dsl>` | 校验并写入 C4 模型文件 | `--path` 文件路径，`--source` DSL 内容 | 无输出（成功时），错误时输出错误信息 | 新增，替代 `python archi-model.py --command write` |
| `dev-team archi check [--staged \| --files <list>]` | import 交叉引用验证 | `--staged` 检查 git staged 文件，或 `--files` 指定文件列表 | stdout JSON: `{violations: [...], warnings: [...]}` | 新增，替代 `python archi-validate.py --staged` |
| `dev-team eval-check --change <name> --phase <phase>` | 前置阶段门控校验 | `--change`, `--phase`, 可选 `--json` | stdout JSON 或 human-readable 文本 | 修改：支持 9 阶段，识别 skipped phase |
| `dev-team eval-log --change <name> --phase <phase> ...` | 追加 eval 条目 | `--change`, `--phase`, `--verdict`, `--report`, `--items`, 可选 `--backtrack-to`, 可选 `--skipped` | 无输出（写入文件） | 修改：支持 `--skipped` 标记 |

### PHASES 常量变更

```
// 旧: 7 阶段
["01-requirements", "02-test-design", "03-dev-proposal", "04-test-gen",
 "05-implementation", "06-code-review", "07-acceptance"]

// 新: 9 阶段
["01-requirements", "02-test-design", "03-dev-proposal", "04-test-gen",
 "05-implement", "06-unit-test", "07-code-review", "08-integration-test",
 "09-acceptance"]
```

注意：05 阶段标识从 `"05-implementation"` 改为 `"05-implement"`，以匹配其他 skill 文件中的命名约定。

### EvalEntry 字段扩展

```
// 标准字段
{
  "phase": "06-unit-test",
  "timestamp": "2026-05-25T10:30:00.000Z",
  "attempt": 1,
  "verdict": "pass" | "fail",
  "report": "Summary text (max 500 chars)",
  "items": [{"item_id": "...", "pass": true, "evidence": "...", "notes": "..."}],
  "backtrack_to": "04-test-gen" | null,
  "phase_suffix": "static-check" | null,     // 新增: implement 阶段 AUTO 子步骤标识
  "skipped": true | undefined,                // 新增: no-op 跳过标记
  "findings": "..." | undefined,              // 新增: 诊断决策树分析过程和结论
  "schema_version": "1.0"
}
```

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D01 | 采用 Prompt 约束 + skill 层校验实现文件隔离，而非物理隔离 | 改动范围可控，不引入 git worktree 或容器基础设施；可渐进增强，如有越狱案例可在后续叠加 worktree 级隔离 | **worktree 物理隔离** — 隔离级别最高但工作流复杂度剧增，需要切换 worktree、合并变更、处理冲突，评估为过度设计；**容器级隔离** — 需要容器运行时，对 CLI 插件来说过于重量级 |
| D02 | 新增独立 06-unit-test 和 08-integration-test 阶段，而非继续作为子步骤 | 使测试执行获得独立的 Generator 和 Evaluator、独立的跳过/重试/回溯能力、独立的经济模型分层（sonnet 执行 + 高级模型校验） | **保持子步骤模式** — 改动最小但无法独立回溯诊断，测试失败根因分析不充分；**单一 test-execution 阶段** — 无法区分 unit 和 integration 的回溯策略 |
| D03 | EXEC 阶段采用 Executor (sonnet) -> Evaluator 模式，而非混合模式 | sonnet 模型可满足测试执行和报告生成的任务需求且成本更低；分离 Executor 和 Evaluator 职责使校验更独立 | **Generator->Evaluator 模式** — Generator 不适合标准化报告生成，且 sonnet 模型单独做 evaluator 质量不够；**单 agent 模式** — 职责混叠，无法独立诊断 |
| D04 | `@likec4/core` 替换自实现 `archi_parser.py` | 100% C4 语法兼容，消除 views/deployment/tags/import 等语法的兼容性风险；LikeC4 社区持续维护语法演进 | **继续维护自实现 parser** — 维护成本高，覆盖 LikeC4 语法仅 65%，语法演进时需手动跟进；**切换其他 C4 parser** — `@likec4/core` 是目前最成熟的 TypeScript C4 解析库，与项目技术栈一致 |
| D05 | 架构工具从 Python 迁移到 TypeScript CLI | 消除 Python/TypeScript 语言不一致的维护负担，所有 CLI 命令集中在同一入口；`@likec4/core` 是 TypeScript 原生库，Python 调用需额外桥接 | **保持 Python 工具链** — 维护两套语言，`@likec4/core` Python 绑定不成熟；**混合模式** — Python CLI + TypeScript 库，增加部署复杂度 |
| D06 | requirements-planner 在 specs/ 中产出模块边界契约 | test-gen 和 implement 双方引用同一份契约，消除接口理解偏差；契约以结构化表格输出，格式统一 | **独立契约文件** — specs/ 目录已经是 capability 定义目录，在其中嵌入契约表格最自然；**设计文档中定义** — test-design.md 和 design.md 分别定义接口，双方自行同步，已有理解偏差问题 |
| D07 | 诊断决策树"无法判断"时调用 AskUserQuestion 而非自动回溯 | 用户交互式选择回溯目标可提高回溯准确性；设置默认超时 5 分钟后自动回退 dev-proposal 避免流水线阻塞 | **自动回溯到 dev-proposal** — 最安全的选择但可能浪费迭代（如实际根因是 test-gen）；**仅输出诊断报告不回溯** — 需要用户手动处理，增加认知负担 |
| D08 | no-op 判定在 skill 层（Bash/CLI）完成而非 Evaluator | 减少 agent 调用开销，降低经济成本；判定逻辑简单（文件存在检查），无需 LLM 处理 | **Evaluator 判定** — 灵活性强但增加了不必要的 LLM 调用开销；**Generator 判定** — Generator 不适合做跳过决策 |
| D09 | code-review 简化为单 Evaluator | 移出 integration-test 子步骤后，单 Evaluator 职责清晰；eval.json 中仅一条 code-review 条目 | **保持双 Evaluator** — 但 integration-test 已独立为 08-integration-test 阶段，code-review 中评审测试执行结果是职责混叠 |
| D10 | implement 阶段 AUTO step 仅保留 static-check | 单元测试执行已独立为 06-unit-test 阶段；static-check 失败后不触发 unit-test 阶段，直接回溯 implement Generator | **保持 AUTO static-check + unit-test** — 但单元测试需要独立生命周期来支持回溯诊断和跳过 |
| D11 | 移除 E2E 测试支持，测试仅分 unit + integration（均使用 mock） | E2E 测试依赖外部环境，在 PGE 工作流中不稳定且难以诊断；unit + integration 覆盖足够验证代码正确性 | **保留 E2E 测试** — 但 E2E 环境搭建和维护成本高，不适合流水线式阶段推进；**E2E 作为独立阶段** — 仍面临环境不稳定问题 |
| D12 | `archi-decide.py`（ADR 管理）不在本次变更范围 | archi-decide.py 使用独立的文件存储和模板，不依赖 `@likec4/core` 或 import 交叉引用，无迁移必要 | **同时迁移 archi-decide.py** — 增加变更范围，但 ADR 管理功能在当前工作流中使用频率低，不构成迁移优先级 |

---

## 依赖

### 运行时依赖

- `@likec4/core: ^1.56.0` — C4 DSL 解析引擎，提供 `LikeC4.fromSource()`, `getErrors()`, `LikeC4Model.Computed` API

### 构建/测试依赖

- `vitest` — TypeScript CLI 的单元测试框架（已在 dev-team/bin/ 中使用）
- `pytest` — Python 工具函数的单元测试框架
- `tsx` 或 `ts-node` — 测试环境中运行 TypeScript CLI 命令的方式

### 被移除的依赖

- `archi_parser.py` — 自实现 C4 DSL 解析器（约 300 行），替换为 `@likec4/core`
- `archi-model.py` — C4 模型管理 CLI（约 180 行），替换为 `dev-team archi` 子命令
- `archi-validate.py` — import 交叉引用验证（约 590 行），替换为 `lib/c4-cross-ref.ts`

---

## 风险与缓解

详见 proposal.md 风险章节。本文档补充以下技术风险：

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `@likec4/core` 包体积 ~5-15MB，增加 npm install 时间 | 首次安装时间增加 | 中 | 只引入核心库（不含 React/Vite/Graphviz 渲染层），随 plugin bin/ 的 npm install 安装，不阻塞非 archi 命令 |
| `lib/c4-cross-ref.ts` 的 import 解析器与 Python 版行为不一致 | 输出差异导致 violations 漏报或误报 | 中 | AC-17 要求输出等价验证；import 解析正则从 Python 逐行对照翻译；用现有项目的 staged diff 做回归测试 |
| unit-test 和 integration-test 的阶段区分标准模糊 | 测试分类不当，影响 no-op 判定和回溯链 | 低 | proposal.md 建议：不依赖外部服务/数据库的测试标记为 unit，依赖 mock 外部服务的标记为 integration |

---

## 迁移步骤

1. **更新 PHASES 常量**: 在 `workflow.ts` 中将 7 阶段列表替换为 9 阶段列表，更新 `getPhaseIndex` 和 `getPriorPhases`
2. **新增 agent prompts**: 创建 `unit-test-executor.md`、`unit-test-evaluator.md`、`integration-test-executor.md`、`integration-test-evaluator.md` 四个 agent prompt 文件
3. **新增 skill 文件**: 创建 `phase-unit-test/SKILL.md` 和 `phase-integration-test/SKILL.md`
4. **修改 implement skill**: 移除 AUTO 步骤中的单元测试执行，仅保留 static-check
5. **修改 code-review skill**: 移除 integration-test 子步骤，恢复为单 Evaluator
6. **修改 test-gen-generator prompt**: 添加源码文件类型黑名单约束
7. **修改 implementation-generator prompt**: 添加 tests/ 目录黑名单约束
8. **修改 requirements-planner prompt**: 添加模块边界契约生成指令
9. **安装 `@likec4/core`**: `npm install @likec4/core@^1.56.0` 到 `plugins/dev-team/bin/`
10. **实现 `lib/c4-cross-ref.ts`**: 重写 import 交叉引用逻辑
11. **实现 `commands/archi.ts`**: 新增 `dev-team archi` 子命令（query, validate, write, check）
12. **注册 archi 命令**: 在 `index.ts` 中注册新的 archi 命令
13. **删除 Python archi 文件**: 删除 `archi_parser.py`、`archi-model.py`、`archi-validate.py`
14. **更新 architecture.md agent**: 将所有 Python archi 命令引用替换为 `dev-team archi`
15. **更新 eval-check CLI**: 支持 9 阶段和 no-op 跳过状态检测
16. **更新 eval-log CLI**: 支持 `--skipped` 和 `--findings` 扩展字段
17. **编写测试文件**: 按 test-design.md 覆盖映射创建所有测试文件
18. **清理 E2E 残留**: 确认 templates/ 和 agents/ 中无 e2e 相关内容

---

## 待决问题

- AskUserQuestion 工具在 Claude Code 中的可用性和超时行为需要确认 — `askUserTimeout` 配置项的命名和默认值需在实现时确认
- 集成测试的运行环境：是否需要为 integration-test Executor 提供特殊的 mock 环境准备指令？
- implement 阶段 `"05-implement"` vs 原有 `"05-implementation"` 的标识符重命名是否需要向后兼容处理？
