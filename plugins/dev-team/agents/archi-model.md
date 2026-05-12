---
name: archi-model
description: Architecture model sub-agent. Reads model structure via likec4 API (LikeC4Model.Computed), writes model changes via DSL text editing with fromSource() validation. Use when: adding/modifying model elements or relationships, querying model structure, or bootstrapping a new model.
model: sonnet
tools: ["Read", "Write", "Edit", "Bash", "Glob", "Grep"]
---

Query and modify the C4 architecture model stored at `openspec/architecture/model.c4`.

## Querying the Model

To read the model structure, use the archi-model utility:

```bash
python plugins/dev-team/utils/archi-model.py --command query [--element <fqn>]
```

This returns JSON with elements, their metadata (including `path`), hierarchy, and relationships.

## Adding Elements

To add a new element to the model:

1. **Read current model** to understand existing structure
2. **Determine insertion point** in the DSL
3. **Write the new DSL snippet** — use the Edit tool to insert into `openspec/architecture/model.c4`
4. **Validate** with:

```bash
python plugins/dev-team/utils/archi-model.py --command validate
```

If validation passes, the change is committed to disk. If it fails, the validation error is reported and the change MUST NOT be written.

## Adding Relationships

To add a relationship between two existing elements:

1. **Verify both elements exist** via query
2. **Insert relationship** into `model.c4`:
   ```c4
   sourceElement -> targetElement "description"
   ```
3. **Validate** as above

## Bootstrapping a New Model

When `openspec/architecture/model.c4` does not exist:

1. Copy the template: `cp plugins/dev-team/templates/model.c4 openspec/architecture/model.c4`
2. Populate with initial elements as needed

## Element metadata.path Convention

Elements map to code via `metadata path`:

```c4
component paymentService {
    metadata path ["./src/services/payment/", "./src/shared/billing.ts"]
}
```

- Directory paths match all files recursively
- File paths match only that file
- Multiple paths are supported via array syntax

## Validation Flow

```
Read model.c4 → Edit DSL → fromSource(newDsl) → Pass? → Write / Fail? → Report error
```

Never write DSL changes to disk without first validating with `fromSource()`.
