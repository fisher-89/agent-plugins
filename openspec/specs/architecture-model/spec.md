# architecture-model Specification

## Purpose
TBD - created by archiving change add-architect-role. Update Purpose after archive.
## Requirements
### Requirement: Query model structure via likec4 API
The system SHALL provide a sub-agent that loads a likec4 workspace from `openspec/architecture/` using `LikeC4.fromWorkspace()` (from `likec4` package), then queries elements, their metadata, hierarchy, and relationships via the computed model returned by `.computedModel()` (type `LikeC4Model.Computed` from `@likec4/core/model`).

#### Scenario: Query all elements
- **WHEN** the sub-agent queries all model elements
- **THEN** the system returns element IDs, kinds, and metadata for every element in the model

#### Scenario: Query element by ID
- **WHEN** the sub-agent queries a specific element by FQN
- **THEN** the system returns the element's children, parent, ancestors, and metadata including `path`

#### Scenario: Query relationships for an element
- **WHEN** the sub-agent queries incoming and outgoing relationships for an element
- **THEN** the system returns all relationship source/target pairs with their titles and tags

### Requirement: Modify model via DSL text editing with syntax validation
The system SHALL support adding and modifying model elements by editing DSL text and validating the result with `fromSource()` before writing.

#### Scenario: Add a new component
- **WHEN** the sub-agent inserts a new `component` block into the DSL
- **THEN** the modified DSL passes `fromSource()` validation before being written to disk

#### Scenario: Add a new relationship
- **WHEN** the sub-agent inserts a new `->` relationship between two existing elements
- **THEN** the modified DSL passes `fromSource()` validation before being written to disk

#### Scenario: Invalid DSL insertion
- **WHEN** the sub-agent produces DSL that fails `fromSource()` validation
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
The system SHALL support creating an initial `model.c4` when no model exists.

#### Scenario: Bootstrap a new model
- **WHEN** `openspec/architecture/model.c4` does not exist
- **THEN** the sub-agent SHALL create a minimal valid model file with `model {}` block and necessary `specification {}` block

