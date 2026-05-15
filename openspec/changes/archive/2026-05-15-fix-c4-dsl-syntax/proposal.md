## Why

The architecture agent generates C4 model files using a DSL syntax that does not match the LikeC4 DSL specification (https://likec4.dev/dsl/). The current generated file `01-core.c4` uses `name: elementKind` instead of `element name`, and flat `metadata path [...]` instead of `metadata { path [...] }`. This makes the model files unparseable by the LikeC4 toolchain and invalid per the DSL spec.

## What Changes

- **BREAKING**: Fix `specification` block syntax from `<name>: elementKind` to `element <name>` in template and all generated/parsed DSL
- **BREAKING**: Fix `metadata` syntax from flat `metadata path [...]` to `metadata { path [...] }` in template and all generated/parsed DSL
- Update `archi-model.py` parser to correctly parse the `metadata { }` block structure
- Update `archi-validate.py` parser to correctly parse the `metadata { }` block structure
- Update `archi-model.py` validator to check `specification` block uses correct `element <name>` syntax
- Update architecture agent instructions with explicit LikeC4 DSL syntax rules and correct examples
- Rewrite `01-core.c4` with correct syntax

## Capabilities

### New Capabilities

- `c4-dsl-syntax`: C4 DSL syntax compliance with LikeC4 specification — correct `element <name>` declarations, `metadata { }` block structure, and structural validation rules

### Modified Capabilities

- `architecture-model`: spec examples use old `metadata.path` syntax — update to `metadata { path [...] }`; parser behavior changes to handle braced metadata blocks
- `architecture-validation`: parser behavior changes to handle braced metadata blocks; spec examples updated
- `architecture-agent`: agent instructions gain explicit LikeC4 syntax rules and correct bootstrap examples

## Impact

- `plugins/dev-team/templates/model.c4` — rewrite with correct syntax
- `plugins/dev-team/utils/archi-model.py` — parser and validator updates
- `plugins/dev-team/utils/archi-validate.py` — parser updates
- `plugins/dev-team/agents/architecture.md` — agent instruction updates
- `openspec/architecture/models/01-core.c4` — rewrite with correct syntax
- All existing `metadata.path` references in specs/ change to `metadata { path [...] }`
