# test-design-template Specification

## Purpose

Define the structural format of `test-design.md.template`, governing how unit test sections are organized per-source-file and integration test sections are organized per-relationship with free-form relationship titles.

## ADDED Requirements

### Requirement: Unit test section uses per-file grouping

The `## 单元测试` section in the template SHALL organize test content using `### {{源文件}} -> {{测试文件}}` as the repeating subsection heading, instead of a flat `### 用例` + `### Mock策略` structure.

Each file subsection SHALL contain exactly three sub-subsections in order: `#### 待测功能`, `#### 用例`, `#### Mock策略`.

#### Scenario: Template unit section has per-file headings

- **WHEN** the template is rendered
- **THEN** the `## 单元测试` section SHALL contain one or more `### <源文件路径> -> <测试文件路径>` subsections
- **AND** each such subsection SHALL contain `#### 待测功能`, `#### 用例`, and `#### Mock策略` in that order

#### Scenario: Source and test file paths are explicit

- **WHEN** a `### <源文件路径> -> <测试文件路径>` heading is present
- **THEN** both paths SHALL be relative to the project root
- **AND** the right-hand side SHALL be the colocated test file path

### Requirement: Unit test sub-subsections have correct format

The `#### 待测功能` subsection SHALL use an unordered list format with each entry as `- <functionName>(): <简短描述>`.

The `#### 用例` table SHALL have columns: `测试对象 | 路径类型 | 测试条件 | 迭代类型`. The `测试文件` column SHALL NOT appear.

The `#### Mock策略` table SHALL have columns: `Mock主体 | Mock方案 | 应用场景`. The `测试文件` column SHALL NOT appear.

#### Scenario: 待测功能 is a list, not a table

- **WHEN** the template is examined
- **THEN** `#### 待测功能` SHALL contain bullet-point entries like `- parseConfig(): 解析配置文件`
- **AND** SHALL NOT use a table format for this section

#### Scenario: 用例 table omits 测试文件 column

- **WHEN** the template is examined
- **THEN** the `#### 用例` table under a unit test file subsection SHALL have exactly 4 columns: `测试对象 | 路径类型 | 测试条件 | 迭代类型`
- **AND** SHALL NOT include a `测试文件` column

#### Scenario: Mock策略 table omits 测试文件 column

- **WHEN** the template is examined
- **THEN** the `#### Mock策略` table under a unit test file subsection SHALL have exactly 3 columns: `Mock主体 | Mock方案 | 应用场景`
- **AND** SHALL NOT include a `测试文件` column

### Requirement: Integration test section uses free-form relationship titles with test file reference

The `## 集成测试` section in the template SHALL organize test content using `### {{关系标题}} → {{测试文件}}` as the repeating subsection heading. The relationship title SHALL be a free-form description of the cross-module interaction, not constrained to a fixed taxonomy.

Each relationship subsection SHALL begin with three metadata elements in order: a `**涉及模块**` table, a `**关联AC**` line, and a `**关系描述**` paragraph.

#### Scenario: Template integration section uses free-form headings

- **WHEN** the template is rendered
- **THEN** the `## 集成测试` section SHALL contain one or more `### <关系标题> → <测试文件路径>` subsections
- **AND** the relationship title SHALL NOT be constrained to a fixed set of values

#### Scenario: Each relationship subsection has mandatory metadata elements

- **WHEN** a `### <关系标题> → <测试文件路径>` heading is present
- **THEN** it SHALL be immediately followed by a `**涉及模块**` table, a `**关联AC**:` line, and a `**关系描述**:` paragraph in that order
- **AND** all three metadata elements SHALL be present

### Requirement: 涉及模块 table enforces cross-module scope

The `**涉及模块**` table SHALL have exactly 2 columns: `模块 | 角色`. It SHALL contain at least 2 rows to enforce the cross-module nature of integration tests.

#### Scenario: 涉及模块 table has 模块 and 角色 columns

- **WHEN** the template is examined
- **THEN** the `**涉及模块**` table SHALL have columns: `模块 | 角色`
- **AND** the template comment SHALL indicate that at least 2 module entries are required

#### Scenario: Role column describes module participation

- **WHEN** the template is examined
- **THEN** the `角色` column SHALL contain descriptions of the module's role in the interaction (e.g., 写入方, 读取方, 中间件, 触发方)
- **AND** the template SHALL include example roles in its HTML comment

### Requirement: 关联AC links relationship to acceptance criteria

The `**关联AC**` line SHALL list one or more AC IDs from the proposal's acceptance criteria. It SHALL be non-empty for each relationship.

#### Scenario: 关联AC references valid AC IDs

- **WHEN** the template is examined
- **THEN** `**关联AC**` SHALL contain at least one AC ID reference (e.g., `AC-1, AC-2`)
- **AND** the AC IDs SHALL correspond to entries in `## 验收范围`

### Requirement: 关系描述 uses narrative paragraph

The `**关系描述**` paragraph SHALL be free-form narrative text describing how the involved modules interact, why this interaction is worth testing, and possible failure modes. It SHALL NOT be a list or table.

#### Scenario: 关系描述 is a narrative paragraph

- **WHEN** the template is examined
- **THEN** `**关系描述**` SHALL be a paragraph of free-form text
- **AND** SHALL NOT use list or table format

### Requirement: Integration test scenarios use per-scenario structure

Each relationship subsection SHALL contain one or more `#### 场景: {{场景名称}}` sub-subsections. Each scenario SHALL begin with a narrative paragraph describing what the scenario validates, its preconditions, input, and expected output.

#### Scenario: At least one scenario per relationship

- **WHEN** a `### <关系标题> → <测试文件>` subsection is present
- **THEN** it SHALL contain at least one `#### 场景: <场景名称>` sub-subsection

#### Scenario: Scenario description is a narrative paragraph

- **WHEN** a `#### 场景: <场景名称>` heading is present
- **THEN** it SHALL be immediately followed by a narrative paragraph describing the scenario
- **AND** the paragraph SHALL address: what is being validated, preconditions, input, and expected output

### Requirement: Integration scenario 用例 table uses correct columns

Each `#### 场景:` sub-subsection SHALL contain a `##### 用例` table with columns: `路径类型 | 测试条件 | 迭代类型`.

#### Scenario: 用例 table has 路径类型 column

- **WHEN** the template is examined
- **THEN** the `##### 用例` table under a `#### 场景:` heading SHALL have exactly 3 columns: `路径类型 | 测试条件 | 迭代类型`

### Requirement: Integration scenario Mock策略 is optional and cross-process only

A `##### Mock策略` table SHALL appear only when the scenario requires mocking cross-process boundaries (external services, file system, network, etc.). Internal module-to-module calls SHALL NOT be mocked.

When no cross-process mocks are needed, the `##### Mock策略` section SHALL be omitted and replaced with an HTML comment noting no mock is required.

The `##### Mock策略` table SHALL have columns: `Mock主体 | Mock方案 | 应用场景`.

#### Scenario: Mock策略 only covers cross-process boundaries

- **WHEN** the template is examined
- **THEN** the HTML comment for `##### Mock策略` SHALL state: "仅列出跨进程边界的Mock。内部模块间调用不mock。如无Mock需求，省略此小节并用注释说明。"

#### Scenario: Mock策略 table has correct columns

- **WHEN** a `##### Mock策略` table is present
- **THEN** it SHALL have exactly 3 columns: `Mock主体 | Mock方案 | 应用场景`

### Requirement: 验收范围 table structure is unchanged

The `## 验收范围` table SHALL retain its existing column set: `AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景`. This table SHALL NOT be modified by this change.

#### Scenario: 验收范围 columns unchanged

- **WHEN** the template is examined
- **THEN** the `## 验收范围` table SHALL have exactly 5 columns: `AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景`
- **AND** SHALL match the current column order and names

### Requirement: Template HTML comments document the structure

Each new or modified subsection heading in the template SHALL include an HTML comment (`<!-- ... -->`) immediately following the heading, explaining the content rules and placeholder semantics for that subsection.

#### Scenario: HTML comments present for key sections

- **WHEN** the template is examined
- **THEN** each of the following SHALL have an adjacent HTML comment block explaining its format and placeholders:
  - `## 集成测试` (comment explaining integration test vs unit test boundary)
  - `### <关系标题> → <测试文件>` (comment explaining free-form title convention)
  - `**涉及模块**` table (comment explaining module-role pairing)
  - `**关联AC**` (comment explaining AC ID format)
  - `**关系描述**` (comment explaining narrative expectations)
  - `#### 场景:` (comment explaining scenario description expectations)
  - `##### 用例` (comment explaining column semantics)
  - `##### Mock策略` (comment explaining cross-process-only rule)

## Module Contract

### Template file: `plugins/dev-team/templates/artifacts/test-design.md.template`

| Section | Description |
|---------|-------------|
| `## 验收范围` | Global AC-to-test mapping table (unchanged) |
| `## 单元测试` | Per-file test design sections, each containing `#### 待测功能` (list) + `#### 用例` (table: 测试对象\|路径类型\|测试条件\|迭代类型) + `#### Mock策略` (table: Mock主体\|Mock方案\|应用场景) |
| `## 集成测试` | Per-relationship sections with free-form `### <关系标题> → <测试文件>` headings, each containing `**涉及模块**` table (模块\|角色), `**关联AC**`, `**关系描述**` paragraph, and one or more `#### 场景:` sub-subsections with `##### 用例` table and optional `##### Mock策略` |
| `## 不可测试项` | List of out-of-scope items with reasons (unchanged) |
