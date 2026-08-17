## Module Contract

### Schema: mutationConfigSchema (`tests[].mutation.cwd`)

| Property | Description |
|----------|-------------|
| **File** | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` |
| **Field** | `mutation.cwd`：`z.string().optional()`，相对 `root` |
| **缺省** | 省略该字段（不 prefault 为 suite `cwd`）；消费者（`resolveSuite`）按 LCA 计算 `mutation_cwd` |
| **出现时** | 覆盖自动 LCA：`path.resolve(absRoot, mutation.cwd)` |
| **JSON Schema** | `plugins/dev-team/bin/dev-team-config.schema.json` 的 `mutation.properties.cwd.description` SHALL 与 Zod `describe` 一致 |

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
  - `mutation` (optional object): `cwd`（optional string，相对 `root`；出现时覆盖自动 LCA；省略时消费者按 `LCA(absRoot, absCwd, dirname(absConfig)?)` 计算 `mutation_cwd`，MUST NOT 再默认等于 suite `cwd`）与 `score` number in `[0, 100]`（缺省为 schema 目录常量，默认 70）
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
- **AND** parsed suite `mutation.cwd` SHALL be omitted（消费者缺省按 LCA 计算 `mutation_cwd`，MUST NOT 把省略当成 suite `cwd`）

#### Scenario: Schema accepts suite mutation.cwd

- **WHEN** a config object `{"tests": [{"root": "pkg", "framework": "vitest", "cwd": "jest", "mutation": {"cwd": ".", "score": 50}}]}` is validated
- **THEN** validation succeeds
- **AND** parsed suite `mutation.cwd` SHALL be `"."`
- **AND** parsed suite `mutation.score` SHALL be `50`

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

---

## ADDED Requirements

### Requirement: mutation.cwd 文档描述为覆盖自动 LCA

**ID**: REQ-CS-MUT-CWD-1
**Priority**: MUST
**Description**: Zod `mutationConfigSchema.cwd` 的 `.describe(...)` 与 `dev-team-config.schema.json` 中 `tests.items.properties.mutation.properties.cwd.description` SHALL 说明该字段为相对 `root` 的可选覆盖，用于覆盖 detect 侧自动 LCA（`LCA(absRoot, absCwd, dirname(absConfig)?)`）。SHALL NOT 再描述为「默认等于 cwd」。

字段仍为 optional、无字符串 prefault：省略时 parse 结果不写入 `mutation.cwd`；消费者不得把「缺省等于 suite cwd」当作 schema 契约。

#### Scenario: Zod describe 不再声称默认等于 cwd

**WHEN** 读取 `config.schema.ts` 中 `mutation.cwd` 的 describe 文案
**THEN** 文案 SHALL 表明该字段覆盖自动 LCA
**AND** SHALL NOT 使用「默认等于cwd」或等价表述作为缺省语义

#### Scenario: JSON schema description 与 Zod 一致

**WHEN** `dev-team-config.schema.json` 被检查
**THEN** `properties.tests.items.properties.mutation.properties.cwd.description` SHALL 表明覆盖自动 LCA
**AND** SHALL NOT 描述为默认等于 suite `cwd`
