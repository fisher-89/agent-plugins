## 权威边界

仅规定 MCP tool **命名规则**。现行工具清单以 `plugins/dev-team/bin/src/mcp.ts` 的 `registerTool` 为准，本文件不维护全量表（决议 C7=A）。

## Requirements

### Requirement: MCP tool 命名采用下划线 xx_yy 格式

MCP server SHALL 使用 `<domain>_<operation>` 下划线格式注册 tool（例：`phase_next`、`phase_log`、`backtrack`）。MUST NOT 使用 `xx/yy` 斜杠格式。

新增 tool SHALL 遵循同一规则。

#### Scenario: tools/list 无斜杠名

- **WHEN** 客户端 `tools/list`
- **THEN** 每个 tool 的 `name` 为 `xx_yy`，无 `xx/yy`

#### Scenario: tools/call 使用下划线名

- **WHEN** `tools/call` 且 `params.name` 为 `"phase_log"`（或其它已注册的 `xx_yy` 名）
- **THEN** server 正确路由并返回结果

### Requirement: registerTool 与构建产物一致

`bin/src/mcp.ts` 中所有 `server.registerTool()` 的第一个参数 SHALL 为 `xx_yy`。编译产物（如 `dev-team-mcp.cjs` / 平台打包 MCP）中的 tool 名 SHALL 与源码一致，无斜杠名。

权限 allowlist（若使用）SHALL 引用下划线形式的完整 MCP tool id（例：`mcp__plugin_dev-team_dev-team__phase_next`），不得保留含 `/` 的旧名。

#### Scenario: 源码无斜杠注册名

- **WHEN** 检查 `mcp.ts` 的 `registerTool` 调用
- **THEN** 不存在形如 `'phase/next'` 的注册名

## Module Contract

| 位置 | Contract |
|------|----------|
| `plugins/dev-team/bin/src/mcp.ts` | 全部 tool `name` 为 `xx_yy` |
| 构建产物 | 与源码同名；无 `xx/yy` |
