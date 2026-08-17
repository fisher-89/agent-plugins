---
name: openspec-archive-change
description: Archive a completed change in the experimental workflow. Use when the user wants to finalize and archive a change after implementation is complete.
---

Archive a completed change in the experimental workflow.

**Input**: Optionally specify a change name. If omitted, check if it can be inferred from conversation context. If vague or ambiguous you MUST prompt for available changes.

**Steps**

1. **Resolve the change and check completion status**

   Call `mcp__plugin_dev-team_dev-team__change_list` once to get all active changes (not archived).

   **If a change name was provided** — find the entry whose `name` is `"<name>"`.

   **If no change name was provided** — use `AskQuestion` to let the user
   select from the active changes. Include the schema used for each change if
   available. **IMPORTANT**: Do NOT guess or auto-select a change. Always let
   the user choose.

   Parse the resolved entry to understand:
   - `artifacts`: Present artifact filenames (e.g. proposal.md, design.md, tasks.md, eval.json)
   - `workflow_done`: Whether all workflow phases have a non-stale pass/skipped entry

   **If the change is missing from the result:**
   - Display warning that the change was not found
   - Use `AskQuestion` to confirm user wants to proceed
   - Proceed if user confirms

   **If any required artifacts are missing or `workflow_done` is `false`:**
   - Display warning listing incomplete artifacts
   - Use `AskQuestion` to confirm user wants to proceed
   - Proceed if user confirms

2. **Assess delta spec sync state**

   Check for delta specs at `openspec/changes/<name>/specs/`. If none exist, proceed without sync prompt.

   **If delta specs exist:**
   - Compare each delta spec with its corresponding main spec at `openspec/specs/<capability>/spec.md`
   - Determine what changes would be applied (adds, modifications, removals, renames)
   - Show a combined summary before prompting

   **Prompt options:**
   - If changes needed: "Sync now (recommended)", "Archive without syncing"
   - If already synced: "Archive now", "Sync anyway", "Cancel"

   If user chooses sync, apply the delta specs to the main specs directly. For each
   capability with a delta spec at `openspec/changes/<name>/specs/<capability>/spec.md`:

   a. **Read the delta spec** and the main spec at `openspec/specs/<capability>/spec.md`
      (the main spec may not exist yet).

   b. **Apply each section intelligently** — the delta expresses *intent*, not a
      wholesale replacement; preserve main-spec content the delta does not mention:

      - **`## ADDED Requirements`** — add the requirement if absent; if it already
        exists, update it to match.
      - **`## MODIFIED Requirements`** — apply the increment only: add new scenarios,
        modify listed scenarios, or change the description, without copying
        existing scenarios.
      - **`## REMOVED Requirements`** — remove the entire requirement block.
      - **`## RENAMED Requirements`** — rename the `FROM:` requirement to `TO:`.

   c. **Create the main spec** at `openspec/specs/<capability>/spec.md` if the
      capability does not exist yet: add a brief `## Purpose` section (TBD is fine)
      plus the ADDED requirements.

   The merge SHOULD be idempotent (re-running produces the same result).

   Proceed to archive regardless of choice.

3. **Perform the archive**

   Create the archive directory if it doesn't exist:
   ```bash
   mkdir -p openspec/changes/archive
   ```

   Generate target name using current date: `YYYY-MM-DD-<change-name>`

   **Check if target already exists:**
   - If yes: Fail with error, suggest renaming existing archive or using different date
   - If no: Move the change directory to archive

   ```bash
   mv openspec/changes/<name> openspec/changes/archive/YYYY-MM-DD-<name>
   ```

4. **Display summary**

   Show archive completion summary including:
   - Change name
   - Schema that was used
   - Archive location
   - Whether specs were synced (if applicable)
   - Note about any warnings (incomplete artifacts/eval failures)

**Output On Success**

```
## Archive Complete

**Change:** <change-name>
**Schema:** <schema-name>
**Archived to:** openspec/changes/archive/YYYY-MM-DD-<name>/
**Specs:** ✓ Synced to main specs (or "No delta specs" or "Sync skipped")

All artifacts complete. Eval chain passed.
```

**Guardrails**
- Always prompt for change selection if not provided
- Use `mcp__plugin_dev-team_dev-team__change_list` (`artifacts` + `workflow_done`) for completion checking
- Don't block archive on warnings - just inform and confirm
- If delta specs exist, always run the sync assessment and show the combined summary before prompting
