## 1. Setup embedded openspec bin

- [x] 1.1 Create `plugins/dev-team/bin/` directory with a `package.json` locking `@fission-ai/openspec` version
- [x] 1.2 Create `plugins/dev-team/bin/openspec` wrapper script (bash/PowerShell) that invokes node on the local openspec.js
- [x] 1.3 Add `bin/` to plugin.json metadata so the harness knows about the embedded CLI

## 2. Rewrite SessionStart hook

- [x] 2.1 Remove `check_likec4_available()` function entirely
- [x] 2.2 Replace `check_openspec_installed()` with `ensure_embedded_openspec()` that checks `plugins/dev-team/bin/node_modules/@fission-ai/openspec/`
- [x] 2.3 Add auto-bootstrap logic: if embedded openspec missing, run `npm install` in `plugins/dev-team/bin/`
- [x] 2.4 Remove the likec4-related issue message

## 3. Remove likec4 from archi-model.py

- [x] 3.1 Delete `_validate_via_cli()` function (the `npx likec4 build` call)
- [x] 3.2 Simplify `validate_dsl()` to call `_validate_structure()` directly without the CLI attempt
- [x] 3.3 Verify `_validate_structure()` is comprehensive enough (check brace balance, required blocks); add missing checks if needed

## 4. Update plugin manifest

- [x] 4.1 Add `openspecVersion` field to `plugins/dev-team/.claude-plugin/plugin.json` to lock the dependency version
- [x] 4.2 Bump plugin patch version

## 5. Verify and clean up

- [x] 5.1 Test: delete global openspec, run session-start hook → should auto-install into bin/ and pass
- [x] 5.2 Test: run `archi-model.py --command validate` → should use Python parser, succeed without likec4
- [x] 5.3 Test: run `archi-validate.py --staged` → should work unchanged (no likec4 dependency)
- [x] 5.4 Verify no remaining `likec4` or `npx likec4` references in plugin code
