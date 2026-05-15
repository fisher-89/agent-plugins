## MODIFIED Requirements

### Requirement: Modify model via DSL text editing with Python syntax validation

The system SHALL support adding and modifying model elements by editing DSL text and validating the result with a pure Python structural validator before writing to a specific file in `models/`. The validator SHALL check that the `specification {}` block uses `element <name>` syntax and that `metadata { }` uses brace-delimited key-value syntax, in addition to brace balance and block presence checks.

#### Scenario: Add a new component

- **WHEN** the utility inserts a new `component` block with `metadata { path ['./src/new/'] }` into the DSL
- **THEN** the modified DSL passes Python structural validation before being written to disk at the specified `--path`

#### Scenario: Add a new relationship

- **WHEN** the utility inserts a new `->` relationship between two existing elements
- **THEN** the modified DSL passes Python structural validation before being written to disk

#### Scenario: Invalid specification syntax

- **WHEN** the utility validates DSL containing `softwareSystem: elementKind` in the specification block
- **THEN** the system SHALL fail validation with an error indicating `element softwareSystem` is the correct syntax

#### Scenario: Invalid metadata syntax

- **WHEN** the utility validates DSL containing `metadata path [...]` without braces
- **THEN** the system SHALL fail validation with an error indicating `metadata { path [...] }` is the correct syntax

#### Scenario: Invalid DSL insertion

- **WHEN** the utility produces DSL that fails Python structural validation
- **THEN** the system SHALL NOT write to disk and SHALL report the validation error

### Requirement: Element-to-code mapping via metadata.path

The system SHALL use `metadata { path <value> }` to associate model elements with their code locations. A path pointing to a directory SHALL match all files within that directory recursively.

#### Scenario: Single directory mapping

- **WHEN** an element has `metadata { path './src/services/payment/' }`
- **THEN** all files in `src/services/payment/` and its subdirectories match that element

#### Scenario: Multiple path mapping

- **WHEN** an element has `metadata { path ['./src/services/payment/', './src/shared/billing.ts'] }`
- **THEN** both the directory tree and the single file match that element

#### Scenario: Path not found

- **WHEN** `metadata { path './nonexistent/' }` points to a directory that does not exist
- **THEN** the validate sub-agent SHALL report a warning

### Requirement: Model initialization

The system SHALL support creating the `models/` directory with an initial `01-core.c4` when no model exists. The bootstrap file SHALL use correct LikeC4 DSL syntax including `element <name>` in the specification block and `metadata { }` for element metadata.

#### Scenario: Bootstrap a new model

- **WHEN** `openspec/architecture/models/` does not exist or is empty
- **THEN** the sub-agent SHALL create the directory and write a minimal `01-core.c4` containing a valid `specification {}` block with `element <kind>` declarations and an empty `model {}` block

#### Scenario: Legacy model.c4 migration

- **WHEN** `openspec/architecture/model.c4` exists but `openspec/architecture/models/` does not
- **THEN** the system SHALL emit a deprecation warning and read from the legacy file, recommending migration to `models/`
