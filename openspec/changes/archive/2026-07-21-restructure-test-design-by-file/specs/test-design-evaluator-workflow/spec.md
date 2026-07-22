# test-design-evaluator-workflow Specification

## Purpose

Define the evaluation checklist for test-design.md, ensuring the generated artifact conforms to the per-file unit test structure and per-relationship integration test structure with free-form relationship titles.

## ADDED Requirements

### Requirement: T8 format check covers per-file unit test structure

The T8 checklist item ("产物结构与模板格式一致") SHALL verify that every `###` subsection under `## 单元测试` follows the `{{源文件}} -> {{测试文件}}` pattern, and that each such subsection contains `#### 待测功能`, `#### 用例`, and `#### Mock策略` in the correct order.

#### Scenario: T8 validates unit test per-file headings

- **WHEN** the evaluator runs T8 against a `test-design.md` with unit test content
- **THEN** the evaluator SHALL verify that each `###` under `## 单元测试` matches the pattern `<source_path> -> <test_path>`
- **AND** fail T8 if any `###` heading deviates from this pattern

#### Scenario: T8 validates unit test subsection completeness

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that each `### <source> -> <test_file>` subsection contains `#### 待测功能`, `#### 用例`, and `#### Mock策略`
- **AND** fail T8 if any of these three sub-subsections is missing

### Requirement: T8 validates unit test table column names

The T8 checklist item SHALL verify that table column names in unit test subsections match the template exactly.

#### Scenario: T8 validates unit 用例 table columns

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that every `#### 用例` table under `## 单元测试` has columns: `测试对象 | 路径类型 | 测试条件 | 迭代类型`
- **AND** fail T8 if any column name differs or if a `测试文件` column is present

#### Scenario: T8 validates unit Mock策略 table columns

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that every `#### Mock策略` table under `## 单元测试` has columns: `Mock主体 | Mock方案 | 应用场景`
- **AND** fail T8 if any column name differs or if a `测试文件` column is present

### Requirement: T8 validates 待测功能 is list format

The T8 checklist item SHALL verify that `#### 待测功能` uses unordered list format (`- func(): desc`), not a table.

#### Scenario: T8 validates 待测功能 is not a table

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that `#### 待测功能` sections do not contain markdown table syntax
- **AND** SHALL verify they use bullet-point list format

### Requirement: T8 format check covers integration test relationship structure

The T8 checklist item SHALL verify that every `###` subsection under `## 集成测试` follows the `{{关系标题}} → {{测试文件路径}}` pattern, and that each such subsection contains the required metadata elements.

#### Scenario: T8 validates integration relationship heading pattern

- **WHEN** the evaluator runs T8 against a `test-design.md` with integration test content
- **THEN** the evaluator SHALL verify that each `###` under `## 集成测试` contains the `→` character followed by a test file path (e.g., `### 关系标题 → \`test_file_path\``)
- **AND** fail T8 if any `###` heading under `## 集成测试` lacks the `→ <test_file>` pattern

#### Scenario: T8 validates 涉及模块 table presence and row count

- **WHEN** the evaluator runs T8
- **THEN** for each `###` relationship subsection under `## 集成测试`, the evaluator SHALL verify that a `**涉及模块**` table exists
- **AND** the table SHALL contain at least 2 data rows (excluding header)
- **AND** fail T8 if `**涉及模块**` is missing or has fewer than 2 rows

#### Scenario: T8 validates 涉及模块 table columns

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that the `**涉及模块**` table has columns: `模块 | 角色`
- **AND** fail T8 if column names differ from the template

#### Scenario: T8 validates 关联AC is non-empty

- **WHEN** the evaluator runs T8
- **THEN** for each `###` relationship subsection, the evaluator SHALL verify that `**关联AC**:` is present and followed by at least one AC ID reference
- **AND** fail T8 if `**关联AC**` is missing or empty

#### Scenario: T8 validates 关系描述 is a non-empty paragraph

- **WHEN** the evaluator runs T8
- **THEN** for each `###` relationship subsection, the evaluator SHALL verify that `**关系描述**:` is present and followed by non-empty narrative text
- **AND** fail T8 if `**关系描述**` is missing, empty, or uses list/table format

### Requirement: T8 validates integration scenario structure

The T8 checklist item SHALL verify that each relationship section contains at least one `#### 场景:` sub-subsection, and that each scenario has a narrative description paragraph and a `##### 用例` table with correct columns.

#### Scenario: T8 validates at least one scenario per relationship

- **WHEN** the evaluator runs T8
- **THEN** for each `###` relationship subsection, the evaluator SHALL verify there is at least one `#### 场景: <名称>` heading
- **AND** fail T8 if a relationship section has no `#### 场景:` sub-subsection

#### Scenario: T8 validates scenario description paragraph

- **WHEN** the evaluator runs T8
- **THEN** for each `#### 场景:` heading, the evaluator SHALL verify there is a non-empty narrative paragraph between the heading and the next `#####` or `####` heading
- **AND** fail T8 if the scenario description paragraph is missing or empty

#### Scenario: T8 validates 用例 table columns in scenarios

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that every `##### 用例` table under `## 集成测试` has columns: `路径类型 | 测试条件 | 迭代类型`
- **AND** fail T8 if any column name differs from the template

### Requirement: T8 validates integration Mock策略 tables

If a `##### Mock策略` table is present under a scenario, the T8 checklist SHALL verify its columns. If absent, T8 SHALL accept the omission as valid (no cross-process mocks needed).

#### Scenario: T8 validates Mock策略 table columns when present

- **WHEN** the evaluator runs T8 and a `##### Mock策略` table is present under a `#### 场景:`
- **THEN** the evaluator SHALL verify the table has columns: `Mock主体 | Mock方案 | 应用场景`
- **AND** fail T8 if column names differ from the template

#### Scenario: T8 accepts omitted Mock策略 when not needed

- **WHEN** the evaluator runs T8 and a scenario has no `##### Mock策略` section
- **THEN** the evaluator SHALL NOT require a Mock策略 table
- **AND** SHALL pass T8 for that scenario (no Mock策略 needed is valid)

### Requirement: T8 validates 验收范围 table remains unchanged

The T8 checklist item SHALL verify that the `## 验收范围` table retains its 5 columns: `AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景`.

#### Scenario: T8 validates 验收范围 table columns

- **WHEN** the evaluator runs T8
- **THEN** the evaluator SHALL verify that the `## 验收范围` table has exactly 5 columns with the names specified in the template
- **AND** fail T8 if columns differ from the template

### Requirement: Evaluator cross-references per-file and per-relationship sections with 验收范围

The evaluator SHALL verify that every `测试文件` and `测试对象/测试场景` in `## 验收范围` has a corresponding subsection in `## 单元测试` or `## 集成测试`, and vice versa.

For integration tests, the cross-reference SHALL verify that every `AC ID` referenced in `**关联AC**` lines appears in `## 验收范围`.

#### Scenario: Cross-reference between 验收范围 and unit test subsections

- **WHEN** the evaluator runs T2 ("`验收范围`与`用例`互相对应")
- **THEN** the evaluator SHALL check that every `测试文件` entry in `## 验收范围` with `测试类型: 单元测试` appears as a heading or referenced file in `## 单元测试` per-file sections

#### Scenario: Cross-reference between 验收范围 and integration relationship sections

- **WHEN** the evaluator runs T2
- **THEN** the evaluator SHALL check that every `AC ID` listed in any `**关联AC**` field under `## 集成测试` appears in `## 验收范围`
- **AND** that every `AC ID` with `测试类型: 集成测试` in `## 验收范围` appears in at least one relationship's `**关联AC**` field

## Module Contract

### Agent file: `plugins/dev-team/agents/test-design-evaluator.md`

| Checklist ID | Check Item | Scope |
|-------------|------------|-------|
| T2 | `验收范围`与`用例`互相对应 | Cross-references per-file headings and per-relationship sections with 验收范围 entries |
| T8 | 产物结构与模板格式一致 | Validates: unit per-file heading pattern, unit sub-section completeness, unit table column names, 待测功能 list format; integration relationship heading `→` pattern, 涉及模块 table (≥2 rows, correct columns), 关联AC non-empty, 关系描述 non-empty paragraph, at least one `#### 场景:` per relationship, scenario description paragraph, `##### 用例` table columns, optional `##### Mock策略` columns, 验收范围 preservation |

### Reference files

| File | Purpose |
|------|---------|
| `plugins/dev-team/templates/artifacts/test-design.md.template` | Reference template for T8 format compliance check |
| `openspec/changes/<change-name>/test-design.md` | The artifact being evaluated |
| `openspec/changes/<change-name>/proposal.md` | Reference for cross-checking AC coverage |
