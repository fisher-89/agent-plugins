## Context

The architecture agent generates C4 model files (`.c4`) that use an incorrect DSL syntax. The LikeC4 DSL specification defines:

- `specification` block: `element <name>` to declare element kinds
- `model` block: `<kind> <Name>` for elements, `source -> target "desc"` for relationships
- `metadata` block: `metadata { key value }` or `metadata { key [array] }` — must be wrapped in braces

The current codebase uses a fabricated syntax (`<name>: elementKind`, `metadata path [...]`) that happens to be parseable by the custom Python parser but is not valid LikeC4 DSL.

## Goals / Non-Goals

**Goals:**
- Make all generated C4 DSL valid per the LikeC4 specification
- Make the Python parsers handle correct LikeC4 `metadata { }` block syntax
- Add specification syntax validation (`element <name>` check)
- Update the bootstrap template and agent instructions with correct examples

**Non-Goals:**
- Full LikeC4 semantic validation (nesting rules, kind checking) — out of scope
- Views, styling, or deployment model support
- Backward compatibility with the old incorrect syntax

## Decisions

### Decision 1: Switch metadata parser from line-based to block-based

**Choice**: Parse `metadata { ... }` as a delimited block with brace-depth tracking, rather than a single-line prefix match.

**Why**: The LikeC4 spec requires `metadata { key value }` or `metadata { key [array] }`. The current parser matches `stripped.startswith("metadata path")` and extracts the rest of the line — this cannot handle the correct format where `path` is a key inside braces.

**Implementation**: When `stripped.startswith("metadata")` and `stripped.endswith("{")`, enter metadata collection mode. Track brace depth. Parse `key value` and `key [array]` lines inside the block until the closing `}`.

### Decision 2: Add specification syntax validation

**Choice**: Add a check in `_validate_structure()` that detects `name: elementKind` patterns and rejects them with a message pointing to the correct `element name` syntax.

**Why**: The current validator only checks brace balance and block presence. Without a syntax check, the agent can generate invalid specification blocks that pass validation.

### Decision 3: Update template to full working example

**Choice**: Expand the template from a minimal empty model to a commented working example showing correct `element <name>`, `metadata { }`, and relationship syntax.

**Why**: The architecture agent uses the template as reference. A correct template prevents future invalid generations.

### Decision 4: Add LikeC4 syntax rules to agent instructions

**Choice**: Add a "DSL Syntax Quick Reference" section to `architecture.md` with concrete examples of correct specification, element, metadata, and relationship syntax.

**Why**: The current agent instructions have no syntax guidance — the bootstrap example is just a comment placeholder. The agent needs authoritative syntax to generate valid DSL.

## Risks / Trade-offs

- **Parser regression**: The block-based metadata parser is more complex than the current line-based approach. → Mitigation: Keep the parser minimal — only track brace depth, don't build a full AST.
- **No backward compat**: Existing model files using old syntax will fail validation after the change. → Mitigation: Only `01-core.c4` exists and will be rewritten in this same change.
