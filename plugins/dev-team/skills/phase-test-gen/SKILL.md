---
name: phase-test-gen
description: |
  EXECUTION phase (G→E): test-gen-generator writes test files to disk (git diff IS the artifact), then test-gen-evaluator checks via git diff against test-design.md.
  Loops on fail until all required checklist items pass.
license: MIT
disable-model-invocation: true
metadata:
  author: dev-team
  version: "1.0"
---

Test generation phase — Generator writes test files to disk, Evaluator inspects git diff.

## Usage

```
/dev-team:phase-test-gen [change-name]
```

## Process

### Step 1: Detect active change

If a change name is provided, use it. Otherwise find the active change.

### Step 2: Verify prerequisite

Check that `openspec/changes/<name>/phases/test-design.md` exists. If not, direct user to run `/dev-team:phase-test-design` first.

### Step 3: G→E Loop

**3a. Invoke Generator:**
```
Agent({
  description: "Generate test files",
  subagent_type: "test-gen-generator",
  prompt: "Generate test skeleton files for change '<name>'. Read test-design.md from openspec/changes/<name>/phases/. Write test files directly to disk following existing project test conventions. The git diff of uncommitted changes IS the artifact — no JSON reports."
})
```

**3b. Read Path Validation (automatic after Generator completes):**
Before invoking the Evaluator, validate the Generator's Read tool calls against the file type blacklist:

```bash
# Check for forbidden source code file reads in bash history or tool calls
# The test-gen-generator should NOT have read any .ts, .py, .js, etc. files
# If violations found, record in eval.json findings and re-invoke Generator with warning
```

If Read violations are found:
- Record a finding in eval.json with phase_suffix "read-validation"
- Re-invoke Generator with specific instructions to avoid reading blacklisted files
- If violations persist after 3 attempts, set verdict "fail" with findings listing the violations

**3c. Invoke Evaluator:**
```
Agent({
  description: "Evaluate generated tests",
  subagent_type: "test-gen-evaluator",
  prompt: "Evaluate generated test code for change '<name>' against test-design.md. Run git diff to inspect the Generator's output. Use your static checklist and append result to eval.json."
})
```

**3d. Check verdict:**
- Read the latest entry for phase "04-test-gen" from eval.json
- If verdict is "pass": phase complete
- If verdict is "fail": re-invoke Generator with failed items and eval notes, re-run Evaluator
- Loop until pass or user interrupts

### Step 4: Report result

Display verdict, pass/total items, and notes.

## EXECUTION Phase Pattern (G→E)

- **Generator** (`test-gen-generator`, sonnet, Read/Write/Grep/Glob/Bash): writes test files to disk — git diff IS the artifact
- **Evaluator** (`test-gen-evaluator`, opus, Read/Write/Bash): inspects git diff + test-design.md, appends to eval.json
- **Loop**: if fail → Generator re-invoked with failed items → Evaluator re-runs
- **No JSON reports from Generator** — the code is self-documenting
