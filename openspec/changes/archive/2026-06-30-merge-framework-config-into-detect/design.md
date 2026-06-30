# 设计: merge-framework-config-into-detect

> **变更**: merge-framework-config-into-detect
> **日期**: 2026-06-29

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Test Framework Registry | 维护 8 个测试框架的硬编码注册表及查询函数 | `lib/test-framework.ts` | 无（纯数据模块） | TypeScript |
| Framework Detection | 基于 glob 匹配检测文件所属框架，传播注册表字段到 plan | `commands/test-detect-frameworks.ts` | `lib/test-framework.ts` | TypeScript |
| MCP Server | 注册 MCP 工具端点 | `mcp.ts` | — | TypeScript |
| Schemas | Zod 输入/输出 schema 定义 | `schemas/` | zod/v4 | TypeScript |
| test-gen-generator Agent | 生成测试骨架文件 | `agents/test-gen-generator.md` | — | Markdown |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 从 `commands/test-get-framework-config.ts` 提取的 FRAMEWORK_REGISTRY、FrameworkConfig 接口、getFrameworkConfig()、getDefaultGlobForFramework()、isTestFramework() |
| `plugins/dev-team/bin/src/lib/test-framework.test.ts` | 从 `commands/test-get-framework-config.test.ts` 迁移的测试，验证 8 框架全部注册表字段 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | import 路径从 `'./test-get-framework-config'` 改为 `'../lib/test-framework'`；`runTestGetFrameworkConfig({framework: x})` 调用改为 `getFrameworkConfig(x)` | 使用新的 lib 模块 |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts` | 更新 import 路径；`runTestGetFrameworkConfig({framework: x})` 改为 `getFrameworkConfig(x)` | 反映新模块路径和函数签名 |
| `plugins/dev-team/bin/src/mcp.ts` | 移除 `registerTestGetFrameworkConfigTool` 函数调用和 `runTestGetFrameworkConfig` import | 取消 `test_get_framework_config` MCP 工具注册 |
| `plugins/dev-team/bin/src/schemas/index.ts` | 移除 `testGetFrameworkConfigInputSchema`、`testGetFrameworkConfigOutputSchema` 的 export | schema 随工具移除 |
| `plugins/dev-team/agents/test-gen-generator.md` | 将第 2 步 "Framework config resolution" 替换为直接使用 `test_detect_frameworks` 返回的 `frameworks[]` 或 `plan[].framework` | 不再调用 `test_get_framework_config` |
| `plugins/dev-team/bin/.claude-plugin/plugin.json` | 升级 `version` 字段（minor 版本） | 反映 MCP 工具移除 |
| `plugins/dev-team/bin/__tests__/test_detect_frameworks_plan_json_only_coverage/test_detect_frameworks_plan_json_only_coverage.test.ts` | 更新 `runTestGetFrameworkConfig` 的 import 路径和函数名 | 反映新 lib 模块 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/test-get-framework-config.ts` | 内容已迁移至 `lib/test-framework.ts` |
| `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts` | 随 MCP 工具移除 |
| `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts` | 测试已迁移至 `lib/test-framework.test.ts` |
| `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts` | 随 schema 移除 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `getFrameworkConfig` | `lib/test-framework.ts` | 新增 | `function getFrameworkConfig(framework: string): FrameworkConfig` | 从注册表查询框架配置。参数 `framework` 为框架名；返回完整 `FrameworkConfig` 对象；未知框架名抛出 Error |
| `getDefaultGlobForFramework` | `lib/test-framework.ts` | 修改（迁移） | `function getDefaultGlobForFramework(framework: string): string` | 返回框架的默认 glob 模式；未知框架名抛出 Error |
| `runTestGetFrameworkConfig` | — | 删除 | — | 原 `commands/test-get-framework-config.ts` 的导出函数，被 `getFrameworkConfig` 替代 |

<!-- 无 CLI 子命令或 HTTP 端点变更 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FrameworkConfig` | `lib/test-framework.ts` | 新增（interface） | 框架注册表条目类型。字段：`framework: TestFrameworks`、`test_cmd: string`、`coverage_cmd: string`、`coverage_format: 'istanbul' \| 'llvm-cov' \| 'node-test' \| 'go-cover' \| 'coverage-py'`、`coverage_output: string`、`coverage_artifacts: string[]`、`coverage_cleanup: string[]`、`default_glob: string` |
| `TestGetFrameworkConfigOptions` | — | 删除 | 原 `commands/test-get-framework-config.ts` 的接口；新函数 `getFrameworkConfig` 使用直接字符串参数，无需此类型 |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `plugins/dev-team/bin/.claude-plugin/plugin.json` | 修改 | — | minor 版本升级（如 `2.5.0` -> `2.6.0`），因 MCP 工具移除符合 semver minor 变更 |

### 不要修改

以下文件不在本变更范围内，实现阶段禁止修改：

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 与框架注册表无关；不包含 `test_get_framework_config` 引用 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 对应 `test-resolve-paths.ts` 的测试；无关联变更 |

<!-- 此列表根据 proposal.md「变更范围 - 不要修改」扩展而来，确保变更边界清晰 -->

---

## 数据模型

`FrameworkConfig` 是变更的核心数据模型，是纯静态注册表，不涉及持久化或运行时变更。

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FrameworkConfig` | `framework: TestFrameworks`（枚举键）、`test_cmd`、`coverage_cmd`、`coverage_format`、`coverage_output`、`coverage_artifacts`、`coverage_cleanup`、`default_glob` | 无外部关系；`FRAMEWORK_REGISTRY` 以 `TestFrameworks` 枚举为键 | 无（硬编码常量，编译时确定） |

---

<!-- 本变更不涉及 HTTP API，省略路由/API 设计节 -->

---

## 依赖

### 运行时依赖

无新增运行时依赖。移除的 `schemas/test-get-framework-config.schema.ts` 依赖 `zod/v4`，但该依赖在整个项目中已存在，不构成新增。

### 构建/测试依赖

无新增构建/测试依赖。所有已有测试工具（`vite-plus/test`）已在项目中配置。

---

## 待决问题

1. **`test_cmd` 字段是否保留在 `FrameworkConfig` 接口中？** 当前设计保留 `test_cmd` 以最小化数据提取的差异，但该字段已在 `FRAMEWORK_REGISTRY` 中存在且无外部消费者。替代方案是移除该字段简化接口。采纳当前决定是因为：移除字段需要同步删除所有注册表条目中的值，增加了额外的无价值变更；保持原样不影响任何功能。
2. **`getFrameworkConfig` 是否保留 `runTestGetFrameworkConfig` 的 options-object 签名？** 当前设计采用简化签名 `getFrameworkConfig(framework: string)`，因为 lib 函数不需要 MCP handlers 的 options-object 模式。如果未来需要更多选项，可恢复为该模式。
