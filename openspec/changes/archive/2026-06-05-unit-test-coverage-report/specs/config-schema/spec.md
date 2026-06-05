## MODIFIED Requirements

### Requirement: config-schema defines a Zod schema for config.json

A Zod schema SHALL be defined at `plugins/dev-team/bin/src/schemas/config.schema.ts` that describes the structure of `openspec/config.json`.
The schema SHALL use `zod/v4` for schema definitions.
The schema SHALL define the following top-level fields:
- `schema`: a string literal type `"spec-driven"` with a default value of `"spec-driven"`
- `context`: an optional string field
- `rules`: an optional object with optional `proposal` (string array) and `tasks` (string array) fields
- `test`: an optional object with the following sub-fields:
  - `frameworks`: an optional field accepting either a single framework name string (valid values: `"jest"`, `"vitest"`, `"vite-plus"`, `"bun"`, `"rust"`) or an array of `{glob: string, framework: string}` objects, defining test framework detection rules
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

- **WHEN** a config object `{"test": {"frameworks": "mocha"}}` is validated with a string value that is not in the valid set (`jest`, `vitest`, `vite-plus`, `bun`, `rust`)
- **THEN** validation fails with a ZodError indicating that the framework name must be one of the valid values

#### Scenario: Schema rejects non-string non-array value for frameworks

- **WHEN** a config object `{"test": {"frameworks": 123}}` is validated
- **THEN** validation fails with a ZodError indicating that the value must be either a string or an array

## ADDED Requirements

### Requirement: config-schema supports runtime lookup of framework config via MCP tools

The config-schema module SHALL NOT directly expose framework command details. Framework commands SHALL be managed by a hardcoded registry in the MCP tool implementation (`test_get_framework_config`), NOT in the config schema. The config schema's role is limited to providing the `test.frameworks` mapping (glob -> framework name) and `test.coverage` (thresholds + overrides for coverage gating).

This separation ensures:
- The config schema remains simple and stable
- Framework-specific command details (which may change with tool versions) are maintained in code
- Users only need to declare which frameworks they use, not the commands to run them

#### Scenario: Config schema only carries glob-to-framework mapping

- **WHEN** a user declares `{"test": {"frameworks": [{"glob": "**/*.test.ts", "framework": "vitest"}]}}`
- **THEN** the config schema validates this structure
- **AND** the actual command `npx vitest run --coverage` is resolved by `test_get_framework_config` at runtime, not from config.json

#### Scenario: Config schema provides thresholds and overrides for evaluator gating

- **WHEN** a user sets `{"test": {"coverage": {"thresholds": {"lines": 90, "branches": 80}, "overrides": [{"glob": "demo/**", "thresholds": {"lines": 60}}]}}}`
- **THEN** the thresholds and overrides are accessible via `config_get` tool and are used by the executor to determine `coverage_pass`

## Module Contract

### Schema: configSchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config.schema.ts` |
| **Type** | `z.ZodObject<...>` |
| **Definition** | Object schema with `schema` (literal `"spec-driven"`, default), `context` (optional string), `rules` (optional object with optional `proposal` and `tasks` string arrays), `test` (optional object with optional `frameworks` accepting either a single framework name string — validated via `z.enum(["jest", "vitest", "vite-plus", "bun", "rust"])` — or an array of `{glob, framework}` objects, and optional `coverage` object with `thresholds: {lines, branches, functions}` defaults 80/70/75 and optional `overrides` array of `{glob, thresholds}`), `.passthrough()` for additional keys |
| **Export** | Named export `configSchema` |

### Type: OpenSpecConfig

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config.schema.ts` |
| **Definition** | `z.infer<typeof configSchema>` — TypeScript type derived from Zod schema |
| **Export** | Named type export |
