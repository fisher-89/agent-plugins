# Tasks: Write Protection Config

## 阶段一：Config Schema 扩展

- [x] 在 `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 中新增 `writeProtectionFileSchema`（z.object: `glob: z.string()`, `reason: z.string().optional()`）
- [x] 在 `plugins/dev-team/bin/src/schemas/config/config.schema.ts` 中新增 `writeProtectionSchema`（z.object: `files: z.array(writeProtectionFileSchema).optional()`）
- [x] 在 `configSchema` 中加入 `write_protection: writeProtectionSchema.optional()` 字段

## 阶段二：创建 protect-files.mjs 脚本

- [x] 创建 `plugins/dev-team/hooks/scripts/protect-files.mjs`，复制 `protect-eval.mjs` 的现有结构作为起点
- [x] 实现 `loadConfig(projectRoot)` — 读取并解析 `openspec/config.json` 的 `write_protection` 字段，异常时返回空配置
- [x] 实现 `globToRegex(glob)` — 将简单 glob 模式转换为 RegExp，支持 `**` / `*` / `?` 通配符
- [x] 实现 `loadPatterns(configDir)` — 合并内置默认保护（`openspec/changes/**/eval.json` + `openspec/config.json`）与用户配置的 glob 模式
- [x] 实现 `isProtected(filePath, patterns)` — 对归一化后的文件路径执行模式匹配，返回匹配结果
- [x] 修改 `buildDenyReason(pattern, filePath, toolName)` — 支持自定义 reason 模板的 `%s`/`%t` 占位符替换，无自定义时使用内置通用拒绝文案
- [x] 修改 `detectBashWrite(cmd, patterns)` — 从命令中提取目标文件路径并调用 `isProtected`，替换原有的硬编码 eval.json 检查
- [x] 修改 `detectPowerShellWrite(cmd, patterns)` — 从命令中提取目标文件路径并调用 `isProtected`，替换原有的硬编码 eval.json 检查
- [x] 修改 `parseInput(raw, patterns)` — 接受 patterns 参数，将各工具检测结果委托给新的 detect 函数
- [x] 更新 `main()` — 启动时调用 `loadPatterns`，将 patterns 传入 `parseInput`

## 阶段三：MCP 工具清理

- [x] 删除 `plugins/dev-team/bin/src/commands/config-set.ts`
- [x] 删除 `plugins/dev-team/bin/src/commands/config-unset.ts`
- [x] 删除 `plugins/dev-team/bin/src/commands/config-context.ts`
- [x] 删除 `plugins/dev-team/bin/src/schemas/config-set.schema.ts`
- [x] 删除 `plugins/dev-team/bin/src/schemas/config-unset.schema.ts`
- [x] 删除 `plugins/dev-team/bin/src/schemas/config-context.schema.ts`
- [x] 删除 `plugins/dev-team/hooks/scripts/protect-eval.mjs`
- [x] 删除 `plugins/dev-team/hooks/scripts/protect-eval.test.mjs`
- [x] 从 `plugins/dev-team/bin/src/schemas/index.ts` 中移除 config-set/unset/context 的 export 语句
- [x] 从 `plugins/dev-team/bin/src/mcp.ts` 中移除 `registerConfigSetTool`、`registerConfigUnsetTool`、`registerConfigContextTool` 三个函数的定义和 main() 中的注册调用，清理相关 import

## 阶段四：Hook 配置更新

- [x] 更新 `plugins/dev-team/hooks/hooks.json` 中 3 处 PreToolUse 的 command 路径，将 `protect-eval.mjs` 替换为 `protect-files.mjs`

## 阶段五：版本号升级

- [x] 将 `plugins/dev-team/.claude-plugin/plugin.json` 的 `version` 从 `"2.8.2"` 升级为 `"2.9.0"`

## 阶段六：验证

- [x] 确认 `protect-files.mjs` 可被 node 直接运行且无语法错误（`node --check plugins/dev-team/hooks/scripts/protect-files.mjs`）
- [x] 确认 `plugins/dev-team/bin` 的 TypeScript 编译通过（`npx tsc --noEmit`），验证 config schema 扩展无类型错误
- [x] 确认已删除的 MCP 工具（config_set/config_unset/config_context）在 `plugins/dev-team/bin/src/mcp.ts` 中不再有任何残留引用
