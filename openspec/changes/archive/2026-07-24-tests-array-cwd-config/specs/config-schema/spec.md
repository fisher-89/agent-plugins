## REMOVED Requirements

### Requirement: JSON schema mirrors Zod schema for exclude fields

**Reason**: 顶层 `test.exclude` 与 `test.overrides[].exclude` 随旧 `test` 对象一并移除；排除规则改为 suite 级 `tests[].excludes`。

**Migration**: 使用 `tests[].excludes`（相对该 suite 的 `root`）。JSON Schema 在 `tests.items.properties.excludes` 下描述。

### Requirement: OpenSpecConfig type reflects exclude field

**Reason**: `OpenSpecConfig['test']` 不再存在；排除字段迁移到 suite。

**Migration**: 使用 `OpenSpecConfig['tests'][number]['excludes']`。

---

## MODIFIED Requirements

### Requirement: config-schema defines a Zod schema for config.json

A Zod schema SHALL be defined at `plugins/dev-team/bin/src/schemas/config/config.schema.ts` that describes the structure of `openspec/config.json`.
The schema SHALL use `zod/v4` for schema definitions.
The schema SHALL define the following top-level fields:
- `schema`: a string literal type `"spec-driven"` with a default value of `"spec-driven"`
- `context`: an optional string field
- `rules`: an optional object with optional `proposal` (string array) and `tasks` (string array) fields
- `tests`: an optional array of **test suite** objects (default `[]` when omitted via `prefault`). Each suite object SHALL define:
  - `root` (required, nonempty string): suite anchor path relative to projectRoot；MUST NOT contain glob wildcard characters `*`, `?`, `{`, or `[`
  - `framework` (required): validated via `testFrameworkSchema`（`"jest"` \| `"vitest"` \| `"vite-plus"` \| `"bun"` \| `"rust"` \| `"node-test"` \| `"go"` \| `"pytest"`）
  - `cwd` (optional string, prefault `"."`): execution directory relative to `root`
  - `config` (optional nonempty string): framework config file path relative to `root`
  - `includes` (optional `string[]`): include globs relative to `root`；when omitted, consumers SHALL treat the suite includes as the framework `default_glob`（not a schema-injected glob string）
  - `excludes` (optional `string[]`): exclude globs relative to `root`
  - `coverage` (optional object): `lines` / `branches` / `functions` numbers in `[0, 100]`，各字段缺省为 schema 目录常量（默认 80 / 70 / 75）
  - `mutation` (optional object): `score` number in `[0, 100]`，缺省为 schema 目录常量（默认 70）
- The schema SHALL NOT define a top-level `test` object with `framework` / `overrides` / global `coverage` / `mutation` / `exclude` as the supported configuration model

Coverage / mutation numeric defaults SHALL be imported from a dedicated constants module under `plugins/dev-team/bin/src/schemas/config/`（例如 `defaults.ts`），不得在消费者中手写级联默认值。

The schema SHALL allow additional unknown fields via `.passthrough()` to avoid rejecting valid configurations with tool-managed keys.
A TypeScript type `OpenSpecConfig` SHALL be exported, derived from the schema using `z.infer` / `z.output`.

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
- **THEN** `OpenSpecConfig` type exists and is assignable to the inferred schema output type

#### Scenario: Minimal suite with root and framework is accepted

- **WHEN** a config object `{"tests": [{"root": "plugins/dev-team/bin", "framework": "vite-plus"}]}` is validated
- **THEN** validation succeeds
- **AND** parsed suite `cwd` SHALL be `"."`
- **AND** parsed suite `coverage.lines` / `branches` / `functions` SHALL equal schema defaults 80 / 70 / 75
- **AND** parsed suite `mutation.score` SHALL equal schema default 70

#### Scenario: Schema rejects root containing glob wildcards

- **WHEN** a config object `{"tests": [{"root": "src/**", "framework": "vitest"}]}` is validated
- **THEN** validation fails with a ZodError indicating that `root` MUST NOT contain wildcards

#### Scenario: Schema rejects suite missing framework

- **WHEN** a config object `{"tests": [{"root": "src"}]}` is validated
- **THEN** validation fails with a ZodError indicating that `framework` is required

#### Scenario: Schema accepts suite with cwd, config, includes, excludes

- **WHEN** a config object `{"tests": [{"root": "plugins/dev-team/bin", "framework": "vite-plus", "cwd": ".", "config": "vite.config.ts", "includes": ["src/**/*.{ts,tsx}"], "excludes": ["src/schemas/**/*"]}]}` is validated
- **THEN** validation succeeds and all provided fields are preserved in the parsed suite

#### Scenario: Schema accepts parent cwd

- **WHEN** a config object `{"tests": [{"root": "plugins/dev-team/bin/src", "cwd": "..", "framework": "vite-plus"}]}` is validated
- **THEN** validation succeeds and parsed `cwd` SHALL be `".."`

#### Scenario: Schema applies partial coverage defaults per field

- **WHEN** a config object `{"tests": [{"root": "src", "framework": "vitest", "coverage": {"lines": 90}}]}` is validated
- **THEN** the parsed suite `coverage` SHALL be `{lines: 90, branches: 70, functions: 75}`

#### Scenario: Schema rejects invalid framework name

- **WHEN** a config object `{"tests": [{"root": "src", "framework": "mocha"}]}` is validated
- **THEN** validation fails with a ZodError indicating the framework MUST be one of the valid enum values

#### Scenario: tests defaults to empty array when omitted

- **WHEN** a config object `{}` is validated
- **THEN** the parsed output `tests` SHALL be `[]`

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

---

## ADDED Requirements

### Requirement: Schema constants module for test thresholds

覆盖率与变异阈值的默认数值 SHALL 定义在 `plugins/dev-team/bin/src/schemas/config/` 目录下的专用常量模块中（例如 `defaults.ts`），并由 `config.schema.ts` 的 suite `coverage` / `mutation` `prefault` 引用。消费者（`test-report`、`test-detect-frameworks` 等）SHALL 使用 schema parse 后的 suite 字段，MUST NOT 再维护一份平行的全局默认级联。

#### Scenario: Constants are shared by schema prefaults

- **WHEN** inspecting the schema constants module
- **THEN** it SHALL export line/branch/function coverage defaults and mutation score default
- **AND** `config.schema.ts` SHALL import those constants for suite `coverage` / `mutation` defaults

#### Scenario: Parsed suite without coverage block still has numeric thresholds

- **WHEN** `{"tests": [{"root": "src", "framework": "vitest"}]}` is validated
- **THEN** parsed `tests[0].coverage` SHALL contain numeric `lines` / `branches` / `functions` from the constants module

### Requirement: JSON schema mirrors Zod tests array

`plugins/dev-team/bin/dev-team-config.schema.json` SHALL describe top-level `tests` as an array of suite objects with properties `root`, `framework`, `cwd`, `config`, `includes`, `excludes`, `coverage`, `mutation`。`root` 与 `framework` SHALL 出现在 suite `required` 中。旧的 `properties.test`（含 `framework` / `overrides` / 全局 `exclude` / `coverage` / `mutation`）SHALL NOT 再作为受支持的配置形状出现。

#### Scenario: JSON schema documents tests suite fields

- **WHEN** `dev-team-config.schema.json` is inspected
- **THEN** `properties.tests` exists with `type: "array"`
- **AND** `properties.tests.items.properties` includes `root`, `framework`, `cwd`, `config`, `includes`, `excludes`, `coverage`, `mutation`
- **AND** `properties.tests.items.required` includes `"root"` and `"framework"`

#### Scenario: JSON schema does not document legacy test object as supported shape

- **WHEN** `dev-team-config.schema.json` is inspected
- **THEN** there SHALL be no supported `properties.test.properties.overrides` suite model for new configs

### Requirement: OpenSpecConfig type reflects tests suites

`OpenSpecConfig` SHALL expose `tests` as an array of suite objects。TypeScript 访问路径 SHALL 为 `config.tests[number].root` / `.framework` / `.cwd` / `.config` / `.includes` / `.excludes` / `.coverage` / `.mutation`。SHALL NOT 再暴露作为正式模型的 `config.test.framework` / `config.test.overrides`。

#### Scenario: OpenSpecConfig includes tests array

- **WHEN** inspecting the `OpenSpecConfig` type
- **THEN** `tests` is an accessible array property
- **AND** each element includes required `root` and `framework` fields

---

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
| **Definition** | Top-level `tests: Suite[]`（optional/`prefault([])`）；suite 必填 `root`+`framework`；可选 `cwd`/`config`/`includes`/`excludes`/`coverage`/`mutation`；无受支持的旧 `test` 对象模型；`.passthrough()` 保留未知键 |
| **Export** | Named export `configSchema` |

### Module: schemas/config/defaults.ts（或等价常量文件）

| Property | Description |
|----------|-------------|
| **Exports** | `TEST_COVERAGE_LINE_DEFAULT`, `TEST_COVERAGE_BRANCH_DEFAULT`, `TEST_COVERAGE_FUNCTION_DEFAULT`, `TEST_MUTATION_SCORE_DEFAULT` |
| **Consumers** | `config.schema.ts` prefaults |

### Type: OpenSpecConfig

| Property | Description |
|----------|-------------|
| **Module** | `schemas/config/config.schema.ts` |
| **Definition** | `z.output<typeof configSchema>` |
| **Key path** | `tests: Array<{ root, framework, cwd, config?, includes?, excludes?, coverage, mutation }>` |
