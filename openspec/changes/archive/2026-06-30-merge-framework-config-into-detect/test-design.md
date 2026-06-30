# 测试设计: merge-framework-config-into-detect

> **日期**: 2026-06-29

---

## 验收范围

<!-- 8 条 AC 来自 proposal.md，逐条映射到测试类型 -->

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | `lib/test-framework.ts` 包含完整的 8 框架注册表（jest, vitest, vite-plus, bun, rust, node-test, go, pytest），所有字段正确 | 单元测试 | `lib/test-framework.test.ts` | getFrameworkConfig - 八框架完整性 |
| AC-2 | `test-detect-frameworks.ts` 从 `../lib/test-framework` 导入 `getFrameworkConfig` 和 `getDefaultGlobForFramework` | 单元测试 | `test-detect-frameworks.test.ts` | 全部现有测试（import 路径更新后全部通过即验证） |
| AC-3 | `mcp.ts` 中无 `registerTestGetFrameworkConfigTool` 调用，无 `runTestGetFrameworkConfig` 导入 | 不可测试 | — | — |
| AC-4 | `schemas/index.ts` 不 export `testGetFrameworkConfigInputSchema`/`testGetFrameworkConfigOutputSchema` | 不可测试 | — | — |
| AC-5 | `commands/test-get-framework-config.ts` 和 `schemas/test-get-framework-config.schema.ts` 从仓库中删除 | 不可测试 | — | — |
| AC-6 | `test-gen-generator.md` 的 "Framework config resolution" 步骤改为仅使用 `test_detect_frameworks` 返回的框架名，无 `test_get_framework_config` 调用 | 不可测试 | — | — |
| AC-7 | `test-detect-frameworks.test.ts` 全部测试用例通过，无 import 错误 | 单元测试 | `test-detect-frameworks.test.ts` | 全部现有 describe |
| AC-8 | `lib/test-framework.test.ts` 验证 8 框架的 `coverage_cmd`、`coverage_format`、`coverage_output`、`coverage_artifacts`、`coverage_cleanup`、`default_glob` 均正确 | 单元测试 | `lib/test-framework.test.ts` | getFrameworkConfig - 八框架字段验证 |

---

## 单元测试

<!-- 覆盖 lib/test-framework.ts（新增）、test-detect-frameworks.test.ts（修改 import/函数名） -->

### 用例

#### 新增测试文件: `lib/test-framework.test.ts`

> 从 `commands/test-get-framework-config.test.ts` 迁移并适配以下变更：
> - import 路径改为 `'../lib/test-framework'`
> - `runTestGetFrameworkConfig({framework: x})` 调用改为 `getFrameworkConfig(x)`
> - 签名从 options-object 简化为直接字符串参数 `getFrameworkConfig(framework: string)`
>
> 迁移后新增对简化签名的边界测试。

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 八框架完整性 | 正向 | 遍历八个框架（jest, vitest, vite-plus, bun, rust, node-test, go, pytest），每个返回非空 `test_cmd`、`coverage_cmd`、`coverage_artifacts`、`coverage_cleanup`、`default_glob` | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 八框架完整性 | 正向 | 八个框架各自返回完整 `FrameworkConfig` 对象，包含全部 8 个字段 | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 八框架完整性 | 正向 | `coverage_format` 值正确: istanbul（jest/vitest/vite-plus/bun）、llvm-cov（rust）、node-test（node-test）、go-cover（go）、coverage-py（pytest） | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 八框架完整性 | 正向 | 每个框架的 `coverage_artifacts` 为非空字符串数组，每个元素为非空字符串 | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 八框架完整性 | 正向 | 每个框架的 `coverage_output` 为 Truthy 值 | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 八框架完整性 | 正向 | 返回对象的 keys 包含全部 8 个字段（framework, test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup, default_glob） | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | jest 返回正确的所有字段 | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | vitest 返回正确的 coverage_artifacts 和 coverage_cleanup | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | rust 返回正确的 coverage_artifacts 和 coverage_cleanup | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | node-test 的 coverage_cmd 为 `"node --test --experimental-test-coverage"`（不含 tee/脚本管道） | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | node-test 的 coverage_output 为 `"coverage/node-test-output.txt"` | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | node-test 的 coverage_artifacts 为 `["coverage/node-test-output.txt"]` | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | go 的 coverage_format 为 `"go-cover"` | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | pytest 的 coverage_format 为 `"coverage-py"` | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 已知框架 | 正向 | 各框架 coverage_cmd 均含 JSON reporter 参数，coverage_artifacts 均为单元素 JSON 摘要路径（jest/vitest/vite-plus/bun/rust） | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - JSON-only 配置 | 正向 | jest 返回 JSON-only coverage_cmd 与单元素 coverage_artifacts | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - JSON-only 配置 | 正向 | vitest 返回含 `--coverage.reporter=json-summary` 的 coverage_cmd | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 未知框架 | 异常 | 未知框架名 `"unknown-framework"` 抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 未知框架 | 异常 | 未知框架名 `"mocha"` 抛出 Error，错误信息列出全部八个框架名 | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | 空字符串框架名 `""` 抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | `null` 框架名抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | `undefined` 框架名抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | 前后带空格的 `"  vitest  "` 抛出 Error（无 trim） | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | 大小写敏感：`"Vitest"` 抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | 含特殊正则字符 `"jest?"` 抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getFrameworkConfig` - 边界值 | 边界 | 超长字符串 `"x".repeat(1001)` 抛出 Error | 新增 |
| `lib/test-framework.test.ts` | `getDefaultGlobForFramework` - 已知框架 | 正向 | jest/vitest/vite-plus/bun 返回 `**/*.{test,spec}.{js,ts,jsx,tsx}` | 新增 |
| `lib/test-framework.test.ts` | `getDefaultGlobForFramework` - 已知框架 | 正向 | rust 返回 `**/tests/**/*.rs` | 新增 |
| `lib/test-framework.test.ts` | `getDefaultGlobForFramework` - 已知框架 | 正向 | node-test 返回 `**/*.test.{mjs,js,cjs}` | 新增 |
| `lib/test-framework.test.ts` | `getDefaultGlobForFramework` - 已知框架 | 正向 | go 返回 `**/*_test.go` | 新增 |
| `lib/test-framework.test.ts` | `getDefaultGlobForFramework` - 已知框架 | 正向 | pytest 返回 `**/test_*.py` | 新增 |
| `lib/test-framework.test.ts` | `getDefaultGlobForFramework` - 边界值 | 异常 | 未知框架名 `"unknown"` 抛出 Error | 新增 |

#### 修改测试文件: `commands/test-detect-frameworks.test.ts`

> 以下变更：
> - import 路径从 `'./test-get-framework-config'` 改为 `'../lib/test-framework'`
> - `runTestGetFrameworkConfig({framework: x})` 调用改为 `getFrameworkConfig(x)`
>
> 全部现有 describe/it 保持不变，仅修改 import 和函数调用签名。

| 测试文件 | 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|---------|----------|----------|----------|
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 全局第一匹配 (AC-4) | 正向 | 文件按默认+override 匹配框架 | 废弃（import 更新后继续运行） |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 全局第一匹配 (AC-4) | 边界 | 文件同时匹配默认和 override 全局时优先匹配第一条 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 反向 | 异常 | 无匹配文件返回 `"unknown"` | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 反向 | 边界 | 空文件列表返回空 detected | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 单框架 | 正向 | 默认配置检测框架并匹配文件 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 单框架 | 正向 | 框架默认全局匹配生效 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 单框架 | 边界 | 无匹配文件时应有结果 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 边界 | 边界 | files 参数为 undefined（自动扫描） | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 边界 | 边界 | config 无 framework | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 边界 | 边界 | 超大文件列表（1000 文件）不报错 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 边界 | 边界 | 文件路径含特殊字符 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 边界 | 边界 | config 完全缺失 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan 内容 | 正向 | 单框架配置生成正确 plan 条目 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan 内容 | 正向 | 多框架多目录配置生成正确 plan 数组 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 单框架 plan | 正向 | vitest 生成 plan，directory 为 "." | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 单框架 plan | 正向 | jest 生成 plan，directory 为 "." | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 单框架 plan | 正向 | rust 生成 plan，directory 为 "."，coverage_format 为 "llvm-cov" | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 空配置 plan | 边界 | 无 framework 配置返回空 plan | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 空配置 plan | 边界 | 无 test 配置返回空 plan | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 向后兼容 | 正向 | detected 字段结构不变 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 向后兼容 | 正向 | frameworks 字段结构不变 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 向后兼容 | 正向 | 首匹配规则不变 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 向后兼容 | 正向 | 无匹配文件返回 "unknown" | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan 覆盖率产物字段 | 正向 | vitest 框架 plan 含 coverage_artifacts 和 coverage_cleanup | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan 覆盖率产物字段 | 正向 | vitest artifacts/cleanup 内容正确 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan 覆盖率产物字段 | 正向 | 多框架 [vitest, rust] 每个条目含对应字段 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan 覆盖率产物字段 | 正向 | rust artifacts/cleanup 内容正确 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan script | 正向 | vitest plan script 非空 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan script | 正向 | 多框架 [vitest, rust] 每个条目 script 非空 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan script | 正向 | script 由 directory/coverage_cmd/coverage_cleanup 正确组装 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan script | 正向 | vite-plus（directory 非 "."）script 含 cd 行 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - plan script | 边界 | 空 framework 配置 plan 为空数组 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - JSON-only 传播 (AC-9) | 正向 | vitest plan coverage_cmd 等于注册表 JSON-only 命令 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - JSON-only 传播 | 正向 | vitest plan coverage_artifacts 为 `["coverage/coverage-summary.json"]` | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - JSON-only 传播 | 正向 | rust plan coverage_cmd 为 `"cargo llvm-cov --json"` | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - JSON-only 传播 | 正向 | 多框架每个 plan 条目与注册表一致 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - go plan (AC-5) | 正向 | go 框架 plan 含 coverage_format: "go-cover" | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - node-test plan (AC-5) | 正向 | node-test 框架 coverage_cmd 为 `"node --test --experimental-test-coverage"` | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - pytest plan (AC-5) | 正向 | pytest 框架 coverage_format 为 "coverage-py" | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - 多框架 (AC-5) | 正向 | [vitest, go] 两条目各自覆盖格式正确 | 废弃 |
| `test-detect-frameworks.test.ts` | `runTestDetectFrameworks` - glob 检测 (AC-5) | 正向 | `*_test.go` 在 go 配置下映射为 go | 废弃 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `test-detect-frameworks.test.ts` | 文件系统（`openspec/config.json`） | 在临时目录中创建 `openspec/config.json`，通过 `projectRoot` 指向该临时目录 | 全部 describe（createTempProject 辅助函数） |
| `test-detect-frameworks.test.ts` | 临时目录生命周期 | `fs.mkdtempSync + fs.rmSync` try/finally 确保清理 | 全部 describe |

> `lib/test-framework.ts` 是纯函数数据模块（无文件系统/网络/环境依赖），无需 mock。

---

## 集成测试

<!-- 变更范围内的集成测试仅涉及 test_detect_frameworks_plan_json_only_coverage 的 import 路径更新 -->

### 用例

| AC ID | 测试文件 | 测试场景 | 测试条件 | 迭代类型 |
|--------|---------|---------|----------|----------|
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | plan JSON-only 与注册表一致 | 五个框架（jest, vitest, vite-plus, bun, rust）plan[0].coverage_cmd/coverage_artifacts 与 `getFrameworkConfig()` 一致 | 废弃（import 路径更新） |
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | plan JSON-only 与注册表一致 | rust plan coverage_cleanup 为 `["coverage", "target/llvm-cov"]` | 废弃 |
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | generateScript 末行 | vitest plan script 末行含 `--coverage.reporter=json-summary` | 废弃 |
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | generateScript 末行 | jest plan script 末行含 `--coverageReporters=json-summary` | 废弃 |
| AC-8 | `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | generateScript 末行 | plan script 末行等于 JSON-only coverage_cmd | 废弃 |

### Mock策略

| 测试文件 | Mock主体 | Mock方案 | 应用场景 |
|---------|---------|----------|----------|
| `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | 文件系统（`openspec/config.json`） | 在临时目录中创建 `openspec/config.json`，通过 `projectRoot` 指向该临时目录 | 全部测试用例（createTempProject 辅助函数） |

---

## 不可测试项

下列 AC 无法通过自动化测试验证，原因如下：

- **AC-3**（`test_get_framework_config` 从 MCP 中移除） — **原因**: MCP 工具注册代码在 `main()` 中的调用移除是结构性变更，通过 TypeScript 编译检查验证（若残留引用则编译报错）。建议在 code review 中确认 `registerTestGetFrameworkConfigTool` 调用和 `runTestGetFrameworkConfig` 导入已移除。
- **AC-4**（`test_get_framework_config` schema 移除） — **原因**: `schemas/index.ts` 的 export 移除是结构性变更，通过 TypeScript 编译检查验证。建议在 code review 中确认 `testGetFrameworkConfigInputSchema`/`testGetFrameworkConfigOutputSchema` 的 export 已移除。
- **AC-5**（源文件 `commands/test-get-framework-config.ts` 和 `schemas/test-get-framework-config.schema.ts` 删除） — **原因**: 文件删除无法通过运行时测试验证。建议在 code review 阶段通过 `git diff` 或文件存在性检查确认。
- **AC-6**（`test-gen-generator.md` 停止调用 `test_get_framework_config`） — **原因**: Markdown 代理文档的指令更改无法通过自动化测试验证。建议在 code review 中确认 `test-gen-generator.md` 中无 `test_get_framework_config` 字符串残留。
