## MODIFIED Requirements

### Requirement: MCP tool 命名采用下划线 xx_yy 格式
The MCP server SHALL register tools with names using underscore `_` as the word separator, following the pattern `<domain>_<operation>`.

当前 11 个 MCP tool 的命名映射（从 `xx/yy` 变更为 `xx_yy`）：

| 旧名称（xx/yy） | 新名称（xx_yy） |
|-----------------|-----------------|
| `phase/log` | `phase_log` |
| `phase/check` | `phase_check` |
| `phase/next` | `phase_next` |
| `archi/query` | `archi_query` |
| `archi/validate` | `archi_validate` |
| `archi/write` | `archi_write` |
| `archi/check` | `archi_check` |
| `config/get` | `config_get` |
| `config/set` | `config_set` |
| `config/unset` | `config_unset` |
| `config/context` | `config_context` |

#### Scenario: tools/list 返回下划线命名的 tool
- **WHEN** MCP client 发起 `tools/list` 请求
- **THEN** 返回的 tool 列表中每个 tool 的 `name` 字段使用 `xx_yy` 下划线格式
- **AND** 不再出现 `xx/yy` 斜杠格式的名称

#### Scenario: tools/call 使用下划线名称调用 tool
- **WHEN** MCP client 发起 `tools/call` 请求，`params.name` 为 `"phase_log"`
- **THEN** server 正确路由到 phase-log 处理逻辑
- **AND** 返回正常的执行结果

#### Scenario: 未来的 tool 也遵循下划线命名
- **WHEN** 新增 MCP tool
- **THEN** tool 名称 SHALL 遵循 `<domain>_<operation>` 下划线格式，而非 `<domain>/<operation>` 斜杠格式

### Requirement: registerTool 调用使用 xx_yy 格式名称
`bin/src/mcp.ts` 中所有 `server.registerTool()` 调用的第一个参数（tool 名称）SHALL 使用 `xx_yy` 下划线格式的字符串。所有 11 个 `registerTool` 调用的名称 SHALL 更新为新格式。

#### Scenario: phase_log 注册为 tool 名称
- **WHEN** 检查 `mcp.ts` 中 `registerTool` 调用
- **THEN** `server.registerTool('phase_log', ...)` 存在
- **AND** `server.registerTool('phase/log', ...)` 不存在

#### Scenario: archi_check 注册为 tool 名称
- **WHEN** 检查 `mcp.ts` 中 `registerTool` 调用
- **THEN** `server.registerTool('archi_check', ...)` 存在
- **AND** `server.registerTool('archi/check', ...)` 不存在

#### Scenario: config_get 注册为 tool 名称
- **WHEN** 检查 `mcp.ts` 中 `registerTool` 调用
- **THEN** `server.registerTool('config_get', ...)` 存在
- **AND** `server.registerTool('config/get', ...)` 不存在

### Requirement: 编译产物 dev-team-mcp.cjs 同步更新
`bin/dev-team-mcp.cjs` SHALL 为 `bin/src/mcp.ts` 修改后重新编译的产物，包含下划线命名的 tool 定义和路由逻辑。

#### Scenario: 构建产物包含新名称
- **WHEN** 执行 `npm run build` 或等效构建命令
- **THEN** `bin/dev-team-mcp.cjs` 中所有 tool 名称使用 `xx_yy` 格式
- **AND** 不存在 `xx/yy` 格式的 tool 名称

### Requirement: settings.local.json 权限 allowlist 更新
`.claude/settings.local.json` 中的 MCP tool 权限 SHALL 更新为新名称（下划线格式）。旧名称 `mcp__plugin_dev-team_dev-team__eval/check` 和 `mcp__plugin_dev-team_dev-team__eval/log` SHALL 被清理替换为 `mcp__plugin_dev-team_dev-team__phase_check` 和 `mcp__plugin_dev-team_dev-team__phase_log`。

#### Scenario: phase_check 和 phase_log 在 allowlist 中
- **WHEN** 读取 `.claude/settings.local.json` 的 `permissions.allow` 数组
- **THEN** 包含 `"mcp__plugin_dev-team_dev-team__phase_check"`
- **AND** 包含 `"mcp__plugin_dev-team_dev-team__phase_log"`
- **AND** 不再包含旧名称 `mcp__plugin_dev-team_dev-team__eval/check` 和 `mcp__plugin_dev-team_dev-team__eval/log`

## Module Contract

### MCP Server (`plugins/dev-team/bin/src/mcp.ts`)

| Function/API | Description | Contract |
|-------------|-------------|----------|
| `server.registerTool(name, ...)` 调用 | Tool 定义注册 | 所有 tool `name` 使用 `xx_yy` 下划线格式 |

### Build Output (`plugins/dev-team/bin/`)

| File | Description | Contract |
|------|-------------|----------|
| `dev-team-mcp.cjs` | Compiled MCP server | 所有 tool name 使用 `xx_yy` 格式 |
