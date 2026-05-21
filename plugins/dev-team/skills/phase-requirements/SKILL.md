---
name: phase-requirements
description: |
  DESIGN phase (P→E): requirements-planner writes proposal.md, then requirements-evaluator checks with static checklist.
  Loops on fail until all required checklist items pass. Use this as the first phase of the PGE workflow.
  Replaces: /dev-team:openspec-propose
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Requirements phase — Planner writes proposal.md and specs/, Evaluator checks proposal.md.

## Usage

```
/dev-team:phase-requirements [change-name]
```

## Process

### Step 1: Detect active change

Source the CLI wrapper for utility functions:

```bash
source plugins/dev-team/utils/openspec-cli.sh
```

Follow the decision tree below based on whether a change name was provided.

**=== Branch A: Change name was provided (`/dev-team:phase-requirements <name>`) ===**

1. Validate the name format:
   ```bash
   validate_change_name "<name>"
   ```
   - If exit code is 1: name is empty. Ask the user to provide a valid kebab-case name.
   - If exit code is 2: name exceeds 128 characters. Inform the user and ask for a shorter name.
   - If exit code is 3: name contains invalid characters or format. Explain that names must be kebab-case (lowercase letters, digits, hyphens only, starting with a letter or digit). Ask for a corrected name.

2. Check whether the change already exists:
   ```bash
   if change_exists "<name>"; then
       echo "Change '<name>' exists, proceeding directly."
   else
       echo "Change '<name>' does not exist, scaffolding..."
       openspec_new_change "<name>"
   fi
   ```
   - **Exists** (exit 0): proceed directly to Step 2.
   - **Not exists** (exit 1): call `openspec_new_change "<name>"` to scaffold the change directory and `.openspec.yaml`, then proceed to Step 2.

3. With a valid change name established, continue to Step 2.

**=== Branch B: No change name provided (`/dev-team:phase-requirements` without arguments) ===**

Detect the user's context:

1. **Check for prior openspec-explore session**: Scan the conversation history for characteristic markers of an explore session:
   - A "What We Figured Out" summary section
   - Design decision tables (columns like "Area | Choice | Rationale")
   - ASCII diagrams (architecture, state machines, data flows)
   - "Options Considered" analysis with pros/cons
   - Key Analysis blocks with performance/scalability notes
   - The user previously invoked `/dev-team:openspec-explore` or a similar explore command

2. **If explore context IS detected** (Branch B1):
   - Extract a structured exploration insights summary containing:
     - Key decisions made (what was chosen and why)
     - Design choices with rationale
     - Options considered and rejected
     - Open questions or unresolved items
   - Then ask the user for a change name:
     ```
     AskUserQuestion("What should we call this change? (kebab-case, e.g., 'add-user-auth')")
     ```
   - Derive kebab-case from the user's response:
     ```bash
     local derived_name
     derived_name="$(derive_kebab_case "$user_response")"
     ```
   - Show the proposed name and ask for confirmation:
     ```
     I'll create a change named '<derived_name>'. Proceed?
     ```
   - If confirmed: call `openspec_new_change "$derived_name"`, then proceed to Step 2.
   - If rejected or user provides a different name: use the user's custom name, re-validate, confirm again, scaffold, proceed to Step 2.
   - If user cancels: output "已取消提案编写，你可以稍后通过 /dev-team:phase-requirements <name> 重新开始" and stop.

3. **If NO explore context is detected** (Branch B2 -- empty/fresh conversation):
   - Use AskUserQuestion with no preset options:
     ```
     AskUserQuestion("想构建什么变更？描述你想实现的功能或修复的问题。")
     ```
   - Wait for the user's response.
   - Derive a kebab-case name from their description:
     ```bash
     local derived_name
     derived_name="$(derive_kebab_case "$user_description")"
     ```
   - If `derived_name` is empty (e.g., purely CJK input with no ASCII characters), ask the user to provide an English kebab-case name directly.
   - Present the derived name to the user:
     ```
     I'll create a change named '<derived_name>'. Proceed? (You can also suggest a different name or type 'cancel')
     ```
   - If confirmed: scaffold and proceed.
   - If user suggests a different name: validate the custom name with `validate_change_name`, scaffold with the custom name, proceed.
   - If user cancels: show "已取消提案编写，你可以稍后通过 /dev-team:phase-requirements <name> 重新开始" and stop.

**=== Name conflict handling (all branches) ===**

If `openspec_new_change` reports that the change already exists (exit code 1 with "already exists" message):
- Append a numeric suffix starting from 2: `<name>-2`, `<name>-3`, etc.
- Re-validate with `validate_change_name`.
- Show the adjusted name and request confirmation again.
- Repeat until a non-conflicting name is found or the user cancels.

**=== Store explore context for Step 3a ===**

If explore context was detected, save the extracted summary in a variable for use in Step 3a:

```bash
EXPLORE_CONTEXT_SUMMARY="<extracted structured insights>"
```

This variable will be injected into the Planner prompt in Step 3a. If no explore context was found, leave `EXPLORE_CONTEXT_SUMMARY` empty.

### Step 2: Check for backtrack marker

Read `openspec/changes/<name>/phases/eval.json` if it exists. Search for entries where `backtrack_to` is `"01-requirements"` and the latest entry for that phase. If found, re-run the Evaluator first:

```
Agent({
  description: "Evaluate proposal.md (backtrack)",
  subagent_type: "requirements-evaluator",
  prompt: "Re-evaluate proposal.md for change '<name>'. A backtrack marker was set. Check against your static checklist and append result to eval.json."
})
```

### Step 3: P→E Loop

Run the Planner → Evaluator loop:

**3a. Invoke Planner:**

First, collect dynamic context from the CLI to enrich the Planner prompt:

```bash
source plugins/dev-team/utils/openspec-cli.sh

# Collect change status and instructions using the cache to avoid redundant calls
STATUS_JSON="$(openspec_cli_cache status "<name>")"
INSTRUCTIONS_JSON="$(openspec_cli_cache instructions "<name>")"
```

Construct the enriched Planner prompt by combining the static template reference with dynamic CLI output and optional explore context:

1. **Base prompt**: Start with the standard instruction.
2. **Static template**: Reference the template file path.
3. **Dynamic CLI instructions** (if available): If `INSTRUCTIONS_JSON` is not `{}`, append the `rules` and `context` fields from the instructions output. The `template` field is intentionally not injected -- the static template path provides the structure. If `INSTRUCTIONS_JSON` is `{}`, omit this section and rely solely on the static template.
4. **Explore context** (if available from Step 1): If `EXPLORE_CONTEXT_SUMMARY` is non-empty and less than 10KB, append it as a separate section. If it exceeds 10KB, truncate it and append:
   ```
   ## 探索上下文（仅供参考）

   <truncated content>

   （以下内容已截断，共 N 行）
   ```
5. **CRITICAL safeguard**: When injecting explore context, preface it with:
   ```
   探索上下文仅供参考，以 CLI 指令和静态模板为准。
   ```

The enriched prompt should look like this (note: only `rules` and `context` from INSTRUCTIONS_JSON are included; the `template` field is intentionally excluded):

```
Agent({
  description: "Write proposal.md and specs/",
  subagent_type: "requirements-planner",
  prompt: "Write proposal.md and specs/ for change '<name>'."

## CLI Instructions

The following dynamic context was provided by openspec for this change:

\`\`\`json
<INSTRUCTIONS_JSON content (rules and context only)>
\`\`\`

## 探索上下文（仅供参考）

探索上下文仅供参考，以 CLI 指令和静态模板为准。

<EXPLORE_CONTEXT_SUMMARY content>

（以下内容已截断，共 N 行）
"
})
```

**Fallback behavior**: If both `openspec status --json` and `openspec instructions` return empty JSON `{}`, fall back to the basic prompt without CLI instructions. The Planner will still produce valid output using the static template alone.

**3b. Invoke Evaluator:**
```
Agent({
  description: "Evaluate proposal.md",
  subagent_type: "requirements-evaluator",
  prompt: "Evaluate proposal.md for change '<name>'. Use your static checklist and append result to eval.json."
})
```

**3c. Check verdict:**
- Read the latest entry for phase "01-requirements" from eval.json
- If verdict is "pass": phase complete, proceed
- If verdict is "fail": re-invoke Planner with failed items from the eval entry, then re-run Evaluator
- Loop until pass or until user interrupts

### Step 4: Report result

Display the Evaluator's verdict, pass/total items, any notes, and a summary of specs/ files generated.

## DESIGN Phase Pattern (P→E)

- **Planner** (`requirements-planner`, opus, Read/Write): writes proposal.md and specs/ artifacts
- **Evaluator** (`requirements-evaluator`, opus, Read/Write): checks with static checklist, appends to eval.json
- **Loop**: if fail → Planner re-invoked with failed items → Evaluator re-runs
- **No Generator**: the Planner IS the producer for DESIGN phases
