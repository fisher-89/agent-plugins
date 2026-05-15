## 1. Fix template

- [x] 1.1 Rewrite `plugins/dev-team/templates/model.c4` with correct LikeC4 DSL syntax (`element <name>`, `metadata { }` blocks, correct relationship examples)

## 2. Fix archi-model.py parser and validator

- [x] 2.1 Update `_parse_dsl()` to parse `metadata { }` blocks (brace-delimited) instead of flat `metadata path` lines
- [x] 2.2 Update `_validate_structure()` to detect and reject invalid `name: elementKind` syntax in specification blocks
- [x] 2.3 Update `_validate_structure()` to detect and reject flat `metadata path [...]` syntax without braces

## 3. Fix archi-validate.py parser

- [x] 3.1 Update `_parse_model_dsl()` to parse `metadata { }` blocks with brace-depth tracking
- [x] 3.2 Update `_parse_metadata_path()` to handle metadata content inside braces

## 4. Fix architecture agent instructions

- [x] 4.1 Add "DSL Syntax Quick Reference" section to `plugins/dev-team/agents/architecture.md` with correct LikeC4 syntax examples for specification, elements, metadata, and relationships
- [x] 4.2 Update bootstrap example in agent instructions to show correct syntax

## 5. Fix generated model file

- [x] 5.1 Rewrite `openspec/architecture/models/01-core.c4` with correct LikeC4 DSL syntax

## 6. Validation

- [x] 6.1 Run `archi-model.py --command validate` to confirm corrected model passes validation
- [x] 6.2 Run `archi-model.py --command query` to confirm parser extracts all elements and relationships correctly

## 7. Gan 优化建议 (P0-P3)

- [x] 7.1 [A2/P0] Fix extend block relationship parsing bug — remove `not extend_stack` condition in both archi-model.py and archi-validate.py
- [x] 7.2 [C1/P0] Commit gate check report status + diff hash — verify report `status` is "clean" and `commit_diff_hash` matches current staged diff
- [x] 7.3 [A1/P1] Extract shared DSL parser module (`archi_parser.py`) — eliminate ~150 lines of duplicated parsing code
- [x] 7.4 [B1/P1] Add mode routing instructions to architecture agent — intent→mode mapping
- [x] 7.5 [B3/P1] Fix cross-file validate ambiguity — auto-prepend specification block when validating single file
- [x] 7.6 [C3/P3] Use `sys.stdlib_module_names` for dynamic Python stdlib detection
