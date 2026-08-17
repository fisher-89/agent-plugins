# spec-list Specification

## Purpose
MCP tool that scans `openspec/specs/*/spec.md` files and returns a list of capabilities, replacing the `openspec spec list --json` CLI command.

## Requirements

### Requirement: spec_list MCP tool is registered

The plugin SHALL register a `spec_list` MCP tool in `mcp.ts` with no required input parameters. The tool SHALL scan `openspec/specs/` for subdirectories containing `spec.md` files and return a list of capability objects.

#### Scenario: Tool appears in MCP tool list

- **WHEN** the MCP server is started
- **THEN** `spec_list` SHALL appear in the list of available MCP tools

#### Scenario: Returns capability list

- **WHEN** `spec_list` is called
- **THEN** it SHALL return an array of capabilities found in `openspec/specs/*/spec.md`
- **AND** each capability SHALL include `id` (directory name) and `title` (from spec content or filename)

### Requirement: spec_list returns the same structure as `openspec spec list --json`

The tool SHALL return a JSON array where each entry has at minimum `id` and `title` fields, matching the field structure that `proposal-planner.md` currently consumes from `openspec spec list --json`.

#### Scenario: Returned capabilities match directory layout

- **WHEN** `spec_list` is called on a project where `openspec/specs/embedded-cli/spec.md` exists
- **THEN** the returned array SHALL include an entry with `id: "embedded-cli"`
- **AND** the `title` field SHALL be populated

#### Scenario: Empty specs directory returns empty array

- **WHEN** `spec_list` is called on a project with no `openspec/specs/` directory or no `spec.md` files
- **THEN** the tool SHALL return an empty array `[]`

### Requirement: spec_list does NOT classify capabilities as 新增/修改

The tool SHALL return a flat capability list only. The classification of capabilities as "新增" or "修改" SHALL remain in the agent side (proposal-planner.md), matching the current behavior where the bundle only returns the raw list.

#### Scenario: No classification in output

- **WHEN** examining the `spec_list` output structure
- **THEN** each capability entry SHALL NOT contain a classification field (such as `change_type` or `status`)
- **AND** the agent SHALL determine the classification based on its own comparison logic

## Module Contract

### MCP 工具注册: `plugins/dev-team/bin/src/mcp.ts`

| 字段 | 值 |
|------|-----|
| 工具名 | `spec_list` |
| 输入 | 无（或 `{}`） |
| 输出 | `Capability[]`（`{ id: string, title: string }`） |
| handler | 调用 `listSpecs()` 函数 |

### 命令实现: `plugins/dev-team/bin/src/commands/spec-list.ts`

| 函数 | 签名 | 描述 |
|------|------|------|
| `listSpecs` | `(specsDir: string) => Capability[]` | 扫描 `openspec/specs/*/spec.md` 返回 capability 列表 |

### 消费者: `plugins/dev-team/agents/proposal-planner.md`

| 变更前 | 变更后 |
|--------|--------|
| `source __DEV_TEAM_ROOT__/utils/openspec-cli.sh && openspec_spec_list "<name>"` | `__MCP:spec_list__` |
