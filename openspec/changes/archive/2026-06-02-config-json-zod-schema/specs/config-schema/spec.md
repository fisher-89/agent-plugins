## ADDED Requirements

### Requirement: config-schema defines a Zod schema for config.json

A Zod schema SHALL be defined at `plugins/dev-team/bin/src/schemas/config.schema.ts` that describes the structure of `openspec/config.json`.
The schema SHALL use `zod/v4` for schema definitions.
The schema SHALL define the following top-level fields:
- `schema`: a string literal type `"spec-driven"` with a default value of `"spec-driven"`
- `context`: an optional string field
- `rules`: an optional object with optional `proposal` (string array) and `tasks` (string array) fields
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

### Requirement: config-schema provides a parseConfig validation function

The module SHALL export a `parseConfig(data: unknown): OpenSpecConfig` function that validates and returns a typed config object.
The function SHALL call `configSchema.parse()` internally and re-throw ZodError with a descriptive message prefix.
The function SHALL return a deep-cloned object to prevent mutation of the validation result.

#### Scenario: parseConfig returns typed object on valid input

- **WHEN** `parseConfig({"schema": "spec-driven"})` is called
- **THEN** the return value has type `OpenSpecConfig` with `schema` equal to `"spec-driven"`

#### Scenario: parseConfig throws on invalid input

- **WHEN** `parseConfig({"schema": null})` is called
- **THEN** a ZodError is thrown with an error message that includes the field path `schema`

### Requirement: config-schema exports safeParseConfig for non-throwing validation

The module SHALL export a `safeParseConfig(data: unknown): { success: true; data: OpenSpecConfig } | { success: false; error: ZodError }` function.
This function SHALL use `configSchema.safeParse()` internally and never throw.

#### Scenario: safeParseConfig returns success on valid input

- **WHEN** `safeParseConfig({"schema": "spec-driven"})` is called
- **THEN** the result has `success: true` and `data.schema` is `"spec-driven"`

#### Scenario: safeParseConfig returns error on invalid input

- **WHEN** `safeParseConfig({"schema": false})` is called
- **THEN** the result has `success: false` and `error` is a `ZodError`

## Module Contract

### Schema: configSchema

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config.schema.ts` |
| **Type** | `z.ZodObject<...>` |
| **Definition** | Object schema with `schema` (literal `"spec-driven"`, default), `context` (optional string), `rules` (optional object with optional `proposal` and `tasks` string arrays), `.passthrough()` for additional keys |
| **Export** | Named export `configSchema` |

### Function: parseConfig

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config.schema.ts` |
| **Signature** | `parseConfig(data: unknown): OpenSpecConfig` |
| **Input** | `data` — raw config data (typically parsed from JSON file) |
| **Output** | `OpenSpecConfig` — parsed and validated config object |
| **Behavior** | Validates `data` against `configSchema`, throws `ZodError` on failure with descriptive message prefix |

### Function: safeParseConfig

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config.schema.ts` |
| **Signature** | `safeParseConfig(data: unknown): SafeParseResult<OpenSpecConfig>` |
| **Input** | `data` — raw config data |
| **Output** | `SafeParseResult<OpenSpecConfig>` — tagged union with `success: true` and `data`, or `success: false` and `error` |
| **Behavior** | Uses `configSchema.safeParse()` internally, never throws |

### Type: OpenSpecConfig

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config.schema.ts` |
| **Definition** | `z.infer<typeof configSchema>` — TypeScript type derived from Zod schema |
| **Export** | Named type export |
