## MODIFIED Requirements

### Requirement: modules 为空时从 config 自动推导扫描目录

当 `resolveTestPaths` 收到空的 `modules` 数组时，函数 SHALL 调用 `runTestDetectFrameworks`（来自 `test-detect-frameworks.ts`）获取配置计划 `plan`，并按以下规则推导源文件扫描范围：

1. 不传 `files` 参数调用 `runTestDetectFrameworks({})`，触发 auto-scan 获取含完整 `plan` 的返回结果
2. 扫描根优先取各 suite 的 `root`（相对于 projectRoot）；若实现仍从 `plan[].directory` 扫描，则 MUST 保证覆盖 suite scope（`root` ∩ `includes` ∩ ¬`excludes`），不得仅因 `cwd` 上移而漏扫 `root` 子树或误扫到 root 外
3. 对每个扫描根递归遍历，收集可测试源文件（复用现有 `collectFiles` 排除规则：跳过 `node_modules`、`.git`、`dist`、`build`、`target` 等）
4. 对收集到的源文件应用现有单元测试路径推导规则（同非空 `modules` 的处理方式），并遵守 suite scope / `isFileExcluded`
5. 若 `plan` 为空数组（无 `tests` suite 配置），则在 `errors` 中添加指导性消息，提示在 `openspec/config.json` 中配置 `tests`

函数 SHALL 对来自多个目录的源文件进行去重（同一文件仅产生一份 `unit_tests` 条目）。

#### Scenario: 从 tests suite root 自动扫描源文件

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 包含 `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus" }]`
- **AND** `plugins/dev-team/bin/src/commands/` 目录下存在 `test-resolve-paths.ts` 文件
- **THEN** `unit_tests` 包含从该 suite 范围内扫描到的源文件的推导结果
- **AND** `errors` 为空数组

#### Scenario: 无 tests 配置时返回指导性错误

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 中 `tests` 为空（`plan` 为空）
- **THEN** `unit_tests` 为空数组
- **AND** `errors` 包含一条消息，指示用户在 `openspec/config.json` 中配置 `tests`

#### Scenario: 多个 suite 指向同一扫描树时去重

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 的 `tests` 中包含两个条目其 scope 覆盖同一源文件
- **AND** 该文件为 `plugins/dev-team/bin/src/foo.ts`
- **THEN** `unit_tests` 中 `foo.ts` 仅出现一次（去重）

### Requirement: modules 非空时基于 test config 过滤

当 `resolveTestPaths` 收到非空的 `modules` 数组时，函数 SHALL 调用 `runTestDetectFrameworks({ files: modules, projectRoot })` 获取文件→框架的检测结果。

1. 仅对 `detected` 数组中的文件推导测试路径（`detected` 中的文件已确认在某一 suite scope 内）
2. 未被 `detected` 的文件（不在任何 suite scope 内）SHALL 在 `errors` 中添加提示
3. `detected` 中每个文件的 `framework` 字段仅供参考，不影响测试路径推导逻辑（推导仍基于文件扩展名，见 `deriveUnitTestPath`）

#### Scenario: 仅返回 tests suite 覆盖范围内的文件

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/app.ts", "tools/deploy.ts"]`
- **AND** `tests` 仅覆盖 `src/`（`tools/` 不在任何 suite scope）
- **THEN** `runTestDetectFrameworks` 返回 `detected` 仅含 `src/app.ts`
- **AND** `unit_tests` 仅包含 `src/app.ts` 的推导结果
- **AND** `errors` 包含 `{path: "tools/deploy.ts", message: "Not in test config scope"}`（或等价文案）

#### Scenario: 所有文件都在 suite 范围内（全量通过）

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/a.ts", "src/b.ts"]`
- **AND** `tests` 覆盖 `src/` 目录
- **THEN** `detected` 包含两个文件
- **AND** `unit_tests` 包含两个文件的推导结果
- **AND** `errors` 为空（无过滤错误）

### Requirement: 非空 modules 模式增加 exclude 过滤

当 `resolveTestPaths` 收到非空 `modules` 数组时，`processNonEmptyModules` 函数 SHALL 在 "in test config scope" 检查通过之后、将源文件加入 `unitTestMap` 之前，应用 exclude 过滤。函数 SHALL 导入 `isFileExcluded` 并获取项目配置，对每个有效源文件调用 `isFileExcluded(posix, config)`；若返回 `true`，则跳过该文件——不添加 `unit_tests` 条目，不加入 `collectedSources`。

此阶段被排除的文件 SHALL NOT 产生 `errors` 条目（排除是主动行为，非错误）。

#### Scenario: 非空 modules 排除文件不出现在 unit_tests

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/app.ts", "src/generated/api.ts"]`
- **AND** 项目配置 `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
- **AND** 两文件均曾落入 includes
- **THEN** `unit_tests` 包含 `{source: "src/app.ts", test_file: "src/app.test.ts"}`
- **AND** `unit_tests` 不包含 source 为 `"src/generated/api.ts"` 的条目
- **AND** `errors` 不包含 `"src/generated/api.ts"`（排除是静默的）

#### Scenario: 非空 modules 模式下 suite-level excludes 同样生效

- **WHEN** `resolveTestPaths` 收到 `modules: ["plugins/dev-team/bin/src/app.ts", "plugins/dev-team/bin/vendor/lib.ts"]`
- **AND** 项目配置 `tests: [{ root: "plugins/dev-team/bin", framework: "vite-plus", excludes: ["vendor/**"] }]`
- **THEN** `unit_tests` 包含 `{source: "plugins/dev-team/bin/src/app.ts", ...}`
- **AND** `unit_tests` 不包含 `"plugins/dev-team/bin/vendor/lib.ts"` 的条目

### Requirement: 空 modules 自动扫描模式增加 exclude 过滤

当 `modules` 为空数组时，`processEmptyModules` 函数 SHALL 在 `isSourceFile` 检查通过之后、将文件加入 `sourceFiles` 集合之前应用 exclude 过滤。对每个发现的源文件，SHALL 调用 `isFileExcluded(posix, config)`；若返回 `true`，则跳过该文件。

#### Scenario: 空 modules 排除文件不出现在 unit_tests

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** 项目配置 `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
- **AND** 自动扫描发现 `src/app.ts`, `src/utils.ts`, `src/generated/api.ts`
- **THEN** `unit_tests` 包含 `"src/app.ts"` 和 `"src/utils.ts"` 的条目
- **AND** `unit_tests` 不包含 `"src/generated/api.ts"` 的条目

### Requirement: git-change 模式通过非空 modules 路径继承 exclude 过滤

当 `modules` 为 `"git-change"` 时，exclude 过滤 SHALL 通过与非空 modules 相同的路径自动生效——`resolveEffectiveModules` 将 git diff 输出转换为模块列表并委托给 `processNonEmptyModules`，因此 `processNonEmptyModules` 中的 exclude 过滤逻辑 SHALL 自动适用。

#### Scenario: git-change 模式遵守 tests[].excludes

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** 项目配置 `tests: [{ root: "src", framework: "vitest", excludes: ["generated/**"] }]`
- **AND** `git diff HEAD --name-only` 返回 `["src/app.ts", "src/generated/api.ts"]`
- **THEN** `unit_tests` 仅包含 `{source: "src/app.ts", test_file: "src/app.test.ts"}`
- **AND** `unit_tests` 不包含 `"src/generated/api.ts"` 的条目

### Requirement: 未配置 exclude 时保持向后兼容

当未配置任何 `tests[].excludes` 时，三种解析模式（非空 modules、空 modules、git-change）在「给定等价 suite 覆盖范围」前提下，SHALL 与变更前「无 exclude」行为一致（除配置键从 `test`/`overrides` 迁到 `tests` 外）。

#### Scenario: 非空 modules 未配置 excludes 时行为稳定

- **WHEN** 项目配置 `tests: [{ root: "src", framework: "vitest" }]` 且无 `excludes`
- **AND** `resolveTestPaths` 收到 `modules: ["src/app.ts", "src/utils.ts"]`
- **AND** 两文件均在 suite scope 内
- **THEN** `unit_tests` 包含两个条目的结果

#### Scenario: 空 modules 未配置 excludes 时行为稳定

- **WHEN** 项目配置无任何 `tests[].excludes`
- **AND** `resolveTestPaths` 收到 `modules: []`
- **THEN** 自动扫描仅受 suite `root`/`includes` 约束，行为确定且可测

#### Scenario: git-change 模式未配置 excludes 时行为稳定

- **WHEN** 项目配置无任何 `tests[].excludes`
- **AND** `resolveTestPaths` 收到 `modules: "git-change"`
- **THEN** git diff 结果仍按 suite scope 过滤，无额外静默排除

---

## ADDED Requirements

### Requirement: 用户提示文案改为 tests

所有由 `test-resolve-paths` 发出的「缺少测试配置」类错误消息 SHALL 引导用户配置 `openspec/config.json` 的 `tests` 数组，MUST NOT 再引用 `test.framework` 或 `test.overrides`。

#### Scenario: 空 plan 错误提到 tests

- **WHEN** `resolveTestPaths` 收到 `modules: []` 且 `tests` 为空
- **THEN** `errors[0].message` SHALL mention `tests`

---

## Module Contract

### Function: resolveTestPaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Config dependency** | `config.tests` via `runTestDetectFrameworks` + `isFileExcluded` |
| **Empty modules** | Scan suite roots / plan directories；无 suite 时 errors 提示配置 `tests` |
| **Exclude** | `tests[].excludes` via `isFileExcluded` |
