# 设计: drop-integration-test-design

> **变更**: drop-integration-test-design
> **日期**: 2026-09-28

---

## 提案与规格同步状态

`proposal.md` 与 `specs/**`（phase-agents / json-design-schemas / pge-workflow-engine / test-only-workflow / cli-unit-test-execute 共 5 个能力 delta）已由提案阶段写入并通过评审。本设计不重复其内容，提案与规格产物不列入下方变更清单与任务列表。

## 方案概述

将双层测试模型（solitary 单元测试 + 独立集成测试层）收敛为单层 **sociable 单元测试**模型，共四个动作：

1. **最小 mock 原则升格为硬检查**：允许 mock 进程边界依赖（数据文件 / 配置 / DB / 接口 / 网络 / 子进程 / 全局变量 / 运行环境）；入参例外——内部模块作为被测 API 显式入参 / 注入依赖传入时允许 mock；其余内部模块间调用禁止 mock、必须真实组合。落点：test-design 模板注释 + test-design-evaluator 新增检查项 + test-gen-evaluator G5 收紧。
2. **组合用例挂靠链路入口模块**：跨模块组合用例 colocate 到链路入口模块（链路发起方 / 最上层调用方）的 per-file 章节 `#### 用例` 表，describe 标题可写链路（如 `CLI参数 → workflow.json持久化`）；不设独立章节、不设 `__tests__/` 组合测试区。`test_resolve_paths` 的 colocated 推导天然覆盖，工具零改动。
3. **摘除集成层**：模板 `## 集成测试` 整章与「测试类型」列、planner Step 9 与命名指南、evaluator T8-I1..I8 与 T2/T5 集成方向、generator Step 2 集成引用、test-gen-evaluator G4 全部移除，后续条目重编号。
4. **文案清理与死模板删除**：`cli.ts` 1 处 + `workflow.ts` 3 处的 `(unit + integration)` 改写；删除 3 个零引用死模板；版本 bump + 重建产物。

全部改动为 Markdown 模板 / agent prompt 文案与 TS 字符串字面量，无逻辑、接口、数据结构变更。sociable 模型下真实组合仍然存在（被测模块内部 import 的协作模块不 mock），跨模块行为覆盖不丢失。

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| test-design 产物模板 | 定义单层 sociable 单元测试设计产物的章节与表格结构：3 列验收范围表、per-file 章节（待测功能/用例/Mock策略）、最小 mock 原则与组合用例挂靠注释 | `plugins/dev-team/templates/artifacts/test-design.md.template` | 无（被 4 个 agent 与 test-gen 引用） | Markdown 模板 |
| test-design-planner | 读取 proposal/design 生成 test-design.md；框架识别；组合用例挂靠链路入口模块 | `plugins/dev-team/agents/test-design-planner.md` | test-design.md.template、`test_detect_frameworks`、`test_resolve_paths` | Markdown agent prompt |
| test-design-evaluator | 静态清单评审 test-design.md；新增最小 mock 硬检查 | `plugins/dev-team/agents/test-design-evaluator.md` | test-design.md.template、`phase_log` | Markdown agent prompt |
| test-gen-generator | 依据 test-design.md 单元测试表格生成测试代码 | `plugins/dev-team/agents/test-gen-generator.md` | `test_detect_frameworks`、test-design.md.template | Markdown agent prompt |
| test-gen-evaluator | 评审生成测试代码；mock 声明合法性对照 Mock策略 表 | `plugins/dev-team/agents/test-gen-evaluator.md` | `workflow.json` files.written、`phase_log` | Markdown agent prompt |
| CLI 入口 | `test-execution` 子命令注册（命令名/选项/行为不变，仅描述文案） | `plugins/dev-team/bin/src/cli.ts` | cac | TypeScript |
| phase 表定义 | requirement / bug-fix / test-only 三种工作流的 test-execution phase prompt 文案 | `plugins/dev-team/bin/src/lib/workflow.ts` | 无新依赖 | TypeScript |
| 版本与产物打包 | 版本 bump 后经 `vp pack` 重建 dist 下三棵 dev-team 产物树 | `plugins/dev-team/package.json` | pnpm、vite-plus（vp pack） | pnpm scripts |

不变项：`test_resolve_paths` / `test_detect_frameworks` 工具及其逻辑零改动；phase 表结构、phase id、前置依赖不变（`workflow.test.ts` 既有「phase 表不含 integration-test」守卫保持）；`dev-design-*` / `code-analyze-planner` 中「design 不得含 test 章节」禁令文案不动；`bin/__tests__/` 与 `build/__tests__/` 存量自称「集成测试」的测试文件与注释不动。

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界；覆盖 proposal.md「变更范围-实现文件」全部 8 项。
     测试文件：预期零新增、零修改——全仓库未发现断言被改文案（`unit + integration`、被删模板文件名）的测试；既有守卫测试保持。 -->

### 新增文件

<!-- 无新增文件：本变更只修改既有模板 / agent prompt / TS 文案并删除死文件，不引入新文件。 -->

无。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/templates/artifacts/test-design.md.template` | 1) `## 验收范围` 注释删「`测试类型`：单元测试 或 集成测试」条目；表头 4 列改 3 列 `AC ID \| 验收条件 \| 被测文件或模块`，占位行删 `单元测试/集成测试` 枚举，被测列占位改 `{{test_file}}`（单值，承载用例的测试文件）；2) `## 单元测试` 章节注释写入组合用例挂靠规则；3) `#### Mock策略` 注释重写 `Mock主体` 定义为最小 mock 原则四要素；4) 整段删除 `## 集成测试` 章节（`### 关系标题 → 测试文件`、涉及模块表、关联AC、关系描述、场景、用例、Mock策略全部子结构）；5) `## 不可测试项` 不动 | AC-1、AC-2。per-file 章节结构（`#### 待测功能` / `#### 用例` / `#### Mock策略`）与 `#### 用例` 表列（`测试对象 \| 路径类型 \| 测试条件 \| 迭代类型`）不变，组合用例直接入该表 |
| `plugins/dev-team/agents/test-design-planner.md` | 1) 删 Step 9（识别跨模块交互并生成集成测试章节，含 a–e 全部子项与「`__tests__/`」放置指引）；2) Step 8 标题「集成测试框架识别」改「测试框架识别」，正文去「集成测试文件」表述（保留 `test_detect_frameworks` 调用，识别测试文件的扩展名、断言库和测试运行器）；3) 原 Step 11 增加组合用例挂靠指引；4) 原 Step 13 写入顺序删除「再逐关系写入 `## 集成测试` 的 per-relationship 章节」分段；5) 删除 Constraints 后「### 集成测试关系标题命名指南」整节；6) 步骤重编号：原 10→9（单元测试路径）、原 11→10（per-file 章节 + 挂靠指引）、原 12→11（errors → 不可测试项）、原 13→12（Write） | AC-3、AC-4。Step 11 末行「将每个 source 填写到验收范围表被测文件或模块列」保持（单值口径天然成立） |
| `plugins/dev-team/agents/test-design-evaluator.md` | 1) T2 改为仅单元测试方向：验收范围每个条目的 `被测文件或模块` 与 `## 单元测试` 的 `### <source> -> <test_file>` heading 双向无缺失；删除集成方向（关联AC 双向映射）表述；2) T5 章节清单改为「验收范围、单元测试、不可测试项（可选）」，删「集成测试（可选）」；3) 删除「#### 集成测试部分检查」整块（T8-I1..I8）；4) T8-G1 判据改为 3 列 `AC ID \| 验收条件 \| 被测文件或模块`（消除「保留 5 列却列 4 名」笔误）；5) 新增 T9 最小 mock 硬检查；6) Process step 4 的「T1-T8 with T8 sub-items」改「T1-T9 with T8 sub-items」 | AC-5、AC-6。T9 为独立单条检查项（无子项），T8 仍按原样在报告中合并为一条 |
| `plugins/dev-team/agents/test-gen-generator.md` | Step 2 表格解析去掉 `集成测试` 引用：仅解析 `单元测试 > 用例` 与 `单元测试 > Mock策略` 表格 | AC-7。其余步骤（框架检测、源码阅读、边界映射、生成约束）不动 |
| `plugins/dev-team/agents/test-gen-evaluator.md` | 1) 删 G4（`集成测试 > 用例` 骨架检查）行；2) 原 G5（mock 实现）收紧为 mock 合法性对照并成为新 G4；3) 原 G6..G9 重编号为 G5..G8；4) Process step 4 逐表对照仅剩 `单元测试 > 用例` 与 `Mock策略` 表格，删除集成测试表格对照段 | AC-8。G3（`单元测试 > 用例` 单表核对）不变，组合用例由其自然覆盖 |
| `plugins/dev-team/bin/src/cli.ts` | `test-execution` 命令描述 `'Run all automated tests (unit + integration) with coverage and generate execution report'` → `'Run all automated tests with coverage and generate execution report'`（原 19 行附近） | AC-10。命令名、选项、action 与执行路径不变 |
| `plugins/dev-team/bin/src/lib/workflow.ts` | 三处 test-execution phase prompt（原 114 / 188 / 281 行附近，分属 requirement / bug-fix / test-only 表）`'Run and fix all tests (unit + integration) for change "<change>".'` → `'Run and fix all tests for change "<change>".'` | AC-10。phase 表结构、phase id、pattern、executor/evaluator、前置依赖均不变 |
| `plugins/dev-team/package.json` | `version` 字段 `2.10.43` → `2.10.44`（patch bump） | AC-11。随后 `pnpm -C plugins/dev-team run build` 重建 dist 产物树（见删除文件注记） |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/templates/test-jest.js` | 死模板（jest 集成测试骨架），全仓库源码零引用，直接删除 |
| `plugins/dev-team/templates/test-pytest.py` | 死模板（pytest 集成测试骨架），同上 |
| `plugins/dev-team/templates/test-rust.rs` | 死模板（rust 集成测试骨架），同上 |

> 注：`dist/claude-plugins/dev-team/templates/`、`dist/cursor-plugins/dev-team/templates/`、`dist/cursor-home-image/dev-team/templates/`（及 `dist/cursor-home-image/dev-team/manifest.json` 中的清单条目）当前仍含这三个文件的拷贝，不手工删产物——由阶段五的 `pnpm -C plugins/dev-team run build` 重建后自然消除并做静态核对。

### 公共函数 / API

<!-- 仅 CLI 子命令描述文案变更，无函数签名、导出、HTTP 端点变更。 -->

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `dev-team test-execution` | `plugins/dev-team/bin/src/cli.ts` | 修改 | `dev-team test-execution [--change <name>] [--project-root <path>] [--files <files>] [--framework <name>] [--skip-mutation] [--force]` | 仅命令描述文案去 `(unit + integration)`；命令名、选项与行为不变 |

### 类型定义

<!-- 无类型变更：不新增/修改任何 interface、type alias、enum 或 class；phase 表内部结构（WorkflowPhase 类型字段）不变。 -->

无。

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `plugins/dev-team/package.json` | 修改 | `2.10.43` → `2.10.44`（semver string） | 按 CLAUDE.md 项目规则，改动插件源头后升版并重建产物 |

---

## 数据模型

无数据模型变更——本变更只改模板 / prompt 文案与字符串字面量，不触及任何持久化结构（`workflow.json`、eval 历史、测试报告 schema 均不动）。

---

<!-- 路由/API 设计：本变更不涉及 HTTP API，省略此节。CLI 子命令见「公共函数 / API」。 -->

## 依赖

### 运行时依赖

- 无新增。dev-team CLI 现有运行时依赖（cac、zod、picomatch、ignore、@likec4/language-services）不受本变更影响。

### 构建/测试依赖

- pnpm — workspace 包管理与 `run build` / `run check` 入口（既有）
- vite-plus（vp pack / vp check）— 产物打包与静态检查（既有）
- knip — 死代码报告（既有，`run check` 内置）

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 修改文件 #1：模板验收范围表改 3 列 `AC ID \| 验收条件 \| 被测文件或模块`、删「测试类型」列与 `## 集成测试` 整章；阶段一任务 |
| AC-2 | 修改文件 #1：`#### Mock策略` 注释写入四要素（进程边界白名单、入参例外、内部模块真实组合、组合用例挂靠链路入口模块）；`## 单元测试` 章节注释同步写挂靠规则 |
| AC-3 | 修改文件 #2：删 Step 9 与「集成测试关系标题命名指南」节；Step 8 改名「测试框架识别」；Step 13（新 12）写入顺序无 `## 集成测试` 分段 |
| AC-4 | 修改文件 #2：原 Step 11（新 10）写入挂靠指引——链路入口模块 colocate、describe 可写链路（如 `CLI参数 → workflow.json持久化`）、禁独立章节与 `__tests__/` 组合区 |
| AC-5 | 修改文件 #3：T2/T5 去集成方向、删 T8-I1..I8 整块、T8-G1 改 3 列判据 |
| AC-6 | 修改文件 #3：新增 T9——`Mock主体` 必须为进程边界依赖或被测 API 显式入参 / 注入依赖传入的内部模块；内部 wiring mock（如 `vi.mock(仓库内模块路径)`）→ fail |
| AC-7 | 修改文件 #4：generator Step 2 仅解析 `单元测试 > 用例` 与 `Mock策略` 表格，无 `集成测试` 引用 |
| AC-8 | 修改文件 #5：删 G4 并重编号（G5..G9 → G4..G8）、新 G4 含「模块 mock 声明必须对应 Mock策略 表合法条目，否则 fail」、Process step 4 仅剩单元测试表格对照 |
| AC-9 | 删除文件 3 项：源文件删除 + 全仓库源码无引用（已核实现有引用仅 dist 产物清单）；阶段四删除 + 阶段五重建后核对 dist 三棵树不再含这三文件 |
| AC-10 | 修改文件 #6/#7：cli.ts 描述与 workflow.ts ×3 prompt 按上表逐字改写；grep 范围为 `plugins/dev-team/` 源码与 dist 产物——`openspec/changes/archive/**` 与 `openspec/specs/**` 中的历史文本不在清理范围（proposal 决策「文案清理范围」：源头文案影响运行时输出，存量注释/归档不影响行为，规格经 delta 在 archive 阶段合并） |
| AC-11 | 修改文件 #8 + 阶段五：`package.json` version bump + `pnpm -C plugins/dev-team run build` 重建，核对 `dist/claude-plugins/dev-team/`、`dist/cursor-plugins/dev-team/`、`dist/cursor-home-image/dev-team/` 与源同步 |

## 待决问题

- 无——挂靠 / 验收范围 / mock 检查三项设计点与范围边界均已由提案定案。测试编写与执行由 test-design / test-gen / test-execution 阶段承接，本设计不含测试策略；守线仅保留静态检查。
