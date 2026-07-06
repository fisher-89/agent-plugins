# 设计: Write Protection Config

> **变更**: write-protection-config
> **日期**: 2026-07-06

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Config Schema | 定义 `write_protection` 配置项的结构和验证规则 | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | zod/v4 | TypeScript |
| MCP Server | 注册/取消注册 MCP 工具；移除三个 config 工具后保留 config_get 等维护工具 | `plugins/dev-team/bin/src/mcp.ts` | @modelcontextprotocol/sdk, zod/v4 | TypeScript |
| Hook: protect-files | 读取 openspec/config.json 的 `write_protection` 配置，合并内置默认保护规则，在 PreToolUse 阶段拦截对受保护文件的直接写入 | `plugins/dev-team/hooks/scripts/protect-files.mjs` | node:fs, node:path, picomatch (自 ./scripts/ 模块解析或内联实现) | Node.js ESM |
| Hook Config | 将 PreToolUse 钩子的脚本引用从 protect-eval.mjs 更新为 protect-files.mjs | `plugins/dev-team/hooks/hooks.json` | — | JSON |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/hooks/scripts/protect-files.mjs` | 从 `protect-eval.mjs` 升级的 PreToolUse hook。读取 openspec/config.json 的 `write_protection` 配置，合并内置默认保护（`openspec/changes/**/eval.json` + `openspec/config.json`），对 Write、Edit、Bash、PowerShell 工具拦截对受保护文件的直接写入。支持自定义拒绝文案（`%s` = 文件路径, `%t` = 工具名称占位符）。 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 新增 `writeProtectionFileSchema` 和 `writeProtectionSchema`，在 `configSchema` 中新增 `write_protection` 可选字段 | 定义配置驱动的写入保护规则类型，复用 `.passthrough()` 保持向后兼容 |
| `plugins/dev-team/bin/src/mcp.ts` | 移除 `registerConfigSetTool`、`registerConfigUnsetTool`、`registerConfigContextTool` 三个函数调用及其 import | 清理不再使用的 MCP 工具 |
| `plugins/dev-team/bin/src/schemas/index.ts` | 移除 `configSetInputSchema`、`configSetOutputSchema`、`configUnsetInputSchema`、`configUnsetOutputSchema`、`configContextInputSchema`、`configContextOutputSchema` 的 export | 清理已删除 schema 的导出 |
| `plugins/dev-team/hooks/hooks.json` | 将所有 `protect-eval.mjs` 引用替换为 `protect-files.mjs`（共 3 处：Write/Edit、Bash、PowerShell 的 matcher） | 指向新的 hook 脚本 |
| `plugins/dev-team/.claude-plugin/plugin.json` | 升级 `version` 字段 | 发布新功能 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/commands/config-set.ts` | config_set MCP 工具的命令实现 |
| `plugins/dev-team/bin/src/commands/config-unset.ts` | config_unset MCP 工具的命令实现 |
| `plugins/dev-team/bin/src/commands/config-context.ts` | config_context MCP 工具的命令实现 |
| `plugins/dev-team/bin/src/schemas/config-set.schema.ts` | config_set 的 input/output schema 定义 |
| `plugins/dev-team/bin/src/schemas/config-unset.schema.ts` | config_unset 的 input/output schema 定义 |
| `plugins/dev-team/bin/src/schemas/config-context.schema.ts` | config_context 的 input/output schema 定义 |

<!-- 旧脚本保留为参考，但不再被 hooks.json 引用，后续归档时可清理 -->
| `plugins/dev-team/hooks/scripts/protect-eval.mjs` | 旧脚本，被 `protect-files.mjs` 替代 |
| `plugins/dev-team/hooks/scripts/protect-eval.test.mjs` | 旧测试文件，与新脚本不兼容 |

### 公共函数 / API

#### protect-files.mjs（新建，从 protect-eval.mjs 升级）

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `loadConfig` | `hooks/scripts/protect-files.mjs` | 新增 | `function loadConfig(projectRoot: string): WriteProtectionConfig` | 读取并解析 `openspec/config.json` 的 `write_protection` 字段，配置文件不存在或格式异常时返回空配置对象 |
| `loadPatterns` | `hooks/scripts/protect-files.mjs` | 新增 | `function loadPatterns(configDir: string): ProtectedPattern[]` | 合并内置默认保护（`openspec/changes/**/eval.json` + `openspec/config.json`）与用户配置的 `write_protection.files`，返回统一模式列表。每个模式包含 `matcher` 函数和可选的 `reason` |
| `isProtected` | `hooks/scripts/protect-files.mjs` | 新增 | `function isProtected(filePath: string, patterns: ProtectedPattern[]): { matched: boolean, matchedGlob?: string, reason?: string }` | 对归一化后的文件路径执行 glob 匹配，返回首个匹配结果 |
| `detectBashWrite` | `hooks/scripts/protect-files.mjs` | 修改 | `function detectBashWrite(cmd: string, patterns: ProtectedPattern[]): { decision: 'allow' | 'deny', reason?: string }` | 从 Bash 命令字符串中检测写操作（`>`、`>>`、`tee`、heredoc、`>&`），提取目标文件路径并检查是否受保护。python/node 命令豁免保留 |
| `detectPowerShellWrite` | `hooks/scripts/protect-files.mjs` | 修改 | `function detectPowerShellWrite(cmd: string, patterns: ProtectedPattern[]): { decision: 'allow' | 'deny', reason?: string }` | 从 PowerShell 命令字符串中检测写操作（Set-Content、Out-File、Add-Content、Export-Csv、Export-CliXml、Tee-Object、`>`、`>>`、`*>`、`[System.IO.File]` 方法），提取目标文件路径并检查是否受保护。python/node 命令豁免保留 |
| `extractChangeName` | `hooks/scripts/protect-files.mjs` | 保留 | `function extractChangeName(filePath: string): string` | 从路径中提取变更名称（匹配 `openspec/changes/([^/]+)`），用于默认拒绝文案的上下文填充 |
| `buildDenyReason` | `hooks/scripts/protect-files.mjs` | 修改 | `function buildDenyReason(pattern: ProtectedPattern, filePath: string, toolName: string): string` | 构建拒绝原因：优先使用模式的自定义 `reason`（支持 `%s`=文件路径、`%t`=工具名称占位符替换），无自定义时使用内置通用拒绝文案 |
| `outputAllow` | `hooks/scripts/protect-files.mjs` | 保留 | `function outputAllow(): string` | 返回 `{ hookSpecificOutput: { permissionDecision: "allow" } }` 的 JSON 字符串 |
| `outputDeny` | `hooks/scripts/protect-files.mjs` | 保留 | `function outputDeny(reason: string): string` | 返回 `{ hookSpecificOutput: { permissionDecision: "deny", permissionDecisionReason: reason } }` 的 JSON 字符串 |
| `parseInput` | `hooks/scripts/protect-files.mjs` | 修改 | `function parseInput(raw: string, patterns: ProtectedPattern[]): { decision: 'allow' | 'deny', reason?: string }` | 解析 stdin JSON 并根据工具类型和受保护模式列表做准入决策。fail-open：输入无效时返回 `allow` |
| `globToRegex` | `hooks/scripts/protect-files.mjs` | 新增 | `function globToRegex(glob: string): RegExp` | 将简单 glob 模式转换为 RegExp 用于路径匹配。支持 `**`（跨路径通配）、`*`（段内通配）、`?`（单字符） |

<!-- protect-eval.mjs 的导出函数（isEvalJsonPath 等）已废弃不再列出；其对应的功能由 protect-files.mjs 的新函数取代 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `WriteProtectionFileConfig` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 新增 | Zod type：`{ glob: string, reason?: string }`，定义单条写入保护规则 |
| `WriteProtectionConfig` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 新增 | Zod type：`{ files?: WriteProtectionFileConfig[] }`，定义 `write_protection` 顶层配置结构 |
| `ProtectedPattern` | `plugins/dev-team/hooks/scripts/protect-files.mjs` | 新增 | Type (JSDoc)：`{ match: (path: string) => boolean, glob: string, reason?: string }`，运行时用于 glob 匹配的模式对象 |

<!-- config-set/unset/context 相关的 schema 类型定义随文件删除而移除，已清理 -->

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `write_protection` | `openspec/config.json` | 新增 | `object` | `undefined`（无写入保护配置时使用内置默认保护） | 写入保护配置顶层键 |
| `write_protection.files` | `openspec/config.json` | 新增 | `array` | `[]`（仅内置默认保护生效） | 需要保护的文件 glob 模式列表 |
| `write_protection.files[].glob` | `openspec/config.json` | 新增 | `string` | — | 文件路径 glob 模式（如 `"openspec/changes/**/*.json"`） |
| `write_protection.files[].reason` | `openspec/config.json` | 新增 | `string` | — | 自定义拒绝原因，支持 `%s`（文件路径）和 `%t`（工具名称）占位符 |
| `version` | `plugins/dev-team/.claude-plugin/plugin.json` | 修改 | `string` | `"2.9.0"` | 版本号从 `2.8.2` 升级到 `2.9.0` |
| hooks.PreToolUse[].hooks[].command | `plugins/dev-team/hooks/hooks.json` | 修改 | `string` | `"node \"${CLAUDE_PLUGIN_ROOT}/hooks/scripts/protect-files.mjs\""` | 3 处 PreToolUse 的脚本路径从 `protect-eval.mjs` 改为 `protect-files.mjs` |

---

## 数据模型

### openspec/config.json — write_protection 配置结构

```typescript
// write_protection 配置项的数据模型
{
  write_protection?: {
    files?: Array<{
      glob: string;    // 文件路径 glob 模式（如 "openspec/changes/**/eval.json"）
      reason?: string; // 自定义拒绝原因，支持 %s（文件路径）、%t（工具名称）占位符
    }>;
  }
}
```

### 内置默认保护（运行时合并，不写回 config.json）

| 模式 | 说明 | 默认拒绝原因模板 |
|------|------|-----------------|
| `openspec/changes/**/eval.json` | 防止直接写入 eval.json | `该文件受写入保护：%s。detected via %t。请使用 phase_log MCP 工具替代。` |
| `openspec/config.json` | 防止直接写入 config.json | `该文件受写入保护：%s。detected via %t。请使用 config_get/config_set MCP 工具替代。` |

### 持久化

| 数据 | 存储位置 | 读写方式 |
|------|----------|----------|
| write_protection 配置 | `openspec/config.json` — 与现有测试、静态分析等配置同一文件 | hook 启动时通过 `readFileSync` + `JSON.parse` 读取；非运行时配置，不由 hook 写入 |
| 内置默认保护模式 | 硬编码于 `protect-files.mjs` 源码中 | 始终生效，无需配置 |

---

## 路由/API 设计

<!-- 本变更为插件内部架构变更，不涉及 HTTP API。MCP 工具层面的变更已在变更清单中覆盖。此节省略。 -->

---

## 依赖

### 运行时依赖

新增依赖 `picomatch`（已在 `plugins/dev-team/bin/package.json` 中存在，v4.0.4）— 用于在 hook 脚本中对文件路径执行 glob 模式匹配。

替代方案：若 hook 脚本无法从 `plugins/dev-team/bin/node_modules` 直接引用 picomatch，则实现内联 glob-to-regex 工具函数（`globToRegex`），零外部依赖。

### 构建/测试依赖

无新增。测试将在 `plugins/dev-team/bin/__tests__/protect-files-regression/` 目录下扩展现有 `protect-eval-regression.test.ts`。

---

## 待决问题

- 无
