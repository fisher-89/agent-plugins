## MODIFIED Requirements

### Requirement: Query model structure via Python DSL parser
The system SHALL provide a utility that parses `model.c4` DSL text using a pure Python parser to extract elements, their metadata, hierarchy, and relationships.

#### Scenario: Query all elements
- **WHEN** the utility queries all model elements
- **THEN** the system returns element IDs, kinds, and metadata for every element in the model

#### Scenario: Query element by ID
- **WHEN** the utility queries a specific element by FQN
- **THEN** the system returns the element's children, parent, ancestors, and metadata including `path`

#### Scenario: Query relationships for an element
- **WHEN** the utility queries incoming and outgoing relationships for an element
- **THEN** the system returns all relationship source/target pairs with their titles and tags

### Requirement: Modify model via DSL text editing with Python syntax validation
The system SHALL support adding and modifying model elements by editing DSL text and validating the result with a pure Python structural validator before writing.

#### Scenario: Add a new component
- **WHEN** the utility inserts a new `component` block into the DSL
- **THEN** the modified DSL passes Python structural validation before being written to disk

#### Scenario: Add a new relationship
- **WHEN** the utility inserts a new `->` relationship between two existing elements
- **THEN** the modified DSL passes Python structural validation before being written to disk

#### Scenario: Invalid DSL insertion
- **WHEN** the utility produces DSL that fails Python structural validation
- **THEN** the system SHALL NOT write to disk and SHALL report the validation error

## REMOVED Requirements

### Requirement: Query model structure via likec4 API
**Reason**: Replaced by pure Python DSL parser. The likec4 API (`LikeC4.fromWorkspace()`) required a 99MB npm dependency for functionality already implemented in Python.
**Migration**: No migration needed. The Python parser in `archi-model.py` already exists and handles all parsing. External callers use the same CLI interface (`archi-model.py --command query`).
