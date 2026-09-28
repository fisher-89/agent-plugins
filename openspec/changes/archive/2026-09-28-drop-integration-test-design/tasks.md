# 任务: drop-integration-test-design

> 实现边界以 `design.md`「变更清单」为准。测试编写与执行由 test-design / test-gen / test-execution 阶段承接，本任务列表不含任何测试任务。

## 阶段一：test-design 模板单层化

- [x] `plugins/dev-team/templates/artifacts/test-design.md.template`：`## 验收范围` 改 3 列——表头 `AC ID | 验收条件 | 被测文件或模块`，注释删「`测试类型`：单元测试 或 集成测试」条目、被测列说明改为「承载用例的测试文件路径（单值）」，占位行删 `单元测试/集成测试` 枚举、被测列占位改 `{{test_file}}`
- [x] 同文件：`## 单元测试` 章节注释写入组合用例挂靠规则——跨模块组合用例 colocate 到链路入口模块（链路发起方 / 最上层调用方）的 `#### 用例` 表，describe 标题可写链路（如 `CLI参数 → workflow.json持久化`），不设独立集成测试章节、不设 `__tests__/` 组合测试区
- [x] 同文件：`#### Mock策略` 注释重写 `Mock主体` 定义为最小 mock 原则四要素——进程边界依赖白名单（数据文件 / 配置 / DB / 接口 / 网络 / 子进程 / 全局变量 / 运行环境）、被测 API 显式入参 / 注入依赖传入的内部模块例外（入参例外）、其余内部模块间调用必须真实组合、组合用例挂靠链路入口模块且同样遵守本约束
- [x] 同文件：整段删除 `## 集成测试` 章节（`### 关系标题 → 测试文件`、涉及模块表、关联AC、关系描述、场景、`##### 用例`、`##### Mock策略` 全部子结构）；`## 不可测试项` 保持不动

## 阶段二：test-design / test-gen 四 agent 文件（依赖阶段一的模板 3 列口径）

- [x] `plugins/dev-team/agents/test-design-planner.md`：删 Step 9（识别跨模块交互并生成集成测试章节，含 a–e 子项与「集成测试文件放置在 `__tests__/`」指引）与「### 集成测试关系标题命名指南」整节；Step 8 标题改「测试框架识别」，正文去「集成测试文件」表述、保留 `test_detect_frameworks` 调用；后续步骤重编号（原 10→9、原 11→10、原 12→11、原 13→12）
- [x] 同文件：新 Step 10（原 11）增加组合用例挂靠指引——为跨模块链路选承载文件时按链路发起方 / 最上层调用方确定入口模块，用例写入其 per-file 章节 `#### 用例` 表，describe 标题可写链路，MUST NOT 指引独立集成章节或 `__tests__/` 组合测试区
- [x] 同文件：新 Step 12（原 13）写入顺序删除「再逐关系写入 `## 集成测试` 的 per-relationship 章节」分段，保留「验收范围 → 单元测试逐文件 → 不可测试项」顺序
- [x] `plugins/dev-team/agents/test-design-evaluator.md`：T2 判断依据改仅单元测试方向（验收范围条目 ↔ `### <source> -> <test_file>` heading 双向无缺失，删关联AC 集成映射）；T5 章节清单改「验收范围、单元测试、不可测试项（可选）」；删「#### 集成测试部分检查」整块（T8-I1..I8）；T8-G1 判据改 3 列 `AC ID | 验收条件 | 被测文件或模块`
- [x] `plugins/dev-team/agents/test-design-evaluator.md`：新增 T9 最小 mock 硬检查——`Mock策略` 表 `Mock主体` 必须为进程边界依赖或被测 API 显式入参 / 注入依赖传入的内部模块；被测模块内部 import 协作模块的 mock（如 `vi.mock(仓库内模块路径)`）→ fail，evidence 指引改为组合真实模块；Process step 4 的清单范围同步为 T1-T9 with T8 sub-items
- [x] `plugins/dev-team/agents/test-gen-generator.md`：Step 2 表格解析仅保留 `单元测试 > 用例` 与 `单元测试 > Mock策略` 两表，删除全部 `集成测试` 引用
- [x] `plugins/dev-team/agents/test-gen-evaluator.md`：删 G4 行；原 G5 收紧为 mock 合法性对照并占位新 G4——逐行对照 `单元测试 > Mock策略`（Mock主体有对应 mock 声明、Mock方案与实现一致、应用场景 describe 正确应用），另验证生成代码中的模块 mock 声明必须对应 Mock策略 表合法条目、无对应条目的内部模块 mock → fail；原 G6..G9 重编号为 G5..G8；Process step 4 删除集成测试表格对照段

## 阶段三：CLI 与 phase prompt 文案（与阶段二无依赖，可并行）

- [x] `plugins/dev-team/bin/src/cli.ts`：`test-execution` 命令描述改为 `Run all automated tests with coverage and generate execution report`（命令名、选项、action 不动）
- [x] `plugins/dev-team/bin/src/lib/workflow.ts`：requirement / bug-fix / test-only 三处 test-execution phase prompt（原 114 / 188 / 281 行附近）改为 `Run and fix all tests for change "<change>".`（phase 表结构、id、pattern、executor/evaluator、前置依赖均不动）

## 阶段四：死模板删除（须先于阶段五重建）

- [x] 删除 `plugins/dev-team/templates/test-jest.js`、`plugins/dev-team/templates/test-pytest.py`、`plugins/dev-team/templates/test-rust.rs`

## 阶段五：版本与产物重建 + 静态守线（静态，不含测试执行）

- [x] `plugins/dev-team/package.json`：`version` 由 `2.10.43` bump 至 `2.10.44`
- [x] 运行 `pnpm -C plugins/dev-team run build` 重建产物；静态核对 `dist/claude-plugins/dev-team/`、`dist/cursor-plugins/dev-team/`、`dist/cursor-home-image/dev-team/` 三棵树与源同步：`templates/` 下不再含 `test-jest.js` / `test-pytest.py` / `test-rust.rs`，`manifest.json` 清单同步，agent / 模板文件为改写后版本
- [x] 静态边界检查：grep `plugins/dev-team/` 源码与 dist 产物树无 `unit + integration`；6 个目标文件（模板 + 4 agent + cli/workflow）无 `集成测试` / `集成测试框架` 残留（`bin/__tests__/`、`build/__tests__/` 存量自称「集成测试」的文件与注释按范围决策不动，不在检查范围；`openspec/` 归档与既有规格不在清理范围）
- [x] 运行 `pnpm -C plugins/dev-team run check`（vp check + knip）通过；既有守卫测试（phase 表不含 integration-test 等）不修改——测试执行统一由 test-execution 阶段承接，本阶段不运行测试
