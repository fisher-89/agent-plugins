## ADDED Requirements

### Requirement: test.exclude field in config Zod schema

**ID**: REQ-CFG-EXC-1
**Priority**: MUST
**Description**: The `test` object in `configSchema` (defined in `plugins/dev-team/bin/src/schemas/config/config.schema.ts`) SHALL include an optional `exclude` field of type `z.array(z.string())`. This field SHALL accept an array of glob patterns that match source file paths to be excluded from the testing pipeline. The field SHALL default to `undefined` when not specified.

#### Scenario: test.exclude accepts valid string array of globs

**WHEN** a config object `{"test": {"exclude": ["**/generated/**", "**/*.d.ts"]}}` is validated against `configSchema`
**THEN** validation succeeds
**AND** the parsed `test.exclude` SHALL equal `["**/generated/**", "**/*.d.ts"]`

#### Scenario: test.exclude defaults to undefined when not specified

**WHEN** a config object `{"test": {"framework": "vitest"}}` is validated against `configSchema`
**THEN** validation succeeds
**AND** `test.exclude` is `undefined`

#### Scenario: test.exclude rejects non-array value

**WHEN** a config object `{"test": {"exclude": "**/generated/**"}}` is validated against `configSchema`
**THEN** validation fails with a ZodError indicating that string is not assignable to array

#### Scenario: empty exclude array is accepted

**WHEN** a config object `{"test": {"exclude": []}}` is validated against `configSchema`
**THEN** validation succeeds
**AND** `test.exclude` SHALL equal `[]`

### Requirement: test.overrides[].exclude field in config Zod schema

**ID**: REQ-CFG-EXC-2
**Priority**: MUST
**Description**: Each entry in the `test.overrides` array schema SHALL include an optional `exclude` field of type `z.array(z.string()).optional()`. This field SHALL accept an array of glob patterns that are scoped to the override's `file` glob. The field SHALL default to `undefined` when not specified.

#### Scenario: override entry with exclude passes validation

**WHEN** a config object `{"test": {"overrides": [{"file": "src/**", "framework": "vitest", "exclude": ["**/legacy/**"]}]}}` is validated against `configSchema`
**THEN** validation succeeds
**AND** `test.overrides[0].exclude` SHALL equal `["**/legacy/**"]`

#### Scenario: override entry without exclude defaults to undefined

**WHEN** a config object `{"test": {"overrides": [{"file": "src/**", "framework": "vitest"}]}}` is validated against `configSchema`
**THEN** validation succeeds
**AND** `test.overrides[0].exclude` is `undefined`

#### Scenario: override entry with invalid exclude type is rejected

**WHEN** a config object `{"test": {"overrides": [{"file": "src/**", "exclude": "single-string"}]}}` is validated against `configSchema`
**THEN** validation fails with a ZodError

### Requirement: JSON schema mirrors Zod schema for exclude fields

**ID**: REQ-CFG-EXC-3
**Priority**: SHOULD
**Description**: The `dev-team-config.schema.json` file SHALL include the `exclude` field in both the `test` object's `properties` and each override item's `properties`. The type SHALL be `"array"` with `"items": {"type": "string"}`. The field SHALL NOT be in the `required` array.

#### Scenario: JSON schema includes test.exclude

**WHEN** `dev-team-config.schema.json` is inspected
**THEN** `properties.test.properties.exclude` exists
**AND** `properties.test.properties.exclude.type` SHALL be `"array"`
**AND** `properties.test.properties.exclude.items.type` SHALL be `"string"`

#### Scenario: JSON schema includes overrides[].exclude

**WHEN** `dev-team-config.schema.json` is inspected
**THEN** `properties.test.properties.overrides.items.properties.exclude` exists
**AND** its `type` SHALL be `"array"`
**AND** its `items.type` SHALL be `"string"`

### Requirement: OpenSpecConfig type reflects exclude field

**ID**: REQ-CFG-EXC-4
**Priority**: MUST
**Description**: The TypeScript type `OpenSpecConfig` (inferred from `configSchema` via `z.infer`) SHALL include `test.exclude` as `string[] | undefined` and `test.overrides[].exclude` as `string[] | undefined`. No manual type annotation is required — the Zod schema inference SHALL produce the correct type.

#### Scenario: OpenSpecConfig type includes test.exclude

**WHEN** inspecting the `OpenSpecConfig` type
**THEN** `test.exclude` is an accessible property of type `string[] | undefined`

#### Scenario: OpenSpecConfig type includes overrides[].exclude

**WHEN** inspecting the `OpenSpecConfig` type
**THEN** `test.overrides[number].exclude` is an accessible property of type `string[] | undefined`

---

## Module Contract (exclude additions)

### Schema: configSchema (added fields)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `test.exclude` | `z.array(z.string()).optional()` | No | Global source file exclusion globs |
| `test.overrides[].exclude` | `z.array(z.string()).optional()` | No | Per-override source file exclusion globs |

### File: dev-team-config.schema.json (added fields)

| JSON Path | Type | Required | Description |
|-----------|------|----------|-------------|
| `properties.test.properties.exclude` | `{ type: "array", items: { type: "string" } }` | No | Global source file exclusion globs |
| `properties.test.properties.overrides.items.properties.exclude` | `{ type: "array", items: { type: "string" } }` | No | Per-override source file exclusion globs |

### Type: OpenSpecConfig (reflected fields)

```typescript
type OpenSpecConfig = {
  test?: {
    exclude?: string[];         // ADDED
    // ... existing fields ...
    overrides?: Array<{
      exclude?: string[];       // ADDED
      // ... existing fields ...
    }>;
  };
}
```
