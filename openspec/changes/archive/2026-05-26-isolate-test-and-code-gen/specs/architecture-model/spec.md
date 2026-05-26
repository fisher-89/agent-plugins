## MODIFIED Requirements

### Requirement: Query model structure via TypeScript CLI
The system SHALL provide a `dev-team archi query` CLI command that parses all `*.c4` files in `openspec/specs/architecture/models/` using `@likec4/core`'s `LikeC4.fromSource()` API to extract elements, their metadata (including `path`), hierarchy, and relationships.
This SHALL replace the Python `archi-model.py --command query` utility.
Files SHALL be loaded in alphabetical order and aggregated into a single DSL text before parsing.

#### Scenario: Query all elements via CLI
- **WHEN** the user runs `dev-team archi query`
- **THEN** the system returns a JSON object with `elements` and `relationships` arrays, each element containing `name`, `kind`, `paths`, `metadata`

#### Scenario: Query element by FQN via CLI
- **WHEN** the user runs `dev-team archi query --element "App.AuthDomain"`
- **THEN** the system returns the element's details and its incoming/outgoing relationships

### Requirement: Modify model via CLI with @likec4/core validation
The system SHALL support adding and modifying model elements by editing DSL text and validating the result with `@likec4/core`'s `LikeC4.fromSource()` + `getErrors()` before writing to a specific file in `models/`.
This SHALL replace the Python `archi-model.py --command validate` and `--command write` utilities.

#### Scenario: Add a new component via CLI
- **WHEN** the user runs `dev-team archi write --path "models/03-payments.c4" --source "<dsl>"`
- **THEN** DSL passes `@likec4/core` validation and is written to the specified file

#### Scenario: Invalid DSL rejected via CLI
- **WHEN** the user runs `dev-team archi write` with DSL that fails `@likec4/core` validation
- **THEN** the system SHALL NOT write to disk and SHALL report the validation errors

### Requirement: Element-to-code mapping via metadata.path (unchanged behavior)
The system SHALL continue to use `metadata { path <value> }` to associate model elements with their code locations. This requirement is unchanged — only the parsing engine changes from Python `archi_parser.py` to `@likec4/core`.

#### Scenario: Path mapping via @likec4/core
- **WHEN** an element has `metadata { path './src/services/payment/' }`
- **THEN** `element.getMetadata('path')` via `@likec4/core` API returns the path array

## REMOVED Requirements

### Requirement: Query model structure via Python DSL parser
The Python `archi-model.py --command query` utility SHALL be removed. Its functionality is fully replaced by `dev-team archi query`.

### Requirement: Modify model via DSL text editing with Python syntax validation
The Python `archi-model.py --command validate` and `--command write` utilities SHALL be removed. Their functionality is fully replaced by `dev-team archi validate` and `dev-team archi write`.

### Requirement: Write requires target path
The Python `archi-model.py --command write --path` requirement SHALL be removed. The `--path` validation logic is re-implemented in the TypeScript CLI.

### Requirement: Duplicate specification blocks are rejected
The Python `archi_parser.check_duplicate_specifications()` requirement SHALL be removed. The `@likec4/core` API handles specification block validation internally.

### Requirement: Model initialization
The Python `archi-model.py --command write` bootstrap logic SHALL be removed. Bootstrap is performed by `dev-team archi write --path models/01-core.c4 --source "<dsl>"`.

## ADDED Requirements

### Requirement: dev-team archi commands replace Python archi-model.py
The system SHALL provide `dev-team archi query`, `dev-team archi validate`, and `dev-team archi write` commands that are behaviorally equivalent to the removed `archi-model.py` commands.
The new commands SHALL use `@likec4/core` as the parsing and validation engine instead of the hand-written Python parser.

#### Scenario: CLI command equivalence
- **WHEN** the user runs `dev-team archi query` on the same model files as `python archi-model.py --command query`
- **THEN** both commands produce equivalent element and relationship data (same names, kinds, paths, relationship sources/targets)

### Requirement: architecture agent references CLI commands
The `architecture.md` agent prompt SHALL reference `dev-team archi <action>` commands instead of `python plugins/dev-team/utils/archi-model.py --command <action>` commands.

#### Scenario: Agent prompt updated
- **WHEN** the architecture agent reads its prompt
- **THEN** all command examples use `dev-team archi query` / `dev-team archi validate` / `dev-team archi write` syntax
- **AND** no references to `archi-model.py` remain in the agent prompt
