# 提案: merge-framework-config-into-detect

> **变更**: merge-framework-config-into-detect
> **日期**: 2026-06-29
> **状态**: 草稿

---

## 问题

`test_get_framework_config` 是一个独立的 MCP 工具，用于查询硬编码的测试框架注册表（FRAMEWORK_REGISTRY），返回测试命令、覆盖率命令等配置。

然而，`test_detect_frameworks` MCP 工具在内部已经通过 `runTestGetFrameworkConfig()` 调用了同一个注册表，并将所有字段（`coverage_cmd`、`coverage_format`、`coverage_output`、`coverage_artifacts`、`coverage_cleanup`）传播到了其输出 `plan[]` 数组中。唯一未传播的字段 `test_cmd` 没有任何消费者实际使用——`unit-test-executor` 直接执行 `plan[].script` 而非 `test_cmd`，`test-gen-generator` 只需要框架名来选择语法。

当前状况导致的问题：
1. **MCP 工具膨胀**：`test_get_framework_config` 作为独立 MCP 工具暴露内部实现细节，增加了 MCP 命名空间的复杂度
2. **消费者冗余调用**：`test-gen-generator` 代理同时调用 `test_detect_frameworks` 和 `test_get_framework_config`，后者仅用于获取框架名（而框架名已在前者的返回中）
3. **维护负担**：修改注册表需要同步更新两个工具的文档和测试

---

## 提案

将 `FRAMEWORK_REGISTRY` 及相关查询函数从 `commands/test-get-framework-config.ts` 提取到 `lib/test-framework.ts` 作为内部模块，然后移除 `test_get_framework_config` MCP 工具。`test_detect_frameworks` 通过内部 import 使用新的 lib 模块，外部行为完全不变。

具体步骤：
1. 新建 `lib/test-framework.ts`，包含 `FRAMEWORK_REGISTRY`、`runTestGetFrameworkConfig()`、`getDefaultGlobForFramework()` 和类型定义
2. 更新 `test-detect-frameworks` 的 import 从 `'./test-get-framework-config'` 改为 `'../lib/test-framework'`
3. 从 MCP 注册（`mcp.ts`）、schemas、测试中移除 `test_get_framework_config` 相关内容
4. 更新 `test-gen-generator` 代理，移除对 `test_get_framework_config` 的调用，仅使用 `test_detect_frameworks` 返回的 `frameworks[]` 或 `plan[].framework`
5. `test_detect_frameworks` 的输出 schema（`plan[]`）保持不变——plan 条目已包含所有消费者需要的字段

---

## 能力

### 移除的能力

- **test-get-framework-config** — 移除 `test_get_framework_config` MCP 工具，将 FRAMEWORK_REGISTRY 内迁为 `lib/test-framework.ts` 内部模块

### 修改的能力

- **test-detect-frameworks** — 导入路径从 `'./test-get-framework-config'` 变更为 `'../lib/test-framework'`；行为、输入输出 schema 不变

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/lib/test-framework.ts`（**新建** — 提取的 FRAMEWORK_REGISTRY 与查询函数）
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`（更新 import 路径）
- `plugins/dev-team/bin/src/mcp.ts`（移除 `registerTestGetFrameworkConfigTool` 调用及 import）
- `plugins/dev-team/bin/src/schemas/index.ts`（移除 `testGetFrameworkConfigInputSchema`、`testGetFrameworkConfigOutputSchema` 的 export）
- `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.ts`（**删除** — schema 文件随工具移除）
- `plugins/dev-team/agents/test-gen-generator.md`（将 "Framework config resolution" 步骤替换为使用 `plan[]` 或 `frameworks[]` 中的框架名）
- `plugins/dev-team/bin/src/commands/test-get-framework-config.ts`（**删除** — 内容已迁移至 lib）

### 测试文件

- `plugins/dev-team/bin/src/lib/test-framework.test.ts`（**新建** — 从 `test-get-framework-config.test.ts` 迁移并适配内部模块测试）
- `plugins/dev-team/bin/src/commands/test-get-framework-config.test.ts`（**删除** — 测试已迁移）
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`（更新 import 路径，验证 plan 字段完整性）
- `plugins/dev-team/bin/src/schemas/test-get-framework-config.schema.test.ts`（**删除** — schema 随工具移除）

### 不要修改

- `test_detect_frameworks` 的输入/输出 schema（`plan[]` 结构完全不变）
- `unit-test-executor.md` 代理（已使用 `plan[]` 且明确声明不调用 `test_get_framework_config`）
- `test-design-planner.md` 代理（未调用以上任一工具）
- 任何其他 MCP 工具或代理的 API 合约

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | FRAMEWORK_REGISTRY 迁移至 lib/test-framework.ts | `lib/test-framework.ts` 包含完整的 8 框架注册表（jest, vitest, vite-plus, bun, rust, node-test, go, pytest），所有字段正确 |
| AC-2 | test-detect-frameworks 使用新 import | `test-detect-frameworks.ts` 从 `../lib/test-framework` 导入 `runTestGetFrameworkConfig` 和 `getDefaultGlobForFramework` |
| AC-3 | test_get_framework_config 从 MCP 中移除 | `mcp.ts` 中无 `registerTestGetFrameworkConfigTool` 调用，无 `runTestGetFrameworkConfig` 导入 |
| AC-4 | test_get_framework_config schema 移除 | `schemas/index.ts` 不 export `testGetFrameworkConfigInputSchema`/`testGetFrameworkConfigOutputSchema` |
| AC-5 | test-get-framework-config 源文件删除 | `commands/test-get-framework-config.ts` 和 `schemas/test-get-framework-config.schema.ts` 从仓库中删除 |
| AC-6 | test-gen-generator 停止调用 test_get_framework_config | `test-gen-generator.md` 的 "Framework config resolution" 步骤改为仅使用 `test_detect_frameworks` 返回的框架名，无 `test_get_framework_config` 调用 |
| AC-7 | test_detect_frameworks 原有测试全部通过 | `test-detect-frameworks.test.ts` 全部测试用例通过，无 import 错误 |
| AC-8 | 新 lib/test-framework 测试覆盖全部注册表字段 | `lib/test-framework.test.ts` 验证 8 框架的 `coverage_cmd`、`coverage_format`、`coverage_output`、`coverage_artifacts`、`coverage_cleanup`、`default_glob` 均正确 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| test-gen-generator 对 test_get_framework_config 的引用遗漏 | 代理在运行时尝试调用不存在的 MCP 工具，导致骨架生成失败 | 低 | 全局 grep 确认所有 `.md` 代理文件中的引用已更新；测试期间运行 `test-gen-generator` 验证 |
| 现有依赖 test_get_framework_config 的外部工作流  | 外部 Claude Code 用户调用时收到 "unknown tool" 错误 | 低 | MCP 工具移除在 minor 版本变更中（2.x），符合 semver；在 plugin.json 升级版本号 |
| lib/test-framework 导出路径错误 | test-detect-frameworks 编译失败 | 低 | 从 lib 的路径 `../lib/test-framework` 已验证；运行 TypeScript 编译检查 |

