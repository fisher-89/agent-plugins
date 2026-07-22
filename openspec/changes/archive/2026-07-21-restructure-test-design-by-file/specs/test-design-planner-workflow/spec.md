# test-design-planner-workflow Specification

## Purpose

Define the output instructions and generation logic for the test-design-planner agent, ensuring it produces test-design.md conforming to the per-file unit test structure and per-relationship integration test structure with free-form relationship titles.

## ADDED Requirements

### Requirement: Planner generates per-file unit test sections

The planner Process SHALL iterate over each source-test file pair discovered by `test_resolve_paths` and generate a separate `### <源文件> -> <测试文件>` subsection in the `## 单元测试` section. For each file pair, the planner SHALL produce:

1. A `#### 待测功能` list, derived by grepping the source file for exported function/class signatures
2. A `#### 用例` table with columns: `测试对象 | 路径类型 | 测试条件 | 迭代类型`, without a `测试文件` column
3. A `#### Mock策略` table with columns: `Mock主体 | Mock方案 | 应用场景`, without a `测试文件` column

#### Scenario: Planner output has per-file groupings

- **WHEN** the planner generates `test-design.md`
- **THEN** each discovered `source -> test_file` pair SHALL produce one `### <source> -> <test_file>` subsection
- **AND** each subsection SHALL contain exactly `#### 待测功能`, `#### 用例`, `#### Mock策略` in order

#### Scenario: 待测功能 lists are derived from source code

- **WHEN** the planner writes a `#### 待测功能` section
- **THEN** the list SHALL be populated by grepping the source file for exported function/class/variable declarations
- **AND** each entry SHALL follow the format `- <name>(): <short description>`

### Requirement: Planner generates free-form relationship sections for integration tests

The planner Process SHALL analyze proposal.md ACs and design.md architecture components to identify cross-module interactions that need integration test coverage. For each distinct interaction, the planner SHALL generate a `### <关系标题> → <测试文件>` subsection under `## 集成测试`.

The relationship title SHALL be a free-form, natural-language description using arrow notation to indicate interaction direction (e.g., `CLI参数 → eval.json持久化`). The title SHALL NOT be constrained to a fixed taxonomy.

#### Scenario: Relationship titles are free-form and descriptive

- **WHEN** the planner generates a `### <关系标题> → <测试文件>` heading
- **THEN** the title SHALL be a natural-language description of the cross-module interaction
- **AND** the title SHALL use arrow notation (`→`) to indicate data or control flow direction where applicable

#### Scenario: Each integration AC maps to at least one relationship

- **WHEN** a proposal.md AC is marked with testing type 集成测试
- **THEN** the planner SHALL ensure that AC appears in the `**关联AC**` of at least one relationship section
- **AND** the planner SHALL NOT leave any integration-test AC unmapped

### Requirement: Planner generates relationship metadata for each integration section

For each relationship section, the planner SHALL generate three metadata elements in order:

1. A `**涉及模块**` table with columns `模块 | 角色`, containing at least 2 rows identifying the involved modules and their roles in the interaction
2. A `**关联AC**` line listing the relevant AC IDs from proposal.md
3. A `**关系描述**` paragraph describing how the modules interact, why this interaction is worth testing, and possible failure modes

#### Scenario: 涉及模块 table has at least 2 entries

- **WHEN** the planner generates a `**涉及模块**` table
- **THEN** the table SHALL contain at least 2 rows (excluding header)
- **AND** each row SHALL specify a module path and its role in the interaction (e.g., 写入方, 读取方, 中间件, 触发方)

#### Scenario: 关联AC is non-empty and references valid ACs

- **WHEN** the planner generates the `**关联AC**` metadata line
- **THEN** it SHALL contain at least one valid AC ID from proposal.md
- **AND** each AC ID SHALL be referenced in `## 验收范围` table

#### Scenario: 关系描述 is a narrative paragraph

- **WHEN** the planner writes the `**关系描述**` section
- **THEN** it SHALL be a paragraph of free-form text
- **AND** SHALL NOT be a list, table, or template placeholder

### Requirement: Planner generates per-scenario structure under each relationship

For each relationship section, the planner SHALL generate one or more `#### 场景: <场景名称>` sub-subsections. Each scenario SHALL:

1. Begin with a narrative paragraph describing what the scenario validates, preconditions, input, and expected output
2. Contain a `##### 用例` table with columns: `路径类型 | 测试条件 | 迭代类型`
3. Optionally contain a `##### Mock策略` table with columns: `Mock主体 | Mock方案 | 应用场景`, only when cross-process mocks are needed

#### Scenario: At least one scenario per relationship

- **WHEN** the planner generates a relationship section
- **THEN** it SHALL contain at least one `#### 场景: <名称>` sub-subsection
- **AND** each scenario SHALL have a unique, descriptive name

#### Scenario: Each scenario has a narrative description

- **WHEN** the planner generates a `#### 场景: <名称>` heading
- **THEN** it SHALL be immediately followed by a narrative paragraph
- **AND** the paragraph SHALL address: what is being validated, preconditions, input, and expected output

#### Scenario: 用例 table under scenario has correct columns

- **WHEN** the planner generates a `##### 用例` table under a scenario
- **THEN** it SHALL have exactly 3 columns: `路径类型 | 测试条件 | 迭代类型`

#### Scenario: Mock策略 is cross-process only

- **WHEN** the planner determines a scenario needs mocking
- **THEN** it SHALL only include mocks for cross-process boundaries (external services, file system, network, environment variables)
- **AND** SHALL NOT mock internal module-to-module calls (those are covered by unit tests)
- **WHEN** no cross-process mocks are needed
- **THEN** the `##### Mock策略` section SHALL be omitted

### Requirement: Planner integrates with test_resolve_paths for unit test file pairs

The planner Process SHALL call `mcp__plugin_dev-team_dev-team__test_resolve_paths` with the `modules` parameter to discover unit test file pairs, then map each `source -> test_file` result to a per-file subsection in `## 单元测试`.

If `test_resolve_paths` returns errors for specific modules, the planner SHALL record those modules in `## 不可测试项`.

#### Scenario: test_resolve_paths output maps to per-file sections

- **WHEN** `test_resolve_paths` returns `unit_tests` entries
- **THEN** each entry SHALL generate exactly one `### <source> -> <test_file>` subsection

#### Scenario: Unresolvable modules logged to 不可测试项

- **WHEN** `test_resolve_paths` returns `errors` with non-empty entries
- **THEN** each error entry SHALL be recorded in `## 不可测试项` with the module path and reason

### Requirement: Planner integrates test_detect_frameworks for integration tests

The planner Process SHALL call `mcp__plugin_dev-team_dev-team__test_detect_frameworks` to identify the test framework, then use the framework conventions (file extension, test runner, assertion library) to determine integration test file placement and naming within `__tests__/` directories.

#### Scenario: Framework detection informs integration test file extension

- **WHEN** the planner generates the test file path in a relationship heading (`→ <test_file>`)
- **THEN** the test file extension SHALL be consistent with the detected framework (e.g., `.test.ts` for vitest, `.spec.ts` for jest)

### Requirement: Planner preserves 验收范围 table structure

The planner SHALL generate the `## 验收范围` table with the same 5 columns as the template: `AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景`. The per-file sections in unit test and per-relationship sections in integration test SHALL reference the same AC IDs and test files as this table.

#### Scenario: 验收范围 references match later sections

- **WHEN** the planner generates `## 验收范围`
- **THEN** every `AC ID` in the 验收范围 table SHALL have corresponding entries in either the `## 单元测试` or `## 集成测试` sections
- **AND** every `测试文件` in 验收范围 SHALL appear in at least one per-file or per-relationship section heading

## Module Contract

### Agent file: `plugins/dev-team/agents/test-design-planner.md`

| Process Step | Description |
|-------------|-------------|
| Step 6 | From proposal.md/design.md, identify cross-module interactions for integration testing and generate free-form relationship titles with arrow notation |
| Step 7 | For each relationship, identify involved modules, their roles, and generate `**涉及模块**` table, `**关联AC**`, and `**关系描述**` paragraph |
| Step 8 | For each relationship, generate one or more `#### 场景:` sub-subsections with narrative description, `##### 用例` table, and optional `##### Mock策略` (cross-process only) |
| Step 9 | Call `test_detect_frameworks` to determine test framework and file naming conventions |
| Step 10 | Call `test_resolve_paths` with `modules` parameter to discover unit test file pairs |
| Step 11 | Map `unit_tests` entries to per-file `### <source> -> <test_file>` subsections with `#### 待测功能`, `#### 用例`, `#### Mock策略` |
| Step 12 | Log unresolvable modules to `## 不可测试项` |
| Output | Write `test-design.md` following the new template structure |

### Dependencies

| Dependency | Purpose |
|------------|---------|
| `mcp__plugin_dev-team_dev-team__test_resolve_paths` | Discover unit test file pairs (source -> test_file) |
| `mcp__plugin_dev-team_dev-team__test_detect_frameworks` | Identify test framework for naming conventions |
| `plugins/dev-team/templates/artifacts/test-design.md.template` | Reference template for section/column structure |
| Proposal.md ACs and design.md architecture components | Source for identifying integration relationships and scenarios |
