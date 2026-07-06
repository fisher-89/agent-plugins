## ADDED Requirements

### Requirement: config-schema defines write_protection sub-schema

The `configSchema` in `plugins/dev-team/bin/src/schemas/config/config.schema.ts` SHALL be extended with a new `write_protection` field.

The field SHALL be optional at the top level.

The field SHALL use a `writeProtectionSchema` object with a single optional `files` array property.

Each entry in the `files` array SHALL be an object with:
- `glob` (string, required, nonempty) — glob pattern matching protected files
- `reason` (string, optional) — custom denial reason with `%s` (file path) and `%t` (tool name) placeholders

The entry schema SHALL use `.passthrough()` to preserve unknown fields, matching the pattern of other config sub-objects.

The `writeProtectionSchema` SHALL be exported as a named export for testing.

#### Scenario: configSchema includes write_protection as optional field

**WHEN** inspecting `configSchema.shape`
**THEN** `write_protection` exists as an optional key

#### Scenario: write_protection field accepts valid configuration

**WHEN** a config object `{ "write_protection": { "files": [{"glob": "*.env"}] } }` is validated against configSchema
**THEN** validation succeeds and the parsed output contains `write_protection.files[0].glob` with value `"*.env"`

#### Scenario: write_protection field type rejects non-object value

**WHEN** a config object `{ "write_protection": "invalid" }` is validated
**THEN** validation fails with a ZodError

#### Scenario: writeProtectionSchema is exported for unit testing

**WHEN** importing from `schemas/config/config.schema.ts`
**THEN** `writeProtectionSchema` is available as a named export

### Requirement: OpenSpecConfig type reflects write_protection

The TypeScript type `OpenSpecConfig` (inferred from `configSchema`) SHALL include the `write_protection` field as optional with the correct nested type.

#### Scenario: OpenSpecConfig type includes write_protection

**WHEN** inspecting the `OpenSpecConfig` type
**THEN** `write_protection` is an accessible optional property

## Module Contract

### Schema: configSchema (added field)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `write_protection` | `z.ZodOptional<typeof writeProtectionSchema>` | No | Write protection configuration |

### Type: writeProtectionSchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Fields** | `files: z.array(writeProtectionEntrySchema).optional()` |
| **Behavior** | `.passthrough()` on each entry for unknown fields |
| **Export** | Named export |

### Type: writeProtectionEntrySchema

| Property | Description |
|----------|-------------|
| **Fields** | `glob: z.string().nonempty()`, `reason: z.string().optional()` |
| **Behavior** | `.passthrough()` for unknown keys |
| **Export** | Internal (not exported) |
