# 测试设计: drop-integration-test-design

> **日期**: 2026-09-28

---

## 验收范围

<!-- 本变更为零行为变更（Markdown 模板 / agent prompt 文案与 TS 字符串字面量，design.md 明确「无逻辑、接口、数据结构变更」，
  且决策「测试文件：预期零新增、零修改」「不为 agent / 模板文案新增守卫测试」。
  逐 AC 判定：全部 AC 均为纯静态约束（模板/prompt 文本结构、文件不存在性、grep 核对、构建同步），无进程内可验证行为，
  故测试类型统一填「不可测试」，原因逐条落「不可测试项」；回归面由既有套件承接（见不可测试项 10），不虚构用例。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|----------|
| AC-1 | test-design.md.template 删「测试类型」列与 `## 集成测试` 整章 | 不可测试 | —（见不可测试项 1） |
| AC-2 | 模板单元测试章节写入最小 mock 原则与挂靠规则 | 不可测试 | —（见不可测试项 2） |
| AC-3 | test-design-planner.md 删 Step 9 与命名指南、Step 8 改名、Step 13 去集成段 | 不可测试 | —（见不可测试项 3） |
| AC-4 | test-design-planner.md Step 11 增加组合用例挂靠指引 | 不可测试 | —（见不可测试项 4） |
| AC-5 | test-design-evaluator.md T2/T5 去集成、删 T8-I 块、T8-G1 改 3 列 | 不可测试 | —（见不可测试项 5） |
| AC-6 | test-design-evaluator.md 新增最小 mock 硬检查 | 不可测试 | —（见不可测试项 6） |
| AC-7 | test-gen-generator.md Step 2 仅解析单元测试表格 | 不可测试 | —（见不可测试项 7） |
| AC-8 | test-gen-evaluator.md 删 G4、重编号、G5 收紧、Process 去集成对照 | 不可测试 | —（见不可测试项 8） |
| AC-9 | 删除 3 个死模板 | 不可测试 | —（见不可测试项 9） |
| AC-10 | cli.ts / workflow.ts 文案清理 | 不可测试 | —（见不可测试项 10） |
| AC-11 | 版本 bump + 重建产物 | 不可测试 | —（见不可测试项 11） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景。本变更为零行为变更，`test_resolve_paths` 对 8 个修改文件解析出 2 个单测对
  （测试文件均既有）与 6 个 Not a testable source file（落不可测试项）。
  两个既有测试文件仅保留章节框架：design 决策测试文件零新增、零修改，不设任何用例；既有用例构成回归网，由 test-execution 阶段全量回归承接。 -->

### `plugins/dev-team/bin/src/cli.ts` -> `plugins/dev-team/bin/src/cli.test.ts`

<!-- design.md 公共函数 / API 仅声明 `dev-team test-execution` 子命令的描述文案变更（命令名、选项、action、执行路径不变），
  属字符串字面量修改而非新增可测行为，故不列待测功能条目、不设用例（proposal：不为文案新增守卫测试）。 -->

#### 待测功能

<!-- design.md 未声明该文件的新增公共 API 或行为变更——唯一条目 `dev-team test-execution` 为纯描述文案改写，行为契约不变。 -->

#### 用例

<!-- 零新增用例（design 决策：测试文件预期零新增、零修改）。已核实全仓库无测试断言 `unit + integration` 文案；
  既有 `cli.test.ts` 与 `bin/__tests__/cli-test-execution-execute/` 套件继续覆盖该命令的注册与执行行为，构成回归网。 -->

#### Mock策略

<!-- 无新增 mock 需求：不新增用例。 -->

### `plugins/dev-team/bin/src/lib/workflow.ts` -> `plugins/dev-team/bin/src/lib/workflow.test.ts`

<!-- design.md 未声明该文件的公共 API 变更（无函数签名、类型、phase 表结构变更），仅 3 处 test-execution phase prompt 字符串字面量文案改写。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更。 -->

#### 用例

<!-- 零新增用例（design 决策：测试文件预期零新增、零修改）。既有 workflow.test.ts 的 phase 表守卫用例
  （如「requirement 表应不包含 unit-test 和 integration-test」「bug-fix 表应不包含 integration-test」）断言对象为 phase id 枚举而非 prompt 文案，
  本次改写不触及，保持原样继续通过；已核实全仓库无测试断言 `unit + integration` 文案。 -->

#### Mock策略

<!-- 无新增 mock 需求：不新增用例。 -->

---

## 集成测试

<!-- 本变更为纯文案/静态变更：cli.ts 与 workflow.ts 仅字符串字面量改写，不产生新的跨模块交互；
  模板与 agent prompt 为 Markdown 文本，无运行时行为。无跨模块交互，不设集成测试关系。 -->

---

## 不可测试项

<!-- `test_resolve_paths` 对 6 个修改文件返回 Not a testable source file（4 个 agents/*.md、1 个 templates/**.md.template、1 个 package.json），
  连同各 AC 的静态约束性质逐条记录如下。 -->

- **AC-1（模板删「测试类型」列与 `## 集成测试` 整章）** — **原因**: `test-design.md.template` 为 Markdown 模板，已核实 `bin/src` 无任何代码读取该文件，模板文本无进程内可验证行为；proposal 明确「不为 agent / 模板文案新增守卫测试」。核对方式为静态 grep / 代码审查。
- **AC-2（模板写入最小 mock 原则与挂靠规则）** — **原因**: 同为模板注释文本约束，无运行时行为；其效果由后续变更的 test-design 产物按新模板生成时体现，经 test-design 流程自身的评审链路把关，不属于本变更的自动化测试对象。
- **AC-3（planner 删 Step 9 与命名指南、Step 8 改名、Step 13 去集成段）** — **原因**: agent prompt 的步骤增删 / 改名为 Markdown 文案约束，prompt 无可执行入口；`test_resolve_paths` 报 Not a testable source file；自动化守卫被 proposal 决策排除。
- **AC-4（planner Step 11 增加组合用例挂靠指引）** — **原因**: 同为 agent prompt 文本内容约束（「不作为」类：不得设独立章节 / `__tests__/` 组合区），无可执行行为。
- **AC-5（evaluator T2/T5 去集成、删 T8-I 块、T8-G1 改 3 列）** — **原因**: evaluator checklist 条目调整为 prompt 文本约束，无运行时行为；`test_resolve_paths` 报 Not a testable source file。
- **AC-6（evaluator 新增最小 mock 硬检查 T9）** — **原因**: T9 的存在性与判据文本为 prompt 内容约束；其判定质量（进程边界 vs 内部模块的语义区分是否过严/过松）只能经真实工作流首跑人工观察（proposal 风险表缓解措施「首跑观察 evaluator 报告再调」），非进程内自动化测试范畴。
- **AC-7（generator Step 2 仅解析单元测试表格）** — **原因**: 「不作为」类文案约束（Step 2 不得引用集成测试表格），agent prompt 无可执行入口。
- **AC-8（test-gen-evaluator 删 G4、重编号、G5 收紧、Process 去集成对照）** — **原因**: checklist 条目删改 / 重编号为 prompt 文本约束，无运行时行为。
- **AC-9（删除 3 个死模板且全仓库无引用）** — **原因**: 文件不存在性约束。已核实 `test-jest.js` / `test-pytest.py` / `test-rust.rs` 源码零引用（现存引用仅 dist 产物清单，由重建后自然消除）；「文件不存在」由静态核对与 code-review / acceptance 阶段确认，不设守卫测试。
- **AC-10（cli.ts / workflow.ts 文案清理）** — **原因**: TS 字符串字面量文案改写，命令名、选项、action 与 phase 表结构均不变，无行为契约变化；grep 无 `unit + integration` 属静态核对。行为回归由既有套件承接：`workflow.test.ts` 的 phase 表守卫、`cli.test.ts` 与 `bin/__tests__/cli-test-execution-execute/` 套件均不断言被改文案（已全仓库核实），零新增、零修改用例。
- **AC-11（版本 bump + 重建产物）** — **原因**: `plugins/dev-team/package.json` 的 version 字段变更与 dist 三棵产物树同步属构建流程核对；`test_detect_frameworks` 对 package.json 判 unknown，`test_resolve_paths` 报 Not a testable source file，无进程内自动化测试路径。产物与源同步由实现后的静态核对与 acceptance 阶段检查。
- **`plugins/dev-team/agents/test-design-planner.md` 等 4 个 agents/*.md 与 `plugins/dev-team/templates/artifacts/test-design.md.template`** — **原因**: `test_resolve_paths` 返回 Not a testable source file；纯 Markdown prompt / 模板文件，无可执行源码，不解析测试路径，不为其创建测试文件。
