## ADDED Requirements

### Requirement: Specification block uses element keyword syntax

The system SHALL declare element kinds in the `specification {}` block using `element <name>` syntax. The system SHALL NOT accept `<name>: elementKind` syntax.

#### Scenario: Valid element kind declaration

- **WHEN** the specification block contains `element softwareSystem`
- **THEN** validation SHALL pass

#### Scenario: Invalid colon syntax rejected

- **WHEN** the specification block contains `softwareSystem: elementKind`
- **THEN** validation SHALL fail with an error indicating the correct `element softwareSystem` syntax

### Requirement: Metadata block uses brace-delimited syntax

The system SHALL use `metadata { key value }` or `metadata { key [array] }` syntax for element metadata. The system SHALL NOT accept flat `metadata key value` syntax without braces.

#### Scenario: Valid metadata with single value

- **WHEN** an element contains `metadata { path './src/services/payment/' }`
- **THEN** the parser SHALL extract `path: ['./src/services/payment/']`

#### Scenario: Valid metadata with array value

- **WHEN** an element contains `metadata { path ['./src/services/payment/', './src/shared/billing.ts'] }`
- **THEN** the parser SHALL extract `path: ['./src/services/payment/', './src/shared/billing.ts']`

#### Scenario: Flat metadata syntax rejected

- **WHEN** an element contains `metadata path ["./src/services/payment/"]` without braces
- **THEN** validation SHALL fail with an error indicating the correct `metadata { path [...] }` syntax

### Requirement: Parsers handle brace-delimited metadata blocks

The Python DSL parsers in `archi-model.py` and `archi-validate.py` SHALL parse `metadata { }` blocks by tracking brace depth and extracting key-value pairs from lines within the block.

#### Scenario: Multi-line metadata block

- **WHEN** an element contains:
  ```
  metadata {
    path ['./hooks/']
    owner 'team-platform'
  }
  ```
- **THEN** the parser SHALL extract both `path` and `owner` keys with their values

#### Scenario: Trailing comma in array

- **WHEN** a metadata array value has a trailing comma like `['a', 'b',]`
- **THEN** the parser SHALL strip the trailing comma and extract `['a', 'b']`
