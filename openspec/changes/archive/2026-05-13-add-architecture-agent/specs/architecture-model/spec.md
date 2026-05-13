## MODIFIED Requirements

### Requirement: Query model structure via Python DSL parser

The system SHALL provide a utility that parses all `*.c4` files in `openspec/architecture/models/` using a pure Python parser to extract elements, their metadata, hierarchy, and relationships. Files SHALL be loaded in alphabetical order and aggregated into a single model.

#### Scenario: Query all elements

- **WHEN** the utility queries all model elements
- **THEN** the system returns element IDs, kinds, and metadata for every element in the aggregated model

#### Scenario: Query element by ID

- **WHEN** the utility queries a specific element by FQN
- **THEN** the system returns the element's children, parent, ancestors, and metadata including `path`

#### Scenario: Query relationships for an element

- **WHEN** the utility queries incoming and outgoing relationships for an element
- **THEN** the system returns all relationship source/target pairs with their titles and tags

#### Scenario: Aggregate multiple model files

- **WHEN** `models/` contains `01-core.c4` and `02-services.c4`
- **THEN** the system SHALL parse them in alphabetical order and merge elements and relationships

### Requirement: Modify model via DSL text editing with Python syntax validation

The system SHALL support adding and modifying model elements by editing DSL text and validating the result with a pure Python structural validator before writing to a specific file in `models/`.

#### Scenario: Add a new component

- **WHEN** the utility inserts a new `component` block into the DSL
- **THEN** the modified DSL passes Python structural validation before being written to disk at the specified `--path`

#### Scenario: Add a new relationship

- **WHEN** the utility inserts a new `->` relationship between two existing elements
- **THEN** the modified DSL passes Python structural validation before being written to disk

#### Scenario: Invalid DSL insertion

- **WHEN** the utility produces DSL that fails Python structural validation
- **THEN** the system SHALL NOT write to disk and SHALL report the validation error

### Requirement: Element-to-code mapping via metadata.path

The system SHALL use `metadata.path` to associate model elements with their code locations. A path pointing to a directory SHALL match all files within that directory recursively.

#### Scenario: Single directory mapping

- **WHEN** an element has `metadata.path "./src/services/payment/"`
- **THEN** all files in `src/services/payment/` and its subdirectories match that element

#### Scenario: Multiple path mapping

- **WHEN** an element has `metadata.path ["./src/services/payment/", "./src/shared/billing.ts"]`
- **THEN** both the directory tree and the single file match that element

#### Scenario: Path not found

- **WHEN** `metadata.path` points to a directory that does not exist
- **THEN** the validate sub-agent SHALL report a warning

### Requirement: Model initialization

The system SHALL support creating the `models/` directory with an initial `01-core.c4` when no model exists.

#### Scenario: Bootstrap a new model

- **WHEN** `openspec/architecture/models/` does not exist or is empty
- **THEN** the sub-agent SHALL create the directory and write a minimal `01-core.c4` containing the `specification {}` and `model {}` blocks

#### Scenario: Legacy model.c4 migration

- **WHEN** `openspec/architecture/model.c4` exists but `openspec/architecture/models/` does not
- **THEN** the system SHALL emit a deprecation warning and read from the legacy file, recommending migration to `models/`

## ADDED Requirements

### Requirement: Write requires target path

The `write` command SHALL require a `--path` argument specifying the target file within `models/`.

#### Scenario: Write to specified path

- **WHEN** `archi-model.py --command write --path models/03-payments.c4 --source "<dsl>"`
- **THEN** the utility SHALL validate the source and write only to `models/03-payments.c4`

#### Scenario: Write rejects paths outside models/

- **WHEN** `--path` targets a file outside `openspec/architecture/models/`
- **THEN** the utility SHALL reject the write with an error

### Requirement: Duplicate specification blocks are rejected

The system SHALL reject model directories where more than one `*.c4` file contains a `specification {}` block.

#### Scenario: Duplicate specification detection

- **WHEN** two files in `models/` each contain a `specification {}` block
- **THEN** validation SHALL fail with an error identifying both files
