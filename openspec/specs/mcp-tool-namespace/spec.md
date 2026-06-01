## Requirements

### Requirement: MCP tool 命名采用层级 xx/yy 格式
The MCP server SHALL register tools with hierarchical names using `/` as the namespace separator, following the pattern `<domain>/<operation>`.

当前 6 个 MCP tool 的命名映射：

| 旧名称 | 新名称 |
|--------|--------|
| `eval_log` | `phase/log` |
| `eval_check` | `phase/check` |
| `archi_query` | `archi/query` |
| `archi_validate` | `archi/validate` |
| `archi_write` | `archi/write` |
| `archi_check` | `archi/check` |

#### Scenario: tools/list 返回层级命名的 tool
- **WHEN** MCP client 发起 `tools/list` 请求
- **THEN** 返回的 tool 列表中每个 tool 的 `name` 字段使用 `xx/yy` 层级格式
- **AND** 不再出现 `xx_yy` 扁平格式的名称

#### Scenario: tools/call 使用层级名称调用 tool
- **WHEN** MCP client 发起 `tools/call` 请求，`params.name` 为 `"phase/log"`
- **THEN** server 正确路由到 eval-log 处理逻辑
- **AND** 返回正常的执行结果

#### Scenario: 未来的 tool 也遵循层级命名
- **WHEN** 新增 MCP tool（如 eval/export, archi/generate）
- **THEN** tool 名称 SHALL 遵循 `<domain>/<operation>` 格式

### Requirement: handleToolCall 按 xx/yy 格式路由
`bin/src/mcp.ts` 中的 `handleToolCall` 函数 SHALL 使用 `xx/yy` 格式的名称进行 switch 分支匹配。所有 6 个 case 语句 SHALL 更新为新名称。

#### Scenario: phase/log 路由到 runEvalLog
- **WHEN** `handleToolCall` 收到 name 为 `"phase/log"` 的调用
- **THEN** 执行 `runEvalLog` 函数
- **AND** 参数传递与旧 `"eval_log"` 分支完全一致

#### Scenario: archi/check 路由到 runCrossRefCheck
- **WHEN** `handleToolCall` 收到 name 为 `"archi/check"` 的调用
- **THEN** 执行 `runCrossRefCheck` 函数
- **AND** 参数传递与旧 `"archi_check"` 分支完全一致

### Requirement: 编译产物 dev-team-mcp.cjs 同步更新
`bin/dev-team-mcp.cjs` SHALL 为 `bin/src/mcp.ts` 修改后重新编译的产物，包含层级命名的 tool 定义和路由逻辑。

#### Scenario: 构建产物包含新名称
- **WHEN** 执行 `npm run build` 或等效构建命令
- **THEN** `bin/dev-team-mcp.cjs` 中的 `TOOLS` 数组使用 `xx/yy` 名称
- **AND** switch case 使用 `xx/yy` 名称

### Requirement: settings.local.json 权限 allowlist 更新
`.claude/settings.local.json` 中的 MCP tool 权限 SHALL 更新为新名称。

#### Scenario: phase/check 和 phase/log 在 allowlist 中
- **WHEN** 读取 `.claude/settings.local.json` 的 `permissions.allow` 数组
- **THEN** 包含 `"mcp__plugin_dev-team_dev-team__phase/check"`
- **AND** 包含 `"mcp__plugin_dev-team_dev-team__phase/log"`
- **AND** 不再包含旧名称 `mcp__plugin_dev-team_dev-team__eval_check` 和 `mcp__plugin_dev-team_dev-team__eval_log`

## Module Contract

### MCP Server (`plugins/dev-team/bin/src/mcp.ts`)

| Function/API | Description | Contract |
|-------------|-------------|----------|
| `TOOLS` array | Tool definition list | Each tool `name` uses `xx/yy` format |
| `handleToolCall(name, args)` | Tool routing switch | Case labels match `xx/yy` format |

### Build Output (`plugins/dev-team/bin/`)

| File | Description | Contract |
|------|-------------|----------|
| `dev-team-mcp.cjs` | Compiled MCP server | Contains `xx/yy` tool names in TOOLS and switch cases |
