# 提案: drop-integration-test-design

> **变更**: drop-integration-test-design
> **日期**: 2026-09-28
> **状态**: draft

---

## 问题

现行 test-design 是双层测试模型：solitary 单元测试（默认 mock 依赖）+ 独立集成测试层（组合真实模块，独立章节 / 独立 checklist / `__tests__/` 路径约定）。两条规则并行导致：

1. **最小 mock 原则纸面化**。单元测试模板的 `Mock主体` 定义已写明「系统源代码之外的依赖」，但无检查强制；跨模块用例另走集成通道后，单元测试层实际放任内部模块 mock，双层模型的分工从未被真正执行。
2. **双通道冗余**。同一「跨模块行为」在 test-design 模板（`## 集成测试` 整章）、test-design-planner（Step 9 + 命名指南）、test-design-evaluator（T2/T5 集成方向 + T8-I1..I8 整块）、test-gen-generator（Step 2 双表解析）、test-gen-evaluator（G4/G5）各维护一套独立表述与检查，而 integration-test 阶段早已从 phase 表移除、`test_resolve_paths` 早已只返回 `unit_tests` —— 设计语言与管线现状脱节。

---

## 提案

收成单层 **sociable 单元测试**模型：把集成层的「仅跨进程边界才 mock」原则吸收进单元测试层并升格为硬检查，然后删掉集成测试这一层。

1. **最小 mock 原则硬检查**（含入参例外）：
   - 允许 mock：进程边界依赖（数据文件 / 配置 / DB / 接口 / 网络 / 子进程 / 全局变量 / 运行环境）。
   - 入参例外：内部模块本身作为被测 API 的显式入参 / 注入依赖传入时，允许 mock —— 被测 API 的契约是「如何消费这个输入」，输入方模块自身的正确性由它自己的单元测试保证。
   - 禁止 mock：其余一切内部模块间调用；被测模块内部 import 并调用的协作模块必须真实组合。
   - 落点：test-design-evaluator 新增检查项；test-gen-evaluator 的 G5 收紧为「生成代码中的模块 mock 声明必须能对应到 Mock策略 表合法条目」。
2. **组合用例挂靠链路入口模块**：跨模块组合用例 colocate 到链路入口模块（链路发起方 / 最上层调用方）的 per-file 章节 `#### 用例` 表，describe 标题可写链路（如 `CLI参数 → workflow.json持久化`）；不设独立章节、不设 `__tests__/` 组合测试区。`test_resolve_paths` 的 colocated 推导天然覆盖，工具零改动；G4 删除后 G3 单表核对自然覆盖组合用例，test-gen 全流程不需要「组合」概念。
3. **摘除集成层**：模板 `## 集成测试` 整章与「测试类型」列、planner Step 9 与命名指南、evaluator T8-I1..I8 整块、generator 双表解析、G4 全部移除；T8-G1 随表格改写为 3 列（「保留 5 列却列 4 名」笔误自然消解）。
4. **文案清理与死模板删除**：`cli.ts` 与 `workflow.ts` 的 "(unit + integration)" 改写；`templates/test-jest.js` / `test-pytest.py` / `test-rust.rs` 全仓库零引用，直接删除。

sociable 模型下真实组合仍然存在（被测模块内部 import 的协作模块不 mock），跨模块行为覆盖不丢失。

---

## 能力

### 新增能力

- 无 —— 本变更是对既有 test-design / test-gen 设计语言的收敛，不引入新能力。

### 修改的能力

- **phase-agents** — `test-design-planner.md` / `test-design-evaluator.md` / `test-gen-generator.md` / `test-gen-evaluator.md` 摘除集成测试层；最小 mock 原则升格为硬检查。
- **json-design-schemas** — `test-design.md.template` 单层化：删「测试类型」列与 `## 集成测试` 整章；单元测试章节注释写入最小 mock 原则（含入参例外）与组合用例挂靠规则。
- **pge-workflow-engine** — test-execution phase 表描述与 `workflow.ts` phase prompt 文案去「单元+集成」表述。
- **test-only-workflow** — test-only phase 表描述同步去层级表述。
- **cli-unit-test-execute** — `test-execution` 子命令描述去 "(unit + integration)"。

---

## 变更范围

### 实现文件

- `plugins/dev-team/templates/artifacts/test-design.md.template` — 删「测试类型」列与 `## 集成测试` 整章；单元测试章节注释写入最小 mock 原则（含入参例外）与组合用例挂靠规则；验收范围表改 3 列
- `plugins/dev-team/agents/test-design-planner.md` — 删 Step 9 与「集成测试关系标题命名指南」；Step 8 改名「测试框架识别」；Step 11 增加组合用例挂靠指引；Step 13 写入顺序去掉集成段；验收范围映射规则同步 3 列
- `plugins/dev-team/agents/test-design-evaluator.md` — T2/T5 去集成方向；删 T8-I 整块；T8-G1 随表格改写；新增最小 mock 硬检查
- `plugins/dev-team/agents/test-gen-generator.md` — Step 2 表格解析去掉「集成测试」引用
- `plugins/dev-team/agents/test-gen-evaluator.md` — 删 G4；G5 收紧；Process step 4 去集成对照；后续条目重编号
- `plugins/dev-team/bin/src/cli.ts` — test-execution 命令描述 `Run all automated tests with coverage and generate execution report`
- `plugins/dev-team/bin/src/lib/workflow.ts` — 三处 test-execution prompt 改为 `Run and fix all tests for change "<change>".`
- `plugins/dev-team/package.json` — 版本 bump，并按项目规则 `pnpm -C plugins/dev-team run build` 重建产物

### 测试文件

- 预期零新增。仅当既有断言引用被改文案时跟随修改（当前全仓库未发现断言 `unit + integration` 的测试；`workflow.test.ts` 既有「phase 表不含 integration-test」守卫保持不变）。
- 不为 agent / 模板文案新增守卫测试。

### 删除文件

- `plugins/dev-team/templates/test-jest.js`
- `plugins/dev-team/templates/test-pytest.py`
- `plugins/dev-team/templates/test-rust.rs`

### 不要修改

- `test_resolve_paths` / `test_detect_frameworks` 工具及其逻辑（colocated 推导已覆盖组合用例；框架识别调用保留）
- phase 表 / phase id（integration-test 阶段已移除，既有守卫测试保持）
- `plugins/dev-team/bin/__tests__/**`、`plugins/dev-team/build/__tests__/**` 中自称「集成测试」的存量测试文件与注释 —— 存量不动，未来新增组合用例按新规则 colocate，两种风格并存可接受
- `dev-design-planner.md` / `dev-design-evaluator.md` / `code-analyze-planner.md` 中「design 不得含 test 章节」类禁令文案（禁令在新模型下依然成立）
- `openspec/specs/**`（经本变更 `specs/` delta 合并，不直接编辑）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | test-design.md.template 删「测试类型」列与 `## 集成测试` 整章 | 读模板：无 `测试类型`、无 `## 集成测试`；验收范围表头为 `AC ID \| 验收条件 \| 被测文件或模块`（被测列单值填承载用例的测试文件） |
| AC-2 | 模板单元测试章节写入最小 mock 原则与挂靠规则 | `#### Mock策略` 注释含：进程边界依赖白名单、被测 API 显式入参例外、内部模块必须真实组合、组合用例挂靠链路入口模块 |
| AC-3 | test-design-planner.md 删 Step 9 与命名指南、Step 8 改名、Step 13 去集成段 | 读 planner：无集成章节生成步骤、无「集成测试关系标题命名指南」节；Step 8 标题为「测试框架识别」；写入顺序无 `## 集成测试` |
| AC-4 | test-design-planner.md Step 11 增加组合用例挂靠指引 | Step 11 含链路入口模块挂靠规则与链路式 describe 命名指引 |
| AC-5 | test-design-evaluator.md T2/T5 去集成、删 T8-I 块、T8-G1 改 3 列 | checklist 无集成方向表述、无 T8-I1..I8 条目；T8-G1 判据与模板 3 列一致 |
| AC-6 | test-design-evaluator.md 新增最小 mock 硬检查 | 存在 Mock策略 检查项：Mock主体 必须为进程边界依赖或被测 API 显式入参的内部模块；内部 wiring mock（如 `vi.mock(仓库内模块路径)`）→ fail |
| AC-7 | test-gen-generator.md Step 2 仅解析单元测试表格 | Step 2 无 `集成测试` 引用 |
| AC-8 | test-gen-evaluator.md 删 G4、重编号、G5 收紧、Process 去集成对照 | checklist 无集成用例骨架检查行；mock 检查项含「模块 mock 声明必须对应 Mock策略 表合法条目」；Process 逐表对照仅剩单元测试表格 |
| AC-9 | 删除 3 个死模板 | `templates/test-jest.js`、`test-pytest.py`、`test-rust.rs` 不存在，全仓库无引用 |
| AC-10 | cli.ts / workflow.ts 文案清理 | 全仓库 grep 无 `unit + integration`；CLI 命令描述与 3 处 phase prompt 已改写 |
| AC-11 | 版本 bump + 重建产物 | `plugins/dev-team/package.json` version 已 bump；`claude-plugins/`、`cursor-plugins/`、`cursor-home-image/` 中 dev-team 产物经 build 刷新与源同步 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 最小 mock 判定需语义理解（进程边界 vs 内部模块），evaluator 可能过严或过松 | test-design / test-gen 误判回环 | 中 | 判据写入模板注释与 evaluator 检查项：白名单 + 入参例外 + 反例（`vi.mock(仓库内模块路径)` → fail）；首跑观察 evaluator 报告再调 |
| 组合用例集中到链路入口模块测试文件，单文件体积增长 | 测试文件可读性下降 | 低 | describe 标题写链路保持可导航；观察后再议拆分，本变更不预设机制 |
| 存量 `__tests__/` 自称「集成测试」的文件与新模型并存 | 认知混乱 | 低 | 存量不动（决策 D6）；若日后造成困扰，另起迁移 change |
| agent 文案变更牵连既有文本断言测试 | 测试套件变红 | 低 | `vp test` 全量回归；被牵连断言跟随改写 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 跨模块组合用例挂靠到哪里 | 全部 colocate 到链路入口模块的 per-file 章节 `#### 用例` 表，不设独立章节 / `__tests__/` 组合测试区 | `test_resolve_paths` 的 source→test_file colocated 推导天然覆盖，工具零改动；G4 删除后 G3 单表核对自然覆盖，test-gen 无需「组合」概念 | 保留独立组合测试章节；新建 `__tests__/` 组合测试区 |
| 验收范围表如何简化 | 删「测试类型」列；「被测文件或模块」只填承载用例的测试文件（单值）；AC 追溯保持文件级映射，不为组合用例补替代机制 | 单层模型不再需要分类；单元用例表本就没有 AC 列 | 保留类型列标注 sociable / solitary |
| 最小 mock 原则如何落地 | 升格为 evaluator 硬检查（白名单 + 入参例外），test-gen 侧 G5 收紧为 mock 合法性对照 | 纸面原则无强制是双层模型失效的根因 | 仅写模板注释不作检查 |
| 内部模块可否 mock | 入参例外：作为被测 API 显式入参 / 注入依赖传入时允许 | 被测 API 契约是「如何消费输入」；真实调用输入方会把两个模块的失败混在一起 | 一律禁止 mock 内部模块 |
| 文案清理范围 | 仅 `cli.ts:19` 与 `workflow.ts` ×3；存量测试文件自称「集成测试」的注释不动 | 源头文案影响运行时输出；存量测试注释不影响行为 | 全仓库迁移存量注释 |
| 死模板处置 | `test-jest.js` / `test-pytest.py` / `test-rust.rs` 直接删除 | 全仓库零引用；test-gen-generator 只引用 test-design.md.template，框架语法表内嵌 agent 正文 | 保留待用 |

### 待决问题

- 无 —— 挂靠 / 验收范围 / mock 检查三项设计点与范围边界均已定。唯「存量集成测试风格文件」若日后与新模型并存造成困扰，可另起迁移 change。

---
