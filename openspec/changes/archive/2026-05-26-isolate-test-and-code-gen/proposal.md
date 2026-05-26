# 提案: 隔离代码生成与测试生成，增强测试执行诊断，引入独立测试执行阶段

> **变更**: isolate-test-and-code-gen
> **日期**: 2026-05-22
> **状态**: draft

---

## 问题

当前 PGE 工作流中 test-gen（测试生成）和 implement（实现生成）两个阶段存在三个结构性问题，导致测试与代码之间的信任关系被破坏。此外，测试执行作为子步骤嵌入其他阶段，缺乏独立的执行诊断能力。

### 问题 1: test-gen/implement 工作区非隔离

test-gen-generator 生成的测试文件与 implementation-generator 的工作区共享同一目录树。implementation-generator 可以看到 test-gen 阶段产出的测试文件，导致两个后果：

- **实现适应测试而非设计**: 代码生成器可能通过阅读测例来"作弊"，写出恰好通过测试但不遵循设计文档的实现代码。
- **测试失效**: 如果测试文件有预期值错误或遗漏场景，实现代码会继承这些错误，无法通过独立测试发现设计层面的偏差。

### 问题 2: 缺乏模块边界契约

test-gen 和 implement 两个阶段各自从设计文档（test-design.md / design.md）推导模块接口签名。没有结构化的、双方共同引用的契约文件：

- test-gen 推导测试 mock 的接口签名时可能错误理解参数类型或返回值。
- implement 推导实现接口时可能出现不同理解。
- 双方各自推导的结果不一致时，只有在测试执行阶段才能发现，增加了迭代成本。

### 问题 3: C4 解析器兼容性风险

当前 C4 DSL 解析由 Python `archi_parser.py` 自实现，覆盖约 65% 的 LikeC4 语法：

- views、deployment、tags、dynamic views、import 跨文件引用等语法完全不支持
- metadata 嵌套结构可能解析出错
- LikeC4 DSL 语法演进时需手动跟进，维护成本高

同时架构工具链分散在三处（`archi_parser.py` 解析、`archi-model.py` 管理、`archi-validate.py` 校验），且与 dev-team CLI 语言不一致（Python vs TypeScript），增加维护负担。

### 问题 4: 测试执行嵌入阶段内部，缺乏独立诊断能力

当前测试执行作为子步骤嵌入 implement 和 code-review 阶段内部：

- **单元测试执行**是 implement 阶段的 AUTO 子步骤，限制在 Generator->Evaluator 循环内部，无法独立回溯和重新执行。
- **集成测试执行**是 code-review 阶段的附加 Evaluator，与代码审查共享同一阶段上下文，职责混叠。
- 测试失败时无法判断失败根因是测试代码错、实现代码错、还是设计文档错。
- 回溯目标局限：子步骤阶段无法直接管理自己的回溯链。

测试执行需要独立的阶段生命周期，以便：
1. 有独立的 Generator（执行器）和 Evaluator（校验器），分别承担测试运行和结果诊断的职责。
2. 可以被独立跳过（no-op）、重试、和回溯。
3. 执行器使用更经济的模型（sonnet）运行测试并产出结构化报告，校验器对报告进行质量评估和根因分析。

---

## 提案

### 方案对比

| 方案 | 描述 | 优点 | 缺点 |
|------|------|------|------|
| **A: Prompt 约束隔离 + 结构化契约 + 独立测试阶段** | test-gen/implement 的 agent prompt 中硬编码文件类型黑名单；requirements 阶段产出结构化模块边界契约；将单元测试和集成测试提升为独立 PGE 阶段 | 改动范围可控，无需基础设施变更；契约文件可被双方引用验证；测试执行获得独立生命周期 | 9 阶段工作流增加流程节点数；executor+evaluator 双 agent 模式增加执行成本 |
| **B: Worktree 物理隔离** | 为 test-gen 和 implement 创建独立的 git worktree，每个 Agent 只有自己 worktree 的读写权限 | 隔离级别最高，物理不可见对方文件 | 工作流复杂度剧增：需要切换 worktree、合并变更、处理冲突；引入额外的 git 操作开销 |
| **C: 容器级隔离** | 为每个 Generator 启动独立容器，通过 volume mount 控制可见文件 | 隔离最彻底 | 开发体验差：需要容器运行时、网络配置；调试困难；对 CLI 插件来说过度设计 |

### 推荐方案: A — Prompt 约束隔离 + 结构化契约 + 独立测试阶段

理由：

1. **改动范围可控**: 仅修改 agent prompt、skill 路由逻辑、evaluator checklist，不引入新的基础设施依赖。
2. **渐进增强**: 先上 prompt 约束，如果后续发现 Agent 越狱行为，可叠加 worktree 级隔离作为增强。
3. **契约驱动**: requirements 阶段产出的结构化模块边界契约可被 test-gen 和 implement 同时引用，从根本上消除接口理解偏差。
4. **诊断价值高**: 测试执行 Evaluator 的诊断决策树可覆盖 80% 以上的常见失败场景，剩下的"无法判断"场景由人工介入。
5. **独立阶段生命周期**: 单元测试和集成测试作为独立的 EXEC 阶段（Generator->Evaluator），拥有独立的跳过、重试、回溯能力。
6. **经济模型分层**: 测试执行器使用 sonnet（更经济的模型）运行测试并产出报告，校验器对报告做质量评估。

### 详细设计

#### 1. 阶段结构调整：从 7 阶段到 9 阶段

原有 7 阶段中，单元测试作为 implement 的 AUTO 子步骤，集成测试作为 code-review 的附加 Evaluator。现将两者提升为独立的 PGE 阶段：

| 序号 | 阶段名称 | 模式 | 变更说明 |
|------|----------|------|----------|
| 01 | requirements | DESIGN (Planner->Evaluator) | 不变 |
| 02 | test-design | DESIGN (Planner->Evaluator) | 不变 |
| 03 | dev-proposal | DESIGN (Planner->Evaluator) | 不变 |
| 04 | test-gen | EXEC (Generator->Evaluator) | 不变 |
| 05 | implement | EXEC (Generator->Evaluator + AUTO static-check) | AUTO 步骤仅保留 static-check（lint + type-check），单元测试移出 |
| **06** | **unit-test** | **EXEC (Executor->Evaluator, sonnet)** | **新增**：执行器运行单元测试产出报告，校验器检查报告质量 |
| 07 | code-review | EVAL-ONLY (Evaluator) | 简化：单 Evaluator，集成测试移出 |
| **08** | **integration-test** | **EXEC (Executor->Evaluator, sonnet)** | **新增**：执行器运行集成测试产出报告，校验器检查报告质量 |
| 09 | acceptance | EVAL-ONLY (Evaluator) | 不变 |

#### 2. requirements 阶段增加模块边界定义

requirements-planner 在生成 proposal.md / specs/ 时，识别受影响的模块目录，并为每个模块定义公共接口契约：

- **public/export 函数**: 名称、参数签名、返回值类型（含类型注解）
- **API 接口**: method、path、request/response schema（JSON 格式）
- **CLI 命令**: 命令名、参数、flags、示例
- **组件 props & events**: prop 名称和类型、emit 事件和 payload（前端项目）

契约写入 `specs/` 目录下对应 capability 的 spec.md 文件中，以结构化表格形式呈现。

#### 3. test-gen/implement 上下文隔离

通过 agent prompt 文件类型约束 + skill 层在校验步骤验证：

- **test-gen-generator** prompt 包含硬编码文件类型黑名单：不得读取 `.ts`、`.py`、`.js`、`.rs`、`.go`、`.java`、`.c`、`.cpp`、`.h`、`.hpp` 等源码文件。
- **implementation-generator** prompt 包含硬编码目录黑名单：不得读取 `tests/`、`__tests__/`、`test/` 目录下的任何文件。
- 两边都只能看到 requirements 到 dev-proposal 的设计产物 + requirements 阶段的模块边界契约。
- skill 层在 Generator 执行后检查 Read 工具的调用路径，验证无违规读取。

#### 4. 独立测试执行阶段：Executor (sonnet) + Evaluator 模式

单元测试（06-unit-test）和集成测试（08-integration-test）各自独立为一个 EXEC 阶段，采用 Executor->Evaluator 模式。

**Executor（sonnet 模型）职责**：
1. 执行对应类型的测试命令（如 `npm test`、`pytest`、`go test`）
2. 捕获测试输出（stdout/stderr），提取总用例数、通过/失败/跳过数、覆盖率数据
3. 生成结构化测试执行报告文件（如 `reports/unit-test-execution.json` 或 `reports/integration-test-execution.json`）
4. 报告 SHALL 包含：测试摘要、失败用例详情（测试名、错误信息、堆栈）、覆盖率数据（百分比、未覆盖文件列表）、执行时长

**Evaluator 职责**：
1. 读取 Executor 产出的测试执行报告文件
2. 对照静态 checklist 校验报告完整性和质量（报告是否包含所有必要字段？覆盖率数据是否完整？）
3. 应用诊断决策树分析失败根因
4. 设置 verdict（pass/fail）和 backtrack_to（当 fail 时）
5. 追加条目到 eval.json

**回溯目标**：
- unit-test Evaluator 可回溯到：test-gen（04）、implement（05）、test-design（02）、dev-proposal（03）
- integration-test Evaluator 可回溯到：test-gen（04）、implement（05）、test-design（02）、dev-proposal（03）、unit-test（06）、code-review（07）

#### 5. 诊断决策树

测试执行 Evaluator 在读取 Executor 产出的报告后，应用诊断决策树判断失败根因：

```
测试失败
├── 语法/import/类型错误 + 测试文件行     -> test-gen 阶段
├── 逻辑错误/返回值不符 + 实现文件行       -> implement 阶段
├── 测试期望与 test-design.md 描述冲突    -> test-design 阶段
├── 接口签名不匹配, 两边都符合各自文档     -> dev-proposal 阶段
└── 无法判断                             -> 调用 AskUserQuestion 工具，询问用户选择回溯目标
```

诊断决策树判定为"无法判断"时，Evaluator 不再自动回溯到 dev-proposal，而是调用 Claude Code 的 `AskUserQuestion` 工具向用户发起交互式提问。提问内容 SHALL 包含：

- 失败的测试阶段（unit-test / integration-test）和测试用例名称
- 诊断分析摘要：已排除的原因（语法错误、逻辑错误、设计冲突、接口不匹配）及排除依据
- 可选择的回溯目标：test-design、dev-proposal、test-gen、implement、unit-test、code-review，或填写"其他"
- 可选的跳过回溯直接手动修复的选项

#### 6. 回溯扩展

独立测试阶段 Evaluator 可回溯到之前的阶段：

- 回溯到 test-design: 测试期望与 test-design.md 冲突时
- 回溯到 dev-proposal: 接口签名不匹配时（最保守的安全回溯）
- 回溯到 test-gen: 语法/import/类型错误在测试文件行时
- 回溯到 implement: 逻辑错误在实现文件行时
- 原有 acceptance -> requirements、code-review -> dev-proposal 的回溯链保持不变

#### 7. no-op phase 机制

- skill 层在调用阶段前检查变更范围是否有对应类型的测试文件
- 对于 06-unit-test: 检查是否存在 `*.test.*`、`tests/unit/` 或 `__tests__/` 文件
- 对于 08-integration-test: 检查是否存在 `*.integration.test.*` 或 `tests/integration/` 文件
- 若无 -> 跳过整个阶段，在 eval.json 中附加 `skipped: true` 条目，注明 "skipped: no applicable tests"
- 若有 -> 正常执行 Executor->Evaluator 流程
- no-op 判定在 skill 层完成（减少 agent 调用开销）

#### 8. 架构工具统一到 TypeScript CLI

将 Python 架构工具链（`archi_parser.py`、`archi-model.py`、`archi-validate.py`，共 ~1070 行）迁移到 dev-team TypeScript CLI，集成 `@likec4/core` 作为 C4 解析引擎：

**解析层替换**: `archi_parser.parse_dsl()` -> `LikeC4.fromSource()` — 100% C4 语法兼容，消除自实现解析器的兼容性风险。

**校验层替换**: `archi_parser.validate_structure()` -> `likec4.getErrors()` — 完整 DSL 诊断。

**查询层替换**: `archi-model.py query` -> `LikeC4Model.Computed` API（elements, relationships, parent, children, incoming, outgoing, ancestors, descendants）。

**交叉引用重写**: `archi-validate.py`（590行）-> `lib/c4-cross-ref.ts` — TypeScript 重写 import 解析（TS/JS/Python）和交叉引用逻辑，保留 path_to_element 映射和 violations/warnings 报告生成。

**CLI 命令统一**: `python archi-model.py --command <action>` -> `dev-team archi <action>`。

新增 CLI 命令：

| 命令 | 功能 | 对应 Python |
|------|------|-------------|
| `dev-team archi query [--element <fqn>]` | 查询模型元素/关系 | archi-model.py query |
| `dev-team archi validate [--source <dsl>]` | 校验 C4 DSL 语法 | archi-model.py validate |
| `dev-team archi write --path <f> --source <dsl>` | 校验并写入模型文件 | archi-model.py write |
| `dev-team archi check [--staged \| --files <l>]` | import 交叉引用验证 | archi-validate.py |

依赖变化：
- `package.json` 新增 `@likec4/core: ^1.56.0`（~5-15MB，仅核心库，不含 React/Vite/Graphviz 渲染层）
- 删除 `archi_parser.py`、`archi-model.py`、`archi-validate.py` 三个 Python 文件
- `architecture.md` agent prompt 更新命令引用：`python plugins/dev-team/utils/archi-*.py` -> `dev-team archi <action>`

---

## 变更范围

### 实现以下特性

- requirements 阶段模块边界契约：识别模块目录，定义 public/export、API、CLI、props/events，写入 specs/ 目录
- test-gen/implement 文件类型隔离：通过 agent prompt 约束 + skill 层面校验
- 移除 E2E 测试支持，测试仅分 unit + integration（均使用 mock）
- implement 阶段 AUTO 步骤简化：仅保留 static-check（lint + type-check），单元测试移出
- 新增 06-unit-test 独立阶段：Executor（sonnet 模型）运行单元测试 -> 产出结构化报告 -> Evaluator 校验报告质量 + 应用诊断决策树
- 新增 08-integration-test 独立阶段：Executor（sonnet 模型）运行集成测试 -> 产出结构化报告 -> Evaluator 校验报告质量 + 应用诊断决策树
- 07-code-review 阶段简化为单 Evaluator（移除 integration-test 子步骤）
- test-execution Evaluator（在 06-unit-test 和 08-integration-test 阶段）含诊断决策树，支持自动回溯
- 回溯目标扩展：unit-test Evaluator 和 integration-test Evaluator 可回溯到 test-design/dev-proposal/test-gen/implement
- no-op phase 机制：skill 层跳过无需执行测试的独立阶段
- 架构工具统一到 TypeScript CLI：集成 `@likec4/core`，移除 Python archi 工具链，新增 `dev-team archi` 子命令
- `archi_parser.py` 的 `parse_dsl()` 替换为 `@likec4/core` 的 `LikeC4.fromSource()` — 100% C4 语法兼容
- `archi-validate.py` 的 import 交叉引用逻辑重写为 `lib/c4-cross-ref.ts`

### 不要修改

- 9 个阶段（requirements、test-design、dev-proposal、test-gen、implement、unit-test、code-review、integration-test、acceptance）的逐级推进模式不动
- requirements/test-design/dev-proposal 的 DESIGN Planner->Evaluator 模式不动
- test-gen 的 EXEC Generator->Evaluator 模式不动（测试代码生成）
- acceptance 阶段的职责不变
- eval.json 的 append-only 格式不变
- test-design 阶段的模板结构不变（仅增加测试分类标记）
- 插件版本号不改动（由后续发布变更处理）
- 不引入容器级隔离或 git worktree 隔离（作为未来增强选项，当前不实现）
- `archi-decide.py`（ADR 管理）不在此次变更范围，保持不变

---

## 能力

### 新增能力

- `module-boundary-contracts` — requirements 阶段产出的结构化接口契约，定义跨模块的公共签名（函数、API、CLI、组件），供 test-gen/implement 引用做交叉验证
- `test-execution-diagnostics` — 独立测试执行阶段的 Executor（sonnet）+ Evaluator 模式；Executor 运行测试并产出结构化报告，Evaluator 读取报告并应用诊断决策树判定回溯目标
- `file-type-isolation` — Agent 文件类型读取约束机制，通过 prompt 黑名单 + skill 层校验实现 test-gen/implement 上下文隔离
- `c4-cli-integration` — dev-team CLI 集成 `@likec4/core` 作为 C4 模型解析引擎，提供 `dev-team archi` 子命令（query / validate / write / check）

### 修改的能力

- `pge-workflow-engine` — 阶段数从 7 扩展到 9（新增 unit-test 和 integration-test）；AUTO 步骤从"static-check + unit-test"简化为仅 static-check；code-review 从双 evaluator 简化为单 evaluator；新增独立测试阶段的 EXEC（Executor->Evaluator）流程支持；新增 no-op phase 判定支持新阶段
- `phase-agents` — 新增 unit-test-executor（sonnet）和 unit-test-evaluator 作为 06-unit-test 阶段的独立 agent；新增 integration-test-executor（sonnet）和 integration-test-evaluator 作为 08-integration-test 阶段的独立 agent；移除了 implement 阶段内部的 unit-test-execution-evaluator 和 code-review 阶段内部的 integration-test-execution-evaluator；test-gen/implement 阶段的 Generator agent 文件类型读取约束保留；architecture agent 命令引用更新为 `dev-team archi`
- `phase-skills` — 新增 `dev-team:phase-unit-test` 和 `dev-team:phase-integration-test` skill；implement 阶段 skill 移除 unit-test AUTO 子步骤，仅保留 static-check；code-review 阶段 skill 移除 integration-test 子步骤，恢复为单 Evaluator；unit-test/integration-test 的 skill 路由逻辑为：Executor（sonnet）-> Evaluator
- `pipeline-backtrack` — 扩展回溯目标：unit-test Evaluator 和 integration-test Evaluator 可回溯到 test-design/dev-proposal/test-gen/implement；移除原有 code-review->dev-proposal 的回溯限制（保持）；新增回溯链完整性校验包含新阶段的相序关系
- `eval-check-cli` — 更新 eval-check 以支持 9 阶段流水线；新增 phase state 对 unit-test 和 integration-test 的 no-op 检测支持；更新阶段依赖关系以匹配扩展的回溯链
- `architecture-model` — **BREAKING**: `archi_parser.py` 的 `parse_dsl()` 和 `archi-model.py` 的查询/校验逻辑替换为 `@likec4/core` API（`LikeC4.fromSource()` + `LikeC4Model.Computed`），以 dev-team CLI `archi` 子命令形式暴露
- `architecture-validation` — **BREAKING**: `archi-validate.py` 的 import 交叉引用和报告生成逻辑重写为 TypeScript（`lib/c4-cross-ref.ts`），以 dev-team CLI `archi check` 子命令形式暴露

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | requirements-planner 产出 specs/ 目录中包含模块边界定义的 capability spec，包含 public 函数签名、API 接口、CLI 命令的结构化表格 | 执行一次完整的 requirements 流程，检查 specs/ 中受影响 capability 的 spec.md 文件是否包含接口契约表格 | P0 |
| AC-2 | test-gen-generator 的 agent prompt 包含源码文件类型黑名单（.ts/.py/.js/.rs/.go/.java），且 skill 层在 Generator 执行后校验无违规读取 | 检查 test-gen 阶段 Generator prompt 模板内容和 skill 路由代码中的校验逻辑 | P0 |
| AC-3 | implementation-generator 的 agent prompt 包含 tests/ 目录黑名单，且 skill 层校验无违规读取 | 检查 implement 阶段 Generator prompt 模板内容和 skill 路由代码中的校验逻辑 | P0 |
| AC-4 | implement 阶段 AUTO 步骤仅执行 static-check（lint + type），不再包含单元测试执行 | 执行 implement 阶段全流程，检查 eval.json 中 implement 条目只有 static-check（无 unit-test-execution），且 timestamp 记录正确 | P0 |
| AC-5 | 06-unit-test 阶段 Executor（sonnet 模型）执行单元测试命令，产出结构化测试执行报告文件（JSON 格式），包含总用例数、通过/失败/跳过数、覆盖率百分比、失败用例详情 | 触发 06-unit-test 阶段执行，检查 `reports/` 目录下存在结构化报告文件且字段完整 | P0 |
| AC-6 | 06-unit-test 阶段 Evaluator 读取 Executor 产出的报告，对照 checklist 校验报告完整性（含全部必要字段），应用诊断决策树判定 verdict 和 backtrack_to | 构造一个单元测试失败场景，检查 Evaluator 输出中 checklist 条目和 backtrack_to 字段的正确性 | P0 |
| AC-7 | 08-integration-test 阶段 Executor（sonnet 模型）执行集成测试命令，产出结构化测试执行报告文件（JSON 格式），内容格式与 unit-test 报告一致 | 触发 08-integration-test 阶段执行，检查 `reports/` 目录下存在结构化报告文件且字段完整 | P0 |
| AC-8 | 08-integration-test 阶段 Evaluator 读取 Executor 产出的报告，校验报告质量，应用诊断决策树判定 verdict 和 backtrack_to | 构造一个集成测试失败场景（根因在单元测试报告不完整），验证 backtrack_to 设置为 06-unit-test | P0 |
| AC-9 | 测试失败时 unit-test / integration-test Evaluator 能根据诊断决策树正确识别根因并设置 backtrack_to（语法错误->test-gen，逻辑错误->implement，设计冲突->test-design，接口不匹配->dev-proposal） | 构造 4 种已知失败场景各一个，运行测试执行 Evaluator 验证 backtrack_to 字段 | P0 |
| AC-10 | 无集成测试的变更，08-integration-test 阶段输出 "skipped: no applicable tests" 并 pass，eval.json 包含 skipped: true | 纯配置文件变更走 requirements 到 acceptance 全流程验证 | P1 |
| AC-11 | 不再生成或引用 E2E 测试文件；测试仅分 unit + integration 两类（均使用 mock） | Grep 确认 templates/ 和 agents/ 中无 e2e 相关内容；检查 test-design.md 模板新增测试分类标记 | P1 |
| AC-12 | static-check 失败时不触发 06-unit-test 阶段，直接回溯 implement 阶段 | 构造一个语法错误的实现，验证 implement 阶段在 static-check 失败后不触发 unit-test 阶段，eval.json 中 unit-test 无条目 | P1 |
| AC-13 | eval-check CLI 能正确识别被 no-op 跳过的 phase（包括 unit-test 和 integration-test），不将其视为缺失或失败 | 构造一个跳过集成测试的变更，运行 eval-check 验证输出包含 "skipped" 状态且不阻塞存档流程 | P2 |
| AC-14 | 07-code-review 阶段为单 Evaluator（不再包含 integration-test-execution 子步骤），eval.json 中 code-review 仅有一条条目 | 执行 code-review 阶段全流程，检查 eval.json 中 07-code-review 仅一条 evaluator 条目 | P1 |
| AC-15 | `dev-team archi query` 输出与当前 `archi-model.py query` 等价的元素和关系 JSON | 对比同一个 .c4 模型文件的新旧命令输出 | P0 |
| AC-16 | `dev-team archi validate` 校验 .c4 DSL 语法，100% 兼容 LikeC4 语法（含 views、deployment、tags 等当前 parser 不支持的特性） | 用 LikeC4 官方 example 项目中的 .c4 文件做输入，验证校验通过 | P0 |
| AC-17 | `dev-team archi check --staged` 输出与当前 `archi-validate.py --staged` 等价的 violations/warnings 报告 | git diff 同样变更集，对比新旧命令输出的 violations 数量和类型 | P0 |
| AC-18 | `architecture.md` agent 中所有 `python plugins/dev-team/utils/archi-*.py` 调用替换为 `dev-team archi` 等效命令 | 全文搜索 architecture.md 确认无残留 Python 命令引用 | P1 |
| AC-19 | `archi_parser.py`、`archi-model.py`、`archi-validate.py` 三个文件已删除 | 确认 plugins/dev-team/utils/ 目录下不存在这三个文件 | P1 |
| AC-20 | 诊断决策树判定"无法判断"时，Evaluator 调用 AskUserQuestion 工具向用户询问回溯目标，提问内容包含错误摘要、已排除项列表和可选回溯目标列表 | 构造一个无法归类到已知失败模式的测试失败场景，验证 Evaluator 调用 AskUserQuestion 且提问内容包含诊断信息（测试名称、已排除原因、回溯目标选项） | P1 |
| AC-21 | AskUserQuestion 超时后（默认 5 分钟）Evaluator 回退到回溯 dev-proposal 并输出诊断报告，超时行为记录在 eval.json 的 findings 中 | 设置 askUserTimeout=10 秒，构造"无法判断"场景，等待超时后验证 backtrack_to=dev-proposal 且 findings 包含超时记录 | P1 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 文件类型隔离仅靠 prompt 约束，Agent 可能"越狱"读取黑名单文件类型 | 隔离失效，implementation-generator 看到测试文件 | 中 | test-gen/implement 阶段的 Generator agent prompt 中硬编码文件类型黑名单和目录黑名单；skill 层校验 Read 工具调用路径；每个 Generator 执行结束后输出读取文件列表供审核。如果出现越狱案例，下一迭代叠加 worktree 级物理隔离 |
| 诊断型 Evaluator 的根因分析准确率不确定，可能误判回溯目标 | 回溯目标错误，浪费迭代次数 | 中 | 诊断 prompt 内置决策树作为硬约束（IF/THEN 逻辑链）；当多个可能原因同时存在时，优先回溯到设计层 dev-proposal（最安全的选择）；Evaluator 输出诊断报告供人工复核 |
| 模块边界契约格式不统一导致 test-gen 和 implement 各自理解偏差 | 集成阶段大量接口不匹配，需要多次迭代修复 | 中 | requirements 阶段边界模板使用结构化表格格式，包含明确的参数名、类型、默认值、是否必需等字段；test-gen/implement 阶段的 Evaluator 必须引用契约 ID 做交叉验证；如果发现不匹配，直接在 Evaluator 中标记并设置 backtrack_to=requirements |
| Executor（sonnet）产出的测试报告格式不稳定或不完整，导致 Evaluator 无法正确校验 | Evaluator 校验失败率增加，需要人工介入修正报告 | 中 | Executor prompt 中使用结构化 JSON schema 约束报告格式；Evaluator 的 checklist 将"报告字段完整性"作为必检项；如果报告缺失关键字段，Evaluator 设置 verdict=fail 并追加 "report_incomplete" 标记，要求重新执行 |
| 9 阶段工作流相比原来的 7 阶段增加流程节点数，可能增加用户的阶段切换认知负担 | 用户需要多执行 2 次手工阶段切换，降低开发效率 | 低 | unit-test 和 integration-test 阶段设计为"免交互"（Executor 自动运行，Evaluator 自动判断），除非需要回溯否则用户无需介入；no-op 机制在无测试时自动跳过两个阶段，不影响纯配置/文档变更 |
| no-op 判定在 skill 层实现，如果判定逻辑有 bug 可能导致必要的测试被跳过 | 变更质量下降，未检测到的回归 | 低 | no-op 判定逻辑只在"无对应类型测试文件"时触发；判定结果写入 eval.json 的 phase 条目中作为 evidence；存档流程的 eval-check 可配置为在关键变更上强制要求测试执行 |
| static-check 失败后跳过单元测试阶段，可能遗漏部分可执行的测试 | 部分逻辑错误延迟到集成测试阶段才发现 | 低 | static-check 失败退回 Generator 修复，修复后从头执行完整 implement 阶段流程（含 static-check 和后续 unit-test 阶段）；如果 static-check 失败仅涉及新增代码的格式化问题，可考虑静默修复后继续 |
| `@likec4/core` npm 依赖在网络受限环境不可用 | 离线环境 npm install 失败，archi 命令无法使用 | 低 | `@likec4/core` 随 plugin 安装时在 bin/ 下 `npm install`，与现有的 `@fission-ai/openspec` 自举逻辑相同；安装失败时不阻塞非 archi 命令 |
| TypeScript 重写的 `lib/c4-cross-ref.ts` import 解析器与原 Python 版行为不一致 | archi check 输出与旧 archi-validate.py 不同，遗漏或误报 | 中 | AC-17 要求输出等价验证；用现有项目的 staged diff 做回归测试；import 解析正则从 Python 逐行对照翻译 |
| `@likec4/core` API breaking change | dev-team archi 命令行为变化或报错 | 低 | `package.json` 锁定 `@likec4/core` 版本为 `^1.56.0`（minor 升级，不自动升级 major）；升级前在测试环境验证 |
| AskUserQuestion 等待用户响应期间流水线暂停，用户长时间不响应导致流水线阻塞 | 流水线阻塞，影响开发效率 | 中 | 设置默认超时 5 分钟，超时后自动回退到 dev-proposal 并输出诊断报告；超时时长通过 CLAUDE.md 的 `askUserTimeout` 配置，项目可自行调整；Evaluator 在 eval.json 中记录超时事件，便于事后审计 |

---

## 待讨论事项

- **诊断决策树"无法判断"时的策略**: 改为调用 AskUserQuestion 工具向用户交互式询问回溯目标，默认超时 5 分钟后自动回退到 dev-proposal。
- **集成测试运行环境**: unit-test 和 integration-test 的区分标准待定。建议：不依赖外部服务/数据库的测试标记为 unit，依赖 mock 外部服务的标记为 integration。integration 测试执行由 Executor 自行准备 mock 环境。
- **implement 阶段 AUTO static-check 失败回退策略**: 如果 static-check 失败，是回到 Generator 重新生成还是允许手动修改后继续？建议 static-check 失败直接退回 Generator（保持 Generator->Evaluator 循环完整性），但允许用户在 eval.json 中添加 manual-override 标记强制继续。
- **`@likec4/core` 与现有 DSL 文件的兼容性**: 现有 `models/` 目录为空（无 .c4 文件），首次集成无迁移成本。但需确认 `@likec4/core` 对模板 `model.c4` 中的语法完全兼容（specification block + extend 层级 + metadata 块）。
- **archi check 报告格式**: import 交叉引用报告输出 JSON 格式是否与旧 `archi-validate.py` 完全一致？建议在 `architecture.md` agent 更新前，用相同的 staged diff 逐字段对比新旧输出。
