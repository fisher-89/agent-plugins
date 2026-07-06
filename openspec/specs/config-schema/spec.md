## MODIFIED Requirements

### Requirement: config-schema defines a Zod schema for config.json

A Zod schema SHALL be defined at `plugins/dev-team/bin/src/schemas/config/config.schema.ts` that describes the structure of `openspec/config.json`.
The schema SHALL use `zod/v4` for schema definitions.
The schema SHALL define the following top-level fields:
- `schema`: a string literal type `"spec-driven"` with a default value of `"spec-driven"`
- `context`: an optional string field
- `rules`: an optional object with optional `proposal` (string array) and `tasks` (string array) fields
- `test`: an optional object with the following sub-fields:
  - `frameworks`: an optional field accepting either a single framework name string (valid values: `"jest"`, `"vitest"`, `"vite-plus"`, `"bun"`, `"rust"`, `"node-test"`, `"go"`, `"pytest"`) or an array of `{glob: string, framework: string}` objects, defining test framework detection rules
  - `coverage`: an optional object with:
    - `thresholds`: an optional object with `lines: number` (default 80), `branches: number` (default 70), `functions: number` (default 75), defining minimum coverage percentages per dimension
    - `overrides`: an optional array of `{glob: string, thresholds: {lines?: number, branches?: number, functions?: number}}` objects, defining per-directory threshold overrides. Missing fields in an override entry inherit the global defaults.
The schema SHALL allow additional unknown fields via `.passthrough()` to avoid rejecting valid configurations with tool-managed keys.
A TypeScript type `OpenSpecConfig` SHALL be exported, derived from the schema using `z.infer<typeof configSchema>`.

#### Scenario: Valid config object passes schema validation

- **WHEN** a config object `{"schema": "spec-driven", "context": "Tech stack: TypeScript"}` is validated against the schema
- **THEN** validation succeeds and returns the parsed object with type `OpenSpecConfig`

#### Scenario: Schema assigns default for missing schema field

- **WHEN** a config object `{}` is validated against the schema
- **THEN** the parsed output includes `schema` with value `"spec-driven"` (the default)

#### Scenario: Schema validates field types

- **WHEN** a config object `{"schema": 123}` is validated against the schema
- **THEN** validation fails with a ZodError indicating that `schema` must be a string literal

#### Scenario: Schema allows additional unknown fields

- **WHEN** a config object `{"schema": "spec-driven", "static_check": ["npm test"]}` is validated
- **THEN** validation succeeds and the extra `static_check` field is preserved in the output

#### Scenario: Schema validates rules sub-object

- **WHEN** a config object `{"rules": {"proposal": ["Keep it short"]}}` is validated
- **THEN** validation succeeds and `rules.proposal` contains the expected string array

#### Scenario: OpenSpecConfig type is inferred from schema

- **WHEN** the module is imported in TypeScript
- **THEN** `OpenSpecConfig` type exists and is assignable to `z.infer<typeof configSchema>`

#### Scenario: Schema validates tests.frameworks structure

- **WHEN** a config object `{"test": {"frameworks": [{"glob": "**/*.test.ts", "framework": "vitest"}, {"glob": "**/test_*.rs", "framework": "rust"}]}}` is validated
- **THEN** validation succeeds and `test.frameworks` contains the expected glob and framework pairs

#### Scenario: Schema assigns defaults for coverage.thresholds

- **WHEN** a config object `{"test": {"frameworks": []}}` is validated without `coverage` or `thresholds` specified
- **THEN** the parsed output includes `test.coverage.thresholds` with defaults `{lines: 80, branches: 70, functions: 75}`

#### Scenario: Schema applies per-field defaults for partial thresholds

- **WHEN** a config object `{"test": {"coverage": {"thresholds": {"lines": 90}}}}` is validated
- **THEN** the parsed output includes `test.coverage.thresholds` with `{lines: 90, branches: 70, functions: 75}` (branches and functions inherit defaults)

#### Scenario: Schema validates coverage.overrides structure

- **WHEN** a config object `{"test": {"coverage": {"overrides": [{"glob": "demo/**", "thresholds": {"lines": 60}}]}}}` is validated
- **THEN** validation succeeds and `overrides[0]` contains `glob: "demo/**"` and `thresholds: {lines: 60}`

#### Scenario: Schema rejects invalid overrides entry

- **WHEN** an overrides entry is missing `glob` field: `{"test": {"coverage": {"overrides": [{"thresholds": {"lines": 60}}]}}}`
- **THEN** validation fails with a ZodError indicating that `glob` is required

#### Scenario: Schema rejects invalid overrides thresholds value

- **WHEN** a config object `{"test": {"coverage": {"overrides": [{"glob": "demo/**", "thresholds": {"lines": "high"}}]}}}` is validated
- **THEN** validation fails with a ZodError indicating that `lines` must be a number

#### Scenario: Schema rejects invalid tests.frameworks entry

- **WHEN** a config object `{"test": {"frameworks": [{"glob": "**/*.test.ts"}]}}` is validated (missing `framework` field)
- **THEN** validation fails with a ZodError indicating the required field

#### Scenario: Schema accepts frameworks as a single string

- **WHEN** a config object `{"test": {"frameworks": "vitest"}}` is validated against the schema
- **THEN** validation succeeds and `test.frameworks` contains the string value `"vitest"`

#### Scenario: Schema accepts frameworks as an array of objects (unchanged behavior)

- **WHEN** a config object `{"test": {"frameworks": [{"glob": "**/*.test.ts", "framework": "vitest"}]}}` is validated
- **THEN** validation succeeds and `test.frameworks` contains the expected array of objects

#### Scenario: Schema rejects invalid framework name string

- **WHEN** a config object `{"test": {"frameworks": "mocha"}}` is validated with a string value that is not in the valid set (`jest`, `vitest`, `vite-plus`, `bun`, `rust`, `node-test`, `go`, `pytest`)
- **THEN** validation fails with a ZodError indicating that the framework name must be one of the valid values

#### Scenario: Schema accepts new framework names node-test, go, and pytest

- **WHEN** a config object `{"test": {"frameworks": "node-test"}}` is validated
- **THEN** validation succeeds and `test.frameworks` contains the string value `"node-test"`

#### Scenario: Schema accepts go as framework string

- **WHEN** a config object `{"test": {"frameworks": "go"}}` is validated
- **THEN** validation succeeds and `test.frameworks` contains the string value `"go"`

#### Scenario: Schema accepts pytest as framework string

- **WHEN** a config object `{"test": {"frameworks": "pytest"}}` is validated
- **THEN** validation succeeds and `test.frameworks` contains the string value `"pytest"`

#### Scenario: Schema rejects non-string non-array value for frameworks

- **WHEN** a config object `{"test": {"frameworks": 123}}` is validated
- **THEN** validation fails with a ZodError indicating that the value must be either a string or an array

## Module Contract

### Schema: testFrameworkSchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Type** | `z.enum([...])` |
| **Valid values** | `"jest"`, `"vitest"`, `"vite-plus"`, `"bun"`, `"rust"`, `"node-test"`, `"go"`, `"pytest"` |
| **Export** | Named export `testFrameworkSchema` |

### Schema: configSchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Type** | `z.ZodObject<...>` |
| **Definition** | Object schema with `schema` (literal `"spec-driven"`, default), `context` (optional string), `rules` (optional object with optional `proposal` and `tasks` string arrays), `test` (optional object with optional `frameworks` accepting either a single framework name string — validated via `testFrameworkSchema` — or an array of `{glob, framework}` objects, and optional `coverage` object with `thresholds: {lines, branches, functions}` defaults 80/70/75 and optional `overrides` array of `{glob, thresholds}`), `.passthrough()` for additional keys |
| **Export** | Named export `configSchema` |

### Type: TestFrameworks

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Definition** | `NonNullable<OpenSpecConfig['test']['framework']>` — union of eight framework name literals |
| **Export** | Named type export |

---

### Requirement: config-schema defines write_protection sub-schema

The `configSchema` in `plugins/dev-team/bin/src/schemas/config/config.schema.ts` SHALL be extended with a new `write_protection` field.

The field SHALL be optional at the top level.

The field SHALL use a `writeProtectionSchema` object with a single optional `files` array property.

Each entry in the `files` array SHALL be an object with:
- `glob` (string, required, nonempty) — glob pattern matching protected files
- `reason` (string, optional) — custom denial reason with `%s` (file path) and `%t` (tool name) placeholders

The entry schema SHALL use `.passthrough()` to preserve unknown fields, matching the pattern of other config sub-objects.

#### Scenario: configSchema includes write_protection as optional field

**WHEN** inspecting `configSchema.shape`
**THEN** `write_protection` exists as an optional key

#### Scenario: write_protection field accepts valid configuration

**WHEN** a config object `{ "write_protection": { "files": [{"glob": "*.env"}] } }` is validated against configSchema
**THEN** validation succeeds and the parsed output contains `write_protection.files[0].glob` with value `"*.env"`

#### Scenario: write_protection field type rejects non-object value

**WHEN** a config object `{ "write_protection": "invalid" }` is validated
**THEN** validation fails with a ZodError

### Requirement: OpenSpecConfig type reflects write_protection

The TypeScript type `OpenSpecConfig` (inferred from `configSchema`) SHALL include the `write_protection` field as optional with the correct nested type.

#### Scenario: OpenSpecConfig type includes write_protection

**WHEN** inspecting the `OpenSpecConfig` type
**THEN** `write_protection` is an accessible optional property

## Module Contract (write_protection additions)

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
