## ADDED Requirements

### Requirement: backtrack_reason field in eval.json schema

The `phaseLogSchema` in `phase-log.schema.ts` SHALL include an optional `backtrack_reason` field of type `z.string().max(500).optional().nullable()`. This field records the reason for a backtrack when `backtrack_to` is set.

The `phaseLogInputSchema` SHALL also include `backtrack_reason` (picked from `phaseLogSchema` or explicitly added) so that the MCP tool input accepts the field.

Old eval.json entries without the `backtrack_reason` field SHALL parse without error — the field defaults to `null`/`undefined` after Zod parsing.

#### Scenario: New entry with backtrack_reason passes schema validation

- **WHEN** `phaseLogSchema.parse({ phase: "proposal", verdict: "pass", report: "...", checklist: [], backtrack_to: "proposal", backtrack_reason: "设计文档缺少 API 契约定义" })` is called
- **THEN** the parsed result includes `backtrack_reason: "设计文档缺少 API 契约定义"`

#### Scenario: Entry without backtrack_reason parses without error (backward compat)

- **WHEN** `phaseLogSchema.parse({ phase: "proposal", verdict: "pass", report: "...", checklist: [], backtrack_to: "proposal" })` is called
- **THEN** no error is thrown

#### Scenario: backtrack_reason longer than 500 characters is rejected

- **WHEN** `phaseLogSchema.parse({ phase: "proposal", verdict: "fail", report: "...", checklist: [], backtrack_to: "proposal", backtrack_reason: "x".repeat(501) })` is called
- **THEN** a Zod error is thrown indicating `backtrack_reason` exceeds the maximum length

### Requirement: phase_log validates backtrack_reason when backtrack_to is set

The `runPhaseLog()` function in `phase-log.ts` SHALL validate that when `backtrack_to` is non-null and non-empty, `backtrack_reason` MUST be a non-empty string. If `backtrack_reason` is missing, undefined, null, or an empty string, the function SHALL throw an error clearly indicating that `backtrack_reason` is required when backtracking.

When `backtrack_to` is null or undefined (no backtrack), `backtrack_reason` SHALL be silently ignored — no validation error and no write into the eval.json entry.

#### Scenario: backtrack_to is set but backtrack_reason is missing causes error

- **WHEN** `runPhaseLog({ phase: "dev-design", backtrack_to: "proposal", report: "...", checklist: [...] })` is called without `backtrack_reason`
- **THEN** an error is thrown with message containing "backtrack_reason"

#### Scenario: backtrack_to is set but backtrack_reason is empty string causes error

- **WHEN** `runPhaseLog({ phase: "dev-design", backtrack_to: "proposal", backtrack_reason: "", report: "...", checklist: [...] })` is called
- **THEN** an error is thrown with message containing "backtrack_reason"

#### Scenario: backtrack_to is null ignores backtrack_reason validation

- **WHEN** `runPhaseLog({ phase: "implement", backtrack_to: null, backtrack_reason: null, report: "...", checklist: [...] })` is called
- **THEN** no error is thrown and the entry is written without `backtrack_reason` field

#### Scenario: backtrack_to is array with reason passes validation

- **WHEN** `runPhaseLog({ phase: "test-gen", backtrack_to: ["test-design", "dev-design"], backtrack_reason: "测试覆盖率不足且在设计中遗漏了边界情况", report: "...", checklist: [...] })` is called
- **THEN** no error is thrown and the entry includes `backtrack_reason`

### Requirement: buildEntry() passes through backtrack_reason

The `BuildEntryParams` type in `eval-json.ts` SHALL include `backtrack_reason` as an optional field. The `buildEntry()` function SHALL include `backtrack_reason` in the output entry when it is explicitly provided, otherwise the field SHALL be omitted (defaulting to `undefined` which Zod parses as `null`/`undefined`).

#### Scenario: buildEntry with backtrack_reason includes it in output

- **WHEN** `buildEntry({ phase: "proposal", backtrack_to: "proposal", backtrack_reason: "需要重新审视需求范围", ... })` is called
- **THEN** the returned entry has `backtrack_reason` set to the provided value

#### Scenario: buildEntry without backtrack_reason omits the field

- **WHEN** `buildEntry({ phase: "proposal", backtrack_to: null, ... })` is called without `backtrack_reason`
- **THEN** the returned entry either omits `backtrack_reason` or has it as `null`

### Requirement: getLatestBacktrackInfo() returns reason alongside target

The `phase-next.ts` SHALL replace `getLatestBacktrackTarget()` with `getLatestBacktrackInfo()`. This new function SHALL return an object `{ target: string | string[] | null, reason: string | null }`.

- `target` SHALL follow the same logic as the current `getLatestBacktrackTarget()`: return the `backtrack_to` value of the latest entry if non-null/non-empty, otherwise null.
- `reason` SHALL return the `backtrack_reason` value of that same latest entry if present, otherwise null.

#### Scenario: getLatestBacktrackInfo returns target and reason from latest entry

- **WHEN** eval.json contains entries including one with `{ backtrack_to: "proposal", backtrack_reason: "设计变更" }` as the latest entry
- **THEN** `getLatestBacktrackInfo()` returns `{ target: "proposal", reason: "设计变更" }`

#### Scenario: getLatestBacktrackInfo returns reason null when entry has no backtrack_reason

- **WHEN** eval.json contains entries including one with `{ backtrack_to: "proposal" }` (no `backtrack_reason` field) as the latest entry
- **THEN** `getLatestBacktrackInfo()` returns `{ target: "proposal", reason: null }`

#### Scenario: getLatestBacktrackInfo returns both null when no backtrack

- **WHEN** eval.json has entries with all `backtrack_to: null`
- **THEN** `getLatestBacktrackInfo()` returns `{ target: null, reason: null }`

#### Scenario: getLatestBacktrackInfo returns reason null for empty eval.json

- **WHEN** eval.json is empty
- **THEN** `getLatestBacktrackInfo()` returns `{ target: null, reason: null }`

### Requirement: phase_next propagates backtrack_reason into prompts

The `buildPhaseDef()` function or the backtrack handling logic in `phase-next.ts` SHALL append the backtrack reason to both **planner** and **evaluator** prompts when a backtrack is detected.

The appended text SHALL follow this format: `\n\n⚠️ 回溯原因: <reason>\n请根据以上原因调整输出。`

If the backtrack entry has no `backtrack_reason` (null), no reason text SHALL be appended to the prompt — the prompt SHALL remain unchanged from current behavior.

#### Scenario: Planner prompt includes backtrack reason when backtrack detected

- **WHEN** eval.json has `backtrack_to: "dev-design"` and `backtrack_reason: "设计文档缺少 API 契约定义"`, and `runPhaseNext()` detects a backtrack to dev-design
- **THEN** the returned `planner.prompt` ends with `⚠️ 回溯原因: 设计文档缺少 API 契约定义`

#### Scenario: Evaluator prompt includes backtrack reason when backtrack detected

- **WHEN** eval.json has `backtrack_to: "dev-design"` and `backtrack_reason: "设计文档缺少 API 契约定义"`, and `runPhaseNext()` detects a backtrack to dev-design
- **THEN** the returned `evaluator.prompt` ends with `⚠️ 回溯原因: 设计文档缺少 API 契约定义`

#### Scenario: Backtrack without reason does not append reason text

- **WHEN** eval.json has `backtrack_to: "proposal"` but no `backtrack_reason` field
- **THEN** the returned `planner.prompt` and `evaluator.prompt` do NOT contain `⚠️ 回溯原因:`

#### Scenario: Array backtrack with reason propagates to prompt

- **WHEN** eval.json has `backtrack_to: ["test-design", "dev-design"]` and `backtrack_reason: "多个 phase 需要调整"`
- **THEN** the returned `planner.prompt` ends with `⚠️ 回溯原因: 多个 phase 需要调整`

## Module Contract

### phase-log.schema.ts

| Export                | Change                   | Purpose                                                                               |
| --------------------- | ------------------------ | ------------------------------------------------------------------------------------- |
| `phaseLogSchema`      | ADDED `backtrack_reason` | Optional field `z.string().max(500).optional().nullable()` to record backtrack reason |
| `phaseLogInputSchema` | ADDED `backtrack_reason` | Picked from `phaseLogSchema`, available in MCP tool input                             |

### eval-json.ts (lib/)

| Export             | Change   | Purpose                                               |
| ------------------ | -------- | ----------------------------------------------------- |
| `BuildEntryParams` | MODIFIED | Added `backtrack_reason` as optional field            |
| `buildEntry()`     | MODIFIED | Passes through `backtrack_reason` to the output entry |

### phase-log.ts (commands/)

| Export          | Change   | Purpose                                                                                 |
| --------------- | -------- | --------------------------------------------------------------------------------------- |
| `runPhaseLog()` | MODIFIED | Validates that `backtrack_reason` is a non-empty string when `backtrack_to` is non-null |

### phase-next.ts (commands/)

| Export                       | Change   | Purpose                                                                                                  |
| ---------------------------- | -------- | -------------------------------------------------------------------------------------------------------- |
| `getLatestBacktrackTarget()` | REMOVED  | Replaced by `getLatestBacktrackInfo()`                                                                   |
| `getLatestBacktrackInfo()`   | ADDED    | Returns `{ target: string \| string[] \| null, reason: string \| null }`                                 |
| `handleBacktrack()`          | MODIFIED | Uses `getLatestBacktrackInfo()` instead of `getLatestBacktrackTarget()`, passes reason to prompt builder |
| `buildPhaseDef()`            | MODIFIED | Appends backtrack reason text to planner prompt when reason is available                                 |
