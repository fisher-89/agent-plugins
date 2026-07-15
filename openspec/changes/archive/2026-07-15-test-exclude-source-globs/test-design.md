# 测试设计: test-exclude-source-globs

> **日期**: 2026-07-14

---

## 验收范围

<!-- 用模板表格逐条映射 proposal.md 的每个 AC：
  - `AC ID`：proposal 中的`验收标准`编号
  - `验收条件`：原文摘录
  - `测试类型`：`单元测试` 或 `集成测试`
  - `测试文件`：目标测试文件路径
  - `测试对象/测试场景`：对应的 describe/it 标题，单元测试中是`测试对象`，集成测试中是`测试场景` -->

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | config-schema 支持 test.exclude | 单元测试 | `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) |
| AC-2 | 共享 exclude 过滤函数 — isFileExcluded | 单元测试 | `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded |
| AC-2 | 共享 exclude 过滤函数 — getExcludeGlobs | 单元测试 | `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | getExcludeGlobs |
| AC-3 | test-detect-frameworks 排除 | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | detectFrameworks — exclude 过滤 (AC-3) |
| AC-4 | test-resolve-paths 排除 | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) |
| AC-5 | test-runner 突变排除 | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — mutation exclude 过滤 (AC-5) |
| AC-6 | 向后兼容 — isFileExcluded | 单元测试 | `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded — 向后兼容 (AC-6) |
| AC-6 | 向后兼容 — test-detect-frameworks | 单元测试 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 向后兼容 (AC-6) |
| AC-6 | 向后兼容 — test-resolve-paths | 单元测试 | `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — 向后兼容 (AC-6) |
| AC-6 | 向后兼容 — test-runner | 单元测试 | `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — 向后兼容 (AC-6) |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景。本变更全部为进程内逻辑（Zod schema 验证、纯函数过滤、mock 模块方法），无需集成测试。 -->

### 用例

<!-- - `测试文件`：文件路径
   - `测试对象`：describe 标题，与验收范围中describe_title一致
   - `路径类型`：`正向`（happy path）、`异常`（error/invalid input）、`边界`（edge case）
   - `测试条件`：it 标题（具体输入与预期输出）
   - `迭代类型`：`新增` 或 `废弃`

   正向场景对应 proposal 的每个 AC，反向场景覆盖错误处理与无效输入，边界场景按下方参数类型映射表系统化生成。 -->

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 正向 | 文件匹配全局 exclude glob 时返回 true | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 正向 | 文件匹配 override-level exclude glob 时返回 true | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 正向 | 文件同时匹配全局和 override exclude 时返回 true | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 正向 | 文件不匹配任何 exclude glob 时返回 false | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 正向 | override-level exclude 仅在该 override file 范围内生效 | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 边界 | exclude 数组为空时返回 false | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 边界 | exclude 未配置（undefined）时返回 false | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 边界 | 路径含特殊字符（空格、Unicode）时 glob 匹配正确 | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 边界 | Windows 反斜杠路径在 glob 匹配前被转换为 POSIX 格式 | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | isFileExcluded | 边界 | 超长路径字符串不应导致异常 | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | getExcludeGlobs | 正向 | 仅全局 exclude 时返回全局列表 | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | getExcludeGlobs | 正向 | 合并全局和 override-level 的 exclude globs | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | getExcludeGlobs | 边界 | 全局和 override 存在重复 glob 时去重 | 新增 |
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | getExcludeGlobs | 边界 | 无任何 exclude 时返回空数组 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 正向 | test.exclude 为合法字符串数组时验证通过 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 正向 | test.overrides[].exclude 为合法字符串数组时验证通过 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 正向 | 同时配置 test.exclude 和 test.overrides[].exclude 时验证通过 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 正向 | test.exclude 省略时验证通过（可选字段） | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 异常 | test.exclude 为字符串而非数组时验证失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 异常 | test.exclude 元素为非字符串（如数字）时验证失败 | 新增 |
| `plugins/dev-team/bin/src/schemas/config/config.schema.test.ts` | configSchema — test.exclude 验证 (AC-1) | 边界 | test.exclude 为空数组 [] 时验证通过（不排除任何文件） | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | detectFrameworks — exclude 过滤 (AC-3) | 正向 | 配置 test.exclude 后被排除的文件不出现在 detected[] 中 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | detectFrameworks — exclude 过滤 (AC-3) | 正向 | 未被排除的文件正常出现在 detected[] 中 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | detectFrameworks — exclude 过滤 (AC-3) | 正向 | override-level exclude 仅在该 override 文件范围内生效 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | detectFrameworks — exclude 过滤 (AC-3) | 正向 | 多框架配置下 exclude 过滤正确作用于各框架 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | detectFrameworks — exclude 过滤 (AC-3) | 边界 | exclude 配置不影响非匹配框架的文件检测 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 向后兼容 (AC-6) | 正向 | 不配置 exclude 时全部现有功能行为不变 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 向后兼容 (AC-6) | 边界 | 配置中存在 `"exclude": []` 空数组时检测结果与无 exclude 配置时一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 向后兼容 (AC-6) | 边界 | 配置中 `test` 节包含 `exclude` 字段但其值为 undefined 时检测结果不受影响 | 新增 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | runTestDetectFrameworks — 向后兼容 (AC-6) | 边界 | 配置中不存在 `test` 节时检测结果与无 exclude 配置时一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) | 正向 | 配置 test.exclude 后被排除的文件不出现在 unit_tests[] 中 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) | 正向 | 未被排除的文件正常出现在 unit_tests[] 中 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) | 正向 | 空 modules 自动扫描模式下 exclude 过滤生效 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) | 正向 | 混合排除/未排除文件时仅未排除文件进入 unit_tests | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) | 边界 | 既不配置全局 exclude 也不配置 override exclude 时全部源文件正常进入 unit_tests | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — exclude 过滤 (AC-4) | 边界 | override-level exclude 不扩展到其他 override 区域 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — 向后兼容 (AC-6) | 正向 | 不配置 exclude 时全部现有功能行为不变 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — 向后兼容 (AC-6) | 边界 | 配置中存在 `"exclude": []` 空数组时路径解析结果与无 exclude 配置时一致 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — 向后兼容 (AC-6) | 边界 | 配置中 `test` 节包含 `exclude` 字段但其值为 undefined 时路径解析结果不受影响 | 新增 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | runTestResolvePaths — 向后兼容 (AC-6) | 边界 | 配置中不存在 `test` 节时路径解析结果与无 exclude 配置时一致 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — mutation exclude 过滤 (AC-5) | 正向 | 配置 exclude 后被排除的源文件不进入 StrykerJS 变异目标列表 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — mutation exclude 过滤 (AC-5) | 正向 | 未配置 exclude 时全部源文件正常进入变异目标列表 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — mutation exclude 过滤 (AC-5) | 异常 | exclude 配置下 mutation 阶段源文件列表为空时静默跳过 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — 向后兼容 (AC-6) | 正向 | 不配置 exclude 时全部现有功能行为不变 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — 向后兼容 (AC-6) | 边界 | 配置中存在 `"exclude": []` 空数组时变异测试源文件列表与无 exclude 配置时一致 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — 向后兼容 (AC-6) | 边界 | 配置中 `test` 节包含 `exclude` 字段但其值为 undefined 时变异测试源文件列表不受影响 | 新增 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | executePlanEntry — 向后兼容 (AC-6) | 边界 | 配置中不存在 `test` 节时变异测试源文件列表与无 exclude 配置时一致 | 新增 |

### 参数类型 → 边界场景映射

<!-- 本变更涉及的主要参数类型及其边界场景覆盖情况：

| 类型 | 边界场景 | 最少数 | 覆盖情况 |
|------|---------|--------|----------|
| `string[]` (exclude) | `[]`, `["single"]`, `["large", "list"]`, `undefined` | 4 edge + 1 normal | 已覆盖：空数组、undefined、单元素、多元素、重复去重 |
| `string` (filePath) | `""`, 超长(>1000), 特殊字符(空格 Unicode), Windows反斜杠 | 4 edge + 1 normal | 已覆盖：超长路径、特殊字符、Windows路径（空字符串路径极端场景可忽略） |
| `boolean` (返回值) | `true`, `false` | 2 | 已覆盖 |
| `Optional[string[]]` (config字段) | `undefined`, `[]`, 正常数组 | 依据 Optional 合并 | 已覆盖 |
-->

### Mock策略

<!-- - `测试文件`：文件路径
   - `Mock主体`：系统源代码之外的依赖（数据文件、配置文件、DB、接口、全局变量、运行环境）
   - `Mock方案`：具体 mock 的路径、输入输出、转换逻辑
   - `应用场景`：哪些 describe 需要此 mock -->

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/src/lib/test-exclude.test.ts` | `picomatch` (通过 `lib/glob.ts`) | 不 mock。直接调用 `matchGlob` 验证集成，依赖 picomatch 的 glob 匹配由 `glob.test.ts` 覆盖 | 所有 isFileExcluded/getExcludeGlobs 测试 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | `openspec/config.json` 文件系统 | 使用 `createTempProject` 创建临时目录写入配置，测试结束后 `cleanup()` 删除 | detectFrameworks — exclude 过滤 (AC-3) |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `runTestDetectFrameworks` (通过 vi.mock) | `vi.mock('./test-detect-frameworks')` 在模块初始化时 mock 整个模块，默认 pass-through。需要模拟 exclude 过滤结果时通过 `mockImplementation` 覆盖返回值 | runTestResolvePaths — exclude 过滤 (AC-4) |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | `openspec/config.json` 文件系统 | 使用 `createTempProject` 创建临时目录 + `writeFile` 写入源文件。在自动扫描场景中通过 `mockImplementation` 控制 `runTestDetectFrameworks` 返回的 plan 内容 | runTestResolvePaths — exclude 过滤 (AC-4)，空 modules 自动扫描模式 |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `child_process.execSync` (通过 vi.mock) | `vi.mock('child_process')` 在模块初始化时 mock `execSync`，默认 `mockExecSync` 返回 JSON 测试结果。需要验证 exclude 过滤时通过 `mockImplementation` 控制返回值 | executePlanEntry — mutation exclude 过滤 (AC-5) |
| `plugins/dev-team/bin/src/lib/test-runner.test.ts` | `runMutationPhase` 的 `isFileExcluded` 调用 | 在 mutation 相关测试中，mock `execSync` 返回测试执行结果后再返回 StrykerJS 结果。exclude 过滤通过控制传递给 `runMutationPhase` 的 `sourceFiles` 数组间接验证 | executePlanEntry — mutation exclude 过滤 (AC-5) |

---

## 集成测试

<!-- 本变更不涉及跨进程/跨模块边界的集成测试场景。所有验收标准均可通过单元测试在进程内模拟配置文件和 mock 依赖完成验证。 -->

本变更无集成测试。所有 AC 均通过单元测试覆盖：

- **AC-1**: Zod schema 的 `safeParse` 在进程内验证 `test.exclude` 字段，无需跨进程依赖
- **AC-2**: `isFileExcluded` / `getExcludeGlobs` 为纯函数，输入 config 对象，输出 boolean / string[]
- **AC-3, AC-4, AC-5**: 被修改的消费端函数 (`detectFrameworksForFiles`, `processNonEmptyModules`, `processEmptyModules`, `runMutationPhase`) 均通过 mock 文件系统和 mock 子进程在单元测试中验证
- **AC-6**: 向后兼容通过不配置 exclude 运行现有测试验证

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因。 -->

- `plugins/dev-team/bin/dev-team-config.schema.json` (JSON Schema 文件) — **原因**: JSON Schema 文件是静态文档，用于 IDE 自动补全，不参与运行时逻辑。Zod schema (`config.schema.ts`) 已经通过 `config.schema.test.ts` 验证了 `test.exclude` 和 `test.overrides[].exclude` 字段的结构正确性。JSON Schema 文件的格式正确性可通过 JSON Schema 验证工具手动验证，不属于自动化测试范畴。
- `plugins/dev-team/bin/dev-team-config.schema.json` 中 `test.exclude` 和 `test.overrides[].exclude` 的 JSON Schema 定义与 Zod schema 的一致性 — **原因**: 两套 schema 定义在各自独立的文件中，没有共享代码。自动化测试只能分别验证 Zod schema（运行时生效）和 JSON Schema（IDE 辅助），无法自动断言两者字段定义完全同步。这属于人工 code review 范围。
