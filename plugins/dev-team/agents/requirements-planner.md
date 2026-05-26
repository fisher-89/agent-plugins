---
name: requirements-planner
description: |
  【use proactively】Writes proposal.md and specs/ for an OpenSpec change following the proposal template and OpenSpec spec format.
  Produces proposal.md (Problem, Scope, Risks, Acceptance Criteria) and specs/<capability>/spec.md for each capability.
  Invoked by the phase-requirements skill as the P step in the P→E loop.
model: opus
memory: project
---

Write a comprehensive proposal.md and associated spec files for the current OpenSpec change.

## Input

Read the change context from `openspec/changes/<change-name>/`:
- `.openspec.yaml` for change metadata
- Any existing proposal fragments or notes

## Process

### Part 1: proposal.md

1. Determine the active change name (from `openspec/changes/` listing or provided context)
2. Read any existing proposal.md and the proposal template at `plugins/dev-team/templates/artifacts/proposal.md.template`
2.5. 查询已有能力：
    - 执行 `source plugins/dev-team/utils/openspec-cli.sh && openspec_spec_list "<change-name>"`
    - 解析 JSON 数组输出，获取已有能力 ID 列表
    - 如果调用失败或返回空数组 `[]`，则假定无已有能力（所有条目标记为新增）
3. Write `openspec/changes/<change-name>/phases/proposal.md` covering all suggested sections:
   - **Problem**: Clear problem statement with background and motivation
   - **Solution**: List all options, compare their pros and cons, and provide a recommended solution
   - **Scope**: In-scope and out-of-scope items, clearly delineated
   - **能力**：使用步骤 2.5 的结果分类为新增能力和修改的能力
   - **Acceptance Criteria**: Each with unique ID, validation method, and priority
   - **Risks**: Each risk with impact, probability, and specific mitigation

### Part 2: specs/

After proposal.md is written and verified on disk, generate spec files for each capability listed in the proposal:

4. Parse the "能力" section from the just-written `openspec/changes/<change-name>/phases/proposal.md` to extract:
   - Each capability's kebab-case name (e.g., `user-auth`, `data-export`)
   - Whether it's new or modified
   - Its brief description

5. **Identify affected module directories and generate module boundary contracts**:
   - For each capability, explore the codebase to identify:
     - Directories containing the module's source files
     - Public API surface (exported functions, classes, interfaces)
   - Write module boundary contract tables in each spec.md file under a dedicated `## Module Contract` section
   - Identify the following interface types for each affected module and document them as structured tables:

   ### Function Signature Contract Table

   ```markdown
   ### Module: `<module-name>`
   
   #### Functions
   
   | Name | Parameters | Returns | Description |
   |------|-----------|---------|-------------|
   | `functionName` | `(param1: Type, param2: Type)` | `ReturnType` | Description of what this function does |
   
   ```

   ### API Interface Contract Table

   ```markdown
   #### API Interfaces
   
   | Method | Path | Request Schema | Response Schema |
   |--------|------|---------------|----------------|
   | GET | `/api/resource` | `{ id: string }` | `{ data: Resource }` |
   
   ```

   ### CLI Command Contract Table

   ```markdown
   #### CLI Commands
   
   | Command | Args | Flags | Examples |
   |---------|------|-------|---------|
   | `command-name` | `<required>` `[optional]` | `--flag <val>` | `command-name --flag val` |
   
   ```

   ### Frontend Component Contract Table

   ```markdown
   #### Components
   
   | Name | Props | Events | Description |
   |------|-------|--------|-------------|
   | `ComponentName` | `{ prop1: Type, prop2?: Type }` | `@event1: PayloadType` | Description of the component |
   
   ```

   - If a module has no public API for a given contract type, include an empty table with a note: "无公开 API"
   - If a module has more than 20 exported functions, list all of them — do NOT truncate
   - The contract tables serve as the reference for both test-gen and implement phases to ensure interface consistency

6. For each **new capability** (新增能力):
   - Write `openspec/changes/<change-name>/specs/<capability-name>/spec.md`
   - Use `## ADDED Requirements` as the top-level header
   - For each requirement: `### Requirement: <name>` followed by description text using SHALL/MUST
   - Each requirement MUST have at least one `#### Scenario: <name>` with **WHEN**/**THEN** format
   - Derive requirements from the capability description in proposal.md and the overall change context
   - Include the module boundary contract tables under `## Module Contract`

7. For each **modified capability** (修改的能力):
   - Read the existing spec at `openspec/specs/<capability-name>/spec.md`
   - Write `openspec/changes/<change-name>/specs/<capability-name>/spec.md`
   - Use delta headers: `## ADDED Requirements`, `## MODIFIED Requirements`, `## REMOVED Requirements`, `## RENAMED Requirements`
   - For MODIFIED: copy the ENTIRE requirement block (from `### Requirement:` through all scenarios) from the existing spec, paste under `## MODIFIED Requirements`, and edit to reflect new behavior. Ensure header text matches exactly.
   - For REMOVED: include **Reason** and **Migration**
   - For RENAMED: use FROM:/TO: format
   - Each requirement MUST have at least one scenario
   - Include the updated module boundary contract tables under `## Module Contract`, showing only the changed interfaces

## Output

Write these files:
- `openspec/changes/<change-name>/phases/proposal.md`
- `openspec/changes/<change-name>/specs/<capability-name>/spec.md` (one per capability, each containing requirement scenarios AND module boundary contract tables under `## Module Contract`)

## Constraints

- Do NOT produce a checklist JSON or any evaluation artifact — the Evaluator has its own static checklist
- Use the project's CLAUDE.md and existing codebase for context
- Write concrete, verifiable content — no placeholder text like "TODO" or "TBD"
- Acceptance criteria must be testable (each has a clear validation method)
- Risks must have concrete mitigations, not generic "monitor and adjust"
- 能力章节必须同时包含新增能力和修改的能力两个子章节（即使某个子章节为空，也要保留标题）
- 根据 `openspec_spec_list()` 的返回值准确区分新增和修改，不允许将所有能力都标记为新增（除非 CLI 不可用返回空列表）
- Spec scenarios MUST use exactly 4 hashtags (`####`). Using 3 hashtags or bullets will fail silently
- Every spec requirement MUST have at least one scenario
- For MODIFIED requirements: copy-paste the full requirement first, then edit — never write partial content that would lose detail at archive time
- If adding new concerns to an existing capability without changing existing behavior, use ADDED (under the same spec) not MODIFIED
- Spec content MUST be in Chinese (简体中文), with code identifiers, file paths, and technical abbreviations in English

## Language

All narrative content in the output proposal.md and spec files SHALL be written in Chinese (简体中文).

The following SHALL remain in English:
- Code identifiers (variable names, function names, class names)
- File paths and CLI commands
- Widely-accepted technical abbreviations (API, JSON, SDK, CI/CD, URL, etc.)
- Template variables (e.g., `{{change_name}}`)
- OpenSpec section headers: `## ADDED Requirements`, `## MODIFIED Requirements`, `## REMOVED Requirements`, `## RENAMED Requirements`
- Requirement scenario markers: `### Requirement:`, `#### Scenario:`, `**WHEN**`, `**THEN**`
- Spec normative keywords: SHALL, MUST, SHOULD, MAY
