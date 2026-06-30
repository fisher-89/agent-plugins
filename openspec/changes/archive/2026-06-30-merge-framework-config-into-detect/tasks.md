# 任务: merge-framework-config-into-detect

<!--
  任务约定：每个 `- [ ]` 条目在实现完成后必须标记为 `- [x]`。
  不要勾选未完成的条目；如果一个任务项涵盖多个子步骤（如缩进列表），所有子步骤都完成后再标记父项为 `- [x]`。
-->

## Phase 1: 创建 lib/test-framework.ts

- [x] 创建 `plugins/dev-team/bin/src/lib/test-framework.ts`，包含以下导出：
  - `FrameworkConfig` interface（8 字段：framework, test_cmd, coverage_cmd, coverage_format, coverage_output, coverage_artifacts, coverage_cleanup, default_glob）
  - `FRAMEWORK_REGISTRY: Record<TestFrameworks, FrameworkConfig>`（8 框架完整配置）
  - `isTestFramework(framework: string): framework is TestFrameworks` 类型守卫
  - `getFrameworkConfig(framework: string): FrameworkConfig`（从 `commands/test-get-framework-config.ts` 的 `runTestGetFrameworkConfig` 迁移，签名从 options-object 简化为直接字符串参数）
  - `getDefaultGlobForFramework(framework: string): string`（同签名迁移）
- [x] 创建 `plugins/dev-team/bin/src/lib/test-framework.test.ts`，从 `commands/test-get-framework-config.test.ts` 迁移全部测试用例，适配以下变更：
  - import 路径改为 `'../lib/test-framework'`
  - `runTestGetFrameworkConfig({framework: x})` 调用改为 `getFrameworkConfig(x)`

## Phase 2: 更新 test-detect-frameworks 消费者

- [x] 修改 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`：
  - import 行从 `'./test-get-framework-config'` 改为 `'../lib/test-framework'`
  - `runTestGetFrameworkConfig({framework: mapping.framework})` 调用改为 `getFrameworkConfig(mapping.framework)`
- [x] 修改 `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`：
  - 更新 import 路径从 `'./test-get-framework-config'` 改为 `'../lib/test-framework'`
  - 更新所有 `runTestGetFrameworkConfig({framework: x})` 调用为 `getFrameworkConfig(x)`
- [x] 修改 `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts`：
  - import 路径改为 `'../../src/lib/test-framework'`
  - `runTestGetFrameworkConfig({framework: x})` 改为 `getFrameworkConfig(x)`

## Phase 3: 移除 test_get_framework_config MCP 工具

- [x] 修改 `plugins/dev-team/bin/src/schemas/index.ts`：
  - 移除 `testGetFrameworkConfigInputSchema`、`testGetFrameworkConfigOutputSchema` 的 export 行
- [x] 删除 `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts`
- [x] 删除 `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts`
- [x] 修改 `plugins/dev-team/bin/src/mcp.ts`：
  - 移除 `import { runTestGetFrameworkConfig } from './commands/test-get-framework-config'`
  - 移除 `testGetFrameworkConfigInputSchema`、`testGetFrameworkConfigOutputSchema` 的 import 声明
  - 移除 `registerTestGetFrameworkConfigTool` 函数定义
  - 移除 `main()` 中的 `registerTestGetFrameworkConfigTool(server)` 调用
- [x] 删除 `plugins/dev-team/bin/src/commands/test-get-framework-config.ts`
- [x] 删除 `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts`

## Phase 4: 更新 test-gen-generator 代理

- [x] 修改 `plugins/dev-team/agents/test-gen-generator.md`：
  - 删除第 2 步 "Framework config resolution" 中调用 `test_get_framework_config` 的指令
  - 将原有步骤替换为直接使用 `test_detect_frameworks` 返回的 `frameworks[]` 列表中的框架名来选择测试语法
  - 同步更新文件末尾的 "Constraints" 章节，移除对 `test_get_framework_config` 工具的描述

## Phase 5: 版本号与编译验证

- [x] 升级 `plugins/dev-team/bin/.claude-plugin/plugin.json` 的 `version` 字段为下一个 minor 版本（如 `2.5.0` -> `2.6.0`）
- [x] 运行 TypeScript 编译检查确认无 import/export 错误：`npx tsc --project plugins/dev-team/bin/tsconfig.json --noEmit`
