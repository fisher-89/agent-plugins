## ADDED Requirements

### Requirement: write_protection config field shape

The `openspec/config.json` schema SHALL define a `write_protection` top-level field with the following structure:

```typescript
{
  files?: Array<{
    glob: string;       // 文件路径 glob 模式，匹配的文件将被保护
    reason?: string;    // 自定义拒绝原因文案（支持 %s = 变更名, %t = 工具名 占位符）
  }>;
}
```

The `files` array SHALL be optional. When absent or empty, the system SHALL still enforce built-in default protections.

The `glob` field SHALL use standard glob patterns (e.g., `**/*.secret`, `openspec/changes/*/eval.json`).

The `reason` field SHALL be optional. When absent, the system SHALL use a built-in default denial message.

#### Scenario: write_protection with single glob entry validates successfully

**WHEN** a config object contains:
```json
{
  "write_protection": {
    "files": [{ "glob": "secrets/**/*.key" }]
  }
}
```
**THEN** the schema validation succeeds
**AND** the parsed output preserves the `write_protection` field

#### Scenario: write_protection with glob and custom reason validates

**WHEN** a config object contains:
```json
{
  "write_protection": {
    "files": [{ "glob": "config/deploy.yaml", "reason": "%s 的部署配置文件 %t 禁止直接修改" }]
  }
}
```
**THEN** the schema validation succeeds
**AND** `files[0].reason` equals the provided string

#### Scenario: write_protection with empty files array is valid

**WHEN** a config object contains `{ "write_protection": { "files": [] } }`
**THEN** the schema validation succeeds

#### Scenario: write_protection entry missing glob field fails validation

**WHEN** a config object contains `{ "write_protection": { "files": [{ "reason": "test" }] } }`
**THEN** the schema validation fails with a ZodError indicating that `glob` is required

#### Scenario: write_protection entry with empty glob string fails validation

**WHEN** a config object contains `{ "write_protection": { "files": [{ "glob": "" }] } }`
**THEN** the schema validation fails with a ZodError indicating that `glob` must not be empty

#### Scenario: write_protection is absent by default, built-in defaults apply

**WHEN** a config object `{}` is validated against the schema
**THEN** validation succeeds
**AND** `write_protection` is absent from the parsed output (undefined)

#### Scenario: unknown fields in write_protection entry pass through via .passthrough()

**WHEN** a config object contains `{ "write_protection": { "files": [{ "glob": "**/*.env", "extra_field": true }] } }`
**THEN** the schema validation succeeds
**AND** `extra_field` is preserved in the output

## Module Contract

### Schema: writeProtectionEntrySchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Type** | `z.ZodObject` with `.passthrough()` |
| **Fields** | `glob` (string, required, nonempty), `reason` (string, optional) |
| **Export** | Internal (used within configSchema definition) |

### Schema: writeProtectionSchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Type** | `z.ZodObject` |
| **Fields** | `files` (array of writeProtectionEntrySchema, optional) |
| **Export** | Internal (used within configSchema definition) |

### Schema: configSchema (write_protection addition)

| Property | Description |
|----------|-------------|
| **Field** | `write_protection` (optional, writeProtectionSchema) |
| **Behavior** | `.passthrough()` ensures unknown entry fields are preserved |
| **Default** | Absent/undefined when not specified |
